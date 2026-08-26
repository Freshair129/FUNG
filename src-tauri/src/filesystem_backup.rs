//! Bounded filesystem adapter for the Phase 4 test-storage proof.
//!
//! This module deliberately has no path-bearing Tauri command and does not
//! deserialize a filesystem path from the web UI. The native folder-picker command retains
//! only the validated root in native state and exposes an opaque root ID. All
//! archive and manifest paths are then derived below the FUNG-owned root.

#![allow(dead_code)]

use crate::backup_archive::{ArchiveEnvelope, ArchiveManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

const OWNED_FOLDER: &str = "FUNG-DEV-TEST";
const ARCHIVES_FOLDER: &str = "archives";
const MANIFESTS_FOLDER: &str = "manifests";
const STAGING_FOLDER: &str = "staging";
const ARCHIVE_EXTENSION: &str = ".fungbk";
const MANIFEST_EXTENSION: &str = ".manifest.json";
const PARTIAL_EXTENSION: &str = ".partial";
const MANIFEST_PARTIAL_EXTENSION: &str = ".manifest.partial";
const VERIFIED_TERMINAL_STATE: &str = "verified";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum FilesystemBackupError {
    #[error("filesystem backup root is unavailable")]
    RootUnavailable,
    #[error("filesystem backup root is not a directory")]
    RootNotDirectory,
    #[error("filesystem backup path escaped the selected root")]
    PathEscape,
    #[error("filesystem backup target is a symlink")]
    SymlinkEscape,
    #[error("filesystem backup archive id is invalid")]
    InvalidArchiveId,
    #[error("filesystem backup staging file already exists")]
    StagingExists,
    #[error("filesystem backup archive already exists")]
    ArchiveExists,
    #[error("filesystem backup manifest already exists")]
    ManifestExists,
    #[error("filesystem backup archive was not found")]
    ArchiveNotFound,
    #[error("filesystem backup manifest is invalid")]
    InvalidManifest,
    #[error("filesystem backup digest mismatch")]
    DigestMismatch,
    #[error("filesystem backup filesystem operation failed")]
    Io,
}

/// Canonical, FUNG-owned storage root derived from a native picker result.
///
/// The fields are intentionally private and the type is not deserializable;
/// this prevents a web payload from manufacturing a path-bearing root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FilesystemRoot {
    selected_root: PathBuf,
    owned_root: PathBuf,
    root_id: String,
    archives_dir: PathBuf,
    manifests_dir: PathBuf,
    staging_dir: PathBuf,
}

#[derive(Default)]
pub(crate) struct FilesystemBackupState {
    root: Arc<Mutex<Option<FilesystemRoot>>>,
}

impl FilesystemBackupState {
    /// Snapshot of the currently selected root, if any. The root never leaves
    /// native state; callers use it to derive bounded archive paths only.
    pub(crate) fn current_root(&self) -> Option<FilesystemRoot> {
        self.root.lock().ok().and_then(|current| current.clone())
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FilesystemRootTerminalState {
    Unavailable,
    Selected,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilesystemRootStatus {
    terminal_state: FilesystemRootTerminalState,
    selected_root_id: Option<String>,
}

impl FilesystemRoot {
    pub(crate) fn root_id(&self) -> &str {
        &self.root_id
    }

    pub(crate) fn selected_root(&self) -> &Path {
        &self.selected_root
    }

    pub(crate) fn owned_root(&self) -> &Path {
        &self.owned_root
    }
}

/// Open the native folder picker and retain only the validated root in native
/// state. The web caller receives an opaque root identifier, never a path.
#[tauri::command]
pub(crate) async fn filesystem_backup_select_root(
    app: tauri::AppHandle,
    state: tauri::State<'_, FilesystemBackupState>,
) -> Result<FilesystemRootStatus, String> {
    let current_root = Arc::clone(&state.root);
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
    let Some(selected) = selected else {
        clear_selected_root(&current_root);
        return Ok(unavailable_root_status());
    };

    let Ok(root) = prepare_root(&selected) else {
        clear_selected_root(&current_root);
        return Ok(unavailable_root_status());
    };
    let root_id = root.root_id().to_owned();
    let Ok(mut current) = current_root.lock() else {
        return Ok(unavailable_root_status());
    };
    *current = Some(root);
    Ok(FilesystemRootStatus {
        terminal_state: FilesystemRootTerminalState::Selected,
        selected_root_id: Some(root_id),
    })
}

fn clear_selected_root(state: &Mutex<Option<FilesystemRoot>>) {
    if let Ok(mut current) = state.lock() {
        *current = None;
    }
}

fn unavailable_root_status() -> FilesystemRootStatus {
    FilesystemRootStatus {
        terminal_state: FilesystemRootTerminalState::Unavailable,
        selected_root_id: None,
    }
}

/// Non-secret metadata persisted next to a completed archive.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilesystemArchiveRecord {
    pub(crate) archive_id: String,
    pub(crate) digest: String,
    pub(crate) byte_count: u64,
    pub(crate) timestamp: String,
    pub(crate) selected_root_id: String,
    pub(crate) relative_archive_name: String,
    pub(crate) terminal_state: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PersistedFilesystemManifest {
    archive: ArchiveManifest,
    record: FilesystemArchiveRecord,
}

/// Canonicalize and prepare the FUNG-owned child layout below a native-picked
/// directory. A picked `FUNG-DEV-TEST` directory is accepted as the owned root;
/// any other picked directory receives that child folder.
pub(crate) fn prepare_root(selected_root: &Path) -> Result<FilesystemRoot, FilesystemBackupError> {
    let selected_metadata =
        fs::symlink_metadata(selected_root).map_err(|_| FilesystemBackupError::RootUnavailable)?;
    if selected_metadata.file_type().is_symlink() {
        return Err(FilesystemBackupError::SymlinkEscape);
    }
    if !selected_metadata.is_dir() {
        return Err(FilesystemBackupError::RootNotDirectory);
    }

    let canonical_selected =
        fs::canonicalize(selected_root).map_err(|_| FilesystemBackupError::RootUnavailable)?;
    let owned_candidate = if canonical_selected.file_name() == Some(OsStr::new(OWNED_FOLDER)) {
        canonical_selected.clone()
    } else {
        canonical_selected.join(OWNED_FOLDER)
    };

    if let Ok(metadata) = fs::symlink_metadata(&owned_candidate) {
        if metadata.file_type().is_symlink() {
            return Err(FilesystemBackupError::SymlinkEscape);
        }
        if !metadata.is_dir() {
            return Err(FilesystemBackupError::RootNotDirectory);
        }
    } else {
        fs::create_dir_all(&owned_candidate).map_err(|_| FilesystemBackupError::Io)?;
    }

    let owned_root =
        fs::canonicalize(&owned_candidate).map_err(|_| FilesystemBackupError::RootUnavailable)?;
    if !owned_root.starts_with(&canonical_selected) {
        return Err(FilesystemBackupError::PathEscape);
    }

    let archives_dir = ensure_child_dir(&owned_root, ARCHIVES_FOLDER)?;
    let manifests_dir = ensure_child_dir(&owned_root, MANIFESTS_FOLDER)?;
    let staging_dir = ensure_child_dir(&owned_root, STAGING_FOLDER)?;

    Ok(FilesystemRoot {
        selected_root: canonical_selected,
        root_id: sha256_hex(owned_root.to_string_lossy().as_bytes()),
        owned_root,
        archives_dir,
        manifests_dir,
        staging_dir,
    })
}

/// Write an encrypted envelope using same-filesystem, create-new staging files
/// and atomic renames. No existing archive or manifest is overwritten.
pub(crate) fn write_encrypted_archive(
    selected_root: &Path,
    envelope: &ArchiveEnvelope,
) -> Result<FilesystemArchiveRecord, FilesystemBackupError> {
    let root = prepare_root(selected_root)?;
    write_encrypted_archive_at_root(&root, envelope)
}

pub(crate) fn write_encrypted_archive_at_root(
    root: &FilesystemRoot,
    envelope: &ArchiveEnvelope,
) -> Result<FilesystemArchiveRecord, FilesystemBackupError> {
    let archive_id = validate_archive_id(&envelope.manifest.archive_id)?;
    let digest = sha256_hex(&envelope.bytes);
    if envelope.manifest.sha256 != digest
        || envelope.manifest.byte_count != envelope.bytes.len() as u64
    {
        return Err(FilesystemBackupError::DigestMismatch);
    }

    let archive_name = format!("{archive_id}{ARCHIVE_EXTENSION}");
    let manifest_name = format!("{archive_id}{MANIFEST_EXTENSION}");
    let archive_path = root.archives_dir.join(&archive_name);
    let manifest_path = root.manifests_dir.join(&manifest_name);
    let archive_partial = root
        .staging_dir
        .join(format!("{archive_id}{PARTIAL_EXTENSION}"));
    let manifest_partial = root
        .staging_dir
        .join(format!("{archive_id}{MANIFEST_PARTIAL_EXTENSION}"));

    ensure_absent(&archive_path, FilesystemBackupError::ArchiveExists)?;
    ensure_absent(&manifest_path, FilesystemBackupError::ManifestExists)?;
    ensure_staging_absent(&archive_partial)?;
    ensure_staging_absent(&manifest_partial)?;

    let mut persisted_archive = envelope.manifest.clone();
    persisted_archive.terminal_state = VERIFIED_TERMINAL_STATE.to_owned();
    let record = FilesystemArchiveRecord {
        archive_id: archive_id.to_owned(),
        digest: digest.clone(),
        byte_count: envelope.bytes.len() as u64,
        timestamp: persisted_archive.created_at.clone(),
        selected_root_id: root.root_id.clone(),
        relative_archive_name: format!("{ARCHIVES_FOLDER}/{archive_name}"),
        terminal_state: VERIFIED_TERMINAL_STATE.to_owned(),
    };
    let persisted_manifest = PersistedFilesystemManifest {
        archive: persisted_archive,
        record: record.clone(),
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&persisted_manifest).map_err(|_| FilesystemBackupError::Io)?;

    // The two staging directories and their final directories share the same
    // selected root, so each rename is atomic on the same filesystem.
    write_new_file(&archive_partial, &envelope.bytes)?;
    fs::rename(&archive_partial, &archive_path).map_err(|_| FilesystemBackupError::Io)?;
    write_new_file(&manifest_partial, &manifest_bytes)?;
    fs::rename(&manifest_partial, &manifest_path).map_err(|_| FilesystemBackupError::Io)?;

    let committed_bytes = fs::read(&archive_path).map_err(|_| FilesystemBackupError::Io)?;
    if sha256_hex(&committed_bytes) != digest {
        return Err(FilesystemBackupError::DigestMismatch);
    }
    Ok(record)
}

/// Read and verify an archive plus its non-secret sidecar metadata.
pub(crate) fn read_archive(
    selected_root: &Path,
    archive_id: &str,
    expected_digest: Option<&str>,
) -> Result<Vec<u8>, FilesystemBackupError> {
    let root = prepare_root(selected_root)?;
    read_archive_at_root(&root, archive_id, expected_digest)
}

pub(crate) fn read_archive_at_root(
    root: &FilesystemRoot,
    archive_id: &str,
    expected_digest: Option<&str>,
) -> Result<Vec<u8>, FilesystemBackupError> {
    let archive_id = validate_archive_id(archive_id)?;
    let archive_path = root
        .archives_dir
        .join(format!("{archive_id}{ARCHIVE_EXTENSION}"));
    let manifest_path = root
        .manifests_dir
        .join(format!("{archive_id}{MANIFEST_EXTENSION}"));
    ensure_existing_file(&archive_path)?;
    ensure_existing_file(&manifest_path)?;

    let archive_path = canonical_file_inside_root(root, &archive_path)?;
    let manifest_path = canonical_file_inside_root(root, &manifest_path)?;
    let bytes = fs::read(archive_path).map_err(|_| FilesystemBackupError::Io)?;
    let manifest_bytes = fs::read(manifest_path).map_err(|_| FilesystemBackupError::Io)?;
    let persisted: PersistedFilesystemManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| FilesystemBackupError::InvalidManifest)?;

    if persisted.record.archive_id != archive_id
        || persisted.record.terminal_state != VERIFIED_TERMINAL_STATE
        || persisted.archive.archive_id != archive_id
        || persisted.archive.terminal_state != VERIFIED_TERMINAL_STATE
        || persisted.archive.byte_count != bytes.len() as u64
    {
        return Err(FilesystemBackupError::InvalidManifest);
    }

    let digest = sha256_hex(&bytes);
    if persisted.record.digest != digest || persisted.archive.sha256 != digest {
        return Err(FilesystemBackupError::DigestMismatch);
    }
    if expected_digest.is_some_and(|expected| expected != digest) {
        return Err(FilesystemBackupError::DigestMismatch);
    }
    Ok(bytes)
}

pub(crate) fn read_archive_record(
    selected_root: &Path,
    archive_id: &str,
) -> Result<FilesystemArchiveRecord, FilesystemBackupError> {
    let root = prepare_root(selected_root)?;
    read_archive_record_at_root(&root, archive_id)
}

pub(crate) fn read_archive_record_at_root(
    root: &FilesystemRoot,
    archive_id: &str,
) -> Result<FilesystemArchiveRecord, FilesystemBackupError> {
    let archive_id = validate_archive_id(archive_id)?;
    let archive_path = root
        .archives_dir
        .join(format!("{archive_id}{ARCHIVE_EXTENSION}"));
    let manifest_path = root
        .manifests_dir
        .join(format!("{archive_id}{MANIFEST_EXTENSION}"));
    ensure_existing_file(&archive_path)?;
    ensure_existing_file(&manifest_path)?;
    let archive_path = canonical_file_inside_root(root, &archive_path)?;
    let manifest_path = canonical_file_inside_root(root, &manifest_path)?;
    let bytes = fs::read(archive_path).map_err(|_| FilesystemBackupError::Io)?;
    let manifest_bytes = fs::read(manifest_path).map_err(|_| FilesystemBackupError::Io)?;
    let persisted: PersistedFilesystemManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| FilesystemBackupError::InvalidManifest)?;
    let digest = sha256_hex(&bytes);
    if persisted.record.archive_id != archive_id
        || persisted.record.terminal_state != VERIFIED_TERMINAL_STATE
        || persisted.record.byte_count != bytes.len() as u64
        || persisted.record.digest != digest
    {
        return Err(FilesystemBackupError::DigestMismatch);
    }
    Ok(persisted.record)
}

/// Return the canonical archive path plus the validated non-secret record and
/// manifest for transport to an external adapter. Validation is repeated
/// through `read_archive_at_root` so callers cannot upload a partial or
/// tampered archive by using only the sidecar metadata.
pub(crate) fn verified_archive_details_at_root(
    root: &FilesystemRoot,
    archive_id: &str,
) -> Result<(PathBuf, FilesystemArchiveRecord, ArchiveManifest), FilesystemBackupError> {
    let archive_id = validate_archive_id(archive_id)?;
    let _bytes = read_archive_at_root(root, archive_id, None)?;
    let archive_path = canonical_file_inside_root(
        root,
        &root
            .archives_dir
            .join(format!("{archive_id}{ARCHIVE_EXTENSION}")),
    )?;
    let manifest_path = canonical_file_inside_root(
        root,
        &root
            .manifests_dir
            .join(format!("{archive_id}{MANIFEST_EXTENSION}")),
    )?;
    let manifest_bytes = fs::read(manifest_path).map_err(|_| FilesystemBackupError::Io)?;
    let persisted: PersistedFilesystemManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| FilesystemBackupError::InvalidManifest)?;
    Ok((archive_path, persisted.record, persisted.archive))
}

/// Rebuild the in-memory envelope for a persisted archive so it can be
/// authenticated and decrypted. The sidecar stores the manifest re-labelled
/// `verified`; decryption pins the pre-transport terminal state, so it is
/// restored here before the envelope leaves this adapter.
pub(crate) fn read_archive_envelope_at_root(
    root: &FilesystemRoot,
    archive_id: &str,
) -> Result<ArchiveEnvelope, FilesystemBackupError> {
    let bytes = read_archive_at_root(root, archive_id, None)?;
    let archive_id = validate_archive_id(archive_id)?;
    let manifest_path = root
        .manifests_dir
        .join(format!("{archive_id}{MANIFEST_EXTENSION}"));
    let manifest_path = canonical_file_inside_root(root, &manifest_path)?;
    let manifest_bytes = fs::read(manifest_path).map_err(|_| FilesystemBackupError::Io)?;
    let persisted: PersistedFilesystemManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| FilesystemBackupError::InvalidManifest)?;
    let mut manifest = persisted.archive;
    manifest.terminal_state = crate::backup_archive::ENCRYPTED_TERMINAL_STATE.to_owned();
    Ok(ArchiveEnvelope { manifest, bytes })
}

/// Enumerate the verified archives beneath the selected root, newest first.
/// Records that fail digest or manifest validation are omitted rather than
/// reported as restorable.
pub(crate) fn list_archive_records_at_root(root: &FilesystemRoot) -> Vec<FilesystemArchiveRecord> {
    let Ok(entries) = fs::read_dir(&root.manifests_dir) else {
        return Vec::new();
    };
    let mut records: Vec<FilesystemArchiveRecord> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let archive_id = name.strip_suffix(MANIFEST_EXTENSION)?.to_owned();
            read_archive_record_at_root(root, &archive_id).ok()
        })
        .collect();
    records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    records
}

fn ensure_child_dir(root: &Path, name: &str) -> Result<PathBuf, FilesystemBackupError> {
    let candidate = root.join(name);
    if let Ok(metadata) = fs::symlink_metadata(&candidate) {
        if metadata.file_type().is_symlink() {
            return Err(FilesystemBackupError::SymlinkEscape);
        }
        if !metadata.is_dir() {
            return Err(FilesystemBackupError::RootNotDirectory);
        }
    } else {
        fs::create_dir(&candidate).map_err(|_| FilesystemBackupError::Io)?;
    }
    let canonical = fs::canonicalize(&candidate).map_err(|_| FilesystemBackupError::Io)?;
    if !canonical.starts_with(root) {
        return Err(FilesystemBackupError::PathEscape);
    }
    Ok(canonical)
}

fn ensure_absent(
    path: &Path,
    existing: FilesystemBackupError,
) -> Result<(), FilesystemBackupError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(FilesystemBackupError::SymlinkEscape)
        }
        Ok(_) => Err(existing),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(FilesystemBackupError::Io),
    }
}

fn ensure_staging_absent(path: &Path) -> Result<(), FilesystemBackupError> {
    ensure_absent(path, FilesystemBackupError::StagingExists)
}

fn ensure_existing_file(path: &Path) -> Result<(), FilesystemBackupError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(FilesystemBackupError::SymlinkEscape)
        }
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(FilesystemBackupError::ArchiveNotFound),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(FilesystemBackupError::ArchiveNotFound)
        }
        Err(_) => Err(FilesystemBackupError::Io),
    }
}

fn canonical_file_inside_root(
    root: &FilesystemRoot,
    path: &Path,
) -> Result<PathBuf, FilesystemBackupError> {
    let canonical = fs::canonicalize(path).map_err(|_| FilesystemBackupError::ArchiveNotFound)?;
    if !canonical.starts_with(&root.owned_root) {
        return Err(FilesystemBackupError::PathEscape);
    }
    Ok(canonical)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), FilesystemBackupError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| FilesystemBackupError::StagingExists)?;
    file.write_all(bytes)
        .map_err(|_| FilesystemBackupError::Io)?;
    file.flush().map_err(|_| FilesystemBackupError::Io)?;
    file.sync_all().map_err(|_| FilesystemBackupError::Io)?;
    Ok(())
}

fn validate_archive_id(value: &str) -> Result<&str, FilesystemBackupError> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(FilesystemBackupError::InvalidArchiveId);
    }
    Ok(value)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_archive::{ArchiveEnvelope, ArchiveManifest};
    use std::fs;
    use tempfile::TempDir;

    fn fixture_envelope(archive_id: &str, bytes: &[u8]) -> ArchiveEnvelope {
        ArchiveEnvelope {
            bytes: bytes.to_vec(),
            manifest: ArchiveManifest {
                archive_id: archive_id.to_owned(),
                format_version: 1,
                source_manifest_digest: "00".repeat(32),
                created_at: "2026-08-14T00:00:00Z".to_owned(),
                byte_count: bytes.len() as u64,
                sha256: sha256_hex(bytes),
                chunk_size: 65_536,
                chunk_count: 1,
                payload_algorithm: "xchacha20poly1305-stream-le31".to_owned(),
                kdf_algorithm: "argon2id".to_owned(),
                kdf_memory_kib: 65_536,
                kdf_iterations: 3,
                kdf_parallelism: 1,
                kdf_salt: "00".repeat(16),
                protected_header_digest: "00".repeat(32),
                stream_nonce_prefix: "22".repeat(16),
                wrap_nonce: "11".repeat(24),
                wrapped_data_key: "33".repeat(48),
                terminal_state: "encrypted_pending_transport".to_owned(),
            },
        }
    }

    fn temp_root() -> (TempDir, FilesystemRoot) {
        let selected = tempfile::tempdir().expect("temp root");
        let root = prepare_root(selected.path()).expect("prepare root");
        (selected, root)
    }

    #[test]
    fn root_layout_is_canonical_and_bounded() {
        let (selected, root) = temp_root();
        let canonical_selected = fs::canonicalize(selected.path()).expect("canonical selected");
        assert!(root.owned_root().starts_with(canonical_selected));
        assert!(root.archives_dir.starts_with(root.owned_root()));
        assert!(root.manifests_dir.starts_with(root.owned_root()));
        assert!(root.staging_dir.starts_with(root.owned_root()));
        assert!(!root.root_id().is_empty());
    }

    #[test]
    fn missing_root_is_unavailable() {
        let selected = tempfile::tempdir().expect("temp root");
        let missing = selected.path().join("does-not-exist");
        assert_eq!(
            prepare_root(&missing),
            Err(FilesystemBackupError::RootUnavailable)
        );
    }

    #[test]
    fn root_picker_status_is_opaque_and_never_serializes_a_path() {
        let status = unavailable_root_status();
        assert_eq!(
            serde_json::to_value(status).expect("status json"),
            serde_json::json!({
                "terminalState": "unavailable",
                "selectedRootId": null,
            })
        );
    }

    #[test]
    fn traversal_archive_ids_are_rejected() {
        for archive_id in ["../escape", "nested/archive", r"nested\archive", ""] {
            assert_eq!(
                validate_archive_id(archive_id),
                Err(FilesystemBackupError::InvalidArchiveId)
            );
        }
    }

    #[test]
    fn write_is_atomic_and_persists_only_non_secret_metadata() {
        let (_selected, root) = temp_root();
        let envelope = fixture_envelope("atomic-1", b"opaque encrypted bytes");
        let record = write_encrypted_archive_at_root(&root, &envelope).expect("write archive");
        let archive_path = root.archives_dir.join("atomic-1.fungbk");
        let manifest_path = root.manifests_dir.join("atomic-1.manifest.json");
        assert_eq!(
            fs::read(&archive_path).expect("archive bytes"),
            envelope.bytes
        );
        assert!(manifest_path.is_file());
        assert!(!root.staging_dir.join("atomic-1.partial").exists());
        assert_eq!(
            read_archive_at_root(&root, "atomic-1", Some(&record.digest)).expect("read"),
            envelope.bytes
        );
        let manifest = fs::read_to_string(manifest_path).expect("manifest");
        assert!(!manifest.contains("recovery_phrase"));
        assert!(!manifest.contains("data_key"));
        assert!(manifest.contains("selectedRootId"));
    }

    #[test]
    fn interrupted_staging_does_not_replace_existing_archive() {
        let (_selected, root) = temp_root();
        let verified = fixture_envelope("verified-1", b"old bytes");
        write_encrypted_archive_at_root(&root, &verified).expect("write verified archive");

        let pending = fixture_envelope("pending-1", b"new bytes");
        let partial = root.staging_dir.join("pending-1.partial");
        fs::write(&partial, b"interrupted").expect("simulate interrupted write");
        assert_eq!(
            write_encrypted_archive_at_root(&root, &pending),
            Err(FilesystemBackupError::StagingExists)
        );
        assert_eq!(
            fs::read(partial).expect("partial preserved"),
            b"interrupted"
        );
        assert_eq!(
            fs::read(root.archives_dir.join("verified-1.fungbk")).expect("old archive"),
            b"old bytes"
        );
    }

    #[test]
    fn digest_mismatch_is_terminal_for_reads() {
        let (_selected, root) = temp_root();
        let envelope = fixture_envelope("digest-1", b"opaque bytes");
        let record = write_encrypted_archive_at_root(&root, &envelope).expect("write archive");
        assert_eq!(
            read_archive_at_root(&root, "digest-1", Some("00")),
            Err(FilesystemBackupError::DigestMismatch)
        );
        assert_eq!(
            read_archive_record_at_root(&root, "digest-1").expect("record"),
            record
        );
    }

    #[test]
    fn symlinked_owned_child_is_rejected_when_platform_allows_symlinks() {
        let selected = tempfile::tempdir().expect("selected root");
        let outside = tempfile::tempdir().expect("outside root");
        let link = selected.path().join(OWNED_FOLDER);

        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path(), &link);
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir(outside.path(), &link);
        #[cfg(not(any(unix, windows)))]
        let linked: std::io::Result<()> = Err(std::io::Error::other("unsupported"));

        if linked.is_err() {
            eprintln!("symlink unavailable; boundary test skipped");
            return;
        }
        assert_eq!(
            prepare_root(selected.path()),
            Err(FilesystemBackupError::SymlinkEscape)
        );
    }

    #[test]
    fn missing_archive_is_reported_without_fabricating_status() {
        let (_selected, root) = temp_root();
        assert_eq!(
            read_archive_at_root(&root, "missing", None),
            Err(FilesystemBackupError::ArchiveNotFound)
        );
    }
}
