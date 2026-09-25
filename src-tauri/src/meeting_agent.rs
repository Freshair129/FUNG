//! Provider-independent local meeting-agent policy and draft coordination.
//!
//! This module is intentionally a pure state machine. Callers must construct
//! committed input/evidence snapshots from the trusted transcript and knowledge
//! services, persist returned drafts through Genesis, and encrypt draft text
//! before any persistent custody. It does not call a model or a provider.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_SESSION_MS: u64 = 4 * 60 * 60 * 1000;
const MAX_TRIGGER_LIFETIME_MS: u64 = 30 * 1000;
const MIN_COMMIT_DEBOUNCE_MS: u64 = 2 * 1000;
const CONTEXT_COOLDOWN_MS: u64 = 60 * 1000;
const MAX_RUNS_PER_MINUTE: usize = 2;
const MAX_RUNS_PER_HOUR: usize = 20;
const MAX_RETRIEVAL_ATTEMPTS: u8 = 3;
const MAX_INPUT_CHARS: usize = 4_000;
const MAX_DRAFT_CHARS: usize = 20_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentMode {
    Off,
    Observe,
    Draft,
    AskedOnly,
    ProactiveBounded,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerKind {
    ExplicitQuestion,
    ContextMention,
    AuthorizedUiAsk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputSourceKind {
    Transcript,
    MeetingChat,
    AuthorizedUiAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InputActor {
    HumanParticipant {
        participant_session_ref_hash: String,
        is_self: bool,
        is_agent: bool,
    },
    AuthorizedOperator {
        actor_ref_hash: String,
        authorized: bool,
    },
    Unknown,
}

/// Ephemeral source text plus immutable identifiers read from a committed
/// event. Do not serialize or log this value; persist only encrypted drafts.
pub(crate) struct CommittedAgentInput {
    pub(crate) session_ref_hash: String,
    pub(crate) source_ref_hash: String,
    pub(crate) event_ref_hash: String,
    pub(crate) revision_ref_hash: String,
    pub(crate) source_generation: u64,
    pub(crate) source_sequence: u64,
    pub(crate) commit_sequence: u64,
    pub(crate) is_final: bool,
    pub(crate) committed_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) source_kind: InputSourceKind,
    pub(crate) actor: InputActor,
    pub(crate) trigger_kind: TriggerKind,
    pub(crate) addressed_to_agent: bool,
    pub(crate) topic_id: Option<String>,
    pub(crate) text: String,
    pub(crate) read_grant_ref_hash: String,
    pub(crate) read_grant_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CurrentInputAuthority {
    pub(crate) source_ref_hash: String,
    pub(crate) revision_ref_hash: Option<String>,
    pub(crate) read_grant_ref_hash: Option<String>,
    pub(crate) read_grant_revision: Option<u64>,
    pub(crate) can_read: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentEvidenceInput {
    pub(crate) evidence_ref_hash: String,
    pub(crate) source_ref_hash: String,
    pub(crate) source_version_hash: String,
    pub(crate) evidence_revision: u64,
    pub(crate) citation_hash: String,
    pub(crate) read_grant_ref_hash: String,
    pub(crate) read_grant_revision: u64,
    pub(crate) captured_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CurrentEvidenceAuthority {
    pub(crate) evidence_ref_hash: String,
    pub(crate) source_ref_hash: Option<String>,
    pub(crate) source_version_hash: Option<String>,
    pub(crate) evidence_revision: Option<u64>,
    pub(crate) read_grant_ref_hash: Option<String>,
    pub(crate) read_grant_revision: Option<u64>,
    pub(crate) can_read: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentCostQuote {
    pub(crate) quote_ref_hash: String,
    pub(crate) currency_code: String,
    /// An upper bound supplied by the selected local execution profile.
    pub(crate) upper_bound_micros: u64,
    pub(crate) valid_until_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentSessionPolicy {
    pub(crate) session_ref_hash: String,
    pub(crate) policy_revision: u64,
    pub(crate) mode: AgentMode,
    pub(crate) enabled_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) meeting_ends_at_ms: Option<u64>,
    pub(crate) read_grant_ref_hash: String,
    pub(crate) read_grant_revision: u64,
    pub(crate) allowed_topics: BTreeSet<String>,
    pub(crate) max_input_age_ms: u64,
    pub(crate) max_trigger_lifetime_ms: u64,
    pub(crate) max_runs_per_minute: u8,
    pub(crate) max_runs_per_hour: u8,
    pub(crate) max_retrieval_attempts: u8,
    pub(crate) max_model_tokens_per_run: u64,
    pub(crate) max_model_tokens_per_session: u64,
    pub(crate) cost_currency_code: String,
    pub(crate) max_cost_micros_per_run: u64,
    pub(crate) max_cost_micros_per_session: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentBlockReason {
    InvalidPolicy,
    SessionMismatch,
    SessionInactive,
    ModeDisabled,
    TriggerNotAllowed,
    TopicMissing,
    TopicNotAllowed,
    InputNotCommitted,
    InputFromSelfOrAgent,
    UnknownInputActor,
    UnauthorisedOperator,
    InvalidInput,
    InputExpired,
    InputStale,
    DebouncePending,
    InputAclUnknown,
    InputAclDenied,
    SourceRevisionMismatch,
    GrantRevisionMismatch,
    EvidenceUnknown,
    EvidenceAclUnknown,
    EvidenceAclDenied,
    EvidenceStale,
    DuplicateEvidence,
    ActiveRunExists,
    RateLimit,
    Cooldown,
    RetrievalLimit,
    TokenLimit,
    CostQuoteMissing,
    CostQuoteStale,
    CostQuoteInvalid,
    CostLimit,
    RunNotFound,
    RunExpired,
    InvalidCompletion,
    CitationMismatch,
}

struct ActiveAgentRun {
    run_id: Uuid,
    input: CommittedAgentInputRef,
    evidence: Vec<AgentEvidenceInput>,
    retrieval_limit: u8,
    token_reservation: u64,
    cost_reservation_micros: u64,
    policy_revision: u64,
    deadline_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommittedAgentInputRef {
    pub(crate) session_ref_hash: String,
    pub(crate) source_ref_hash: String,
    pub(crate) event_ref_hash: String,
    pub(crate) revision_ref_hash: String,
    pub(crate) source_generation: u64,
    pub(crate) source_sequence: u64,
    pub(crate) commit_sequence: u64,
    pub(crate) is_final: bool,
    pub(crate) committed_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) source_kind: InputSourceKind,
    pub(crate) trigger_kind: TriggerKind,
    pub(crate) topic_id: Option<String>,
    pub(crate) input_hash: String,
    pub(crate) read_grant_ref_hash: String,
    pub(crate) read_grant_revision: u64,
}

pub(crate) struct AgentRunPermit {
    pub(crate) run_id: Uuid,
    pub(crate) policy_revision: u64,
    pub(crate) cost_quote_ref_hash: String,
    pub(crate) reserved_cost_micros: u64,
    pub(crate) input: CommittedAgentInputRef,
    pub(crate) evidence: Vec<AgentEvidenceInput>,
    pub(crate) max_retrieval_attempts: u8,
    pub(crate) max_model_tokens: u64,
    pub(crate) deadline_ms: u64,
    query_text: String,
}

impl AgentRunPermit {
    /// Returns transient query text for the local retrieval/model owner.
    /// Callers must keep it in memory only and never log it.
    pub(crate) fn query_text(&self) -> &str {
        &self.query_text
    }
}

/// Plaintext draft body. Deliberately has no `Debug`/serde implementation.
pub(crate) struct LocalAgentDraft {
    pub(crate) draft_id: Uuid,
    pub(crate) run_id: Uuid,
    pub(crate) session_ref_hash: String,
    pub(crate) input_ref: CommittedAgentInputRef,
    pub(crate) evidence: Vec<AgentEvidenceInput>,
    pub(crate) created_at_ms: u64,
    pub(crate) policy_revision: u64,
    body: String,
}

impl LocalAgentDraft {
    /// The caller must encrypt this body before persistent custody.
    pub(crate) fn body_for_encryption(&self) -> &str {
        &self.body
    }
}

pub(crate) enum LocalDraftContent {
    Grounded {
        body: String,
        citations: Vec<String>,
    },
    NeedClarification {
        body: String,
    },
}

pub(crate) struct AgentRunCompletion {
    pub(crate) run_id: Uuid,
    pub(crate) now_ms: u64,
    pub(crate) retrieval_attempts: u8,
    pub(crate) model_tokens_used: u64,
    /// `None` means final cost is unknown; the full upper-bound reservation
    /// remains charged to the session budget.
    pub(crate) actual_cost_micros: Option<u64>,
    pub(crate) current_input_authority: CurrentInputAuthority,
    pub(crate) current_evidence_authority: Vec<CurrentEvidenceAuthority>,
    pub(crate) content: LocalDraftContent,
}

#[derive(Default)]
pub(crate) struct MeetingAgentCoordinator {
    session_ref_hash: Option<String>,
    active_run: Option<ActiveAgentRun>,
    recent_runs_ms: VecDeque<u64>,
    recent_context_keys: BTreeMap<String, u64>,
    model_tokens_reserved: u64,
    cost_micros_reserved: u64,
}

impl MeetingAgentCoordinator {
    pub(crate) fn new(session_ref_hash: String) -> Result<Self, AgentBlockReason> {
        if !is_sha256(&session_ref_hash) {
            return Err(AgentBlockReason::InvalidPolicy);
        }
        Ok(Self {
            session_ref_hash: Some(session_ref_hash),
            ..Self::default()
        })
    }

    /// Authorizes one local run from a final committed input and current ACL
    /// snapshots. It never performs retrieval, inference, or publication.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_run(
        &mut self,
        policy: &AgentSessionPolicy,
        input: CommittedAgentInput,
        current_input_authority: &CurrentInputAuthority,
        evidence: Vec<AgentEvidenceInput>,
        current_evidence_authority: &[CurrentEvidenceAuthority],
        quote: Option<AgentCostQuote>,
        max_model_tokens: u64,
        max_retrieval_attempts: u8,
        now_ms: u64,
    ) -> Result<AgentRunPermit, AgentBlockReason> {
        validate_policy(policy)?;
        if self.session_ref_hash.as_deref() != Some(policy.session_ref_hash.as_str()) {
            return Err(AgentBlockReason::SessionMismatch);
        }
        if self.active_run.is_some() {
            return Err(AgentBlockReason::ActiveRunExists);
        }
        if now_ms < policy.enabled_at_ms || now_ms >= policy.expires_at_ms {
            return Err(AgentBlockReason::SessionInactive);
        }
        validate_mode_and_topic(policy, &input)?;
        self.enforce_context_cooldown(&input, now_ms)?;
        let input_ref = validate_input(policy, &input, current_input_authority, now_ms)?;
        if input.source_kind == InputSourceKind::AuthorizedUiAction && evidence.is_empty() {
            return Err(AgentBlockReason::EvidenceUnknown);
        }
        validate_evidence(policy, &evidence, current_evidence_authority, now_ms)?;
        if max_retrieval_attempts == 0
            || max_retrieval_attempts > policy.max_retrieval_attempts
            || max_retrieval_attempts > MAX_RETRIEVAL_ATTEMPTS
        {
            return Err(AgentBlockReason::RetrievalLimit);
        }
        if max_model_tokens == 0 || max_model_tokens > policy.max_model_tokens_per_run {
            return Err(AgentBlockReason::TokenLimit);
        }
        if self.model_tokens_reserved.saturating_add(max_model_tokens)
            > policy.max_model_tokens_per_session
        {
            return Err(AgentBlockReason::TokenLimit);
        }
        let quote = quote.ok_or(AgentBlockReason::CostQuoteMissing)?;
        if !is_sha256(&quote.quote_ref_hash)
            || !is_currency_code(&quote.currency_code)
            || quote.currency_code != policy.cost_currency_code
            || quote.valid_until_ms <= now_ms
        {
            return Err(if quote.valid_until_ms <= now_ms {
                AgentBlockReason::CostQuoteStale
            } else {
                AgentBlockReason::CostQuoteInvalid
            });
        }
        if quote.upper_bound_micros > policy.max_cost_micros_per_run
            || self
                .cost_micros_reserved
                .saturating_add(quote.upper_bound_micros)
                > policy.max_cost_micros_per_session
        {
            return Err(AgentBlockReason::CostLimit);
        }
        self.enforce_rate(policy, input.trigger_kind, now_ms)?;

        let run_id = Uuid::new_v4();
        let deadline_ms = input.expires_at_ms.min(policy.expires_at_ms);
        let query_text = input.text.clone();
        self.model_tokens_reserved = self.model_tokens_reserved.saturating_add(max_model_tokens);
        self.cost_micros_reserved = self
            .cost_micros_reserved
            .saturating_add(quote.upper_bound_micros);
        self.recent_runs_ms.push_back(now_ms);
        if input.trigger_kind == TriggerKind::ContextMention {
            let key = context_key(input.topic_id.as_deref(), &input.text);
            self.recent_context_keys.insert(key, now_ms);
        }
        self.active_run = Some(ActiveAgentRun {
            run_id,
            input: input_ref.clone(),
            evidence: evidence.clone(),
            retrieval_limit: max_retrieval_attempts,
            token_reservation: max_model_tokens,
            cost_reservation_micros: quote.upper_bound_micros,
            policy_revision: policy.policy_revision,
            deadline_ms,
        });

        Ok(AgentRunPermit {
            run_id,
            policy_revision: policy.policy_revision,
            cost_quote_ref_hash: quote.quote_ref_hash,
            reserved_cost_micros: quote.upper_bound_micros,
            input: input_ref,
            evidence,
            max_retrieval_attempts,
            max_model_tokens,
            deadline_ms,
            query_text,
        })
    }

    /// Completes a local run only after rechecking freshness and ACL state.
    /// A stale or invalid completion consumes the reservation and is discarded.
    pub(crate) fn finish_run(
        &mut self,
        policy: &AgentSessionPolicy,
        completion: AgentRunCompletion,
    ) -> Result<LocalAgentDraft, AgentBlockReason> {
        let active = self
            .active_run
            .as_ref()
            .ok_or(AgentBlockReason::RunNotFound)?;
        if active.run_id != completion.run_id {
            return Err(AgentBlockReason::RunNotFound);
        }
        let active = self
            .active_run
            .take()
            .ok_or(AgentBlockReason::RunNotFound)?;
        validate_policy(policy)?;
        if completion.now_ms >= active.deadline_ms
            || completion.now_ms >= policy.expires_at_ms
            || completion.now_ms < active.input.committed_at_ms
        {
            return Err(AgentBlockReason::RunExpired);
        }
        if self.session_ref_hash.as_deref() != Some(policy.session_ref_hash.as_str())
            || active.input.session_ref_hash != policy.session_ref_hash
        {
            return Err(AgentBlockReason::SessionMismatch);
        }
        if active.policy_revision != policy.policy_revision {
            return Err(AgentBlockReason::GrantRevisionMismatch);
        }
        if completion
            .now_ms
            .saturating_sub(active.input.committed_at_ms)
            > policy.max_input_age_ms
        {
            return Err(AgentBlockReason::InputStale);
        }
        validate_input_ref_authority(
            &active.input,
            &policy.read_grant_ref_hash,
            policy.read_grant_revision,
            &completion.current_input_authority,
        )?;
        validate_evidence(
            policy,
            &active.evidence,
            &completion.current_evidence_authority,
            completion.now_ms,
        )?;
        if completion.retrieval_attempts > active.retrieval_limit
            || completion.retrieval_attempts > MAX_RETRIEVAL_ATTEMPTS
            || completion.model_tokens_used > active.token_reservation
        {
            return Err(AgentBlockReason::InvalidCompletion);
        }
        let actual_cost = completion
            .actual_cost_micros
            .unwrap_or(active.cost_reservation_micros);
        if actual_cost > active.cost_reservation_micros {
            return Err(AgentBlockReason::CostLimit);
        }
        self.cost_micros_reserved = self
            .cost_micros_reserved
            .saturating_sub(active.cost_reservation_micros)
            .saturating_add(actual_cost);
        let body = match completion.content {
            LocalDraftContent::Grounded { body, citations } => {
                validate_draft_body(&body)?;
                if citations.is_empty()
                    || citations.iter().any(|citation| {
                        !active
                            .evidence
                            .iter()
                            .any(|evidence| evidence.citation_hash == *citation)
                    })
                {
                    return Err(AgentBlockReason::CitationMismatch);
                }
                body
            }
            LocalDraftContent::NeedClarification { body } => {
                validate_draft_body(&body)?;
                body
            }
        };
        Ok(LocalAgentDraft {
            draft_id: Uuid::new_v4(),
            run_id: active.run_id,
            session_ref_hash: policy.session_ref_hash.clone(),
            input_ref: active.input,
            evidence: active.evidence,
            created_at_ms: completion.now_ms,
            policy_revision: policy.policy_revision,
            body,
        })
    }

    /// Cancels an active run. Unknown actual cost conservatively retains the
    /// full reservation; no draft is produced.
    pub(crate) fn cancel_run(
        &mut self,
        run_id: Uuid,
        actual_cost_micros: Option<u64>,
    ) -> Result<(), AgentBlockReason> {
        let active = self
            .active_run
            .as_ref()
            .ok_or(AgentBlockReason::RunNotFound)?;
        if active.run_id != run_id {
            return Err(AgentBlockReason::RunNotFound);
        }
        let active = self
            .active_run
            .take()
            .ok_or(AgentBlockReason::RunNotFound)?;
        if let Some(actual) = actual_cost_micros {
            if actual > active.cost_reservation_micros {
                return Err(AgentBlockReason::CostLimit);
            }
            self.cost_micros_reserved = self
                .cost_micros_reserved
                .saturating_sub(active.cost_reservation_micros)
                .saturating_add(actual);
        }
        Ok(())
    }

    fn enforce_rate(
        &mut self,
        policy: &AgentSessionPolicy,
        _trigger_kind: TriggerKind,
        now_ms: u64,
    ) -> Result<(), AgentBlockReason> {
        while self
            .recent_runs_ms
            .front()
            .is_some_and(|started| now_ms.saturating_sub(*started) >= 60 * 60 * 1000)
        {
            self.recent_runs_ms.pop_front();
        }
        let last_minute = self
            .recent_runs_ms
            .iter()
            .filter(|started| now_ms.saturating_sub(**started) < 60 * 1000)
            .count();
        if last_minute >= usize::from(policy.max_runs_per_minute)
            || self.recent_runs_ms.len() >= usize::from(policy.max_runs_per_hour)
        {
            return Err(AgentBlockReason::RateLimit);
        }
        Ok(())
    }

    fn enforce_context_cooldown(
        &mut self,
        input: &CommittedAgentInput,
        now_ms: u64,
    ) -> Result<(), AgentBlockReason> {
        self.recent_context_keys
            .retain(|_, timestamp| now_ms.saturating_sub(*timestamp) < CONTEXT_COOLDOWN_MS);
        if input.trigger_kind != TriggerKind::ContextMention {
            return Ok(());
        }
        let key = context_key(input.topic_id.as_deref(), &input.text);
        if self
            .recent_context_keys
            .get(&key)
            .is_some_and(|timestamp| now_ms.saturating_sub(*timestamp) < CONTEXT_COOLDOWN_MS)
        {
            return Err(AgentBlockReason::Cooldown);
        }
        Ok(())
    }
}

fn validate_policy(policy: &AgentSessionPolicy) -> Result<(), AgentBlockReason> {
    if !is_sha256(&policy.session_ref_hash)
        || !is_sha256(&policy.read_grant_ref_hash)
        || policy.policy_revision == 0
        || policy.read_grant_revision == 0
        || policy.enabled_at_ms >= policy.expires_at_ms
        || policy.expires_at_ms.saturating_sub(policy.enabled_at_ms) > MAX_SESSION_MS
        || policy
            .meeting_ends_at_ms
            .is_some_and(|end| end < policy.expires_at_ms)
        || policy.max_input_age_ms == 0
        || policy.max_input_age_ms > MAX_SESSION_MS
        || policy.max_trigger_lifetime_ms == 0
        || policy.max_trigger_lifetime_ms > MAX_TRIGGER_LIFETIME_MS
        || usize::from(policy.max_runs_per_minute) > MAX_RUNS_PER_MINUTE
        || usize::from(policy.max_runs_per_hour) > MAX_RUNS_PER_HOUR
        || policy.max_runs_per_minute == 0
        || policy.max_runs_per_hour == 0
        || policy.max_retrieval_attempts == 0
        || policy.max_retrieval_attempts > MAX_RETRIEVAL_ATTEMPTS
        || policy.max_model_tokens_per_run == 0
        || policy.max_model_tokens_per_session < policy.max_model_tokens_per_run
        || !is_currency_code(&policy.cost_currency_code)
        || policy.max_cost_micros_per_run == 0
        || policy.max_cost_micros_per_session < policy.max_cost_micros_per_run
        || policy
            .allowed_topics
            .iter()
            .any(|topic| !is_canonical_topic(topic))
    {
        return Err(AgentBlockReason::InvalidPolicy);
    }
    if policy.mode == AgentMode::ProactiveBounded && policy.allowed_topics.is_empty() {
        return Err(AgentBlockReason::InvalidPolicy);
    }
    Ok(())
}

fn validate_mode_and_topic(
    policy: &AgentSessionPolicy,
    input: &CommittedAgentInput,
) -> Result<(), AgentBlockReason> {
    let trigger_allowed = match policy.mode {
        AgentMode::Off | AgentMode::Observe => return Err(AgentBlockReason::ModeDisabled),
        AgentMode::Draft => matches!(
            input.trigger_kind,
            TriggerKind::ExplicitQuestion
                | TriggerKind::ContextMention
                | TriggerKind::AuthorizedUiAsk
        ),
        AgentMode::AskedOnly => match input.trigger_kind {
            TriggerKind::ExplicitQuestion => input.addressed_to_agent,
            TriggerKind::AuthorizedUiAsk => true,
            TriggerKind::ContextMention => false,
        },
        AgentMode::ProactiveBounded => matches!(
            input.trigger_kind,
            TriggerKind::ExplicitQuestion | TriggerKind::ContextMention
        ),
    };
    if !trigger_allowed {
        return Err(AgentBlockReason::TriggerNotAllowed);
    }
    match input.topic_id.as_deref() {
        Some(topic) if is_canonical_topic(topic) => {
            if policy.mode == AgentMode::ProactiveBounded && !policy.allowed_topics.contains(topic)
            {
                return Err(AgentBlockReason::TopicNotAllowed);
            }
            if !policy.allowed_topics.is_empty() && !policy.allowed_topics.contains(topic) {
                return Err(AgentBlockReason::TopicNotAllowed);
            }
        }
        Some(_) => return Err(AgentBlockReason::TopicNotAllowed),
        None if policy.mode == AgentMode::ProactiveBounded => {
            return Err(AgentBlockReason::TopicMissing)
        }
        None => {}
    }
    Ok(())
}

fn validate_input(
    policy: &AgentSessionPolicy,
    input: &CommittedAgentInput,
    authority: &CurrentInputAuthority,
    now_ms: u64,
) -> Result<CommittedAgentInputRef, AgentBlockReason> {
    if input.commit_sequence == 0
        || !input.is_final
        || input.source_generation == 0
        || input.source_sequence == 0
        || !is_sha256(&input.session_ref_hash)
        || !is_sha256(&input.source_ref_hash)
        || !is_sha256(&input.event_ref_hash)
        || !is_sha256(&input.revision_ref_hash)
        || input.session_ref_hash != policy.session_ref_hash
        || input.text.trim().is_empty()
        || input.text.chars().count() > MAX_INPUT_CHARS
        || input.text.chars().any(char::is_control)
    {
        return Err(AgentBlockReason::InvalidInput);
    }
    if input.committed_at_ms > now_ms
        || input.expires_at_ms <= now_ms
        || input.expires_at_ms <= input.committed_at_ms
        || input.expires_at_ms.saturating_sub(input.committed_at_ms)
            > policy.max_trigger_lifetime_ms
    {
        return Err(AgentBlockReason::InputExpired);
    }
    let age = now_ms.saturating_sub(input.committed_at_ms);
    if age > policy.max_input_age_ms {
        return Err(AgentBlockReason::InputStale);
    }
    if age < MIN_COMMIT_DEBOUNCE_MS && input.trigger_kind != TriggerKind::AuthorizedUiAsk {
        return Err(AgentBlockReason::DebouncePending);
    }
    match (&input.source_kind, &input.actor) {
        (
            InputSourceKind::Transcript | InputSourceKind::MeetingChat,
            InputActor::HumanParticipant {
                participant_session_ref_hash,
                is_self,
                is_agent,
            },
        ) if is_sha256(participant_session_ref_hash) && !*is_self && !*is_agent => {}
        (_, InputActor::HumanParticipant { is_self: true, .. })
        | (_, InputActor::HumanParticipant { is_agent: true, .. }) => {
            return Err(AgentBlockReason::InputFromSelfOrAgent)
        }
        (
            InputSourceKind::AuthorizedUiAction,
            InputActor::AuthorizedOperator {
                authorized: true,
                actor_ref_hash,
            },
        ) if is_sha256(actor_ref_hash) => {}
        (InputSourceKind::AuthorizedUiAction, InputActor::AuthorizedOperator { .. }) => {
            return Err(AgentBlockReason::UnauthorisedOperator)
        }
        (_, InputActor::Unknown) => return Err(AgentBlockReason::UnknownInputActor),
        _ => return Err(AgentBlockReason::InvalidInput),
    }
    if input.source_kind == InputSourceKind::AuthorizedUiAction
        && input.trigger_kind != TriggerKind::AuthorizedUiAsk
    {
        return Err(AgentBlockReason::TriggerNotAllowed);
    }
    if authority.can_read.is_none() {
        return Err(AgentBlockReason::InputAclUnknown);
    }
    if authority.can_read != Some(true) {
        return Err(AgentBlockReason::InputAclDenied);
    }
    if authority.source_ref_hash != input.source_ref_hash
        || authority.revision_ref_hash.as_deref() != Some(input.revision_ref_hash.as_str())
    {
        return Err(AgentBlockReason::SourceRevisionMismatch);
    }
    validate_read_grant(
        &input.read_grant_ref_hash,
        input.read_grant_revision,
        &policy.read_grant_ref_hash,
        policy.read_grant_revision,
        authority.read_grant_ref_hash.as_deref(),
        authority.read_grant_revision,
    )?;
    let input_hash = sha256_hex(input.text.as_bytes());
    Ok(CommittedAgentInputRef {
        session_ref_hash: input.session_ref_hash.clone(),
        source_ref_hash: input.source_ref_hash.clone(),
        event_ref_hash: input.event_ref_hash.clone(),
        revision_ref_hash: input.revision_ref_hash.clone(),
        source_generation: input.source_generation,
        source_sequence: input.source_sequence,
        commit_sequence: input.commit_sequence,
        is_final: input.is_final,
        committed_at_ms: input.committed_at_ms,
        expires_at_ms: input.expires_at_ms,
        source_kind: input.source_kind,
        trigger_kind: input.trigger_kind,
        topic_id: input.topic_id.clone(),
        input_hash,
        read_grant_ref_hash: input.read_grant_ref_hash.clone(),
        read_grant_revision: input.read_grant_revision,
    })
}

fn validate_input_ref_authority(
    input: &CommittedAgentInputRef,
    policy_grant_ref_hash: &str,
    policy_grant_revision: u64,
    authority: &CurrentInputAuthority,
) -> Result<(), AgentBlockReason> {
    if authority.can_read.is_none() {
        return Err(AgentBlockReason::InputAclUnknown);
    }
    if authority.can_read != Some(true) {
        return Err(AgentBlockReason::InputAclDenied);
    }
    if authority.source_ref_hash != input.source_ref_hash
        || authority.revision_ref_hash.as_deref() != Some(input.revision_ref_hash.as_str())
    {
        return Err(AgentBlockReason::SourceRevisionMismatch);
    }
    validate_read_grant(
        &input.read_grant_ref_hash,
        input.read_grant_revision,
        policy_grant_ref_hash,
        policy_grant_revision,
        authority.read_grant_ref_hash.as_deref(),
        authority.read_grant_revision,
    )
}

fn validate_evidence(
    policy: &AgentSessionPolicy,
    evidence: &[AgentEvidenceInput],
    authority: &[CurrentEvidenceAuthority],
    now_ms: u64,
) -> Result<(), AgentBlockReason> {
    if evidence.is_empty() {
        return Ok(());
    }
    if evidence.len() != authority.len() {
        return Err(AgentBlockReason::EvidenceUnknown);
    }
    let mut seen = BTreeSet::new();
    for item in evidence {
        if !seen.insert(item.evidence_ref_hash.as_str()) {
            return Err(AgentBlockReason::DuplicateEvidence);
        }
        if !is_sha256(&item.evidence_ref_hash)
            || !is_sha256(&item.source_ref_hash)
            || !is_sha256(&item.source_version_hash)
            || !is_sha256(&item.citation_hash)
            || !is_sha256(&item.read_grant_ref_hash)
            || item.evidence_revision == 0
            || item.read_grant_revision == 0
            || item.captured_at_ms > now_ms
        {
            return Err(AgentBlockReason::EvidenceUnknown);
        }
        if now_ms.saturating_sub(item.captured_at_ms) > policy.max_input_age_ms {
            return Err(AgentBlockReason::EvidenceStale);
        }
        let current = authority
            .iter()
            .find(|current| current.evidence_ref_hash == item.evidence_ref_hash)
            .ok_or(AgentBlockReason::EvidenceUnknown)?;
        if current.can_read.is_none()
            || current.source_ref_hash.is_none()
            || current.source_version_hash.is_none()
            || current.evidence_revision.is_none()
            || current.read_grant_ref_hash.is_none()
            || current.read_grant_revision.is_none()
        {
            return Err(AgentBlockReason::EvidenceAclUnknown);
        }
        if current.can_read != Some(true) {
            return Err(AgentBlockReason::EvidenceAclDenied);
        }
        if current.source_version_hash.as_deref() != Some(item.source_version_hash.as_str())
            || current.source_ref_hash.as_deref() != Some(item.source_ref_hash.as_str())
            || current.evidence_revision != Some(item.evidence_revision)
        {
            return Err(AgentBlockReason::SourceRevisionMismatch);
        }
        validate_read_grant(
            &item.read_grant_ref_hash,
            item.read_grant_revision,
            &item.read_grant_ref_hash,
            item.read_grant_revision,
            current.read_grant_ref_hash.as_deref(),
            current.read_grant_revision,
        )?;
    }
    Ok(())
}

fn validate_read_grant(
    captured_ref_hash: &str,
    captured_revision: u64,
    policy_ref_hash: &str,
    policy_revision: u64,
    current_ref_hash: Option<&str>,
    current_revision: Option<u64>,
) -> Result<(), AgentBlockReason> {
    if current_ref_hash.is_none() || current_revision.is_none() {
        return Err(AgentBlockReason::InputAclUnknown);
    }
    if captured_ref_hash != policy_ref_hash
        || captured_revision != policy_revision
        || current_ref_hash != Some(captured_ref_hash)
        || current_revision != Some(captured_revision)
    {
        return Err(AgentBlockReason::GrantRevisionMismatch);
    }
    Ok(())
}

fn validate_draft_body(body: &str) -> Result<(), AgentBlockReason> {
    if body.trim().is_empty()
        || body.chars().count() > MAX_DRAFT_CHARS
        || body.chars().any(|character| character == '\0')
    {
        return Err(AgentBlockReason::InvalidCompletion);
    }
    Ok(())
}

fn context_key(topic: Option<&str>, text: &str) -> String {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    sha256_hex(format!("{}\u{1}{normalized}", topic.unwrap_or("-")).as_bytes())
}

fn is_canonical_topic(topic: &str) -> bool {
    if topic.is_empty() || topic.len() > 64 {
        return false;
    }
    let bytes = topic.as_bytes();
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(byte))
        && !matches!(bytes[0], b'.' | b'-' | b'_')
        && !matches!(bytes[bytes.len() - 1], b'.' | b'-' | b'_')
}

fn is_currency_code(currency: &str) -> bool {
    currency.len() == 3 && currency.bytes().all(|byte| byte.is_ascii_uppercase())
}

pub(crate) fn topic_ref_hash(topic: &str) -> Result<String, AgentBlockReason> {
    if !is_canonical_topic(topic) {
        return Err(AgentBlockReason::TopicNotAllowed);
    }
    Ok(sha256_hex(topic.as_bytes()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: u64 = 100_000;

    fn hash(value: &str) -> String {
        sha256_hex(value.as_bytes())
    }

    fn policy(mode: AgentMode) -> AgentSessionPolicy {
        AgentSessionPolicy {
            session_ref_hash: hash("session"),
            policy_revision: 1,
            mode,
            enabled_at_ms: 1,
            expires_at_ms: 10_000_000,
            meeting_ends_at_ms: Some(10_000_000),
            read_grant_ref_hash: hash("read-grant"),
            read_grant_revision: 1,
            allowed_topics: BTreeSet::from(["finance".to_string()]),
            max_input_age_ms: 30_000,
            max_trigger_lifetime_ms: MAX_TRIGGER_LIFETIME_MS,
            max_runs_per_minute: 2,
            max_runs_per_hour: 20,
            max_retrieval_attempts: 3,
            max_model_tokens_per_run: 500,
            max_model_tokens_per_session: 1_000,
            cost_currency_code: "USD".to_string(),
            max_cost_micros_per_run: 500,
            max_cost_micros_per_session: 1_000,
        }
    }

    fn input(trigger_kind: TriggerKind, text: &str, source_sequence: u64) -> CommittedAgentInput {
        let committed_at_ms = NOW_MS - 3_000;
        CommittedAgentInput {
            session_ref_hash: hash("session"),
            source_ref_hash: hash("transcript-source"),
            event_ref_hash: hash(&format!("event-{source_sequence}")),
            revision_ref_hash: hash(&format!("revision-{source_sequence}")),
            source_generation: 1,
            source_sequence,
            commit_sequence: source_sequence,
            is_final: true,
            committed_at_ms,
            expires_at_ms: committed_at_ms + MAX_TRIGGER_LIFETIME_MS,
            source_kind: InputSourceKind::Transcript,
            actor: InputActor::HumanParticipant {
                participant_session_ref_hash: hash("participant"),
                is_self: false,
                is_agent: false,
            },
            trigger_kind,
            addressed_to_agent: trigger_kind == TriggerKind::ExplicitQuestion,
            topic_id: Some("finance".to_string()),
            text: text.to_string(),
            read_grant_ref_hash: hash("read-grant"),
            read_grant_revision: 1,
        }
    }

    fn input_authority(input: &CommittedAgentInput) -> CurrentInputAuthority {
        CurrentInputAuthority {
            source_ref_hash: input.source_ref_hash.clone(),
            revision_ref_hash: Some(input.revision_ref_hash.clone()),
            read_grant_ref_hash: Some(input.read_grant_ref_hash.clone()),
            read_grant_revision: Some(input.read_grant_revision),
            can_read: Some(true),
        }
    }

    fn evidence() -> (AgentEvidenceInput, CurrentEvidenceAuthority) {
        let item = AgentEvidenceInput {
            evidence_ref_hash: hash("evidence"),
            source_ref_hash: hash("source"),
            source_version_hash: hash("source-version-1"),
            evidence_revision: 1,
            citation_hash: hash("citation"),
            read_grant_ref_hash: hash("evidence-read-grant"),
            read_grant_revision: 2,
            captured_at_ms: NOW_MS - 2_000,
        };
        let current = CurrentEvidenceAuthority {
            evidence_ref_hash: item.evidence_ref_hash.clone(),
            source_ref_hash: Some(item.source_ref_hash.clone()),
            source_version_hash: Some(item.source_version_hash.clone()),
            evidence_revision: Some(item.evidence_revision),
            read_grant_ref_hash: Some(item.read_grant_ref_hash.clone()),
            read_grant_revision: Some(item.read_grant_revision),
            can_read: Some(true),
        };
        (item, current)
    }

    fn quote(amount_micros: u64) -> AgentCostQuote {
        AgentCostQuote {
            quote_ref_hash: hash("local-cost-quote"),
            currency_code: "USD".to_string(),
            upper_bound_micros: amount_micros,
            valid_until_ms: NOW_MS + 50_000,
        }
    }

    fn begin(
        coordinator: &mut MeetingAgentCoordinator,
        policy: &AgentSessionPolicy,
        input: CommittedAgentInput,
        evidence: Vec<AgentEvidenceInput>,
        authority: Vec<CurrentEvidenceAuthority>,
        quote: Option<AgentCostQuote>,
        now_ms: u64,
    ) -> Result<AgentRunPermit, AgentBlockReason> {
        let input_authority = input_authority(&input);
        coordinator.begin_run(
            policy,
            input,
            &input_authority,
            evidence,
            &authority,
            quote,
            100,
            2,
            now_ms,
        )
    }

    #[test]
    fn final_committed_input_and_current_evidence_can_create_a_private_draft() {
        let policy = policy(AgentMode::Draft);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        let source_input = input(
            TriggerKind::ExplicitQuestion,
            "What was last year's revenue?",
            1,
        );
        let current_input = input_authority(&source_input);
        let (evidence, current_evidence) = evidence();
        let permit = begin(
            &mut coordinator,
            &policy,
            source_input,
            vec![evidence.clone()],
            vec![current_evidence.clone()],
            Some(quote(100)),
            NOW_MS,
        )
        .unwrap();
        let draft = coordinator
            .finish_run(
                &policy,
                AgentRunCompletion {
                    run_id: permit.run_id,
                    now_ms: NOW_MS + 1_000,
                    retrieval_attempts: 1,
                    model_tokens_used: 80,
                    actual_cost_micros: Some(70),
                    current_input_authority: current_input,
                    current_evidence_authority: vec![current_evidence],
                    content: LocalDraftContent::Grounded {
                        body: "The cited report shows the requested figure.".to_string(),
                        citations: vec![evidence.citation_hash],
                    },
                },
            )
            .unwrap();
        assert_eq!(
            draft.body_for_encryption(),
            "The cited report shows the requested figure."
        );
        assert_eq!(draft.evidence.len(), 1);
    }

    #[test]
    fn provisional_self_and_unknown_acl_inputs_fail_closed() {
        let policy = policy(AgentMode::Draft);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        let mut provisional = input(TriggerKind::ExplicitQuestion, "question", 1);
        provisional.is_final = false;
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                provisional,
                vec![],
                vec![],
                Some(quote(10)),
                NOW_MS
            )
            .err(),
            Some(AgentBlockReason::InvalidInput)
        );

        let mut self_input = input(TriggerKind::ExplicitQuestion, "question", 2);
        self_input.actor = InputActor::HumanParticipant {
            participant_session_ref_hash: hash("self"),
            is_self: true,
            is_agent: false,
        };
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                self_input,
                vec![],
                vec![],
                Some(quote(10)),
                NOW_MS
            )
            .err(),
            Some(AgentBlockReason::InputFromSelfOrAgent)
        );

        let current = CurrentInputAuthority {
            can_read: None,
            ..input_authority(&input(TriggerKind::ExplicitQuestion, "question", 3))
        };
        let source_input = input(TriggerKind::ExplicitQuestion, "question", 3);
        assert_eq!(
            coordinator
                .begin_run(
                    &policy,
                    source_input,
                    &current,
                    vec![],
                    &[],
                    Some(quote(10)),
                    100,
                    1,
                    NOW_MS,
                )
                .err(),
            Some(AgentBlockReason::InputAclUnknown)
        );
    }

    #[test]
    fn asked_only_rejects_context_and_source_revision_changes() {
        let policy = policy(AgentMode::AskedOnly);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                input(TriggerKind::ContextMention, "mention", 1),
                vec![],
                vec![],
                Some(quote(10)),
                NOW_MS,
            )
            .err(),
            Some(AgentBlockReason::TriggerNotAllowed)
        );

        let source_input = input(TriggerKind::ExplicitQuestion, "question", 2);
        let mut authority = input_authority(&source_input);
        authority.revision_ref_hash = Some(hash("newer-revision"));
        assert_eq!(
            coordinator
                .begin_run(
                    &policy,
                    source_input,
                    &authority,
                    vec![],
                    &[],
                    Some(quote(10)),
                    100,
                    1,
                    NOW_MS,
                )
                .err(),
            Some(AgentBlockReason::SourceRevisionMismatch)
        );
    }

    #[test]
    fn proactive_runs_require_allowlisted_topics_and_context_cooldown() {
        let policy = policy(AgentMode::ProactiveBounded);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        let mut unapproved = input(TriggerKind::ContextMention, "new revenue topic", 1);
        unapproved.topic_id = Some("hiring".to_string());
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                unapproved,
                vec![],
                vec![],
                Some(quote(10)),
                NOW_MS
            )
            .err(),
            Some(AgentBlockReason::TopicNotAllowed)
        );

        let first = input(TriggerKind::ContextMention, "same mention", 2);
        let permit = begin(
            &mut coordinator,
            &policy,
            first,
            vec![],
            vec![],
            Some(quote(10)),
            NOW_MS,
        )
        .unwrap();
        coordinator.cancel_run(permit.run_id, Some(0)).unwrap();
        let second = input(TriggerKind::ContextMention, "same   mention", 3);
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                second,
                vec![],
                vec![],
                Some(quote(10)),
                NOW_MS + 1_000,
            )
            .err(),
            Some(AgentBlockReason::Cooldown)
        );
    }

    #[test]
    fn active_run_rate_limit_retrieval_and_cost_caps_are_enforced() {
        let policy = policy(AgentMode::Draft);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        let first = begin(
            &mut coordinator,
            &policy,
            input(TriggerKind::ExplicitQuestion, "first", 1),
            vec![],
            vec![],
            Some(quote(100)),
            NOW_MS,
        )
        .unwrap();
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                input(TriggerKind::ExplicitQuestion, "second", 2),
                vec![],
                vec![],
                Some(quote(100)),
                NOW_MS,
            )
            .err(),
            Some(AgentBlockReason::ActiveRunExists)
        );
        coordinator.cancel_run(first.run_id, Some(0)).unwrap();
        let second = begin(
            &mut coordinator,
            &policy,
            input(TriggerKind::ExplicitQuestion, "second", 2),
            vec![],
            vec![],
            Some(quote(100)),
            NOW_MS,
        )
        .unwrap();
        coordinator.cancel_run(second.run_id, Some(0)).unwrap();
        assert_eq!(
            begin(
                &mut coordinator,
                &policy,
                input(TriggerKind::ExplicitQuestion, "third", 3),
                vec![],
                vec![],
                Some(quote(100)),
                NOW_MS,
            )
            .err(),
            Some(AgentBlockReason::RateLimit)
        );

        let mut fresh_coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        assert_eq!(
            begin(
                &mut fresh_coordinator,
                &policy,
                input(TriggerKind::ExplicitQuestion, "cost cap", 4),
                vec![],
                vec![],
                Some(quote(501)),
                NOW_MS,
            )
            .err(),
            Some(AgentBlockReason::CostLimit)
        );
    }

    #[test]
    fn evidence_acl_or_source_revision_mismatch_prevents_a_run() {
        let policy = policy(AgentMode::Draft);
        let mut coordinator = MeetingAgentCoordinator::new(hash("session")).unwrap();
        let source_input = input(TriggerKind::ExplicitQuestion, "question", 1);
        let (evidence, mut current) = evidence();
        current.source_version_hash = Some(hash("stale-source-version"));
        assert_eq!(
            coordinator
                .begin_run(
                    &policy,
                    source_input,
                    &input_authority(&input(TriggerKind::ExplicitQuestion, "question", 1)),
                    vec![evidence],
                    &[current],
                    Some(quote(10)),
                    100,
                    1,
                    NOW_MS,
                )
                .err(),
            Some(AgentBlockReason::SourceRevisionMismatch)
        );
    }
}
