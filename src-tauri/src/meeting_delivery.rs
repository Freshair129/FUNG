//! Provider-neutral approval and delivery outbox transitions.
//!
//! This module contains no transport callback. `prepare_handoff` returns a
//! proposed durable state plus a descriptor; the integration owner must commit
//! the state and destination lease before it invokes any qualified adapter.
//! Payload text is accepted only transiently to compute its digest. The outbox
//! retains an encrypted asset reference and hashes, never plaintext content.

#![allow(dead_code)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use crate::meeting_agent::{AgentMode, TriggerKind};

const MAX_PAYLOAD_BYTES: usize = 100_000;
const MAX_EVIDENCE_ITEMS: usize = 32;
const MAX_APPROVAL_LIFETIME_MS: u64 = 15 * 60 * 1000;
const MAX_DELIVERY_LIFETIME_MS: u64 = 4 * 60 * 60 * 1000;
const MAX_DESTINATION_LEASE_MS: u64 = 60 * 1000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryState {
    Draft,
    AwaitingApproval,
    ApprovedLocalOnly,
    Ready,
    Sending,
    ProviderAccepted,
    Delivered,
    Blocked,
    Expired,
    Cancelled,
    Failed,
    DeliveryUnknown,
    Superseded,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PayloadKind {
    Text,
    Link,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChannelKind {
    MeetingChat,
    MeetingThread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterReadiness {
    Unavailable,
    AvailableUnqualified,
    Qualified,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalScope {
    LocalPreviewOnly,
    ExternalPublish,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct EncryptedPayloadBinding {
    /// Opaque asset ID understood only by the encrypted custody owner.
    pub(crate) encrypted_asset_ref: String,
    pub(crate) encrypted_asset_ref_hash: String,
    pub(crate) ciphertext_sha256: String,
    pub(crate) payload_sha256: String,
    pub(crate) kind: PayloadKind,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct DestinationBinding {
    pub(crate) provider_ref_hash: String,
    pub(crate) account_ref_hash: String,
    pub(crate) occurrence_ref_hash: String,
    pub(crate) session_ref_hash: String,
    pub(crate) channel_ref_hash: String,
    pub(crate) thread_ref_hash: Option<String>,
    pub(crate) channel_kind: ChannelKind,
    pub(crate) audience_policy_revision: u64,
    pub(crate) audience_binding_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct DeliveryEvidenceBinding {
    pub(crate) evidence_ref_hash: String,
    pub(crate) source_ref_hash: String,
    pub(crate) source_version_hash: String,
    pub(crate) source_revision: u64,
    pub(crate) read_grant_ref_hash: String,
    pub(crate) read_grant_revision: u64,
    pub(crate) share_grant_ref_hash: String,
    pub(crate) share_grant_revision: u64,
    pub(crate) citation_hash: String,
    pub(crate) captured_at_ms: u64,
    pub(crate) max_age_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PublicationGrantBinding {
    pub(crate) grant_ref_hash: String,
    pub(crate) grant_revision: u64,
    pub(crate) session_ref_hash: String,
    pub(crate) policy_revision: u64,
    pub(crate) mode: AgentMode,
    pub(crate) trigger_kind: TriggerKind,
    pub(crate) topic_ref_hash: String,
    pub(crate) allowed_topic_ref_hashes: BTreeSet<String>,
    pub(crate) allowed_payload_kinds: BTreeSet<String>,
    pub(crate) allowed_audience_class_ref_hashes: BTreeSet<String>,
    pub(crate) allowed_link_hosts: BTreeSet<String>,
    pub(crate) max_payload_bytes: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) currency_code: String,
    pub(crate) cost_cap_micros: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ApprovalSnapshot {
    pub(crate) approver_ref_hash: String,
    pub(crate) binding_hash: String,
    pub(crate) cost_quote_hash: Option<String>,
    pub(crate) approved_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) scope: ApprovalScope,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct DeliveryAttemptSnapshot {
    pub(crate) attempt_id: Uuid,
    pub(crate) attempt_number: u32,
    pub(crate) idempotency_key_hash: String,
    pub(crate) binding_hash: String,
    pub(crate) started_at_ms: u64,
    pub(crate) lease_ref_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct DeliveryReceiptSnapshot {
    pub(crate) attempt_id: Uuid,
    pub(crate) proof_hash: String,
    pub(crate) provider_message_ref_hash: Option<String>,
    pub(crate) observed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct MeetingDeliveryIntent {
    intent_id: Uuid,
    revision: u64,
    idempotency_key_hash: String,
    session_ref_hash: String,
    trigger_ref_hash: String,
    answer_revision: u64,
    created_at_ms: u64,
    expires_at_ms: u64,
    state: DeliveryState,
    payload: EncryptedPayloadBinding,
    destination: DestinationBinding,
    evidence: Vec<DeliveryEvidenceBinding>,
    grant: PublicationGrantBinding,
    payload_hash: String,
    destination_hash: String,
    evidence_hash: String,
    grant_hash: String,
    binding_hash: String,
    approval: Option<ApprovalSnapshot>,
    attempt_count: u32,
    active_attempt: Option<DeliveryAttemptSnapshot>,
    last_receipt: Option<DeliveryReceiptSnapshot>,
    last_error_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CurrentEvidenceAuthority {
    pub(crate) evidence_ref_hash: String,
    pub(crate) source_ref_hash: Option<String>,
    pub(crate) source_version_hash: Option<String>,
    pub(crate) source_revision: Option<u64>,
    pub(crate) read_grant_ref_hash: Option<String>,
    pub(crate) read_grant_revision: Option<u64>,
    pub(crate) share_grant_ref_hash: Option<String>,
    pub(crate) share_grant_revision: Option<u64>,
    pub(crate) can_read: Option<bool>,
    pub(crate) can_share: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AudienceSnapshot {
    pub(crate) audience_binding_hash: String,
    pub(crate) audience_policy_revision: u64,
    pub(crate) audience_class_ref_hash: String,
    pub(crate) is_known: Option<bool>,
    pub(crate) has_unknown_members: Option<bool>,
    pub(crate) can_receive: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeliveryCostQuote {
    pub(crate) quote_ref_hash: String,
    pub(crate) currency_code: String,
    /// Bounded upper estimate, in one-millionth currency units.
    pub(crate) upper_bound_micros: u64,
    pub(crate) valid_until_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeliveryCostBudget {
    pub(crate) currency_code: String,
    pub(crate) session_spent_micros: u64,
    pub(crate) session_cap_micros: u64,
    pub(crate) ledger_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct DeliveryCostReservation {
    pub(crate) reservation_ref_hash: String,
    pub(crate) intent_id: Uuid,
    pub(crate) session_ref_hash: String,
    pub(crate) binding_hash: String,
    pub(crate) cost_quote_hash: String,
    pub(crate) reserved_cost_micros: u64,
    pub(crate) ledger_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DestinationLeaseSnapshot {
    pub(crate) lease_ref_hash: String,
    pub(crate) destination_hash: String,
    pub(crate) intent_revision: u64,
    pub(crate) acquired_at_ms: u64,
    pub(crate) expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeliveryChecks {
    pub(crate) now_ms: u64,
    pub(crate) session_ref_hash: Option<String>,
    pub(crate) session_active: Option<bool>,
    pub(crate) grant_ref_hash: Option<String>,
    pub(crate) grant_revision: Option<u64>,
    pub(crate) grant_active: Option<bool>,
    pub(crate) grant_expires_at_ms: Option<u64>,
    pub(crate) policy_revision: Option<u64>,
    pub(crate) current_mode: Option<AgentMode>,
    pub(crate) current_trigger_kind: Option<TriggerKind>,
    pub(crate) current_topic_ref_hash: Option<String>,
    pub(crate) current_topic_allowed: Option<bool>,
    pub(crate) payload_kind_allowed: Option<bool>,
    pub(crate) audience: Option<AudienceSnapshot>,
    pub(crate) evidence_authority: Vec<CurrentEvidenceAuthority>,
    pub(crate) cost_quote: Option<DeliveryCostQuote>,
    pub(crate) cost_budget: Option<DeliveryCostBudget>,
    /// Must come from the same durable transaction that reserves session cost.
    pub(crate) cost_reservation: Option<DeliveryCostReservation>,
    pub(crate) adapter_readiness: AdapterReadiness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryBlockReason {
    InvalidIntent,
    InvalidHash,
    Expired,
    SessionUnknown,
    SessionInactive,
    GrantUnknown,
    GrantInactive,
    GrantRevisionMismatch,
    PolicyRevisionMismatch,
    ModeUnavailable,
    ModeNotAllowed,
    TriggerNotAllowed,
    TopicUnknown,
    TopicDenied,
    PayloadDenied,
    AudienceUnknown,
    AudienceChanged,
    AudienceDenied,
    EvidenceUnknown,
    EvidenceAclUnknown,
    EvidenceAclDenied,
    SourceRevisionMismatch,
    EvidenceStale,
    CostQuoteMissing,
    CostQuoteStale,
    CostQuoteInvalid,
    CostCapExceeded,
    AdapterUnavailable,
    AdapterUnqualified,
    ApprovalRequired,
    ApprovalInvalidated,
    InvalidTransition,
    LeaseInvalid,
    AttemptMismatch,
    ReceiptInvalid,
    UncertainIsTerminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedHandoff {
    /// Persist this complete next state before invoking any adapter.
    pub(crate) next_intent: MeetingDeliveryIntent,
    /// Contains only opaque references and binding hashes, not payload text.
    pub(crate) descriptor: HandoffDescriptor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HandoffDescriptor {
    pub(crate) intent_id: Uuid,
    pub(crate) attempt_id: Uuid,
    pub(crate) encrypted_payload_asset_ref: String,
    pub(crate) payload_asset_ref_hash: String,
    pub(crate) payload_sha256: String,
    pub(crate) destination_hash: String,
    pub(crate) evidence_hash: String,
    pub(crate) grant_hash: String,
    pub(crate) idempotency_key_hash: String,
    pub(crate) binding_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HandoffOutcome {
    DefinitelyNotSent {
        error_hash: String,
    },
    PossiblySent {
        error_hash: String,
    },
    ProviderAccepted {
        proof_hash: String,
        provider_message_ref_hash: String,
    },
    Delivered {
        proof_hash: String,
        provider_message_ref_hash: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReconciliationOutcome {
    ProviderAccepted {
        proof_hash: String,
        provider_message_ref_hash: String,
    },
    Delivered {
        proof_hash: String,
        provider_message_ref_hash: String,
    },
    ProvenNotDelivered {
        proof_hash: String,
    },
    StillUnknown {
        proof_hash: String,
    },
}

impl MeetingDeliveryIntent {
    pub(crate) fn intent_id(&self) -> Uuid {
        self.intent_id
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn state(&self) -> DeliveryState {
        self.state
    }

    pub(crate) fn session_ref_hash(&self) -> &str {
        &self.session_ref_hash
    }

    pub(crate) fn destination(&self) -> &DestinationBinding {
        &self.destination
    }

    pub(crate) fn evidence(&self) -> &[DeliveryEvidenceBinding] {
        &self.evidence
    }

    pub(crate) fn grant(&self) -> &PublicationGrantBinding {
        &self.grant
    }

    pub(crate) fn payload(&self) -> &EncryptedPayloadBinding {
        &self.payload
    }

    pub(crate) fn approval(&self) -> Option<&ApprovalSnapshot> {
        self.approval.as_ref()
    }

    pub(crate) fn binding_hash(&self) -> &str {
        &self.binding_hash
    }

    pub(crate) fn destination_hash(&self) -> &str {
        &self.destination_hash
    }

    pub(crate) fn payload_hash(&self) -> &str {
        &self.payload_hash
    }

    pub(crate) fn evidence_hash(&self) -> &str {
        &self.evidence_hash
    }

    pub(crate) fn grant_hash(&self) -> &str {
        &self.grant_hash
    }

    pub(crate) fn idempotency_key_hash(&self) -> &str {
        &self.idempotency_key_hash
    }

    pub(crate) fn payload_asset_ref_hash(&self) -> &str {
        &self.payload.encrypted_asset_ref_hash
    }

    pub(crate) fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    pub(crate) fn last_receipt(&self) -> Option<&DeliveryReceiptSnapshot> {
        self.last_receipt.as_ref()
    }

    /// Builds an immutable encrypted-payload reference from a transient exact
    /// preview. The caller encrypts and stores `payload_bytes` before saving
    /// this intent; this type never retains those bytes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_draft(
        idempotency_key: &[u8],
        session_ref_hash: String,
        trigger_ref_hash: String,
        answer_revision: u64,
        created_at_ms: u64,
        expires_at_ms: u64,
        encrypted_asset_ref: String,
        ciphertext_sha256: String,
        payload_kind: PayloadKind,
        payload_bytes: &[u8],
        destination: DestinationBinding,
        mut evidence: Vec<DeliveryEvidenceBinding>,
        grant: PublicationGrantBinding,
    ) -> Result<Self, DeliveryBlockReason> {
        if payload_bytes.is_empty()
            || payload_bytes.len() > MAX_PAYLOAD_BYTES
            || payload_bytes.len() as u64 > grant.max_payload_bytes
        {
            return Err(DeliveryBlockReason::InvalidIntent);
        }
        validate_payload_content(payload_kind, payload_bytes, &grant)?;
        let payload = EncryptedPayloadBinding {
            encrypted_asset_ref_hash: sha256_hex(encrypted_asset_ref.as_bytes()),
            encrypted_asset_ref,
            ciphertext_sha256,
            payload_sha256: sha256_hex(payload_bytes),
            kind: payload_kind,
        };
        validate_new_snapshot(
            idempotency_key,
            &session_ref_hash,
            &trigger_ref_hash,
            answer_revision,
            created_at_ms,
            expires_at_ms,
            &payload,
            &destination,
            &evidence,
            &grant,
        )?;
        evidence.sort_by(|left, right| left.evidence_ref_hash.cmp(&right.evidence_ref_hash));
        let idempotency_key_hash = sha256_hex(idempotency_key);
        let (payload_hash, destination_hash, evidence_hash, grant_hash, binding_hash) =
            binding_hashes(&payload, &destination, &evidence, &grant)?;
        Ok(Self {
            intent_id: Uuid::new_v4(),
            revision: 1,
            idempotency_key_hash,
            session_ref_hash,
            trigger_ref_hash,
            answer_revision,
            created_at_ms,
            expires_at_ms,
            state: DeliveryState::Draft,
            payload,
            destination,
            evidence,
            grant,
            payload_hash,
            destination_hash,
            evidence_hash,
            grant_hash,
            binding_hash,
            approval: None,
            attempt_count: 0,
            active_attempt: None,
            last_receipt: None,
            last_error_hash: None,
        })
    }

    pub(crate) fn request_approval(&self) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if self.state != DeliveryState::Draft {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        let mut next = self.clone();
        next.state = DeliveryState::AwaitingApproval;
        next.revision = next.revision.saturating_add(1);
        Ok(next)
    }

    /// Records exact-content human approval. With no qualified adapter the
    /// state is `ApprovedLocalOnly`; it cannot be handed to a provider.
    pub(crate) fn approve_manual(
        &self,
        approver_ref_hash: String,
        checks: &DeliveryChecks,
        scope: ApprovalScope,
    ) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if self.state != DeliveryState::AwaitingApproval {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        if !is_sha256(&approver_ref_hash) {
            return Err(DeliveryBlockReason::InvalidHash);
        }
        validate_current(self, checks, scope == ApprovalScope::ExternalPublish, false)?;
        if scope == ApprovalScope::ExternalPublish {
            match checks.adapter_readiness {
                AdapterReadiness::Unavailable => {
                    return Err(DeliveryBlockReason::AdapterUnavailable)
                }
                AdapterReadiness::AvailableUnqualified => {
                    return Err(DeliveryBlockReason::AdapterUnqualified)
                }
                AdapterReadiness::Qualified => {}
            }
        }
        let approval_expires_at = self
            .expires_at_ms
            .min(self.grant.expires_at_ms)
            .min(checks.now_ms.saturating_add(MAX_APPROVAL_LIFETIME_MS));
        if approval_expires_at <= checks.now_ms {
            return Err(DeliveryBlockReason::Expired);
        }
        let mut next = self.clone();
        let approved_cost_hash = if scope == ApprovalScope::ExternalPublish {
            Some(cost_quote_hash(
                checks
                    .cost_quote
                    .as_ref()
                    .ok_or(DeliveryBlockReason::CostQuoteMissing)?,
            )?)
        } else {
            None
        };
        next.approval = Some(ApprovalSnapshot {
            approver_ref_hash,
            binding_hash: self.binding_hash.clone(),
            cost_quote_hash: approved_cost_hash,
            approved_at_ms: checks.now_ms,
            expires_at_ms: approval_expires_at,
            scope,
        });
        next.state = match scope {
            ApprovalScope::LocalPreviewOnly => DeliveryState::ApprovedLocalOnly,
            ApprovalScope::ExternalPublish => DeliveryState::Ready,
        };
        next.revision = next.revision.saturating_add(1);
        Ok(next)
    }

    /// Rebinds an unsubmitted intent after an edit and invalidates any prior
    /// approval. In-flight or terminal intents are immutable; corrections use
    /// a new intent linked by the caller.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn revise_unsubmitted(
        &self,
        payload_kind: PayloadKind,
        encrypted_asset_ref: String,
        ciphertext_sha256: String,
        payload_bytes: &[u8],
        destination: DestinationBinding,
        mut evidence: Vec<DeliveryEvidenceBinding>,
        grant: PublicationGrantBinding,
        now_ms: u64,
    ) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if !matches!(
            self.state,
            DeliveryState::Draft
                | DeliveryState::AwaitingApproval
                | DeliveryState::ApprovedLocalOnly
                | DeliveryState::Ready
        ) || self.attempt_count != 0
        {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        if now_ms >= self.expires_at_ms
            || payload_bytes.is_empty()
            || payload_bytes.len() > MAX_PAYLOAD_BYTES
            || payload_bytes.len() as u64 > grant.max_payload_bytes
        {
            return Err(DeliveryBlockReason::Expired);
        }
        validate_payload_content(payload_kind, payload_bytes, &grant)?;
        let payload = EncryptedPayloadBinding {
            encrypted_asset_ref_hash: sha256_hex(encrypted_asset_ref.as_bytes()),
            encrypted_asset_ref,
            ciphertext_sha256,
            payload_sha256: sha256_hex(payload_bytes),
            kind: payload_kind,
        };
        let fresh_idempotency = Uuid::new_v4().as_bytes().to_vec();
        validate_new_snapshot(
            &fresh_idempotency,
            &self.session_ref_hash,
            &self.trigger_ref_hash,
            self.answer_revision,
            self.created_at_ms,
            self.expires_at_ms,
            &payload,
            &destination,
            &evidence,
            &grant,
        )?;
        evidence.sort_by(|left, right| left.evidence_ref_hash.cmp(&right.evidence_ref_hash));
        let (payload_hash, destination_hash, evidence_hash, grant_hash, binding_hash) =
            binding_hashes(&payload, &destination, &evidence, &grant)?;
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        next.idempotency_key_hash = sha256_hex(&fresh_idempotency);
        next.payload = payload;
        next.destination = destination;
        next.evidence = evidence;
        next.grant = grant;
        next.payload_hash = payload_hash;
        next.destination_hash = destination_hash;
        next.evidence_hash = evidence_hash;
        next.grant_hash = grant_hash;
        next.binding_hash = binding_hash;
        next.approval = None;
        next.state = DeliveryState::AwaitingApproval;
        next.last_error_hash = None;
        Ok(next)
    }

    /// Returns a proposed `sending` transition. The caller must commit
    /// `next_intent` and the checked destination lease before any adapter call.
    /// No network operation is performed here.
    pub(crate) fn prepare_handoff(
        &self,
        checks: &DeliveryChecks,
        lease: &DestinationLeaseSnapshot,
        expected_revision: u64,
    ) -> Result<PreparedHandoff, DeliveryBlockReason> {
        if self.state != DeliveryState::Ready || expected_revision != self.revision {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        validate_approval(self, checks, ApprovalScope::ExternalPublish)?;
        validate_current(self, checks, true, true)?;
        match checks.adapter_readiness {
            AdapterReadiness::Unavailable => return Err(DeliveryBlockReason::AdapterUnavailable),
            AdapterReadiness::AvailableUnqualified => {
                return Err(DeliveryBlockReason::AdapterUnqualified)
            }
            AdapterReadiness::Qualified => {}
        }
        if lease.destination_hash != self.destination_hash
            || lease.intent_revision != self.revision
            || !is_sha256(&lease.lease_ref_hash)
            || lease.acquired_at_ms > checks.now_ms
            || lease.expires_at_ms <= checks.now_ms
            || lease.expires_at_ms.saturating_sub(lease.acquired_at_ms) > MAX_DESTINATION_LEASE_MS
        {
            return Err(DeliveryBlockReason::LeaseInvalid);
        }
        let attempt_id = Uuid::new_v4();
        let attempt = DeliveryAttemptSnapshot {
            attempt_id,
            attempt_number: self.attempt_count.saturating_add(1),
            idempotency_key_hash: self.idempotency_key_hash.clone(),
            binding_hash: self.binding_hash.clone(),
            started_at_ms: checks.now_ms,
            lease_ref_hash: lease.lease_ref_hash.clone(),
        };
        let mut next = self.clone();
        next.state = DeliveryState::Sending;
        next.revision = next.revision.saturating_add(1);
        next.attempt_count = attempt.attempt_number;
        next.active_attempt = Some(attempt.clone());
        Ok(PreparedHandoff {
            descriptor: HandoffDescriptor {
                intent_id: self.intent_id,
                attempt_id,
                encrypted_payload_asset_ref: self.payload.encrypted_asset_ref.clone(),
                payload_asset_ref_hash: self.payload.encrypted_asset_ref_hash.clone(),
                payload_sha256: self.payload_hash.clone(),
                destination_hash: self.destination_hash.clone(),
                evidence_hash: self.evidence_hash.clone(),
                grant_hash: self.grant_hash.clone(),
                idempotency_key_hash: self.idempotency_key_hash.clone(),
                binding_hash: self.binding_hash.clone(),
            },
            next_intent: next,
        })
    }

    pub(crate) fn record_handoff_outcome(
        &self,
        attempt_id: Uuid,
        outcome: HandoffOutcome,
        observed_at_ms: u64,
    ) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if self.state != DeliveryState::Sending {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        let attempt = self
            .active_attempt
            .as_ref()
            .ok_or(DeliveryBlockReason::AttemptMismatch)?;
        if attempt.attempt_id != attempt_id
            || attempt.binding_hash != self.binding_hash
            || observed_at_ms < attempt.started_at_ms
        {
            return Err(DeliveryBlockReason::AttemptMismatch);
        }
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        match outcome {
            HandoffOutcome::DefinitelyNotSent { error_hash } => {
                if !is_sha256(&error_hash) {
                    return Err(DeliveryBlockReason::InvalidHash);
                }
                next.state = DeliveryState::Failed;
                next.active_attempt = None;
                next.last_error_hash = Some(error_hash);
            }
            HandoffOutcome::PossiblySent { error_hash } => {
                if !is_sha256(&error_hash) {
                    return Err(DeliveryBlockReason::InvalidHash);
                }
                next.state = DeliveryState::DeliveryUnknown;
                // Retain this reference for reconciliation. Unknown is
                // terminal for automatic retries, never a fresh attempt.
                next.active_attempt = Some(attempt.clone());
                next.last_error_hash = Some(error_hash);
            }
            HandoffOutcome::ProviderAccepted {
                proof_hash,
                provider_message_ref_hash,
            } => {
                validate_receipt(&proof_hash, &provider_message_ref_hash)?;
                next.state = DeliveryState::ProviderAccepted;
                next.active_attempt = None;
                next.last_receipt = Some(DeliveryReceiptSnapshot {
                    attempt_id,
                    proof_hash,
                    provider_message_ref_hash: Some(provider_message_ref_hash),
                    observed_at_ms,
                });
            }
            HandoffOutcome::Delivered {
                proof_hash,
                provider_message_ref_hash,
            } => {
                validate_receipt(&proof_hash, &provider_message_ref_hash)?;
                next.state = DeliveryState::Delivered;
                next.active_attempt = None;
                next.last_receipt = Some(DeliveryReceiptSnapshot {
                    attempt_id,
                    proof_hash,
                    provider_message_ref_hash: Some(provider_message_ref_hash),
                    observed_at_ms,
                });
            }
        }
        Ok(next)
    }

    /// Reconciliation may confirm an unknown handoff or prove it was not
    /// delivered. It never returns an intent to `ready`; retry needs a newly
    /// authorized intent and exact approval.
    pub(crate) fn reconcile_unknown(
        &self,
        outcome: ReconciliationOutcome,
        observed_at_ms: u64,
    ) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if self.state != DeliveryState::DeliveryUnknown {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        let attempt = self
            .active_attempt
            .as_ref()
            .ok_or(DeliveryBlockReason::AttemptMismatch)?;
        if observed_at_ms < attempt.started_at_ms {
            return Err(DeliveryBlockReason::ReceiptInvalid);
        }
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        match outcome {
            ReconciliationOutcome::ProviderAccepted {
                proof_hash,
                provider_message_ref_hash,
            } => {
                validate_receipt(&proof_hash, &provider_message_ref_hash)?;
                next.state = DeliveryState::ProviderAccepted;
                next.last_receipt = Some(DeliveryReceiptSnapshot {
                    attempt_id: attempt.attempt_id,
                    proof_hash,
                    provider_message_ref_hash: Some(provider_message_ref_hash),
                    observed_at_ms,
                });
            }
            ReconciliationOutcome::Delivered {
                proof_hash,
                provider_message_ref_hash,
            } => {
                validate_receipt(&proof_hash, &provider_message_ref_hash)?;
                next.state = DeliveryState::Delivered;
                next.last_receipt = Some(DeliveryReceiptSnapshot {
                    attempt_id: attempt.attempt_id,
                    proof_hash,
                    provider_message_ref_hash: Some(provider_message_ref_hash),
                    observed_at_ms,
                });
            }
            ReconciliationOutcome::ProvenNotDelivered { proof_hash } => {
                if !is_sha256(&proof_hash) {
                    return Err(DeliveryBlockReason::InvalidHash);
                }
                next.state = DeliveryState::Failed;
                next.last_error_hash = Some(proof_hash);
            }
            ReconciliationOutcome::StillUnknown { proof_hash } => {
                if !is_sha256(&proof_hash) {
                    return Err(DeliveryBlockReason::InvalidHash);
                }
                return Err(DeliveryBlockReason::UncertainIsTerminal);
            }
        }
        next.active_attempt = None;
        Ok(next)
    }

    /// Provider acceptance alone never means delivered. A separately verified
    /// receipt may advance the state while preserving the same message ID.
    pub(crate) fn confirm_delivery(
        &self,
        proof_hash: String,
        provider_message_ref_hash: String,
        observed_at_ms: u64,
    ) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if self.state != DeliveryState::ProviderAccepted {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        let prior = self
            .last_receipt
            .as_ref()
            .ok_or(DeliveryBlockReason::ReceiptInvalid)?;
        validate_receipt(&proof_hash, &provider_message_ref_hash)?;
        if prior.provider_message_ref_hash.as_deref() != Some(provider_message_ref_hash.as_str())
            || observed_at_ms < prior.observed_at_ms
        {
            return Err(DeliveryBlockReason::ReceiptInvalid);
        }
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        next.state = DeliveryState::Delivered;
        next.last_receipt = Some(DeliveryReceiptSnapshot {
            attempt_id: prior.attempt_id,
            proof_hash,
            provider_message_ref_hash: Some(provider_message_ref_hash),
            observed_at_ms,
        });
        Ok(next)
    }

    pub(crate) fn cancel_unsubmitted(&self) -> Result<Self, DeliveryBlockReason> {
        validate_intent_integrity(self)?;
        if !matches!(
            self.state,
            DeliveryState::Draft
                | DeliveryState::AwaitingApproval
                | DeliveryState::ApprovedLocalOnly
                | DeliveryState::Ready
        ) || self.attempt_count != 0
        {
            return Err(DeliveryBlockReason::InvalidTransition);
        }
        let mut next = self.clone();
        next.state = DeliveryState::Cancelled;
        next.approval = None;
        next.revision = next.revision.saturating_add(1);
        Ok(next)
    }
}

fn validate_approval(
    intent: &MeetingDeliveryIntent,
    checks: &DeliveryChecks,
    required_scope: ApprovalScope,
) -> Result<(), DeliveryBlockReason> {
    let approval = intent
        .approval
        .as_ref()
        .ok_or(DeliveryBlockReason::ApprovalRequired)?;
    if approval.binding_hash != intent.binding_hash {
        return Err(DeliveryBlockReason::ApprovalInvalidated);
    }
    if approval.scope != required_scope || approval.expires_at_ms <= checks.now_ms {
        return Err(DeliveryBlockReason::ApprovalInvalidated);
    }
    if required_scope == ApprovalScope::ExternalPublish {
        let current_quote_hash = cost_quote_hash(
            checks
                .cost_quote
                .as_ref()
                .ok_or(DeliveryBlockReason::CostQuoteMissing)?,
        )?;
        if approval.cost_quote_hash.as_deref() != Some(current_quote_hash.as_str()) {
            return Err(DeliveryBlockReason::ApprovalInvalidated);
        }
    }
    Ok(())
}

fn validate_current(
    intent: &MeetingDeliveryIntent,
    checks: &DeliveryChecks,
    for_external_publish: bool,
    require_cost_reservation: bool,
) -> Result<(), DeliveryBlockReason> {
    validate_intent_integrity(intent)?;
    if checks.now_ms >= intent.expires_at_ms || checks.now_ms >= intent.grant.expires_at_ms {
        return Err(DeliveryBlockReason::Expired);
    }
    if checks.session_ref_hash.as_deref() != Some(intent.session_ref_hash.as_str()) {
        return Err(DeliveryBlockReason::SessionUnknown);
    }
    if checks.session_active.is_none() {
        return Err(DeliveryBlockReason::SessionUnknown);
    }
    if checks.session_active != Some(true) {
        return Err(DeliveryBlockReason::SessionInactive);
    }
    if checks.grant_ref_hash.is_none()
        || checks.grant_revision.is_none()
        || checks.grant_active.is_none()
        || checks.grant_expires_at_ms.is_none()
    {
        return Err(DeliveryBlockReason::GrantUnknown);
    }
    if checks.grant_ref_hash.as_deref() != Some(intent.grant.grant_ref_hash.as_str())
        || checks.grant_revision != Some(intent.grant.grant_revision)
        || checks.policy_revision != Some(intent.grant.policy_revision)
    {
        return Err(DeliveryBlockReason::GrantRevisionMismatch);
    }
    if checks.grant_active != Some(true)
        || checks
            .grant_expires_at_ms
            .is_some_and(|expires| checks.now_ms >= expires)
    {
        return Err(DeliveryBlockReason::GrantInactive);
    }
    if checks.grant_expires_at_ms != Some(intent.grant.expires_at_ms) {
        return Err(DeliveryBlockReason::GrantRevisionMismatch);
    }
    if intent.grant.session_ref_hash != intent.session_ref_hash
        || intent.destination.session_ref_hash != intent.session_ref_hash
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    let mode = checks
        .current_mode
        .ok_or(DeliveryBlockReason::ModeUnavailable)?;
    let trigger = checks
        .current_trigger_kind
        .ok_or(DeliveryBlockReason::ModeUnavailable)?;
    let topic_hash = checks
        .current_topic_ref_hash
        .as_deref()
        .ok_or(DeliveryBlockReason::TopicUnknown)?;
    if mode != intent.grant.mode
        || trigger != intent.grant.trigger_kind
        || topic_hash != intent.grant.topic_ref_hash
    {
        return Err(DeliveryBlockReason::PolicyRevisionMismatch);
    }
    if for_external_publish {
        match mode {
            AgentMode::AskedOnly
                if matches!(
                    trigger,
                    TriggerKind::ExplicitQuestion | TriggerKind::AuthorizedUiAsk
                ) => {}
            AgentMode::ProactiveBounded
                if matches!(
                    trigger,
                    TriggerKind::ExplicitQuestion | TriggerKind::ContextMention
                ) => {}
            _ => return Err(DeliveryBlockReason::ModeNotAllowed),
        }
        if checks.current_topic_allowed.is_none() {
            return Err(DeliveryBlockReason::TopicUnknown);
        }
        if checks.current_topic_allowed != Some(true)
            || !intent.grant.allowed_topic_ref_hashes.contains(topic_hash)
        {
            return Err(DeliveryBlockReason::TopicDenied);
        }
        if checks.payload_kind_allowed.is_none() {
            return Err(DeliveryBlockReason::PayloadDenied);
        }
        if checks.payload_kind_allowed != Some(true)
            || !intent
                .grant
                .allowed_payload_kinds
                .contains(payload_kind_code(intent.payload.kind))
        {
            return Err(DeliveryBlockReason::PayloadDenied);
        }
    }
    let audience = checks
        .audience
        .as_ref()
        .ok_or(DeliveryBlockReason::AudienceUnknown)?;
    if audience.is_known.is_none()
        || audience.has_unknown_members.is_none()
        || audience.can_receive.is_none()
    {
        return Err(DeliveryBlockReason::AudienceUnknown);
    }
    if audience.audience_binding_hash != intent.destination.audience_binding_hash
        || audience.audience_policy_revision != intent.destination.audience_policy_revision
    {
        return Err(DeliveryBlockReason::AudienceChanged);
    }
    if audience.is_known != Some(true) || audience.has_unknown_members != Some(false) {
        return Err(DeliveryBlockReason::AudienceUnknown);
    }
    if audience.can_receive != Some(true)
        || !intent
            .grant
            .allowed_audience_class_ref_hashes
            .contains(&audience.audience_class_ref_hash)
    {
        return Err(DeliveryBlockReason::AudienceDenied);
    }
    if checks.evidence_authority.len() != intent.evidence.len() {
        return Err(DeliveryBlockReason::EvidenceUnknown);
    }
    for captured in &intent.evidence {
        if checks.now_ms.saturating_sub(captured.captured_at_ms) > captured.max_age_ms {
            return Err(DeliveryBlockReason::EvidenceStale);
        }
        let current = checks
            .evidence_authority
            .iter()
            .find(|item| item.evidence_ref_hash == captured.evidence_ref_hash)
            .ok_or(DeliveryBlockReason::EvidenceUnknown)?;
        if current.source_ref_hash.is_none()
            || current.source_version_hash.is_none()
            || current.source_revision.is_none()
            || current.read_grant_ref_hash.is_none()
            || current.read_grant_revision.is_none()
            || current.share_grant_ref_hash.is_none()
            || current.share_grant_revision.is_none()
            || current.can_read.is_none()
            || current.can_share.is_none()
        {
            return Err(DeliveryBlockReason::EvidenceAclUnknown);
        }
        if current.can_read != Some(true) || current.can_share != Some(true) {
            return Err(DeliveryBlockReason::EvidenceAclDenied);
        }
        if current.source_ref_hash.as_deref() != Some(captured.source_ref_hash.as_str())
            || current.source_version_hash.as_deref() != Some(captured.source_version_hash.as_str())
            || current.source_revision != Some(captured.source_revision)
            || current.read_grant_ref_hash.as_deref() != Some(captured.read_grant_ref_hash.as_str())
            || current.read_grant_revision != Some(captured.read_grant_revision)
            || current.share_grant_ref_hash.as_deref()
                != Some(captured.share_grant_ref_hash.as_str())
            || current.share_grant_revision != Some(captured.share_grant_revision)
        {
            return Err(DeliveryBlockReason::SourceRevisionMismatch);
        }
    }
    if for_external_publish {
        let quote = checks
            .cost_quote
            .as_ref()
            .ok_or(DeliveryBlockReason::CostQuoteMissing)?;
        let budget = checks
            .cost_budget
            .as_ref()
            .ok_or(DeliveryBlockReason::CostQuoteMissing)?;
        if !is_sha256(&quote.quote_ref_hash)
            || !is_currency_code(&quote.currency_code)
            || quote.currency_code != intent.grant.currency_code
            || quote.currency_code != budget.currency_code
        {
            return Err(DeliveryBlockReason::CostQuoteInvalid);
        }
        if quote.valid_until_ms <= checks.now_ms {
            return Err(DeliveryBlockReason::CostQuoteStale);
        }
        if quote.upper_bound_micros > intent.grant.cost_cap_micros
            || quote.upper_bound_micros > budget.session_cap_micros
            || budget
                .session_spent_micros
                .saturating_add(quote.upper_bound_micros)
                > budget.session_cap_micros
        {
            return Err(DeliveryBlockReason::CostCapExceeded);
        }
        if require_cost_reservation {
            let budget_revision = checks
                .cost_budget
                .as_ref()
                .map(|budget| budget.ledger_revision)
                .ok_or(DeliveryBlockReason::CostQuoteMissing)?;
            let reservation = checks
                .cost_reservation
                .as_ref()
                .ok_or(DeliveryBlockReason::CostCapExceeded)?;
            if !is_sha256(&reservation.reservation_ref_hash)
                || reservation.intent_id != intent.intent_id
                || reservation.session_ref_hash != intent.session_ref_hash
                || reservation.binding_hash != intent.binding_hash
                || reservation.cost_quote_hash != cost_quote_hash(quote)?
                || reservation.reserved_cost_micros != quote.upper_bound_micros
                || reservation.ledger_revision == 0
                || reservation.ledger_revision != budget_revision
            {
                return Err(DeliveryBlockReason::CostCapExceeded);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_new_snapshot(
    idempotency_key: &[u8],
    session_ref_hash: &str,
    trigger_ref_hash: &str,
    answer_revision: u64,
    created_at_ms: u64,
    expires_at_ms: u64,
    payload: &EncryptedPayloadBinding,
    destination: &DestinationBinding,
    evidence: &[DeliveryEvidenceBinding],
    grant: &PublicationGrantBinding,
) -> Result<(), DeliveryBlockReason> {
    if idempotency_key.is_empty()
        || !is_sha256(session_ref_hash)
        || !is_sha256(trigger_ref_hash)
        || answer_revision == 0
        || expires_at_ms <= created_at_ms
        || expires_at_ms.saturating_sub(created_at_ms) > MAX_DELIVERY_LIFETIME_MS
        || !valid_opaque_asset_ref(&payload.encrypted_asset_ref)
        || payload.encrypted_asset_ref_hash != sha256_hex(payload.encrypted_asset_ref.as_bytes())
        || !is_sha256(&payload.ciphertext_sha256)
        || !is_sha256(&payload.payload_sha256)
        || !valid_destination(destination)
        || evidence.is_empty()
        || evidence.len() > MAX_EVIDENCE_ITEMS
        || !valid_grant(grant)
        || grant.session_ref_hash != session_ref_hash
        || destination.session_ref_hash != session_ref_hash
        || grant.expires_at_ms < expires_at_ms
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    let mut seen = BTreeSet::new();
    if evidence
        .iter()
        .any(|item| !seen.insert(item.evidence_ref_hash.as_str()) || !valid_evidence(item))
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    Ok(())
}

fn binding_hashes(
    payload: &EncryptedPayloadBinding,
    destination: &DestinationBinding,
    evidence: &[DeliveryEvidenceBinding],
    grant: &PublicationGrantBinding,
) -> Result<(String, String, String, String, String), DeliveryBlockReason> {
    let payload_hash = canonical_sha256(payload)?;
    let destination_hash = canonical_sha256(destination)?;
    let evidence_hash = canonical_sha256(evidence)?;
    let grant_hash = canonical_sha256(grant)?;
    let binding_hash = canonical_sha256(&(
        &payload_hash,
        &destination_hash,
        &evidence_hash,
        &grant_hash,
    ))?;
    Ok((
        payload_hash,
        destination_hash,
        evidence_hash,
        grant_hash,
        binding_hash,
    ))
}

fn valid_destination(destination: &DestinationBinding) -> bool {
    is_sha256(&destination.provider_ref_hash)
        && is_sha256(&destination.account_ref_hash)
        && is_sha256(&destination.occurrence_ref_hash)
        && is_sha256(&destination.session_ref_hash)
        && is_sha256(&destination.channel_ref_hash)
        && destination.thread_ref_hash.as_deref().is_none_or(is_sha256)
        && destination.audience_policy_revision > 0
        && is_sha256(&destination.audience_binding_hash)
}

fn valid_opaque_asset_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn valid_evidence(item: &DeliveryEvidenceBinding) -> bool {
    is_sha256(&item.evidence_ref_hash)
        && is_sha256(&item.source_ref_hash)
        && is_sha256(&item.source_version_hash)
        && item.source_revision > 0
        && is_sha256(&item.read_grant_ref_hash)
        && item.read_grant_revision > 0
        && is_sha256(&item.share_grant_ref_hash)
        && item.share_grant_revision > 0
        && is_sha256(&item.citation_hash)
        && item.max_age_ms > 0
}

fn valid_grant(grant: &PublicationGrantBinding) -> bool {
    is_sha256(&grant.grant_ref_hash)
        && grant.grant_revision > 0
        && is_sha256(&grant.session_ref_hash)
        && grant.policy_revision > 0
        && is_sha256(&grant.topic_ref_hash)
        && grant
            .allowed_topic_ref_hashes
            .iter()
            .all(|topic| is_sha256(topic))
        && grant
            .allowed_payload_kinds
            .iter()
            .all(|kind| matches!(kind.as_str(), "text" | "link"))
        && grant
            .allowed_audience_class_ref_hashes
            .iter()
            .all(|audience| is_sha256(audience))
        && grant.allowed_link_hosts.iter().all(|host| valid_host(host))
        && grant.max_payload_bytes > 0
        && grant.max_payload_bytes <= MAX_PAYLOAD_BYTES as u64
        && grant.expires_at_ms > 0
        && is_currency_code(&grant.currency_code)
        && grant.cost_cap_micros > 0
}

fn payload_kind_code(kind: PayloadKind) -> &'static str {
    match kind {
        PayloadKind::Text => "text",
        PayloadKind::Link => "link",
    }
}

fn validate_payload_content(
    kind: PayloadKind,
    payload_bytes: &[u8],
    grant: &PublicationGrantBinding,
) -> Result<(), DeliveryBlockReason> {
    if payload_bytes.is_empty()
        || payload_bytes.len() > MAX_PAYLOAD_BYTES
        || payload_bytes.len() as u64 > grant.max_payload_bytes
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    let body =
        std::str::from_utf8(payload_bytes).map_err(|_| DeliveryBlockReason::InvalidIntent)?;
    if body
        .chars()
        .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    match kind {
        PayloadKind::Text => {
            let lowercase = body.to_ascii_lowercase();
            if lowercase.contains("file:")
                || lowercase.contains("http://")
                || lowercase.contains("https://")
                || contains_local_path(body)
            {
                return Err(DeliveryBlockReason::PayloadDenied);
            }
        }
        PayloadKind::Link => {
            if body.trim() != body || body.chars().any(char::is_whitespace) {
                return Err(DeliveryBlockReason::PayloadDenied);
            }
            let url = Url::parse(body).map_err(|_| DeliveryBlockReason::PayloadDenied)?;
            let host = url.host_str().ok_or(DeliveryBlockReason::PayloadDenied)?;
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || !grant
                    .allowed_link_hosts
                    .contains(&host.to_ascii_lowercase())
            {
                return Err(DeliveryBlockReason::PayloadDenied);
            }
        }
    }
    Ok(())
}

fn valid_host(host: &str) -> bool {
    if host.is_empty()
        || host != host.to_ascii_lowercase()
        || host.contains('*')
        || host.parse::<std::net::IpAddr>().is_ok()
        || host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return false;
    }
    Url::parse(&format!("https://{host}/")).is_ok_and(|url| {
        url.host_str() == Some(host)
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

fn contains_local_path(body: &str) -> bool {
    let bytes = body.as_bytes();
    let has_windows_drive_path = bytes.windows(3).any(|part| {
        part[0].is_ascii_alphabetic() && part[1] == b':' && matches!(part[2], b'\\' | b'/')
    });
    let has_unc_path = body.contains("\\\\");
    let has_unix_absolute_path = body.split_whitespace().any(|token| {
        token.starts_with('/')
            && token
                .as_bytes()
                .iter()
                .filter(|byte| **byte == b'/')
                .count()
                > 1
    });
    has_windows_drive_path || has_unc_path || has_unix_absolute_path
}

fn validate_receipt(proof_hash: &str, message_ref_hash: &str) -> Result<(), DeliveryBlockReason> {
    if !is_sha256(proof_hash) || !is_sha256(message_ref_hash) {
        return Err(DeliveryBlockReason::ReceiptInvalid);
    }
    Ok(())
}

fn canonical_sha256<T: Serialize + ?Sized>(value: &T) -> Result<String, DeliveryBlockReason> {
    let encoded = serde_json::to_vec(value).map_err(|_| DeliveryBlockReason::InvalidIntent)?;
    Ok(sha256_hex(&encoded))
}

fn cost_quote_hash(quote: &DeliveryCostQuote) -> Result<String, DeliveryBlockReason> {
    canonical_sha256(&(
        &quote.quote_ref_hash,
        &quote.currency_code,
        quote.upper_bound_micros,
        quote.valid_until_ms,
    ))
}

fn validate_intent_integrity(intent: &MeetingDeliveryIntent) -> Result<(), DeliveryBlockReason> {
    let (payload_hash, destination_hash, evidence_hash, grant_hash, binding_hash) = binding_hashes(
        &intent.payload,
        &intent.destination,
        &intent.evidence,
        &intent.grant,
    )?;
    if intent.payload_hash != payload_hash
        || intent.destination_hash != destination_hash
        || intent.evidence_hash != evidence_hash
        || intent.grant_hash != grant_hash
        || intent.binding_hash != binding_hash
        || !is_sha256(&intent.idempotency_key_hash)
    {
        return Err(DeliveryBlockReason::InvalidIntent);
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_currency_code(currency: &str) -> bool {
    currency.len() == 3 && currency.bytes().all(|byte| byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: u64 = 2_000;

    fn hash(value: &str) -> String {
        sha256_hex(value.as_bytes())
    }

    fn destination() -> DestinationBinding {
        DestinationBinding {
            provider_ref_hash: hash("provider"),
            account_ref_hash: hash("account"),
            occurrence_ref_hash: hash("occurrence"),
            session_ref_hash: hash("session"),
            channel_ref_hash: hash("channel"),
            thread_ref_hash: None,
            channel_kind: ChannelKind::MeetingChat,
            audience_policy_revision: 1,
            audience_binding_hash: hash("audience"),
        }
    }

    fn evidence() -> DeliveryEvidenceBinding {
        DeliveryEvidenceBinding {
            evidence_ref_hash: hash("evidence"),
            source_ref_hash: hash("source"),
            source_version_hash: hash("source-version"),
            source_revision: 2,
            read_grant_ref_hash: hash("read-grant"),
            read_grant_revision: 3,
            share_grant_ref_hash: hash("share-grant"),
            share_grant_revision: 4,
            citation_hash: hash("citation"),
            captured_at_ms: 1_000,
            max_age_ms: 20_000,
        }
    }

    fn grant() -> PublicationGrantBinding {
        PublicationGrantBinding {
            grant_ref_hash: hash("grant"),
            grant_revision: 5,
            session_ref_hash: hash("session"),
            policy_revision: 6,
            mode: AgentMode::AskedOnly,
            trigger_kind: TriggerKind::ExplicitQuestion,
            topic_ref_hash: hash("finance"),
            allowed_topic_ref_hashes: BTreeSet::from([hash("finance")]),
            allowed_payload_kinds: BTreeSet::from(["text".to_string(), "link".to_string()]),
            allowed_audience_class_ref_hashes: BTreeSet::from([hash("known-participants")]),
            allowed_link_hosts: BTreeSet::from(["docs.example.com".to_string()]),
            max_payload_bytes: 5_000,
            expires_at_ms: 50_000,
            currency_code: "USD".to_string(),
            cost_cap_micros: 500,
        }
    }

    fn draft() -> MeetingDeliveryIntent {
        MeetingDeliveryIntent::new_draft(
            b"intent-idempotency-key",
            hash("session"),
            hash("trigger"),
            1,
            1_000,
            12_000,
            "asset-opaque-1".to_string(),
            hash("ciphertext"),
            PayloadKind::Text,
            b"The report supports this answer. See citation A.",
            destination(),
            vec![evidence()],
            grant(),
        )
        .unwrap()
    }

    fn checks() -> DeliveryChecks {
        DeliveryChecks {
            now_ms: NOW_MS,
            session_ref_hash: Some(hash("session")),
            session_active: Some(true),
            grant_ref_hash: Some(hash("grant")),
            grant_revision: Some(5),
            grant_active: Some(true),
            grant_expires_at_ms: Some(50_000),
            policy_revision: Some(6),
            current_mode: Some(AgentMode::AskedOnly),
            current_trigger_kind: Some(TriggerKind::ExplicitQuestion),
            current_topic_ref_hash: Some(hash("finance")),
            current_topic_allowed: Some(true),
            payload_kind_allowed: Some(true),
            audience: Some(AudienceSnapshot {
                audience_binding_hash: hash("audience"),
                audience_policy_revision: 1,
                audience_class_ref_hash: hash("known-participants"),
                is_known: Some(true),
                has_unknown_members: Some(false),
                can_receive: Some(true),
            }),
            evidence_authority: vec![CurrentEvidenceAuthority {
                evidence_ref_hash: hash("evidence"),
                source_ref_hash: Some(hash("source")),
                source_version_hash: Some(hash("source-version")),
                source_revision: Some(2),
                read_grant_ref_hash: Some(hash("read-grant")),
                read_grant_revision: Some(3),
                share_grant_ref_hash: Some(hash("share-grant")),
                share_grant_revision: Some(4),
                can_read: Some(true),
                can_share: Some(true),
            }],
            cost_quote: Some(DeliveryCostQuote {
                quote_ref_hash: hash("cost-quote"),
                currency_code: "USD".to_string(),
                upper_bound_micros: 100,
                valid_until_ms: 10_000,
            }),
            cost_budget: Some(DeliveryCostBudget {
                currency_code: "USD".to_string(),
                session_spent_micros: 0,
                session_cap_micros: 500,
                ledger_revision: 2,
            }),
            cost_reservation: None,
            adapter_readiness: AdapterReadiness::Qualified,
        }
    }

    fn awaiting_approval() -> MeetingDeliveryIntent {
        draft().request_approval().unwrap()
    }

    fn approved_ready() -> MeetingDeliveryIntent {
        awaiting_approval()
            .approve_manual(hash("operator"), &checks(), ApprovalScope::ExternalPublish)
            .unwrap()
    }

    #[test]
    fn approval_binds_exact_payload_destination_evidence_grant_and_cost_quote() {
        let waiting = awaiting_approval();
        let mut altered_audience = checks();
        altered_audience.audience.as_mut().unwrap().is_known = None;
        assert_eq!(
            waiting
                .approve_manual(
                    hash("operator"),
                    &altered_audience,
                    ApprovalScope::LocalPreviewOnly,
                )
                .err(),
            Some(DeliveryBlockReason::AudienceUnknown)
        );

        let mut changed_source = checks();
        changed_source.evidence_authority[0].source_version_hash = Some(hash("new-version"));
        assert_eq!(
            waiting
                .approve_manual(
                    hash("operator"),
                    &changed_source,
                    ApprovalScope::ExternalPublish,
                )
                .err(),
            Some(DeliveryBlockReason::SourceRevisionMismatch)
        );

        let ready = approved_ready();
        let mut changed_quote = checks();
        changed_quote
            .cost_quote
            .as_mut()
            .unwrap()
            .upper_bound_micros += 1;
        assert_eq!(
            ready
                .prepare_handoff(
                    &changed_quote,
                    &DestinationLeaseSnapshot {
                        lease_ref_hash: hash("lease"),
                        destination_hash: ready.destination_hash.clone(),
                        intent_revision: ready.revision,
                        acquired_at_ms: NOW_MS,
                        expires_at_ms: NOW_MS + 1_000,
                    },
                    ready.revision,
                )
                .err(),
            Some(DeliveryBlockReason::ApprovalInvalidated)
        );
    }

    #[test]
    fn editing_an_unsubmitted_payload_invalidates_exact_approval() {
        let ready = approved_ready();
        let revised = ready
            .revise_unsubmitted(
                PayloadKind::Text,
                "asset-opaque-2".to_string(),
                hash("new-ciphertext"),
                b"Edited after approval.",
                destination(),
                vec![evidence()],
                grant(),
                NOW_MS,
            )
            .unwrap();
        assert_eq!(revised.state, DeliveryState::AwaitingApproval);
        assert!(revised.approval.is_none());
        assert_ne!(revised.binding_hash, ready.binding_hash);
    }

    #[test]
    fn local_preview_approval_never_becomes_provider_ready() {
        let waiting = awaiting_approval();
        let mut no_adapter = checks();
        no_adapter.adapter_readiness = AdapterReadiness::Unavailable;
        no_adapter.cost_quote = None;
        no_adapter.cost_budget = None;
        let approved = waiting
            .approve_manual(
                hash("operator"),
                &no_adapter,
                ApprovalScope::LocalPreviewOnly,
            )
            .unwrap();
        assert_eq!(approved.state, DeliveryState::ApprovedLocalOnly);
        assert_eq!(
            approved
                .prepare_handoff(
                    &no_adapter,
                    &DestinationLeaseSnapshot {
                        lease_ref_hash: hash("lease"),
                        destination_hash: approved.destination_hash.clone(),
                        intent_revision: approved.revision,
                        acquired_at_ms: NOW_MS,
                        expires_at_ms: NOW_MS + 1_000,
                    },
                    approved.revision,
                )
                .err(),
            Some(DeliveryBlockReason::InvalidTransition)
        );
    }

    #[test]
    fn uncertain_handoff_is_reconciliation_only_and_cannot_be_retried() {
        let ready = approved_ready();
        let quote = checks().cost_quote.unwrap();
        let prepared = ready
            .prepare_handoff(
                &DeliveryChecks {
                    cost_reservation: Some(DeliveryCostReservation {
                        reservation_ref_hash: hash("reservation"),
                        intent_id: ready.intent_id,
                        session_ref_hash: ready.session_ref_hash.clone(),
                        binding_hash: ready.binding_hash.clone(),
                        cost_quote_hash: cost_quote_hash(&quote).unwrap(),
                        reserved_cost_micros: quote.upper_bound_micros,
                        ledger_revision: 2,
                    }),
                    ..checks()
                },
                &DestinationLeaseSnapshot {
                    lease_ref_hash: hash("lease"),
                    destination_hash: ready.destination_hash.clone(),
                    intent_revision: ready.revision,
                    acquired_at_ms: NOW_MS,
                    expires_at_ms: NOW_MS + 1_000,
                },
                ready.revision,
            )
            .unwrap();
        let attempt_id = prepared.descriptor.attempt_id;
        let unknown = prepared
            .next_intent
            .record_handoff_outcome(
                attempt_id,
                HandoffOutcome::PossiblySent {
                    error_hash: hash("timeout-after-handoff"),
                },
                NOW_MS + 1,
            )
            .unwrap();
        assert_eq!(unknown.state, DeliveryState::DeliveryUnknown);
        assert_eq!(
            unknown
                .prepare_handoff(
                    &checks(),
                    &DestinationLeaseSnapshot {
                        lease_ref_hash: hash("second-lease"),
                        destination_hash: unknown.destination_hash.clone(),
                        intent_revision: unknown.revision,
                        acquired_at_ms: NOW_MS + 2,
                        expires_at_ms: NOW_MS + 1_000,
                    },
                    unknown.revision,
                )
                .err(),
            Some(DeliveryBlockReason::InvalidTransition)
        );
        let delivered = unknown
            .reconcile_unknown(
                ReconciliationOutcome::Delivered {
                    proof_hash: hash("provider-delivery-proof"),
                    provider_message_ref_hash: hash("provider-message"),
                },
                NOW_MS + 5,
            )
            .unwrap();
        assert_eq!(delivered.state, DeliveryState::Delivered);
    }

    #[test]
    fn accepted_requires_separate_delivery_receipt() {
        let ready = approved_ready();
        let quote = checks().cost_quote.unwrap();
        let prepared = ready
            .prepare_handoff(
                &DeliveryChecks {
                    cost_reservation: Some(DeliveryCostReservation {
                        reservation_ref_hash: hash("reservation"),
                        intent_id: ready.intent_id,
                        session_ref_hash: ready.session_ref_hash.clone(),
                        binding_hash: ready.binding_hash.clone(),
                        cost_quote_hash: cost_quote_hash(&quote).unwrap(),
                        reserved_cost_micros: quote.upper_bound_micros,
                        ledger_revision: 2,
                    }),
                    ..checks()
                },
                &DestinationLeaseSnapshot {
                    lease_ref_hash: hash("lease"),
                    destination_hash: ready.destination_hash.clone(),
                    intent_revision: ready.revision,
                    acquired_at_ms: NOW_MS,
                    expires_at_ms: NOW_MS + 1_000,
                },
                ready.revision,
            )
            .unwrap();
        let message_ref_hash = hash("provider-message");
        let accepted = prepared
            .next_intent
            .record_handoff_outcome(
                prepared.descriptor.attempt_id,
                HandoffOutcome::ProviderAccepted {
                    proof_hash: hash("accepted-proof"),
                    provider_message_ref_hash: message_ref_hash.clone(),
                },
                NOW_MS + 1,
            )
            .unwrap();
        assert_eq!(accepted.state, DeliveryState::ProviderAccepted);
        assert_eq!(
            accepted
                .confirm_delivery(hash("delivered-proof"), message_ref_hash, NOW_MS + 2,)
                .unwrap()
                .state,
            DeliveryState::Delivered
        );
    }

    #[test]
    fn external_handoff_requires_cost_reservation_and_approved_https_host() {
        let ready = approved_ready();
        let quote = checks().cost_quote.unwrap();
        let lease = DestinationLeaseSnapshot {
            lease_ref_hash: hash("lease"),
            destination_hash: ready.destination_hash.clone(),
            intent_revision: ready.revision,
            acquired_at_ms: NOW_MS,
            expires_at_ms: NOW_MS + MAX_DESTINATION_LEASE_MS + 1,
        };
        let mut with_reservation = checks();
        with_reservation.cost_reservation = Some(DeliveryCostReservation {
            reservation_ref_hash: hash("reservation"),
            intent_id: ready.intent_id,
            session_ref_hash: ready.session_ref_hash.clone(),
            binding_hash: ready.binding_hash.clone(),
            cost_quote_hash: cost_quote_hash(&quote).unwrap(),
            reserved_cost_micros: quote.upper_bound_micros,
            ledger_revision: 2,
        });
        assert_eq!(
            ready
                .prepare_handoff(&with_reservation, &lease, ready.revision)
                .err(),
            Some(DeliveryBlockReason::LeaseInvalid)
        );

        let link = MeetingDeliveryIntent::new_draft(
            b"another-idempotency-key",
            hash("session"),
            hash("trigger"),
            1,
            1_000,
            12_000,
            "asset-opaque-3".to_string(),
            hash("ciphertext"),
            PayloadKind::Link,
            b"https://not-approved.example.com/report",
            destination(),
            vec![evidence()],
            grant(),
        );
        assert_eq!(link.err(), Some(DeliveryBlockReason::PayloadDenied));

        let local_path = MeetingDeliveryIntent::new_draft(
            b"path-idempotency-key",
            hash("session"),
            hash("trigger"),
            1,
            1_000,
            12_000,
            "asset-opaque-4".to_string(),
            hash("ciphertext"),
            PayloadKind::Text,
            b"Open C:\\Users\\pc\\secret.txt",
            destination(),
            vec![evidence()],
            grant(),
        );
        assert_eq!(local_path.err(), Some(DeliveryBlockReason::PayloadDenied));

        let _ = cost_quote_hash(&quote).unwrap();
    }
}
