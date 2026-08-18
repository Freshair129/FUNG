//! Backup job boundary: Genesis full export → encrypt → bounded filesystem
//! write, plus clean-target restore with post-restore verification.
//!
//! GenesisBlockDB is the only export/restore authority; this module never
//! opens a Genesis projection directly. Every response DTO carries only
//! non-secret fields, and every failure keeps the previous verified archive
//! and the current local Genesis state untouched.

use crate::backup_archive::{self, ArchiveError};
use crate::filesystem_backup::{
    self, FilesystemArchiveRecord, FilesystemBackupError, FilesystemBackupState, FilesystemRoot,
};
use genesis_block_native::{BackupExportRequest, BackupRestoreRequest, Storage};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;
use zeroize::Zeroizing;

const RESTORE_TARGET_PREFIX: &str = "restore-";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum BackupJobError {
    #[error("backup root is unavailable")]
    RootUnavailable,
    #[error("another backup or restore job is already running")]
    JobAlreadyRunning,
    #[error("recovery phrase is invalid")]
    InvalidRecoveryPhrase,
    #[error("genesis export failed")]
    ExportFailed,
    #[error("archive encryption failed")]
    EncryptionFailed,
    #[error("archive authentication failed")]
    AuthenticationFailed,
    #[error("archive write failed: {0}")]
    WriteFailed(FilesystemBackupError),
    #[error("archive is unavailable: {0}")]
    ArchiveUnavailable(FilesystemBackupError),
    #[error("restore target parent is unavailable")]
    RestoreParentUnavailable,
    #[error("restore target already exists")]
    RestoreTargetExists,
    #[error("genesis restore failed")]
    RestoreFailed,
    #[error("post-restore verification failed")]
    VerificationFailed,
    #[error("backup staging failed")]
    StagingFailed,
}

/// Guard that serializes backup/restore jobs and always releases the flag.
struct JobGuard(Arc<AtomicBool>);

impl JobGuard {
    fn acquire(flag: &Arc<AtomicBool>) -> Result<Self, BackupJobError> {
        if flag
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(BackupJobError::JobAlreadyRunning);
        }
        Ok(Self(Arc::clone(flag)))
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(crate) struct BackupJobState {
    restore_parent: Arc<Mutex<Option<PathBuf>>>,
    job_running: Arc<AtomicBool>,
}

impl BackupJobState {
    pub(crate) fn restore_parent(&self) -> Option<PathBuf> {
        self.restore_parent
            .lock()
            .ok()
            .and_then(|parent| parent.clone())
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupTerminalState {
    Unavailable,
    NoVerifiedArchive,
    Verified,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupStatus {
    terminal_state: BackupTerminalState,
    archive: Option<FilesystemArchiveRecord>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RestoreParentTerminalState {
    Unavailable,
    Selected,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreParentStatus {
    terminal_state: RestoreParentTerminalState,
    selected_target_id: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreResult {
    archive_id: String,
    restored_bundle_sha256: String,
    terminal_state: String,
}

/// Export the full Genesis snapshot, encrypt it under the recovery phrase,
/// and write the archive beneath the bounded root. The archive is `verified`
/// only after the committed bytes match the manifest digest (enforced by the
/// filesystem adapter). The plaintext bundle only ever exists inside
/// `work_dir`, which must be outside the selected backup root, and is removed
/// before returning.
pub(crate) fn run_backup_job(
    storage: &Storage,
    root: &FilesystemRoot,
    work_dir: &Path,
    archive_id: &str,
    created_at: &str,
    recovery_phrase: &str,
) -> Result<FilesystemArchiveRecord, BackupJobError> {
    if work_dir.starts_with(root.owned_root()) {
        return Err(BackupJobError::StagingFailed);
    }
    fs::create_dir_all(work_dir).map_err(|_| BackupJobError::StagingFailed)?;
    let bundle_path = work_dir.join(format!("{archive_id}.genesis"));
    if bundle_path.exists() {
        return Err(BackupJobError::StagingFailed);
    }

    let export_result = storage
        .export_backup(BackupExportRequest {
            destination: bundle_path.clone(),
        })
        .map_err(|_| BackupJobError::ExportFailed);
    let record = export_result.and_then(|bundle| {
        let plaintext = Zeroizing::new(
            fs::read(&bundle_path).map_err(|_| BackupJobError::StagingFailed)?,
        );
        let envelope = backup_archive::encrypt_archive(
            archive_id,
            &bundle.sha256,
            created_at,
            &plaintext,
            recovery_phrase,
        )
        .map_err(|error| match error {
            ArchiveError::InvalidRecoveryPhrase => BackupJobError::InvalidRecoveryPhrase,
            _ => BackupJobError::EncryptionFailed,
        })?;
        filesystem_backup::write_encrypted_archive_at_root(root, &envelope)
            .map_err(BackupJobError::WriteFailed)
    });
    // Always remove the plaintext bundle, success or failure.
    let _ = fs::remove_file(&bundle_path);
    record
}

/// Read, authenticate, and decrypt an archive, then invoke Genesis restore
/// into a clean `restore-<archive-id>` target that must not exist beforehand.
/// The restored bundle digest must match the source manifest digest recorded
/// at backup time; on any failure the current local state is untouched and a
/// partially created target is removed rather than reported as restored.
pub(crate) fn run_restore_job(
    root: &FilesystemRoot,
    restore_parent: &Path,
    work_dir: &Path,
    archive_id: &str,
    recovery_phrase: &str,
) -> Result<RestoreResult, BackupJobError> {
    let parent_metadata = fs::symlink_metadata(restore_parent)
        .map_err(|_| BackupJobError::RestoreParentUnavailable)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(BackupJobError::RestoreParentUnavailable);
    }
    let restore_parent = fs::canonicalize(restore_parent)
        .map_err(|_| BackupJobError::RestoreParentUnavailable)?;
    // Plaintext must never land beneath the encrypted-only backup root.
    if restore_parent.starts_with(root.owned_root()) || work_dir.starts_with(root.owned_root()) {
        return Err(BackupJobError::RestoreParentUnavailable);
    }

    let envelope = filesystem_backup::read_archive_envelope_at_root(root, archive_id)
        .map_err(BackupJobError::ArchiveUnavailable)?;
    let expected_bundle_digest = envelope.manifest.source_manifest_digest.clone();

    // Authenticate and decrypt fully before any restore-side mutation.
    let plaintext = Zeroizing::new(
        backup_archive::decrypt_archive(&envelope, recovery_phrase).map_err(|error| {
            match error {
                ArchiveError::InvalidRecoveryPhrase => BackupJobError::InvalidRecoveryPhrase,
                _ => BackupJobError::AuthenticationFailed,
            }
        })?,
    );

    let target_root = restore_parent.join(format!("{RESTORE_TARGET_PREFIX}{archive_id}"));
    if target_root.exists() {
        return Err(BackupJobError::RestoreTargetExists);
    }

    fs::create_dir_all(work_dir).map_err(|_| BackupJobError::StagingFailed)?;
    let bundle_path = work_dir.join(format!("{archive_id}.restore.genesis"));
    if bundle_path.exists() {
        return Err(BackupJobError::StagingFailed);
    }
    let restore_result = fs::write(&bundle_path, plaintext.as_slice())
        .map_err(|_| BackupJobError::StagingFailed)
        .and_then(|_| {
            Storage::restore_backup(BackupRestoreRequest {
                bundle_path: bundle_path.clone(),
                target_root: target_root.clone(),
            })
            .map_err(|_| BackupJobError::RestoreFailed)
        })
        .and_then(|restored| {
            if restored.sha256 != expected_bundle_digest {
                return Err(BackupJobError::VerificationFailed);
            }
            Ok(RestoreResult {
                archive_id: archive_id.to_owned(),
                restored_bundle_sha256: restored.sha256,
                terminal_state: "restored".to_owned(),
            })
        });
    let _ = fs::remove_file(&bundle_path);
    if restore_result.is_err() {
        // The target was created by this failed job only; the source archive
        // and current local Genesis state are never touched here.
        let _ = fs::remove_dir_all(&target_root);
    }
    restore_result
}

fn status_for_root(root: Option<FilesystemRoot>) -> BackupStatus {
    let Some(root) = root else {
        return BackupStatus {
            terminal_state: BackupTerminalState::Unavailable,
            archive: None,
        };
    };
    let mut records = filesystem_backup::list_archive_records_at_root(&root);
    match records.first_mut() {
        Some(_) => BackupStatus {
            terminal_state: BackupTerminalState::Verified,
            archive: Some(records.remove(0)),
        },
        None => BackupStatus {
            terminal_state: BackupTerminalState::NoVerifiedArchive,
            archive: None,
        },
    }
}

fn new_archive_id() -> String {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    format!("fung-{stamp}-{}", uuid::Uuid::new_v4().simple())
}

#[tauri::command]
pub(crate) fn backup_status(
    fs_state: tauri::State<'_, FilesystemBackupState>,
) -> BackupStatus {
    status_for_root(fs_state.current_root())
}

#[tauri::command]
pub(crate) fn backup_list_archives(
    fs_state: tauri::State<'_, FilesystemBackupState>,
) -> Vec<FilesystemArchiveRecord> {
    fs_state
        .current_root()
        .map(|root| filesystem_backup::list_archive_records_at_root(&root))
        .unwrap_or_default()
}

/// Generate the 24-word recovery phrase for the setup acknowledgement flow.
/// The phrase is returned once for display; it is never persisted, logged, or
/// embedded in any status/manifest DTO.
#[tauri::command]
pub(crate) fn backup_generate_recovery_phrase() -> Result<String, String> {
    backup_archive::generate_recovery_phrase().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn backup_run(
    recovery_phrase: String,
    app_state: tauri::State<'_, crate::AppState>,
    fs_state: tauri::State<'_, FilesystemBackupState>,
    job_state: tauri::State<'_, BackupJobState>,
) -> Result<FilesystemArchiveRecord, String> {
    let recovery_phrase = Zeroizing::new(recovery_phrase);
    let guard = JobGuard::acquire(&job_state.job_running).map_err(|error| error.to_string())?;
    let root = fs_state
        .current_root()
        .ok_or_else(|| BackupJobError::RootUnavailable.to_string())?;
    let storage = Arc::clone(&app_state.genesis);
    let work_dir = app_state.data_root.join("backup-staging");
    let archive_id = new_archive_id();
    let created_at = crate::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let record = run_backup_job(
            &storage,
            &root,
            &work_dir,
            &archive_id,
            &created_at,
            &recovery_phrase,
        );
        drop(guard);
        record
    })
    .await
    .map_err(|_| BackupJobError::StagingFailed.to_string())?;
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn backup_restore(
    archive_id: String,
    recovery_phrase: String,
    app_state: tauri::State<'_, crate::AppState>,
    fs_state: tauri::State<'_, FilesystemBackupState>,
    job_state: tauri::State<'_, BackupJobState>,
) -> Result<RestoreResult, String> {
    let recovery_phrase = Zeroizing::new(recovery_phrase);
    let guard = JobGuard::acquire(&job_state.job_running).map_err(|error| error.to_string())?;
    let root = fs_state
        .current_root()
        .ok_or_else(|| BackupJobError::RootUnavailable.to_string())?;
    let restore_parent = job_state
        .restore_parent()
        .ok_or_else(|| BackupJobError::RestoreParentUnavailable.to_string())?;
    let work_dir = app_state.data_root.join("backup-staging");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let outcome = run_restore_job(
            &root,
            &restore_parent,
            &work_dir,
            &archive_id,
            &recovery_phrase,
        );
        drop(guard);
        outcome
    })
    .await
    .map_err(|_| BackupJobError::StagingFailed.to_string())?;
    result.map_err(|error| error.to_string())
}

/// Open the native folder picker for the clean restore parent. Only an opaque
/// identifier reaches the web caller; the path stays in native state.
#[tauri::command]
pub(crate) async fn backup_restore_select_target(
    app: tauri::AppHandle,
    job_state: tauri::State<'_, BackupJobState>,
) -> Result<RestoreParentStatus, String> {
    let current_parent = Arc::clone(&job_state.restore_parent);
    let (sender, receiver) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |selection| {
        let path = selection.and_then(|file_path| file_path.into_path().ok());
        let _ = sender.send(path);
    });
    let selected = tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv_timeout(Duration::from_secs(300))
            .ok()
            .flatten()
    })
    .await
    .ok()
    .flatten();

    let validated = selected.and_then(|path| {
        let metadata = fs::symlink_metadata(&path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return None;
        }
        fs::canonicalize(&path).ok()
    });
    let Ok(mut current) = current_parent.lock() else {
        return Ok(unavailable_restore_parent_status());
    };
    match validated {
        Some(parent) => {
            let target_id = sha256_hex(parent.to_string_lossy().as_bytes());
            *current = Some(parent);
            Ok(RestoreParentStatus {
                terminal_state: RestoreParentTerminalState::Selected,
                selected_target_id: Some(target_id),
            })
        }
        None => {
            *current = None;
            Ok(unavailable_restore_parent_status())
        }
    }
}

fn unavailable_restore_parent_status() -> RestoreParentStatus {
    RestoreParentStatus {
        terminal_state: RestoreParentTerminalState::Unavailable,
        selected_target_id: None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::OpenOptions;
    use serde_json::json;
    use tempfile::TempDir;

    fn test_phrase() -> String {
        bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap().to_string()
    }

    fn open_fixture_storage(parent: &TempDir) -> Storage {
        let path = parent.path().join("source-genesis");
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        let timestamp = "2026-08-19T00:00:00Z".to_string();
        for id in ["job-note-a", "job-note-b"] {
            crate::genesis_adapter::commit_note(
                &storage,
                &crate::mobile::MobileNoteInput {
                    id: id.to_string(),
                    title: id.to_string(),
                    body: "Phase 4 backup job fixture".to_string(),
                    project_id: "job-project".to_string(),
                    created_at: timestamp.clone(),
                    updated_at: timestamp.clone(),
                    evidence_label: Some("fixture".to_string()),
                },
                "projects/job-project",
            )
            .unwrap();
        }
        crate::genesis_adapter::commit_relation(
            &storage,
            "job-project",
            &crate::mobile::GraphEdgeInput {
                id: "job-edge-a-b".to_string(),
                source_id: "job-note-a".to_string(),
                target_id: "job-note-b".to_string(),
                predicate: "supports".to_string(),
                status: "confirmed".to_string(),
            },
            &timestamp,
        )
        .unwrap();
        let recording = crate::genesis_adapter::start_capture(
            &storage,
            "job-project",
            "projects/job-project",
            "job-recording",
            "projects/job-project/job-recording/manifest.json",
            &timestamp,
        )
        .unwrap();
        crate::genesis_adapter::append_capture_chunk(
            &storage,
            &recording,
            crate::genesis_adapter::AudioChunk {
                id: "job-audio-chunk",
                file_path: "projects/job-project/job-recording/segment-000001.m4a",
                start_ms: 0,
                end_ms: 1_000,
                byte_size: 128,
                checksum: "fixture-sha256",
                timestamp: &timestamp,
            },
        )
        .unwrap();
        storage
    }

    struct JobFixture {
        _source_parent: TempDir,
        _selected: TempDir,
        work: TempDir,
        restore_parent: TempDir,
        storage: Storage,
        root: FilesystemRoot,
    }

    fn job_fixture() -> JobFixture {
        let source_parent = tempfile::tempdir().unwrap();
        let storage = open_fixture_storage(&source_parent);
        let selected = tempfile::tempdir().unwrap();
        let root = filesystem_backup::prepare_root(selected.path()).unwrap();
        JobFixture {
            _source_parent: source_parent,
            _selected: selected,
            work: tempfile::tempdir().unwrap(),
            restore_parent: tempfile::tempdir().unwrap(),
            storage,
            root,
        }
    }

    #[test]
    fn backup_job_exports_encrypts_writes_and_restores_into_clean_target() {
        let fixture = job_fixture();
        let phrase = test_phrase();
        let source_frontier = fixture.storage.stable_frontier();

        let record = run_backup_job(
            &fixture.storage,
            &fixture.root,
            fixture.work.path(),
            "job-archive-1",
            "2026-08-19T00:10:00Z",
            &phrase,
        )
        .unwrap();
        assert_eq!(record.terminal_state, "verified");
        assert_eq!(record.archive_id, "job-archive-1");
        // The plaintext staging bundle must be gone after the job.
        assert!(!fixture.work.path().join("job-archive-1.genesis").exists());
        // Status truth: the verified archive is discoverable from disk alone.
        let status = status_for_root(Some(fixture.root.clone()));
        assert_eq!(status.terminal_state, BackupTerminalState::Verified);

        let restore = run_restore_job(
            &fixture.root,
            fixture.restore_parent.path(),
            fixture.work.path(),
            "job-archive-1",
            &phrase,
        )
        .unwrap();
        assert_eq!(restore.terminal_state, "restored");

        // Deep post-restore verification: the clean target reproduces the
        // fixture notes, graph relation, and audio-chunk metadata.
        let restored_path = std::fs::canonicalize(fixture.restore_parent.path())
            .unwrap()
            .join("restore-job-archive-1");
        let restored = Storage::open(OpenOptions {
            path: restored_path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .unwrap();
        assert_eq!(restored.stable_frontier(), source_frontier);
        assert!(restored.node_view("job-note-a").is_some());
        assert!(restored.node_view("job-note-b").is_some());
        let edges = crate::genesis_adapter::query(
            &restored,
            "graph_edges",
            &["id"],
            vec![crate::genesis_adapter::eq(
                "graph_edges",
                "project_id",
                json!("job-project"),
            )],
            10,
        )
        .unwrap();
        assert_eq!(edges.len(), 1);
        let chunks = crate::genesis_adapter::query(
            &restored,
            "audio_chunks",
            &["id", "checksum"],
            vec![crate::genesis_adapter::eq(
                "audio_chunks",
                "recording_id",
                json!("job-recording"),
            )],
            10,
        )
        .unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["audio_chunks.checksum"], "fixture-sha256");
    }

    #[test]
    fn restore_failures_preserve_state_and_never_leave_a_partial_target() {
        let fixture = job_fixture();
        let phrase = test_phrase();
        run_backup_job(
            &fixture.storage,
            &fixture.root,
            fixture.work.path(),
            "job-archive-2",
            "2026-08-19T00:20:00Z",
            &phrase,
        )
        .unwrap();

        // Wrong secret fails before any restore-side mutation.
        let wrong_phrase = bip39::Mnemonic::from_entropy(&[9u8; 32]).unwrap().to_string();
        assert_eq!(
            run_restore_job(
                &fixture.root,
                fixture.restore_parent.path(),
                fixture.work.path(),
                "job-archive-2",
                &wrong_phrase,
            ),
            Err(BackupJobError::AuthenticationFailed)
        );
        let target = std::fs::canonicalize(fixture.restore_parent.path())
            .unwrap()
            .join("restore-job-archive-2");
        assert!(!target.exists());

        // Missing archive is reported truthfully.
        assert!(matches!(
            run_restore_job(
                &fixture.root,
                fixture.restore_parent.path(),
                fixture.work.path(),
                "missing-archive",
                &phrase,
            ),
            Err(BackupJobError::ArchiveUnavailable(
                FilesystemBackupError::ArchiveNotFound
            ))
        ));

        // Tampered archive bytes are rejected by the filesystem digest check.
        let archive_path = std::fs::canonicalize(fixture.root.selected_root())
            .unwrap()
            .join("FUNG-DEV-TEST")
            .join("archives")
            .join("job-archive-2.fungbk");
        let mut bytes = fs::read(&archive_path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&archive_path, &bytes).unwrap();
        assert!(matches!(
            run_restore_job(
                &fixture.root,
                fixture.restore_parent.path(),
                fixture.work.path(),
                "job-archive-2",
                &phrase,
            ),
            Err(BackupJobError::ArchiveUnavailable(
                FilesystemBackupError::DigestMismatch
            ))
        ));
        assert!(!target.exists());

        // Refuse in-place overwrite: an existing target is never replaced.
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("existing.txt"), b"keep me").unwrap();
        bytes[last] ^= 1; // untamper
        fs::write(&archive_path, &bytes).unwrap();
        assert_eq!(
            run_restore_job(
                &fixture.root,
                fixture.restore_parent.path(),
                fixture.work.path(),
                "job-archive-2",
                &phrase,
            ),
            Err(BackupJobError::RestoreTargetExists)
        );
        assert_eq!(fs::read(target.join("existing.txt")).unwrap(), b"keep me");
    }

    #[test]
    fn failed_backup_keeps_previous_verified_archive() {
        let fixture = job_fixture();
        let phrase = test_phrase();
        let first = run_backup_job(
            &fixture.storage,
            &fixture.root,
            fixture.work.path(),
            "job-archive-3",
            "2026-08-19T00:30:00Z",
            &phrase,
        )
        .unwrap();

        // Invalid recovery phrase: no archive is created, the previous one stays.
        assert_eq!(
            run_backup_job(
                &fixture.storage,
                &fixture.root,
                fixture.work.path(),
                "job-archive-4",
                "2026-08-19T00:31:00Z",
                "not a valid phrase",
            ),
            Err(BackupJobError::InvalidRecoveryPhrase)
        );
        let records = filesystem_backup::list_archive_records_at_root(&fixture.root);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0], first);
        // No plaintext bundle remains in staging after a failed job.
        assert!(!fixture.work.path().join("job-archive-4.genesis").exists());
    }

    #[test]
    fn backup_job_refuses_plaintext_staging_beneath_the_encrypted_root() {
        let fixture = job_fixture();
        let inside_root = fixture.root.owned_root().join("staging");
        assert_eq!(
            run_backup_job(
                &fixture.storage,
                &fixture.root,
                &inside_root,
                "job-archive-5",
                "2026-08-19T00:40:00Z",
                &test_phrase(),
            ),
            Err(BackupJobError::StagingFailed)
        );
    }

    #[test]
    fn status_without_root_is_fail_closed_and_serializes_no_secret_material() {
        let status = status_for_root(None);
        assert_eq!(status.terminal_state, BackupTerminalState::Unavailable);
        assert!(status.archive.is_none());
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!({
                "terminalState": "unavailable",
                "archive": null,
            })
        );
    }

    #[test]
    fn backup_status_boundary_has_no_secret_response_fields() {
        let source =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/backup.rs")).unwrap();

        for prohibited_field in [
            ["recovery", "Secret"].concat(),
            ["data", "Key"].concat(),
            ["provider", "Token"].concat(),
        ] {
            assert!(
                !source.contains(&prohibited_field),
                "backup status must not add {prohibited_field}"
            );
        }
    }

    #[test]
    fn job_guard_serializes_backup_and_restore_jobs() {
        let flag = Arc::new(AtomicBool::new(false));
        let guard = JobGuard::acquire(&flag).unwrap();
        assert_eq!(
            JobGuard::acquire(&flag).map(|_| ()),
            Err(BackupJobError::JobAlreadyRunning)
        );
        drop(guard);
        assert!(JobGuard::acquire(&flag).is_ok());
    }
}
