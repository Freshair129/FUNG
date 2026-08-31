//! Trust foundation for controlled external MCP retrieval.
//! @req FR-108, FR-112, FR-113, NFR-103, NFR-107, NFR-108
//! @tested tests/externalMeetingTools.test.mjs
//!
//! Sprint 2 added default-deny policy, exact preview hashing, minimization,
//! keyring lifecycle, structured audit, and untrusted-result sanitization.
//! Sprint 3 consumes this pure trust layer from sibling transport and command
//! modules; this file itself intentionally owns no process or Tauri boundary.

use genesis_block_native::Storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "FUNG";
const KEYRING_ACCOUNT_PREFIX: &str = "external-mcp/";

pub(crate) struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub(crate) fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CredentialReference {
    pub(crate) service: String,
    pub(crate) account: String,
}

pub(crate) trait ConnectorCredentialStore {
    fn set(&mut self, account: &str, secret: &str) -> Result<(), String>;
    /// Part of the credential contract and exercised by its tests. Execution
    /// resolves secrets through `resolve_connector_credential`, so no
    /// production path calls this directly yet.
    #[allow(dead_code)]
    fn get(&mut self, account: &str) -> Result<Option<String>, String>;
    fn delete(&mut self, account: &str) -> Result<(), String>;
}

pub(crate) struct OsConnectorCredentialStore;

impl ConnectorCredentialStore for OsConnectorCredentialStore {
    fn set(&mut self, account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(KEYRING_SERVICE, account)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|error| error.to_string())
    }

    fn get(&mut self, account: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(KEYRING_SERVICE, account).and_then(|entry| entry.get_password()) {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn delete(&mut self, account: &str) -> Result<(), String> {
        match keyring::Entry::new(KEYRING_SERVICE, account)
            .and_then(|entry| entry.delete_credential())
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn credential_reference(connector_id: &str) -> Result<CredentialReference, ExternalMcpErrorCode> {
    if connector_id.is_empty()
        || connector_id.len() > 128
        || !connector_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ExternalMcpErrorCode::KeyringUnavailable);
    }
    Ok(CredentialReference {
        service: KEYRING_SERVICE.into(),
        account: format!("{KEYRING_ACCOUNT_PREFIX}{connector_id}"),
    })
}

fn validate_credential_reference(
    reference: &CredentialReference,
) -> Result<(), ExternalMcpErrorCode> {
    let connector_id = reference
        .account
        .strip_prefix(KEYRING_ACCOUNT_PREFIX)
        .ok_or(ExternalMcpErrorCode::KeyringUnavailable)?;
    if credential_reference(connector_id)? != *reference {
        return Err(ExternalMcpErrorCode::KeyringUnavailable);
    }
    Ok(())
}

pub(crate) fn store_connector_credential(
    store: &mut impl ConnectorCredentialStore,
    connector_id: &str,
    secret: SecretValue,
) -> Result<CredentialReference, ExternalMcpErrorCode> {
    if secret.expose().is_empty() {
        return Err(ExternalMcpErrorCode::KeyringUnavailable);
    }
    let reference = credential_reference(connector_id)?;
    store
        .set(&reference.account, secret.expose())
        .map_err(|_| ExternalMcpErrorCode::KeyringUnavailable)?;
    Ok(reference)
}

/// Retained with the credential contract: no connector requiring a secret is
/// wired yet, so nothing calls this outside its tests.
#[allow(dead_code)]
pub(crate) fn resolve_connector_credential(
    store: &mut impl ConnectorCredentialStore,
    reference: &CredentialReference,
) -> Result<Option<SecretValue>, ExternalMcpErrorCode> {
    validate_credential_reference(reference)?;
    store
        .get(&reference.account)
        .map(|secret| secret.map(SecretValue::new))
        .map_err(|_| ExternalMcpErrorCode::KeyringUnavailable)
}

pub(crate) fn disconnect_connector_credential(
    store: &mut impl ConnectorCredentialStore,
    reference: &CredentialReference,
) -> Result<(), ExternalMcpErrorCode> {
    validate_credential_reference(reference)?;
    store
        .delete(&reference.account)
        .map_err(|_| ExternalMcpErrorCode::KeyringUnavailable)
}

pub(crate) fn disconnect_connector(
    storage: &Storage,
    store: &mut impl ConnectorCredentialStore,
    connector_id: &str,
    reference: &CredentialReference,
    disconnected_at: &str,
) -> Result<usize, ExternalMcpErrorCode> {
    let connector = crate::genesis_adapter::query(
        storage,
        "external_connections",
        &[
            "id",
            "provider",
            "account_label",
            "status",
            "transport",
            "endpoint",
            "credential_ref",
            "capabilities_json",
            "created_at",
            "updated_at",
        ],
        vec![crate::genesis_adapter::eq(
            "external_connections",
            "id",
            serde_json::json!(connector_id),
        )],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .into_iter()
    .next()
    .ok_or(ExternalMcpErrorCode::ConnectorNotFound)?;

    let grants = crate::genesis_adapter::query(
        storage,
        "meeting_tool_grants",
        &[
            "id",
            "project_id",
            "recording_id",
            "connector_id",
            "capabilities_json",
            "granted_at",
            "expires_at",
            "revoked_at",
        ],
        vec![crate::genesis_adapter::eq(
            "meeting_tool_grants",
            "connector_id",
            serde_json::json!(connector_id),
        )],
        1_000,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;

    let mut mutations = grants
        .iter()
        .filter(|row| row["meeting_tool_grants.revoked_at"].is_null())
        .map(|row| {
            crate::genesis_adapter::upsert(
                "meeting_tool_grants",
                serde_json::json!({
                    "id": row["meeting_tool_grants.id"],
                    "project_id": row["meeting_tool_grants.project_id"],
                    "recording_id": row["meeting_tool_grants.recording_id"],
                    "connector_id": row["meeting_tool_grants.connector_id"],
                    "capabilities_json": row["meeting_tool_grants.capabilities_json"],
                    "granted_at": row["meeting_tool_grants.granted_at"],
                    "expires_at": row["meeting_tool_grants.expires_at"],
                    "revoked_at": disconnected_at,
                }),
            )
        })
        .collect::<Vec<_>>();
    let revoked_count = mutations.len();

    let disconnected_connector = |credential_ref: serde_json::Value| {
        crate::genesis_adapter::upsert(
            "external_connections",
            serde_json::json!({
                "id": connector["external_connections.id"],
                "provider": connector["external_connections.provider"],
                "account_label": connector["external_connections.account_label"],
                "status": "disconnected",
                "transport": connector["external_connections.transport"],
                "endpoint": connector["external_connections.endpoint"],
                "credential_ref": credential_ref,
                "capabilities_json": connector["external_connections.capabilities_json"],
                "created_at": connector["external_connections.created_at"],
                "updated_at": disconnected_at,
            }),
        )
    };

    mutations.push(disconnected_connector(
        connector["external_connections.credential_ref"].clone(),
    ));
    crate::genesis_adapter::commit_rows(storage, mutations)
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;

    disconnect_connector_credential(store, reference)?;
    crate::genesis_adapter::commit_rows(
        storage,
        vec![disconnected_connector(serde_json::Value::Null)],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;

    Ok(revoked_count)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MeetingToolGrant {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) connector_id: String,
    pub(crate) capabilities: Vec<ConnectorCapability>,
    pub(crate) granted_at: String,
    pub(crate) expires_at: String,
    pub(crate) revoked_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ConnectorTransport {
    #[serde(rename = "stdio")]
    Stdio,
    #[serde(rename = "streamable_http")]
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ConnectorCapability {
    #[serde(rename = "documents.search")]
    DocumentsSearch,
    #[serde(rename = "documents.get_metadata")]
    DocumentsGetMetadata,
    #[serde(rename = "crm.customer_status.read")]
    CrmCustomerStatusRead,
}

impl ConnectorCapability {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentsSearch => "documents.search",
            Self::DocumentsGetMetadata => "documents.get_metadata",
            Self::CrmCustomerStatusRead => "crm.customer_status.read",
        }
    }

    fn from_advertised(value: &str) -> Result<Self, ExternalMcpErrorCode> {
        match value {
            "documents.search" => Ok(Self::DocumentsSearch),
            "documents.get_metadata" => Ok(Self::DocumentsGetMetadata),
            "crm.customer_status.read" => Ok(Self::CrmCustomerStatusRead),
            value
                if ["create", "update", "delete", "write", "send"]
                    .iter()
                    .any(|operation| value.split('.').any(|part| part == *operation)) =>
            {
                Err(ExternalMcpErrorCode::WriteToolDenied)
            }
            _ => Err(ExternalMcpErrorCode::CapabilityDenied),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreviewState {
    Suggested,
    Previewed,
    Denied,
    ApprovedOnce,
    Running,
    Completed,
    Failed,
    Cancelled,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExternalConnectorSummary {
    pub(crate) id: String,
    pub(crate) provider: String,
    pub(crate) account_label: String,
    pub(crate) status: String,
    pub(crate) transport: ConnectorTransport,
    pub(crate) endpoint: String,
    pub(crate) credential_ref: Option<String>,
    pub(crate) capabilities: Vec<ConnectorCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
// Specified in the external-retrieval contract. The operator UI drives
// preview/approve directly, so nothing constructs a suggestion yet.
#[allow(dead_code)]
pub(crate) struct MeetingToolSuggestion {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) question: String,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) requested_capability: ConnectorCapability,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MeetingToolPreview {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) connector_id: String,
    pub(crate) tool_name: String,
    pub(crate) capability: ConnectorCapability,
    pub(crate) arguments_hash: String,
    pub(crate) approved_fields: Vec<String>,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) state: PreviewState,
    pub(crate) expires_at: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExternalToolRun {
    pub(crate) id: String,
    pub(crate) preview_id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) connector_id: String,
    pub(crate) tool_name: String,
    pub(crate) capability: ConnectorCapability,
    pub(crate) request_hash: String,
    pub(crate) output_hash: Option<String>,
    pub(crate) status: ToolRunStatus,
    pub(crate) started_at: String,
    pub(crate) finished_at: Option<String>,
    pub(crate) error_code: Option<ExternalMcpErrorCode>,
    pub(crate) result_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExternalToolResult {
    pub(crate) id: String,
    pub(crate) run_id: String,
    pub(crate) mime_type: String,
    pub(crate) sanitized_payload: serde_json::Value,
    pub(crate) source_refs: Vec<String>,
    pub(crate) byte_size: u64,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ExternalMcpErrorCode {
    ConnectorNotFound,
    ConnectorUnhealthy,
    CapabilityDenied,
    ApprovalRequired,
    PreviewChanged,
    GrantExpired,
    GrantRevoked,
    WriteToolDenied,
    EgressFieldDenied,
    ToolTimeout,
    ToolCancelled,
    ResultTooLarge,
    ResultUnsafe,
    KeyringUnavailable,
}

impl ExternalMcpErrorCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ConnectorNotFound => "CONNECTOR_NOT_FOUND",
            Self::ConnectorUnhealthy => "CONNECTOR_UNHEALTHY",
            Self::CapabilityDenied => "CAPABILITY_DENIED",
            Self::ApprovalRequired => "APPROVAL_REQUIRED",
            Self::PreviewChanged => "PREVIEW_CHANGED",
            Self::GrantExpired => "GRANT_EXPIRED",
            Self::GrantRevoked => "GRANT_REVOKED",
            Self::WriteToolDenied => "WRITE_TOOL_DENIED",
            Self::EgressFieldDenied => "EGRESS_FIELD_DENIED",
            Self::ToolTimeout => "TOOL_TIMEOUT",
            Self::ToolCancelled => "TOOL_CANCELLED",
            Self::ResultTooLarge => "RESULT_TOO_LARGE",
            Self::ResultUnsafe => "RESULT_UNSAFE",
            Self::KeyringUnavailable => "KEYRING_UNAVAILABLE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
// Contract error envelope. Commands return `ExternalMcpErrorCode` directly,
// so the wrapper is unused until an surface needs the message alongside it.
#[allow(dead_code)]
pub(crate) struct ExternalMcpError {
    pub(crate) code: ExternalMcpErrorCode,
    pub(crate) message: String,
    pub(crate) capture_safe: bool,
    pub(crate) run_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Audit vocabulary from the contract; only the variants on the wired
// preview/run/result path are emitted so far.
#[allow(dead_code)]
pub(crate) enum ExternalAuditEventType {
    Suggestion,
    Preview,
    Denial,
    Approval,
    Execution,
    Cancellation,
    Timeout,
    Failure,
    Completion,
    Revocation,
    Disconnect,
}

impl ExternalAuditEventType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Suggestion => "external_tool.suggestion",
            Self::Preview => "external_tool.preview",
            Self::Denial => "external_tool.denial",
            Self::Approval => "external_tool.approval",
            Self::Execution => "external_tool.execution",
            Self::Cancellation => "external_tool.cancellation",
            Self::Timeout => "external_tool.timeout",
            Self::Failure => "external_tool.failure",
            Self::Completion => "external_tool.completion",
            Self::Revocation => "external_tool.revocation",
            Self::Disconnect => "external_tool.disconnect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalAuditActor {
    User,
    System,
}

impl ExternalAuditActor {
    const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

pub(crate) struct ExternalAuditEvent {
    pub(crate) project_id: String,
    pub(crate) event_type: ExternalAuditEventType,
    pub(crate) actor: ExternalAuditActor,
    pub(crate) correlation_id: String,
    pub(crate) recording_id: String,
    pub(crate) connector_id: Option<String>,
    pub(crate) capability: Option<ConnectorCapability>,
    pub(crate) tool_name: Option<String>,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) approved_fields: Vec<String>,
    pub(crate) request_hash: Option<String>,
    pub(crate) output_hash: Option<String>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) result_byte_count: Option<u64>,
    pub(crate) error_code: Option<ExternalMcpErrorCode>,
    pub(crate) created_at: String,
}

fn is_safe_audit_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
}

fn is_sha256_reference(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_external_audit(event: &ExternalAuditEvent) -> Result<(), ExternalMcpErrorCode> {
    let required_tokens = [
        event.project_id.as_str(),
        event.correlation_id.as_str(),
        event.recording_id.as_str(),
    ];
    let optional_tokens = [event.connector_id.as_deref(), event.tool_name.as_deref()];
    if required_tokens
        .iter()
        .any(|value| !is_safe_audit_token(value))
        || optional_tokens
            .iter()
            .flatten()
            .any(|value| !is_safe_audit_token(value))
        || event.evidence_refs.len() > 64
        || event.approved_fields.len() > 32
        || event
            .evidence_refs
            .iter()
            .any(|value| !is_safe_audit_token(value))
        || event
            .approved_fields
            .iter()
            .any(|value| !is_safe_audit_token(value) || is_sensitive_egress_field(value))
        || event
            .request_hash
            .iter()
            .chain(event.output_hash.iter())
            .any(|value| !is_sha256_reference(value))
        || chrono::DateTime::parse_from_rfc3339(&event.created_at).is_err()
    {
        return Err(ExternalMcpErrorCode::ResultUnsafe);
    }
    Ok(())
}

pub(crate) fn append_external_audit(
    storage: &Storage,
    event: &ExternalAuditEvent,
) -> Result<(), ExternalMcpErrorCode> {
    validate_external_audit(event)?;
    let payload = serde_json::json!({
        "correlationId": event.correlation_id,
        "recordingId": event.recording_id,
        "connectorId": event.connector_id,
        "capability": event.capability.map(ConnectorCapability::as_str),
        "toolName": event.tool_name,
        "evidenceRefs": sorted_unique(&event.evidence_refs),
        "approvedFields": sorted_unique(&event.approved_fields),
        "requestHash": event.request_hash,
        "outputHash": event.output_hash,
        "durationMs": event.duration_ms,
        "resultByteCount": event.result_byte_count,
        "errorCode": event.error_code.map(ExternalMcpErrorCode::as_str),
    });
    crate::genesis_adapter::commit_rows(
        storage,
        vec![crate::genesis_adapter::upsert(
            "audit_events",
            serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "project_id": event.project_id,
                "event_type": event.event_type.as_str(),
                "actor": event.actor.as_str(),
                "payload_json": payload,
                "created_at": event.created_at,
            }),
        )],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)
}

pub(crate) struct PolicyEvaluation<'a> {
    pub(crate) grant: Option<&'a MeetingToolGrant>,
    pub(crate) preview: &'a MeetingToolPreview,
    pub(crate) advertised_capability: &'a str,
    pub(crate) approved_preview_hash: Option<&'a str>,
    pub(crate) now: &'a str,
}

fn parse_policy_time(value: &str) -> Result<chrono::DateTime<chrono::Utc>, ExternalMcpErrorCode> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| ExternalMcpErrorCode::GrantExpired)
}

pub(crate) fn evaluate_policy(input: PolicyEvaluation<'_>) -> Result<(), ExternalMcpErrorCode> {
    let advertised = ConnectorCapability::from_advertised(input.advertised_capability)?;
    let grant = input.grant.ok_or(ExternalMcpErrorCode::CapabilityDenied)?;

    if grant.revoked_at.is_some() {
        return Err(ExternalMcpErrorCode::GrantRevoked);
    }

    let now = parse_policy_time(input.now)?;
    if parse_policy_time(&grant.granted_at)? > now {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    if parse_policy_time(&grant.expires_at)? <= now
        || parse_policy_time(&input.preview.expires_at)? <= now
    {
        return Err(ExternalMcpErrorCode::GrantExpired);
    }

    if grant.project_id != input.preview.project_id
        || grant.recording_id != input.preview.recording_id
        || grant.connector_id != input.preview.connector_id
        || !grant.capabilities.contains(&input.preview.capability)
        || advertised != input.preview.capability
    {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }

    if input.preview.state != PreviewState::Previewed {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }

    let approved_hash = input
        .approved_preview_hash
        .ok_or(ExternalMcpErrorCode::ApprovalRequired)?;
    if approved_hash != input.preview.arguments_hash {
        return Err(ExternalMcpErrorCode::PreviewChanged);
    }

    Ok(())
}

pub(crate) struct PreviewHashInput<'a> {
    pub(crate) connector_id: &'a str,
    pub(crate) tool_name: &'a str,
    pub(crate) capability: ConnectorCapability,
    pub(crate) arguments: &'a serde_json::Value,
    pub(crate) evidence_refs: &'a [String],
    pub(crate) approved_fields: &'a [String],
    pub(crate) project_id: &'a str,
    pub(crate) recording_id: &'a str,
    pub(crate) expires_at: &'a str,
}

fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&object[key]));
            }
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical_json).collect())
        }
        value => value.clone(),
    }
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
}

pub(crate) fn preview_hash(input: PreviewHashInput<'_>) -> String {
    let canonical = serde_json::json!({
        "approvedFields": sorted_unique(input.approved_fields),
        "arguments": canonical_json(input.arguments),
        "capability": input.capability.as_str(),
        "connectorId": input.connector_id,
        "evidenceRefs": sorted_unique(input.evidence_refs),
        "expiresAt": input.expires_at,
        "projectId": input.project_id,
        "recordingId": input.recording_id,
        "toolName": input.tool_name,
    });
    let encoded = serde_json::to_vec(&canonical).expect("preview hash input is JSON serializable");
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn normalized_field_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_sensitive_egress_field(field: &str) -> bool {
    matches!(
        normalized_field_name(field).as_str(),
        "audio"
            | "audiobytes"
            | "audiofile"
            | "fulltranscript"
            | "rawtranscript"
            | "transcript"
            | "authorization"
            | "authheader"
            | "credential"
            | "credentials"
            | "secret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
    )
}

fn contains_sensitive_egress_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            is_sensitive_egress_field(key) || contains_sensitive_egress_field(value)
        }),
        serde_json::Value::Array(items) => items.iter().any(contains_sensitive_egress_field),
        _ => false,
    }
}

pub(crate) fn minimize_arguments(
    arguments: &serde_json::Value,
    approved_fields: &[String],
) -> Result<serde_json::Value, ExternalMcpErrorCode> {
    let object = arguments
        .as_object()
        .ok_or(ExternalMcpErrorCode::EgressFieldDenied)?;
    if approved_fields.is_empty()
        || approved_fields
            .iter()
            .any(|field| is_sensitive_egress_field(field) || !object.contains_key(field.as_str()))
        || object.keys().any(|field| {
            !approved_fields.iter().any(|approved| approved == field)
                || is_sensitive_egress_field(field)
        })
        || contains_sensitive_egress_field(arguments)
    {
        return Err(ExternalMcpErrorCode::EgressFieldDenied);
    }

    Ok(canonical_json(arguments))
}

const MAX_EXTERNAL_RESULT_BYTES: usize = 256 * 1024;
const MAX_EXTERNAL_RESULT_DEPTH: usize = 16;
const MAX_EXTERNAL_RESULT_NODES: usize = 1_001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SanitizedExternalResult {
    pub(crate) payload: serde_json::Value,
    pub(crate) byte_size: u64,
    pub(crate) output_hash: String,
}

fn sanitize_external_string(value: &str) -> String {
    let without_controls = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
        .collect::<String>();
    let normalized = without_controls.trim().to_ascii_lowercase();
    if normalized.starts_with("file:")
        || normalized.starts_with("javascript:")
        || normalized.starts_with("data:")
    {
        return "[blocked-url]".into();
    }

    without_controls
        .replace('&', "&amp;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sanitize_external_value(
    value: &serde_json::Value,
    depth: usize,
    item_count: &mut usize,
) -> Result<serde_json::Value, ExternalMcpErrorCode> {
    *item_count += 1;
    if depth > MAX_EXTERNAL_RESULT_DEPTH || *item_count > MAX_EXTERNAL_RESULT_NODES {
        return Err(ExternalMcpErrorCode::ResultUnsafe);
    }

    match value {
        serde_json::Value::Object(object) => {
            let mut sanitized = serde_json::Map::new();
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if key.len() > 160 || key.chars().any(char::is_control) {
                    return Err(ExternalMcpErrorCode::ResultUnsafe);
                }
                sanitized.insert(
                    key.clone(),
                    sanitize_external_value(&object[key], depth + 1, item_count)?,
                );
            }
            Ok(serde_json::Value::Object(sanitized))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| sanitize_external_value(item, depth + 1, item_count))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        serde_json::Value::String(value) => {
            Ok(serde_json::Value::String(sanitize_external_string(value)))
        }
        value => Ok(value.clone()),
    }
}

pub(crate) fn sanitize_external_result(
    value: &serde_json::Value,
) -> Result<SanitizedExternalResult, ExternalMcpErrorCode> {
    let input = serde_json::to_vec(value).map_err(|_| ExternalMcpErrorCode::ResultUnsafe)?;
    if input.len() > MAX_EXTERNAL_RESULT_BYTES {
        return Err(ExternalMcpErrorCode::ResultTooLarge);
    }

    let mut item_count = 0;
    let payload = sanitize_external_value(value, 0, &mut item_count)?;
    let encoded = serde_json::to_vec(&payload).map_err(|_| ExternalMcpErrorCode::ResultUnsafe)?;
    if encoded.len() > MAX_EXTERNAL_RESULT_BYTES {
        return Err(ExternalMcpErrorCode::ResultTooLarge);
    }

    Ok(SanitizedExternalResult {
        payload,
        byte_size: encoded.len() as u64,
        output_hash: format!("sha256:{:x}", Sha256::digest(&encoded)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};

    fn open_storage() -> (tempfile::TempDir, Storage) {
        let directory = tempfile::tempdir().expect("temporary Genesis directory");
        let storage = Storage::open(OpenOptions {
            path: directory.path().display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .expect("open Genesis storage");
        crate::genesis_adapter::install(&storage).expect("install FUNG schema");
        (directory, storage)
    }

    #[test]
    fn contract_serializes_read_only_capability_and_preview_without_secret_fields() {
        let preview = MeetingToolPreview {
            id: "preview-1".into(),
            project_id: "project-1".into(),
            recording_id: "recording-1".into(),
            connector_id: "connector-1".into(),
            tool_name: "search_documents".into(),
            capability: ConnectorCapability::DocumentsSearch,
            arguments_hash: "sha256:abc".into(),
            approved_fields: vec!["query".into()],
            evidence_refs: vec!["segment-7".into()],
            state: PreviewState::Previewed,
            expires_at: "2026-08-11T04:00:00Z".into(),
            created_at: "2026-08-11T03:55:00Z".into(),
        };

        let value = serde_json::to_value(preview).expect("preview serializes");
        assert_eq!(value["capability"], "documents.search");
        assert_eq!(value["state"], "previewed");
        assert!(value.get("credential").is_none());
        assert!(value.get("secret").is_none());
    }

    #[test]
    fn contract_exposes_stable_default_deny_error_codes() {
        assert_eq!(
            ExternalMcpErrorCode::ApprovalRequired.as_str(),
            "APPROVAL_REQUIRED"
        );
        assert_eq!(
            ExternalMcpErrorCode::WriteToolDenied.as_str(),
            "WRITE_TOOL_DENIED"
        );
        assert_eq!(ExternalMcpErrorCode::GrantRevoked.as_str(), "GRANT_REVOKED");
        assert_eq!(
            ExternalMcpErrorCode::KeyringUnavailable.as_str(),
            "KEYRING_UNAVAILABLE"
        );
    }

    #[test]
    fn contract_rejects_unapproved_or_write_capabilities() {
        assert!(serde_json::from_str::<ConnectorCapability>("\"documents.create\"").is_err());
        assert!(serde_json::from_str::<ConnectorCapability>("\"crm.customer.update\"").is_err());
    }

    #[test]
    fn trust_layer_stays_independent_from_process_and_tauri_boundaries() {
        let source = include_str!("external_mcp.rs");
        let forbidden = [
            ["#[tauri", "::command]"].concat(),
            ["req", "west::"].concat(),
            ["Tcp", "Stream"].concat(),
            ["std::process::", "Command"].concat(),
        ];
        for forbidden in forbidden {
            assert!(
                !source.contains(&forbidden),
                "Sprint 1 contract unexpectedly contains execution backend: {forbidden}"
            );
        }
    }

    #[test]
    fn policy_matrix_defaults_to_deny_for_unsafe_preview_states() {
        let base_grant = MeetingToolGrant {
            id: "grant-1".into(),
            project_id: "project-1".into(),
            recording_id: "recording-1".into(),
            connector_id: "connector-1".into(),
            capabilities: vec![ConnectorCapability::DocumentsSearch],
            granted_at: "2026-08-11T03:00:00Z".into(),
            expires_at: "2026-08-11T05:00:00Z".into(),
            revoked_at: None,
        };
        let base_preview = MeetingToolPreview {
            id: "preview-1".into(),
            project_id: "project-1".into(),
            recording_id: "recording-1".into(),
            connector_id: "connector-1".into(),
            tool_name: "search_documents".into(),
            capability: ConnectorCapability::DocumentsSearch,
            arguments_hash: "sha256:approved".into(),
            approved_fields: vec!["query".into()],
            evidence_refs: vec!["segment-7".into()],
            state: PreviewState::Previewed,
            expires_at: "2026-08-11T04:30:00Z".into(),
            created_at: "2026-08-11T03:55:00Z".into(),
        };

        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: None,
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: None,
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::CapabilityDenied)
        );

        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&base_grant),
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: None,
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::ApprovalRequired)
        );

        let mut expired = base_grant.clone();
        expired.expires_at = "2026-08-11T03:59:59Z".into();
        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&expired),
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: Some("sha256:approved"),
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::GrantExpired)
        );

        let mut revoked = base_grant.clone();
        revoked.revoked_at = Some("2026-08-11T03:30:00Z".into());
        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&revoked),
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: Some("sha256:approved"),
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::GrantRevoked)
        );

        let mut not_yet_granted = base_grant.clone();
        not_yet_granted.granted_at = "2026-08-11T04:00:01Z".into();
        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&not_yet_granted),
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: Some("sha256:approved"),
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::CapabilityDenied)
        );

        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&base_grant),
                preview: &base_preview,
                advertised_capability: "documents.create",
                approved_preview_hash: Some("sha256:approved"),
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::WriteToolDenied)
        );

        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&base_grant),
                preview: &base_preview,
                advertised_capability: "documents.search",
                approved_preview_hash: Some("sha256:changed"),
                now: "2026-08-11T04:00:00Z",
            }),
            Err(ExternalMcpErrorCode::PreviewChanged)
        );
    }

    #[test]
    fn policy_allows_one_exact_approved_read_only_preview() {
        let grant = MeetingToolGrant {
            id: "grant-1".into(),
            project_id: "project-1".into(),
            recording_id: "recording-1".into(),
            connector_id: "connector-1".into(),
            capabilities: vec![ConnectorCapability::DocumentsSearch],
            granted_at: "2026-08-11T03:00:00Z".into(),
            expires_at: "2026-08-11T05:00:00Z".into(),
            revoked_at: None,
        };
        let preview = MeetingToolPreview {
            id: "preview-1".into(),
            project_id: "project-1".into(),
            recording_id: "recording-1".into(),
            connector_id: "connector-1".into(),
            tool_name: "search_documents".into(),
            capability: ConnectorCapability::DocumentsSearch,
            arguments_hash: "sha256:approved".into(),
            approved_fields: vec!["query".into()],
            evidence_refs: vec!["segment-7".into()],
            state: PreviewState::Previewed,
            expires_at: "2026-08-11T04:30:00Z".into(),
            created_at: "2026-08-11T03:55:00Z".into(),
        };

        assert_eq!(
            evaluate_policy(PolicyEvaluation {
                grant: Some(&grant),
                preview: &preview,
                advertised_capability: "documents.search",
                approved_preview_hash: Some("sha256:approved"),
                now: "2026-08-11T04:00:00Z",
            }),
            Ok(())
        );
    }

    #[test]
    fn preview_hash_is_canonical_and_binds_every_approved_dimension() {
        let evidence_a = vec!["segment-8".into(), "segment-7".into()];
        let evidence_b = vec!["segment-7".into(), "segment-8".into()];
        let fields_a = vec!["owner".into(), "query".into()];
        let fields_b = vec!["query".into(), "owner".into()];
        let arguments_a =
            serde_json::json!({"query":"contract", "filters":{"owner":"Sales", "year":2026}});
        let arguments_b =
            serde_json::json!({"filters":{"year":2026, "owner":"Sales"}, "query":"contract"});

        let base = preview_hash(PreviewHashInput {
            connector_id: "connector-1",
            tool_name: "search_documents",
            capability: ConnectorCapability::DocumentsSearch,
            arguments: &arguments_a,
            evidence_refs: &evidence_a,
            approved_fields: &fields_a,
            project_id: "project-1",
            recording_id: "recording-1",
            expires_at: "2026-08-11T04:30:00Z",
        });
        let reordered = preview_hash(PreviewHashInput {
            connector_id: "connector-1",
            tool_name: "search_documents",
            capability: ConnectorCapability::DocumentsSearch,
            arguments: &arguments_b,
            evidence_refs: &evidence_b,
            approved_fields: &fields_b,
            project_id: "project-1",
            recording_id: "recording-1",
            expires_at: "2026-08-11T04:30:00Z",
        });

        assert_eq!(base, reordered, "object/field/ref order must be canonical");
        assert!(base.starts_with("sha256:"));
        assert_eq!(base.len(), "sha256:".len() + 64);

        let changed = preview_hash(PreviewHashInput {
            connector_id: "connector-1",
            tool_name: "search_documents",
            capability: ConnectorCapability::DocumentsSearch,
            arguments: &serde_json::json!({"query":"different", "filters":{"owner":"Sales", "year":2026}}),
            evidence_refs: &evidence_a,
            approved_fields: &fields_a,
            project_id: "project-1",
            recording_id: "recording-1",
            expires_at: "2026-08-11T04:30:00Z",
        });
        assert_ne!(base, changed);
    }

    #[test]
    fn minimizer_allows_only_exact_approved_fields_and_rejects_sensitive_context() {
        assert_eq!(
            minimize_arguments(
                &serde_json::json!({"query":"เอกสารสัญญา"}),
                &["query".into()]
            ),
            Ok(serde_json::json!({"query":"เอกสารสัญญา"}))
        );

        assert_eq!(
            minimize_arguments(
                &serde_json::json!({"query":"contract", "limit":20}),
                &["query".into()]
            ),
            Err(ExternalMcpErrorCode::EgressFieldDenied)
        );
        assert_eq!(
            minimize_arguments(
                &serde_json::json!({"fullTranscript":"raw meeting"}),
                &["fullTranscript".into()]
            ),
            Err(ExternalMcpErrorCode::EgressFieldDenied)
        );
        assert_eq!(
            minimize_arguments(
                &serde_json::json!({"filters":{"authorization":"Bearer secret"}}),
                &["filters".into()]
            ),
            Err(ExternalMcpErrorCode::EgressFieldDenied)
        );
    }

    #[derive(Default)]
    struct FakeCredentialStore {
        values: std::collections::HashMap<String, String>,
        operations: Vec<String>,
    }

    impl ConnectorCredentialStore for FakeCredentialStore {
        fn set(&mut self, account: &str, secret: &str) -> Result<(), String> {
            self.values.insert(account.into(), secret.into());
            self.operations.push(format!("set:{account}"));
            Ok(())
        }

        fn get(&mut self, account: &str) -> Result<Option<String>, String> {
            self.operations.push(format!("get:{account}"));
            Ok(self.values.get(account).cloned())
        }

        fn delete(&mut self, account: &str) -> Result<(), String> {
            self.values.remove(account);
            self.operations.push(format!("delete:{account}"));
            Ok(())
        }
    }

    #[test]
    fn keyring_lifecycle_persists_only_reference_and_removes_secret_on_disconnect() {
        let mut store = FakeCredentialStore::default();
        let secret_text = "top-secret-connector-token";
        let reference = store_connector_credential(
            &mut store,
            "connector-1",
            SecretValue::new(secret_text.into()),
        )
        .expect("store credential");

        let serialized = serde_json::to_string(&reference).expect("reference serializes");
        assert!(!serialized.contains(secret_text));
        assert_eq!(reference.service, "FUNG");
        assert_eq!(reference.account, "external-mcp/connector-1");

        let resolved = resolve_connector_credential(&mut store, &reference)
            .expect("resolve credential")
            .expect("credential exists");
        assert_eq!(resolved.expose(), secret_text);
        drop(resolved);

        disconnect_connector_credential(&mut store, &reference).expect("delete credential");
        assert!(resolve_connector_credential(&mut store, &reference)
            .expect("resolve after delete")
            .is_none());
        assert_eq!(
            store.operations,
            [
                "set:external-mcp/connector-1",
                "get:external-mcp/connector-1",
                "delete:external-mcp/connector-1",
                "get:external-mcp/connector-1",
            ]
        );

        let malformed = CredentialReference {
            service: "FUNG".into(),
            account: "external-mcp/../../unexpected".into(),
        };
        assert!(matches!(
            resolve_connector_credential(&mut store, &malformed),
            Err(ExternalMcpErrorCode::KeyringUnavailable)
        ));
    }

    #[test]
    fn disconnect_revokes_only_target_connector_grants_and_marks_it_disconnected() {
        let (_directory, storage) = open_storage();
        crate::genesis_adapter::commit_rows(
            &storage,
            vec![
                crate::genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "project-1", "name": "MCP Test", "storage_path": "test-path",
                        "active_recording_id": null, "created_at": "2026-08-11T03:00:00Z",
                        "updated_at": "2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "recording-1", "project_id": "project-1", "source": "microphone",
                        "input_path": null, "canonical_audio_path": "recordings/recording-1.wav",
                        "status": "recording", "duration_ms": 0,
                        "created_at": "2026-08-11T03:00:00Z", "updated_at": "2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "external_connections",
                    serde_json::json!({
                        "id": "connector-1", "provider": "local-mcp", "account_label": "KB",
                        "status": "connected", "transport": "stdio", "endpoint": "approved-mcp",
                        "credential_ref": "{\"service\":\"FUNG\",\"account\":\"external-mcp/connector-1\"}",
                        "capabilities_json": ["documents.search"],
                        "created_at": "2026-08-11T03:00:00Z", "updated_at": "2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "external_connections",
                    serde_json::json!({
                        "id": "connector-2", "provider": "local-mcp", "account_label": "CRM",
                        "status": "connected", "transport": "stdio", "endpoint": "approved-crm",
                        "credential_ref": null, "capabilities_json": ["crm.customer_status.read"],
                        "created_at": "2026-08-11T03:00:00Z", "updated_at": "2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "meeting_tool_grants",
                    serde_json::json!({
                        "id": "grant-1", "project_id": "project-1", "recording_id": "recording-1",
                        "connector_id": "connector-1", "capabilities_json": ["documents.search"],
                        "granted_at": "2026-08-11T03:00:00Z", "expires_at": "2026-08-11T05:00:00Z",
                        "revoked_at": null
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "meeting_tool_grants",
                    serde_json::json!({
                        "id": "grant-2", "project_id": "project-1", "recording_id": "recording-1",
                        "connector_id": "connector-2", "capabilities_json": ["crm.customer_status.read"],
                        "granted_at": "2026-08-11T03:00:00Z", "expires_at": "2026-08-11T05:00:00Z",
                        "revoked_at": null
                    }),
                ),
            ],
        )
        .expect("seed connector grants");

        let mut keyring = FakeCredentialStore::default();
        let reference = store_connector_credential(
            &mut keyring,
            "connector-1",
            SecretValue::new("secret-value".into()),
        )
        .expect("seed keyring");

        let revoked = disconnect_connector(
            &storage,
            &mut keyring,
            "connector-1",
            &reference,
            "2026-08-11T04:00:00Z",
        )
        .expect("disconnect connector");
        assert_eq!(revoked, 1);
        assert!(resolve_connector_credential(&mut keyring, &reference)
            .expect("resolve after disconnect")
            .is_none());

        let grants = crate::genesis_adapter::query(
            &storage,
            "meeting_tool_grants",
            &["id", "revoked_at"],
            vec![],
            10,
        )
        .expect("query grants");
        let target = grants
            .iter()
            .find(|row| row["meeting_tool_grants.id"] == "grant-1")
            .expect("target grant");
        let other = grants
            .iter()
            .find(|row| row["meeting_tool_grants.id"] == "grant-2")
            .expect("other grant");
        assert_eq!(
            target["meeting_tool_grants.revoked_at"],
            "2026-08-11T04:00:00Z"
        );
        assert!(other["meeting_tool_grants.revoked_at"].is_null());

        let connector = crate::genesis_adapter::query(
            &storage,
            "external_connections",
            &["status", "credential_ref", "updated_at"],
            vec![crate::genesis_adapter::eq(
                "external_connections",
                "id",
                serde_json::json!("connector-1"),
            )],
            1,
        )
        .expect("query connector");
        assert_eq!(connector[0]["external_connections.status"], "disconnected");
        assert!(connector[0]["external_connections.credential_ref"].is_null());
        assert_eq!(
            connector[0]["external_connections.updated_at"],
            "2026-08-11T04:00:00Z"
        );
    }

    #[test]
    fn structured_audit_persists_provenance_without_secret_or_raw_content_fields() {
        let (_directory, storage) = open_storage();
        crate::genesis_adapter::commit_rows(
            &storage,
            vec![crate::genesis_adapter::upsert(
                "projects",
                serde_json::json!({
                    "id": "project-1", "name": "Audit Test", "storage_path": "test-path",
                    "active_recording_id": null, "created_at": "2026-08-11T03:00:00Z",
                    "updated_at": "2026-08-11T03:00:00Z"
                }),
            )],
        )
        .expect("seed audit project");

        append_external_audit(
            &storage,
            &ExternalAuditEvent {
                project_id: "project-1".into(),
                event_type: ExternalAuditEventType::Completion,
                actor: ExternalAuditActor::System,
                correlation_id: "run-1".into(),
                recording_id: "recording-1".into(),
                connector_id: Some("connector-1".into()),
                capability: Some(ConnectorCapability::DocumentsSearch),
                tool_name: Some("search_documents".into()),
                evidence_refs: vec!["segment-7".into()],
                approved_fields: vec!["query".into()],
                request_hash: Some(format!("sha256:{}", "a".repeat(64))),
                output_hash: Some(format!("sha256:{}", "b".repeat(64))),
                duration_ms: Some(120),
                result_byte_count: Some(42),
                error_code: None,
                created_at: "2026-08-11T04:00:00Z".into(),
            },
        )
        .expect("append external audit");

        let rows = crate::genesis_adapter::query(
            &storage,
            "audit_events",
            &["event_type", "actor", "payload_json", "created_at"],
            vec![],
            10,
        )
        .expect("query audit");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0]["audit_events.event_type"],
            "external_tool.completion"
        );
        assert_eq!(rows[0]["audit_events.actor"], "system");
        let payload = &rows[0]["audit_events.payload_json"];
        assert_eq!(payload["correlationId"], "run-1");
        assert_eq!(payload["evidenceRefs"], serde_json::json!(["segment-7"]));
        assert_eq!(payload["approvedFields"], serde_json::json!(["query"]));
        assert_eq!(payload["durationMs"], 120);
        assert_eq!(payload["resultByteCount"], 42);

        let encoded = serde_json::to_string(payload).expect("audit payload serializes");
        for forbidden in [
            "secret",
            "credential",
            "authorization",
            "fullTranscript",
            "rawTranscript",
            "arguments",
            "rawOutput",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "audit leaked field: {forbidden}"
            );
        }
    }

    #[test]
    fn audit_rejects_sensitive_or_unbounded_metadata_before_persistence() {
        let (_directory, storage) = open_storage();
        let unsafe_event = ExternalAuditEvent {
            project_id: "project-1".into(),
            event_type: ExternalAuditEventType::Denial,
            actor: ExternalAuditActor::System,
            correlation_id: "run-1".into(),
            recording_id: "recording-1".into(),
            connector_id: Some("connector-1".into()),
            capability: Some(ConnectorCapability::DocumentsSearch),
            tool_name: Some("search_documents".into()),
            evidence_refs: vec!["segment-7".into()],
            approved_fields: vec!["fullTranscript".into()],
            request_hash: None,
            output_hash: None,
            duration_ms: None,
            result_byte_count: None,
            error_code: Some(ExternalMcpErrorCode::EgressFieldDenied),
            created_at: "2026-08-11T04:00:00Z".into(),
        };
        assert_eq!(
            append_external_audit(&storage, &unsafe_event),
            Err(ExternalMcpErrorCode::ResultUnsafe)
        );
    }

    #[test]
    fn sanitizer_neutralizes_active_content_and_local_file_urls() {
        let result = sanitize_external_result(&serde_json::json!({
            "title": "Approved document",
            "summary": "<script>alert('x')</script><b onclick=steal()>Status</b>",
            "local": "file:///C:/Users/private.txt",
            "action": "javascript:alert(1)",
            "source": "https://kb.example/doc/42"
        }))
        .expect("sanitize hostile result");

        assert_eq!(result.payload["title"], "Approved document");
        assert_eq!(result.payload["local"], "[blocked-url]");
        assert_eq!(result.payload["action"], "[blocked-url]");
        assert_eq!(result.payload["source"], "https://kb.example/doc/42");
        assert_eq!(
            result.payload["summary"],
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&lt;b onclick=steal()&gt;Status&lt;/b&gt;"
        );
        assert!(result.byte_size > 0);
        assert!(result.output_hash.starts_with("sha256:"));
    }

    #[test]
    fn sanitizer_enforces_byte_depth_and_item_limits() {
        assert_eq!(
            sanitize_external_result(&serde_json::json!({"body": "x".repeat(256 * 1024)})),
            Err(ExternalMcpErrorCode::ResultTooLarge)
        );

        let mut deeply_nested = serde_json::json!("leaf");
        for _ in 0..18 {
            deeply_nested = serde_json::json!({"child": deeply_nested});
        }
        assert_eq!(
            sanitize_external_result(&deeply_nested),
            Err(ExternalMcpErrorCode::ResultUnsafe)
        );
        assert!(sanitize_external_result(&serde_json::Value::Array(
            (0..1_000).map(serde_json::Value::from).collect()
        ))
        .is_ok());
        assert_eq!(
            sanitize_external_result(&serde_json::Value::Array(
                (0..1_001).map(serde_json::Value::from).collect()
            )),
            Err(ExternalMcpErrorCode::ResultUnsafe)
        );
    }
}
