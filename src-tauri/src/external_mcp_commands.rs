//! Durable command-side orchestration for approved external MCP reads.
//! @req FR-106, FR-107, FR-108, FR-110, FR-111, FR-112, FR-113, FR-114, FR-116, NFR-102, NFR-105, NFR-107, NFR-108, NFR-110
//! @tested tests/externalMeetingTools.test.mjs

use crate::external_mcp::{
    append_external_audit, disconnect_connector, evaluate_policy, minimize_arguments, preview_hash,
    sanitize_external_result, store_connector_credential, ConnectorCapability,
    ConnectorCredentialStore, CredentialReference, ExternalAuditActor, ExternalAuditEvent,
    ExternalAuditEventType, ExternalConnectorSummary, ExternalMcpErrorCode, ExternalToolResult,
    ExternalToolRun, MeetingToolGrant, MeetingToolPreview, OsConnectorCredentialStore,
    PolicyEvaluation, PreviewHashInput, PreviewState, SecretValue, ToolRunStatus,
};
use crate::external_mcp_transport::{
    execute_stdio_tool, validate_stdio_config, AllowedStdioTool, ExternalCancellation,
    ExternalExecutionLimits, StdioConnectorConfig,
};
use genesis_block_native::Storage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct ExternalMcpRuntime {
    cancellations: Mutex<HashMap<String, ExternalCancellation>>,
}

impl ExternalMcpRuntime {
    pub(crate) fn register(
        &self,
        run_id: &str,
    ) -> Result<ExternalCancellation, ExternalMcpErrorCode> {
        if !safe_id(run_id) {
            return Err(ExternalMcpErrorCode::ApprovalRequired);
        }
        let mut cancellations = self
            .cancellations
            .lock()
            .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
        if cancellations.contains_key(run_id) {
            return Err(ExternalMcpErrorCode::ApprovalRequired);
        }
        let cancellation = ExternalCancellation::new();
        cancellations.insert(run_id.to_owned(), cancellation.clone());
        Ok(cancellation)
    }

    pub(crate) fn cancel(&self, run_id: &str) -> bool {
        let Ok(cancellations) = self.cancellations.lock() else {
            return false;
        };
        let Some(cancellation) = cancellations.get(run_id) else {
            return false;
        };
        cancellation.cancel();
        true
    }

    pub(crate) fn finish(&self, run_id: &str) {
        if let Ok(mut cancellations) = self.cancellations.lock() {
            cancellations.remove(run_id);
        }
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        self.cancellations
            .lock()
            .map(|cancellations| cancellations.len())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MeetingToolSuggestInput {
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) connector_id: String,
    pub(crate) capability: ConnectorCapability,
    pub(crate) arguments: Value,
    pub(crate) evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingToolPreviewEnvelope {
    pub(crate) preview: MeetingToolPreview,
    pub(crate) arguments: Value,
    pub(crate) grant: Option<MeetingToolGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MeetingToolExecuteInput {
    pub(crate) run_id: String,
    pub(crate) preview_id: String,
    pub(crate) approved_preview_hash: String,
    pub(crate) arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MeetingToolRevokeInput {
    pub(crate) grant_id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingToolRevokeReceipt {
    pub(crate) grant_id: String,
    pub(crate) revoked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingToolCancelReceipt {
    pub(crate) run_id: String,
    pub(crate) cancellation_requested: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExternalConnectorRegisterInput {
    pub(crate) id: String,
    pub(crate) account_label: String,
    pub(crate) executable: String,
    pub(crate) capabilities: Vec<ConnectorCapability>,
    pub(crate) credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalConnectorDisconnectReceipt {
    pub(crate) connector_id: String,
    pub(crate) revoked_grants: usize,
    pub(crate) disconnected_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingToolExecutionEnvelope {
    pub(crate) run: ExternalToolRun,
    pub(crate) result: ExternalToolResult,
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
}

fn safe_connector_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn qualified<'a>(row: &'a Value, table: &str, column: &str) -> Option<&'a Value> {
    row.get(format!("{table}.{column}"))
}

fn required_string(row: &Value, table: &str, column: &str) -> Result<String, ExternalMcpErrorCode> {
    qualified(row, table, column)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)
}

fn json_column(row: &Value, table: &str, column: &str) -> Result<Value, ExternalMcpErrorCode> {
    let value = qualified(row, table, column)
        .cloned()
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    match value {
        Value::String(encoded) => {
            serde_json::from_str(&encoded).map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)
        }
        value => Ok(value),
    }
}

fn capability_tool_name(capability: ConnectorCapability) -> &'static str {
    match capability {
        ConnectorCapability::DocumentsSearch => "search_documents",
        ConnectorCapability::DocumentsGetMetadata => "get_document_metadata",
        ConnectorCapability::CrmCustomerStatusRead => "get_customer_status",
    }
}

fn validate_text(value: Option<&Value>, max: usize) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty() && value.len() <= max)
}

fn validate_capability_arguments(
    capability: ConnectorCapability,
    arguments: &Value,
) -> Result<Vec<String>, ExternalMcpErrorCode> {
    let object = arguments
        .as_object()
        .ok_or(ExternalMcpErrorCode::EgressFieldDenied)?;
    let valid = match capability {
        ConnectorCapability::DocumentsSearch => {
            object.len() == 1 && validate_text(object.get("query"), 512)
        }
        ConnectorCapability::DocumentsGetMetadata => {
            object.len() == 1 && validate_text(object.get("documentId"), 256)
        }
        ConnectorCapability::CrmCustomerStatusRead => {
            let fields = object.get("fields").and_then(Value::as_array);
            object.len() == 2
                && validate_text(object.get("customerKey"), 256)
                && fields.is_some_and(|fields| {
                    !fields.is_empty()
                        && fields.len() <= 5
                        && fields.iter().all(|field| {
                            matches!(
                                field.as_str(),
                                Some("status" | "stage" | "owner" | "nextStep" | "updatedAt")
                            )
                        })
                })
        }
    };
    if !valid {
        return Err(ExternalMcpErrorCode::EgressFieldDenied);
    }
    let mut fields = object.keys().cloned().collect::<Vec<_>>();
    fields.sort();
    Ok(fields)
}

fn parse_capabilities(value: Value) -> Result<Vec<ConnectorCapability>, ExternalMcpErrorCode> {
    serde_json::from_value(value).map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)
}

fn connector_from_row(row: &Value) -> Result<ExternalConnectorSummary, ExternalMcpErrorCode> {
    let transport = serde_json::from_value(
        qualified(row, "external_connections", "transport")
            .cloned()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    Ok(ExternalConnectorSummary {
        id: required_string(row, "external_connections", "id")?,
        provider: required_string(row, "external_connections", "provider")?,
        account_label: required_string(row, "external_connections", "account_label")?,
        status: required_string(row, "external_connections", "status")?,
        transport,
        endpoint: required_string(row, "external_connections", "endpoint")?,
        credential_ref: qualified(row, "external_connections", "credential_ref")
            .and_then(Value::as_str)
            .map(str::to_owned),
        capabilities: parse_capabilities(json_column(
            row,
            "external_connections",
            "capabilities_json",
        )?)?,
    })
}

pub(crate) fn list_external_connectors(
    storage: &Storage,
) -> Result<Vec<ExternalConnectorSummary>, ExternalMcpErrorCode> {
    let mut connectors = crate::genesis_adapter::query(
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
        ],
        vec![crate::genesis_adapter::eq(
            "external_connections",
            "provider",
            serde_json::json!("local-mcp"),
        )],
        1_000,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .iter()
    .map(connector_from_row)
    .collect::<Result<Vec<_>, _>>()?;
    connectors.sort_by(|left, right| left.account_label.cmp(&right.account_label));
    Ok(connectors)
}

pub(crate) fn register_external_connector(
    storage: &Storage,
    keyring: &mut impl ConnectorCredentialStore,
    input: ExternalConnectorRegisterInput,
    created_at: &str,
) -> Result<ExternalConnectorSummary, ExternalMcpErrorCode> {
    if !safe_connector_id(&input.id)
        || input.account_label.trim().is_empty()
        || input.account_label.len() > 120
        || chrono::DateTime::parse_from_rfc3339(created_at).is_err()
        || input.capabilities.is_empty()
        || input.capabilities.len() > 3
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let mut capabilities = input.capabilities;
    capabilities.sort_by_key(|capability| capability.as_str());
    capabilities.dedup();
    let config = StdioConnectorConfig {
        connector_id: input.id.clone(),
        executable: input.executable.clone().into(),
        arguments: vec![],
        allowed_tools: capabilities
            .iter()
            .map(|capability| AllowedStdioTool {
                name: capability_tool_name(*capability).into(),
                capability: *capability,
            })
            .collect(),
    };
    validate_stdio_config(&config)?;
    let existing = crate::genesis_adapter::query(
        storage,
        "external_connections",
        &["status"],
        vec![crate::genesis_adapter::eq(
            "external_connections",
            "id",
            serde_json::json!(&input.id),
        )],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    if existing.iter().any(|row| {
        qualified(row, "external_connections", "status").and_then(Value::as_str)
            != Some("disconnected")
    }) {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }

    let reference = input
        .credential
        .filter(|credential| !credential.is_empty())
        .map(|credential| {
            store_connector_credential(keyring, &input.id, SecretValue::new(credential))
        })
        .transpose()?;
    let credential_ref = reference
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| ExternalMcpErrorCode::KeyringUnavailable)?;
    let mutation = crate::genesis_adapter::upsert(
        "external_connections",
        serde_json::json!({
            "id":input.id,
            "provider":"local-mcp",
            "account_label":input.account_label.trim(),
            "status":"connected",
            "transport":"stdio",
            "endpoint":input.executable,
            "credential_ref":credential_ref,
            "capabilities_json":capabilities.iter().map(|capability| capability.as_str()).collect::<Vec<_>>(),
            "created_at":created_at,
            "updated_at":created_at,
        }),
    );
    if crate::genesis_adapter::commit_rows(storage, vec![mutation]).is_err() {
        if let Some(reference) = reference {
            let _ = crate::external_mcp::disconnect_connector_credential(keyring, &reference);
        }
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    list_external_connectors(storage)?
        .into_iter()
        .find(|connector| connector.id == input.id)
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)
}

pub(crate) fn disconnect_external_connector(
    storage: &Storage,
    keyring: &mut impl ConnectorCredentialStore,
    connector_id: &str,
    disconnected_at: &str,
) -> Result<ExternalConnectorDisconnectReceipt, ExternalMcpErrorCode> {
    if !safe_connector_id(connector_id)
        || chrono::DateTime::parse_from_rfc3339(disconnected_at).is_err()
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let connector = crate::genesis_adapter::query(
        storage,
        "external_connections",
        &["credential_ref"],
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
    let reference = qualified(&connector, "external_connections", "credential_ref")
        .and_then(Value::as_str)
        .map(serde_json::from_str::<CredentialReference>)
        .transpose()
        .map_err(|_| ExternalMcpErrorCode::KeyringUnavailable)?
        .unwrap_or_else(|| CredentialReference {
            service: "FUNG".into(),
            account: format!("external-mcp/{connector_id}"),
        });
    let revoked_grants =
        disconnect_connector(storage, keyring, connector_id, &reference, disconnected_at)?;
    Ok(ExternalConnectorDisconnectReceipt {
        connector_id: connector_id.into(),
        revoked_grants,
        disconnected_at: disconnected_at.into(),
    })
}

fn parse_grant(row: &Value) -> Result<MeetingToolGrant, ExternalMcpErrorCode> {
    Ok(MeetingToolGrant {
        id: required_string(row, "meeting_tool_grants", "id")?,
        project_id: required_string(row, "meeting_tool_grants", "project_id")?,
        recording_id: required_string(row, "meeting_tool_grants", "recording_id")?,
        connector_id: required_string(row, "meeting_tool_grants", "connector_id")?,
        capabilities: parse_capabilities(json_column(
            row,
            "meeting_tool_grants",
            "capabilities_json",
        )?)?,
        granted_at: required_string(row, "meeting_tool_grants", "granted_at")?,
        expires_at: required_string(row, "meeting_tool_grants", "expires_at")?,
        revoked_at: qualified(row, "meeting_tool_grants", "revoked_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn load_grants(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
    connector_id: &str,
) -> Result<Vec<MeetingToolGrant>, ExternalMcpErrorCode> {
    crate::genesis_adapter::query(
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
        vec![
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "recording_id",
                serde_json::json!(recording_id),
            ),
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "connector_id",
                serde_json::json!(connector_id),
            ),
        ],
        1_000,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .iter()
    .map(parse_grant)
    .collect()
}

pub(crate) fn ensure_meeting_tool_grant(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
    connector_id: &str,
    capability: ConnectorCapability,
    granted_at: &str,
    expires_at: &str,
) -> Result<MeetingToolGrant, ExternalMcpErrorCode> {
    if !safe_id(project_id)
        || !safe_id(recording_id)
        || !safe_connector_id(connector_id)
        || chrono::DateTime::parse_from_rfc3339(granted_at).is_err()
        || chrono::DateTime::parse_from_rfc3339(expires_at).is_err()
    {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }
    let granted = chrono::DateTime::parse_from_rfc3339(granted_at)
        .map_err(|_| ExternalMcpErrorCode::GrantExpired)?;
    let expiry = chrono::DateTime::parse_from_rfc3339(expires_at)
        .map_err(|_| ExternalMcpErrorCode::GrantExpired)?;
    if expiry <= granted {
        return Err(ExternalMcpErrorCode::GrantExpired);
    }
    let (_, connector_capabilities) = load_connector_capabilities(storage, connector_id)?;
    if !connector_capabilities.contains(&capability) {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    if let Some(grant) = load_grants(storage, project_id, recording_id, connector_id)?
        .into_iter()
        .find(|grant| {
            grant.revoked_at.is_none()
                && grant.capabilities.contains(&capability)
                && chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
                    .is_ok_and(|existing_expiry| existing_expiry > granted)
        })
    {
        return Ok(grant);
    }
    let grant = MeetingToolGrant {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        recording_id: recording_id.into(),
        connector_id: connector_id.into(),
        capabilities: vec![capability],
        granted_at: granted_at.into(),
        expires_at: expires_at.into(),
        revoked_at: None,
    };
    crate::genesis_adapter::commit_rows(
        storage,
        vec![crate::genesis_adapter::upsert(
            "meeting_tool_grants",
            serde_json::json!({
                "id":grant.id,
                "project_id":grant.project_id,
                "recording_id":grant.recording_id,
                "connector_id":grant.connector_id,
                "capabilities_json":[capability.as_str()],
                "granted_at":grant.granted_at,
                "expires_at":grant.expires_at,
                "revoked_at":null,
            }),
        )],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    append_external_audit(
        storage,
        &ExternalAuditEvent {
            project_id: grant.project_id.clone(),
            event_type: ExternalAuditEventType::Approval,
            actor: ExternalAuditActor::User,
            correlation_id: grant.id.clone(),
            recording_id: grant.recording_id.clone(),
            connector_id: Some(grant.connector_id.clone()),
            capability: Some(capability),
            tool_name: Some(capability_tool_name(capability).into()),
            evidence_refs: vec![],
            approved_fields: vec![],
            request_hash: None,
            output_hash: None,
            duration_ms: None,
            result_byte_count: None,
            error_code: None,
            created_at: granted_at.into(),
        },
    )?;
    Ok(grant)
}

fn load_connector_capabilities(
    storage: &Storage,
    connector_id: &str,
) -> Result<(String, Vec<ConnectorCapability>), ExternalMcpErrorCode> {
    let row = crate::genesis_adapter::query(
        storage,
        "external_connections",
        &["status", "transport", "endpoint", "capabilities_json"],
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
    if required_string(&row, "external_connections", "status")? != "connected"
        || required_string(&row, "external_connections", "transport")? != "stdio"
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let endpoint = required_string(&row, "external_connections", "endpoint")?;
    let capabilities = parse_capabilities(json_column(
        &row,
        "external_connections",
        "capabilities_json",
    )?)?;
    Ok((endpoint, capabilities))
}

fn preview_from_row(row: &Value) -> Result<MeetingToolPreview, ExternalMcpErrorCode> {
    let capability: ConnectorCapability = serde_json::from_value(
        qualified(row, "external_tool_previews", "capability")
            .cloned()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let state: PreviewState = serde_json::from_value(
        qualified(row, "external_tool_previews", "state")
            .cloned()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    Ok(MeetingToolPreview {
        id: required_string(row, "external_tool_previews", "id")?,
        project_id: required_string(row, "external_tool_previews", "project_id")?,
        recording_id: required_string(row, "external_tool_previews", "recording_id")?,
        connector_id: required_string(row, "external_tool_previews", "connector_id")?,
        tool_name: required_string(row, "external_tool_previews", "tool_name")?,
        capability,
        arguments_hash: required_string(row, "external_tool_previews", "arguments_hash")?,
        approved_fields: serde_json::from_value(json_column(
            row,
            "external_tool_previews",
            "approved_fields_json",
        )?)
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?,
        evidence_refs: serde_json::from_value(json_column(
            row,
            "external_tool_previews",
            "evidence_refs_json",
        )?)
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?,
        state,
        expires_at: required_string(row, "external_tool_previews", "expires_at")?,
        created_at: required_string(row, "external_tool_previews", "created_at")?,
    })
}

fn load_preview(
    storage: &Storage,
    preview_id: &str,
) -> Result<MeetingToolPreview, ExternalMcpErrorCode> {
    let row = crate::genesis_adapter::query(
        storage,
        "external_tool_previews",
        &[
            "id",
            "project_id",
            "recording_id",
            "connector_id",
            "tool_name",
            "capability",
            "arguments_hash",
            "approved_fields_json",
            "evidence_refs_json",
            "state",
            "expires_at",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "external_tool_previews",
            "id",
            serde_json::json!(preview_id),
        )],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .into_iter()
    .next()
    .ok_or(ExternalMcpErrorCode::ApprovalRequired)?;
    preview_from_row(&row)
}

fn preview_mutation(
    preview: &MeetingToolPreview,
    state: PreviewState,
) -> genesis_block_native::RelationalRowMutation {
    crate::genesis_adapter::upsert(
        "external_tool_previews",
        serde_json::json!({
            "id": preview.id,
            "project_id": preview.project_id,
            "recording_id": preview.recording_id,
            "connector_id": preview.connector_id,
            "tool_name": preview.tool_name,
            "capability": preview.capability.as_str(),
            "arguments_hash": preview.arguments_hash,
            "approved_fields_json": preview.approved_fields,
            "evidence_refs_json": preview.evidence_refs,
            "state": serde_json::to_value(state).expect("preview state serializes"),
            "expires_at": preview.expires_at,
            "created_at": preview.created_at,
        }),
    )
}

pub(crate) fn create_meeting_tool_preview(
    storage: &Storage,
    input: MeetingToolSuggestInput,
    now: &str,
) -> Result<MeetingToolPreviewEnvelope, ExternalMcpErrorCode> {
    if !safe_id(&input.project_id)
        || !safe_id(&input.recording_id)
        || !safe_id(&input.connector_id)
        || input.evidence_refs.is_empty()
        || input.evidence_refs.len() > 64
        || input.evidence_refs.iter().any(|value| !safe_id(value))
    {
        return Err(ExternalMcpErrorCode::EgressFieldDenied);
    }
    let approved_fields = validate_capability_arguments(input.capability, &input.arguments)?;
    let arguments = minimize_arguments(&input.arguments, &approved_fields)?;
    let (_, connector_capabilities) = load_connector_capabilities(storage, &input.connector_id)?;
    if !connector_capabilities.contains(&input.capability) {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    let now_timestamp = chrono::DateTime::parse_from_rfc3339(now)
        .map_err(|_| ExternalMcpErrorCode::GrantExpired)?
        .with_timezone(&chrono::Utc);
    let preview_expiry = now_timestamp + chrono::Duration::minutes(2);
    let grants = load_grants(
        storage,
        &input.project_id,
        &input.recording_id,
        &input.connector_id,
    )?;
    let grant = grants
        .iter()
        .find(|grant| grant.capabilities.contains(&input.capability))
        .ok_or(ExternalMcpErrorCode::CapabilityDenied)?;
    let grant_expiry = chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
        .map_err(|_| ExternalMcpErrorCode::GrantExpired)?
        .with_timezone(&chrono::Utc);
    let expires_at = std::cmp::min(preview_expiry, grant_expiry).to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let tool_name = capability_tool_name(input.capability).to_owned();
    let arguments_hash = preview_hash(PreviewHashInput {
        connector_id: &input.connector_id,
        tool_name: &tool_name,
        capability: input.capability,
        arguments: &arguments,
        evidence_refs: &input.evidence_refs,
        approved_fields: &approved_fields,
        project_id: &input.project_id,
        recording_id: &input.recording_id,
        expires_at: &expires_at,
    });
    let preview = MeetingToolPreview {
        id,
        project_id: input.project_id,
        recording_id: input.recording_id,
        connector_id: input.connector_id,
        tool_name,
        capability: input.capability,
        arguments_hash,
        approved_fields,
        evidence_refs: input.evidence_refs,
        state: PreviewState::Previewed,
        expires_at,
        created_at: now.to_owned(),
    };
    evaluate_policy(PolicyEvaluation {
        grant: Some(grant),
        preview: &preview,
        advertised_capability: preview.capability.as_str(),
        approved_preview_hash: Some(&preview.arguments_hash),
        now,
    })?;
    crate::genesis_adapter::commit_rows(
        storage,
        vec![preview_mutation(&preview, PreviewState::Previewed)],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    append_external_audit(
        storage,
        &ExternalAuditEvent {
            project_id: preview.project_id.clone(),
            event_type: ExternalAuditEventType::Preview,
            actor: ExternalAuditActor::System,
            correlation_id: preview.id.clone(),
            recording_id: preview.recording_id.clone(),
            connector_id: Some(preview.connector_id.clone()),
            capability: Some(preview.capability),
            tool_name: Some(preview.tool_name.clone()),
            evidence_refs: preview.evidence_refs.clone(),
            approved_fields: preview.approved_fields.clone(),
            request_hash: Some(preview.arguments_hash.clone()),
            output_hash: None,
            duration_ms: None,
            result_byte_count: None,
            error_code: None,
            created_at: now.to_owned(),
        },
    )?;
    Ok(MeetingToolPreviewEnvelope {
        preview,
        arguments,
        grant: None,
    })
}

fn run_mutation(run: &ExternalToolRun) -> genesis_block_native::RelationalRowMutation {
    crate::genesis_adapter::upsert(
        "external_tool_runs",
        serde_json::json!({
            "id":run.id,
            "preview_id":run.preview_id,
            "project_id":run.project_id,
            "recording_id":run.recording_id,
            "connector_id":run.connector_id,
            "tool_name":run.tool_name,
            "capability":run.capability.as_str(),
            "request_hash":run.request_hash,
            "output_hash":run.output_hash,
            "status":serde_json::to_value(run.status).expect("run state serializes"),
            "started_at":run.started_at,
            "finished_at":run.finished_at,
            "error_code":run.error_code.map(ExternalMcpErrorCode::as_str),
            "result_ref":run.result_ref,
        }),
    )
}

fn run_from_row(row: &Value) -> Result<ExternalToolRun, ExternalMcpErrorCode> {
    let status = serde_json::from_value(
        qualified(row, "external_tool_runs", "status")
            .cloned()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let capability = serde_json::from_value(
        qualified(row, "external_tool_runs", "capability")
            .cloned()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let error_code = qualified(row, "external_tool_runs", "error_code")
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    Ok(ExternalToolRun {
        id: required_string(row, "external_tool_runs", "id")?,
        preview_id: required_string(row, "external_tool_runs", "preview_id")?,
        project_id: required_string(row, "external_tool_runs", "project_id")?,
        recording_id: required_string(row, "external_tool_runs", "recording_id")?,
        connector_id: required_string(row, "external_tool_runs", "connector_id")?,
        tool_name: required_string(row, "external_tool_runs", "tool_name")?,
        capability,
        request_hash: required_string(row, "external_tool_runs", "request_hash")?,
        output_hash: qualified(row, "external_tool_runs", "output_hash")
            .and_then(Value::as_str)
            .map(str::to_owned),
        status,
        started_at: required_string(row, "external_tool_runs", "started_at")?,
        finished_at: qualified(row, "external_tool_runs", "finished_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
        error_code,
        result_ref: qualified(row, "external_tool_runs", "result_ref")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

pub(crate) fn list_external_tool_runs(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<ExternalToolRun>, ExternalMcpErrorCode> {
    if !safe_id(project_id) || !safe_id(recording_id) {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }
    let mut runs = crate::genesis_adapter::query(
        storage,
        "external_tool_runs",
        &[
            "id",
            "preview_id",
            "project_id",
            "recording_id",
            "connector_id",
            "tool_name",
            "capability",
            "request_hash",
            "output_hash",
            "status",
            "started_at",
            "finished_at",
            "error_code",
            "result_ref",
        ],
        vec![
            crate::genesis_adapter::eq(
                "external_tool_runs",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "external_tool_runs",
                "recording_id",
                serde_json::json!(recording_id),
            ),
        ],
        1_000,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .iter()
    .map(run_from_row)
    .collect::<Result<Vec<_>, _>>()?;
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at.clone()));
    Ok(runs)
}

pub(crate) fn load_external_tool_result(
    storage: &Storage,
    result_id: &str,
) -> Result<ExternalToolResult, ExternalMcpErrorCode> {
    if !safe_id(result_id) {
        return Err(ExternalMcpErrorCode::ResultUnsafe);
    }
    let row = crate::genesis_adapter::query(
        storage,
        "external_tool_results",
        &[
            "id",
            "run_id",
            "mime_type",
            "sanitized_payload_json",
            "source_refs_json",
            "byte_size",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "external_tool_results",
            "id",
            serde_json::json!(result_id),
        )],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .into_iter()
    .next()
    .ok_or(ExternalMcpErrorCode::ResultUnsafe)?;
    Ok(ExternalToolResult {
        id: required_string(&row, "external_tool_results", "id")?,
        run_id: required_string(&row, "external_tool_results", "run_id")?,
        mime_type: required_string(&row, "external_tool_results", "mime_type")?,
        sanitized_payload: json_column(&row, "external_tool_results", "sanitized_payload_json")?,
        source_refs: serde_json::from_value(json_column(
            &row,
            "external_tool_results",
            "source_refs_json",
        )?)
        .map_err(|_| ExternalMcpErrorCode::ResultUnsafe)?,
        byte_size: qualified(&row, "external_tool_results", "byte_size")
            .and_then(Value::as_u64)
            .ok_or(ExternalMcpErrorCode::ResultUnsafe)?,
        created_at: required_string(&row, "external_tool_results", "created_at")?,
    })
}

pub(crate) fn revoke_meeting_tool_grant(
    storage: &Storage,
    input: MeetingToolRevokeInput,
    revoked_at: &str,
) -> Result<MeetingToolRevokeReceipt, ExternalMcpErrorCode> {
    if !safe_id(&input.grant_id)
        || !safe_id(&input.project_id)
        || !safe_id(&input.recording_id)
        || chrono::DateTime::parse_from_rfc3339(revoked_at).is_err()
    {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }
    let row = crate::genesis_adapter::query(
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
        vec![
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "id",
                serde_json::json!(&input.grant_id),
            ),
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "project_id",
                serde_json::json!(&input.project_id),
            ),
            crate::genesis_adapter::eq(
                "meeting_tool_grants",
                "recording_id",
                serde_json::json!(&input.recording_id),
            ),
        ],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?
    .into_iter()
    .next()
    .ok_or(ExternalMcpErrorCode::CapabilityDenied)?;
    let mut grant = parse_grant(&row)?;
    let effective_revoked_at = grant
        .revoked_at
        .clone()
        .unwrap_or_else(|| revoked_at.to_owned());
    if grant.revoked_at.is_none() {
        grant.revoked_at = Some(effective_revoked_at.clone());
        crate::genesis_adapter::commit_rows(
            storage,
            vec![crate::genesis_adapter::upsert(
                "meeting_tool_grants",
                serde_json::json!({
                    "id":grant.id,
                    "project_id":grant.project_id,
                    "recording_id":grant.recording_id,
                    "connector_id":grant.connector_id,
                    "capabilities_json":grant.capabilities.iter().map(|capability| capability.as_str()).collect::<Vec<_>>(),
                    "granted_at":grant.granted_at,
                    "expires_at":grant.expires_at,
                    "revoked_at":effective_revoked_at,
                }),
            )],
        )
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
        append_external_audit(
            storage,
            &ExternalAuditEvent {
                project_id: grant.project_id.clone(),
                event_type: ExternalAuditEventType::Revocation,
                actor: ExternalAuditActor::User,
                correlation_id: grant.id.clone(),
                recording_id: grant.recording_id.clone(),
                connector_id: Some(grant.connector_id.clone()),
                capability: None,
                tool_name: None,
                evidence_refs: vec![],
                approved_fields: vec![],
                request_hash: None,
                output_hash: None,
                duration_ms: None,
                result_byte_count: None,
                error_code: Some(ExternalMcpErrorCode::GrantRevoked),
                created_at: effective_revoked_at.clone(),
            },
        )?;
    }
    Ok(MeetingToolRevokeReceipt {
        grant_id: grant.id,
        revoked_at: effective_revoked_at,
    })
}

fn audit_terminal(
    storage: &Storage,
    run: &ExternalToolRun,
    preview: &MeetingToolPreview,
    event_type: ExternalAuditEventType,
    byte_count: Option<u64>,
    created_at: &str,
) -> Result<(), ExternalMcpErrorCode> {
    let started = chrono::DateTime::parse_from_rfc3339(&run.started_at)
        .map_err(|_| ExternalMcpErrorCode::ResultUnsafe)?;
    let finished = chrono::DateTime::parse_from_rfc3339(created_at)
        .map_err(|_| ExternalMcpErrorCode::ResultUnsafe)?;
    append_external_audit(
        storage,
        &ExternalAuditEvent {
            project_id: run.project_id.clone(),
            event_type,
            actor: ExternalAuditActor::System,
            correlation_id: run.id.clone(),
            recording_id: run.recording_id.clone(),
            connector_id: Some(run.connector_id.clone()),
            capability: Some(run.capability),
            tool_name: Some(run.tool_name.clone()),
            evidence_refs: preview.evidence_refs.clone(),
            approved_fields: preview.approved_fields.clone(),
            request_hash: Some(run.request_hash.clone()),
            output_hash: run.output_hash.clone(),
            duration_ms: Some((finished - started).num_milliseconds().max(0) as u64),
            result_byte_count: byte_count,
            error_code: run.error_code,
            created_at: created_at.to_owned(),
        },
    )
}

pub(crate) fn execute_approved_preview(
    storage: &Storage,
    input: MeetingToolExecuteInput,
    config: &StdioConnectorConfig,
    limits: ExternalExecutionLimits,
    cancellation: &ExternalCancellation,
    started_at: &str,
    finished_at_override: Option<&str>,
) -> Result<ExternalToolRun, ExternalMcpErrorCode> {
    if !safe_id(&input.run_id) || !safe_id(&input.preview_id) {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }
    let duplicate = crate::genesis_adapter::query(
        storage,
        "external_tool_runs",
        &["id"],
        vec![crate::genesis_adapter::eq(
            "external_tool_runs",
            "preview_id",
            serde_json::json!(&input.preview_id),
        )],
        1,
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    if !duplicate.is_empty() {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }

    let preview = load_preview(storage, &input.preview_id)?;
    let (endpoint, connector_capabilities) =
        load_connector_capabilities(storage, &preview.connector_id)?;
    if config.connector_id != preview.connector_id
        || config.executable.as_path() != std::path::Path::new(&endpoint)
        || !connector_capabilities.contains(&preview.capability)
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let approved_fields = validate_capability_arguments(preview.capability, &input.arguments)?;
    if approved_fields != preview.approved_fields {
        return Err(ExternalMcpErrorCode::PreviewChanged);
    }
    let arguments = minimize_arguments(&input.arguments, &preview.approved_fields)?;
    let recomputed = preview_hash(PreviewHashInput {
        connector_id: &preview.connector_id,
        tool_name: &preview.tool_name,
        capability: preview.capability,
        arguments: &arguments,
        evidence_refs: &preview.evidence_refs,
        approved_fields: &preview.approved_fields,
        project_id: &preview.project_id,
        recording_id: &preview.recording_id,
        expires_at: &preview.expires_at,
    });
    if recomputed != preview.arguments_hash || input.approved_preview_hash != recomputed {
        return Err(ExternalMcpErrorCode::PreviewChanged);
    }
    let grants = load_grants(
        storage,
        &preview.project_id,
        &preview.recording_id,
        &preview.connector_id,
    )?;
    let grant = grants
        .iter()
        .find(|grant| grant.capabilities.contains(&preview.capability))
        .ok_or(ExternalMcpErrorCode::CapabilityDenied)?;
    evaluate_policy(PolicyEvaluation {
        grant: Some(grant),
        preview: &preview,
        advertised_capability: preview.capability.as_str(),
        approved_preview_hash: Some(&input.approved_preview_hash),
        now: started_at,
    })?;

    let mut run = ExternalToolRun {
        id: input.run_id,
        preview_id: preview.id.clone(),
        project_id: preview.project_id.clone(),
        recording_id: preview.recording_id.clone(),
        connector_id: preview.connector_id.clone(),
        tool_name: preview.tool_name.clone(),
        capability: preview.capability,
        request_hash: recomputed,
        output_hash: None,
        status: ToolRunStatus::Running,
        started_at: started_at.to_owned(),
        finished_at: None,
        error_code: None,
        result_ref: None,
    };
    crate::genesis_adapter::commit_rows(
        storage,
        vec![
            run_mutation(&run),
            preview_mutation(&preview, PreviewState::Running),
        ],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    append_external_audit(
        storage,
        &ExternalAuditEvent {
            project_id: run.project_id.clone(),
            event_type: ExternalAuditEventType::Execution,
            actor: ExternalAuditActor::User,
            correlation_id: run.id.clone(),
            recording_id: run.recording_id.clone(),
            connector_id: Some(run.connector_id.clone()),
            capability: Some(run.capability),
            tool_name: Some(run.tool_name.clone()),
            evidence_refs: preview.evidence_refs.clone(),
            approved_fields: preview.approved_fields.clone(),
            request_hash: Some(run.request_hash.clone()),
            output_hash: None,
            duration_ms: None,
            result_byte_count: None,
            error_code: None,
            created_at: started_at.to_owned(),
        },
    )?;

    let execution = execute_stdio_tool(
        config,
        &preview.tool_name,
        preview.capability,
        &arguments,
        limits,
        cancellation,
    );
    let completed_at = finished_at_override
        .map(str::to_owned)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let output = match execution {
        Ok(output) => output,
        Err(code) => {
            run.status = if code == ExternalMcpErrorCode::ToolCancelled {
                ToolRunStatus::Cancelled
            } else {
                ToolRunStatus::Failed
            };
            run.finished_at = Some(completed_at.clone());
            run.error_code = Some(code);
            crate::genesis_adapter::commit_rows(
                storage,
                vec![
                    run_mutation(&run),
                    preview_mutation(
                        &preview,
                        if code == ExternalMcpErrorCode::ToolCancelled {
                            PreviewState::Cancelled
                        } else {
                            PreviewState::Failed
                        },
                    ),
                ],
            )
            .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
            let event_type = match code {
                ExternalMcpErrorCode::ToolCancelled => ExternalAuditEventType::Cancellation,
                ExternalMcpErrorCode::ToolTimeout => ExternalAuditEventType::Timeout,
                _ => ExternalAuditEventType::Failure,
            };
            audit_terminal(storage, &run, &preview, event_type, None, &completed_at)?;
            return Err(code);
        }
    };
    let sanitized = match sanitize_external_result(&output.payload) {
        Ok(sanitized) => sanitized,
        Err(code) => {
            run.status = ToolRunStatus::Failed;
            run.finished_at = Some(completed_at.clone());
            run.error_code = Some(code);
            crate::genesis_adapter::commit_rows(
                storage,
                vec![
                    run_mutation(&run),
                    preview_mutation(&preview, PreviewState::Failed),
                ],
            )
            .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
            audit_terminal(
                storage,
                &run,
                &preview,
                ExternalAuditEventType::Failure,
                None,
                &completed_at,
            )?;
            return Err(code);
        }
    };
    let result_id = uuid::Uuid::new_v4().to_string();
    run.output_hash = Some(sanitized.output_hash.clone());
    run.status = ToolRunStatus::Completed;
    run.finished_at = Some(completed_at.clone());
    run.result_ref = Some(result_id.clone());
    crate::genesis_adapter::commit_rows(
        storage,
        vec![
            crate::genesis_adapter::upsert(
                "external_tool_results",
                serde_json::json!({
                    "id":result_id,
                    "run_id":run.id,
                    "mime_type":"application/json",
                    "sanitized_payload_json":sanitized.payload,
                    "source_refs_json":output.source_refs,
                    "byte_size":sanitized.byte_size,
                    "created_at":completed_at,
                }),
            ),
            run_mutation(&run),
            preview_mutation(&preview, PreviewState::Completed),
        ],
    )
    .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    audit_terminal(
        storage,
        &run,
        &preview,
        ExternalAuditEventType::Completion,
        Some(sanitized.byte_size),
        &completed_at,
    )?;
    Ok(run)
}

fn stdio_config_for_preview(
    storage: &Storage,
    preview_id: &str,
) -> Result<StdioConnectorConfig, ExternalMcpErrorCode> {
    let preview = load_preview(storage, preview_id)?;
    let (endpoint, capabilities) = load_connector_capabilities(storage, &preview.connector_id)?;
    if !capabilities.contains(&preview.capability) {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    Ok(StdioConnectorConfig {
        connector_id: preview.connector_id,
        executable: endpoint.into(),
        arguments: vec![],
        allowed_tools: vec![crate::external_mcp_transport::AllowedStdioTool {
            name: preview.tool_name,
            capability: preview.capability,
        }],
    })
}

#[tauri::command]
pub(crate) fn external_connectors_list(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<ExternalConnectorSummary>, ExternalMcpErrorCode> {
    list_external_connectors(&state.genesis)
}

#[tauri::command]
pub(crate) fn external_connector_register(
    input: ExternalConnectorRegisterInput,
    state: tauri::State<'_, crate::AppState>,
) -> Result<ExternalConnectorSummary, ExternalMcpErrorCode> {
    if !state.external_meeting_tools_enabled {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    register_external_connector(
        &state.genesis,
        &mut OsConnectorCredentialStore,
        input,
        &chrono::Utc::now().to_rfc3339(),
    )
}

#[tauri::command]
pub(crate) fn external_connector_disconnect(
    connector_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<ExternalConnectorDisconnectReceipt, ExternalMcpErrorCode> {
    disconnect_external_connector(
        &state.genesis,
        &mut OsConnectorCredentialStore,
        &connector_id,
        &chrono::Utc::now().to_rfc3339(),
    )
}

#[tauri::command]
pub(crate) fn meeting_tool_suggest(
    input: MeetingToolSuggestInput,
    state: tauri::State<'_, crate::AppState>,
) -> Result<MeetingToolPreviewEnvelope, ExternalMcpErrorCode> {
    if !state.external_meeting_tools_enabled {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    validate_capability_arguments(input.capability, &input.arguments)?;
    let now = chrono::Utc::now();
    let grant = ensure_meeting_tool_grant(
        &state.genesis,
        &input.project_id,
        &input.recording_id,
        &input.connector_id,
        input.capability,
        &now.to_rfc3339(),
        &(now + chrono::Duration::minutes(15)).to_rfc3339(),
    )?;
    let mut envelope = create_meeting_tool_preview(&state.genesis, input, &now.to_rfc3339())?;
    envelope.grant = Some(grant);
    Ok(envelope)
}

#[tauri::command]
pub(crate) async fn meeting_tool_execute(
    input: MeetingToolExecuteInput,
    state: tauri::State<'_, crate::AppState>,
) -> Result<MeetingToolExecutionEnvelope, ExternalMcpErrorCode> {
    if !state.external_meeting_tools_enabled {
        return Err(ExternalMcpErrorCode::CapabilityDenied);
    }
    let config = stdio_config_for_preview(&state.genesis, &input.preview_id)?;
    let run_id = input.run_id.clone();
    let cancellation = state.external_mcp.register(&run_id)?;
    let storage = state.genesis.clone();
    let started_at = chrono::Utc::now().to_rfc3339();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        execute_approved_preview(
            &storage,
            input,
            &config,
            ExternalExecutionLimits {
                timeout: Duration::from_secs(15),
            },
            &cancellation,
            &started_at,
            None,
        )
    })
    .await;
    state.external_mcp.finish(&run_id);
    let run = joined.map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)??;
    let result = load_external_tool_result(
        &state.genesis,
        run.result_ref
            .as_deref()
            .ok_or(ExternalMcpErrorCode::ResultUnsafe)?,
    )?;
    Ok(MeetingToolExecutionEnvelope { run, result })
}

#[tauri::command]
pub(crate) fn meeting_tool_cancel(
    run_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<MeetingToolCancelReceipt, ExternalMcpErrorCode> {
    if !safe_id(&run_id) {
        return Err(ExternalMcpErrorCode::ApprovalRequired);
    }
    Ok(MeetingToolCancelReceipt {
        cancellation_requested: state.external_mcp.cancel(&run_id),
        run_id,
    })
}

#[tauri::command]
pub(crate) fn meeting_tool_revoke(
    input: MeetingToolRevokeInput,
    state: tauri::State<'_, crate::AppState>,
) -> Result<MeetingToolRevokeReceipt, ExternalMcpErrorCode> {
    revoke_meeting_tool_grant(&state.genesis, input, &chrono::Utc::now().to_rfc3339())
}

#[tauri::command]
pub(crate) fn meeting_tool_runs_list(
    project_id: String,
    recording_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<ExternalToolRun>, ExternalMcpErrorCode> {
    list_external_tool_runs(&state.genesis, &project_id, &recording_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_mcp::{
        ConnectorCapability, ConnectorCredentialStore, ExternalMcpErrorCode, PreviewState,
    };
    use crate::external_mcp_transport::{
        AllowedStdioTool, ExternalCancellation, ExternalExecutionLimits, StdioConnectorConfig,
    };
    use genesis_block_native::{OpenOptions, Storage};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::OnceLock;
    use std::time::Duration;

    #[derive(Default)]
    struct FakeCredentialStore {
        values: std::collections::HashMap<String, String>,
    }

    impl ConnectorCredentialStore for FakeCredentialStore {
        fn set(&mut self, account: &str, secret: &str) -> Result<(), String> {
            self.values.insert(account.into(), secret.into());
            Ok(())
        }

        fn get(&mut self, account: &str) -> Result<Option<String>, String> {
            Ok(self.values.get(account).cloned())
        }

        fn delete(&mut self, account: &str) -> Result<(), String> {
            self.values.remove(account);
            Ok(())
        }
    }

    fn open_storage() -> (tempfile::TempDir, Storage) {
        let directory = tempfile::tempdir().expect("temporary Genesis directory");
        let storage = Storage::open(OpenOptions {
            path: directory.path().display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .expect("open Genesis storage");
        crate::genesis_adapter::install(&storage).expect("install FUNG schema");
        (directory, storage)
    }

    /// True when `built` exists and is at least as new as `source`, i.e. the
    /// compiled fixture already reflects the current source.
    fn is_newer_than(built: &PathBuf, source: &PathBuf) -> bool {
        let modified = |path: &PathBuf| std::fs::metadata(path).and_then(|meta| meta.modified());
        match (modified(built), modified(source)) {
            (Ok(built_at), Ok(source_at)) => built_at >= source_at,
            _ => false,
        }
    }

    fn fixture_executable() -> PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .join("test-fixtures");
                std::fs::create_dir_all(&output_dir).expect("create fixture output directory");
                let executable = output_dir.join(if cfg!(windows) {
                    "fake-external-mcp-commands.exe"
                } else {
                    "fake-external-mcp-commands"
                });
                let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join("fixtures")
                    .join("fake_external_mcp.rs");
                // `OnceLock` is per-process, so every `cargo test` run used to
                // recompile and rewrite this binary. On Windows a freshly
                // written unsigned executable pays a full malware scan on its
                // first exec — measured at ~1.3s, against tool budgets of two
                // seconds — which made every fixture-spawning test racy. Reuse
                // a binary that is already newer than its source.
                if is_newer_than(&executable, &source) {
                    return executable;
                }
                let status =
                    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                        .arg(&source)
                        .arg("--edition=2021")
                        .arg("-o")
                        .arg(&executable)
                        .status()
                        .expect("compile MCP fixture");
                assert!(status.success(), "fixture compilation failed");
                executable
            })
            .clone()
    }

    fn seed(storage: &Storage, endpoint: &str) {
        crate::genesis_adapter::commit_rows(
            storage,
            vec![
                crate::genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id":"project-1", "name":"MCP Test", "storage_path":"test-path",
                        "active_recording_id":null, "created_at":"2026-08-11T03:00:00Z",
                        "updated_at":"2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id":"recording-1", "project_id":"project-1", "source":"microphone",
                        "input_path":null, "canonical_audio_path":"recordings/recording-1.wav",
                        "status":"recording", "duration_ms":0,
                        "created_at":"2026-08-11T03:00:00Z", "updated_at":"2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "external_connections",
                    serde_json::json!({
                        "id":"connector-1", "provider":"fixture", "account_label":"Fixture KB",
                        "status":"connected", "transport":"stdio", "endpoint":endpoint,
                        "credential_ref":null, "capabilities_json":["documents.search"],
                        "created_at":"2026-08-11T03:00:00Z", "updated_at":"2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "meeting_tool_grants",
                    serde_json::json!({
                        "id":"grant-1", "project_id":"project-1", "recording_id":"recording-1",
                        "connector_id":"connector-1", "capabilities_json":["documents.search"],
                        "granted_at":"2026-08-11T03:00:00Z", "expires_at":"2026-08-11T05:00:00Z",
                        "revoked_at":null
                    }),
                ),
            ],
        )
        .expect("seed external MCP state");
    }

    #[test]
    fn approved_preview_executes_once_and_persists_only_sanitized_audited_result() {
        let (_directory, storage) = open_storage();
        let executable = fixture_executable();
        seed(&storage, &executable.display().to_string());
        let counter = tempfile::NamedTempFile::new().expect("counter file");
        std::fs::write(counter.path(), "0").expect("initialize counter");

        let preview = create_meeting_tool_preview(
            &storage,
            MeetingToolSuggestInput {
                project_id: "project-1".into(),
                recording_id: "recording-1".into(),
                connector_id: "connector-1".into(),
                capability: ConnectorCapability::DocumentsSearch,
                arguments: serde_json::json!({"query":"contract"}),
                evidence_refs: vec!["segment-7".into()],
            },
            "2026-08-11T04:00:00Z",
        )
        .expect("create preview");
        assert_eq!(preview.preview.state, PreviewState::Previewed);
        assert_eq!(std::fs::read_to_string(counter.path()).unwrap(), "0");

        let config = StdioConnectorConfig {
            connector_id: "connector-1".into(),
            executable,
            arguments: vec!["normal".into(), counter.path().display().to_string()],
            allowed_tools: vec![AllowedStdioTool {
                name: "search_documents".into(),
                capability: ConnectorCapability::DocumentsSearch,
            }],
        };
        let run = execute_approved_preview(
            &storage,
            MeetingToolExecuteInput {
                run_id: "run-1".into(),
                preview_id: preview.preview.id.clone(),
                approved_preview_hash: preview.preview.arguments_hash.clone(),
                arguments: preview.arguments.clone(),
            },
            &config,
            ExternalExecutionLimits {
                timeout: Duration::from_secs(2),
            },
            &ExternalCancellation::new(),
            "2026-08-11T04:00:01Z",
            Some("2026-08-11T04:00:02Z"),
        )
        .expect("execute approved preview");
        assert_eq!(run.status, crate::external_mcp::ToolRunStatus::Completed);
        assert_eq!(std::fs::read_to_string(counter.path()).unwrap(), "1");

        let loaded_result = load_external_tool_result(
            &storage,
            run.result_ref.as_deref().expect("completed run result ref"),
        )
        .expect("load sanitized result envelope");
        assert_eq!(loaded_result.run_id, "run-1");
        assert_eq!(loaded_result.source_refs, vec!["kb://documents/42"]);

        let results = crate::genesis_adapter::query(
            &storage,
            "external_tool_results",
            &["sanitized_payload_json", "source_refs_json", "byte_size"],
            vec![],
            10,
        )
        .expect("query result");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0]["external_tool_results.sanitized_payload_json"]["items"][0]["title"],
            "Approved contract"
        );

        assert_eq!(
            execute_approved_preview(
                &storage,
                MeetingToolExecuteInput {
                    run_id: "run-2".into(),
                    preview_id: preview.preview.id,
                    approved_preview_hash: preview.preview.arguments_hash,
                    arguments: preview.arguments,
                },
                &config,
                ExternalExecutionLimits {
                    timeout: Duration::from_secs(2),
                },
                &ExternalCancellation::new(),
                "2026-08-11T04:00:03Z",
                Some("2026-08-11T04:00:04Z"),
            ),
            Err(ExternalMcpErrorCode::ApprovalRequired)
        );
        assert_eq!(std::fs::read_to_string(counter.path()).unwrap(), "1");

        let audits = crate::genesis_adapter::query(
            &storage,
            "audit_events",
            &["event_type", "payload_json"],
            vec![],
            20,
        )
        .expect("query audit events");
        assert!(audits.iter().any(|row| {
            row["audit_events.event_type"] == "external_tool.completion"
                && row["audit_events.payload_json"]["correlationId"] == "run-1"
        }));
        let encoded = serde_json::to_string(&audits).expect("audit serializes");
        assert!(!encoded.contains("contract"));

        let runs = list_external_tool_runs(&storage, "project-1", "recording-1")
            .expect("list external runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "run-1");

        let receipt = revoke_meeting_tool_grant(
            &storage,
            MeetingToolRevokeInput {
                grant_id: "grant-1".into(),
                project_id: "project-1".into(),
                recording_id: "recording-1".into(),
            },
            "2026-08-11T04:01:00Z",
        )
        .expect("revoke grant");
        assert_eq!(receipt.grant_id, "grant-1");
        assert_eq!(receipt.revoked_at, "2026-08-11T04:01:00Z");
    }

    #[test]
    fn connector_exit_and_sanitizer_rejection_fail_the_run_without_touching_capture() {
        for (mode, expected) in [
            ("exit-on-call", ExternalMcpErrorCode::ConnectorUnhealthy),
            ("deep-result", ExternalMcpErrorCode::ResultUnsafe),
        ] {
            let (_directory, storage) = open_storage();
            let executable = fixture_executable();
            seed(&storage, &executable.display().to_string());
            let preview = create_meeting_tool_preview(
                &storage,
                MeetingToolSuggestInput {
                    project_id: "project-1".into(),
                    recording_id: "recording-1".into(),
                    connector_id: "connector-1".into(),
                    capability: ConnectorCapability::DocumentsSearch,
                    arguments: serde_json::json!({"query":"contract"}),
                    evidence_refs: vec!["segment-7".into()],
                },
                "2026-08-11T04:00:00Z",
            )
            .expect("create preview");
            let config = StdioConnectorConfig {
                connector_id: "connector-1".into(),
                executable: executable.clone(),
                arguments: vec![mode.into()],
                allowed_tools: vec![AllowedStdioTool {
                    name: "search_documents".into(),
                    capability: ConnectorCapability::DocumentsSearch,
                }],
            };

            assert_eq!(
                execute_approved_preview(
                    &storage,
                    MeetingToolExecuteInput {
                        run_id: format!("run-{mode}"),
                        preview_id: preview.preview.id,
                        approved_preview_hash: preview.preview.arguments_hash,
                        arguments: preview.arguments,
                    },
                    &config,
                    ExternalExecutionLimits {
                        timeout: Duration::from_secs(2),
                    },
                    &ExternalCancellation::new(),
                    "2026-08-11T04:00:01Z",
                    Some("2026-08-11T04:00:02Z"),
                ),
                Err(expected)
            );

            let runs = list_external_tool_runs(&storage, "project-1", "recording-1")
                .expect("list failed run");
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].status, ToolRunStatus::Failed);
            assert_eq!(runs[0].error_code, Some(expected));
            let recordings = crate::genesis_adapter::query(
                &storage,
                "recordings",
                &["status", "duration_ms"],
                vec![crate::genesis_adapter::eq(
                    "recordings",
                    "id",
                    serde_json::json!("recording-1"),
                )],
                1,
            )
            .expect("query capture ledger");
            assert_eq!(recordings[0]["recordings.status"], "recording");
            assert_eq!(recordings[0]["recordings.duration_ms"], 0);
        }
    }

    #[test]
    fn approved_mock_crm_status_read_is_minimized_sanitized_and_persisted() {
        let (_directory, storage) = open_storage();
        let executable = fixture_executable();
        seed(&storage, &executable.display().to_string());
        crate::genesis_adapter::commit_rows(
            &storage,
            vec![
                crate::genesis_adapter::upsert(
                    "external_connections",
                    serde_json::json!({
                        "id":"connector-1", "provider":"fixture", "account_label":"Fixture CRM",
                        "status":"connected", "transport":"stdio", "endpoint":executable,
                        "credential_ref":null,
                        "capabilities_json":["crm.customer_status.read"],
                        "created_at":"2026-08-11T03:00:00Z", "updated_at":"2026-08-11T03:00:00Z"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "meeting_tool_grants",
                    serde_json::json!({
                        "id":"grant-1", "project_id":"project-1", "recording_id":"recording-1",
                        "connector_id":"connector-1",
                        "capabilities_json":["crm.customer_status.read"],
                        "granted_at":"2026-08-11T03:00:00Z", "expires_at":"2026-08-11T05:00:00Z",
                        "revoked_at":null
                    }),
                ),
            ],
        )
        .expect("switch fixture to CRM capability");
        let preview = create_meeting_tool_preview(
            &storage,
            MeetingToolSuggestInput {
                project_id: "project-1".into(),
                recording_id: "recording-1".into(),
                connector_id: "connector-1".into(),
                capability: ConnectorCapability::CrmCustomerStatusRead,
                arguments: serde_json::json!({
                    "customerKey":"customer-42",
                    "fields":["status", "stage"]
                }),
                evidence_refs: vec!["segment-9".into()],
            },
            "2026-08-11T04:00:00Z",
        )
        .expect("create CRM preview");
        let run = execute_approved_preview(
            &storage,
            MeetingToolExecuteInput {
                run_id: "run-crm".into(),
                preview_id: preview.preview.id,
                approved_preview_hash: preview.preview.arguments_hash,
                arguments: preview.arguments,
            },
            &StdioConnectorConfig {
                connector_id: "connector-1".into(),
                executable,
                arguments: vec!["crm".into()],
                allowed_tools: vec![AllowedStdioTool {
                    name: "get_customer_status".into(),
                    capability: ConnectorCapability::CrmCustomerStatusRead,
                }],
            },
            ExternalExecutionLimits {
                timeout: Duration::from_secs(2),
            },
            &ExternalCancellation::new(),
            "2026-08-11T04:00:01Z",
            Some("2026-08-11T04:00:02Z"),
        )
        .expect("execute approved CRM preview");
        assert_eq!(run.status, ToolRunStatus::Completed);
        let results = crate::genesis_adapter::query(
            &storage,
            "external_tool_results",
            &["sanitized_payload_json", "source_refs_json"],
            vec![],
            1,
        )
        .expect("query CRM result");
        assert_eq!(
            results[0]["external_tool_results.sanitized_payload_json"]["status"],
            "active"
        );
        assert_eq!(
            results[0]["external_tool_results.source_refs_json"][0],
            "crm://customers/customer-42"
        );
    }

    #[test]
    fn runtime_register_cancel_finish_is_duplicate_safe_and_bounded() {
        let runtime = ExternalMcpRuntime::default();
        let cancellation = runtime.register("run-1").expect("register run");
        assert_eq!(runtime.active_count(), 1);
        assert!(matches!(
            runtime.register("run-1"),
            Err(ExternalMcpErrorCode::ApprovalRequired)
        ));
        assert!(runtime.cancel("run-1"));
        assert!(cancellation.is_cancelled());
        runtime.finish("run-1");
        assert_eq!(runtime.active_count(), 0);
        assert!(!runtime.cancel("run-1"));
    }

    #[test]
    fn connector_register_list_grant_and_disconnect_use_keyring_and_genesis_only() {
        let (_directory, storage) = open_storage();
        let executable = fixture_executable();
        let mut keyring = FakeCredentialStore::default();
        let connector = register_external_connector(
            &storage,
            &mut keyring,
            ExternalConnectorRegisterInput {
                id: "connector-ui".into(),
                account_label: "Local knowledge fixture".into(),
                executable: executable.display().to_string(),
                capabilities: vec![ConnectorCapability::DocumentsSearch],
                credential: Some("fixture-secret".into()),
            },
            "2026-08-11T05:00:00Z",
        )
        .expect("register connector");
        assert_eq!(connector.status, "connected");
        assert!(connector.credential_ref.is_some());
        assert_eq!(keyring.values.len(), 1);

        let connectors = list_external_connectors(&storage).expect("list connectors");
        assert_eq!(connectors, vec![connector.clone()]);
        let encoded = serde_json::to_string(&connectors).expect("connector summaries serialize");
        assert!(!encoded.contains("fixture-secret"));

        crate::genesis_adapter::commit_rows(
            &storage,
            vec![
                crate::genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id":"project-ui", "name":"UI meeting", "storage_path":"test-path",
                        "active_recording_id":"recording-ui", "created_at":"t", "updated_at":"t"
                    }),
                ),
                crate::genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id":"recording-ui", "project_id":"project-ui", "source":"microphone",
                        "input_path":null, "canonical_audio_path":"recordings/ui.wav",
                        "status":"recording", "duration_ms":0, "created_at":"t", "updated_at":"t"
                    }),
                ),
            ],
        )
        .expect("seed meeting scope");
        let grant = ensure_meeting_tool_grant(
            &storage,
            "project-ui",
            "recording-ui",
            "connector-ui",
            ConnectorCapability::DocumentsSearch,
            "2026-08-11T05:01:00Z",
            "2026-08-11T05:16:00Z",
        )
        .expect("create meeting grant");
        assert_eq!(grant.expires_at, "2026-08-11T05:16:00Z");

        let receipt = disconnect_external_connector(
            &storage,
            &mut keyring,
            "connector-ui",
            "2026-08-11T05:02:00Z",
        )
        .expect("disconnect connector");
        assert_eq!(receipt.revoked_grants, 1);
        assert!(keyring.values.is_empty());
        assert_eq!(
            list_external_connectors(&storage).unwrap()[0].status,
            "disconnected"
        );
    }

    #[test]
    fn tauri_command_surface_registers_the_approved_sprint_four_commands() {
        let commands = include_str!("external_mcp_commands.rs");
        let lib = include_str!("lib.rs");
        for command in [
            "external_connectors_list",
            "external_connector_register",
            "external_connector_disconnect",
            "meeting_tool_suggest",
            "meeting_tool_execute",
            "meeting_tool_cancel",
            "meeting_tool_revoke",
            "meeting_tool_runs_list",
        ] {
            assert!(commands.contains(&format!("fn {command}(")));
            assert!(lib.contains(&format!("external_mcp_commands::{command}")));
        }
        for forbidden in [
            ["raw", "_credential"].concat(),
            ["raw", "Credential"].concat(),
        ] {
            assert!(!commands.contains(&forbidden));
        }
    }
}
