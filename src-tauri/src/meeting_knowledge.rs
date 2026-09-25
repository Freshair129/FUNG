//! Local-only knowledge ingestion, encrypted custody primitives and retrieval.
//!
//! This module deliberately owns no database or UI authority. Native callers
//! must supply the selected custody root, the trusted read boundary and the
//! keyring adapter. The serial integrator connects the typed traits below to
//! the Genesis schema, native identity lifecycle and encrypted backup path.

#![allow(dead_code)]

use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use chrono::{DateTime, Datelike, NaiveDate, SecondsFormat, Utc};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub(crate) const MAX_IMPORT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_ENCRYPTED_ASSET_BYTES: usize = 25 * 1024 * 1024 + 16;
const MAX_ENCRYPTED_CHUNK_BYTES: usize = 16 * 1024;
const MAX_PDF_PAGES: u32 = 500;
const MAX_PARSER_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PARSER_STDERR_BYTES: usize = 4 * 1024;
const MAX_QUERY_CHARS: usize = 2_000;
const MAX_CONTEXT_CHARS: usize = 12_000;
const MAX_SEARCH_RESULTS: usize = 8;
const MAX_SELECTED_COLLECTIONS: usize = 64;
const MAX_SEARCH_CANDIDATES: usize = 2_000;
const DEFAULT_CHUNK_CHARS: usize = 3_000;
const PARSER_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(windows)]
#[path = "meeting_knowledge_windows.rs"]
mod windows_pdf_parser;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KnowledgeDocumentFormat {
    Text,
    Markdown,
    Pdf,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedPage {
    pub page_index: Option<u32>,
    pub start_char: u64,
    pub end_char: u64,
    pub text: String,
}

impl Drop for ParsedPage {
    fn drop(&mut self) {
        self.text.zeroize();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedDocument {
    pub parser_version: String,
    pub parser_fingerprint: String,
    pub dependency: ParserDependency,
    pub content_sha256: String,
    pub mime_type: String,
    pub source_bytes: u64,
    pub pages: Vec<ParsedPage>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParserDependency {
    pub name: String,
    pub version: String,
    pub license: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ParserInvocation {
    /// Native-bundled, allowlisted Python executable and parser script.
    pub python: PathBuf,
    pub script: PathBuf,
    /// Separate minimal Python/pypdf runtime. PDF work is rejected when it is
    /// absent or the platform sandbox cannot be established.
    pub pdf_parser_runtime: Option<PathBuf>,
    /// Directory selected by the native file-picker/custody service.
    pub selected_root: PathBuf,
    pub selected_file: PathBuf,
}

/// Runs the pinned extractor against one native-selected file. PDF input is
/// available only through the qualified platform process sandbox.
pub(crate) fn parse_selected_file(invocation: &ParserInvocation) -> Result<ParsedDocument, String> {
    let root = invocation
        .selected_root
        .canonicalize()
        .map_err(|_| "selected custody root is unavailable".to_string())?;
    let file = invocation
        .selected_file
        .canonicalize()
        .map_err(|_| "selected document is unavailable".to_string())?;
    if !file.starts_with(&root) || !file.is_file() {
        return Err("selected document is outside its granted custody root".to_string());
    }
    let metadata = file
        .metadata()
        .map_err(|_| "selected document metadata is unavailable".to_string())?;
    if metadata.len() == 0 || metadata.len() > MAX_IMPORT_BYTES {
        return Err("selected document exceeds the 25 MiB import bound".to_string());
    }
    if !root.is_dir() {
        return Err("selected custody root is unavailable".to_string());
    }
    let (input_sha256, input_bytes) = hash_file(&file)?;
    let expected_mime = match file
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" => "text/plain",
        "md" => "text/markdown",
        "pdf" => "application/pdf",
        _ => return Err("selected document format is unsupported".to_string()),
    };

    let (status_success, output, output_overflow) = if expected_mime == "application/pdf" {
        let runtime = invocation
            .pdf_parser_runtime
            .as_ref()
            .ok_or_else(|| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        let mut input_file =
            File::open(&file).map_err(|_| "selected document is unavailable".to_string())?;
        let mut input_bytes = Vec::with_capacity(input_bytes.min(MAX_IMPORT_BYTES) as usize);
        input_file
            .by_ref()
            .take(MAX_IMPORT_BYTES + 1)
            .read_to_end(&mut input_bytes)
            .map_err(|_| "selected document could not be read".to_string())?;
        let input = Zeroizing::new(input_bytes);
        if input.is_empty()
            || input.len() as u64 > MAX_IMPORT_BYTES
            || hex_lower(&Sha256::digest(input.as_slice())) != input_sha256
        {
            return Err("selected document changed during local parsing".to_string());
        }
        #[cfg(windows)]
        {
            windows_pdf_parser::run(runtime, &input)?
        }
        #[cfg(not(windows))]
        {
            let _ = (runtime, input);
            return Err("PDF_PARSER_SANDBOX_UNSUPPORTED".to_string());
        }
    } else {
        let mut child = Command::new(&invocation.python)
            .arg("-I")
            .arg(&invocation.script)
            .arg("--root")
            .arg(&root)
            .arg("--input")
            .arg(&file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| "local document parser could not start".to_string())?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "local document parser stdout is unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "local document parser stderr is unavailable".to_string())?;
        let stdout_thread =
            thread::spawn(move || read_bounded_and_drain(stdout, MAX_PARSER_OUTPUT_BYTES));
        let stderr_thread =
            thread::spawn(move || read_bounded_and_drain(stderr, MAX_PARSER_STDERR_BYTES));

        let deadline = Instant::now() + PARSER_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err("local document parser exceeded the 60 second bound".to_string());
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err("local document parser status is unavailable".to_string());
                }
            }
        };

        let (output, output_overflow) = stdout_thread
            .join()
            .map_err(|_| "local document parser output thread failed".to_string())??;
        let _ = stderr_thread.join(); // stderr is drained but never surfaced to callers.
        (status.success(), output, output_overflow)
    };
    if output_overflow {
        return Err("local document parser output exceeded its bound".to_string());
    }
    let output = Zeroizing::new(output);
    let parsed: ParserResponse = serde_json::from_slice(output.as_slice())
        .map_err(|_| "local document parser returned an invalid response".to_string())?;
    if !status_success || !parsed.ok {
        let error_code = parsed
            .error_code
            .filter(|code| {
                !code.is_empty()
                    && code.len() <= 64
                    && code
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
            .unwrap_or_else(|| "parse_failed".to_string());
        return Err(format!("local document parser failed: {}", error_code));
    }
    let document = parsed
        .document
        .ok_or_else(|| "local document parser omitted its result".to_string())?;
    let (final_sha256, final_bytes) = hash_file(&file)?;
    if input_sha256 != final_sha256 || input_bytes != final_bytes {
        return Err("selected document changed during local parsing".to_string());
    }
    validate_parsed_document(&document, input_bytes)?;
    if document.content_sha256 != input_sha256 || document.mime_type != expected_mime {
        return Err("local document parser returned inconsistent source identity".to_string());
    }
    Ok(document)
}

fn hash_file(path: &std::path::Path) -> Result<(String, u64), String> {
    let mut file = File::open(path).map_err(|_| "selected document is unavailable".to_string())?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "selected document could not be read".to_string())?;
        if count == 0 {
            break;
        }
        bytes = bytes.saturating_add(count as u64);
        if bytes > MAX_IMPORT_BYTES {
            return Err("selected document exceeds the 25 MiB import bound".to_string());
        }
        hasher.update(&buffer[..count]);
    }
    let digest = hasher.finalize();
    let hex = hex_lower(&digest);
    Ok((hex, bytes))
}

fn read_bounded_and_drain<R: Read>(
    mut reader: R,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), String> {
    let mut retained = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut overflow = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| "local document parser output could not be read".to_string())?;
        if count == 0 {
            break;
        }
        let available = max_bytes.saturating_sub(retained.len());
        let keep = count.min(available);
        retained.extend_from_slice(&buffer[..keep]);
        overflow |= keep < count;
    }
    Ok((retained, overflow))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParserResponse {
    ok: bool,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    document: Option<ParsedDocument>,
}

fn validate_parsed_document(document: &ParsedDocument, source_bytes: u64) -> Result<(), String> {
    if document.source_bytes != source_bytes
        || !valid_sha256(&document.content_sha256)
        || !valid_sha256(&document.parser_fingerprint)
        || document.pages.len() > MAX_PDF_PAGES as usize
        || document.parser_version != "fung-knowledge-extractor/0.1.0"
        || (document.mime_type == "application/pdf"
            && (document.dependency.name != "pypdf"
                || document.dependency.version != "6.10.0"
                || document.dependency.license != "BSD-3-Clause"))
        || (document.mime_type != "application/pdf"
            && (document.dependency.name != "python-stdlib"
                || document.dependency.version != "stdlib"
                || document.dependency.license != "Python-PSF-2.0"))
    {
        return Err("local document parser returned inconsistent provenance".to_string());
    }
    let text_chars: usize = document
        .pages
        .iter()
        .map(|page| page.text.chars().count())
        .sum();
    if text_chars > 4 * 1024 * 1024 {
        return Err("local document parser text exceeded its bound".to_string());
    }
    for page in &document.pages {
        if page.start_char > page.end_char
            || page.end_char - page.start_char != page.text.chars().count() as u64
        {
            return Err("local document parser returned invalid text locators".to_string());
        }
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkLocator {
    pub page_index: Option<u32>,
    pub start_char: u64,
    pub end_char: u64,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtractedChunk {
    pub locator: ChunkLocator,
    pub text: String,
}

impl Drop for ExtractedChunk {
    fn drop(&mut self) {
        self.text.zeroize();
    }
}

/// Split extracted text without losing the source's page, character or line
/// locator. Chunks are ephemeral until encrypted by the native custody layer.
pub(crate) fn chunk_document(document: &ParsedDocument) -> Vec<ExtractedChunk> {
    let mut chunks = Vec::new();
    for page in &document.pages {
        let chars: Vec<char> = page.text.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            let hard_end = (start + DEFAULT_CHUNK_CHARS).min(chars.len());
            let end = if hard_end < chars.len() {
                chars[start..hard_end]
                    .iter()
                    .rposition(|character| *character == '\n')
                    .filter(|offset| *offset > DEFAULT_CHUNK_CHARS / 2)
                    .map(|offset| start + offset + 1)
                    .unwrap_or(hard_end)
            } else {
                hard_end
            };
            let text: String = chars[start..end].iter().copied().collect();
            let line_base = page.text[..char_to_byte(&page.text, start)]
                .chars()
                .filter(|character| *character == '\n')
                .count() as u32;
            let line_count = text.chars().filter(|character| *character == '\n').count() as u32;
            chunks.push(ExtractedChunk {
                locator: ChunkLocator {
                    page_index: page.page_index,
                    start_char: page.start_char + start as u64,
                    end_char: page.start_char + end as u64,
                    start_line: if page.page_index.is_none() {
                        Some(line_base + 1)
                    } else {
                        None
                    },
                    end_line: if page.page_index.is_none() {
                        Some(line_base + line_count + 1)
                    } else {
                        None
                    },
                },
                text,
            });
            start = end;
        }
    }
    chunks
}

fn char_to_byte(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PrivateAssetPurpose {
    KnowledgeDocument,
    KnowledgeEvidence,
    AgentDraft,
    DeliveryPreview,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrivateAssetContext {
    pub vault_id: String,
    pub entity_id: String,
    pub version: u64,
    pub purpose: PrivateAssetPurpose,
}

impl PrivateAssetContext {
    fn validate(&self) -> Result<(), String> {
        if !valid_opaque_id(&self.vault_id)
            || !valid_opaque_id(&self.entity_id)
            || self.version == 0
        {
            return Err("private asset encryption context is invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub(crate) struct KnowledgeKeyRef(String);

impl KnowledgeKeyRef {
    pub(crate) fn new(value: String) -> Result<Self, String> {
        let valid_suffix = value
            .strip_prefix("knowledge:")
            .and_then(|suffix| Uuid::parse_str(suffix).ok())
            .is_some();
        if !valid_suffix {
            return Err("knowledge key reference is invalid".to_string());
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for KnowledgeKeyRef {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EncryptedKnowledgePayload {
    pub version: u8,
    pub key_ref: KnowledgeKeyRef,
    pub nonce: Vec<u8>,
    pub aad_sha256: String,
    pub ciphertext_sha256: String,
    pub ciphertext: Vec<u8>,
}

/// Seals document chunks/excerpts before they enter Genesis rows, SQLite WAL
/// or ordinary backup staging. The key itself must come from native secure
/// storage and is never serialized or sent to a renderer.
pub(crate) fn seal_private_payload(
    key: &[u8; 32],
    key_ref: KnowledgeKeyRef,
    context: &PrivateAssetContext,
    plaintext: &[u8],
) -> Result<EncryptedKnowledgePayload, String> {
    context.validate()?;
    if plaintext.len() > MAX_IMPORT_BYTES as usize {
        return Err("private asset exceeds the 25 MiB custody bound".to_string());
    }
    let aad = serde_json::to_vec(context)
        .map_err(|_| "private asset context could not be encoded".to_string())?;
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "private asset key is invalid".to_string())?;
    let mut nonce_bytes = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| "private asset encryption failed".to_string())?;
    Ok(EncryptedKnowledgePayload {
        version: 1,
        key_ref,
        nonce: nonce_bytes.to_vec(),
        aad_sha256: sha256_hex(&aad),
        ciphertext_sha256: sha256_hex(&ciphertext),
        ciphertext,
    })
}

pub(crate) fn open_private_payload(
    key: &[u8; 32],
    context: &PrivateAssetContext,
    payload: &EncryptedKnowledgePayload,
) -> Result<Zeroizing<Vec<u8>>, String> {
    context.validate()?;
    if payload.version != 1
        || payload.nonce.len() != 24
        || payload.ciphertext.len() > MAX_ENCRYPTED_ASSET_BYTES
        || sha256_hex(&payload.ciphertext) != payload.ciphertext_sha256
    {
        return Err("private asset envelope is malformed".to_string());
    }
    let aad = serde_json::to_vec(context)
        .map_err(|_| "private asset context could not be encoded".to_string())?;
    if sha256_hex(&aad) != payload.aad_sha256 {
        return Err("private asset encryption context mismatch".to_string());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "private asset key is invalid".to_string())?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&payload.nonce),
            Payload {
                msg: &payload.ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| "private asset authentication failed".to_string())?;
    Ok(Zeroizing::new(plaintext))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn valid_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.chars().any(char::is_control)
        && !value.contains("..")
        && !value.contains('/')
        && !value.contains('\\')
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CitationLocator {
    TextSpan {
        start_line: u32,
        end_line: u32,
        start_char: u64,
        end_char: u64,
    },
    PdfPage {
        page_number: u32,
        start_char: u64,
        end_char: u64,
    },
    SpreadsheetCellRange {
        sheet_ref: String,
        range: String,
    },
    TranscriptRange {
        recording_id: String,
        revision_id: String,
        start_ms: u64,
        end_ms: u64,
    },
}

impl ChunkLocator {
    pub(crate) fn as_citation_locator(&self) -> Result<CitationLocator, String> {
        if self.end_char <= self.start_char {
            return Err("knowledge chunk locator is empty".to_string());
        }
        if let Some(page_index) = self.page_index {
            return Ok(CitationLocator::PdfPage {
                page_number: page_index
                    .checked_add(1)
                    .ok_or_else(|| "PDF page number is out of range".to_string())?,
                start_char: self.start_char,
                end_char: self.end_char,
            });
        }
        let start_line = self
            .start_line
            .filter(|line| *line > 0)
            .ok_or_else(|| "text citation has no starting line".to_string())?;
        let end_line = self
            .end_line
            .filter(|line| *line >= start_line)
            .ok_or_else(|| "text citation has an invalid ending line".to_string())?;
        Ok(CitationLocator::TextSpan {
            start_line,
            end_line,
            start_char: self.start_char,
            end_char: self.end_char,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvidenceCitation {
    pub collection_id: String,
    pub document_id: String,
    pub document_version_id: String,
    pub document_version_number: u64,
    pub source_version: String,
    pub content_sha256: String,
    pub locator: CitationLocator,
    pub retrieved_at: String,
    pub read_grant_id: String,
    pub acl_revision: u64,
}

impl EvidenceCitation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !valid_opaque_id(&self.collection_id)
            || !valid_opaque_id(&self.document_id)
            || !valid_opaque_id(&self.document_version_id)
            || !valid_opaque_id(&self.source_version)
            || !valid_sha256(&self.content_sha256)
            || DateTime::parse_from_rfc3339(&self.retrieved_at).is_err()
            || !valid_opaque_id(&self.read_grant_id)
            || self.acl_revision == 0
        {
            return Err("knowledge citation identity is invalid".to_string());
        }
        let valid_locator = match &self.locator {
            CitationLocator::TextSpan {
                start_line,
                end_line,
                start_char,
                end_char,
            } => *start_line > 0 && end_line >= start_line && end_char > start_char,
            CitationLocator::PdfPage {
                page_number,
                start_char,
                end_char,
            } => *page_number > 0 && end_char > start_char,
            CitationLocator::SpreadsheetCellRange { sheet_ref, range } => {
                valid_opaque_id(sheet_ref) && !range.is_empty() && range.len() <= 128
            }
            CitationLocator::TranscriptRange {
                recording_id,
                revision_id,
                start_ms,
                end_ms,
            } => valid_opaque_id(recording_id) && valid_opaque_id(revision_id) && end_ms > start_ms,
        };
        if !valid_locator {
            return Err("knowledge citation locator is invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredKnowledgeChunk {
    pub id: String,
    pub collection_id: String,
    pub document_id: String,
    pub document_version_id: String,
    pub document_version_number: u64,
    pub source_version: String,
    pub content_sha256: String,
    pub acl_revision: u64,
    pub key_ref: KnowledgeKeyRef,
    pub private_context: PrivateAssetContext,
    pub locator: CitationLocator,
    pub payload: EncryptedKnowledgePayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KnowledgeReadGrant {
    pub grant_id: String,
    pub owner_principal_ref: String,
    pub vault_id: String,
    pub account_generation: u64,
    pub identity_generation: u64,
    pub private_asset_id: String,
    pub collection_id: String,
    pub document_id: String,
    pub document_version_id: String,
    pub acl_revision: u64,
}

/// Native integration implements this boundary using the current owner/vault,
/// selected session collections and current source ACL/version. Capture the
/// accepted same-broker AccountCommitFence and identity generation in each
/// grant, authorize before decryption, then revalidate them before returning.
pub(crate) trait KnowledgeReadBoundary {
    fn authorize(
        &self,
        query: &KnowledgeQuery,
        selected_collections: &BTreeSet<String>,
        chunk: &StoredKnowledgeChunk,
    ) -> Result<Option<KnowledgeReadGrant>, String>;

    fn open(
        &self,
        grant: &KnowledgeReadGrant,
        context: &PrivateAssetContext,
        key_ref: &KnowledgeKeyRef,
        payload: &EncryptedKnowledgePayload,
    ) -> Result<Zeroizing<Vec<u8>>, String>;

    fn revalidate(&self, grant: &KnowledgeReadGrant) -> Result<bool, String>;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KnowledgeScopeKind {
    MeetingReferences,
    SelectedCollections,
}

#[derive(Clone, Debug)]
pub(crate) struct KnowledgeQuery {
    pub meeting_session_id: String,
    pub query: String,
    pub scope: KnowledgeScopeKind,
    pub selected_collection_ids: Vec<String>,
    pub max_results: usize,
    pub max_context_chars: usize,
    pub deadline: Instant,
}

impl KnowledgeQuery {
    pub(crate) fn local(
        meeting_session_id: String,
        query: String,
        selected_collection_ids: Vec<String>,
    ) -> Self {
        Self {
            meeting_session_id,
            query,
            scope: KnowledgeScopeKind::SelectedCollections,
            selected_collection_ids,
            max_results: MAX_SEARCH_RESULTS,
            max_context_chars: MAX_CONTEXT_CHARS,
            deadline: Instant::now() + Duration::from_secs(10),
        }
    }

    fn validate(&self) -> Result<BTreeSet<String>, String> {
        if self.query.trim().is_empty()
            || !valid_opaque_id(&self.meeting_session_id)
            || self.query.chars().count() > MAX_QUERY_CHARS
            || self.max_results == 0
            || self.max_results > MAX_SEARCH_RESULTS
            || self.max_context_chars == 0
            || self.max_context_chars > MAX_CONTEXT_CHARS
            || self.selected_collection_ids.is_empty()
            || self.selected_collection_ids.len() > MAX_SELECTED_COLLECTIONS
        {
            return Err("knowledge query is invalid or exceeds its local bounds".to_string());
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || remaining > Duration::from_secs(10) {
            return Err("knowledge query deadline is outside its local bound".to_string());
        }
        if matches!(&self.scope, KnowledgeScopeKind::MeetingReferences)
            && self.selected_collection_ids.len() != 1
        {
            return Err(
                "meeting-reference search requires one explicitly bound collection".to_string(),
            );
        }
        let unique: BTreeSet<String> = self.selected_collection_ids.iter().cloned().collect();
        if unique.len() != self.selected_collection_ids.len() {
            return Err("knowledge query repeats a collection".to_string());
        }
        if unique
            .iter()
            .any(|collection_id| !valid_opaque_id(collection_id))
        {
            return Err("knowledge query contains an invalid collection selector".to_string());
        }
        Ok(unique)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchHit {
    pub citation: EvidenceCitation,
    pub excerpt: String,
    pub score_basis_points: u32,
}

impl Drop for KnowledgeSearchHit {
    fn drop(&mut self) {
        self.excerpt.zeroize();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KnowledgeSearchStatus {
    Found,
    NotFound,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchResponse {
    pub status: KnowledgeSearchStatus,
    pub scope: KnowledgeScopeKind,
    pub evidence: Vec<KnowledgeSearchHit>,
}

/// Local lexical retrieval: ACL/source-version filtering precedes decryption
/// and ranking, result count/context are bounded, and every excerpt cites the
/// exact immutable document version and locator.
pub(crate) fn search_selected_knowledge(
    boundary: &dyn KnowledgeReadBoundary,
    chunks: &[StoredKnowledgeChunk],
    query: &KnowledgeQuery,
) -> Result<KnowledgeSearchResponse, String> {
    let selected = query.validate()?;
    if chunks.len() > MAX_SEARCH_CANDIDATES {
        return Err("knowledge candidate page exceeded its local bound".to_string());
    }
    let query_terms = lexical_terms(&query.query);
    if query_terms.is_empty() {
        return Err("knowledge query has no searchable terms".to_string());
    }

    let mut candidates = Vec::new();
    for chunk in chunks {
        if Instant::now() >= query.deadline {
            return Err("knowledge search exceeded its local deadline".to_string());
        }
        if !selected.contains(&chunk.collection_id) {
            continue;
        }
        let Some(grant) = boundary.authorize(query, &selected, chunk)? else {
            continue;
        };
        if grant.collection_id != chunk.collection_id
            || grant.document_id != chunk.document_id
            || grant.document_version_id != chunk.document_version_id
            || grant.acl_revision != chunk.acl_revision
            || !valid_opaque_id(&grant.owner_principal_ref)
            || grant.vault_id != chunk.private_context.vault_id
            || grant.account_generation == 0
            || grant.identity_generation == 0
            || !valid_opaque_id(&grant.grant_id)
            || !valid_opaque_id(&grant.private_asset_id)
            || grant.private_asset_id != chunk.id
            || chunk.private_context.entity_id != chunk.id
            || chunk.private_context.version != chunk.document_version_number
            || chunk.document_version_number == 0
            || chunk.private_context.purpose != PrivateAssetPurpose::KnowledgeDocument
            || chunk.key_ref != chunk.payload.key_ref
            || chunk.payload.ciphertext.len() > MAX_ENCRYPTED_CHUNK_BYTES
            || !valid_sha256(&chunk.content_sha256)
            || !valid_opaque_id(&chunk.id)
            || !valid_opaque_id(&chunk.collection_id)
            || !valid_opaque_id(&chunk.document_id)
            || !valid_opaque_id(&chunk.document_version_id)
            || !valid_opaque_id(&chunk.source_version)
        {
            return Err(
                "knowledge read grant does not match the selected source version".to_string(),
            );
        }
        let plaintext = boundary.open(
            &grant,
            &chunk.private_context,
            &chunk.key_ref,
            &chunk.payload,
        )?;
        let text = Zeroizing::new(
            std::str::from_utf8(&plaintext)
                .map_err(|_| "authorized knowledge chunk is not valid UTF-8".to_string())?
                .to_string(),
        );
        let chunk_terms = lexical_terms(&text);
        let overlap = query_terms.intersection(&chunk_terms).count();
        if overlap > 0 {
            let score_basis_points = ((overlap * 10_000) / query_terms.len()) as u32;
            candidates.push((score_basis_points, chunk.clone(), grant, text));
        }
    }

    candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    let mut hits = Vec::new();
    let mut context_chars = 0;
    for (score, chunk, grant, mut excerpt) in candidates.into_iter().take(query.max_results) {
        if Instant::now() >= query.deadline {
            return Err("knowledge search exceeded its local deadline".to_string());
        }
        if !boundary.revalidate(&grant)? {
            return Err("knowledge source authorization changed during retrieval".to_string());
        }
        let available = query.max_context_chars.saturating_sub(context_chars);
        if available == 0 {
            break;
        }
        if excerpt.chars().count() > available {
            excerpt = Zeroizing::new(excerpt.chars().take(available).collect());
        }
        let excerpt_chars = excerpt.chars().count();
        context_chars += excerpt_chars;
        let citation = EvidenceCitation {
            collection_id: chunk.collection_id,
            document_id: chunk.document_id,
            document_version_id: chunk.document_version_id,
            document_version_number: chunk.document_version_number,
            source_version: chunk.source_version,
            content_sha256: chunk.content_sha256,
            locator: chunk.locator,
            retrieved_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            read_grant_id: grant.grant_id,
            acl_revision: grant.acl_revision,
        };
        citation.validate()?;
        hits.push(KnowledgeSearchHit {
            citation,
            excerpt: excerpt.to_string(),
            score_basis_points: score,
        });
    }
    let status = if hits.is_empty() {
        KnowledgeSearchStatus::NotFound
    } else {
        KnowledgeSearchStatus::Found
    };
    Ok(KnowledgeSearchResponse {
        status,
        scope: query.scope.clone(),
        evidence: hits,
    })
}

fn lexical_terms(text: &str) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    let mut word = String::new();
    let mut run = Vec::new();
    let flush_word = |word: &mut String, terms: &mut BTreeSet<String>| {
        if word.chars().count() >= 2 {
            terms.insert(word.to_lowercase());
        }
        word.clear();
    };
    let flush_run = |run: &mut Vec<char>, terms: &mut BTreeSet<String>| {
        if run.len() >= 2 {
            for window in run.windows(2) {
                terms.insert(window.iter().copied().collect::<String>());
            }
        } else if let Some(character) = run.first() {
            terms.insert(character.to_string());
        }
        run.clear();
    };
    let mut in_non_ascii_run = false;
    for character in text.chars() {
        if character.is_alphanumeric() {
            if character.is_ascii() {
                if in_non_ascii_run {
                    flush_run(&mut run, &mut terms);
                    in_non_ascii_run = false;
                }
                word.push(character);
            } else {
                flush_word(&mut word, &mut terms);
                in_non_ascii_run = true;
                run.push(character);
            }
        } else {
            flush_word(&mut word, &mut terms);
            if in_non_ascii_run {
                flush_run(&mut run, &mut terms);
                in_non_ascii_run = false;
            }
        }
    }
    flush_word(&mut word, &mut terms);
    if in_non_ascii_run {
        flush_run(&mut run, &mut terms);
    }
    terms
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MetricBasis {
    Actual,
    Budget,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricObservation {
    pub metric_key: String,
    pub organization_ref: String,
    pub period_start: String,
    pub period_end: String,
    pub calendar: String,
    pub unit: String,
    pub currency: Option<String>,
    pub scale: String,
    pub basis: MetricBasis,
    pub value: Decimal,
    pub citation: EvidenceCitation,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Decimal {
    #[serde(with = "decimal_i128_string")]
    pub coefficient: i128,
    pub scale: u32,
}

mod decimal_i128_string {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &i128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<i128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

impl Decimal {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.bytes().any(|byte| matches!(byte, b'e' | b'E' | b'+')) {
            return Err("metric decimal must use plain base-10 notation".to_string());
        }
        let (negative, unsigned) = value
            .strip_prefix('-')
            .map(|rest| (true, rest))
            .unwrap_or((false, value));
        let mut parts = unsigned.split('.');
        let whole = parts.next().unwrap_or_default();
        let fraction = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() > 18
            || whole.len() + fraction.len() > 38
        {
            return Err("metric decimal is invalid or exceeds precision bounds".to_string());
        }
        let digits = format!("{whole}{fraction}");
        let mut coefficient = digits
            .parse::<i128>()
            .map_err(|_| "metric decimal exceeds integer bounds".to_string())?;
        if negative {
            coefficient = coefficient
                .checked_neg()
                .ok_or_else(|| "metric decimal exceeds integer bounds".to_string())?;
        }
        Ok(Self {
            coefficient,
            scale: fraction.len() as u32,
        })
    }

    pub(crate) fn format(self) -> String {
        if self.scale > 18 {
            return "invalid_decimal".to_string();
        }
        let negative = self.coefficient < 0;
        let digits = self.coefficient.unsigned_abs().to_string();
        let scale = self.scale as usize;
        let padded = if scale >= digits.len() {
            format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits)
        } else {
            digits
        };
        let formatted = if scale == 0 {
            padded
        } else {
            let point = padded.len() - scale;
            format!("{}.{}", &padded[..point], &padded[point..])
        };
        let formatted = if scale > 0 {
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            formatted
        };
        if negative && formatted != "0" {
            format!("-{formatted}")
        } else {
            formatted
        }
    }
}

pub(crate) fn seal_metric_observation_value(
    key: &[u8; 32],
    key_ref: &KnowledgeKeyRef,
    vault_id: &str,
    observation_id: &str,
    value: Decimal,
) -> Result<String, String> {
    let context = PrivateAssetContext {
        vault_id: vault_id.to_string(),
        entity_id: observation_id.to_string(),
        version: 1,
        purpose: PrivateAssetPurpose::KnowledgeEvidence,
    };
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&value)
            .map_err(|_| "metric observation value could not be encoded".to_string())?,
    );
    let payload = seal_private_payload(key, key_ref.clone(), &context, plaintext.as_slice())?;
    serde_json::to_string(&payload)
        .map_err(|_| "metric observation encryption envelope could not be encoded".to_string())
}

pub(crate) fn open_metric_observation_value(
    key: &[u8; 32],
    key_ref: &KnowledgeKeyRef,
    vault_id: &str,
    observation_id: &str,
    encoded_payload: &str,
) -> Result<Decimal, String> {
    let payload: EncryptedKnowledgePayload = serde_json::from_str(encoded_payload)
        .map_err(|_| "metric observation encryption envelope is malformed".to_string())?;
    if payload.key_ref.as_str() != key_ref.as_str()
        || payload.ciphertext.len() > MAX_ENCRYPTED_CHUNK_BYTES
    {
        return Err("metric observation encryption envelope is invalid".to_string());
    }
    let context = PrivateAssetContext {
        vault_id: vault_id.to_string(),
        entity_id: observation_id.to_string(),
        version: 1,
        purpose: PrivateAssetPurpose::KnowledgeEvidence,
    };
    let plaintext = open_private_payload(key, &context, &payload)?;
    let value: Decimal = serde_json::from_slice(plaintext.as_slice())
        .map_err(|_| "metric observation plaintext is malformed".to_string())?;
    if value.scale > 18 || value.format() == "invalid_decimal" {
        return Err("metric observation plaintext is malformed".to_string());
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MetricArithmeticError {
    InvalidEvidence,
    IncompatibleObservations,
    ConflictingSources,
    NotComputable,
    Overflow,
}

/// Computes `(actual - budget) / budget * 100` with checked integer decimal
/// arithmetic and two decimal places. Use
/// [`compute_selected_actual_vs_budget`] to reject conflicts across multiple
/// candidate sources before calling this single-pair operation.
pub(crate) fn actual_vs_budget_percent(
    actual: &MetricObservation,
    budget: &MetricObservation,
) -> Result<Decimal, MetricArithmeticError> {
    validate_metric_observation(actual)?;
    validate_metric_observation(budget)?;
    if actual.basis != MetricBasis::Actual || budget.basis != MetricBasis::Budget {
        return Err(MetricArithmeticError::IncompatibleObservations);
    }
    if actual.metric_key != budget.metric_key
        || actual.organization_ref != budget.organization_ref
        || actual.period_start != budget.period_start
        || actual.period_end != budget.period_end
        || actual.calendar != budget.calendar
        || actual.unit != budget.unit
        || actual.currency != budget.currency
        || actual.scale != budget.scale
    {
        return Err(MetricArithmeticError::IncompatibleObservations);
    }
    if budget.value.coefficient == 0 {
        return Err(MetricArithmeticError::NotComputable);
    }
    let common_scale = actual.value.scale.max(budget.value.scale);
    if common_scale > 18 {
        return Err(MetricArithmeticError::Overflow);
    }
    let actual_factor = pow10(common_scale - actual.value.scale)?;
    let budget_factor = pow10(common_scale - budget.value.scale)?;
    let actual_value = actual
        .value
        .coefficient
        .checked_mul(actual_factor)
        .ok_or(MetricArithmeticError::Overflow)?;
    let budget_value = budget
        .value
        .coefficient
        .checked_mul(budget_factor)
        .ok_or(MetricArithmeticError::Overflow)?;
    let difference = actual_value
        .checked_sub(budget_value)
        .ok_or(MetricArithmeticError::Overflow)?;
    let numerator = difference
        .checked_mul(10_000)
        .ok_or(MetricArithmeticError::Overflow)?;
    let denominator = budget_value;
    let mut coefficient = numerator
        .checked_div(denominator)
        .ok_or(MetricArithmeticError::Overflow)?;
    let remainder = numerator
        .checked_rem(denominator)
        .ok_or(MetricArithmeticError::Overflow)?;
    let denominator_abs = denominator.unsigned_abs();
    let round_threshold = denominator_abs / 2 + denominator_abs % 2;
    if remainder.unsigned_abs() >= round_threshold {
        let positive = (numerator >= 0) == (denominator >= 0);
        coefficient = coefficient
            .checked_add(if positive { 1 } else { -1 })
            .ok_or(MetricArithmeticError::Overflow)?;
    }
    Ok(Decimal {
        coefficient,
        scale: 2,
    })
}

pub(crate) fn validate_metric_observation(
    observation: &MetricObservation,
) -> Result<(), MetricArithmeticError> {
    let valid_dates = NaiveDate::parse_from_str(&observation.period_start, "%Y-%m-%d")
        .ok()
        .zip(NaiveDate::parse_from_str(&observation.period_end, "%Y-%m-%d").ok())
        .map(|(start, end)| end >= start)
        .unwrap_or(false);
    if !valid_opaque_id(&observation.metric_key)
        || !valid_opaque_id(&observation.organization_ref)
        || !valid_opaque_id(&observation.calendar)
        || !valid_opaque_id(&observation.unit)
        || !valid_opaque_id(&observation.scale)
        || observation
            .currency
            .as_ref()
            .is_some_and(|currency| !valid_opaque_id(currency))
        || observation.value.scale > 18
        || !valid_dates
        || observation.citation.validate().is_err()
    {
        return Err(MetricArithmeticError::InvalidEvidence);
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricComputation {
    pub percentage: Decimal,
    pub citations: Vec<EvidenceCitation>,
}

/// Resolves exactly one selected actual/budget pair. Conflicting sources or
/// multiple unresolved observations fail closed before arithmetic.
pub(crate) fn compute_selected_actual_vs_budget(
    actuals: &[MetricObservation],
    budgets: &[MetricObservation],
) -> Result<MetricComputation, MetricArithmeticError> {
    let observations: Vec<MetricObservation> = actuals.iter().chain(budgets).cloned().collect();
    if !conflicting_metric_sources(&observations).is_empty() {
        return Err(MetricArithmeticError::ConflictingSources);
    }
    if actuals.is_empty() || budgets.is_empty() {
        return Err(MetricArithmeticError::NotComputable);
    }
    if actuals.len() != 1 || budgets.len() != 1 {
        return Err(MetricArithmeticError::IncompatibleObservations);
    }
    let actual = &actuals[0];
    let budget = &budgets[0];
    let percentage = actual_vs_budget_percent(actual, budget)?;
    Ok(MetricComputation {
        percentage,
        citations: vec![actual.citation.clone(), budget.citation.clone()],
    })
}

fn pow10(exponent: u32) -> Result<i128, MetricArithmeticError> {
    let mut value = 1_i128;
    for _ in 0..exponent {
        value = value
            .checked_mul(10)
            .ok_or(MetricArithmeticError::Overflow)?;
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MetricConflictKey {
    pub metric_key: String,
    pub organization_ref: String,
    pub period_start: String,
    pub period_end: String,
    pub calendar: String,
    pub unit: String,
    pub currency: Option<String>,
    pub scale: String,
    pub basis: MetricBasis,
}

pub(crate) fn conflicting_metric_sources(observations: &[MetricObservation]) -> BTreeSet<String> {
    let mut grouped: BTreeMap<MetricConflictKey, (Decimal, BTreeSet<String>)> = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    for observation in observations {
        let key = MetricConflictKey {
            metric_key: observation.metric_key.clone(),
            organization_ref: observation.organization_ref.clone(),
            period_start: observation.period_start.clone(),
            period_end: observation.period_end.clone(),
            calendar: observation.calendar.clone(),
            unit: observation.unit.clone(),
            currency: observation.currency.clone(),
            scale: observation.scale.clone(),
            basis: observation.basis.clone(),
        };
        match grouped.get_mut(&key) {
            Some((value, source_refs)) if *value != observation.value => {
                conflicts.extend(source_refs.iter().cloned());
                conflicts.insert(observation.citation.document_version_id.clone());
            }
            Some((_, source_refs)) => {
                source_refs.insert(observation.citation.document_version_id.clone());
            }
            None => {
                grouped.insert(
                    key,
                    (
                        observation.value,
                        BTreeSet::from([observation.citation.document_version_id.clone()]),
                    ),
                );
            }
        }
    }
    conflicts
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CalendarBasis {
    Gregorian,
    Buddhist,
    FiscalNeedsConfiguration,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedPeriod {
    pub start: String,
    pub end: String,
    pub calendar: String,
    pub displayed_year: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum TemporalResolution {
    Resolved { period: ResolvedPeriod },
    NeedsClarification { reason: String },
}

/// `meeting_local_date` must come from the trusted meeting timezone context.
/// Fiscal periods are never inferred when no fiscal calendar was selected.
pub(crate) fn previous_year_period(
    meeting_local_date: NaiveDate,
    basis: CalendarBasis,
) -> TemporalResolution {
    if matches!(&basis, CalendarBasis::FiscalNeedsConfiguration) {
        return TemporalResolution::NeedsClarification {
            reason: "fiscal calendar is not configured".to_string(),
        };
    }
    let Some(year) = meeting_local_date.year().checked_sub(1) else {
        return TemporalResolution::NeedsClarification {
            reason: "meeting date is outside supported calendar range".to_string(),
        };
    };
    let Some(start) = NaiveDate::from_ymd_opt(year, 1, 1) else {
        return TemporalResolution::NeedsClarification {
            reason: "previous calendar year is invalid".to_string(),
        };
    };
    let Some(end) = NaiveDate::from_ymd_opt(year, 12, 31) else {
        return TemporalResolution::NeedsClarification {
            reason: "previous calendar year is invalid".to_string(),
        };
    };
    let (calendar, displayed_year) = match basis {
        CalendarBasis::Gregorian => ("gregorian", year),
        CalendarBasis::Buddhist => match year.checked_add(543) {
            Some(displayed_year) => ("buddhist", displayed_year),
            None => {
                return TemporalResolution::NeedsClarification {
                    reason: "meeting date is outside supported calendar range".to_string(),
                }
            }
        },
        CalendarBasis::FiscalNeedsConfiguration => unreachable!(),
    };
    TemporalResolution::Resolved {
        period: ResolvedPeriod {
            start: start.format("%Y-%m-%d").to_string(),
            end: end.format("%Y-%m-%d").to_string(),
            calendar: calendar.to_string(),
            displayed_year,
        },
    }
}

/// Pointer into the accepted People overlay. Display names are deliberately
/// absent; this value cannot establish identity without a current native review.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewedPeopleLinkRef {
    pub identity_link_id: String,
    pub expected_revision: u64,
    pub evidence_revision: u64,
    pub person_ref: String,
    pub state: String,
}

impl ReviewedPeopleLinkRef {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !self.identity_link_id.starts_with("identity-link:")
            || self.expected_revision == 0
            || self.evidence_revision != self.expected_revision
            || !valid_opaque_id(&self.person_ref)
            || self.state != "confirmed"
        {
            return Err("People link is not a current reviewed identity reference".to_string());
        }
        Ok(())
    }
}

/// Native keyring integration. A missing key must return an unavailable error;
/// callers must never mint a replacement key while opening an existing asset.
pub(crate) trait KnowledgeKeyProvider {
    fn load_key(&self, key_ref: &KnowledgeKeyRef) -> Result<Zeroizing<[u8; 32]>, String>;
}

const KEY_UNAVAILABLE: &str = "KEY_UNAVAILABLE";
const KEYRING_UNAVAILABLE: &str = "KEYRING_UNAVAILABLE";
const KEY_REF_COLLISION: &str = "KEY_REF_COLLISION";
const KEYRING_VERIFY_FAILED: &str = "KEYRING_VERIFY_FAILED";
const KEYRING_MUTEX_WAIT_MS: u32 = 15_000;

/// Minimal keyring boundary so creation semantics can be tested without a
/// real OS credential store. Implementations must make `create_if_absent`
/// refuse an existing slot rather than replacing it.
pub(crate) trait KnowledgeKeyringStore: Send + Sync {
    fn read_secret(&self, key_ref: &KnowledgeKeyRef) -> Result<Option<Zeroizing<Vec<u8>>>, String>;
    fn create_if_absent(&self, key_ref: &KnowledgeKeyRef, secret: &[u8]) -> Result<(), String>;
}

/// Native knowledge keys use a distinct account namespace from People keys.
/// Reads never create a missing slot; key creation is an explicit operation.
pub(crate) struct OsKnowledgeKeyBackend;

static NATIVE_KNOWLEDGE_KEYRING_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
struct NativeKeyringProcessLock {
    handle: windows_sys::Win32::Foundation::HANDLE,
    owned: bool,
}

#[cfg(not(windows))]
struct NativeKeyringProcessLock;

#[cfg(windows)]
impl NativeKeyringProcessLock {
    fn acquire(name: &str) -> Result<Self, String> {
        use std::ptr::null;
        use windows_sys::Win32::{
            Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
            System::Threading::{CreateMutexW, WaitForSingleObject},
        };

        let name = name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        unsafe {
            let handle = CreateMutexW(null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(KEYRING_UNAVAILABLE.to_string());
            }
            let result = WaitForSingleObject(handle, KEYRING_MUTEX_WAIT_MS);
            if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
                CloseHandle(handle);
                return Err(KEYRING_UNAVAILABLE.to_string());
            }
            Ok(Self {
                handle,
                owned: true,
            })
        }
    }

    fn release(mut self) -> Result<(), String> {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::ReleaseMutex};

        let released = unsafe { ReleaseMutex(self.handle) } != 0;
        self.owned = false;
        let closed = unsafe { CloseHandle(self.handle) } != 0;
        self.handle = std::ptr::null_mut();
        if released && closed {
            Ok(())
        } else {
            Err(KEYRING_UNAVAILABLE.to_string())
        }
    }
}

#[cfg(windows)]
impl Drop for NativeKeyringProcessLock {
    fn drop(&mut self) {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::ReleaseMutex};
        unsafe {
            if !self.handle.is_null() && self.owned {
                let _ = ReleaseMutex(self.handle);
            }
            if !self.handle.is_null() {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(not(windows))]
impl NativeKeyringProcessLock {
    fn acquire(_name: &str) -> Result<Self, String> {
        Ok(Self)
    }

    fn release(self) -> Result<(), String> {
        Ok(())
    }
}

fn knowledge_keyring_mutex_name(key_ref: &KnowledgeKeyRef) -> String {
    format!(
        "Global\\FUNG-knowledge-keyring-write-{:x}",
        Sha256::digest(key_ref.as_str().as_bytes())
    )
}

fn knowledge_keyring_account(key_ref: &KnowledgeKeyRef) -> Result<String, String> {
    let suffix = key_ref
        .as_str()
        .strip_prefix("knowledge:")
        .and_then(|suffix| Uuid::parse_str(suffix).ok())
        .ok_or_else(|| KEY_UNAVAILABLE.to_string())?;
    Ok(format!("knowledge::{suffix}"))
}

impl KnowledgeKeyringStore for OsKnowledgeKeyBackend {
    fn read_secret(&self, key_ref: &KnowledgeKeyRef) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        let account = knowledge_keyring_account(key_ref)?;
        let entry =
            keyring::Entry::new("FUNG", &account).map_err(|_| KEYRING_UNAVAILABLE.to_string())?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(KEYRING_UNAVAILABLE.to_string()),
        }
    }

    fn create_if_absent(&self, key_ref: &KnowledgeKeyRef, secret: &[u8]) -> Result<(), String> {
        let _write_guard = NATIVE_KNOWLEDGE_KEYRING_WRITE_LOCK
            .lock()
            .map_err(|_| KEYRING_UNAVAILABLE.to_string())?;
        let process_guard =
            NativeKeyringProcessLock::acquire(&knowledge_keyring_mutex_name(key_ref))?;
        let account = knowledge_keyring_account(key_ref)?;
        let entry =
            keyring::Entry::new("FUNG", &account).map_err(|_| KEYRING_UNAVAILABLE.to_string())?;
        match entry.get_secret() {
            Ok(existing) => {
                let _existing = Zeroizing::new(existing);
                return Err(KEY_REF_COLLISION.to_string());
            }
            Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err(KEYRING_UNAVAILABLE.to_string()),
        }
        entry
            .set_secret(secret)
            .map_err(|_| KEYRING_UNAVAILABLE.to_string())?;
        let stored = entry
            .get_secret()
            .map_err(|_| KEYRING_VERIFY_FAILED.to_string())?;
        let stored = Zeroizing::new(stored);
        if stored.as_slice() != secret {
            return Err(KEYRING_VERIFY_FAILED.to_string());
        }
        process_guard.release()
    }
}

impl KnowledgeKeyProvider for OsKnowledgeKeyBackend {
    fn load_key(&self, key_ref: &KnowledgeKeyRef) -> Result<Zeroizing<[u8; 32]>, String> {
        self.read(key_ref)
    }
}

impl OsKnowledgeKeyBackend {
    /// Creates a fresh key only when explicitly called and only in an unused
    /// keyring slot. It returns the exact keyring-read-back bytes.
    pub(crate) fn create(&self, key_ref: &KnowledgeKeyRef) -> Result<Zeroizing<[u8; 32]>, String> {
        let mut key = Zeroizing::new([0_u8; 32]);
        OsRng
            .try_fill_bytes(&mut *key)
            .map_err(|_| "OS_RANDOM_UNAVAILABLE".to_string())?;
        create_key_at(self, key_ref.clone(), key).map(|material| material.key)
    }

    /// Reads an existing key without provisioning a replacement when missing.
    pub(crate) fn read(&self, key_ref: &KnowledgeKeyRef) -> Result<Zeroizing<[u8; 32]>, String> {
        load_key_from_store(self, key_ref)
    }

    /// Restores an owner-authorized backup key without replacing an existing
    /// key reference. A matching existing key is an idempotent success.
    pub(crate) fn restore(
        &self,
        key_ref: &KnowledgeKeyRef,
        recovered: &[u8; 32],
    ) -> Result<Zeroizing<[u8; 32]>, String> {
        match self.read_secret(key_ref)? {
            Some(existing) if existing.as_slice() == recovered => return self.read(key_ref),
            Some(_) => return Err(KEY_REF_COLLISION.to_string()),
            None => {}
        }
        if let Err(create_error) = self.create_if_absent(key_ref, recovered) {
            match self.read_secret(key_ref)? {
                Some(existing) if existing.as_slice() == recovered => return self.read(key_ref),
                Some(_) => return Err(KEY_REF_COLLISION.to_string()),
                None => return Err(create_error),
            }
        }
        let restored = self.read(key_ref)?;
        if restored.as_ref() != recovered {
            return Err(KEYRING_VERIFY_FAILED.to_string());
        }
        Ok(restored)
    }

    /// Creates a new immutable key reference for rotation. The caller must
    /// re-encrypt assets before retiring the old key reference.
    pub(crate) fn new_rotation_ref(
        &self,
    ) -> Result<(KnowledgeKeyRef, Zeroizing<[u8; 32]>), String> {
        let material = create_random_key(self)?;
        Ok((material.key_ref, material.key))
    }

    pub(crate) fn rotate(
        &self,
        current_key_ref: &KnowledgeKeyRef,
    ) -> Result<(KnowledgeKeyRef, Zeroizing<[u8; 32]>), String> {
        let material = rotate_key_with_store(self, current_key_ref)?;
        Ok((material.key_ref, material.key))
    }
}

/// Newly-created material is returned only after the keyring reads back the
/// exact bytes. The caller owns wrapping/re-encrypting assets under this key.
pub(crate) struct KnowledgeKeyMaterial {
    pub key_ref: KnowledgeKeyRef,
    pub key: Zeroizing<[u8; 32]>,
}

fn load_key_from_store(
    store: &dyn KnowledgeKeyringStore,
    key_ref: &KnowledgeKeyRef,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let secret = store
        .read_secret(key_ref)?
        .ok_or_else(|| KEY_UNAVAILABLE.to_string())?;
    let bytes = secret.as_slice();
    if bytes.len() != 32 {
        return Err(KEY_UNAVAILABLE.to_string());
    }
    let mut key = Zeroizing::new([0_u8; 32]);
    key.copy_from_slice(bytes);
    Ok(key)
}

fn random_key_ref() -> Result<KnowledgeKeyRef, String> {
    let mut bytes = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "OS_RANDOM_UNAVAILABLE".to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    KnowledgeKeyRef::new(format!("knowledge:{}", Uuid::from_bytes(bytes)))
}

fn create_key_at(
    store: &dyn KnowledgeKeyringStore,
    key_ref: KnowledgeKeyRef,
    key: Zeroizing<[u8; 32]>,
) -> Result<KnowledgeKeyMaterial, String> {
    if store.read_secret(&key_ref)?.is_some() {
        return Err(KEY_REF_COLLISION.to_string());
    }
    store.create_if_absent(&key_ref, &key[..])?;
    let read_back = store
        .read_secret(&key_ref)?
        .ok_or_else(|| KEYRING_VERIFY_FAILED.to_string())?;
    if read_back.as_slice() != &key[..] {
        return Err(KEYRING_VERIFY_FAILED.to_string());
    }
    Ok(KnowledgeKeyMaterial { key_ref, key })
}

/// Explicitly provisions a fresh, purpose-separated 256-bit key. Existing
/// references are never overwritten; a collision fails closed.
fn create_random_key(store: &dyn KnowledgeKeyringStore) -> Result<KnowledgeKeyMaterial, String> {
    let key_ref = random_key_ref()?;
    let mut key = Zeroizing::new([0_u8; 32]);
    OsRng
        .try_fill_bytes(&mut *key)
        .map_err(|_| "OS_RANDOM_UNAVAILABLE".to_string())?;
    create_key_at(store, key_ref, key)
}

/// Provisions a new immutable key after confirming the current reference is
/// readable. The existing slot is retained so asset re-encryption can finish
/// before a separate, explicit retirement operation.
fn rotate_key_with_store(
    store: &dyn KnowledgeKeyringStore,
    current_key_ref: &KnowledgeKeyRef,
) -> Result<KnowledgeKeyMaterial, String> {
    let _current_key = load_key_from_store(store, current_key_ref)?;
    create_random_key(store)
}

#[cfg(test)]
mod knowledge_keyring_tests {
    use super::{
        create_key_at, create_random_key, knowledge_keyring_mutex_name, load_key_from_store,
        rotate_key_with_store, KnowledgeKeyRef, KnowledgeKeyringStore,
    };
    use std::{collections::BTreeMap, sync::Mutex};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    #[derive(Default)]
    struct FakeKnowledgeKeyring {
        entries: Mutex<BTreeMap<String, Zeroizing<Vec<u8>>>>,
    }

    impl FakeKnowledgeKeyring {
        fn seed(&self, key_ref: &KnowledgeKeyRef, bytes: &[u8]) {
            self.entries
                .lock()
                .unwrap()
                .insert(key_ref.as_str().to_string(), Zeroizing::new(bytes.to_vec()));
        }

        fn count(&self) -> usize {
            self.entries.lock().unwrap().len()
        }
    }

    impl KnowledgeKeyringStore for FakeKnowledgeKeyring {
        fn read_secret(
            &self,
            key_ref: &KnowledgeKeyRef,
        ) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
            Ok(self
                .entries
                .lock()
                .map_err(|_| "fake keyring unavailable".to_string())?
                .get(key_ref.as_str())
                .map(|secret| Zeroizing::new(secret.to_vec())))
        }

        fn create_if_absent(&self, key_ref: &KnowledgeKeyRef, secret: &[u8]) -> Result<(), String> {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| "fake keyring unavailable".to_string())?;
            if entries.contains_key(key_ref.as_str()) {
                return Err("KEY_REF_COLLISION".to_string());
            }
            entries.insert(
                key_ref.as_str().to_string(),
                Zeroizing::new(secret.to_vec()),
            );
            Ok(())
        }
    }

    fn key_ref(value: u128) -> KnowledgeKeyRef {
        KnowledgeKeyRef::new(format!("knowledge:{}", Uuid::from_u128(value))).unwrap()
    }

    #[test]
    fn missing_read_is_unavailable_and_does_not_create_a_key() {
        let store = FakeKnowledgeKeyring::default();
        let missing = load_key_from_store(&store, &key_ref(1));
        assert!(matches!(missing, Err(ref error) if error == "KEY_UNAVAILABLE"));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn explicit_create_is_read_back_and_subsequent_read_matches() {
        let store = FakeKnowledgeKeyring::default();
        let created = create_random_key(&store).unwrap();
        let loaded = load_key_from_store(&store, &created.key_ref).unwrap();
        assert_eq!(&*loaded, &*created.key);
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn duplicate_reference_is_rejected_without_overwriting_original() {
        let store = FakeKnowledgeKeyring::default();
        let reference = key_ref(2);
        let original =
            create_key_at(&store, reference.clone(), Zeroizing::new([0x31_u8; 32])).unwrap();
        let duplicate = create_key_at(&store, reference.clone(), Zeroizing::new([0x42_u8; 32]));
        assert!(matches!(
            duplicate,
            Err(ref error) if error == "KEY_REF_COLLISION"
        ));
        assert_eq!(
            &*load_key_from_store(&store, &reference).unwrap(),
            &*original.key
        );
    }

    #[test]
    fn keyring_process_mutex_uses_global_namespace() {
        let name = knowledge_keyring_mutex_name(&key_ref(1));
        assert!(name.starts_with("Global\\FUNG-knowledge-keyring-write-"));
    }

    #[test]
    fn preexisting_slot_collision_is_refused_without_overwrite() {
        let store = FakeKnowledgeKeyring::default();
        let reference = key_ref(3);
        store.seed(&reference, &[0x55_u8; 32]);
        let collision = create_key_at(&store, reference.clone(), Zeroizing::new([0x66_u8; 32]));
        assert!(matches!(
            collision,
            Err(ref error) if error == "KEY_REF_COLLISION"
        ));
        let preserved = load_key_from_store(&store, &reference).unwrap();
        assert_eq!(&*preserved, &[0x55_u8; 32]);
        assert_eq!(store.count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_keyring_named_mutex_serializes_processes() {
        const LOCK_ENV: &str = "FUNG_KEYRING_MUTEX_TEST_LOCK";
        const MARKER_ENV: &str = "FUNG_KEYRING_MUTEX_TEST_MARKER";
        if let (Ok(lock_name), Ok(marker_path)) =
            (std::env::var(LOCK_ENV), std::env::var(MARKER_ENV))
        {
            let _guard = super::NativeKeyringProcessLock::acquire(&lock_name).unwrap();
            let marker = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker_path)
                .expect("named mutex must serialize child processes");
            std::thread::sleep(std::time::Duration::from_millis(500));
            drop(marker);
            std::fs::remove_file(marker_path).unwrap();
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let lock_name = format!("Global\\FUNG-keyring-race-test-{}", Uuid::new_v4());
        let marker_path = directory.path().join("active-child");
        let executable = std::env::current_exe().unwrap();
        let test_name = "meeting_knowledge::knowledge_keyring_tests::windows_keyring_named_mutex_serializes_processes";
        let mut children = Vec::new();
        for _ in 0..2 {
            children.push(
                std::process::Command::new(&executable)
                    .arg("--exact")
                    .arg(test_name)
                    .arg("--nocapture")
                    .env(LOCK_ENV, &lock_name)
                    .env(MARKER_ENV, &marker_path)
                    .spawn()
                    .unwrap(),
            );
        }
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
    }

    #[test]
    fn rotation_creates_a_new_key_and_preserves_the_old_reference() {
        let store = FakeKnowledgeKeyring::default();
        let original_ref = key_ref(4);
        let original =
            create_key_at(&store, original_ref.clone(), Zeroizing::new([0x17_u8; 32])).unwrap();
        let rotated = rotate_key_with_store(&store, &original_ref).unwrap();
        assert_ne!(rotated.key_ref, original_ref);
        assert_ne!(&*rotated.key, &*original.key);
        assert_eq!(
            &*load_key_from_store(&store, &original_ref).unwrap(),
            &*original.key
        );
        assert_eq!(
            &*load_key_from_store(&store, &rotated.key_ref).unwrap(),
            &*rotated.key
        );
        assert_eq!(store.count(), 2);
    }
}
