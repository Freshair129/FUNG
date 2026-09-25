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
//! private assets        v2+: opaque ciphertexts in manifest order,
//!                       followed by the opaque recovery packages
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
// Keep v1/v2 readable. V3 adds purpose-separated People assets and additional
// owner-wrapped recovery keys while retaining the earlier manifest fields.
const CONTAINER_VERSION_V1: u16 = 1;
const CONTAINER_VERSION_V2: u16 = 2;
const CONTAINER_VERSION_V3: u16 = 3;
const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_PRIVATE_ASSET_COUNT: usize = 10_000;
pub(crate) const MAX_PRIVATE_ASSET_BYTES: u64 = 40 * 1024 * 1024;
const MAX_PRIVATE_ASSETS_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RECOVERY_PACKAGE_BYTES: u64 = 16 * 1024 * 1024;

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
    #[error("backup private asset digest mismatch")]
    PrivateAssetDigestMismatch,
    #[error("backup private asset reference is duplicated")]
    DuplicatePrivateAssetRef,
    #[error("backup private asset vault or account binding is inconsistent")]
    PrivateAssetBindingMismatch,
    #[error("backup private asset metadata is malformed")]
    PrivateAssetMalformed,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    private_meeting_assets: Option<PrivateMeetingAssetsManifest>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PrivateMeetingAssetPurpose {
    KnowledgeDocument,
    KnowledgeEvidence,
    MeetingEvidence,
    AgentDraft,
    DeliveryPreview,
    PeopleProfile,
}

/// An already-encrypted asset. `encrypted_bytes` must contain only opaque
/// ciphertext; this container never accepts or stages source plaintext.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncryptedMeetingAsset {
    pub(crate) asset_ref: String,
    pub(crate) entity_id: String,
    pub(crate) purpose: PrivateMeetingAssetPurpose,
    pub(crate) version: u32,
    pub(crate) vault_id: String,
    pub(crate) account_binding: String,
    pub(crate) ciphertext_sha256: String,
    pub(crate) encrypted_bytes: Vec<u8>,
}

/// A native-generated, already-encrypted package for recovering vault keys.
/// Its bytes remain opaque to this backup container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncryptedNativeRecoveryPackage {
    pub(crate) package_ref: String,
    pub(crate) version: u32,
    pub(crate) vault_id: String,
    pub(crate) account_binding: String,
    pub(crate) ciphertext_sha256: String,
    pub(crate) encrypted_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivateMeetingAssetsBundle {
    pub(crate) contract_version: u16,
    pub(crate) vault_id: String,
    pub(crate) account_binding: String,
    pub(crate) assets: Vec<EncryptedMeetingAsset>,
    pub(crate) recovery_package: EncryptedNativeRecoveryPackage,
    pub(crate) additional_recovery_packages: Vec<EncryptedNativeRecoveryPackage>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateMeetingAssetEntry {
    asset_ref: String,
    entity_id: String,
    purpose: PrivateMeetingAssetPurpose,
    version: u32,
    vault_id: String,
    account_binding: String,
    byte_count: u64,
    ciphertext_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryPackageEntry {
    package_ref: String,
    version: u32,
    vault_id: String,
    account_binding: String,
    byte_count: u64,
    ciphertext_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateMeetingAssetsManifest {
    contract_version: u16,
    vault_id: String,
    account_binding: String,
    assets: Vec<PrivateMeetingAssetEntry>,
    recovery_package: RecoveryPackageEntry,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    additional_recovery_packages: Vec<RecoveryPackageEntry>,
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
    /// Opaque encrypted private meeting assets. Restore callers must hand
    /// these to native custody; this payload layer never writes them to disk.
    pub(crate) private_meeting_assets: Option<PrivateMeetingAssetsBundle>,
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

fn valid_private_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.'))
}

fn valid_asset_file_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_private_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_private_manifest(manifest: &PrivateMeetingAssetsManifest) -> Result<u64, PayloadError> {
    if manifest.contract_version != 1
        || !valid_private_ref(&manifest.vault_id)
        || !valid_private_ref(&manifest.account_binding)
        || manifest.assets.len() > MAX_PRIVATE_ASSET_COUNT
    {
        return Err(PayloadError::PrivateAssetMalformed);
    }
    let mut refs = HashSet::with_capacity(
        manifest.assets.len() + 1 + manifest.additional_recovery_packages.len(),
    );
    let mut total = 0_u64;
    for asset in &manifest.assets {
        if !valid_asset_file_ref(&asset.asset_ref)
            || !valid_private_ref(&asset.entity_id)
            || asset.version == 0
            || !valid_private_digest(&asset.ciphertext_sha256)
            || asset.byte_count < 16
        {
            return Err(PayloadError::PrivateAssetMalformed);
        }
        if !refs.insert(asset.asset_ref.as_str()) {
            return Err(PayloadError::DuplicatePrivateAssetRef);
        }
        if asset.vault_id != manifest.vault_id || asset.account_binding != manifest.account_binding
        {
            return Err(PayloadError::PrivateAssetBindingMismatch);
        }
        if asset.byte_count > MAX_PRIVATE_ASSET_BYTES {
            return Err(PayloadError::TooLarge {
                actual: asset.byte_count,
            });
        }
        total = total
            .checked_add(asset.byte_count)
            .ok_or(PayloadError::TooLarge { actual: u64::MAX })?;
    }
    if manifest.additional_recovery_packages.len() > 15 {
        return Err(PayloadError::PrivateAssetMalformed);
    }
    for recovery in std::iter::once(&manifest.recovery_package)
        .chain(manifest.additional_recovery_packages.iter())
    {
        if !valid_private_ref(&recovery.package_ref)
            || recovery.version == 0
            || !valid_private_digest(&recovery.ciphertext_sha256)
            || recovery.byte_count < 16
        {
            return Err(PayloadError::PrivateAssetMalformed);
        }
        if !refs.insert(recovery.package_ref.as_str()) {
            return Err(PayloadError::DuplicatePrivateAssetRef);
        }
        if recovery.vault_id != manifest.vault_id
            || recovery.account_binding != manifest.account_binding
        {
            return Err(PayloadError::PrivateAssetBindingMismatch);
        }
        if recovery.byte_count > MAX_RECOVERY_PACKAGE_BYTES {
            return Err(PayloadError::TooLarge {
                actual: recovery.byte_count,
            });
        }
        total = total
            .checked_add(recovery.byte_count)
            .ok_or(PayloadError::TooLarge { actual: u64::MAX })?;
    }
    if total > MAX_PRIVATE_ASSETS_BYTES {
        return Err(PayloadError::TooLarge { actual: total });
    }
    Ok(total)
}

fn private_manifest_for(
    bundle: &PrivateMeetingAssetsBundle,
) -> Result<(PrivateMeetingAssetsManifest, u64), PayloadError> {
    if bundle.contract_version != 1
        || !valid_private_ref(&bundle.vault_id)
        || !valid_private_ref(&bundle.account_binding)
        || bundle.assets.len() > MAX_PRIVATE_ASSET_COUNT
    {
        return Err(PayloadError::PrivateAssetMalformed);
    }
    let assets: Vec<PrivateMeetingAssetEntry> = bundle
        .assets
        .iter()
        .map(|asset| {
            if asset.vault_id != bundle.vault_id || asset.account_binding != bundle.account_binding
            {
                return Err(PayloadError::PrivateAssetBindingMismatch);
            }
            if asset.ciphertext_sha256 != sha256_hex(&asset.encrypted_bytes) {
                return Err(PayloadError::PrivateAssetDigestMismatch);
            }
            Ok(PrivateMeetingAssetEntry {
                asset_ref: asset.asset_ref.clone(),
                entity_id: asset.entity_id.clone(),
                purpose: asset.purpose,
                version: asset.version,
                vault_id: asset.vault_id.clone(),
                account_binding: asset.account_binding.clone(),
                byte_count: asset.encrypted_bytes.len() as u64,
                ciphertext_sha256: asset.ciphertext_sha256.clone(),
            })
        })
        .collect::<Result<_, _>>()?;
    let recovery = &bundle.recovery_package;
    if recovery.vault_id != bundle.vault_id || recovery.account_binding != bundle.account_binding {
        return Err(PayloadError::PrivateAssetBindingMismatch);
    }
    if recovery.ciphertext_sha256 != sha256_hex(&recovery.encrypted_bytes) {
        return Err(PayloadError::PrivateAssetDigestMismatch);
    }
    let additional_recovery_packages = bundle
        .additional_recovery_packages
        .iter()
        .map(|recovery| {
            if recovery.vault_id != bundle.vault_id
                || recovery.account_binding != bundle.account_binding
            {
                return Err(PayloadError::PrivateAssetBindingMismatch);
            }
            if recovery.ciphertext_sha256 != sha256_hex(&recovery.encrypted_bytes) {
                return Err(PayloadError::PrivateAssetDigestMismatch);
            }
            Ok(RecoveryPackageEntry {
                package_ref: recovery.package_ref.clone(),
                version: recovery.version,
                vault_id: recovery.vault_id.clone(),
                account_binding: recovery.account_binding.clone(),
                byte_count: recovery.encrypted_bytes.len() as u64,
                ciphertext_sha256: recovery.ciphertext_sha256.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = PrivateMeetingAssetsManifest {
        contract_version: bundle.contract_version,
        vault_id: bundle.vault_id.clone(),
        account_binding: bundle.account_binding.clone(),
        assets,
        recovery_package: RecoveryPackageEntry {
            package_ref: recovery.package_ref.clone(),
            version: recovery.version,
            vault_id: recovery.vault_id.clone(),
            account_binding: recovery.account_binding.clone(),
            byte_count: recovery.encrypted_bytes.len() as u64,
            ciphertext_sha256: recovery.ciphertext_sha256.clone(),
        },
        additional_recovery_packages,
    };
    let total = validate_private_manifest(&manifest)?;
    Ok((manifest, total))
}

/// Retains the existing v1 format when no private assets are supplied.
#[allow(dead_code)] // The v1 packer remains available for legacy backup compatibility tests.
pub(crate) fn pack(
    genesis_bundle: &[u8],
    inventory: &AudioInventory,
) -> Result<Vec<u8>, PayloadError> {
    pack_with_private_meeting_assets(genesis_bundle, inventory, None)
}

/// Assembles the plaintext that the outer authenticated archive encrypts.
/// A supplied private-assets bundle selects container v3; all asset and key
/// recovery bytes must already be encrypted by their native custody owners.
pub(crate) fn pack_with_private_meeting_assets(
    genesis_bundle: &[u8],
    inventory: &AudioInventory,
    private_assets: Option<&PrivateMeetingAssetsBundle>,
) -> Result<Vec<u8>, PayloadError> {
    let audio_bytes = inventory.stored_bytes();
    let (version, private_manifest, private_bytes) = match private_assets {
        Some(bundle) => {
            let (manifest, bytes) = private_manifest_for(bundle)?;
            (CONTAINER_VERSION_V3, Some(manifest), bytes)
        }
        None => (CONTAINER_VERSION_V1, None, 0),
    };
    let total = (genesis_bundle.len() as u64)
        .checked_add(audio_bytes)
        .and_then(|bytes| bytes.checked_add(private_bytes))
        .ok_or(PayloadError::TooLarge { actual: u64::MAX })?;
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
        format_version: version,
        genesis_bundle: BundleEntry {
            byte_count: genesis_bundle.len() as u64,
            sha256: sha256_hex(genesis_bundle),
        },
        audio,
        private_meeting_assets: private_manifest,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| PayloadError::Malformed)?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(PayloadError::TooLarge {
            actual: manifest_bytes.len() as u64,
        });
    }

    let mut out = Vec::with_capacity(
        MAGIC.len()
            + 2
            + 4
            + manifest_bytes.len()
            + genesis_bundle.len()
            + audio_bytes as usize
            + private_bytes as usize,
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&manifest_bytes);
    out.extend_from_slice(genesis_bundle);
    for file in &inventory.staged {
        out.extend_from_slice(&file.bytes);
    }
    if let Some(bundle) = private_assets {
        for asset in &bundle.assets {
            out.extend_from_slice(&asset.encrypted_bytes);
        }
        out.extend_from_slice(&bundle.recovery_package.encrypted_bytes);
        for recovery in &bundle.additional_recovery_packages {
            out.extend_from_slice(&recovery.encrypted_bytes);
        }
    }
    Ok(out)
}

/// Splits a decrypted plaintext back into the Genesis bundle and its audio.
/// Plaintext without the container magic is a pre-container archive and is
/// returned as a bundle with no audio.
fn unpack_private_assets(
    manifest: &PrivateMeetingAssetsManifest,
    plaintext: &[u8],
    offset: &mut usize,
) -> Result<PrivateMeetingAssetsBundle, PayloadError> {
    validate_private_manifest(manifest)?;
    let mut assets = Vec::with_capacity(manifest.assets.len());
    for entry in &manifest.assets {
        let byte_count =
            usize::try_from(entry.byte_count).map_err(|_| PayloadError::PrivateAssetMalformed)?;
        let encrypted_bytes = take(plaintext, offset, byte_count)?.to_vec();
        if sha256_hex(&encrypted_bytes) != entry.ciphertext_sha256 {
            return Err(PayloadError::PrivateAssetDigestMismatch);
        }
        assets.push(EncryptedMeetingAsset {
            asset_ref: entry.asset_ref.clone(),
            entity_id: entry.entity_id.clone(),
            purpose: entry.purpose,
            version: entry.version,
            vault_id: entry.vault_id.clone(),
            account_binding: entry.account_binding.clone(),
            ciphertext_sha256: entry.ciphertext_sha256.clone(),
            encrypted_bytes,
        });
    }
    let recovery = &manifest.recovery_package;
    let byte_count =
        usize::try_from(recovery.byte_count).map_err(|_| PayloadError::PrivateAssetMalformed)?;
    let encrypted_bytes = take(plaintext, offset, byte_count)?.to_vec();
    if sha256_hex(&encrypted_bytes) != recovery.ciphertext_sha256 {
        return Err(PayloadError::PrivateAssetDigestMismatch);
    }
    let mut additional_recovery_packages =
        Vec::with_capacity(manifest.additional_recovery_packages.len());
    for recovery in &manifest.additional_recovery_packages {
        let byte_count = usize::try_from(recovery.byte_count)
            .map_err(|_| PayloadError::PrivateAssetMalformed)?;
        let encrypted_bytes = take(plaintext, offset, byte_count)?.to_vec();
        if sha256_hex(&encrypted_bytes) != recovery.ciphertext_sha256 {
            return Err(PayloadError::PrivateAssetDigestMismatch);
        }
        additional_recovery_packages.push(EncryptedNativeRecoveryPackage {
            package_ref: recovery.package_ref.clone(),
            version: recovery.version,
            vault_id: recovery.vault_id.clone(),
            account_binding: recovery.account_binding.clone(),
            ciphertext_sha256: recovery.ciphertext_sha256.clone(),
            encrypted_bytes,
        });
    }
    Ok(PrivateMeetingAssetsBundle {
        contract_version: manifest.contract_version,
        vault_id: manifest.vault_id.clone(),
        account_binding: manifest.account_binding.clone(),
        assets,
        recovery_package: EncryptedNativeRecoveryPackage {
            package_ref: recovery.package_ref.clone(),
            version: recovery.version,
            vault_id: recovery.vault_id.clone(),
            account_binding: recovery.account_binding.clone(),
            ciphertext_sha256: recovery.ciphertext_sha256.clone(),
            encrypted_bytes,
        },
        additional_recovery_packages,
    })
}

pub(crate) fn unpack(plaintext: &[u8]) -> Result<UnpackedPayload, PayloadError> {
    if plaintext.len() < MAGIC.len() || &plaintext[..MAGIC.len()] != MAGIC {
        return Ok(UnpackedPayload {
            genesis_bundle: plaintext.to_vec(),
            audio: Vec::new(),
            stored_bytes: Vec::new(),
            private_meeting_assets: None,
        });
    }

    let mut offset = MAGIC.len();
    let version = read_u16(plaintext, &mut offset)?;
    if version != CONTAINER_VERSION_V1
        && version != CONTAINER_VERSION_V2
        && version != CONTAINER_VERSION_V3
    {
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
    match (version, manifest.private_meeting_assets.is_some()) {
        (CONTAINER_VERSION_V1, true)
        | (CONTAINER_VERSION_V2, false)
        | (CONTAINER_VERSION_V3, false) => return Err(PayloadError::Malformed),
        _ => {}
    }

    let bundle_len =
        usize::try_from(manifest.genesis_bundle.byte_count).map_err(|_| PayloadError::Malformed)?;
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
    let private_meeting_assets = manifest
        .private_meeting_assets
        .as_ref()
        .map(|private_manifest| unpack_private_assets(private_manifest, plaintext, &mut offset))
        .transpose()?;
    if offset != plaintext.len() {
        return Err(PayloadError::Malformed);
    }

    Ok(UnpackedPayload {
        genesis_bundle,
        audio: manifest.audio,
        stored_bytes,
        private_meeting_assets,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrivateMeetingAssetsRestoreSummary {
    pub(crate) restored_asset_count: usize,
    pub(crate) recovery_package_saved: bool,
    pub(crate) owner_key_verified: bool,
}

/// Restores only ciphertext and a phrase-wrapped key package below the fresh
/// target. It never installs ownership or writes a decrypted vault key.
pub(crate) fn extract_private_meeting_assets(
    target_root: &Path,
    payload: &UnpackedPayload,
) -> Result<PrivateMeetingAssetsRestoreSummary, PayloadError> {
    let Some(bundle) = payload.private_meeting_assets.as_ref() else {
        return Ok(PrivateMeetingAssetsRestoreSummary::default());
    };
    let root = fs::canonicalize(target_root).map_err(|_| PayloadError::ExtractFailed)?;
    let knowledge_dir = ensure_output_directory(&root, &["meeting-assets", "knowledge"])?;
    let people_dir = ensure_output_directory(&root, &["meeting-assets", "people"])?;
    let recovery_dir = ensure_output_directory(&root, &["meeting-assets", "recovery"])?;
    for asset in &bundle.assets {
        if !valid_asset_file_ref(&asset.asset_ref)
            || asset.ciphertext_sha256 != sha256_hex(&asset.encrypted_bytes)
        {
            return Err(PayloadError::PrivateAssetDigestMismatch);
        }
        let destination = match asset.purpose {
            PrivateMeetingAssetPurpose::PeopleProfile => {
                people_dir.join(format!("{}.enc", asset.asset_ref))
            }
            _ => knowledge_dir.join(format!("{}.enc", asset.asset_ref)),
        };
        write_new_file(&destination, &asset.encrypted_bytes)?;
    }

    let package = &bundle.recovery_package;
    if package.ciphertext_sha256 != sha256_hex(&package.encrypted_bytes) {
        return Err(PayloadError::PrivateAssetDigestMismatch);
    }
    let mut recovery_packages = Vec::with_capacity(1 + bundle.additional_recovery_packages.len());
    recovery_packages.push(package);
    recovery_packages.extend(bundle.additional_recovery_packages.iter());
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct RecoveryMetadata<'a> {
        package_ref: &'a str,
        version: u32,
        vault_id: &'a str,
        account_binding: &'a str,
        ciphertext_sha256: &'a str,
    }
    for package in &recovery_packages {
        if package.ciphertext_sha256 != sha256_hex(&package.encrypted_bytes) {
            return Err(PayloadError::PrivateAssetDigestMismatch);
        }
        let package_name = sha256_hex(package.package_ref.as_bytes());
        write_new_file(
            &recovery_dir.join(format!("{package_name}.pkg")),
            &package.encrypted_bytes,
        )?;
        let metadata = RecoveryMetadata {
            package_ref: &package.package_ref,
            version: package.version,
            vault_id: &package.vault_id,
            account_binding: &package.account_binding,
            ciphertext_sha256: &package.ciphertext_sha256,
        };
        let metadata_bytes =
            serde_json::to_vec(&metadata).map_err(|_| PayloadError::ExtractFailed)?;
        write_new_file(
            &recovery_dir.join(format!("{package_name}.json")),
            &metadata_bytes,
        )?;
    }

    Ok(PrivateMeetingAssetsRestoreSummary {
        restored_asset_count: bundle.assets.len(),
        recovery_package_saved: true,
        owner_key_verified: false,
    })
}

fn ensure_output_directory(root: &Path, components: &[&str]) -> Result<PathBuf, PayloadError> {
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(PayloadError::ExtractFailed);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|_| PayloadError::ExtractFailed)?;
            }
            Err(_) => return Err(PayloadError::ExtractFailed),
        }
        let canonical = fs::canonicalize(&current).map_err(|_| PayloadError::ExtractFailed)?;
        if !canonical.starts_with(root) {
            return Err(PayloadError::UnsafeEntryPath);
        }
        current = canonical;
    }
    Ok(current)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), PayloadError> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| PayloadError::ExtractFailed)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| PayloadError::ExtractFailed)
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

    fn private_bundle() -> PrivateMeetingAssetsBundle {
        // Deterministic binary placeholders: this layer treats bytes as opaque
        // ciphertext and never interprets or stages source plaintext.
        let encrypted_asset = vec![0xa5; 32];
        let encrypted_recovery = vec![0x3c; 48];
        PrivateMeetingAssetsBundle {
            contract_version: 1,
            vault_id: "vault:fixture-1".to_string(),
            account_binding: "principal:fixture-1".to_string(),
            assets: vec![EncryptedMeetingAsset {
                asset_ref: "asset-document-version-1".to_string(),
                entity_id: "document-1".to_string(),
                purpose: PrivateMeetingAssetPurpose::KnowledgeDocument,
                version: 1,
                vault_id: "vault:fixture-1".to_string(),
                account_binding: "principal:fixture-1".to_string(),
                ciphertext_sha256: sha256_hex(&encrypted_asset),
                encrypted_bytes: encrypted_asset,
            }],
            recovery_package: EncryptedNativeRecoveryPackage {
                package_ref: "recovery-package:fixture-1".to_string(),
                version: 1,
                vault_id: "vault:fixture-1".to_string(),
                account_binding: "principal:fixture-1".to_string(),
                ciphertext_sha256: sha256_hex(&encrypted_recovery),
                encrypted_bytes: encrypted_recovery,
            },
            additional_recovery_packages: Vec::new(),
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
        assert!(payload.private_meeting_assets.is_none());
    }

    #[test]
    fn legacy_and_existing_v1_payloads_remain_supported_without_private_assets() {
        let packed_v1 = pack(b"bundle-v1", &AudioInventory::default()).expect("pack v1");
        let mut offset = MAGIC.len();
        assert_eq!(
            read_u16(&packed_v1, &mut offset).expect("version"),
            CONTAINER_VERSION_V1
        );
        let payload = unpack(&packed_v1).expect("unpack existing v1");
        assert_eq!(payload.genesis_bundle, b"bundle-v1");
        assert!(payload.private_meeting_assets.is_none());
    }

    #[test]
    fn private_assets_round_trip_inside_explicit_v3_container() {
        let private = private_bundle();
        let packed = pack_with_private_meeting_assets(
            b"genesis-bundle-v3",
            &AudioInventory::default(),
            Some(&private),
        )
        .expect("pack private assets");
        let mut offset = MAGIC.len();
        assert_eq!(
            read_u16(&packed, &mut offset).expect("version"),
            CONTAINER_VERSION_V3
        );

        let payload = unpack(&packed).expect("unpack private assets");
        assert_eq!(payload.genesis_bundle, b"genesis-bundle-v3");
        assert_eq!(payload.private_meeting_assets, Some(private));
    }

    #[test]
    fn people_assets_and_distinct_key_packages_round_trip_separately() {
        let mut private = private_bundle();
        let people_ciphertext = vec![0x71; 40];
        private.assets.push(EncryptedMeetingAsset {
            asset_ref: "asset-people-profile-1".to_string(),
            entity_id: "profile:fixture-1".to_string(),
            purpose: PrivateMeetingAssetPurpose::PeopleProfile,
            version: 1,
            vault_id: private.vault_id.clone(),
            account_binding: private.account_binding.clone(),
            ciphertext_sha256: sha256_hex(&people_ciphertext),
            encrypted_bytes: people_ciphertext.clone(),
        });
        let people_recovery = vec![0x4d; 48];
        private
            .additional_recovery_packages
            .push(EncryptedNativeRecoveryPackage {
                package_ref: "people-metadata-key:fixture-1".to_string(),
                version: 1,
                vault_id: private.vault_id.clone(),
                account_binding: private.account_binding.clone(),
                ciphertext_sha256: sha256_hex(&people_recovery),
                encrypted_bytes: people_recovery.clone(),
            });

        let packed =
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private))
                .expect("pack people assets");
        let restored = unpack(&packed).expect("unpack people assets");
        assert_eq!(restored.private_meeting_assets, Some(private.clone()));

        let target = TempDir::new().expect("fresh target");
        extract_private_meeting_assets(target.path(), &restored).expect("extract people assets");
        assert_eq!(
            fs::read(
                target
                    .path()
                    .join("meeting-assets/people/asset-people-profile-1.enc")
            )
            .expect("people asset"),
            people_ciphertext,
        );
        let package_name = sha256_hex(b"people-metadata-key:fixture-1");
        assert_eq!(
            fs::read(
                target
                    .path()
                    .join(format!("meeting-assets/recovery/{package_name}.pkg"))
            )
            .expect("people recovery package"),
            people_recovery,
        );
    }

    #[test]
    fn private_asset_and_recovery_package_tampering_fail_hash_validation() {
        let private = private_bundle();
        let packed =
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private))
                .expect("pack");

        let mut altered_asset = packed.clone();
        let asset_last = altered_asset.len() - private.recovery_package.encrypted_bytes.len() - 1;
        altered_asset[asset_last] ^= 0xff;
        assert_eq!(
            unpack(&altered_asset),
            Err(PayloadError::PrivateAssetDigestMismatch)
        );

        let mut altered_recovery = packed;
        let recovery_last = altered_recovery.len() - 1;
        altered_recovery[recovery_last] ^= 0xff;
        assert_eq!(
            unpack(&altered_recovery),
            Err(PayloadError::PrivateAssetDigestMismatch)
        );

        let mut wrong_hash = private_bundle();
        wrong_hash.assets[0].ciphertext_sha256 = "0".repeat(64);
        assert_eq!(
            pack_with_private_meeting_assets(
                b"bundle",
                &AudioInventory::default(),
                Some(&wrong_hash)
            ),
            Err(PayloadError::PrivateAssetDigestMismatch)
        );
    }

    #[test]
    fn private_asset_binding_must_match_its_vault_and_account_bundle() {
        let mut private = private_bundle();
        private.assets[0].account_binding = "principal:other-account".to_string();
        assert_eq!(
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private)),
            Err(PayloadError::PrivateAssetBindingMismatch)
        );
    }

    #[test]
    fn duplicate_private_asset_and_recovery_references_are_rejected() {
        let mut private = private_bundle();
        private.recovery_package.package_ref = private.assets[0].asset_ref.clone();
        assert_eq!(
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private)),
            Err(PayloadError::DuplicatePrivateAssetRef)
        );
    }

    #[test]
    fn private_asset_size_limit_is_checked_before_payload_assembly() {
        let mut private = private_bundle();
        private.assets[0].encrypted_bytes = vec![0x5a; MAX_PRIVATE_ASSET_BYTES as usize + 1];
        private.assets[0].ciphertext_sha256 = sha256_hex(&private.assets[0].encrypted_bytes);
        assert!(matches!(
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private)),
            Err(PayloadError::TooLarge { .. })
        ));
    }

    #[test]
    fn fresh_target_reopen_keeps_private_ciphertext_in_memory_for_native_custody() {
        let private = private_bundle();
        let packed =
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private))
                .expect("pack");
        let target = TempDir::new().expect("fresh target");
        let restored = unpack(&packed).expect("fresh-target unpack");
        extract_audio(target.path(), &restored).expect("restore audio inventory");
        assert_eq!(
            fs::read_dir(target.path()).expect("target listing").count(),
            0
        );

        let reopened_bytes = pack_with_private_meeting_assets(
            &restored.genesis_bundle,
            &AudioInventory::default(),
            restored.private_meeting_assets.as_ref(),
        )
        .expect("repack reopened encrypted assets");
        let reopened = unpack(&reopened_bytes).expect("reopen payload");
        assert_eq!(reopened.private_meeting_assets, Some(private));
    }

    #[test]
    fn private_ciphertext_and_wrapped_key_restore_only_below_fresh_target() {
        let private = private_bundle();
        let packed =
            pack_with_private_meeting_assets(b"bundle", &AudioInventory::default(), Some(&private))
                .expect("pack");
        let target = TempDir::new().expect("fresh target");
        let restored = unpack(&packed).expect("unpack");
        let summary = extract_private_meeting_assets(target.path(), &restored).expect("extract");
        assert_eq!(summary.restored_asset_count, 1);
        assert!(summary.recovery_package_saved);
        assert!(!summary.owner_key_verified);
        assert_eq!(
            fs::read(
                target
                    .path()
                    .join("meeting-assets/knowledge/asset-document-version-1.enc")
            )
            .unwrap(),
            private.assets[0].encrypted_bytes,
        );
        let package_name = sha256_hex(private.recovery_package.package_ref.as_bytes());
        assert_eq!(
            fs::read(
                target
                    .path()
                    .join(format!("meeting-assets/recovery/{package_name}.pkg"))
            )
            .unwrap(),
            private.recovery_package.encrypted_bytes,
        );
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
        packed[MAGIC.len()] = 0x04;
        assert_eq!(unpack(&packed), Err(PayloadError::UnsupportedVersion));
    }
}
