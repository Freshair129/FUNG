//! Provider-neutral meeting transport contracts.
//!
//! This module describes the boundary only. The production implementation is
//! deliberately unconfigured; it cannot join a room or publish content. A raw
//! provider DTO remains untrusted until a trusted verifier port has checked its
//! signature, scope, age and replay status.

#![allow(dead_code)]

use serde::Serialize;
use std::fmt;

pub(crate) const PROVIDER_NOT_CONFIGURED: &str = "PROVIDER_NOT_CONFIGURED";
pub(crate) const MAX_ID_BYTES: usize = 128;
pub(crate) const MAX_LABEL_BYTES: usize = 256;
pub(crate) const MAX_CODEC_BYTES: usize = 32;
pub(crate) const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_AUDIO_FRAME_BYTES: usize = 512 * 1024;
pub(crate) const MAX_TEXT_BYTES: usize = 2_000;
pub(crate) const MAX_LINK_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdapterErrorCode {
    ProviderNotConfigured,
    InvalidRequest,
    ScopeMismatch,
    AuthenticationRequired,
    AuthenticationFailed,
    ReplayDetected,
    CursorGap,
    UnsupportedCapability,
    TransportUnavailable,
}

impl AdapterErrorCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProviderNotConfigured => PROVIDER_NOT_CONFIGURED,
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::ScopeMismatch => "SCOPE_MISMATCH",
            Self::AuthenticationRequired => "AUTHENTICATION_REQUIRED",
            Self::AuthenticationFailed => "AUTHENTICATION_FAILED",
            Self::ReplayDetected => "REPLAY_DETECTED",
            Self::CursorGap => "CURSOR_GAP",
            Self::UnsupportedCapability => "UNSUPPORTED_CAPABILITY",
            Self::TransportUnavailable => "TRANSPORT_UNAVAILABLE",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MeetingAdapterError {
    pub(crate) code: AdapterErrorCode,
    detail: &'static str,
}

impl MeetingAdapterError {
    pub(crate) fn new(code: AdapterErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl fmt::Display for MeetingAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for MeetingAdapterError {}

fn invalid_request(detail: &'static str) -> MeetingAdapterError {
    MeetingAdapterError::new(AdapterErrorCode::InvalidRequest, detail)
}

fn provider_not_configured() -> MeetingAdapterError {
    MeetingAdapterError::new(
        AdapterErrorCode::ProviderNotConfigured,
        "no meeting transport provider is configured",
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum CapabilityAvailability {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum ReadinessBlocker {
    ProviderNotConfigured,
}

/// Protocol capability and operational readiness are reported separately.
/// Unknown capabilities are not promises that a future provider supports them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CapabilityReport {
    pub(crate) receive_audio: CapabilityAvailability,
    pub(crate) receive_video: CapabilityAvailability,
    pub(crate) participant_metadata: CapabilityAvailability,
    pub(crate) publish_text: CapabilityAvailability,
    pub(crate) publish_link: CapabilityAvailability,
    /// These output modes are outside the approved MVP contract.
    pub(crate) publish_file: CapabilityAvailability,
    pub(crate) publish_audio: CapabilityAvailability,
    pub(crate) configured: bool,
    pub(crate) qualified: bool,
    pub(crate) blocker: Option<ReadinessBlocker>,
}

impl CapabilityReport {
    fn unconfigured() -> Self {
        Self {
            receive_audio: CapabilityAvailability::Unknown,
            receive_video: CapabilityAvailability::Unknown,
            participant_metadata: CapabilityAvailability::Unknown,
            publish_text: CapabilityAvailability::Unknown,
            publish_link: CapabilityAvailability::Unknown,
            publish_file: CapabilityAvailability::Unsupported,
            publish_audio: CapabilityAvailability::Unsupported,
            configured: false,
            qualified: false,
            blocker: Some(ReadinessBlocker::ProviderNotConfigured),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SessionBinding {
    pub(crate) source_account_id: String,
    pub(crate) account_generation: u64,
    pub(crate) occurrence_id: String,
    pub(crate) occurrence_generation: u64,
    pub(crate) session_id: String,
    pub(crate) session_generation: u64,
    pub(crate) source_id: String,
    pub(crate) source_generation: u64,
}

impl SessionBinding {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        validate_id(&self.source_account_id)?;
        validate_id(&self.occurrence_id)?;
        validate_id(&self.session_id)?;
        validate_id(&self.source_id)?;
        if self.account_generation == 0
            || self.occurrence_generation == 0
            || self.session_generation == 0
            || self.source_generation == 0
        {
            return Err(invalid_request(
                "account, occurrence, session, and source generations must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrepareSessionRequest {
    pub(crate) binding: SessionBinding,
    pub(crate) expires_at_ms: i64,
}

impl PrepareSessionRequest {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.binding.validate()?;
        if self.expires_at_ms <= 0 {
            return Err(invalid_request("session expiry must be positive"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedSession {
    pub(crate) binding: SessionBinding,
    pub(crate) transport_session_id: String,
    pub(crate) expires_at_ms: i64,
}

impl PreparedSession {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.binding.validate()?;
        validate_id(&self.transport_session_id)?;
        if self.expires_at_ms <= 0 {
            return Err(invalid_request("prepared session expiry must be positive"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportSessionState {
    Prepared,
    Joining,
    Joined,
    LeavePending,
    Left,
    JoinDenied,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderSession {
    pub(crate) prepared: PreparedSession,
    pub(crate) provider_session_ref: String,
    pub(crate) state: TransportSessionState,
}

impl ProviderSession {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.prepared.validate()?;
        validate_id(&self.provider_session_ref)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ProviderParticipant {
    pub(crate) provider_ref: String,
    pub(crate) display_label: Option<String>,
}

impl fmt::Debug for ProviderParticipant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderParticipant")
            .field("provider_ref_present", &!self.provider_ref.is_empty())
            .field("display_label_present", &self.display_label.is_some())
            .finish()
    }
}

impl ProviderParticipant {
    fn validate(&self) -> Result<(), MeetingAdapterError> {
        validate_id(&self.provider_ref)?;
        if let Some(label) = &self.display_label {
            if label.len() > MAX_LABEL_BYTES || label.chars().any(char::is_control) {
                return Err(invalid_request("participant label is invalid or oversized"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransportDestination {
    pub(crate) source_account_id: String,
    pub(crate) account_generation: u64,
    pub(crate) occurrence_id: String,
    pub(crate) occurrence_generation: u64,
    pub(crate) channel_id: String,
}

impl TransportDestination {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        validate_id(&self.source_account_id)?;
        validate_id(&self.occurrence_id)?;
        validate_id(&self.channel_id)?;
        if self.account_generation == 0 || self.occurrence_generation == 0 {
            return Err(invalid_request("destination generations must be positive"));
        }
        Ok(())
    }

    pub(crate) fn validate_for(&self, binding: &SessionBinding) -> Result<(), MeetingAdapterError> {
        self.validate()?;
        if self.source_account_id != binding.source_account_id
            || self.account_generation != binding.account_generation
            || self.occurrence_id != binding.occurrence_id
            || self.occurrence_generation != binding.occurrence_generation
        {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::ScopeMismatch,
                "destination account or occurrence does not match the session binding",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameKind {
    Audio,
    Video,
}

/// Bounded media payload after a trusted provider adapter has normalized it.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ProviderFrame {
    pub(crate) kind: FrameKind,
    pub(crate) codec: String,
    pub(crate) width: Option<u16>,
    pub(crate) height: Option<u16>,
    pub(crate) sample_rate_hz: Option<u32>,
    pub(crate) channels: Option<u8>,
    pub(crate) duration_ms: u32,
    pub(crate) payload: Vec<u8>,
}

impl fmt::Debug for ProviderFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderFrame")
            .field("kind", &self.kind)
            .field("codec", &self.codec)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("sample_rate_hz", &self.sample_rate_hz)
            .field("channels", &self.channels)
            .field("duration_ms", &self.duration_ms)
            .field("payload_bytes", &self.payload.len())
            .finish()
    }
}

impl ProviderFrame {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        if self.codec.is_empty()
            || self.codec.len() > MAX_CODEC_BYTES
            || !self.codec.is_ascii()
            || self.codec.chars().any(char::is_control)
            || self.duration_ms == 0
            || self.duration_ms > 2_000
            || self.payload.is_empty()
            || self.payload.len() > MAX_FRAME_BYTES
        {
            return Err(invalid_request("media frame is empty or exceeds bounds"));
        }

        match self.kind {
            FrameKind::Audio => {
                if self.payload.len() > MAX_AUDIO_FRAME_BYTES
                    || self.width.is_some()
                    || self.height.is_some()
                    || !matches!(self.sample_rate_hz, Some(8_000..=96_000))
                    || !matches!(self.channels, Some(1..=2))
                {
                    return Err(invalid_request(
                        "audio frame metadata is invalid or oversized",
                    ));
                }
            }
            FrameKind::Video => {
                let (Some(width), Some(height)) = (self.width, self.height) else {
                    return Err(invalid_request("video frame dimensions are required"));
                };
                if width == 0
                    || height == 0
                    || width > 4_096
                    || height > 4_096
                    || u32::from(width) * u32::from(height) > 16_777_216
                    || self.sample_rate_hz.is_some()
                    || self.channels.is_some()
                {
                    return Err(invalid_request("video frame dimensions are invalid"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NormalizedEventKind {
    ParticipantJoined(ProviderParticipant),
    ParticipantLeft(ProviderParticipant),
    MediaFrame(ProviderFrame),
}

/// Provider-neutral envelope. The two cursors have separate meanings: source
/// sequence is scoped to one source generation; event cursor is scoped to the
/// owning session's ordered stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UntrustedProviderEvent {
    pub(crate) binding: SessionBinding,
    pub(crate) source_sequence: u64,
    pub(crate) event_cursor: u64,
    pub(crate) occurred_at_ms: i64,
    pub(crate) participant: Option<ProviderParticipant>,
    pub(crate) kind: NormalizedEventKind,
}

impl UntrustedProviderEvent {
    fn validate_for(&self, expected: &SessionBinding) -> Result<(), MeetingAdapterError> {
        expected.validate()?;
        self.binding.validate()?;
        if &self.binding != expected {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::ScopeMismatch,
                "event account, occurrence, session, or generation does not match",
            ));
        }
        if self.source_sequence == 0 || self.event_cursor == 0 || self.occurred_at_ms < 0 {
            return Err(invalid_request(
                "event sequence, cursor, or timestamp is invalid",
            ));
        }
        if let Some(participant) = &self.participant {
            participant.validate()?;
        }
        match &self.kind {
            NormalizedEventKind::ParticipantJoined(participant)
            | NormalizedEventKind::ParticipantLeft(participant) => participant.validate(),
            NormalizedEventKind::MediaFrame(frame) => frame.validate(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationRecord {
    pub(crate) verifier_id: String,
    pub(crate) key_id: String,
    pub(crate) signature_sha256: String,
    pub(crate) verified_at_ms: i64,
}

/// Implementations are trusted ingress boundaries. They must verify the
/// provider signature, account/occurrence/session binding, timestamp window,
/// and signature replay before returning success. Sequence/cursor validation
/// is independently enforced here.
pub(crate) trait ProviderIngressVerifier: Send + Sync {
    fn verify_signature_scope_and_freshness(
        &self,
        expected: &SessionBinding,
        event: &UntrustedProviderEvent,
    ) -> Result<VerificationRecord, MeetingAdapterError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedIngress(VerificationRecord);

impl AuthenticatedIngress {
    pub(crate) fn verifier_id(&self) -> &str {
        &self.0.verifier_id
    }

    pub(crate) fn key_id(&self) -> &str {
        &self.0.key_id
    }

    pub(crate) fn signature_sha256(&self) -> &str {
        &self.0.signature_sha256
    }

    pub(crate) fn verified_at_ms(&self) -> i64 {
        self.0.verified_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IngressAuthentication {
    UnverifiedRawDto,
    Verified(AuthenticatedIngress),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedProviderEvent {
    event: UntrustedProviderEvent,
    authentication: IngressAuthentication,
}

impl NormalizedProviderEvent {
    pub(crate) fn event(&self) -> &UntrustedProviderEvent {
        &self.event
    }

    pub(crate) fn authentication(&self) -> &IngressAuthentication {
        &self.authentication
    }

    pub(crate) fn require_authenticated(&self) -> Result<(), MeetingAdapterError> {
        match self.authentication {
            IngressAuthentication::Verified(_) => Ok(()),
            IngressAuthentication::UnverifiedRawDto => Err(MeetingAdapterError::new(
                AdapterErrorCode::AuthenticationRequired,
                "raw provider DTO has not passed a trusted verifier",
            )),
        }
    }
}

/// Per-source ordered-ingress cursor. It rejects duplicate/replayed sequence
/// numbers and gaps without treating an event signature as authentic. Signature
/// replay checks remain exclusive to `ProviderIngressVerifier`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EventCursorTracker {
    binding: SessionBinding,
    last_source_sequence: u64,
    last_event_cursor: u64,
}

impl EventCursorTracker {
    pub(crate) fn new(
        binding: SessionBinding,
        last_source_sequence: u64,
        last_event_cursor: u64,
    ) -> Result<Self, MeetingAdapterError> {
        binding.validate()?;
        Ok(Self {
            binding,
            last_source_sequence,
            last_event_cursor,
        })
    }

    pub(crate) fn accept(
        &mut self,
        event: &NormalizedProviderEvent,
    ) -> Result<(), MeetingAdapterError> {
        event.event.validate_for(&self.binding)?;
        let incoming = &event.event;
        if incoming.source_sequence <= self.last_source_sequence
            || incoming.event_cursor <= self.last_event_cursor
        {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::ReplayDetected,
                "event sequence or cursor is stale or duplicated",
            ));
        }
        let Some(expected_sequence) = self.last_source_sequence.checked_add(1) else {
            return Err(invalid_request("source sequence space is exhausted"));
        };
        let Some(expected_cursor) = self.last_event_cursor.checked_add(1) else {
            return Err(invalid_request("event cursor space is exhausted"));
        };
        if incoming.source_sequence != expected_sequence || incoming.event_cursor != expected_cursor
        {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::CursorGap,
                "event sequence or session cursor has a gap",
            ));
        }
        self.last_source_sequence = incoming.source_sequence;
        self.last_event_cursor = incoming.event_cursor;
        Ok(())
    }
}

/// Normalize bounds and exact session scope without making an authentication
/// claim. Consumers that admit provider-controlled media must call
/// `require_authenticated` before accepting this event.
pub(crate) fn normalize_unverified_event(
    expected: &SessionBinding,
    event: UntrustedProviderEvent,
) -> Result<NormalizedProviderEvent, MeetingAdapterError> {
    event.validate_for(expected)?;
    Ok(NormalizedProviderEvent {
        event,
        authentication: IngressAuthentication::UnverifiedRawDto,
    })
}

/// Authenticate only through an explicitly supplied verifier. Signature replay
/// checking is the verifier's responsibility; this function additionally
/// validates the normalized envelope and exact session generations.
pub(crate) fn normalize_verified_event(
    expected: &SessionBinding,
    event: UntrustedProviderEvent,
    verifier: &dyn ProviderIngressVerifier,
) -> Result<NormalizedProviderEvent, MeetingAdapterError> {
    event.validate_for(expected)?;
    let record = verifier.verify_signature_scope_and_freshness(expected, &event)?;
    if validate_id(&record.verifier_id).is_err()
        || validate_id(&record.key_id).is_err()
        || !is_sha256_hex(&record.signature_sha256)
        || record.verified_at_ms <= 0
    {
        return Err(MeetingAdapterError::new(
            AdapterErrorCode::AuthenticationFailed,
            "trusted verifier returned invalid evidence",
        ));
    }
    Ok(NormalizedProviderEvent {
        event,
        authentication: IngressAuthentication::Verified(AuthenticatedIngress(record)),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeliveryState {
    Accepted,
    Delivered,
    Rejected,
    DeliveryUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceiptOrigin {
    Provider,
    Fixture,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransportReceipt {
    pub(crate) receipt_id: String,
    pub(crate) transport_session_id: String,
    pub(crate) destination: TransportDestination,
    pub(crate) payload_sha256: String,
    pub(crate) state: DeliveryState,
    pub(crate) origin: ReceiptOrigin,
    pub(crate) observed_at_ms: i64,
}

impl TransportReceipt {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        validate_id(&self.receipt_id)?;
        validate_id(&self.transport_session_id)?;
        self.destination.validate()?;
        if !is_sha256_hex(&self.payload_sha256) || self.observed_at_ms <= 0 {
            return Err(invalid_request("transport receipt evidence is invalid"));
        }
        Ok(())
    }

    pub(crate) fn validate_for(
        &self,
        session: &ProviderSession,
    ) -> Result<(), MeetingAdapterError> {
        self.validate()?;
        if self.transport_session_id != session.prepared.transport_session_id {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::ScopeMismatch,
                "receipt belongs to another transport session",
            ));
        }
        self.destination.validate_for(&session.prepared.binding)
    }

    pub(crate) fn validate_for_provider(
        &self,
        session: &ProviderSession,
    ) -> Result<(), MeetingAdapterError> {
        self.validate_for(session)?;
        if self.is_fixture() {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::AuthenticationFailed,
                "fixture receipt cannot be represented as provider evidence",
            ));
        }
        Ok(())
    }

    pub(crate) fn is_fixture(&self) -> bool {
        self.origin == ReceiptOrigin::Fixture
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct PublishTextRequest {
    pub(crate) destination: TransportDestination,
    pub(crate) text: String,
    pub(crate) payload_sha256: String,
}

impl fmt::Debug for PublishTextRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublishTextRequest")
            .field("destination", &self.destination)
            .field("text_bytes", &self.text.len())
            .field("payload_sha256", &self.payload_sha256)
            .finish()
    }
}

impl PublishTextRequest {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.destination.validate()?;
        if self.text.trim().is_empty()
            || self.text.len() > MAX_TEXT_BYTES
            || self.text.contains('\0')
            || !is_sha256_hex(&self.payload_sha256)
        {
            return Err(invalid_request(
                "text payload is empty, oversized, or has an invalid digest",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_for(&self, binding: &SessionBinding) -> Result<(), MeetingAdapterError> {
        self.destination.validate_for(binding)?;
        self.validate()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct PublishLinkRequest {
    pub(crate) destination: TransportDestination,
    pub(crate) label: String,
    pub(crate) url: String,
    pub(crate) payload_sha256: String,
}

impl fmt::Debug for PublishLinkRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublishLinkRequest")
            .field("destination", &self.destination)
            .field("label_bytes", &self.label.len())
            .field("url_bytes", &self.url.len())
            .field("payload_sha256", &self.payload_sha256)
            .finish()
    }
}

impl PublishLinkRequest {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.destination.validate()?;
        let authority = self
            .url
            .strip_prefix("https://")
            .map(|rest| rest.split(['/', '?', '#']).next().unwrap_or(""));
        if self.label.trim().is_empty()
            || self.label.len() > MAX_LABEL_BYTES
            || self.label.chars().any(char::is_control)
            || self.url.len() > MAX_LINK_BYTES
            || authority.is_none_or(|host| host.is_empty())
            || self.url.contains('@')
            || self.url.chars().any(char::is_whitespace)
            || !is_sha256_hex(&self.payload_sha256)
        {
            return Err(invalid_request(
                "link payload must be a bounded HTTPS URL and digest",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_for(&self, binding: &SessionBinding) -> Result<(), MeetingAdapterError> {
        self.destination.validate_for(binding)?;
        self.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReconcileRequest {
    pub(crate) binding: SessionBinding,
    pub(crate) transport_session_id: String,
    pub(crate) after_receipt_id: Option<String>,
    pub(crate) expected_cursor: u64,
}

impl ReconcileRequest {
    pub(crate) fn validate(&self) -> Result<(), MeetingAdapterError> {
        self.binding.validate()?;
        validate_id(&self.transport_session_id)?;
        if self.expected_cursor == 0 {
            return Err(invalid_request("reconciliation cursor must be positive"));
        }
        if let Some(receipt_id) = &self.after_receipt_id {
            validate_id(receipt_id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReconcileResult {
    pub(crate) state: TransportSessionState,
    pub(crate) next_cursor: u64,
    pub(crate) receipts: Vec<TransportReceipt>,
    pub(crate) unresolved_external_state: bool,
}

impl ReconcileResult {
    pub(crate) fn validate_for(
        &self,
        request: &ReconcileRequest,
    ) -> Result<(), MeetingAdapterError> {
        request.validate()?;
        if self.next_cursor < request.expected_cursor {
            return Err(MeetingAdapterError::new(
                AdapterErrorCode::CursorGap,
                "reconciliation result moved behind the requested cursor",
            ));
        }
        for receipt in &self.receipts {
            receipt.validate()?;
            if receipt.transport_session_id != request.transport_session_id {
                return Err(MeetingAdapterError::new(
                    AdapterErrorCode::ScopeMismatch,
                    "reconciliation returned a receipt from another transport session",
                ));
            }
            receipt.destination.validate_for(&request.binding)?;
        }
        Ok(())
    }
}

/// Single provider-neutral lifecycle and publication boundary. Implementations
/// must persist delivery intent before handoff; an uncertain handoff must be
/// returned as `DeliveryUnknown` and reconciled before any retry.
pub(crate) trait MeetingTransport: Send + Sync {
    fn probe_capabilities(&self) -> CapabilityReport;
    fn prepare(
        &self,
        request: &PrepareSessionRequest,
    ) -> Result<PreparedSession, MeetingAdapterError>;
    fn join(&self, session: &PreparedSession) -> Result<ProviderSession, MeetingAdapterError>;
    fn status(
        &self,
        session: &ProviderSession,
    ) -> Result<TransportSessionState, MeetingAdapterError>;
    fn leave(
        &self,
        session: &ProviderSession,
    ) -> Result<TransportSessionState, MeetingAdapterError>;
    fn publish_text(
        &self,
        session: &ProviderSession,
        request: &PublishTextRequest,
    ) -> Result<TransportReceipt, MeetingAdapterError>;
    fn publish_link(
        &self,
        session: &ProviderSession,
        request: &PublishLinkRequest,
    ) -> Result<TransportReceipt, MeetingAdapterError>;
    fn reconcile(&self, request: &ReconcileRequest)
        -> Result<ReconcileResult, MeetingAdapterError>;
}

/// Production default. No method in this adapter performs provider work.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct UnconfiguredMeetingTransport;

impl MeetingTransport for UnconfiguredMeetingTransport {
    fn probe_capabilities(&self) -> CapabilityReport {
        CapabilityReport::unconfigured()
    }

    fn prepare(
        &self,
        _request: &PrepareSessionRequest,
    ) -> Result<PreparedSession, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn join(&self, _session: &PreparedSession) -> Result<ProviderSession, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn status(
        &self,
        _session: &ProviderSession,
    ) -> Result<TransportSessionState, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn leave(
        &self,
        _session: &ProviderSession,
    ) -> Result<TransportSessionState, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn publish_text(
        &self,
        _session: &ProviderSession,
        _request: &PublishTextRequest,
    ) -> Result<TransportReceipt, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn publish_link(
        &self,
        _session: &ProviderSession,
        _request: &PublishLinkRequest,
    ) -> Result<TransportReceipt, MeetingAdapterError> {
        Err(provider_not_configured())
    }

    fn reconcile(
        &self,
        _request: &ReconcileRequest,
    ) -> Result<ReconcileResult, MeetingAdapterError> {
        Err(provider_not_configured())
    }
}

fn validate_id(value: &str) -> Result<(), MeetingAdapterError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(invalid_request("identifier is empty or exceeds bounds"));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> SessionBinding {
        SessionBinding {
            source_account_id: "account-1".into(),
            account_generation: 1,
            occurrence_id: "occurrence-1".into(),
            occurrence_generation: 1,
            session_id: "session-1".into(),
            session_generation: 1,
            source_id: "source-1".into(),
            source_generation: 1,
        }
    }

    fn destination() -> TransportDestination {
        TransportDestination {
            source_account_id: "account-1".into(),
            account_generation: 1,
            occurrence_id: "occurrence-1".into(),
            occurrence_generation: 1,
            channel_id: "chat-1".into(),
        }
    }

    fn event(sequence: u64, cursor: u64) -> UntrustedProviderEvent {
        UntrustedProviderEvent {
            binding: binding(),
            source_sequence: sequence,
            event_cursor: cursor,
            occurred_at_ms: 10,
            participant: None,
            kind: NormalizedEventKind::ParticipantJoined(ProviderParticipant {
                provider_ref: "participant-1".into(),
                display_label: Some("Guest".into()),
            }),
        }
    }

    fn prepared() -> PreparedSession {
        PreparedSession {
            binding: binding(),
            transport_session_id: "transport-1".into(),
            expires_at_ms: 1_000,
        }
    }

    fn session() -> ProviderSession {
        ProviderSession {
            prepared: prepared(),
            provider_session_ref: "provider-session-1".into(),
            state: TransportSessionState::Joined,
        }
    }

    fn hash() -> String {
        "a".repeat(64)
    }

    #[derive(Default)]
    struct FixtureTransport;

    impl MeetingTransport for FixtureTransport {
        fn probe_capabilities(&self) -> CapabilityReport {
            CapabilityReport {
                receive_audio: CapabilityAvailability::Supported,
                receive_video: CapabilityAvailability::Unsupported,
                participant_metadata: CapabilityAvailability::Supported,
                publish_text: CapabilityAvailability::Supported,
                publish_link: CapabilityAvailability::Supported,
                publish_file: CapabilityAvailability::Unsupported,
                publish_audio: CapabilityAvailability::Unsupported,
                configured: false,
                qualified: false,
                blocker: Some(ReadinessBlocker::ProviderNotConfigured),
            }
        }

        fn prepare(
            &self,
            request: &PrepareSessionRequest,
        ) -> Result<PreparedSession, MeetingAdapterError> {
            request.validate()?;
            Ok(prepared())
        }

        fn join(
            &self,
            prepared_session: &PreparedSession,
        ) -> Result<ProviderSession, MeetingAdapterError> {
            prepared_session.validate()?;
            Ok(session())
        }

        fn status(
            &self,
            session: &ProviderSession,
        ) -> Result<TransportSessionState, MeetingAdapterError> {
            session.validate()?;
            Ok(session.state)
        }

        fn leave(
            &self,
            session: &ProviderSession,
        ) -> Result<TransportSessionState, MeetingAdapterError> {
            session.validate()?;
            Ok(TransportSessionState::Left)
        }

        fn publish_text(
            &self,
            session: &ProviderSession,
            request: &PublishTextRequest,
        ) -> Result<TransportReceipt, MeetingAdapterError> {
            session.validate()?;
            request.validate_for(&session.prepared.binding)?;
            Ok(fixture_receipt(
                &session.prepared.transport_session_id,
                &request.destination,
                &request.payload_sha256,
            ))
        }

        fn publish_link(
            &self,
            session: &ProviderSession,
            request: &PublishLinkRequest,
        ) -> Result<TransportReceipt, MeetingAdapterError> {
            session.validate()?;
            request.validate_for(&session.prepared.binding)?;
            Ok(fixture_receipt(
                &session.prepared.transport_session_id,
                &request.destination,
                &request.payload_sha256,
            ))
        }

        fn reconcile(
            &self,
            request: &ReconcileRequest,
        ) -> Result<ReconcileResult, MeetingAdapterError> {
            request.validate()?;
            let result = ReconcileResult {
                state: TransportSessionState::Joined,
                next_cursor: request.expected_cursor,
                receipts: vec![],
                unresolved_external_state: false,
            };
            result.validate_for(request)?;
            Ok(result)
        }
    }

    fn fixture_receipt(
        session_id: &str,
        destination: &TransportDestination,
        payload_sha256: &str,
    ) -> TransportReceipt {
        TransportReceipt {
            receipt_id: "fixture-receipt-1".into(),
            transport_session_id: session_id.into(),
            destination: destination.clone(),
            payload_sha256: payload_sha256.into(),
            state: DeliveryState::Accepted,
            origin: ReceiptOrigin::Fixture,
            observed_at_ms: 10,
        }
    }

    #[derive(Default)]
    struct FixtureVerifier {
        replay: bool,
    }

    impl ProviderIngressVerifier for FixtureVerifier {
        fn verify_signature_scope_and_freshness(
            &self,
            _expected: &SessionBinding,
            _event: &UntrustedProviderEvent,
        ) -> Result<VerificationRecord, MeetingAdapterError> {
            if self.replay {
                return Err(MeetingAdapterError::new(
                    AdapterErrorCode::ReplayDetected,
                    "fixture verifier detected a replay",
                ));
            }
            Ok(VerificationRecord {
                verifier_id: "fixture-verifier".into(),
                key_id: "fixture-key".into(),
                signature_sha256: hash(),
                verified_at_ms: 10,
            })
        }
    }

    #[test]
    fn unconfigured_transport_never_joins_or_publishes() {
        let transport = UnconfiguredMeetingTransport;
        let report = transport.probe_capabilities();
        assert!(!report.configured);
        assert!(!report.qualified);
        assert_eq!(report.publish_file, CapabilityAvailability::Unsupported);
        assert_eq!(
            report.blocker,
            Some(ReadinessBlocker::ProviderNotConfigured)
        );

        let request = PrepareSessionRequest {
            binding: binding(),
            expires_at_ms: 100,
        };
        assert_eq!(
            transport.prepare(&request).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
        assert_eq!(
            transport.join(&prepared()).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
        assert_eq!(
            transport.status(&session()).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
        assert_eq!(
            transport.leave(&session()).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );

        let text = PublishTextRequest {
            destination: destination(),
            text: "hello".into(),
            payload_sha256: hash(),
        };
        assert_eq!(
            transport.publish_text(&session(), &text).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
        let link = PublishLinkRequest {
            destination: destination(),
            label: "source".into(),
            url: "https://example.test/source".into(),
            payload_sha256: hash(),
        };
        assert_eq!(
            transport.publish_link(&session(), &link).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
        let reconcile = ReconcileRequest {
            binding: binding(),
            transport_session_id: "transport-1".into(),
            after_receipt_id: None,
            expected_cursor: 1,
        };
        assert_eq!(
            transport.reconcile(&reconcile).unwrap_err().code,
            AdapterErrorCode::ProviderNotConfigured
        );
    }

    #[test]
    fn raw_dto_is_normalized_but_never_claimed_authenticated() {
        let normalized = normalize_unverified_event(&binding(), event(1, 1)).unwrap();
        assert_eq!(
            normalized.authentication(),
            &IngressAuthentication::UnverifiedRawDto
        );
        assert_eq!(
            normalized.require_authenticated().unwrap_err().code,
            AdapterErrorCode::AuthenticationRequired
        );
    }

    #[test]
    fn cursor_tracker_rejects_duplicate_and_gapped_sequences() {
        let mut tracker = EventCursorTracker::new(binding(), 0, 0).unwrap();
        let first = normalize_unverified_event(&binding(), event(1, 1)).unwrap();
        tracker.accept(&first).unwrap();
        assert_eq!(
            tracker.accept(&first).unwrap_err().code,
            AdapterErrorCode::ReplayDetected
        );

        let gap = normalize_unverified_event(&binding(), event(3, 3)).unwrap();
        assert_eq!(
            tracker.accept(&gap).unwrap_err().code,
            AdapterErrorCode::CursorGap
        );
    }

    #[test]
    fn trusted_verifier_is_required_for_authenticated_ingress_and_replay_rejection() {
        let verified =
            normalize_verified_event(&binding(), event(1, 1), &FixtureVerifier::default()).unwrap();
        verified.require_authenticated().unwrap();
        let IngressAuthentication::Verified(evidence) = verified.authentication() else {
            panic!("expected verifier evidence");
        };
        assert_eq!(evidence.verifier_id(), "fixture-verifier");
        assert_eq!(evidence.key_id(), "fixture-key");
        assert_eq!(evidence.signature_sha256(), hash());
        assert_eq!(evidence.verified_at_ms(), 10);

        let replay = FixtureVerifier { replay: true };
        assert_eq!(
            normalize_verified_event(&binding(), event(1, 1), &replay)
                .unwrap_err()
                .code,
            AdapterErrorCode::ReplayDetected,
        );
    }

    #[test]
    fn event_bounds_and_exact_scope_fail_closed() {
        let mut mismatched = event(1, 1);
        mismatched.binding.source_generation = 2;
        assert_eq!(
            normalize_unverified_event(&binding(), mismatched)
                .unwrap_err()
                .code,
            AdapterErrorCode::ScopeMismatch,
        );

        assert_eq!(
            normalize_unverified_event(&binding(), event(0, 1))
                .unwrap_err()
                .code,
            AdapterErrorCode::InvalidRequest,
        );

        let mut oversized = event(1, 1);
        oversized.kind = NormalizedEventKind::MediaFrame(ProviderFrame {
            kind: FrameKind::Audio,
            codec: "pcm16".into(),
            width: None,
            height: None,
            sample_rate_hz: Some(16_000),
            channels: Some(1),
            duration_ms: 2_000,
            payload: vec![0; MAX_AUDIO_FRAME_BYTES + 1],
        });
        assert_eq!(
            normalize_unverified_event(&binding(), oversized)
                .unwrap_err()
                .code,
            AdapterErrorCode::InvalidRequest,
        );
    }

    #[test]
    fn fixture_transport_is_test_only_and_marks_receipts_as_fixture() {
        let fixture = FixtureTransport;
        let prepared = fixture
            .prepare(&PrepareSessionRequest {
                binding: binding(),
                expires_at_ms: 100,
            })
            .unwrap();
        let joined = fixture.join(&prepared).unwrap();
        let request = PublishTextRequest {
            destination: destination(),
            text: "hello".into(),
            payload_sha256: hash(),
        };
        let receipt = fixture.publish_text(&joined, &request).unwrap();
        receipt.validate().unwrap();
        receipt.validate_for(&joined).unwrap();
        assert_eq!(
            receipt.validate_for_provider(&joined).unwrap_err().code,
            AdapterErrorCode::AuthenticationFailed,
        );
        assert_eq!(receipt.origin, ReceiptOrigin::Fixture);
    }

    #[test]
    fn publish_requests_reject_unbounded_or_unsafe_values() {
        let bad_text = PublishTextRequest {
            destination: destination(),
            text: "x".repeat(MAX_TEXT_BYTES + 1),
            payload_sha256: hash(),
        };
        assert_eq!(
            bad_text.validate().unwrap_err().code,
            AdapterErrorCode::InvalidRequest
        );

        let bad_link = PublishLinkRequest {
            destination: destination(),
            label: "source".into(),
            url: "http://example.test/source".into(),
            payload_sha256: hash(),
        };
        assert_eq!(
            bad_link.validate().unwrap_err().code,
            AdapterErrorCode::InvalidRequest
        );
    }

    #[test]
    fn publish_destination_is_bound_to_account_and_occurrence_generations() {
        let fixture = FixtureTransport;
        let joined = fixture.join(&prepared()).unwrap();
        let mut request = PublishTextRequest {
            destination: destination(),
            text: "hello".into(),
            payload_sha256: hash(),
        };
        request.destination.occurrence_generation = 2;
        assert_eq!(
            fixture.publish_text(&joined, &request).unwrap_err().code,
            AdapterErrorCode::ScopeMismatch,
        );
    }
}
