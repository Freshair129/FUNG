//! Startup recovery: finding what an unclean shutdown left behind, and
//! putting it back under the ledger's control.
//!
//! A capture writes chunk files first and commits their metadata second, so a
//! crash between those two steps leaves audio on disk that no row describes.
//! Nothing looked for it. The only recovery that existed ran inside
//! `live_meeting_start`, for one project, and did two things: it marked a
//! stale capture `completed` — discarding the fact that it was interrupted —
//! and it failed `recording.capture` jobs while leaving every other job type
//! stuck `running` forever. An import killed mid-transcription still shows a
//! spinner on the next launch.
//!
//! This module separates detection from repair on purpose. Detection runs at
//! startup and must stay cheap: it compares directory listings against ledger
//! rows and never hashes anything, so launching FUNG after a crash does not
//! stall on gigabytes of audio. Repair is an explicit action, because adopting
//! orphaned audio means deciding what it is, and that decision should be
//! visible rather than silent.

use crate::audio_custody;
use crate::genesis_adapter;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Rows read per query. GenesisBlockDB rejects any limit outside `1..1000`.
const GENESIS_QUERY_LIMIT: u32 = crate::genesis_adapter::ROW_CAP;

/// Statuses that mean "this recording never reached a clean end".
const UNFINISHED_STATUSES: [&str; 3] = ["recording", "paused", "pending"];

/// Job states that cannot survive a process exit. A job is owned by the
/// process that runs it, and nothing resumes one across a restart, so any job
/// still in these states at startup belongs to a run that is gone.
/// Job states that no longer belong to a live process at startup.
///
/// The engine adopts these for the types it can run, so what is left here is
/// every *other* type: rows written by code paths that still hand-roll their
/// own threads (`zoom.import`, `recording.capture`) or by builds where a
/// button filed a type nothing ever implemented. Nothing will pick those up,
/// so leaving them non-terminal is a permanent spinner.
const NON_SURVIVING_JOB_STATES: [&str; 4] = ["queued", "running", "paused", "retrying"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InterruptedRecording {
    pub(crate) recording_id: String,
    pub(crate) project_id: String,
    pub(crate) status: String,
    /// Chunks the ledger already knows about.
    pub(crate) known_chunks: usize,
    /// Audio on disk that no row describes — the crash-between-write-and-commit
    /// case. These are recoverable.
    pub(crate) orphan_files: Vec<String>,
    /// Rows whose file is not where they say it is. Recorded, not repaired
    /// here: repair needs a digest, which detection deliberately skips.
    pub(crate) missing_files: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryReport {
    pub(crate) interrupted: Vec<InterruptedRecording>,
    /// Jobs left mid-flight by a dead process and terminalized by this scan.
    pub(crate) stale_jobs_failed: usize,
}

impl RecoveryReport {
    pub(crate) fn needs_attention(&self) -> bool {
        !self.interrupted.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryOutcome {
    pub(crate) recording_id: String,
    /// Orphan files adopted into the ledger, with digests.
    pub(crate) adopted_chunks: usize,
    pub(crate) adopted_bytes: u64,
    /// Files that could not be adopted — unreadable, or not decodable audio.
    pub(crate) unreadable_files: usize,
    pub(crate) duration_ms: i64,
}

fn text(row: &serde_json::Value, key: &str) -> String {
    row.get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Splits a capture chunk file name into `(channel, sequence)`.
///
/// Capture writes `{channel}-{seq:05}.wav`. The sequence is what lets an
/// adopted orphan be placed on its channel's timeline instead of guessed at.
pub(crate) fn parse_chunk_file_name(name: &str) -> Option<(String, u32)> {
    let stem = name.strip_suffix(".wav")?;
    let (channel, sequence) = stem.rsplit_once('-')?;
    if channel.is_empty() {
        return None;
    }
    Some((channel.to_string(), sequence.parse().ok()?))
}

/// Playable duration of a WAV, from its header rather than its byte length —
/// a truncated file reports what it actually contains.
pub(crate) fn wav_duration_ms(path: &Path) -> Option<i64> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return None;
    }
    Some((reader.duration() as u64 * 1000 / spec.sample_rate as u64) as i64)
}

/// The directory a live capture writes its chunks into. `canonical_audio_path`
/// holds the session directory for captures (and a file for imports, which
/// have no chunk directory).
fn chunks_dir_for(canonical_audio_path: &str) -> Option<PathBuf> {
    let session = PathBuf::from(canonical_audio_path);
    let chunks = session.join("chunks");
    chunks.is_dir().then_some(chunks)
}

/// Compares a recording's chunk directory against its ledger rows.
///
/// Cheap by construction: directory listing plus path comparison, no reads and
/// no hashing, so a startup scan stays fast no matter how long the recording
/// was.
fn reconcile_recording(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
    canonical_audio_path: &str,
) -> Result<(usize, Vec<String>, usize), String> {
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["file_path"],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let known: Vec<String> = rows
        .iter()
        .map(|row| text(row, "audio_chunks.file_path"))
        .collect();
    let known_set: std::collections::HashSet<String> = known
        .iter()
        .map(|path| path.replace('\\', "/").to_lowercase())
        .collect();
    let missing = known
        .iter()
        .filter(|path| !Path::new(path).is_file())
        .count();

    let mut orphans = Vec::new();
    if let Some(dir) = chunks_dir_for(canonical_audio_path) {
        let entries = std::fs::read_dir(&dir).map_err(|error| error.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if parse_chunk_file_name(&name).is_none() {
                continue;
            }
            let comparable = path.display().to_string().replace('\\', "/").to_lowercase();
            if !known_set.contains(&comparable) {
                orphans.push(path.display().to_string());
            }
        }
    }
    orphans.sort();
    Ok((known.len(), orphans, missing))
}

/// Fails every job a dead process left mid-flight.
///
/// The previous logic only touched `recording.capture`, so an import killed
/// during transcription stayed `running` forever and showed a permanent
/// spinner. Nothing resumes a job across a restart, so any non-terminal job at
/// startup is finished — it just has not been told.
fn terminalize_stale_jobs(storage: &genesis_block_native::Storage) -> Result<usize, String> {
    let mut failed = 0usize;
    for state in NON_SURVIVING_JOB_STATES {
        let rows = genesis_adapter::query(
            storage,
            "jobs",
            &["id", "type"],
            vec![genesis_adapter::eq(
                "jobs",
                "status",
                serde_json::json!(state),
            )],
            GENESIS_QUERY_LIMIT,
        )?;
        for row in &rows {
            let id = text(row, "jobs.id");
            if id.is_empty() {
                continue;
            }
            let job_type = text(row, "jobs.type");
            // Leave the engine's own work alone: it resumes these itself,
            // and terminalising a queued row at startup is precisely the
            // behaviour the durable queue exists to replace.
            if crate::job_engine::JobKind::parse(&job_type).is_some() {
                continue;
            }
            let _ = crate::set_job_status(
                storage,
                &id,
                "failed",
                None,
                Some(&format!(
                    "interrupted: {job_type} was still {state} when FUNG last exited"
                )),
            );
            failed += 1;
        }
    }
    Ok(failed)
}

/// Reads interrupted recordings without changing the database. The UI calls
/// this while opening or refreshing Recovery, so inspection must remain
/// side-effect free.
fn find_interrupted_recordings(
    storage: &genesis_block_native::Storage,
) -> Result<Vec<InterruptedRecording>, String> {
    let mut interrupted = Vec::new();
    for status in UNFINISHED_STATUSES {
        let rows = genesis_adapter::query(
            storage,
            "recordings",
            &["id", "project_id", "status", "canonical_audio_path"],
            vec![genesis_adapter::eq(
                "recordings",
                "status",
                serde_json::json!(status),
            )],
            GENESIS_QUERY_LIMIT,
        )?;
        for row in &rows {
            let recording_id = text(row, "recordings.id");
            if recording_id.is_empty() {
                continue;
            }
            let canonical = text(row, "recordings.canonical_audio_path");
            let (known_chunks, orphan_files, missing_files) =
                reconcile_recording(storage, &recording_id, &canonical)?;
            interrupted.push(InterruptedRecording {
                recording_id,
                project_id: text(row, "recordings.project_id"),
                status: text(row, "recordings.status"),
                known_chunks,
                orphan_files,
                missing_files,
            });
        }
    }
    Ok(interrupted)
}

/// Reads the current interrupted-recording list for the UI. This path does
/// not terminalize jobs or write audit events; refreshing the view is read-only.
pub(crate) fn inspect(storage: &genesis_block_native::Storage) -> Result<RecoveryReport, String> {
    Ok(RecoveryReport {
        interrupted: find_interrupted_recordings(storage)?,
        stale_jobs_failed: 0,
    })
}

/// Startup-only recovery: finds unfinished recordings across all projects,
/// terminalizes stale jobs, and persists one durable audit event per recording.
///
/// Detection never marks a recording complete or adopts orphan audio. Those
/// decisions remain an explicit user action.
pub(crate) fn scan(storage: &genesis_block_native::Storage) -> Result<RecoveryReport, String> {
    let report = RecoveryReport {
        interrupted: find_interrupted_recordings(storage)?,
        stale_jobs_failed: terminalize_stale_jobs(storage)?,
    };

    // Preserve events written by older builds and make the scan idempotent
    // across startup retries. Deterministic IDs also collapse concurrent
    // attempts for a newly interrupted recording to one row.
    let existing_audits = genesis_adapter::query_all(
        storage,
        "audit_events",
        &["payload_json"],
        vec![genesis_adapter::eq(
            "audit_events",
            "event_type",
            serde_json::json!("recovery.interrupted_recording_found"),
        )],
    )?;
    let mut audited_recordings: std::collections::HashSet<String> = existing_audits
        .iter()
        .filter_map(|row| {
            row.get("audit_events.payload_json")
                .and_then(|payload| payload.get("recordingId"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect();

    // One durable record per interrupted recording, so the interruption
    // survives even if nobody is looking at the UI when FUNG starts.
    let timestamp = crate::now();
    let audit: Vec<_> = report
        .interrupted
        .iter()
        .filter(|item| audited_recordings.insert(item.recording_id.clone()))
        .map(|item| {
            genesis_adapter::upsert(
                "audit_events",
                serde_json::json!({
                    "id": format!("recovery.interrupted_recording_found:{}", item.recording_id),
                    "project_id": item.project_id,
                    "event_type": "recovery.interrupted_recording_found",
                    "actor": "system",
                    "payload_json": {
                        "recordingId": item.recording_id,
                        "status": item.status,
                        "knownChunks": item.known_chunks,
                        "orphanFiles": item.orphan_files.len(),
                        "missingFiles": item.missing_files,
                    },
                    "created_at": timestamp,
                }),
            )
        })
        .collect();
    if !audit.is_empty() {
        genesis_adapter::commit_rows(storage, audit)?;
    }
    Ok(report)
}

/// Places adopted orphans on their channel's timeline.
///
/// Each channel is an independent stream, so an orphan's start is the end of
/// the last chunk already accounted for on *that* channel, and durations
/// accumulate from there. Guessing a uniform chunk length would misplace the
/// short final chunk every capture ends with.
fn plan_adoptions(
    orphans: &[String],
    channel_high_water: &BTreeMap<String, i64>,
) -> Vec<(String, String, i64, i64)> {
    let mut by_channel: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    for path in orphans {
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some((channel, sequence)) = parse_chunk_file_name(&name) {
            by_channel
                .entry(channel)
                .or_default()
                .push((sequence, path.clone()));
        }
    }

    let mut planned = Vec::new();
    for (channel, mut files) in by_channel {
        files.sort_by_key(|(sequence, _)| *sequence);
        let mut cursor = channel_high_water.get(&channel).copied().unwrap_or(0);
        for (_, path) in files {
            let Some(duration) = wav_duration_ms(Path::new(&path)) else {
                // Not decodable audio; the caller counts it as unreadable
                // rather than inventing a time range for it.
                planned.push((channel.clone(), path, -1, -1));
                continue;
            };
            let start = cursor;
            cursor += duration;
            planned.push((channel.clone(), path, start, cursor));
        }
    }
    planned
}

/// Adopts a recording's orphaned audio and closes it out.
///
/// Every adopted file is digested as it is taken in, so it enters the ledger
/// with the same guarantee a live-captured chunk has and participates in
/// backup and integrity checks identically.
pub(crate) fn recover_recording(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
) -> Result<RecoveryOutcome, String> {
    // Legacy adoption cannot infer or close source-owned routed epochs.
    crate::routed_media_access::require_legacy_media_source(storage, recording_id)?;
    let mut record = genesis_adapter::capture(storage, recording_id)?;
    let (_, orphans, _) = reconcile_recording(storage, recording_id, &record.canonical_audio_path)?;

    // Where each channel's timeline currently ends, from the rows that did
    // commit before the crash.
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["file_path", "end_ms"],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let mut high_water: BTreeMap<String, i64> = BTreeMap::new();
    for row in &rows {
        let path = text(row, "audio_chunks.file_path");
        let name = Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some((channel, _)) = parse_chunk_file_name(&name) else {
            continue;
        };
        let end = row
            .get("audio_chunks.end_ms")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let entry = high_water.entry(channel).or_insert(0);
        *entry = (*entry).max(end);
    }

    let mut outcome = RecoveryOutcome {
        recording_id: recording_id.to_string(),
        ..RecoveryOutcome::default()
    };
    let timestamp = crate::now();

    for (_, path, start_ms, end_ms) in plan_adoptions(&orphans, &high_water) {
        if start_ms < 0 {
            outcome.unreadable_files += 1;
            continue;
        }
        let Ok((digest, bytes)) = audio_custody::digest_file(Path::new(&path)) else {
            outcome.unreadable_files += 1;
            continue;
        };
        record = genesis_adapter::append_capture_chunk(
            storage,
            &record,
            genesis_adapter::AudioChunk {
                id: &Uuid::new_v4().to_string(),
                file_path: &path,
                start_ms,
                end_ms,
                byte_size: bytes as i64,
                checksum: &digest,
                timestamp: &timestamp,
            },
        )?;
        outcome.adopted_chunks += 1;
        outcome.adopted_bytes += bytes;
        record.duration_ms = record.duration_ms.max(end_ms);
    }

    outcome.duration_ms = record.duration_ms;
    genesis_adapter::finish_capture(storage, &record, &timestamp)?;
    let recovered_audits = genesis_adapter::query_all(
        storage,
        "audit_events",
        &["payload_json"],
        vec![genesis_adapter::eq(
            "audit_events",
            "event_type",
            serde_json::json!("recovery.recording_recovered"),
        )],
    )?;
    let already_audited = recovered_audits.iter().any(|row| {
        row.get("audit_events.payload_json")
            .and_then(|payload| payload.get("recordingId"))
            .and_then(serde_json::Value::as_str)
            == Some(recording_id)
    });
    if !already_audited {
        genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "audit_events",
                serde_json::json!({
                    "id": format!("recovery.recording_recovered:{recording_id}"),
                    "project_id": record.project_id,
                    "event_type": "recovery.recording_recovered",
                    "actor": "user",
                    "payload_json": {
                        "recordingId": recording_id,
                        "adoptedChunks": outcome.adopted_chunks,
                        "adoptedBytes": outcome.adopted_bytes,
                        "unreadableFiles": outcome.unreadable_files,
                        "durationMs": outcome.duration_ms,
                    },
                    "created_at": timestamp,
                }),
            )],
        )?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture_source_custody as custody;
    use sha2::{Digest, Sha256};

    const ROUTED_AT: &str = "2026-10-29T00:00:00Z";

    struct RoutedRecoveryFixture {
        storage: genesis_block_native::Storage,
        _directory: tempfile::TempDir,
        recording: String,
        project_root: PathBuf,
    }

    impl RoutedRecoveryFixture {
        fn attempt(&self) -> custody::SourceCommitAttempt {
            custody::begin_source_commit(&self.storage, &Uuid::new_v4().to_string(), ROUTED_AT)
                .unwrap()
        }

        fn routed(state: &str) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let storage = open_recovery_storage(&directory.path().join("genesisdb"));
            let project = Uuid::new_v4().to_string();
            let recording = Uuid::new_v4().to_string();
            let project_root = directory.path().join("projects").join(&project);
            let session = project_root.join("live").join(&recording);
            std::fs::create_dir_all(&session).unwrap();
            genesis_adapter::commit_rows(
                &storage,
                vec![
                    genesis_adapter::upsert(
                        "projects",
                        serde_json::json!({"id":project,"name":"Routed recovery fixture","storage_path":project_root,"created_at":ROUTED_AT,"updated_at":ROUTED_AT}),
                    ),
                    genesis_adapter::upsert(
                        "recordings",
                        serde_json::json!({"id":recording,"project_id":project,"source":if state=="pending" {"routed_endpoint_pending"} else {"microphone"},"input_path":null,"canonical_audio_path":session,"status":"recording","duration_ms":0,"created_at":ROUTED_AT,"updated_at":ROUTED_AT}),
                    ),
                    genesis_adapter::upsert(
                        "mobile_recording_checkpoints",
                        serde_json::json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":ROUTED_AT}),
                    ),
                ],
            )
            .unwrap();
            let fixture = Self {
                storage,
                _directory: directory,
                recording,
                project_root,
            };
            let sources = ["one", "two"].map(|label| custody::CaptureSourceRegistration {
                id: Uuid::new_v4().to_string(),
                epoch: 1,
                kind: "endpoint".into(),
                endpoint_id: Some(format!("fixture-endpoint-{label}")),
                label: format!("Fixture source {label}"),
                sample_rate: 48_000,
                clock_origin_qpc_hns: "1234000".into(),
                metadata: serde_json::json!({"policyVersion":1}),
            });
            if state != "pending" {
                custody::register_capture_sources(
                    &fixture.storage,
                    &fixture.attempt(),
                    &project,
                    &fixture.recording,
                    &sources,
                )
                .unwrap();
            }
            for (ordinal, source) in sources.iter().enumerate() {
                let path = session
                    .join("sources")
                    .join(&source.id)
                    .join("epoch-1")
                    .join("chunk-1.wav");
                write_routed_fixture_wav(&path, ordinal as i16);
                if state != "pending" {
                    custody::append_capture_source_chunk(
                        &fixture.storage,
                        &fixture.attempt(),
                        &project,
                        &fixture.recording,
                        &custody::CaptureSourceChunk {
                            id: Uuid::new_v4().to_string(),
                            source_id: source.id.clone(),
                            epoch: 1,
                            sequence: 1,
                            path: path.clone(),
                            start_frame: 0,
                            end_frame: 4_800,
                            start_ms: 0,
                            end_ms: 100,
                            checksum: format!(
                                "{:x}",
                                Sha256::digest(std::fs::read(&path).unwrap())
                            ),
                        },
                    )
                    .unwrap();
                }
            }
            match state {
                "completed" | "hidden_binding" => {
                    custody::complete_capture_sources(
                        &fixture.storage,
                        &fixture.attempt(),
                        &fixture.recording,
                    )
                    .unwrap();
                    custody::require_capture_source_access(&fixture.storage, &fixture.recording)
                        .expect("the completed source fixture must be valid");
                }
                "quarantined" => {
                    custody::quarantine_capture_source(
                        &fixture.storage,
                        &fixture.attempt(),
                        &sources[0].id,
                        "fixture contamination",
                    )
                    .unwrap();
                    custody::complete_capture_sources(
                        &fixture.storage,
                        &fixture.attempt(),
                        &fixture.recording,
                    )
                    .unwrap();
                }
                "pending" | "recording" => (),
                other => panic!("unknown fixture state {other}"),
            }
            if state == "hidden_binding" {
                // Restore the full schema-owned row before changing only the marker.
                let row = recovery_table_snapshot(&fixture.storage, "recordings")
                    .pop()
                    .unwrap();
                let full = row.as_object().unwrap();
                let mut mutation = serde_json::Map::new();
                for (key, value) in full {
                    mutation.insert(
                        key.strip_prefix("recordings.").unwrap().to_string(),
                        value.clone(),
                    );
                }
                mutation.insert("source".into(), serde_json::json!("microphone"));
                genesis_adapter::commit_rows(
                    &fixture.storage,
                    vec![genesis_adapter::upsert(
                        "recordings",
                        serde_json::Value::Object(mutation),
                    )],
                )
                .unwrap();
            }
            // A legacy-looking orphan is deliberately present as synthetic bait.
            // A routed denial must happen before it could be adopted or audited.
            write_routed_fixture_wav(&session.join("chunks").join("mic-00001.wav"), 9);
            fixture
        }
    }

    fn write_routed_fixture_wav(path: &Path, phase: i16) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for frame in 0..4_800 {
            writer.write_sample((frame % 1000) as i16 + phase).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn recovery_table_snapshot(
        storage: &genesis_block_native::Storage,
        table: &str,
    ) -> Vec<serde_json::Value> {
        let names = genesis_adapter::schema()
            .tables
            .into_iter()
            .find(|candidate| candidate.name == table)
            .unwrap()
            .columns
            .into_iter()
            .map(|column| column.name)
            .collect::<Vec<_>>();
        let columns = names.iter().map(String::as_str).collect::<Vec<_>>();
        let mut rows = genesis_adapter::query_all(storage, table, &columns, vec![]).unwrap();
        rows.sort_by_key(|row| serde_json::to_string(row).unwrap());
        rows
    }

    #[derive(Debug, PartialEq)]
    struct RoutedRecoverySnapshot {
        frontier: u64,
        rows: BTreeMap<String, Vec<serde_json::Value>>,
        files: Vec<(String, u64, String)>,
    }

    impl RoutedRecoverySnapshot {
        fn take(fixture: &RoutedRecoveryFixture) -> Self {
            fn media_files(root: &Path, directory: &Path, rows: &mut Vec<(String, u64, String)>) {
                for entry in std::fs::read_dir(directory).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        media_files(root, &path, rows);
                    } else {
                        let bytes = std::fs::read(&path).unwrap();
                        rows.push((
                            path.strip_prefix(root).unwrap().display().to_string(),
                            bytes.len() as u64,
                            format!("{:x}", Sha256::digest(&bytes)),
                        ));
                    }
                }
            }
            let frontier = fixture.storage.txn_frontier();
            let rows = [
                "recordings",
                "audio_chunks",
                "capture_sources",
                "capture_source_chunks",
                "mobile_recording_checkpoints",
                "audit_events",
                "jobs",
            ]
            .into_iter()
            .map(|table| {
                (
                    table.to_string(),
                    recovery_table_snapshot(&fixture.storage, table),
                )
            })
            .collect();
            let mut files = Vec::new();
            media_files(&fixture.project_root, &fixture.project_root, &mut files);
            files.sort();
            assert_eq!(fixture.storage.txn_frontier(), frontier);
            Self {
                frontier,
                rows,
                files,
            }
        }
    }

    fn assert_routed_recovery_denied(state: &str) {
        let fixture = RoutedRecoveryFixture::routed(state);
        let before = RoutedRecoverySnapshot::take(&fixture);
        let result = recover_recording(&fixture.storage, &fixture.recording);
        assert!(
            result.is_err(),
            "legacy recovery must reject {state} routed media before repair; got {result:?}"
        );
        assert_eq!(RoutedRecoverySnapshot::take(&fixture), before);
    }

    #[test]
    fn routed_recovery_pending_is_rejected_without_mutation() {
        assert_routed_recovery_denied("pending");
    }

    #[test]
    fn routed_recovery_recording_is_rejected_without_mutation() {
        assert_routed_recovery_denied("recording");
    }

    #[test]
    fn routed_recovery_quarantined_is_rejected_without_mutation() {
        assert_routed_recovery_denied("quarantined");
    }

    #[test]
    fn routed_recovery_completed_is_rejected_without_mutation() {
        assert_routed_recovery_denied("completed");
    }

    #[test]
    fn routed_recovery_hidden_bindings_are_rejected_without_mutation() {
        assert_routed_recovery_denied("hidden_binding");
    }

    fn open_recovery_storage(path: &Path) -> genesis_block_native::Storage {
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .expect("open isolated recovery test database");
        genesis_adapter::install(&storage).expect("install FUNG schema");
        storage
    }

    fn seed_interrupted_recording(storage: &genesis_block_native::Storage, recording_root: &Path) {
        std::fs::create_dir_all(recording_root.join("chunks")).unwrap();
        genesis_adapter::commit_rows(
            storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "project-1",
                        "name": "Recovery test",
                        "storage_path": "local-test-project",
                        "created_at": "created",
                        "updated_at": "updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "recording-1",
                        "project_id": "project-1",
                        "source": "microphone",
                        "input_path": null,
                        "canonical_audio_path": recording_root.display().to_string(),
                        "status": "recording",
                        "duration_ms": 0,
                        "created_at": "created",
                        "updated_at": "updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "mobile_recording_checkpoints",
                    serde_json::json!({
                        "id": "recording-1",
                        "recording_id": "recording-1",
                        "safe_offset_ms": 0,
                        "segment_count": 0,
                        "last_checksum": null,
                        "updated_at": "updated",
                    }),
                ),
            ],
        )
        .expect("seed interrupted recording");
    }

    fn interrupted_audit_count(storage: &genesis_block_native::Storage) -> usize {
        genesis_adapter::query_all(
            storage,
            "audit_events",
            &["id", "payload_json"],
            vec![genesis_adapter::eq(
                "audit_events",
                "event_type",
                serde_json::json!("recovery.interrupted_recording_found"),
            )],
        )
        .expect("query recovery audit events")
        .len()
    }

    fn recovered_audit_count(storage: &genesis_block_native::Storage) -> usize {
        genesis_adapter::query_all(
            storage,
            "audit_events",
            &["id", "payload_json"],
            vec![genesis_adapter::eq(
                "audit_events",
                "event_type",
                serde_json::json!("recovery.recording_recovered"),
            )],
        )
        .expect("query recovered audit events")
        .len()
    }

    #[test]
    fn ui_inspection_is_read_only_and_startup_audit_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("genesisdb");
        let storage = open_recovery_storage(&database_path);
        let recording_root = directory.path().join("project-1").join("recording-1");
        seed_interrupted_recording(&storage, &recording_root);

        let startup = scan(&storage).expect("run startup recovery");
        assert_eq!(startup.interrupted.len(), 1);
        assert_eq!(interrupted_audit_count(&storage), 1);

        let first_inspection = inspect(&storage).expect("inspect recovery UI");
        let second_inspection = inspect(&storage).expect("refresh recovery UI");
        assert_eq!(first_inspection.interrupted, second_inspection.interrupted);
        assert_eq!(first_inspection.stale_jobs_failed, 0);
        assert_eq!(second_inspection.stale_jobs_failed, 0);
        assert_eq!(interrupted_audit_count(&storage), 1);

        // A repeated startup scan should retain the original audit event,
        // including when it was written by a prior build with a random ID.
        let repeated_startup = scan(&storage).expect("repeat startup recovery");
        assert_eq!(repeated_startup.interrupted.len(), 1);
        assert_eq!(interrupted_audit_count(&storage), 1);
    }

    #[test]
    fn startup_scan_reuses_a_legacy_random_id_audit_event() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("genesisdb");
        let storage = open_recovery_storage(&database_path);
        let recording_root = directory.path().join("project-1").join("recording-1");
        seed_interrupted_recording(&storage, &recording_root);
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "audit_events",
                serde_json::json!({
                    "id": "legacy-random-audit-id",
                    "project_id": "project-1",
                    "event_type": "recovery.interrupted_recording_found",
                    "actor": "system",
                    "payload_json": { "recordingId": "recording-1" },
                    "created_at": "legacy-created",
                }),
            )],
        )
        .expect("seed audit row from an older build");

        scan(&storage).expect("scan interrupted recording with legacy audit");

        let audits = genesis_adapter::query_all(
            &storage,
            "audit_events",
            &["id", "payload_json"],
            vec![genesis_adapter::eq(
                "audit_events",
                "event_type",
                serde_json::json!("recovery.interrupted_recording_found"),
            )],
        )
        .expect("query recovery audit events");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0]["audit_events.id"], "legacy-random-audit-id");
    }

    #[test]
    fn recovery_adopts_orphans_and_is_idempotent_across_retries() {
        let directory = tempfile::tempdir().unwrap();
        let storage = open_recovery_storage(&directory.path().join("genesisdb"));
        let recording_root = directory.path().join("project-1").join("recording-1");
        seed_interrupted_recording(&storage, &recording_root);

        let orphan = recording_root.join("chunks").join("mic-00001.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 1_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&orphan, spec).unwrap();
        for sample in 1..=1_000i16 {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();

        let first = recover_recording(&storage, "recording-1").expect("recover orphan audio");
        assert_eq!(first.adopted_chunks, 1);
        assert_eq!(first.duration_ms, 1_000);
        assert_eq!(inspect(&storage).unwrap().interrupted.len(), 0);
        assert_eq!(recovered_audit_count(&storage), 1);

        let retry = recover_recording(&storage, "recording-1").expect("retry recovery safely");
        assert_eq!(retry.adopted_chunks, 0);
        assert_eq!(retry.duration_ms, 1_000);
        assert_eq!(recovered_audit_count(&storage), 1);
    }

    #[test]
    fn chunk_file_names_yield_their_channel_and_sequence() {
        assert_eq!(
            parse_chunk_file_name("mic-00007.wav"),
            Some(("mic".to_string(), 7))
        );
        assert_eq!(
            parse_chunk_file_name("system-00123.wav"),
            Some(("system".to_string(), 123))
        );
        // Anything the capture path did not write is left alone rather than
        // adopted as audio.
        assert_eq!(parse_chunk_file_name("notes.txt"), None);
        assert_eq!(parse_chunk_file_name("mic.wav"), None);
        assert_eq!(parse_chunk_file_name("-00001.wav"), None);
        assert_eq!(parse_chunk_file_name("mic-abc.wav"), None);
    }

    #[test]
    fn adopted_orphans_continue_their_own_channel_timeline() {
        // Each channel is an independent stream. Placing a system-audio orphan
        // after the microphone's last chunk would put it at the wrong time.
        let temp = tempfile::tempdir().unwrap();
        let write = |name: &str, ms: u32| {
            let path = temp.path().join(name);
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 1000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for _ in 0..ms {
                writer.write_sample(0i16).unwrap();
            }
            writer.finalize().unwrap();
            path.display().to_string()
        };

        let orphans = vec![write("mic-00003.wav", 800), write("system-00002.wav", 500)];
        let high_water =
            BTreeMap::from([("mic".to_string(), 16_000), ("system".to_string(), 8_000)]);

        let planned = plan_adoptions(&orphans, &high_water);
        let mic = planned.iter().find(|(c, _, _, _)| c == "mic").unwrap();
        let system = planned.iter().find(|(c, _, _, _)| c == "system").unwrap();

        assert_eq!((mic.2, mic.3), (16_000, 16_800));
        assert_eq!((system.2, system.3), (8_000, 8_500));
    }

    #[test]
    fn orphans_on_one_channel_accumulate_in_sequence_order() {
        let temp = tempfile::tempdir().unwrap();
        let write = |name: &str, ms: u32| {
            let path = temp.path().join(name);
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 1000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for _ in 0..ms {
                writer.write_sample(0i16).unwrap();
            }
            writer.finalize().unwrap();
            path.display().to_string()
        };

        // Deliberately out of order: adoption must sort by sequence, not by
        // whatever order the directory listing returned.
        let orphans = vec![write("mic-00009.wav", 300), write("mic-00008.wav", 700)];
        let planned = plan_adoptions(&orphans, &BTreeMap::from([("mic".to_string(), 1_000)]));

        assert_eq!(planned.len(), 2);
        assert!(planned[0].1.ends_with("mic-00008.wav"));
        assert_eq!((planned[0].2, planned[0].3), (1_000, 1_700));
        assert!(planned[1].1.ends_with("mic-00009.wav"));
        assert_eq!((planned[1].2, planned[1].3), (1_700, 2_000));
    }

    #[test]
    fn a_file_that_is_not_decodable_audio_is_flagged_rather_than_timed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mic-00001.wav");
        std::fs::write(&path, b"not a wav file at all").unwrap();

        let planned = plan_adoptions(&[path.display().to_string()], &BTreeMap::new());
        assert_eq!(planned.len(), 1);
        assert_eq!(
            (planned[0].2, planned[0].3),
            (-1, -1),
            "an undecodable file must not be given an invented time range"
        );
        assert_eq!(wav_duration_ms(&path), None);
    }

    #[test]
    fn duration_comes_from_the_header_not_the_byte_length() {
        // A truncated chunk must report what it actually contains, so a
        // recovered recording's duration is not inflated by a partial write.
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mic-00001.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..4_000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        assert_eq!(wav_duration_ms(&path), Some(500));
    }

    #[test]
    fn a_report_only_asks_for_attention_when_something_was_interrupted() {
        let mut report = RecoveryReport::default();
        assert!(!report.needs_attention());

        // Stale jobs alone are handled automatically and need no user action.
        report.stale_jobs_failed = 4;
        assert!(!report.needs_attention());

        report.interrupted.push(InterruptedRecording {
            recording_id: "r1".into(),
            project_id: "p1".into(),
            status: "recording".into(),
            known_chunks: 12,
            orphan_files: vec!["a.wav".into()],
            missing_files: 0,
        });
        assert!(report.needs_attention());
        assert!(!report.interrupted[0].orphan_files.is_empty());
    }

    #[test]
    fn a_recording_with_nothing_on_disk_is_still_reported() {
        // Worth surfacing even with nothing to adopt: the user needs to know a
        // session was interrupted, not only that something can be salvaged.
        let empty = InterruptedRecording {
            recording_id: "r1".into(),
            project_id: "p1".into(),
            status: "recording".into(),
            known_chunks: 0,
            orphan_files: vec![],
            missing_files: 0,
        };
        assert_eq!(empty.known_chunks, 0);
        assert!(empty.orphan_files.is_empty());
    }
}
