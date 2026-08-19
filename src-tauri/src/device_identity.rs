use std::fs;
use std::io;
use std::path::Path;

use base64::Engine;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256, Sha512};

use crate::{AppError, AppResult};

const KEY_FILE: &str = "device_identity.key";

#[derive(Debug, Clone, Serialize)]
pub struct DeviceIdentity {
    pub fingerprint: String,
    pub created: bool,
}

fn io_error(context: &str, error: impl std::fmt::Display) -> AppError {
    AppError::Io(io::Error::other(format!("{context}: {error}")))
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

/// Load the raw 32-byte ed25519 seed (same file/format as ensure_identity_in_dir).
fn load_seed(dir: &Path) -> AppResult<[u8; 32]> {
    let path = dir.join(KEY_FILE);
    let encoded = fs::read_to_string(&path).map_err(|e| io_error("identity read", e))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| io_error("identity decode", e))?;
    bytes
        .try_into()
        .map_err(|_| io_error("identity key length", "expected 32 bytes"))
}

/// Export the full ed25519 verifying key as base64, for publishing to Supabase.
pub fn public_key_b64_in_dir(dir: &Path) -> AppResult<String> {
    let seed = load_seed(dir)?;
    let key = SigningKey::from_bytes(&seed);
    Ok(base64::engine::general_purpose::STANDARD.encode(key.verifying_key().as_bytes()))
}

/// X25519 static secret derived from the ed25519 seed via the standard map:
/// SHA-512(seed)[..32], clamped. (x25519_dalek clamps on use, but we clamp here
/// so the raw bytes handed to `snow` are already a valid X25519 scalar.)
pub fn x25519_static_secret_in_dir(dir: &Path) -> AppResult<[u8; 32]> {
    let seed = load_seed(dir)?;
    let hash = Sha512::digest(seed);
    let mut s = [0u8; 32];
    s.copy_from_slice(&hash[..32]);
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;
    Ok(s)
}

/// Convert a peer's published ed25519 public key to its X25519 (Montgomery u) form.
///
/// Validates the bytes as a real ed25519 verifying key first (rejects points not
/// on the curve / small-order points that VerifyingKey::from_bytes screens out),
/// then decompresses the same bytes as an Edwards point and maps to Montgomery
/// form via the standard birational equivalence.
pub fn x25519_public_from_ed25519_b64(ed_pub_b64: &str) -> AppResult<[u8; 32]> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(ed_pub_b64.trim())
        .map_err(|e| io_error("peer pubkey decode", e))?;
    let arr: [u8; 32] = raw
        .try_into()
        .map_err(|_| io_error("peer pubkey length", "expected 32 bytes"))?;
    let _vk = VerifyingKey::from_bytes(&arr).map_err(|e| io_error("peer pubkey parse", e))?;
    let point = curve25519_dalek::edwards::CompressedEdwardsY(arr)
        .decompress()
        .ok_or_else(|| io_error("peer pubkey decompress", "invalid Edwards point"))?;
    Ok(point.to_montgomery().to_bytes())
}

#[tauri::command]
pub fn device_public_key(app: tauri::AppHandle) -> AppResult<String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::MissingAppDataDir)?;
    public_key_b64_in_dir(&dir)
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
        assert!(id
            .fingerprint
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn public_key_b64_hashes_to_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let id = ensure_identity_in_dir(dir.path()).unwrap();
        let pub_b64 = public_key_b64_in_dir(dir.path()).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&pub_b64)
            .unwrap();
        let digest: String = sha2::Sha256::digest(&raw)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(digest, id.fingerprint);
        assert_eq!(raw.len(), 32);
    }

    #[test]
    fn x25519_conversion_enables_ecdh_agreement() {
        // Two identities; each converts its own secret and the other's public.
        // The X25519 ECDH must agree in both directions (proves the birational map is correct).
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(a.path()).unwrap();
        ensure_identity_in_dir(b.path()).unwrap();
        let a_sec = x25519_static_secret_in_dir(a.path()).unwrap();
        let b_sec = x25519_static_secret_in_dir(b.path()).unwrap();
        let a_pub =
            x25519_public_from_ed25519_b64(&public_key_b64_in_dir(a.path()).unwrap()).unwrap();
        let b_pub =
            x25519_public_from_ed25519_b64(&public_key_b64_in_dir(b.path()).unwrap()).unwrap();
        let ab = x25519_dalek::x25519(a_sec, b_pub);
        let ba = x25519_dalek::x25519(b_sec, a_pub);
        assert_eq!(ab, ba);
    }
}
