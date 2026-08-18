//! Versioned in-memory encrypted archive envelope for Phase 4.
//!
//! This module deliberately stops at bytes-in/bytes-out. It does not select a
//! filesystem path, write an archive, call a provider, or invoke Genesis
//! restore. Those operations belong to later approved tasks.

#![allow(dead_code)]

use argon2::{Algorithm, Argon2, Params, Version};
use bip39::{Language, Mnemonic};
use chacha20poly1305::{
    aead::{
        stream::{DecryptorLE31, EncryptorLE31},
        Aead, Payload,
    },
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

#[cfg(test)]
use argon2::{AssociatedData, ParamsBuilder};

pub(crate) const ARCHIVE_FORMAT_VERSION: u16 = 1;
pub(crate) const ARCHIVE_CHUNK_SIZE: usize = 64 * 1024;
const MAGIC: &[u8; 8] = b"FUNGBK01";
const STREAM_NONCE_SIZE: usize = 20;
const SALT_SIZE: usize = 16;
const WRAP_NONCE_SIZE: usize = 24;
const DATA_KEY_SIZE: usize = 32;
const AUTH_TAG_SIZE: usize = 16;
const WRAPPED_DATA_KEY_SIZE: usize = DATA_KEY_SIZE + AUTH_TAG_SIZE;
const MAX_PROTECTED_HEADER_SIZE: usize = 16 * 1024;
const MAX_CHUNK_COUNT: u32 = 0x0fff_ffff;
const PAYLOAD_ALGORITHM: &str = "XChaCha20-Poly1305/STREAM-LE31";
const KDF_ALGORITHM: &str = "Argon2id-v1.3";
const KDF_MEMORY_KIB: u32 = 64 * 1024;
const KDF_ITERATIONS: u32 = 3;
const KDF_PARALLELISM: u32 = 1;
const KDF_OUTPUT_BYTES: usize = DATA_KEY_SIZE;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ArchiveError {
    #[error("invalid archive input")]
    InvalidInput,
    #[error("invalid recovery phrase")]
    InvalidRecoveryPhrase,
    #[error("invalid archive format")]
    InvalidFormat,
    #[error("archive authentication failed")]
    AuthenticationFailed,
    #[error("archive digest mismatch")]
    DigestMismatch,
    #[error("archive key derivation failed")]
    KeyDerivationFailed,
    #[error("archive serialization failed")]
    SerializationFailed,
    #[error("archive is too large")]
    TooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveManifest {
    pub(crate) archive_id: String,
    pub(crate) format_version: u16,
    pub(crate) source_manifest_digest: String,
    pub(crate) created_at: String,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
    pub(crate) chunk_size: u32,
    pub(crate) chunk_count: u32,
    pub(crate) payload_algorithm: String,
    pub(crate) kdf_algorithm: String,
    pub(crate) kdf_memory_kib: u32,
    pub(crate) kdf_iterations: u32,
    pub(crate) kdf_parallelism: u32,
    pub(crate) kdf_salt: String,
    pub(crate) protected_header_digest: String,
    pub(crate) stream_nonce_prefix: String,
    pub(crate) wrap_nonce: String,
    pub(crate) wrapped_data_key: String,
    pub(crate) terminal_state: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveEnvelope {
    pub(crate) manifest: ArchiveManifest,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtectedHeader {
    archive_id: String,
    format_version: u16,
    source_manifest_digest: String,
    created_at: String,
    chunk_size: u32,
    chunk_count: u32,
    payload_algorithm: String,
    kdf_algorithm: String,
}

struct ParsedWire {
    version: u16,
    protected_header: ProtectedHeader,
    protected_header_digest: [u8; 32],
    kdf_salt: [u8; SALT_SIZE],
    wrap_nonce: [u8; WRAP_NONCE_SIZE],
    stream_nonce_prefix: [u8; STREAM_NONCE_SIZE],
    wrapped_data_key: Vec<u8>,
    chunks: Vec<Vec<u8>>,
}

/// Generate a portable English BIP-39 24-word recovery phrase.
///
/// The returned string is secret material and is intentionally not part of any
/// serializable archive type. Callers must show it only during native setup.
pub(crate) fn generate_recovery_phrase() -> Result<String, ArchiveError> {
    let mut entropy = [0u8; 32];
    OsRng.fill_bytes(&mut entropy);
    let phrase = Mnemonic::from_entropy(&entropy)
        .map(|mnemonic| mnemonic.to_string())
        .map_err(|_| ArchiveError::InvalidRecoveryPhrase);
    entropy.zeroize();
    phrase
}

pub(crate) fn encrypt_archive(
    archive_id: &str,
    source_manifest_digest: &str,
    created_at: &str,
    plaintext: &[u8],
    recovery_phrase: &str,
) -> Result<ArchiveEnvelope, ArchiveError> {
    validate_identity(archive_id, source_manifest_digest, created_at)?;
    let phrase_password = normalized_phrase_password(recovery_phrase)?;
    let chunk_count = chunk_count_for(plaintext.len())?;

    let protected_header = ProtectedHeader {
        archive_id: archive_id.to_owned(),
        format_version: ARCHIVE_FORMAT_VERSION,
        source_manifest_digest: source_manifest_digest.to_owned(),
        created_at: created_at.to_owned(),
        chunk_size: ARCHIVE_CHUNK_SIZE as u32,
        chunk_count,
        payload_algorithm: PAYLOAD_ALGORITHM.to_owned(),
        kdf_algorithm: KDF_ALGORITHM.to_owned(),
    };
    let protected_header_bytes =
        serde_json::to_vec(&protected_header).map_err(|_| ArchiveError::SerializationFailed)?;
    if protected_header_bytes.len() > MAX_PROTECTED_HEADER_SIZE {
        return Err(ArchiveError::TooLarge);
    }
    let protected_header_digest = sha256_bytes(&protected_header_bytes);

    let mut kdf_salt = [0u8; SALT_SIZE];
    let mut wrap_nonce = [0u8; WRAP_NONCE_SIZE];
    let mut stream_nonce_prefix = [0u8; STREAM_NONCE_SIZE];
    OsRng.fill_bytes(&mut kdf_salt);
    OsRng.fill_bytes(&mut wrap_nonce);
    OsRng.fill_bytes(&mut stream_nonce_prefix);

    let mut wrap_key = derive_wrap_key(&phrase_password, &kdf_salt)?;
    let mut data_key = Zeroizing::new([0u8; DATA_KEY_SIZE]);
    OsRng.fill_bytes(&mut *data_key);

    let wrap_cipher = XChaCha20Poly1305::new_from_slice(&*wrap_key)
        .map_err(|_| ArchiveError::KeyDerivationFailed)?;
    let wrapped_data_key = wrap_cipher
        .encrypt(
            XNonce::from_slice(&wrap_nonce),
            Payload {
                msg: &*data_key,
                aad: &protected_header_digest,
            },
        )
        .map_err(|_| ArchiveError::AuthenticationFailed)?;
    wrap_key.zeroize();

    let data_cipher = XChaCha20Poly1305::new_from_slice(&*data_key)
        .map_err(|_| ArchiveError::KeyDerivationFailed)?;
    let stream_nonce = <chacha20poly1305::aead::stream::Nonce<
        XChaCha20Poly1305,
        chacha20poly1305::aead::stream::StreamLE31<XChaCha20Poly1305>,
    >>::clone_from_slice(&stream_nonce_prefix);
    let mut encryptor = EncryptorLE31::from_aead(data_cipher, &stream_nonce);
    let mut encrypted_chunks = Vec::with_capacity(chunk_count as usize);
    let mut chunks = plaintext.chunks(ARCHIVE_CHUNK_SIZE);
    if plaintext.is_empty() {
        encrypted_chunks.push(
            encryptor
                .encrypt_last(Payload {
                    msg: &[],
                    aad: &protected_header_digest,
                })
                .map_err(|_| ArchiveError::AuthenticationFailed)?,
        );
    } else {
        for chunk in chunks.by_ref().take(chunk_count as usize - 1) {
            let payload = Payload {
                msg: chunk,
                aad: &protected_header_digest,
            };
            encrypted_chunks.push(
                encryptor
                    .encrypt_next(payload)
                    .map_err(|_| ArchiveError::AuthenticationFailed)?,
            );
        }
        let final_chunk = chunks.next().ok_or(ArchiveError::InvalidFormat)?;
        encrypted_chunks.push(
            encryptor
                .encrypt_last(Payload {
                    msg: final_chunk,
                    aad: &protected_header_digest,
                })
                .map_err(|_| ArchiveError::AuthenticationFailed)?,
        );
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&ARCHIVE_FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(protected_header_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&protected_header_bytes);
    bytes.extend_from_slice(&protected_header_digest);
    bytes.extend_from_slice(&kdf_salt);
    bytes.extend_from_slice(&wrap_nonce);
    bytes.extend_from_slice(&stream_nonce_prefix);
    bytes.extend_from_slice(&(wrapped_data_key.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&wrapped_data_key);
    bytes.extend_from_slice(&chunk_count.to_le_bytes());
    for chunk in &encrypted_chunks {
        if chunk.len() < AUTH_TAG_SIZE || chunk.len() > ARCHIVE_CHUNK_SIZE + AUTH_TAG_SIZE {
            return Err(ArchiveError::InvalidFormat);
        }
        bytes.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        bytes.extend_from_slice(chunk);
    }

    let manifest = ArchiveManifest {
        archive_id: archive_id.to_owned(),
        format_version: ARCHIVE_FORMAT_VERSION,
        source_manifest_digest: source_manifest_digest.to_owned(),
        created_at: created_at.to_owned(),
        byte_count: bytes.len() as u64,
        sha256: hex_encode(&sha256_bytes(&bytes)),
        chunk_size: ARCHIVE_CHUNK_SIZE as u32,
        chunk_count,
        payload_algorithm: PAYLOAD_ALGORITHM.to_owned(),
        kdf_algorithm: KDF_ALGORITHM.to_owned(),
        kdf_memory_kib: KDF_MEMORY_KIB,
        kdf_iterations: KDF_ITERATIONS,
        kdf_parallelism: KDF_PARALLELISM,
        kdf_salt: hex_encode(&kdf_salt),
        protected_header_digest: hex_encode(&protected_header_digest),
        stream_nonce_prefix: hex_encode(&stream_nonce_prefix),
        wrap_nonce: hex_encode(&wrap_nonce),
        wrapped_data_key: hex_encode(&wrapped_data_key),
        terminal_state: "encrypted_pending_transport".to_owned(),
    };

    Ok(ArchiveEnvelope { manifest, bytes })
}

pub(crate) fn decrypt_archive(
    envelope: &ArchiveEnvelope,
    recovery_phrase: &str,
) -> Result<Vec<u8>, ArchiveError> {
    if envelope.manifest.byte_count != envelope.bytes.len() as u64
        || envelope.manifest.sha256 != hex_encode(&sha256_bytes(&envelope.bytes))
    {
        return Err(ArchiveError::DigestMismatch);
    }

    let parsed = parse_wire(&envelope.bytes)?;
    validate_manifest(&envelope.manifest, &parsed, &envelope.bytes)?;
    let phrase_password = normalized_phrase_password(recovery_phrase)?;
    let mut wrap_key = derive_wrap_key(&phrase_password, &parsed.kdf_salt)?;
    let wrap_cipher = XChaCha20Poly1305::new_from_slice(&*wrap_key)
        .map_err(|_| ArchiveError::KeyDerivationFailed)?;
    let mut wrapped_data_key = parsed.wrapped_data_key.clone();
    let data_key_bytes = wrap_cipher
        .decrypt(
            XNonce::from_slice(&parsed.wrap_nonce),
            Payload {
                msg: &wrapped_data_key,
                aad: &parsed.protected_header_digest,
            },
        )
        .map_err(|_| ArchiveError::AuthenticationFailed)?;
    wrap_key.zeroize();
    wrapped_data_key.zeroize();
    if data_key_bytes.len() != DATA_KEY_SIZE {
        return Err(ArchiveError::AuthenticationFailed);
    }
    let mut data_key = Zeroizing::new([0u8; DATA_KEY_SIZE]);
    data_key.copy_from_slice(&data_key_bytes);
    let mut data_key_bytes = data_key_bytes;
    data_key_bytes.zeroize();

    let data_cipher = XChaCha20Poly1305::new_from_slice(&*data_key)
        .map_err(|_| ArchiveError::AuthenticationFailed)?;
    let stream_nonce = <chacha20poly1305::aead::stream::Nonce<
        XChaCha20Poly1305,
        chacha20poly1305::aead::stream::StreamLE31<XChaCha20Poly1305>,
    >>::clone_from_slice(&parsed.stream_nonce_prefix);
    let mut decryptor = DecryptorLE31::from_aead(data_cipher, &stream_nonce);
    let mut plaintext = Vec::new();
    let mut chunks = parsed.chunks.iter();
    for chunk in chunks.by_ref().take(parsed.chunks.len() - 1) {
        let payload = Payload {
            msg: chunk,
            aad: &parsed.protected_header_digest,
        };
        plaintext.extend_from_slice(
            &decryptor
                .decrypt_next(payload)
                .map_err(|_| ArchiveError::AuthenticationFailed)?,
        );
    }
    let final_chunk = chunks.next().ok_or(ArchiveError::InvalidFormat)?;
    plaintext.extend_from_slice(
        &decryptor
            .decrypt_last(Payload {
                msg: final_chunk,
                aad: &parsed.protected_header_digest,
            })
            .map_err(|_| ArchiveError::AuthenticationFailed)?,
    );
    let expected_chunks = chunk_count_for(plaintext.len())?;
    if expected_chunks != parsed.protected_header.chunk_count {
        return Err(ArchiveError::AuthenticationFailed);
    }
    Ok(plaintext)
}

fn validate_identity(
    archive_id: &str,
    source_manifest_digest: &str,
    created_at: &str,
) -> Result<(), ArchiveError> {
    if archive_id.trim().is_empty()
        || source_manifest_digest.trim().is_empty()
        || created_at.trim().is_empty()
    {
        return Err(ArchiveError::InvalidInput);
    }
    if archive_id.len() > 256 || source_manifest_digest.len() > 512 || created_at.len() > 128 {
        return Err(ArchiveError::TooLarge);
    }
    Ok(())
}

fn chunk_count_for(plaintext_len: usize) -> Result<u32, ArchiveError> {
    let count = if plaintext_len == 0 {
        1
    } else {
        (plaintext_len / ARCHIVE_CHUNK_SIZE)
            .checked_add(usize::from(plaintext_len % ARCHIVE_CHUNK_SIZE != 0))
            .ok_or(ArchiveError::TooLarge)?
    };
    if count > MAX_CHUNK_COUNT as usize || count > u32::MAX as usize {
        return Err(ArchiveError::TooLarge);
    }
    Ok(count as u32)
}

fn normalized_phrase_password(phrase: &str) -> Result<Zeroizing<Vec<u8>>, ArchiveError> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase)
        .map_err(|_| ArchiveError::InvalidRecoveryPhrase)?;
    if mnemonic.word_count() != 24 {
        return Err(ArchiveError::InvalidRecoveryPhrase);
    }
    Ok(Zeroizing::new(mnemonic.to_string().into_bytes()))
}

fn derive_wrap_key(
    password: &[u8],
    salt: &[u8; SALT_SIZE],
) -> Result<Zeroizing<[u8; DATA_KEY_SIZE]>, ArchiveError> {
    let params = Params::new(
        KDF_MEMORY_KIB,
        KDF_ITERATIONS,
        KDF_PARALLELISM,
        Some(KDF_OUTPUT_BYTES),
    )
    .map_err(|_| ArchiveError::KeyDerivationFailed)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = Zeroizing::new([0u8; DATA_KEY_SIZE]);
    argon
        .hash_password_into(password, salt, &mut *output)
        .map_err(|_| ArchiveError::KeyDerivationFailed)?;
    Ok(output)
}

fn parse_wire(bytes: &[u8]) -> Result<ParsedWire, ArchiveError> {
    let mut offset = 0usize;
    if take(bytes, &mut offset, MAGIC.len())? != MAGIC {
        return Err(ArchiveError::InvalidFormat);
    }
    let version = read_u16(bytes, &mut offset)?;
    if version != ARCHIVE_FORMAT_VERSION {
        return Err(ArchiveError::InvalidFormat);
    }
    let protected_header_len = read_u32(bytes, &mut offset)? as usize;
    if protected_header_len == 0 || protected_header_len > MAX_PROTECTED_HEADER_SIZE {
        return Err(ArchiveError::InvalidFormat);
    }
    let protected_header_bytes = take(bytes, &mut offset, protected_header_len)?;
    let protected_header: ProtectedHeader =
        serde_json::from_slice(protected_header_bytes).map_err(|_| ArchiveError::InvalidFormat)?;
    let protected_header_digest = read_array::<32>(bytes, &mut offset)?;
    if sha256_bytes(protected_header_bytes) != protected_header_digest {
        return Err(ArchiveError::AuthenticationFailed);
    }
    if protected_header.format_version != version
        || protected_header.chunk_size != ARCHIVE_CHUNK_SIZE as u32
        || protected_header.payload_algorithm != PAYLOAD_ALGORITHM
        || protected_header.kdf_algorithm != KDF_ALGORITHM
        || protected_header.chunk_count == 0
        || protected_header.chunk_count > MAX_CHUNK_COUNT
    {
        return Err(ArchiveError::InvalidFormat);
    }

    let kdf_salt = read_array::<SALT_SIZE>(bytes, &mut offset)?;
    let wrap_nonce = read_array::<WRAP_NONCE_SIZE>(bytes, &mut offset)?;
    let stream_nonce_prefix = read_array::<STREAM_NONCE_SIZE>(bytes, &mut offset)?;
    let wrapped_len = read_u32(bytes, &mut offset)? as usize;
    if wrapped_len != WRAPPED_DATA_KEY_SIZE {
        return Err(ArchiveError::InvalidFormat);
    }
    let wrapped_data_key = take(bytes, &mut offset, wrapped_len)?.to_vec();
    let chunk_count = read_u32(bytes, &mut offset)?;
    if chunk_count != protected_header.chunk_count || chunk_count == 0 {
        return Err(ArchiveError::InvalidFormat);
    }
    let minimum_frame_size = 4usize + AUTH_TAG_SIZE;
    let remaining = bytes.len().saturating_sub(offset);
    if chunk_count as usize > remaining / minimum_frame_size {
        return Err(ArchiveError::InvalidFormat);
    }
    let mut chunks = Vec::with_capacity(chunk_count as usize);
    for _ in 0..chunk_count {
        let chunk_len = read_u32(bytes, &mut offset)? as usize;
        if !(AUTH_TAG_SIZE..=ARCHIVE_CHUNK_SIZE + AUTH_TAG_SIZE).contains(&chunk_len) {
            return Err(ArchiveError::InvalidFormat);
        }
        chunks.push(take(bytes, &mut offset, chunk_len)?.to_vec());
    }
    if offset != bytes.len() {
        return Err(ArchiveError::InvalidFormat);
    }

    Ok(ParsedWire {
        version,
        protected_header,
        protected_header_digest,
        kdf_salt,
        wrap_nonce,
        stream_nonce_prefix,
        wrapped_data_key,
        chunks,
    })
}

fn validate_manifest(
    manifest: &ArchiveManifest,
    parsed: &ParsedWire,
    bytes: &[u8],
) -> Result<(), ArchiveError> {
    if manifest.format_version != parsed.version
        || manifest.archive_id != parsed.protected_header.archive_id
        || manifest.source_manifest_digest != parsed.protected_header.source_manifest_digest
        || manifest.created_at != parsed.protected_header.created_at
        || manifest.chunk_size != parsed.protected_header.chunk_size
        || manifest.chunk_count != parsed.protected_header.chunk_count
        || manifest.payload_algorithm != parsed.protected_header.payload_algorithm
        || manifest.kdf_algorithm != parsed.protected_header.kdf_algorithm
        || manifest.kdf_memory_kib != KDF_MEMORY_KIB
        || manifest.kdf_iterations != KDF_ITERATIONS
        || manifest.kdf_parallelism != KDF_PARALLELISM
        || manifest.byte_count != bytes.len() as u64
        || manifest.terminal_state != "encrypted_pending_transport"
        || manifest.kdf_salt != hex_encode(&parsed.kdf_salt)
        || manifest.protected_header_digest != hex_encode(&parsed.protected_header_digest)
        || manifest.stream_nonce_prefix != hex_encode(&parsed.stream_nonce_prefix)
        || manifest.wrap_nonce != hex_encode(&parsed.wrap_nonce)
        || manifest.wrapped_data_key != hex_encode(&parsed.wrapped_data_key)
    {
        return Err(ArchiveError::InvalidFormat);
    }
    Ok(())
}

fn take<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8], ArchiveError> {
    let end = offset.checked_add(len).ok_or(ArchiveError::InvalidFormat)?;
    if end > bytes.len() {
        return Err(ArchiveError::InvalidFormat);
    }
    let result = &bytes[*offset..end];
    *offset = end;
    Ok(result)
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> Result<u16, ArchiveError> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, ArchiveError> {
    Ok(u32::from_le_bytes(read_array::<4>(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], ArchiveError> {
    let slice = take(bytes, offset, N)?;
    let mut result = [0u8; N];
    result.copy_from_slice(slice);
    Ok(result)
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_phrase() -> String {
        Mnemonic::from_entropy(&[0u8; 32]).unwrap().to_string()
    }

    #[test]
    fn bip39_24_word_entropy_round_trip_matches_known_vector_shape() {
        let entropy = [0u8; 32];
        let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();
        assert_eq!(mnemonic.word_count(), 24);
        assert_eq!(mnemonic.words().next(), Some("abandon"));
        assert_eq!(mnemonic.to_entropy(), entropy);
        let phrase = mnemonic.to_string();
        let parsed = Mnemonic::parse_in_normalized(Language::English, &phrase).unwrap();
        assert_eq!(parsed.to_entropy(), entropy);
    }

    #[test]
    fn argon2id_reference_vector_is_stable() {
        let params = ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .data(AssociatedData::new(&[0x04; 12]).unwrap())
            .build()
            .unwrap();
        let argon =
            Argon2::new_with_secret(&[0x03; 8], Algorithm::Argon2id, Version::V0x13, params)
                .unwrap();
        let mut output = [0u8; 32];
        argon
            .hash_password_into(&[0x01; 32], &[0x02; 16], &mut output)
            .unwrap();
        assert_eq!(
            hex_encode(&output),
            "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
        );
    }

    #[test]
    fn xchacha20poly1305_reference_vector_is_stable() {
        let key: [u8; 32] = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d,
            0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
            0x9c, 0x9d, 0x9e, 0x9f,
        ];
        let nonce: [u8; 24] = [
            0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d,
            0x4e, 0x4f, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
        ];
        let aad = [
            0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
        ];
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let expected = [
            0xbd, 0x6d, 0x17, 0x9d, 0x3e, 0x83, 0xd4, 0x3b, 0x95, 0x76, 0x57, 0x94, 0x93, 0xc0,
            0xe9, 0x39, 0x57, 0x2a, 0x17, 0x00, 0x25, 0x2b, 0xfa, 0xcc, 0xbe, 0xd2, 0x90, 0x2c,
            0x21, 0x39, 0x6c, 0xbb, 0x73, 0x1c, 0x7f, 0x1b, 0x0b, 0x4a, 0xa6, 0x44, 0x0b, 0xf3,
            0xa8, 0x2f, 0x4e, 0xda, 0x7e, 0x39, 0xae, 0x64, 0xc6, 0x70, 0x8c, 0x54, 0xc2, 0x16,
            0xcb, 0x96, 0xb7, 0x2e, 0x12, 0x13, 0xb4, 0x52, 0x2f, 0x8c, 0x9b, 0xa4, 0x0d, 0xb5,
            0xd9, 0x45, 0xb1, 0x1b, 0x69, 0xb9, 0x82, 0xc1, 0xbb, 0x9e, 0x3f, 0x3f, 0xac, 0x2b,
            0xc3, 0x69, 0x48, 0x8f, 0x76, 0xb2, 0x38, 0x35, 0x65, 0xd3, 0xff, 0xf9, 0x21, 0xf9,
            0x66, 0x4c, 0x97, 0x63, 0x7d, 0xa9, 0x76, 0x88, 0x12, 0xf6, 0x15, 0xc6, 0x8b, 0x13,
            0xb5, 0x2e, 0xc0, 0x87, 0x59, 0x24, 0xc1, 0xc7, 0x98, 0x79, 0x47, 0xde, 0xaf, 0xd8,
            0x78, 0x0a, 0xcf, 0x49,
        ];
        let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(ciphertext, expected);
    }

    #[test]
    fn archive_contract_rejects_wrong_secret_tamper_and_truncation() {
        let phrase = test_phrase();
        let plaintext = vec![0x5au8; ARCHIVE_CHUNK_SIZE + 17];
        let mut envelope = encrypt_archive(
            "archive-test",
            "source-digest",
            "2026-08-14T03:00:00+07:00",
            &plaintext,
            &phrase,
        )
        .unwrap();
        assert_eq!(decrypt_archive(&envelope, &phrase).unwrap(), plaintext);

        let wrong_phrase = Mnemonic::from_entropy(&[1u8; 32]).unwrap().to_string();
        assert_eq!(
            decrypt_archive(&envelope, &wrong_phrase),
            Err(ArchiveError::AuthenticationFailed)
        );

        let last_byte = envelope.bytes.len() - 1;
        envelope.bytes[last_byte] ^= 1;
        envelope.manifest.sha256 = hex_encode(&sha256_bytes(&envelope.bytes));
        assert_eq!(
            decrypt_archive(&envelope, &phrase),
            Err(ArchiveError::AuthenticationFailed)
        );

        envelope.bytes.pop();
        envelope.manifest.byte_count = envelope.bytes.len() as u64;
        envelope.manifest.sha256 = hex_encode(&sha256_bytes(&envelope.bytes));
        assert!(matches!(
            decrypt_archive(&envelope, &phrase),
            Err(ArchiveError::InvalidFormat | ArchiveError::AuthenticationFailed)
        ));
    }

    #[test]
    fn archive_contract_round_trips_empty_payload_and_never_serializes_secret_fields() {
        let phrase = test_phrase();
        let first = encrypt_archive("empty-a", "source-a", "now", &[], &phrase).unwrap();
        let second = encrypt_archive("empty-b", "source-b", "now", &[], &phrase).unwrap();
        assert_eq!(decrypt_archive(&first, &phrase).unwrap(), Vec::<u8>::new());
        assert_ne!(
            first.manifest.stream_nonce_prefix,
            second.manifest.stream_nonce_prefix
        );
        assert_ne!(first.manifest.wrap_nonce, second.manifest.wrap_nonce);

        let serialized = serde_json::to_string(&first.manifest).unwrap();
        assert!(!serialized.contains(&phrase));
        assert!(!serialized.contains("recoverySecret"));
        assert!(!serialized.contains("dataKey"));
        assert!(!serialized.contains("providerToken"));
    }
}
