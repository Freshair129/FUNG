use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256, Sha512};

use crate::{AppError, AppResult};

const KEY_FILE: &str = "device_identity.key";
const KEYRING_SERVICE: &str = "FUNG";
const DEVICE_IDENTITY_KEYRING_ACCOUNT: &str = "device_identity_keyring";

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

fn identity_keyring_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, DEVICE_IDENTITY_KEYRING_ACCOUNT)
        .map_err(|_| AppError::InvalidInput("device_identity_keyring_unavailable".to_owned()))
}

fn parse_seed(bytes: &[u8]) -> AppResult<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| AppError::InvalidInput("device_identity_keyring_invalid".to_owned()))
}

trait IdentityBackend {
    fn read_keyring(&mut self) -> AppResult<Option<Vec<u8>>>;
    fn write_keyring(&mut self, seed: &[u8; 32]) -> AppResult<()>;
    fn read_legacy(&mut self) -> AppResult<Option<Vec<u8>>>;
    fn remove_legacy(&mut self) -> AppResult<()>;
}

struct OsIdentityBackend {
    dir: PathBuf,
    entry: keyring::Entry,
}

impl OsIdentityBackend {
    fn new(dir: &Path) -> AppResult<Self> {
        Ok(Self {
            dir: dir.to_path_buf(),
            entry: identity_keyring_entry()?,
        })
    }

    fn legacy_path(&self) -> PathBuf {
        self.dir.join(KEY_FILE)
    }
}

impl IdentityBackend for OsIdentityBackend {
    fn read_keyring(&mut self) -> AppResult<Option<Vec<u8>>> {
        match self.entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::InvalidInput(
                "device_identity_keyring_unavailable".to_owned(),
            )),
        }
    }

    fn write_keyring(&mut self, seed: &[u8; 32]) -> AppResult<()> {
        self.entry
            .set_secret(seed)
            .map_err(|_| AppError::InvalidInput("device_identity_keyring_write_failed".to_owned()))
    }

    fn read_legacy(&mut self) -> AppResult<Option<Vec<u8>>> {
        let path = self.legacy_path();
        if !path.is_file() {
            return Ok(None);
        }
        let encoded = fs::read_to_string(path).map_err(|error| io_error("identity read", error))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|error| io_error("identity decode", error))?;
        Ok(Some(bytes))
    }

    fn remove_legacy(&mut self) -> AppResult<()> {
        let path = self.legacy_path();
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| io_error("identity legacy removal", error))?;
            if path.exists() {
                return Err(AppError::InvalidInput(
                    "device_identity_legacy_removal_failed".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

struct MigratedIdentity {
    seed: [u8; 32],
    created: bool,
}

fn migrate_identity_with_backend<B: IdentityBackend>(
    backend: &mut B,
) -> AppResult<MigratedIdentity> {
    let keyring_seed = backend
        .read_keyring()?
        .map(|bytes| parse_seed(&bytes))
        .transpose()?;
    let legacy_seed = backend
        .read_legacy()?
        .map(|bytes| parse_seed(&bytes))
        .transpose()?;

    let (seed, created) = match (keyring_seed, legacy_seed) {
        (Some(keyring_seed), Some(legacy_seed)) if keyring_seed != legacy_seed => {
            return Err(AppError::InvalidInput(
                "device_identity_conflict".to_owned(),
            ));
        }
        (Some(seed), Some(_)) => (seed, false),
        (Some(seed), None) => (seed, false),
        (None, Some(seed)) => {
            backend.write_keyring(&seed)?;
            let verified = backend.read_keyring()?.ok_or_else(|| {
                AppError::InvalidInput("device_identity_keyring_readback_failed".to_owned())
            })?;
            let verified = parse_seed(&verified).map_err(|_| {
                AppError::InvalidInput("device_identity_keyring_readback_failed".to_owned())
            })?;
            if verified != seed {
                return Err(AppError::InvalidInput(
                    "device_identity_keyring_readback_failed".to_owned(),
                ));
            }
            (seed, false)
        }
        (None, None) => {
            let generated = SigningKey::generate(&mut rand::rngs::OsRng).to_bytes();
            backend.write_keyring(&generated)?;
            let verified = backend.read_keyring()?.ok_or_else(|| {
                AppError::InvalidInput("device_identity_keyring_readback_failed".to_owned())
            })?;
            let verified = parse_seed(&verified).map_err(|_| {
                AppError::InvalidInput("device_identity_keyring_readback_failed".to_owned())
            })?;
            if verified != generated {
                return Err(AppError::InvalidInput(
                    "device_identity_keyring_readback_failed".to_owned(),
                ));
            }
            (generated, true)
        }
    };

    if legacy_seed.is_some() {
        backend.remove_legacy()?;
    }

    Ok(MigratedIdentity { seed, created })
}

fn identity_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn identity_from_seed(seed: &[u8; 32], created: bool) -> DeviceIdentity {
    let key = SigningKey::from_bytes(seed);
    DeviceIdentity {
        fingerprint: fingerprint_of(&key),
        created,
    }
}

/// Secure desktop identity lifecycle. A legacy file is accepted only as a
/// migration input: the keyring write and verified readback must succeed before
/// the file is removed. A conflicting file/keyring pair fails closed so the
/// device fingerprint can never silently change.
pub(crate) fn secure_identity_in_dir(dir: &Path) -> AppResult<DeviceIdentity> {
    let _guard = identity_lock()
        .lock()
        .map_err(|_| AppError::InvalidInput("device_identity_keyring_unavailable".to_owned()))?;
    fs::create_dir_all(dir).map_err(|e| io_error("identity dir", e))?;
    let mut backend = OsIdentityBackend::new(dir)?;
    let migrated = migrate_identity_with_backend(&mut backend)?;
    Ok(identity_from_seed(&migrated.seed, migrated.created))
}

pub(crate) fn secure_signing_key_in_dir(dir: &Path) -> AppResult<SigningKey> {
    secure_identity_in_dir(dir)?;
    let seed = OsIdentityBackend::new(dir)?
        .read_keyring()?
        .map(|bytes| parse_seed(&bytes))
        .transpose()?
        .ok_or_else(|| AppError::InvalidInput("device_identity_keyring_unavailable".to_owned()))?;
    Ok(SigningKey::from_bytes(&seed))
}

pub(crate) fn authorization_identity_in_dir(dir: &Path) -> AppResult<(String, String)> {
    let key = secure_signing_key_in_dir(dir)?;
    let public_key = key.verifying_key();
    Ok((
        base64::engine::general_purpose::STANDARD.encode(public_key.as_bytes()),
        fingerprint_of(&key),
    ))
}

pub(crate) fn sign_authorization_in_dir(dir: &Path, message: &[u8]) -> AppResult<String> {
    let key = secure_signing_key_in_dir(dir)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(key.sign(message).to_bytes()))
}

/// Legacy file-backed identity helper used by FUNGWIRE fixture tests and for
/// reading pre-migration test data. Desktop commands use `secure_identity_in_dir`
/// above and never create a new plaintext identity file.
#[cfg(any(not(desktop), test))]
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
    #[cfg(desktop)]
    {
        secure_identity_in_dir(&dir)
    }
    #[cfg(not(desktop))]
    {
        ensure_identity_in_dir(&dir)
    }
}

/// Load the raw seed for the FUNGWIRE boundary. Existing fixture files remain
/// supported; migrated desktop identities are read from the OS keyring.
fn load_seed(dir: &Path) -> AppResult<[u8; 32]> {
    #[cfg(all(desktop, not(test)))]
    {
        secure_identity_in_dir(dir)?;
        OsIdentityBackend::new(dir)?
            .read_keyring()?
            .map(|bytes| parse_seed(&bytes))
            .transpose()?
            .ok_or_else(|| io_error("identity read", "secure identity missing"))
    }
    #[cfg(any(not(desktop), test))]
    {
        let path = dir.join(KEY_FILE);
        read_legacy_seed(&path)?.ok_or_else(|| io_error("identity read", "identity missing"))
    }
}

#[cfg(any(not(desktop), test))]
fn read_legacy_seed(path: &Path) -> AppResult<Option<[u8; 32]>> {
    if !path.is_file() {
        return Ok(None);
    }
    let encoded = fs::read_to_string(path).map_err(|error| io_error("identity read", error))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|error| io_error("identity decode", error))?;
    let seed = bytes
        .try_into()
        .map_err(|_| io_error("identity key length", "expected 32 bytes"))?;
    Ok(Some(seed))
}

/// Export the full ed25519 verifying key as base64, for publishing to Supabase.
#[cfg(test)]
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
    #[cfg(desktop)]
    let key = secure_signing_key_in_dir(&dir)?;
    #[cfg(not(desktop))]
    let key = SigningKey::from_bytes(&load_seed(&dir)?);
    Ok(base64::engine::general_purpose::STANDARD.encode(key.verifying_key().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeIdentityBackend {
        keyring: Option<Vec<u8>>,
        legacy: Option<Vec<u8>>,
        readback: Option<Vec<u8>>,
        read_keyring_count: usize,
        fail_write: bool,
        fail_remove: bool,
        removed: bool,
        events: Vec<&'static str>,
    }

    impl FakeIdentityBackend {
        fn with_legacy(seed: [u8; 32]) -> Self {
            Self {
                keyring: None,
                legacy: Some(seed.to_vec()),
                readback: None,
                read_keyring_count: 0,
                fail_write: false,
                fail_remove: false,
                removed: false,
                events: Vec::new(),
            }
        }

        fn with_readback(mut self, seed: [u8; 32]) -> Self {
            self.readback = Some(seed.to_vec());
            self
        }

        fn fail_write(mut self) -> Self {
            self.fail_write = true;
            self
        }

        fn fail_remove(mut self) -> Self {
            self.fail_remove = true;
            self
        }

        fn legacy_present(&self) -> bool {
            self.legacy.is_some()
        }

        fn was_removed(&self) -> bool {
            self.removed
        }

        fn events(&self) -> &[&'static str] {
            &self.events
        }
    }

    impl IdentityBackend for FakeIdentityBackend {
        fn read_keyring(&mut self) -> AppResult<Option<Vec<u8>>> {
            self.events.push("read_keyring");
            let value = if self.read_keyring_count == 0 {
                self.keyring.clone()
            } else {
                self.readback.clone().or_else(|| self.keyring.clone())
            };
            self.read_keyring_count += 1;
            Ok(value)
        }

        fn write_keyring(&mut self, seed: &[u8; 32]) -> AppResult<()> {
            self.events.push("write_keyring");
            if self.fail_write {
                return Err(AppError::InvalidInput("fake_write_failed".to_owned()));
            }
            self.keyring = Some(seed.to_vec());
            Ok(())
        }

        fn read_legacy(&mut self) -> AppResult<Option<Vec<u8>>> {
            self.events.push("read_legacy");
            Ok(self.legacy.clone())
        }

        fn remove_legacy(&mut self) -> AppResult<()> {
            self.events.push("remove_legacy");
            self.removed = true;
            if self.fail_remove {
                return Err(AppError::InvalidInput("fake_remove_failed".to_owned()));
            }
            self.legacy = None;
            Ok(())
        }
    }

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

    #[test]
    fn legacy_migration_writes_reads_compares_then_removes_legacy_key() {
        let seed = [7u8; 32];
        let mut backend = FakeIdentityBackend::with_legacy(seed);

        let migrated = migrate_identity_with_backend(&mut backend).unwrap();

        assert_eq!(migrated.seed, seed);
        assert!(!backend.legacy_present());
        assert_eq!(
            backend.events(),
            [
                "read_keyring",
                "read_legacy",
                "write_keyring",
                "read_keyring",
                "remove_legacy"
            ]
        );
    }

    #[test]
    fn legacy_migration_keeps_file_when_keyring_readback_differs() {
        let seed = [8u8; 32];
        let mut backend = FakeIdentityBackend::with_legacy(seed).with_readback([9u8; 32]);

        assert!(migrate_identity_with_backend(&mut backend).is_err());
        assert!(backend.legacy_present());
        assert!(!backend.was_removed());
        assert_eq!(
            backend.events(),
            [
                "read_keyring",
                "read_legacy",
                "write_keyring",
                "read_keyring"
            ]
        );
    }

    #[test]
    fn legacy_migration_keeps_file_when_write_or_remove_fails() {
        let seed = [10u8; 32];

        let mut write_failure = FakeIdentityBackend::with_legacy(seed).fail_write();
        assert!(migrate_identity_with_backend(&mut write_failure).is_err());
        assert!(write_failure.legacy_present());
        assert!(!write_failure.was_removed());

        let mut remove_failure = FakeIdentityBackend::with_legacy(seed).fail_remove();
        assert!(migrate_identity_with_backend(&mut remove_failure).is_err());
        assert!(remove_failure.legacy_present());
        assert!(remove_failure.was_removed());
    }
}
