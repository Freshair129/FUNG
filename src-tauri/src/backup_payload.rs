//! Backup payload container: the plaintext that the archive envelope
//! encrypts.
//!
//! Before this module existed the encrypted plaintext *was* the Genesis
//! bundle, and nothing else. That made a "verified" archive a database-only
//! backup: `audio_chunks` rows survived a restore while the WAV files they
//! name — which live on the filesystem, not inside Genesis — did not. A clean
//! machine restored transcripts pointing at paths that had never existed on
//! it.
//!
//! The container fixes that by carrying the Genesis bundle *and* every audio
//! file the ledger references, under one authenticated envelope. Layout:
//!
//! ```text
//! "FUNGPL01"            8 bytes  magic
//! version               u16 LE
//! manifest_len          u32 LE
//! manifest              manifest_len bytes of UTF-8 JSON
//! genesis bundle        manifest.genesis_bundle.byte_count bytes
//! audio payloads        concatenated, in manifest.audio order,
//!                       only for entries whose status is `stored`
//! ```
//!
//! The envelope above this layer already provides confidentiality and
//! authentication, so the container itself carries no crypto — only digests,
//! which let a restore prove each extracted file is byte-identical to what
//! the backup read.
//!
//! **Legacy archives.** Plaintext that does not begin with [`MAGIC`] is a
//! pre-container archive: the whole blob is the Genesis bundle and there is
//! no audio. [`unpack`] handles that shape so archives written before this
//! change stay restorable.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const MAGIC: &[u8; 8] = b"FUNGPL01";
const CONTAINER_VERSION: u16 = 1;
const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

/// Ceiling on the assembled plaintext (Genesis bundle plus every audio file).
///
/// The envelope encrypts from a single in-memory buffer, so the whole payload
/// is resident at once. This bound turns "the machine ran out of memory
/// somewhere inside a backup" into a named, actionable failure that leaves the
/// previous verified archive untouched. It is an interim guard: lifting it
/// means teaching [`crate::backup_archive`] to encrypt from a reader, which is
/// a separate change.
pub(crate) const MAX_PAYLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Longest single path component kept when deriving an archive-relative name.
const MAX_NAME_LEN: usize = 96;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum PayloadError {
    #[error("backup payload is malformed")]
    Malformed,
    #[error("backup payload version is unsupported")]
    UnsupportedVersion,
    #[error("backup payload entry path is unsafe")]
    UnsafeEntryPath,
    #[error("backup payload digest mismatch")]
    DigestMismatch,
    #[error(
        "backup payload would be {actual} bytes, above the {MAX_PAYLOAD_BYTES} byte in-memory limit"
    )]
    TooLarge { actual: u64 },
    #[error("backup payload could not be written to the restore target")]
    ExtractFailed,
}

/// Why a referenced audio file is not in the archive.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AudioOmission {
    /// The ledger names a path that could not be read at backup time.
    Unreadable,
    /// The file was read but did not match the digest the ledger recorded,
    /// so it is no longer the audio the transcript was derived from.
    DigestMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AudioRole {
    /// A durable capture chunk from `audio_chunks`.
    Chunk,
    /// A recording's `canonical_audio_path` — the whole-file source used by
    /// imports, which have no chunk rows.
    Canonical,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioEntry {
    /// Archive-relative destination, always forward-slashed and always below
    /// `audio/`. Generated here, and re-validated on unpack.
    pub(crate) relative_path: String,
    /// Absolute path the file occupied on the machine that made the backup.
    /// Kept so a future in-place restore can relink `audio_chunks.file_path`
    /// without guessing; nothing reads it during extraction.
    pub(crate) source_path: String,
    pub(crate) recording_id: String,
    pub(crate) role: AudioRole,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
    /// `None` when the bytes are in the archive; `Some(_)` records a file the
    /// ledger referenced that this archive could not carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) omitted: Option<AudioOmission>,
}

impl AudioEntry {
    fn is_stored(&self) -> bool {
        self.omitted.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleEntry {
    byte_count: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PayloadManifest {
    format_version: u16,
    genesis_bundle: BundleEntry,
    audio: Vec<AudioEntry>,
}

/// One audio file read from disk and ready to pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StagedAudio {
    pub(crate) entry: AudioEntry,
    pub(crate) bytes: Vec<u8>,
}

/// What a backup found when it walked the ledger's audio references.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AudioInventory {
    pub(crate) staged: Vec<StagedAudio>,
    /// Entries the archive cannot carry. They are still written into the
    /// manifest so a restore can report what was already lost at backup time
    /// rather than silently presenting an incomplete project as whole.
    pub(crate) omitted: Vec<AudioEntry>,
}

impl AudioInventory {
    pub(crate) fn stored_count(&self) -> usize {
        self.staged.len()
    }

    pub(crate) fn omitted_count(&self) -> usize {
        self.omitted.len()
    }

    pub(crate) fn stored_bytes(&self) -> u64 {
        self.staged.iter().map(|file| file.entry.byte_count).sum()
    }
}

/// A decrypted container, split back into its parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnpackedPayload {
    pub(crate) genesis_bundle: Vec<u8>,
    /// Every audio entry the archive knows about, stored and omitted alike.
    pub(crate) audio: Vec<AudioEntry>,
    /// Bytes for the stored entries, in `audio` order (omitted entries have
    /// no slot here).
    stored_bytes: Vec<Vec<u8>>,
}

impl UnpackedPayload {
    pub(crate) fn omitted(&self) -> impl Iterator<Item = &AudioEntry> {
        self.audio.iter().filter(|entry| !entry.is_stored())
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> Result<u16, PayloadError> {
    let slice = take(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, PayloadError> {
    let slice = take(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn take<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8], PayloadError> {
    let end = offset.checked_add(len).ok_or(PayloadError::Malformed)?;
    let slice = bytes.get(*offset..end).ok_or(PayloadError::Malformed)?;
    *offset = end;
    Ok(slice)
}

/// Rejects anything that could escape the extraction root: absolute paths,
/// drive prefixes, `..`, `.`, empty components, and backslashes (which are a
/// separator on Windows but an ordinary character in an archive string, so a
/// crafted `a\..\..\b` must not survive). Paths this module generates are safe
/// by construction; this exists because an archive is authenticated, not
/// trusted — anyone holding the recovery phrase can author one.
fn validate_relative_path(candidate: &str) -> Result<PathBuf, PayloadError> {
    if candidate.is_empty()
        || candidate.contains('\\')
        || candidate.contains('\0')
        || !candidate.starts_with("audio/")
    {
        return Err(PayloadError::UnsafeEntryPath);
    }
    let path = Path::new(candidate);
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                if part.is_empty() {
                    return Err(PayloadError::UnsafeEntryPath);
                }
            }
            _ => return Err(PayloadError::UnsafeEntryPath),
        }
    }
    Ok(path.to_path_buf())
}

/// Keeps a filename recognisable after a restore without letting ledger data
/// choose path syntax. Everything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_NAME_LEN));
    for character in raw.chars() {
        if out.len() >= MAX_NAME_LEN {
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
        "file".to_string()
    } else {
        trimmed
    }
}

fn file_name_of(source_path: &str) -> String {
    let raw = Path::new(source_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        // A path with no final component still needs a slot in the archive.
        .unwrap_or_else(|| "file".to_string());
    sanitize_component(&raw)
}

/// Builds the archive-relative destination for one source file, disambiguating
/// against names already claimed in this archive. Two chunks that sanitize to
/// the same name (different directories, or characters that both fold to `_`)
/// must not overwrite each other on extraction.
pub(crate) fn relative_path_for(
    recording_id: &str,
    role: AudioRole,
    source_path: &str,
    claimed: &mut HashSet<String>,
) -> String {
    let folder = match role {
        AudioRole::Chunk => "chunks",
        AudioRole::Canonical => "source",
    };
    let recording = sanitize_component(recording_id);
    let name = file_name_of(source_path);
    let base = format!("audio/{recording}/{folder}/{name}");
    if claimed.insert(base.clone()) {
        return base;
    }
    for suffix in 1..u32::MAX {
        let candidate = format!("audio/{recording}/{folder}/{suffix}-{name}");
        if claimed.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("u32 range exhausted while disambiguating one archive entry")
}

/// Reads one referenced audio file and classifies it. `expected_sha256` is the
/// digest the ledger recorded at capture time; when present and different, the
/// file on disk is no longer the audio the transcript came from, so it is
/// recorded as omitted rather than packed under a digest that would lie.
pub(crate) fn stage_audio_file(
    source_path: &str,
    recording_id: &str,
    role: AudioRole,
    expected_sha256: Option<&str>,
    relative_path: String,
) -> Result<StagedAudio, AudioEntry> {
    let omit = |omission: AudioOmission, byte_count: u64, sha256: String| AudioEntry {
        relative_path: relative_path.clone(),
        source_path: source_path.to_string(),
        recording_id: recording_id.to_string(),
        role,
        byte_count,
        sha256,
        omitted: Some(omission),
    };

    let bytes = match fs::read(source_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(omit(
                AudioOmission::Unreadable,
                0,
                expected_sha256.unwrap_or_default().to_string(),
            ))
        }
    };
    let digest = sha256_hex(&bytes);
    if let Some(expected) = expected_sha256 {
        // Genesis stores the capture-time digest; only compare when the ledger
        // actually has one (imports do not).
        if !expected.is_empty() && !expected.eq_ignore_ascii_case(&digest) {
            return Err(omit(
                AudioOmission::DigestMismatch,
                bytes.len() as u64,
                digest,
            ));
        }
    }
    Ok(StagedAudio {
        entry: AudioEntry {
            relative_path,
            source_path: source_path.to_string(),
            recording_id: recording_id.to_string(),
            role,
            byte_count: bytes.len() as u64,
            sha256: digest,
            omitted: None,
        },
        bytes,
    })
}

/// Assembles the plaintext the envelope will encrypt.
pub(crate) fn pack(
    genesis_bundle: &[u8],
    inventory: &AudioInventory,
) -> Result<Vec<u8>, PayloadError> {
    let audio_bytes: u64 = inventory.stored_bytes();
    let total = genesis_bundle.len() as u64 + audio_bytes;
    if total > MAX_PAYLOAD_BYTES {
        return Err(PayloadError::TooLarge { actual: total });
    }

    // Stored entries first, then omitted ones: the reader walks stored entries
    // in manifest order against a single byte stream, so keeping them
    // contiguous makes that walk independent of how the caller ordered them.
    let mut audio: Vec<AudioEntry> = inventory
        .staged
        .iter()
        .map(|file| file.entry.clone())
        .collect();
    audio.extend(inventory.omitted.iter().cloned());

    let manifest = PayloadManifest {
        format_version: CONTAINER_VERSION,
        genesis_bundle: BundleEntry {
            byte_count: genesis_bundle.len() as u64,
            sha256: sha256_hex(genesis_bundle),
        },
        audio,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| PayloadError::Malformed)?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(PayloadError::TooLarge {
            actual: manifest_bytes.len() as u64,
        });
    }

    let mut out = Vec::with_capacity(
        MAGIC.len() + 2 + 4 + manifest_bytes.len() + genesis_bundle.len() + audio_bytes as usize,
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
    out.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&manifest_bytes);
    out.extend_from_slice(genesis_bundle);
    for file in &inventory.staged {
        out.extend_from_slice(&file.bytes);
    }
    Ok(out)
}

/// Splits a decrypted plaintext back into the Genesis bundle and its audio.
/// Plaintext without the container magic is a pre-container archive and is
/// returned as a bundle with no audio.
pub(crate) fn unpack(plaintext: &[u8]) -> Result<UnpackedPayload, PayloadError> {
    if plaintext.len() < MAGIC.len() || &plaintext[..MAGIC.len()] != MAGIC {
        return Ok(UnpackedPayload {
            genesis_bundle: plaintext.to_vec(),
            audio: Vec::new(),
            stored_bytes: Vec::new(),
        });
    }

    let mut offset = MAGIC.len();
    let version = read_u16(plaintext, &mut offset)?;
    if version != CONTAINER_VERSION {
        return Err(PayloadError::UnsupportedVersion);
    }
    let manifest_len = read_u32(plaintext, &mut offset)? as usize;
    if manifest_len > MAX_MANIFEST_BYTES {
        return Err(PayloadError::Malformed);
    }
    let manifest_bytes = take(plaintext, &mut offset, manifest_len)?;
    let manifest: PayloadManifest =
        serde_json::from_slice(manifest_bytes).map_err(|_| PayloadError::Malformed)?;
    if manifest.format_version != version {
        return Err(PayloadError::Malformed);
    }

    let bundle_len = usize::try_from(manifest.genesis_bundle.byte_count)
        .map_err(|_| PayloadError::Malformed)?;
    let genesis_bundle = take(plaintext, &mut offset, bundle_len)?.to_vec();
    if sha256_hex(&genesis_bundle) != manifest.genesis_bundle.sha256 {
        return Err(PayloadError::DigestMismatch);
    }

    let mut stored_bytes = Vec::new();
    let mut claimed: HashSet<&str> = HashSet::new();
    for entry in &manifest.audio {
        validate_relative_path(&entry.relative_path)?;
        if !claimed.insert(entry.relative_path.as_str()) {
            // Two entries mapping to one destination would make extraction
            // order decide which file survives.
            return Err(PayloadError::UnsafeEntryPath);
        }
        if !entry.is_stored() {
            continue;
        }
        let len = usize::try_from(entry.byte_count).map_err(|_| PayloadError::Malformed)?;
        let bytes = take(plaintext, &mut offset, len)?.to_vec();
        if sha256_hex(&bytes) != entry.sha256 {
            return Err(PayloadError::DigestMismatch);
        }
        stored_bytes.push(bytes);
    }
    if offset != plaintext.len() {
        return Err(PayloadError::Malformed);
    }

    Ok(UnpackedPayload {
        genesis_bundle,
        audio: manifest.audio,
        stored_bytes,
    })
}

/// Summary of what a restore actually put on disk.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioRestoreSummary {
    pub(crate) restored_file_count: usize,
    pub(crate) restored_byte_count: u64,
    /// Files the archive recorded but could not carry, because they were
    /// already unreadable or already modified when the backup ran.
    pub(crate) omitted_file_count: usize,
}

/// Writes every stored audio file below `target_root`, re-verifying each
/// digest as it lands. `target_root` is the freshly created restore target,
/// so nothing here can overwrite live project data.
pub(crate) fn extract_audio(
    target_root: &Path,
    payload: &UnpackedPayload,
) -> Result<AudioRestoreSummary, PayloadError> {
    let mut summary = AudioRestoreSummary {
        omitted_file_count: payload.omitted().count(),
        ..AudioRestoreSummary::default()
    };
    let mut stored = payload.stored_bytes.iter();
    for entry in payload.audio.iter().filter(|entry| entry.is_stored()) {
        let relative = validate_relative_path(&entry.relative_path)?;
        let destination = target_root.join(&relative);
        // `relative` is component-checked above, so this only guards against a
        // symlinked ancestor inside the freshly created target.
        if !destination.starts_with(target_root) {
            return Err(PayloadError::UnsafeEntryPath);
        }
        let bytes = stored.next().ok_or(PayloadError::Malformed)?;
        if sha256_hex(bytes) != entry.sha256 {
            return Err(PayloadError::DigestMismatch);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| PayloadError::ExtractFailed)?;
        }
        fs::write(&destination, bytes).map_err(|_| PayloadError::ExtractFailed)?;
        summary.restored_file_count += 1;
        summary.restored_byte_count += entry.byte_count;
    }
    // A manifest that promised more stored entries than the byte stream held
    // would already have failed in `unpack`; this catches the inverse.
    if stored.next().is_some() {
        return Err(PayloadError::Malformed);
    }
    Ok(summary)
}

/// Writes the archive's audio manifest beside the extracted files, so a
/// person recovering a project can see every original path, digest and
/// omission without decrypting the archive again.
pub(crate) fn write_audio_manifest(
    target_root: &Path,
    payload: &UnpackedPayload,
) -> Result<(), PayloadError> {
    if payload.audio.is_empty() {
        return Ok(());
    }
    let manifest_path = target_root.join("audio-manifest.json");
    let body =
        serde_json::to_vec_pretty(&payload.audio).map_err(|_| PayloadError::ExtractFailed)?;
    fs::write(manifest_path, body).map_err(|_| PayloadError::ExtractFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn staged(relative: &str, bytes: &[u8]) -> StagedAudio {
        StagedAudio {
            entry: AudioEntry {
                relative_path: relative.to_string(),
                source_path: format!("D:/orig/{relative}"),
                recording_id: "rec-1".to_string(),
                role: AudioRole::Chunk,
                byte_count: bytes.len() as u64,
                sha256: sha256_hex(bytes),
                omitted: None,
            },
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn round_trip_carries_the_bundle_and_every_audio_file() {
        let inventory = AudioInventory {
            staged: vec![
                staged("audio/rec-1/chunks/mic-00001.wav", b"first chunk bytes"),
                staged("audio/rec-1/chunks/mic-00002.wav", b"second chunk bytes"),
            ],
            omitted: vec![],
        };
        let packed = pack(b"genesis-bundle-bytes", &inventory).expect("pack");
        let payload = unpack(&packed).expect("unpack");

        assert_eq!(payload.genesis_bundle, b"genesis-bundle-bytes");
        assert_eq!(payload.stored_bytes.len(), 2);
        assert_eq!(payload.audio.len(), 2);
        assert_eq!(payload.stored_bytes[0], b"first chunk bytes");
        assert_eq!(payload.stored_bytes[1], b"second chunk bytes");
    }

    #[test]
    fn a_pre_container_archive_still_restores_as_a_bundle_without_audio() {
        // Archives written before this module existed encrypted the raw
        // Genesis bundle; they must keep restoring.
        let legacy = b"raw-genesis-bundle-with-no-container-header".to_vec();
        let payload = unpack(&legacy).expect("legacy unpack");
        assert_eq!(payload.genesis_bundle, legacy);
        assert!(payload.audio.is_empty());
        assert_eq!(payload.stored_bytes.len(), 0);
    }

    #[test]
    fn omitted_entries_are_recorded_but_carry_no_bytes() {
        let inventory = AudioInventory {
            staged: vec![staged("audio/rec-1/chunks/kept.wav", b"kept")],
            omitted: vec![AudioEntry {
                relative_path: "audio/rec-1/chunks/gone.wav".to_string(),
                source_path: "D:/orig/gone.wav".to_string(),
                recording_id: "rec-1".to_string(),
                role: AudioRole::Chunk,
                byte_count: 0,
                sha256: "deadbeef".to_string(),
                omitted: Some(AudioOmission::Unreadable),
            }],
        };
        let packed = pack(b"bundle", &inventory).expect("pack");
        let payload = unpack(&packed).expect("unpack");

        assert_eq!(payload.stored_bytes.len(), 1);
        assert_eq!(payload.audio.len(), 2);
        assert_eq!(payload.omitted().count(), 1);
        assert_eq!(
            payload.omitted().next().unwrap().omitted,
            Some(AudioOmission::Unreadable)
        );
    }

    #[test]
    fn a_corrupted_audio_payload_fails_instead_of_restoring_wrong_bytes() {
        let inventory = AudioInventory {
            staged: vec![staged("audio/rec-1/chunks/mic.wav", b"original audio")],
            omitted: vec![],
        };
        let mut packed = pack(b"bundle", &inventory).expect("pack");
        let last = packed.len() - 1;
        packed[last] ^= 0xff;

        assert_eq!(unpack(&packed), Err(PayloadError::DigestMismatch));
    }

    #[test]
    fn a_corrupted_genesis_bundle_fails_the_whole_payload() {
        let inventory = AudioInventory::default();
        let mut packed = pack(b"bundle-bytes", &inventory).expect("pack");
        let position = packed.len() - 3;
        packed[position] ^= 0xff;

        assert_eq!(unpack(&packed), Err(PayloadError::DigestMismatch));
    }

    #[test]
    fn traversal_and_absolute_entry_paths_are_rejected() {
        for hostile in [
            "audio/../../escape.wav",
            "../audio/escape.wav",
            "/etc/passwd",
            "audio/a\\..\\..\\escape.wav",
            "chunks/not-under-audio.wav",
            "",
        ] {
            assert_eq!(
                validate_relative_path(hostile),
                Err(PayloadError::UnsafeEntryPath),
                "must reject {hostile:?}"
            );
        }
        assert!(validate_relative_path("audio/rec-1/chunks/mic-00001.wav").is_ok());
    }

    #[test]
    fn two_entries_claiming_one_destination_are_rejected() {
        let inventory = AudioInventory {
            staged: vec![
                staged("audio/rec-1/chunks/same.wav", b"one"),
                staged("audio/rec-1/chunks/same.wav", b"two"),
            ],
            omitted: vec![],
        };
        let packed = pack(b"bundle", &inventory).expect("pack");
        assert_eq!(unpack(&packed), Err(PayloadError::UnsafeEntryPath));
    }

    #[test]
    fn colliding_source_names_get_distinct_archive_paths() {
        let mut claimed = HashSet::new();
        let first = relative_path_for("rec-1", AudioRole::Chunk, "D:/a/mic.wav", &mut claimed);
        let second = relative_path_for("rec-1", AudioRole::Chunk, "D:/b/mic.wav", &mut claimed);
        assert_ne!(first, second);
        assert_eq!(first, "audio/rec-1/chunks/mic.wav");
        assert_eq!(second, "audio/rec-1/chunks/1-mic.wav");
    }

    #[test]
    fn ledger_supplied_names_cannot_choose_path_syntax() {
        let mut claimed = HashSet::new();
        let path = relative_path_for(
            "../../rec",
            AudioRole::Canonical,
            "D:/x/../../evil name.wav",
            &mut claimed,
        );
        assert_eq!(path, "audio/_.._rec/source/evil_name.wav");
        assert!(validate_relative_path(&path).is_ok());
    }

    #[test]
    fn extraction_writes_verified_files_below_the_target_root() {
        let temp = TempDir::new().expect("temp dir");
        let inventory = AudioInventory {
            staged: vec![
                staged("audio/rec-1/chunks/mic-00001.wav", b"chunk one"),
                staged("audio/rec-1/source/import.m4a", b"imported source"),
            ],
            omitted: vec![AudioEntry {
                relative_path: "audio/rec-1/chunks/lost.wav".to_string(),
                source_path: "D:/orig/lost.wav".to_string(),
                recording_id: "rec-1".to_string(),
                role: AudioRole::Chunk,
                byte_count: 0,
                sha256: String::new(),
                omitted: Some(AudioOmission::Unreadable),
            }],
        };
        let payload = unpack(&pack(b"bundle", &inventory).expect("pack")).expect("unpack");

        let summary = extract_audio(temp.path(), &payload).expect("extract");
        assert_eq!(summary.restored_file_count, 2);
        assert_eq!(summary.restored_byte_count, 9 + 15);
        assert_eq!(summary.omitted_file_count, 1);

        assert_eq!(
            fs::read(temp.path().join("audio/rec-1/chunks/mic-00001.wav")).expect("chunk"),
            b"chunk one"
        );
        assert_eq!(
            fs::read(temp.path().join("audio/rec-1/source/import.m4a")).expect("source"),
            b"imported source"
        );
        assert!(!temp.path().join("audio/rec-1/chunks/lost.wav").exists());
    }

    #[test]
    fn staging_reports_a_missing_file_instead_of_failing_the_backup() {
        let outcome = stage_audio_file(
            "D:/definitely/not/here.wav",
            "rec-1",
            AudioRole::Chunk,
            Some("aa".repeat(32).as_str()),
            "audio/rec-1/chunks/here.wav".to_string(),
        );
        let entry = outcome.expect_err("missing file must be reported as omitted");
        assert_eq!(entry.omitted, Some(AudioOmission::Unreadable));
        assert_eq!(entry.byte_count, 0);
    }

    #[test]
    fn staging_reports_a_file_that_no_longer_matches_its_ledger_digest() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("mic.wav");
        fs::write(&path, b"edited since capture").expect("write");

        let outcome = stage_audio_file(
            path.to_str().expect("utf-8 path"),
            "rec-1",
            AudioRole::Chunk,
            Some(&sha256_hex(b"original capture bytes")),
            "audio/rec-1/chunks/mic.wav".to_string(),
        );
        let entry = outcome.expect_err("digest mismatch must be reported as omitted");
        assert_eq!(entry.omitted, Some(AudioOmission::DigestMismatch));
    }

    #[test]
    fn staging_accepts_a_file_matching_its_ledger_digest() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("mic.wav");
        fs::write(&path, b"capture bytes").expect("write");

        let staged = stage_audio_file(
            path.to_str().expect("utf-8 path"),
            "rec-1",
            AudioRole::Chunk,
            Some(&sha256_hex(b"capture bytes")),
            "audio/rec-1/chunks/mic.wav".to_string(),
        )
        .expect("matching digest must stage");
        assert_eq!(staged.bytes, b"capture bytes");
        assert_eq!(staged.entry.byte_count, 13);
        assert!(staged.entry.omitted.is_none());
    }

    #[test]
    fn an_oversized_payload_is_refused_by_name_rather_than_exhausting_memory() {
        // Declare a stored entry far above the cap without allocating it: the
        // guard reads declared sizes, so it can reject before any read.
        let inventory = AudioInventory {
            staged: vec![StagedAudio {
                entry: AudioEntry {
                    relative_path: "audio/rec-1/chunks/huge.wav".to_string(),
                    source_path: "D:/orig/huge.wav".to_string(),
                    recording_id: "rec-1".to_string(),
                    role: AudioRole::Chunk,
                    byte_count: MAX_PAYLOAD_BYTES + 1,
                    sha256: String::new(),
                    omitted: None,
                },
                bytes: Vec::new(),
            }],
            omitted: vec![],
        };
        assert_eq!(
            pack(b"bundle", &inventory),
            Err(PayloadError::TooLarge {
                actual: MAX_PAYLOAD_BYTES + 1 + 6
            })
        );
    }

    #[test]
    fn trailing_bytes_after_the_declared_payload_are_rejected() {
        let mut packed = pack(b"bundle", &AudioInventory::default()).expect("pack");
        packed.push(0x00);
        assert_eq!(unpack(&packed), Err(PayloadError::Malformed));
    }

    #[test]
    fn a_future_container_version_is_refused_rather_than_misread() {
        let mut packed = pack(b"bundle", &AudioInventory::default()).expect("pack");
        packed[MAGIC.len()] = 0x02;
        assert_eq!(unpack(&packed), Err(PayloadError::UnsupportedVersion));
    }
}
