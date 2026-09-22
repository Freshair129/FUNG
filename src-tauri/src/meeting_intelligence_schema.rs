use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_IDENTITY_ENVELOPE_CIPHERTEXT_BYTES: usize = 4096;

/// Versioned local contract shared by the LT, KE, MA, and SI-API lanes.
///
/// This module deliberately contains data contracts and fail-closed validation
/// only. Genesis transaction construction stays in `genesis_adapter` so later
/// consumers cannot bypass the single persistence boundary.
pub(crate) const CONTRACT_VERSION: i64 = 1;
pub(crate) const SCHEMA_VERSION: i64 = 11;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct MeetingScope {
    pub project_id: String,
    pub recording_id: String,
    pub meeting_session_id: String,
    pub source_session_id: String,
    pub track_id: String,
    pub source_generation: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceCoverageKind {
    Audio,
    Gap,
}

impl SourceCoverageKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Gap => "gap",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct SourceCoverageInput {
    pub id: String,
    pub source_session_id: String,
    pub track_id: String,
    pub source_generation: i64,
    pub sequence_no: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub kind: SourceCoverageKind,
    pub audio_chunk_id: Option<String>,
    /// These are caller observations. The adapter replaces them with values
    /// read from the durable audio chunk and the file itself before commit.
    pub file_path: Option<String>,
    pub byte_size: Option<i64>,
    pub checksum: Option<String>,
    pub gap_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TranscriptOrigin {
    LocalAsr,
    ProviderAsr,
    Human,
    Refinement,
}

impl TranscriptOrigin {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::LocalAsr => "local_asr",
            Self::ProviderAsr => "provider_asr",
            Self::Human => "human",
            Self::Refinement => "refinement",
        }
    }

    pub(crate) fn is_manual(&self) -> bool {
        matches!(self, Self::Human)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct TranscriptRevisionInput {
    pub id: String,
    pub utterance_id: String,
    pub revision: i64,
    pub supersedes_revision: Option<i64>,
    pub expected_revision: Option<i64>,
    pub origin: TranscriptOrigin,
    pub raw_text: String,
    pub effective_text: String,
    pub language: Option<String>,
    pub confidence: Option<f64>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub model_run_id: Option<String>,
    pub review_state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ParticipantAttribution {
    pub kind: String,
    pub participant_session_id: Option<String>,
    pub speaker_cluster_id: Option<String>,
    pub label_snapshot_ref: Option<String>,
    pub provider_ref_ciphertext: Option<PrivateIdentityReference>,
    pub identity_link_id: Option<String>,
    pub identity_expected_revision: Option<i64>,
    pub person_ref_ciphertext: Option<PrivateIdentityReference>,
    pub evidence_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct KnowledgeEvidenceInput {
    pub id: String,
    pub collection_id: String,
    pub document_id: String,
    pub document_version_id: String,
    pub source_version: String,
    pub evidence_bundle_id: Option<String>,
    pub citation: Value,
    pub read_grant_id: Option<String>,
    pub share_grant_id: Option<String>,
    pub read_state: String,
    pub share_state: String,
    pub audience_policy_revision: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct MeetingAgentGrantContract {
    pub contract_version: i64,
    pub grant_id: String,
    pub meeting_session_id: String,
    pub mode: String,
    pub policy_version: String,
    pub expected_revision: i64,
    pub state: String,
    pub collection_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct MeetingDeliveryContract {
    pub contract_version: i64,
    pub grant_id: Option<String>,
    pub evidence_bundle_id: Option<String>,
    pub destination_id: String,
    pub payload_hash: String,
    pub idempotency_key: String,
    pub audience_policy_revision: i64,
    pub state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ParticipantSourceEvidenceContract {
    pub contract_version: i64,
    pub participant_session_id: String,
    pub source_session_id: String,
    pub track_id: String,
    pub source_generation: i64,
    pub speaker_cluster_id: String,
    pub label_snapshot_ref: String,
    pub evidence_revision: i64,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct AtomicMeetingRequest {
    pub event_id: String,
    pub source_cursor: i64,
    pub committed_cursor: i64,
    pub scope: MeetingScope,
    pub sources: Vec<SourceCoverageInput>,
    pub revision: TranscriptRevisionInput,
    pub attribution: ParticipantAttribution,
    pub knowledge: Option<KnowledgeEvidenceInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct MeetingCommitAttempt {
    pub transaction_id: String,
    pub expected_frontier: u64,
    pub committed_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct CommittedMeetingEvent {
    pub contract_version: i64,
    pub event_id: String,
    pub transaction_id: String,
    pub cursor: i64,
    pub revision_id: String,
    pub payload_hash: String,
    pub commit_sequence: Option<u64>,
    pub idempotent: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PrivateIdentityReference {
    pub encrypted_blob_ref: String,
    pub ciphertext_sha256: String,
    pub envelope: IdentityEnvelope,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct IdentityEnvelope {
    pub version: u8,
    pub key_ref: String,
    pub nonce: Vec<u8>,
    pub aad_sha256: String,
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct IdentityAadContext {
    pub account_ref: Option<String>,
    pub scope: String,
    pub vault_id: String,
    pub entity_id: String,
    pub revision: i64,
    pub model_context: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PersonIdentityPayload {
    pub link_id: String,
    pub profile_id: String,
    pub person_id: String,
    pub display_name: String,
    pub account_ref: Option<String>,
    pub vault_id: String,
    pub relationship_revision: i64,
    pub profile_revision: i64,
}

impl Zeroize for PersonIdentityPayload {
    fn zeroize(&mut self) {
        self.link_id.zeroize();
        self.profile_id.zeroize();
        self.person_id.zeroize();
        self.display_name.zeroize();
        self.account_ref.zeroize();
        self.vault_id.zeroize();
        self.relationship_revision.zeroize();
        self.profile_revision.zeroize();
    }
}

#[derive(PartialEq, Eq)]
pub(crate) struct AuthorizedPerson {
    pub person_id: String,
    pub display_name: String,
    pub link_id: String,
    pub profile_id: String,
    pub profile_revision: i64,
}

impl std::fmt::Debug for AuthorizedPerson {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizedPerson")
            .field("redacted", &true)
            .finish()
    }
}

impl Drop for AuthorizedPerson {
    fn drop(&mut self) {
        self.person_id.zeroize();
        self.display_name.zeroize();
        self.link_id.zeroize();
        self.profile_id.zeroize();
        self.profile_revision.zeroize();
    }
}

pub(crate) trait IdentityKeyBackend {
    fn get_key(&self, key_ref: &str) -> Result<Option<Vec<u8>>, String>;
}

pub(crate) struct OsPeopleMetadataKeyBackend;

impl IdentityKeyBackend for OsPeopleMetadataKeyBackend {
    fn get_key(&self, key_ref: &str) -> Result<Option<Vec<u8>>, String> {
        validate_key_ref(key_ref)?;
        let suffix = key_ref
            .strip_prefix("people_metadata:")
            .ok_or_else(|| "identity key namespace is invalid".to_string())?;
        let account = format!("people_metadata::{suffix}");
        let entry = keyring::Entry::new("FUNG", &account)
            .map_err(|_| "identity keyring unavailable".to_string())?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("identity keyring unavailable".to_string()),
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryIdentityKeyBackend {
    keys: std::collections::BTreeMap<String, Vec<u8>>,
}

#[cfg(test)]
impl InMemoryIdentityKeyBackend {
    pub(crate) fn insert(&mut self, key_ref: &str, key: Vec<u8>) {
        self.keys.insert(key_ref.to_string(), key);
    }
}

#[cfg(test)]
impl Drop for InMemoryIdentityKeyBackend {
    fn drop(&mut self) {
        for key in self.keys.values_mut() {
            key.zeroize();
        }
    }
}

#[cfg(test)]
impl IdentityKeyBackend for InMemoryIdentityKeyBackend {
    fn get_key(&self, key_ref: &str) -> Result<Option<Vec<u8>>, String> {
        validate_key_ref(key_ref)?;
        Ok(self.keys.get(key_ref).cloned())
    }
}

fn valid_id(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("invalid {field}"));
    }
    Ok(())
}

fn valid_sha256(value: &str, field: &str) -> Result<(), String> {
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(format!("invalid {field}"));
    }
    Ok(())
}

impl MeetingScope {
    pub(crate) fn validate(&self) -> Result<(), String> {
        valid_id(&self.project_id, "project_id")?;
        valid_id(&self.recording_id, "recording_id")?;
        valid_id(&self.meeting_session_id, "meeting_session_id")?;
        valid_id(&self.source_session_id, "source_session_id")?;
        valid_id(&self.track_id, "track_id")?;
        if self.source_generation < 1 {
            return Err("source_generation must start at 1".to_string());
        }
        Ok(())
    }
}

impl SourceCoverageInput {
    pub(crate) fn validate(&self, scope: &MeetingScope) -> Result<(), String> {
        valid_id(&self.id, "source coverage id")?;
        valid_id(&self.source_session_id, "source_session_id")?;
        valid_id(&self.track_id, "track_id")?;
        if self.source_session_id != scope.source_session_id
            || self.track_id != scope.track_id
            || self.source_generation != scope.source_generation
        {
            return Err("source coverage scope/generation mismatch".to_string());
        }
        if self.sequence_no < 0 || self.start_ms < 0 || self.end_ms <= self.start_ms {
            return Err("source coverage range is invalid".to_string());
        }
        match self.kind {
            SourceCoverageKind::Audio => {
                valid_id(
                    self.audio_chunk_id
                        .as_deref()
                        .ok_or_else(|| "audio coverage needs audio_chunk_id".to_string())?,
                    "audio_chunk_id",
                )?;
                if let Some(path) = &self.file_path {
                    if path.trim().is_empty() || path.contains("://") || path.starts_with("\\\\") {
                        return Err("audio coverage path must be a local path".to_string());
                    }
                }
                if let Some(byte_size) = self.byte_size {
                    if byte_size <= 0 {
                        return Err("audio coverage byte_size must be positive".to_string());
                    }
                }
                if let Some(checksum) = &self.checksum {
                    valid_sha256(checksum, "audio coverage checksum")?;
                }
                if self.gap_reason.is_some() {
                    return Err("audio coverage cannot carry gap_reason".to_string());
                }
            }
            SourceCoverageKind::Gap => {
                if self.audio_chunk_id.is_some()
                    || self.file_path.is_some()
                    || self.byte_size.is_some()
                    || self.checksum.is_some()
                {
                    return Err("gap coverage cannot carry audio custody fields".to_string());
                }
                let reason = self
                    .gap_reason
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "gap coverage needs a non-empty reason".to_string())?;
                if reason.eq_ignore_ascii_case("silence")
                    || reason.to_ascii_lowercase().contains("silence")
                {
                    return Err("a source gap cannot be labelled silence".to_string());
                }
            }
        }
        Ok(())
    }
}

impl ParticipantAttribution {
    pub(crate) fn validate(&self) -> Result<(), String> {
        valid_id(&self.kind, "attribution kind")?;
        if self.participant_session_id.is_none() && self.speaker_cluster_id.is_none() {
            return Err(
                "attribution needs participant_session_id or speaker_cluster_id".to_string(),
            );
        }
        if self.evidence_revision < 0 {
            return Err("attribution evidence_revision must be non-negative".to_string());
        }
        for (value, field) in [
            (
                self.participant_session_id.as_deref(),
                "participant_session_id",
            ),
            (self.speaker_cluster_id.as_deref(), "speaker_cluster_id"),
            (self.label_snapshot_ref.as_deref(), "label_snapshot_ref"),
        ] {
            if let Some(value) = value {
                valid_id(value, field)?;
            }
        }
        if let Some(reference) = &self.provider_ref_ciphertext {
            validate_private_identity_reference(reference)?;
        }
        if let Some(reference) = &self.person_ref_ciphertext {
            validate_private_identity_reference(reference)?;
        }
        if let Some(link_id) = &self.identity_link_id {
            valid_id(link_id, "identity_link_id")?;
            if !link_id.starts_with("identity-link:") {
                return Err("identity_link_id must be an opaque native reference".to_string());
            }
            if self.kind != "confirmed_person" {
                return Err(
                    "only confirmed_person attribution may carry an identity link".to_string(),
                );
            }
            if self.identity_expected_revision.is_none() {
                return Err("identity link requires expected review revision".to_string());
            }
        } else if self.identity_expected_revision.is_some() || self.person_ref_ciphertext.is_some()
        {
            return Err("identity review fields require an identity link".to_string());
        }
        if self.kind == "confirmed_person" && self.identity_link_id.is_none() {
            return Err(
                "confirmed_person attribution requires a reviewed identity link".to_string(),
            );
        }
        Ok(())
    }
}

impl KnowledgeEvidenceInput {
    pub(crate) fn validate(&self) -> Result<(), String> {
        valid_id(&self.id, "knowledge evidence id")?;
        valid_id(&self.collection_id, "knowledge collection id")?;
        valid_id(&self.document_id, "knowledge document id")?;
        valid_id(&self.document_version_id, "knowledge document version id")?;
        valid_id(&self.source_version, "knowledge source version")?;
        if self
            .evidence_bundle_id
            .as_deref()
            .is_some_and(|value| valid_id(value, "evidence bundle id").is_err())
        {
            return Err("invalid evidence bundle id".to_string());
        }
        valid_id(&self.read_state, "read_state")?;
        valid_id(&self.share_state, "share_state")?;
        if self.read_state == "granted" && self.read_grant_id.is_none() {
            return Err("read grant state needs read_grant_id".to_string());
        }
        if self.share_state == "granted" {
            if self.read_state != "granted"
                || self.share_grant_id.is_none()
                || self.audience_policy_revision.is_none()
            {
                return Err("sharing requires an explicit readable audience grant".to_string());
            }
        } else if self.share_grant_id.is_some() {
            return Err("share_grant_id is not valid without granted share_state".to_string());
        }
        if self
            .audience_policy_revision
            .is_some_and(|revision| revision < 0)
        {
            return Err("audience_policy_revision must be non-negative".to_string());
        }
        reject_sensitive_json_keys(&self.citation, "knowledge citation")?;
        Ok(())
    }
}

impl AtomicMeetingRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        valid_id(&self.event_id, "event_id")?;
        self.scope.validate()?;
        if self.source_cursor < 0 || self.committed_cursor < 0 || self.sources.is_empty() {
            return Err("source/event cursors or coverage are invalid".to_string());
        }
        for source in &self.sources {
            source.validate(&self.scope)?;
        }
        let mut sequences = self
            .sources
            .iter()
            .map(|source| source.sequence_no)
            .collect::<Vec<_>>();
        sequences.sort_unstable();
        if sequences
            .windows(2)
            .any(|window| window[1] != window[0].saturating_add(1))
            || sequences.last().copied() != Some(self.source_cursor)
        {
            return Err(
                "source coverage sequence and source cursor are not contiguous".to_string(),
            );
        }
        for pair in self.sources.iter().enumerate() {
            for other in self.sources.iter().skip(pair.0 + 1) {
                if pair.1.track_id == other.track_id
                    && pair.1.source_generation == other.source_generation
                    && pair.1.start_ms < other.end_ms
                    && other.start_ms < pair.1.end_ms
                {
                    return Err("source coverage ranges overlap".to_string());
                }
            }
        }

        let revision = &self.revision;
        valid_id(&revision.id, "transcript revision id")?;
        valid_id(&revision.utterance_id, "utterance_id")?;
        if revision.revision < 1
            || revision.start_ms < 0
            || revision.end_ms <= revision.start_ms
            || revision.raw_text.is_empty()
            || revision.effective_text.is_empty()
        {
            return Err("transcript revision fields or range are invalid".to_string());
        }
        if revision
            .confidence
            .is_some_and(|confidence| !(0.0..=1.0).contains(&confidence))
        {
            return Err("confidence must be nullable or between 0 and 1".to_string());
        }
        if revision.expected_revision.is_some_and(|value| value < 0)
            || revision.supersedes_revision.is_some_and(|value| value < 1)
        {
            return Err("revision expectations must be non-negative".to_string());
        }
        if revision
            .model_run_id
            .as_deref()
            .is_some_and(|value| value.is_empty())
        {
            return Err("model_run_id cannot be empty".to_string());
        }
        if revision.review_state != "unreviewed" && revision.review_state != "reviewed" {
            return Err("review_state must be unreviewed or reviewed".to_string());
        }
        if revision.start_ms
            < self
                .sources
                .iter()
                .map(|source| source.start_ms)
                .min()
                .unwrap_or(0)
            || revision.end_ms
                > self
                    .sources
                    .iter()
                    .map(|source| source.end_ms)
                    .max()
                    .unwrap_or(0)
        {
            return Err("transcript revision is outside supplied source coverage".to_string());
        }
        self.attribution.validate()?;
        if let Some(knowledge) = &self.knowledge {
            knowledge.validate()?;
        }
        Ok(())
    }
}

pub(crate) fn audio_range_is_covered(spans: &[(i64, i64)], start_ms: i64, end_ms: i64) -> bool {
    if start_ms < 0 || end_ms <= start_ms {
        return false;
    }
    let mut sorted = spans.to_vec();
    sorted.sort_unstable_by_key(|span| span.0);
    let mut cursor = start_ms;
    for (start, end) in sorted {
        if end <= cursor {
            continue;
        }
        if start > cursor {
            return false;
        }
        cursor = cursor.max(end);
        if cursor >= end_ms {
            return true;
        }
    }
    false
}

pub(crate) fn canonical_sha256<T: Serialize>(value: &T) -> Result<String, String> {
    let encoded = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_key_ref(value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("people_metadata:") else {
        return Err("identity key must use the people_metadata namespace".to_string());
    };
    if suffix.len() != 32
        || !suffix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("identity key reference must be opaque".to_string());
    }
    Ok(())
}

fn validate_blob_ref(value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("identity-envelope:") else {
        return Err("private identity data needs an opaque envelope reference".to_string());
    };
    if suffix.len() != 32
        || !suffix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("private identity data needs an opaque envelope reference".to_string());
    }
    Ok(())
}

impl PersonIdentityPayload {
    fn validate(&self) -> Result<(), String> {
        valid_id(&self.link_id, "identity link id")?;
        valid_id(&self.profile_id, "opaque profile id")?;
        if !self.profile_id.starts_with("profile:") {
            return Err("identity profile id must be an opaque native reference".to_string());
        }
        valid_id(&self.person_id, "decrypted person id")?;
        valid_id(&self.display_name, "decrypted person display name")?;
        if let Some(account_ref) = self.account_ref.as_deref() {
            valid_id(account_ref, "identity account ref")?;
        }
        valid_id(&self.vault_id, "identity vault id")?;
        if self.relationship_revision < 1 || self.profile_revision < 1 {
            return Err("identity relationship revisions must be positive".to_string());
        }
        Ok(())
    }
}

pub(crate) fn seal_person_identity(
    payload: &PersonIdentityPayload,
    context: &IdentityAadContext,
    key_ref: &str,
    backend: &dyn IdentityKeyBackend,
) -> Result<PrivateIdentityReference, String> {
    validate_key_ref(key_ref)?;
    payload.validate()?;
    if context.revision < 1 {
        return Err("identity AAD revision must be positive".to_string());
    }
    let key = backend
        .get_key(key_ref)?
        .ok_or_else(|| "identity key is unavailable".to_string())?;
    let key = Zeroizing::new(key);
    if key.len() != 32 {
        return Err("identity key has invalid length".to_string());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| "identity cipher initialization failed".to_string())?;
    let aad =
        serde_json::to_vec(context).map_err(|_| "identity AAD serialization failed".to_string())?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(payload)
            .map_err(|_| "identity payload serialization failed".to_string())?,
    );
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|_| "identity encryption failed".to_string())?;
    let reference = PrivateIdentityReference {
        encrypted_blob_ref: format!("identity-envelope:{}", Uuid::new_v4().simple()),
        ciphertext_sha256: sha256_hex(&ciphertext),
        envelope: IdentityEnvelope {
            version: 1,
            key_ref: key_ref.to_string(),
            nonce: nonce.to_vec(),
            aad_sha256: sha256_hex(&aad),
            ciphertext,
        },
    };
    validate_private_identity_reference(&reference)?;
    Ok(reference)
}

pub(crate) fn open_person_identity(
    reference: &PrivateIdentityReference,
    context: &IdentityAadContext,
    expected_link_id: &str,
    expected_revision: i64,
    backend: &dyn IdentityKeyBackend,
) -> Result<AuthorizedPerson, String> {
    validate_private_identity_reference(reference)?;
    if context.revision != expected_revision {
        return Err("identity AAD revision does not match review revision".to_string());
    }
    let key = backend
        .get_key(&reference.envelope.key_ref)?
        .ok_or_else(|| "identity key is unavailable".to_string())?;
    let key = Zeroizing::new(key);
    if key.len() != 32 {
        return Err("identity key has invalid length".to_string());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| "identity cipher initialization failed".to_string())?;
    let aad =
        serde_json::to_vec(context).map_err(|_| "identity AAD serialization failed".to_string())?;
    if sha256_hex(&aad) != reference.envelope.aad_sha256 {
        return Err("identity AAD context mismatch".to_string());
    }
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&reference.envelope.nonce),
                Payload {
                    msg: &reference.envelope.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| "identity authentication failed".to_string())?,
    );
    let payload = Zeroizing::new(
        serde_json::from_slice::<PersonIdentityPayload>(&plaintext)
            .map_err(|_| "identity payload decode failed".to_string())?,
    );
    payload.validate()?;
    if payload.link_id != expected_link_id
        || payload.account_ref != context.account_ref
        || payload.vault_id != context.vault_id
        || payload.relationship_revision != expected_revision
    {
        return Err("identity semantic relationship mismatch".to_string());
    }
    Ok(AuthorizedPerson {
        person_id: payload.person_id.clone(),
        display_name: payload.display_name.clone(),
        link_id: payload.link_id.clone(),
        profile_id: payload.profile_id.clone(),
        profile_revision: payload.profile_revision,
    })
}

pub(crate) fn validate_private_identity_reference(
    reference: &PrivateIdentityReference,
) -> Result<(), String> {
    validate_blob_ref(&reference.encrypted_blob_ref)?;
    valid_sha256(&reference.ciphertext_sha256, "ciphertext_sha256")?;
    if reference.envelope.version != 1
        || reference.envelope.nonce.len() != 24
        || reference.envelope.ciphertext.len() < 16
    {
        return Err("identity envelope is malformed".to_string());
    }
    if reference.envelope.ciphertext.len() > MAX_IDENTITY_ENVELOPE_CIPHERTEXT_BYTES {
        return Err("identity envelope exceeds the bounded payload limit".to_string());
    }
    validate_key_ref(&reference.envelope.key_ref)?;
    valid_sha256(&reference.envelope.aad_sha256, "identity aad digest")?;
    if sha256_hex(&reference.envelope.ciphertext) != reference.ciphertext_sha256 {
        return Err("identity ciphertext digest mismatch".to_string());
    }
    Ok(())
}

fn reject_sensitive_json_keys(value: &Value, field: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let normalized = key.to_ascii_lowercase();
                if [
                    "secret",
                    "token",
                    "password",
                    "private_key",
                    "biometric",
                    "embedding",
                    "voiceprint",
                ]
                .iter()
                .any(|forbidden| normalized.contains(forbidden))
                {
                    return Err(format!("{field} contains protected field {key}"));
                }
                reject_sensitive_json_keys(child, field)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_sensitive_json_keys(child, field)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_text_is_not_an_identity_key() {
        let first = TranscriptRevisionInput {
            id: "revision-a".to_string(),
            utterance_id: "utterance-a".to_string(),
            revision: 1,
            supersedes_revision: None,
            expected_revision: None,
            origin: TranscriptOrigin::LocalAsr,
            raw_text: "same words".to_string(),
            effective_text: "same words".to_string(),
            language: None,
            confidence: None,
            start_ms: 0,
            end_ms: 100,
            model_run_id: None,
            review_state: "unreviewed".to_string(),
        };
        let mut second = first.clone();
        second.id = "revision-b".to_string();
        second.utterance_id = "utterance-b".to_string();
        assert_ne!(
            canonical_sha256(&first).unwrap(),
            canonical_sha256(&second).unwrap()
        );
    }

    #[test]
    fn gap_and_identity_boundaries_are_fail_closed() {
        let scope = MeetingScope {
            project_id: "p".to_string(),
            recording_id: "r".to_string(),
            meeting_session_id: "m".to_string(),
            source_session_id: "s".to_string(),
            track_id: "t".to_string(),
            source_generation: 1,
        };
        let gap = SourceCoverageInput {
            id: "gap".to_string(),
            source_session_id: "s".to_string(),
            track_id: "t".to_string(),
            source_generation: 1,
            sequence_no: 0,
            start_ms: 0,
            end_ms: 100,
            kind: SourceCoverageKind::Gap,
            audio_chunk_id: None,
            file_path: None,
            byte_size: None,
            checksum: None,
            gap_reason: Some("silence".to_string()),
        };
        assert!(gap.validate(&scope).is_err());
        let identity = ParticipantAttribution {
            kind: "confirmed_person".to_string(),
            participant_session_id: Some("participant-session".to_string()),
            speaker_cluster_id: None,
            label_snapshot_ref: Some("label-snapshot".to_string()),
            provider_ref_ciphertext: None,
            identity_link_id: Some("link".to_string()),
            identity_expected_revision: Some(1),
            person_ref_ciphertext: None,
            evidence_revision: 1,
        };
        assert!(identity.validate().is_err());
    }

    #[test]
    fn oversized_identity_envelope_fails_closed_before_decrypt() {
        let ciphertext = vec![0u8; MAX_IDENTITY_ENVELOPE_CIPHERTEXT_BYTES + 1];
        let reference = PrivateIdentityReference {
            encrypted_blob_ref: "identity-envelope:11111111111111111111111111111111".to_string(),
            ciphertext_sha256: sha256_hex(&ciphertext),
            envelope: IdentityEnvelope {
                version: 1,
                key_ref: "people_metadata:22222222222222222222222222222222".to_string(),
                nonce: vec![0; 24],
                aad_sha256: "3".repeat(64),
                ciphertext,
            },
        };
        let backend = InMemoryIdentityKeyBackend::default();
        let context = IdentityAadContext {
            account_ref: Some("account".to_string()),
            scope: "scope".to_string(),
            vault_id: "vault".to_string(),
            entity_id: "identity-link:11111111111111111111111111111111".to_string(),
            revision: 1,
            model_context: "none".to_string(),
        };
        let error = open_person_identity(
            &reference,
            &context,
            "identity-link:11111111111111111111111111111111",
            1,
            &backend,
        )
        .unwrap_err();
        assert!(error.contains("bounded payload limit"));
    }
}
