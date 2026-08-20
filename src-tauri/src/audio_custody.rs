//! Audio custody: taking ownership of the audio a project depends on, and
//! being able to say whether it is still there and still itself.
//!
//! FUNG's product promise is that source audio is protected before anything
//! derived from it. The code did not match that. Capture wrote chunk files and
//! recorded an absolute path; import recorded the *user's own* path and copied
//! nothing. Nothing ever re-read a stored digest, so a deleted, truncated or
//! replaced file stayed invisible until something tried to use it — and moving
//! a project folder silently orphaned every recording in it while the ledger
//! still reported them complete.
//!
//! This module supplies the three things custody actually requires:
//!
//! 1. [`take_custody_of_import`] — copy imported audio into the project before
//!    depending on it, and digest what landed.
//! 2. [`resolve_chunk_path`] — find a chunk whose project has moved, by
//!    rebuilding its location from the project root rather than trusting an
//!    absolute path recorded on another machine.
//! 3. [`verify_project_audio`] — read every chunk back and classify it against
//!    the digest recorded at capture time.
//!
//! Digests are streamed, never buffered whole: a three-hour capture is
//! gigabytes, and verification must not need that much memory to run.

use crate::genesis_adapter;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Path segments that mark where a project's own audio tree begins. A chunk
/// recorded as `D:\old\projects\p1\live\r1\chunks\mic-1.wav` is still findable
/// under a project that now lives at `E:\new\projects\p1`, because everything
/// from the anchor onward is project-relative by construction.
const PROJECT_AUDIO_ANCHORS: [&str; 2] = ["live", "imports"];

/// Read buffer for streaming digests. Large enough that a gigabyte file is not
/// a million syscalls, small enough to stay irrelevant to peak memory.
const DIGEST_BUFFER_BYTES: usize = 1024 * 1024;

/// Rows read per query. GenesisBlockDB rejects any limit outside `1..1000`.
const GENESIS_QUERY_LIMIT: u32 = crate::genesis_adapter::ROW_CAP;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CustodyError {
    #[error("the file to import could not be read")]
    SourceUnreadable,
    #[error("the project storage directory could not be prepared")]
    StorageUnavailable,
    #[error("the imported copy could not be written")]
    CopyFailed,
    #[error("the imported copy did not match the file that was read")]
    CopyMismatch,
    #[error("this project already holds a different file under that name")]
    DestinationOccupied,
    #[error("audio inventory could not be read from Genesis: {0}")]
    InventoryFailed(String),
}

/// Audio the project now owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustodiedAudio {
    pub(crate) stored_path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) byte_size: i64,
}

/// Streams a file and returns `(sha256, byte_count)` without holding it in
/// memory.
pub(crate) fn digest_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; DIGEST_BUFFER_BYTES];
    let mut total: u64 = 0;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

/// Keeps an imported filename recognisable without letting it choose path
/// syntax. Everything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize_file_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(120));
    for character in raw.chars() {
        if out.len() >= 120 {
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
            out.push(character);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('.').to_string();
    if trimmed.is_empty() {
        "imported-audio".to_string()
    } else {
        trimmed
    }
}

/// Where a recording's imported source lives inside its project.
pub(crate) fn import_destination(
    project_storage: &Path,
    recording_id: &str,
    source: &Path,
) -> PathBuf {
    let name = source
        .file_name()
        .map(|name| sanitize_file_name(&name.to_string_lossy()))
        .unwrap_or_else(|| "imported-audio".to_string());
    project_storage
        .join("imports")
        .join(sanitize_file_name(recording_id))
        .join(name)
}

/// Copies an imported file into the project and digests what landed.
///
/// The digest is taken by reading the destination back, not by hashing the
/// source in passing: it has to describe the bytes the project will actually
/// depend on, otherwise a partial or altered write would be recorded under a
/// digest that vouches for something else.
pub(crate) fn take_custody_of_import(
    project_storage: &Path,
    recording_id: &str,
    source: &Path,
) -> Result<CustodiedAudio, CustodyError> {
    let (source_digest, source_bytes) =
        digest_file(source).map_err(|_| CustodyError::SourceUnreadable)?;

    let destination = import_destination(project_storage, recording_id, source);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|_| CustodyError::StorageUnavailable)?;
    }

    // Re-importing the identical file is idempotent; a *different* file under
    // the same name is refused rather than silently overwriting audio some
    // other recording may already reference.
    if destination.exists() {
        match digest_file(&destination) {
            Ok((existing, bytes)) if existing == source_digest => {
                return Ok(CustodiedAudio {
                    stored_path: destination,
                    sha256: existing,
                    byte_size: bytes as i64,
                })
            }
            _ => return Err(CustodyError::DestinationOccupied),
        }
    }

    fs::copy(source, &destination).map_err(|_| CustodyError::CopyFailed)?;
    let (stored_digest, stored_bytes) = digest_file(&destination).map_err(|_| {
        let _ = fs::remove_file(&destination);
        CustodyError::CopyFailed
    })?;
    if stored_digest != source_digest || stored_bytes != source_bytes {
        // Never leave a copy the ledger would vouch for incorrectly.
        let _ = fs::remove_file(&destination);
        return Err(CustodyError::CopyMismatch);
    }

    Ok(CustodiedAudio {
        stored_path: destination,
        sha256: stored_digest,
        byte_size: stored_bytes as i64,
    })
}

/// Rebuilds a chunk's location under `project_storage` from the tail of a
/// recorded path, starting at the last project-audio anchor.
///
/// Returns `None` when the recorded path has no anchor — a path from outside
/// the project tree, which relocation cannot reason about.
pub(crate) fn relocated_candidate(project_storage: &Path, recorded: &str) -> Option<PathBuf> {
    // Recorded paths may use either separator, since a backup can be restored
    // on a different platform from the one that wrote it.
    let normalized = recorded.replace('\\', "/");
    let segments: Vec<&str> = normalized
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    let anchor = segments
        .iter()
        .rposition(|segment| PROJECT_AUDIO_ANCHORS.contains(segment))?;
    let mut candidate = project_storage.to_path_buf();
    for segment in &segments[anchor..] {
        // A recorded path is data. Refuse to let it climb out of the project.
        if *segment == ".." {
            return None;
        }
        candidate.push(segment);
    }
    Some(candidate)
}

/// Finds a chunk, preferring the recorded path and falling back to the same
/// position under the project's current root.
pub(crate) fn resolve_chunk_path(project_storage: &Path, recorded: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(recorded);
    if direct.is_file() {
        return Some(direct);
    }
    relocated_candidate(project_storage, recorded).filter(|candidate| candidate.is_file())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChunkState {
    /// Present at its recorded path, digest matches.
    Intact,
    /// Found under the project's current root instead, digest matches. The
    /// ledger row has been repaired to point at it.
    Relocated,
    /// Present and readable, but no longer the audio the transcript came from.
    Modified,
    /// Not at its recorded path and not under the project root.
    Missing,
    /// Recorded with no digest to check against, so presence is all that can
    /// be asserted.
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkFinding {
    pub(crate) chunk_id: String,
    pub(crate) recording_id: String,
    pub(crate) recorded_path: String,
    pub(crate) state: ChunkState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioIntegrityReport {
    pub(crate) checked: usize,
    pub(crate) intact: usize,
    pub(crate) relocated: usize,
    pub(crate) modified: usize,
    pub(crate) missing: usize,
    pub(crate) unverifiable: usize,
    /// Everything that is not `Intact`. A clean project reports an empty list
    /// rather than a summary the caller has to interpret.
    pub(crate) problems: Vec<ChunkFinding>,
}

impl AudioIntegrityReport {
    /// True when every chunk the ledger references is present and matches its
    /// recorded digest.
    pub(crate) fn is_clean(&self) -> bool {
        self.missing == 0 && self.modified == 0
    }

    fn record(&mut self, finding: ChunkFinding) {
        self.checked += 1;
        match finding.state {
            ChunkState::Intact => self.intact += 1,
            ChunkState::Relocated => self.relocated += 1,
            ChunkState::Modified => self.modified += 1,
            ChunkState::Missing => self.missing += 1,
            ChunkState::Unverifiable => self.unverifiable += 1,
        }
        if finding.state != ChunkState::Intact {
            self.problems.push(finding);
        }
    }
}

/// Classifies one chunk. Pure apart from reading the file, so the decision
/// table is testable without a ledger.
pub(crate) fn classify_chunk(
    project_storage: &Path,
    recorded_path: &str,
    expected_digest: Option<&str>,
) -> (ChunkState, Option<PathBuf>) {
    let Some(found) = resolve_chunk_path(project_storage, recorded_path) else {
        return (ChunkState::Missing, None);
    };
    let moved = found.as_path() != Path::new(recorded_path);
    let Some(expected) = expected_digest.filter(|digest| !digest.is_empty()) else {
        return (ChunkState::Unverifiable, Some(found));
    };
    match digest_file(&found) {
        Ok((actual, _)) if actual.eq_ignore_ascii_case(expected) => {
            let state = if moved {
                ChunkState::Relocated
            } else {
                ChunkState::Intact
            };
            (state, Some(found))
        }
        Ok(_) => (ChunkState::Modified, Some(found)),
        // Resolvable but unreadable is indistinguishable from absent for any
        // purpose that matters here.
        Err(_) => (ChunkState::Missing, None),
    }
}

/// Reads every chunk of a project back and reports what is still there.
///
/// A chunk found under the project's current root with a matching digest has
/// its row repaired to the new path. That is safe precisely because the digest
/// proves identity — the file is not merely *plausibly* the right one. Nothing
/// is repaired on a digest mismatch.
pub(crate) fn verify_project_audio(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> Result<AudioIntegrityReport, CustodyError> {
    let project = genesis_adapter::query(
        storage,
        "projects",
        &["storage_path"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(CustodyError::InventoryFailed)?;
    let storage_path = project
        .first()
        .and_then(|row| row.get("projects.storage_path"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let project_storage = PathBuf::from(&storage_path);

    let recordings = genesis_adapter::query(
        storage,
        "recordings",
        &["id"],
        vec![genesis_adapter::eq(
            "recordings",
            "project_id",
            serde_json::json!(project_id),
        )],
        GENESIS_QUERY_LIMIT,
    )
    .map_err(CustodyError::InventoryFailed)?;

    let mut report = AudioIntegrityReport::default();
    let mut repairs = Vec::new();

    for recording in &recordings {
        let Some(recording_id) = recording
            .get("recordings.id")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let chunks = genesis_adapter::query(
            storage,
            "audio_chunks",
            &[
                "id",
                "sequence_no",
                "file_path",
                "start_ms",
                "end_ms",
                "byte_size",
                "checksum",
                "created_at",
            ],
            vec![genesis_adapter::eq(
                "audio_chunks",
                "recording_id",
                serde_json::json!(recording_id),
            )],
            GENESIS_QUERY_LIMIT,
        )
        .map_err(CustodyError::InventoryFailed)?;

        for chunk in &chunks {
            let text = |key: &str| {
                chunk
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let recorded_path = text("audio_chunks.file_path");
            let checksum = text("audio_chunks.checksum");
            let (state, found) =
                classify_chunk(&project_storage, &recorded_path, Some(checksum.as_str()));

            if state == ChunkState::Relocated {
                if let Some(found) = &found {
                    // Whole-row upsert: `commit_rows` replaces the row, so
                    // every column has to be restated, not just the path.
                    repairs.push(genesis_adapter::upsert(
                        "audio_chunks",
                        serde_json::json!({
                            "id": text("audio_chunks.id"),
                            "recording_id": recording_id,
                            "sequence_no": chunk.get("audio_chunks.sequence_no").cloned().unwrap_or(serde_json::Value::from(0)),
                            "file_path": found.display().to_string(),
                            "start_ms": chunk.get("audio_chunks.start_ms").cloned().unwrap_or(serde_json::Value::from(0)),
                            "end_ms": chunk.get("audio_chunks.end_ms").cloned().unwrap_or(serde_json::Value::from(0)),
                            "byte_size": chunk.get("audio_chunks.byte_size").cloned().unwrap_or(serde_json::Value::from(0)),
                            "checksum": checksum,
                            "created_at": text("audio_chunks.created_at"),
                        }),
                    ));
                }
            }

            report.record(ChunkFinding {
                chunk_id: text("audio_chunks.id"),
                recording_id: recording_id.to_string(),
                recorded_path,
                state,
            });
        }
    }

    if !repairs.is_empty() {
        genesis_adapter::commit_rows(storage, repairs).map_err(CustodyError::InventoryFailed)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn an_import_is_copied_into_the_project_and_digested() {
        // Regression: `import_and_transcribe` recorded the user's own path and
        // copied nothing, so deleting the source invalidated a recording the
        // ledger still reported completed.
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("elsewhere").join("meeting.m4a");
        write(&source, b"imported audio bytes");
        let project = temp.path().join("projects").join("p1");

        let custodied = take_custody_of_import(&project, "rec-1", &source).unwrap();

        assert_eq!(
            custodied.stored_path,
            project.join("imports").join("rec-1").join("meeting.m4a")
        );
        assert!(custodied.stored_path.is_file());
        assert_eq!(custodied.byte_size, 20);
        assert_eq!(
            custodied.sha256,
            digest_file(&source).unwrap().0,
            "the stored digest must describe the bytes the project now owns"
        );

        // The project survives losing the original.
        fs::remove_file(&source).unwrap();
        assert_eq!(
            fs::read(&custodied.stored_path).unwrap(),
            b"imported audio bytes"
        );
    }

    #[test]
    fn re_importing_the_same_file_is_idempotent_but_a_different_one_is_refused() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("p1");
        let source = temp.path().join("a").join("clip.wav");
        write(&source, b"original");

        let first = take_custody_of_import(&project, "rec-1", &source).unwrap();
        let again = take_custody_of_import(&project, "rec-1", &source).unwrap();
        assert_eq!(first, again);

        // Same filename, different bytes: refuse rather than overwrite audio
        // another row may already vouch for.
        let other = temp.path().join("b").join("clip.wav");
        write(&other, b"a completely different recording");
        assert_eq!(
            take_custody_of_import(&project, "rec-1", &other),
            Err(CustodyError::DestinationOccupied)
        );
        assert_eq!(fs::read(&first.stored_path).unwrap(), b"original");
    }

    #[test]
    fn a_moved_project_can_still_find_its_chunks() {
        // The defect this exists for: absolute paths recorded on one machine
        // orphan every recording when the project folder moves.
        let temp = TempDir::new().unwrap();
        let moved_project = temp.path().join("new-home").join("p1");
        let chunk = moved_project
            .join("live")
            .join("rec-1")
            .join("chunks")
            .join("mic-00001.wav");
        write(&chunk, b"capture bytes");

        let recorded = r"D:\old-home\projects\p1\live\rec-1\chunks\mic-00001.wav";
        let resolved = resolve_chunk_path(&moved_project, recorded).expect("must relocate");
        assert_eq!(resolved, chunk);
    }

    #[test]
    fn relocation_never_climbs_out_of_the_project() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("p1");
        // A recorded path is ledger data, not a trusted constant.
        assert_eq!(
            relocated_candidate(&project, "live/../../../etc/passwd"),
            None
        );
        assert_eq!(
            relocated_candidate(&project, "/tmp/no-anchor/file.wav"),
            None
        );
    }

    #[test]
    fn classification_separates_intact_relocated_modified_and_missing() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("p1");

        let here = project.join("live").join("r1").join("chunks").join("a.wav");
        write(&here, b"aaa");
        let digest = digest_file(&here).unwrap().0;

        // Present where recorded, digest matches.
        let (state, _) = classify_chunk(&project, &here.display().to_string(), Some(&digest));
        assert_eq!(state, ChunkState::Intact);

        // Recorded elsewhere, found under the project root, digest matches.
        let (state, found) = classify_chunk(
            &project,
            r"D:\gone\projects\p1\live\r1\chunks\a.wav",
            Some(&digest),
        );
        assert_eq!(state, ChunkState::Relocated);
        assert_eq!(found.unwrap(), here);

        // Present, but no longer the audio the transcript came from.
        let (state, _) = classify_chunk(
            &project,
            &here.display().to_string(),
            Some(&"ff".repeat(32)),
        );
        assert_eq!(state, ChunkState::Modified);

        // Gone entirely.
        let (state, _) = classify_chunk(
            &project,
            &project
                .join("live")
                .join("r1")
                .join("chunks")
                .join("vanished.wav")
                .display()
                .to_string(),
            Some(&digest),
        );
        assert_eq!(state, ChunkState::Missing);

        // No digest recorded: presence is all that can honestly be claimed.
        let (state, _) = classify_chunk(&project, &here.display().to_string(), None);
        assert_eq!(state, ChunkState::Unverifiable);
    }

    #[test]
    fn a_report_is_clean_only_when_nothing_is_missing_or_modified() {
        let mut report = AudioIntegrityReport::default();
        report.record(ChunkFinding {
            chunk_id: "c1".into(),
            recording_id: "r1".into(),
            recorded_path: "a".into(),
            state: ChunkState::Intact,
        });
        assert!(report.is_clean());
        assert!(report.problems.is_empty(), "intact chunks are not problems");

        report.record(ChunkFinding {
            chunk_id: "c2".into(),
            recording_id: "r1".into(),
            recorded_path: "b".into(),
            state: ChunkState::Relocated,
        });
        // A relocated chunk is still intact audio; it is listed so the move is
        // visible, but it does not make the project unclean.
        assert!(report.is_clean());
        assert_eq!(report.problems.len(), 1);

        report.record(ChunkFinding {
            chunk_id: "c3".into(),
            recording_id: "r1".into(),
            recorded_path: "c".into(),
            state: ChunkState::Missing,
        });
        assert!(!report.is_clean());
        assert_eq!(report.checked, 3);
        assert_eq!(report.missing, 1);
    }

    #[test]
    fn digests_stream_rather_than_buffering_the_whole_file() {
        // Guards the property, not the implementation: a file larger than the
        // read buffer must digest identically to the same bytes in memory.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("big.wav");
        let bytes: Vec<u8> = (0..(DIGEST_BUFFER_BYTES * 2 + 12345))
            .map(|index| (index % 251) as u8)
            .collect();
        write(&path, &bytes);

        let (digest, size) = digest_file(&path).unwrap();
        assert_eq!(size, bytes.len() as u64);
        assert_eq!(digest, format!("{:x}", Sha256::digest(&bytes)));
    }

    #[test]
    fn an_import_name_cannot_choose_path_syntax() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("p1");
        let destination =
            import_destination(&project, "../../rec", Path::new("../../../evil .wav"));
        assert!(destination.starts_with(&project));
        assert_eq!(
            destination,
            project.join("imports").join("_.._rec").join("evil_.wav")
        );
    }
}
