use std::fs;
use std::io;
use std::path::Path;

use base64::Engine;
use ed25519_dalek::SigningKey;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{AppError, AppResult};

const KEY_FILE: &str = "device_identity.key";

#[derive(Debug, Clone, Serialize)]
pub struct DeviceIdentity {
    pub fingerprint: String,
    pub created: bool,
}

fn io_error(context: &str, error: impl std::fmt::Display) -> AppError {
    AppError::Io(io::Error::new(
        io::ErrorKind::Other,
        format!("{context}: {error}"),
    ))
}

fn fingerprint_of(signing_key: &SigningKey) -> String {
    let public = signing_key.verifying_key();
    let digest = Sha256::digest(public.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// File-backed identity storage inside the app data dir. On Windows the file
/// lives in %APPDATA%; on Android inside the app-private files dir. Keystore /
/// keyring hardening is a Phase 1 backlog item (spec §15.4).
pub fn ensure_identity_in_dir(dir: &Path) -> AppResult<DeviceIdentity> {
    fs::create_dir_all(dir).map_err(|e| io_error("identity dir", e))?;
    let path = dir.join(KEY_FILE);
    if path.exists() {
        let encoded = fs::read_to_string(&path).map_err(|e| io_error("identity read", e))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| io_error("identity decode", e))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| io_error("identity key length", "expected 32 bytes"))?;
        let key = SigningKey::from_bytes(&arr);
        return Ok(DeviceIdentity {
            fingerprint: fingerprint_of(&key),
            created: false,
        });
    }
    let key = SigningKey::generate(&mut rand::rngs::OsRng);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key.to_bytes());
    fs::write(&path, encoded).map_err(|e| io_error("identity write", e))?;
    Ok(DeviceIdentity {
        fingerprint: fingerprint_of(&key),
        created: true,
    })
}

#[tauri::command]
pub fn device_identity_ensure(app: tauri::AppHandle) -> AppResult<DeviceIdentity> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::MissingAppDataDir)?;
    ensure_identity_in_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_and_reloads_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_identity_in_dir(dir.path()).unwrap();
        assert!(first.created);
        let second = ensure_identity_in_dir(dir.path()).unwrap();
        assert!(!second.created);
        assert_eq!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn fingerprint_is_64_hex_chars() {
        let dir = tempfile::tempdir().unwrap();
        let id = ensure_identity_in_dir(dir.path()).unwrap();
        assert_eq!(id.fingerprint.len(), 64);
        assert!(id.fingerprint.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
