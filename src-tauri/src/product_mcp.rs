//! Local-only Product MCP application boundary.
//!
//! Grants are explicit, project-bound, default-deny, and backed by a bearer
//! whose digest is the only token material stored in GenesisBlockDB. The
//! transport is deliberately started only from Settings.

use crate::{
    genesis_adapter::{self},
    job_engine::{self, JobEngine, JobKind, TranscriptionMode},
};
use genesis_block_native::RelationalFilter;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::State;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::product_mcp_transport::{
    GrantIdentity, HandlerReply, ParsedRequest, ProductMcpConfig, ProductMcpServer,
};

const DEFAULT_PORT: u16 = 49_175;
const PROTOCOL_VERSION: &str = "2026-07-28";
const KEYRING_SERVICE: &str = "FUNG";
const KEYRING_PREFIX: &str = "product-mcp/";
const MAX_JOB_SCAN_ROWS_PER_REQUEST: u32 = genesis_adapter::ROW_CAP;
const MAX_JOB_CURSOR_BYTES: usize = 1024;
const SUPPORTED_SCOPES: &[&str] = &["workspace.read", "job.control"];
const GLOBAL_TOOLS: &[&str] = &[
    "fung.health",
    "fung.capabilities.get",
    "fung.permissions.get",
];

pub(crate) struct Runtime {
    port: u16,
    server: Option<ProductMcpServer>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            server: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerStatus {
    pub(crate) running: bool,
    pub(crate) bind: String,
    pub(crate) port: u16,
    pub(crate) endpoint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GrantInput {
    pub(crate) client_name: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) project_scope: Vec<String>,
    pub(crate) all_projects: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GrantSummary {
    pub(crate) id: String,
    pub(crate) client_name: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) project_scope: Vec<String>,
    pub(crate) all_projects: bool,
    pub(crate) active: bool,
    pub(crate) created_at: String,
    pub(crate) expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatedGrant {
    pub(crate) grant: GrantSummary,
    /// Returned once to the caller; never persisted outside OS Credential
    /// Manager and never included in the grant list or audit ledger.
    pub(crate) token: String,
}

#[derive(Debug, Clone)]
struct Grant {
    id: String,
    client_name: String,
    client_version: Option<String>,
    workspace_id: String,
    scopes: Vec<String>,
    project_scope: Vec<String>,
    all_projects: bool,
    credential_account: String,
    active: bool,
    expires_at: Option<String>,
    created_at: String,
}

impl Grant {
    fn summary(&self) -> GrantSummary {
        GrantSummary {
            id: self.id.clone(),
            client_name: self.client_name.clone(),
            scopes: self.scopes.clone(),
            project_scope: self.project_scope.clone(),
            all_projects: self.all_projects,
            active: self.active,
            created_at: self.created_at.clone(),
            expires_at: self.expires_at.clone(),
        }
    }

    fn permits_project(&self, project_id: &str) -> bool {
        self.all_projects
            || self
                .project_scope
                .iter()
                .any(|allowed| allowed == project_id)
    }

    fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|allowed| allowed == scope)
    }
}

pub(crate) fn workspace_id(data_root: &Path) -> io::Result<String> {
    fs::create_dir_all(data_root)?;
    let path = data_root.join("product-mcp-workspace-id");
    if let Ok(value) = fs::read_to_string(&path) {
        let value = value.trim();
        if Uuid::parse_str(value).is_ok() {
            return Ok(value.to_owned());
        }
    }
    let value = Uuid::new_v4().to_string();
    let temporary = data_root.join(format!(".product-mcp-workspace-id-{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, format!("{value}\n"))?;
    match fs::rename(&temporary, &path) {
        Ok(()) => Ok(value),
        Err(error) if path.exists() => {
            let _ = fs::remove_file(&temporary);
            let existing = fs::read_to_string(path)?;
            let existing = existing.trim().to_owned();
            if Uuid::parse_str(&existing).is_ok() {
                Ok(existing)
            } else {
                Err(error)
            }
        }
        Err(error) => {
            let _ = fs::remove_file(temporary);
            Err(error)
        }
    }
}

pub(crate) fn status(runtime: &Runtime) -> ServerStatus {
    ServerStatus {
        running: runtime.server.is_some(),
        bind: "127.0.0.1".to_owned(),
        port: runtime.port,
        endpoint: format!("http://127.0.0.1:{}/mcp", runtime.port),
    }
}

pub(crate) fn start(
    runtime: &mut Runtime,
    storage: Arc<genesis_block_native::Storage>,
    jobs: JobEngine,
    workspace_id: String,
    operations: Arc<Mutex<()>>,
    port: u16,
) -> Result<ServerStatus, String> {
    if !(49_152..=65_535).contains(&port) {
        return Err("port must be between 49152 and 65535".to_owned());
    }
    if runtime.server.is_some() {
        return Err("Product MCP is already running".to_owned());
    }
    let auth_storage = Arc::clone(&storage);
    let auth_workspace_id = workspace_id.clone();
    let auth =
        move |request: &ParsedRequest| authenticate(&auth_storage, &auth_workspace_id, request);
    let handler = move |request: &ParsedRequest, grant: &GrantIdentity| {
        dispatch(&storage, &jobs, &workspace_id, &operations, request, grant)
    };
    let server = ProductMcpServer::start(
        ProductMcpConfig {
            port,
            ..ProductMcpConfig::default()
        },
        auth,
        handler,
    )
    .map_err(|error| format!("could not start Product MCP on loopback: {error}"))?;
    runtime.port = server.local_addr().port();
    runtime.server = Some(server);
    Ok(status(runtime))
}

pub(crate) fn stop(runtime: &mut Runtime) -> ServerStatus {
    if let Some(mut server) = runtime.server.take() {
        server.stop();
    }
    status(runtime)
}

pub(crate) fn shutdown(runtime: &Mutex<Runtime>) {
    let mut runtime = runtime
        .lock()
        .expect("Product MCP runtime mutex poisoned during shutdown");
    let _ = stop(&mut runtime);
}

fn authenticate(
    storage: &genesis_block_native::Storage,
    workspace_id: &str,
    request: &ParsedRequest,
) -> Result<GrantIdentity, HandlerReply> {
    let token = request
        .authorization
        .as_deref()
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .ok_or_else(|| http_error(401, request, -32001, "Bearer grant required"))?;
    let digest = sha256(token.as_bytes());
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_grants",
        &[
            "id",
            "workspace_id",
            "client_name",
            "client_version",
            "scopes_json",
            "project_scope_json",
            "all_projects",
            "credential_account",
            "active",
            "expires_at",
            "created_at",
        ],
        vec![genesis_adapter::eq(
            "product_mcp_grants",
            "token_sha256",
            json!(digest),
        )],
        1,
    )
    .map_err(|_| http_error(503, request, -32003, "Grant store unavailable"))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| http_error(401, request, -32001, "Invalid or revoked grant"))?;
    let grant = grant_from_row(&row)
        .map_err(|_| http_error(503, request, -32003, "Grant store unavailable"))?;
    let expired = grant
        .expires_at
        .as_deref()
        .is_some_and(|expires_at| expires_at <= crate::now().as_str());
    if !grant.active || expired || grant.workspace_id != workspace_id {
        return Err(http_error(401, request, -32001, "Invalid or revoked grant"));
    }
    GrantIdentity::new(grant.id)
        .ok_or_else(|| http_error(401, request, -32001, "Invalid grant identity"))
}

fn grant_by_id(storage: &genesis_block_native::Storage, grant_id: &str) -> Result<Grant, String> {
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_grants",
        &[
            "id",
            "workspace_id",
            "client_name",
            "client_version",
            "scopes_json",
            "project_scope_json",
            "all_projects",
            "credential_account",
            "active",
            "expires_at",
            "created_at",
        ],
        vec![genesis_adapter::eq(
            "product_mcp_grants",
            "id",
            json!(grant_id),
        )],
        1,
    )?;
    let row = rows.into_iter().next().ok_or("grant not found")?;
    grant_from_row(&row)
}

fn grant_from_row(row: &Value) -> Result<Grant, String> {
    let get_string = |key: &str| {
        row.get(format!("product_mcp_grants.{key}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("grant field {key} unavailable"))
    };
    let get_optional = |key: &str| {
        row.get(format!("product_mcp_grants.{key}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let get_json_list = |key: &str| -> Result<Vec<String>, String> {
        let value = row
            .get(format!("product_mcp_grants.{key}"))
            .ok_or_else(|| format!("grant field {key} unavailable"))?;
        match value {
            Value::Array(_) => {
                serde_json::from_value(value.clone()).map_err(|error| error.to_string())
            }
            Value::String(serialized) => {
                serde_json::from_str(serialized).map_err(|error| error.to_string())
            }
            _ => Err(format!("grant field {key} is invalid")),
        }
    };
    Ok(Grant {
        id: get_string("id")?,
        workspace_id: get_string("workspace_id")?,
        client_name: get_string("client_name")?,
        client_version: get_optional("client_version"),
        scopes: get_json_list("scopes_json")?,
        project_scope: get_json_list("project_scope_json")?,
        all_projects: row
            .get("product_mcp_grants.all_projects")
            .and_then(Value::as_bool)
            .ok_or("grant all_projects unavailable")?,
        credential_account: get_string("credential_account")?,
        active: row
            .get("product_mcp_grants.active")
            .and_then(Value::as_bool)
            .ok_or("grant active unavailable")?,
        expires_at: get_optional("expires_at"),
        created_at: get_string("created_at")?,
    })
}

fn dispatch(
    storage: &genesis_block_native::Storage,
    jobs: &JobEngine,
    workspace_id: &str,
    operations: &Mutex<()>,
    request: &ParsedRequest,
    identity: &GrantIdentity,
) -> HandlerReply {
    let grant = match grant_by_id(storage, &identity.grant_id) {
        Ok(grant) if grant.active && grant.workspace_id == workspace_id => grant,
        _ => return http_error(401, request, -32001, "Invalid or revoked grant"),
    };
    if !request.params.is_object() {
        return protocol_error(request, -32602, "Request params must be an object");
    }
    match request.method.as_str() {
        "ping" => rpc_result(request, json!({})),
        "server/discover" => rpc_result(
            request,
            json!({
                "resultType": "complete",
                "supportedVersions": [PROTOCOL_VERSION],
                "capabilities": {"tools": {}},
                "_meta": {
                    "io.modelcontextprotocol/serverInfo": {
                        "name": "FUNG Desktop",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                },
                "instructions": "FUNG local Desktop tools provide scoped access to approved project data and job controls.",
                "ttlMs": 300000,
                "cacheScope": "public"
            }),
        ),
        "tools/list" => rpc_result(request, tools_list(&grant)),
        "tools/call" => call_tool(storage, jobs, operations, request, &grant),
        _ => http_error(404, request, -32601, "Method not found"),
    }
}

fn call_tool(
    storage: &genesis_block_native::Storage,
    jobs: &JobEngine,
    operations: &Mutex<()>,
    request: &ParsedRequest,
    grant: &Grant,
) -> HandlerReply {
    let name = request.tool_name.as_deref().unwrap_or_default();
    if request
        .params
        .get("arguments")
        .is_some_and(|arguments| !arguments.is_object())
    {
        let error = domain_error(
            "validation.failed",
            "arguments must be an object",
            false,
            None,
        );
        if audit(
            storage,
            grant,
            name,
            required_scopes(name),
            "validation.failed",
            &[],
            None,
        )
        .is_err()
        {
            return domain_result(
                request,
                domain_error(
                    "storage.unavailable",
                    "Could not record the local audit event",
                    true,
                    None,
                ),
            );
        }
        return domain_result(request, error);
    }
    let arguments = request
        .params
        .get("arguments")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let required_scopes = required_scopes(name);
    if !required_scopes.iter().all(|scope| grant.has_scope(scope)) {
        let error = domain_error(
            "permission.denied",
            "Grant does not include the required scope",
            false,
            None,
        );
        if audit(storage, grant, name, required_scopes, "denied", &[], None).is_err() {
            return domain_result(
                request,
                domain_error(
                    "storage.unavailable",
                    "Could not record the local audit event",
                    true,
                    None,
                ),
            );
        }
        return domain_result(request, error);
    }
    if !is_tool_visible(grant, name) {
        return protocol_error(request, -32602, "Unknown tool or invalid tool arguments");
    }
    if let Err(message) = validate_arguments(name, &arguments) {
        let error = domain_error("validation.failed", &message, false, None);
        if audit(
            storage,
            grant,
            name,
            required_scopes,
            "validation.failed",
            &[],
            None,
        )
        .is_err()
        {
            return domain_result(
                request,
                domain_error(
                    "storage.unavailable",
                    "Could not record the local audit event",
                    true,
                    None,
                ),
            );
        }
        return domain_result(request, error);
    }
    let result = match name {
        "fung.health" => Ok(json!({
            "status": "ready",
            "app": "FUNG",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_revision": PROTOCOL_VERSION,
            "storage_available": true
        })),
        "fung.capabilities.get" => Ok(capabilities(grant)),
        "fung.permissions.get" => Ok(json!({
            "grant_id": grant.id,
            "active": grant.active,
            "workspace_id": grant.workspace_id,
            "scopes": grant.scopes,
            "project_scope": grant.project_scope,
            "all_projects": grant.all_projects,
            "expires_at": grant.expires_at
        })),
        "fung.job.list" => list_jobs_for_grant(storage, grant, &arguments),
        "fung.job.get" => get_job_for_grant(storage, grant, &arguments),
        "fung.job.control" => {
            let _operation = operations
                .lock()
                .expect("Product MCP operation mutex poisoned");
            control_job(storage, jobs, grant, &arguments)
        }
        _ => Err(domain_error(
            "feature.not_implemented",
            "Tool is not implemented",
            false,
            None,
        )),
    };
    match result {
        Ok(value) => {
            let object_ids = object_ids(&value);
            if name != "fung.job.control"
                && audit(
                    storage,
                    grant,
                    name,
                    required_scopes,
                    "succeeded",
                    &object_ids,
                    None,
                )
                .is_err()
            {
                return domain_result(
                    request,
                    domain_error(
                        "storage.unavailable",
                        "Could not record the local audit event",
                        true,
                        None,
                    ),
                );
            }
            tool_result(request, value, false)
        }
        Err(error) => {
            let outcome = error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("failed");
            if audit(storage, grant, name, required_scopes, outcome, &[], None).is_err() {
                return domain_result(
                    request,
                    domain_error(
                        "storage.unavailable",
                        "Could not record the local audit event",
                        true,
                        None,
                    ),
                );
            }
            domain_result(request, error)
        }
    }
}

fn required_scopes(name: &str) -> &'static [&'static str] {
    match name {
        "fung.job.list" | "fung.job.get" => &["workspace.read"],
        "fung.job.control" => &["job.control"],
        _ => &[],
    }
}

fn validate_arguments(name: &str, arguments: &Value) -> Result<(), String> {
    let object = arguments.as_object().ok_or("arguments must be an object")?;
    let (allowed, required): (&[&str], &[&str]) = match name {
        "fung.health" | "fung.capabilities.get" | "fung.permissions.get" => (&[], &[]),
        "fung.job.list" => (&["project_id", "page_size", "cursor"], &[]),
        "fung.job.get" => (&["job_id"], &["job_id"]),
        "fung.job.control" => (
            &["job_id", "action", "idempotency_key"],
            &["job_id", "action", "idempotency_key"],
        ),
        _ => return Err("unknown tool".to_owned()),
    };
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unexpected argument: {key}"));
    }
    for key in required {
        if !object.contains_key(*key) {
            return Err(format!("{key} is required"));
        }
    }
    for key in ["project_id", "job_id", "idempotency_key"] {
        if let Some(value) = object.get(key) {
            if !value.as_str().is_some_and(|text| !text.trim().is_empty()) {
                return Err(format!("{key} must be a non-empty string"));
            }
        }
    }
    if object
        .get("page_size")
        .is_some_and(|value| !value.as_u64().is_some_and(|size| (1..=100).contains(&size)))
    {
        return Err("page_size must be an integer between 1 and 100".to_owned());
    }
    if let Some(cursor) = object.get("cursor") {
        if !cursor.is_null()
            && !cursor
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.len() <= MAX_JOB_CURSOR_BYTES)
        {
            return Err("cursor must be a non-empty string or null".to_owned());
        }
    }
    if let Some(action) = object.get("action") {
        if !matches!(action.as_str(), Some("cancel" | "retry")) {
            return Err("action must be cancel or retry".to_owned());
        }
    }
    if object
        .get("idempotency_key")
        .and_then(Value::as_str)
        .is_some_and(|key| {
            key.len() > 128 || !key.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        })
    {
        return Err("idempotency_key must contain 1-128 printable ASCII characters".to_owned());
    }
    Ok(())
}

fn is_tool_visible(grant: &Grant, name: &str) -> bool {
    GLOBAL_TOOLS.contains(&name)
        || (name == "fung.job.list" || name == "fung.job.get") && grant.has_scope("workspace.read")
        || name == "fung.job.control" && grant.has_scope("job.control")
}

fn capabilities(grant: &Grant) -> Value {
    let names = tool_names(grant);
    let mut features = vec![
        "health".to_owned(),
        "capabilities".to_owned(),
        "permissions".to_owned(),
    ];
    if grant.has_scope("workspace.read") {
        features.push("jobs.read".to_owned());
    }
    if grant.has_scope("job.control") {
        features.push("jobs.control".to_owned());
    }
    json!({
        "protocol_revision": PROTOCOL_VERSION,
        "features": features,
        "limits": {"default_page_size": 20, "max_page_size": 100, "max_request_bytes": 1048576},
        "tool_names": names
    })
}

fn tool_names(grant: &Grant) -> Vec<&'static str> {
    let mut names = GLOBAL_TOOLS.to_vec();
    if grant.has_scope("workspace.read") {
        names.extend(["fung.job.list", "fung.job.get"]);
    }
    if grant.has_scope("job.control") {
        names.push("fung.job.control");
    }
    names
}

fn tools_list(grant: &Grant) -> Value {
    let tools = tool_names(grant)
        .into_iter()
        .map(tool_definition)
        .collect::<Vec<_>>();
    json!({"resultType": "complete", "tools": tools, "ttlMs": 0, "cacheScope": "private"})
}

fn tool_definition(name: &str) -> Value {
    let (description, input_schema, output_schema, annotations) = match name {
        "fung.health" => ("Return local FUNG and Genesis availability without exposing paths or project content.", json!({"type":"object","properties":{},"additionalProperties":false}), json!({"type":"object","required":["status","app","version","protocol_revision","storage_available"],"properties":{"status":{"type":"string","enum":["ready","degraded"]},"app":{"type":"string"},"version":{"type":"string"},"protocol_revision":{"type":"string"},"storage_available":{"type":"boolean"}},"additionalProperties":false}), read_only_annotations()),
        "fung.capabilities.get" => ("Return protocol features, limits, and tools available to this grant.", json!({"type":"object","properties":{},"additionalProperties":false}), json!({"type":"object","required":["protocol_revision","features","limits","tool_names"],"properties":{"protocol_revision":{"type":"string"},"features":{"type":"array","items":{"type":"string"}},"limits":{"type":"object","required":["default_page_size","max_page_size","max_request_bytes"],"properties":{"default_page_size":{"type":"integer","minimum":1},"max_page_size":{"type":"integer","minimum":1},"max_request_bytes":{"type":"integer","minimum":1}},"additionalProperties":false},"tool_names":{"type":"array","items":{"type":"string"}}},"additionalProperties":false}), read_only_annotations()),
        "fung.permissions.get" => ("Return only this local grant's scopes and workspace/project binding.", json!({"type":"object","properties":{},"additionalProperties":false}), json!({"type":"object","required":["grant_id","active","workspace_id","scopes","project_scope","all_projects"],"properties":{"grant_id":{"type":"string"},"active":{"type":"boolean"},"workspace_id":{"type":"string"},"scopes":{"type":"array","items":{"type":"string"}},"project_scope":{"type":"array","items":{"type":"string"}},"all_projects":{"type":"boolean"},"expires_at":{"type":["string","null"],"format":"date-time"}},"additionalProperties":false}), read_only_annotations()),
        "fung.job.list" => ("List durable jobs visible to this grant, optionally limited to one authorized project.", json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1},"page_size":{"type":"integer","minimum":1,"maximum":100,"default":20},"cursor":{"type":["string","null"]}},"additionalProperties":false}), json!({"type":"object","required":["items","next_cursor"],"properties":{"items":{"type":"array","items":{"type":"object","required":["job_id","project_id","kind","state","created_at","updated_at"],"properties":{"job_id":{"type":"string"},"project_id":{"type":"string"},"kind":{"type":"string"},"state":{"type":"string"},"created_at":{"type":"string","format":"date-time"},"updated_at":{"type":"string","format":"date-time"}},"additionalProperties":false}},"next_cursor":{"type":["string","null"]}},"additionalProperties":false}), read_only_annotations()),
        "fung.job.get" => ("Read the current authorized durable-job status and related object IDs.", json!({"type":"object","required":["job_id"],"properties":{"job_id":{"type":"string","minLength":1}},"additionalProperties":false}), json!({"type":"object","required":["job_id","kind","state","created_at","updated_at","related_object_ids"],"properties":{"job_id":{"type":"string"},"project_id":{"type":"string"},"kind":{"type":"string"},"state":{"type":"string"},"created_at":{"type":"string","format":"date-time"},"updated_at":{"type":"string","format":"date-time"},"progress":{"type":"integer","minimum":0,"maximum":100},"related_object_ids":{"type":"array","items":{"type":"string"}},"result_summary":{"type":["string","null"]}},"additionalProperties":false}), read_only_annotations()),
        "fung.job.control" => ("Cancel or retry a supported authorized local job with durable idempotency.", json!({"type":"object","required":["job_id","action","idempotency_key"],"properties":{"job_id":{"type":"string","minLength":1},"action":{"type":"string","enum":["cancel","retry"]},"idempotency_key":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[\\x20-\\x7E]+$"}},"additionalProperties":false}), json!({"type":"object","required":["job_id","action","accepted","state"],"properties":{"job_id":{"type":"string"},"action":{"type":"string","enum":["cancel","retry"]},"accepted":{"type":"boolean"},"state":{"type":"string"},"retry_job_id":{"type":["string","null"]}},"additionalProperties":false}), json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false})),
        _ => ("", json!({"type":"object"}), json!({"type":"object"}), json!({})),
    };
    json!({"name": name, "description": description, "inputSchema": input_schema, "outputSchema": output_schema, "annotations": annotations})
}

fn read_only_annotations() -> Value {
    json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false})
}

fn list_jobs_for_grant(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    arguments: &Value,
) -> Result<Value, Value> {
    let project_filter = optional_string(arguments, "project_id")?;
    if let Some(project_id) = project_filter.as_deref() {
        if !grant.permits_project(project_id) {
            return Err(domain_error(
                "permission.denied",
                "Project is outside this grant",
                false,
                None,
            ));
        }
    }
    let page_size = arguments
        .get("page_size")
        .and_then(Value::as_u64)
        .unwrap_or(20);
    if !(1..=100).contains(&page_size) {
        return Err(domain_error(
            "validation.failed",
            "page_size must be between 1 and 100",
            false,
            None,
        ));
    }
    let cursor = optional_string(arguments, "cursor")?;
    let (mut scan_offset, mut last_scanned_id) = cursor
        .as_deref()
        .map(|cursor| decode_job_cursor(cursor, &grant.id, project_filter.as_deref()))
        .transpose()
        .map_err(|_| domain_error("validation.failed", "cursor is invalid", false, None))?
        .unwrap_or((0, None));
    let filters: Vec<RelationalFilter> = project_filter
        .as_deref()
        .map(|project_id| genesis_adapter::eq("jobs", "project_id", json!(project_id)))
        .into_iter()
        .collect();
    let columns = [
        "id",
        "project_id",
        "type",
        "status",
        "created_at",
        "updated_at",
    ];
    let mut jobs: Vec<(String, String, String, String, String, String)> = Vec::new();
    let mut scanned_rows = 0_u32;
    let mut exhausted = false;
    let mut lookahead_offset = None;

    while scanned_rows < MAX_JOB_SCAN_ROWS_PER_REQUEST {
        let query_limit =
            (MAX_JOB_SCAN_ROWS_PER_REQUEST - scanned_rows).min(genesis_adapter::ROW_CAP);
        let page_start = scan_offset;
        let rows = genesis_adapter::query_page(
            storage,
            "jobs",
            &columns,
            filters.clone(),
            query_limit,
            page_start,
        )
        .map_err(|_| {
            domain_error(
                "storage.unavailable",
                "Job store is unavailable",
                true,
                None,
            )
        })?;

        if rows.is_empty() {
            exhausted = true;
            break;
        }

        let row_count = rows.len() as u32;
        let mut processed = 0_u32;
        for (index, row) in rows.iter().enumerate() {
            let row_offset = page_start.checked_add(index as u32).ok_or_else(|| {
                domain_error(
                    "storage.unavailable",
                    "Job history exceeds the supported cursor range",
                    false,
                    None,
                )
            })?;
            let Some(id) = row
                .get("jobs.id")
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                processed = index as u32 + 1;
                continue;
            };
            if last_scanned_id
                .as_deref()
                .is_some_and(|last_id| id.as_str() <= last_id)
            {
                processed = index as u32 + 1;
                continue;
            }
            let project = row
                .get("jobs.project_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let permitted = project
                .as_deref()
                .is_some_and(|project_id| grant.permits_project(project_id));
            if permitted {
                if jobs.len() == page_size as usize {
                    // Keep the cursor before this lookahead row so the next
                    // page returns it rather than skipping it.
                    lookahead_offset = Some(row_offset);
                    break;
                }
                let Some(kind) = row
                    .get("jobs.type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    last_scanned_id = Some(id);
                    processed = index as u32 + 1;
                    continue;
                };
                let Some(state) = row
                    .get("jobs.status")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    last_scanned_id = Some(id);
                    processed = index as u32 + 1;
                    continue;
                };
                let Some(created_at) = row
                    .get("jobs.created_at")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    last_scanned_id = Some(id);
                    processed = index as u32 + 1;
                    continue;
                };
                let Some(updated_at) = row
                    .get("jobs.updated_at")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    last_scanned_id = Some(id);
                    processed = index as u32 + 1;
                    continue;
                };
                jobs.push((
                    id.clone(),
                    project.unwrap_or_default(),
                    kind,
                    state,
                    created_at,
                    updated_at,
                ));
            }
            last_scanned_id = Some(id);
            processed = index as u32 + 1;
        }

        if let Some(offset) = lookahead_offset {
            scan_offset = offset;
            break;
        }

        scan_offset = page_start.checked_add(processed).ok_or_else(|| {
            domain_error(
                "storage.unavailable",
                "Job history exceeds the supported cursor range",
                false,
                None,
            )
        })?;
        scanned_rows = scanned_rows.saturating_add(processed);
        if row_count < query_limit {
            exhausted = true;
            break;
        }
    }

    let next_cursor_position = if lookahead_offset.is_some() {
        Some(scan_offset)
    } else if exhausted {
        None
    } else {
        let more_rows =
            genesis_adapter::query_page(storage, "jobs", &["id"], filters.clone(), 1, scan_offset)
                .map_err(|_| {
                    domain_error(
                        "storage.unavailable",
                        "Job store is unavailable",
                        true,
                        None,
                    )
                })?
                .first()
                .is_some();
        more_rows.then_some(scan_offset)
    };
    let next_cursor = next_cursor_position
        .map(|offset| {
            encode_job_cursor(
                &grant.id,
                project_filter.as_deref(),
                offset,
                last_scanned_id.as_deref(),
            )
        })
        .transpose()
        .map_err(|_| {
            domain_error(
                "storage.unavailable",
                "Could not encode the job-list cursor",
                false,
                None,
            )
        })?;
    let items = jobs
        .into_iter()
        .map(|(id, project_id, kind, state, created_at, updated_at)| {
            json!({"job_id":id,"project_id":project_id,"kind":kind,"state":state,"created_at":created_at,"updated_at":updated_at})
        })
        .collect::<Vec<_>>();
    Ok(json!({"items":items,"next_cursor":next_cursor}))
}

fn get_job_for_grant(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    arguments: &Value,
) -> Result<Value, Value> {
    let id = required_string(arguments, "job_id")?;
    let rows = genesis_adapter::query(
        storage,
        "jobs",
        &[
            "id",
            "project_id",
            "type",
            "status",
            "progress",
            "input_refs_json",
            "output_refs_json",
            "created_at",
            "updated_at",
        ],
        vec![genesis_adapter::eq("jobs", "id", json!(id))],
        1,
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Job store is unavailable",
            true,
            None,
        )
    })?;
    let row = rows.into_iter().next().ok_or_else(|| {
        domain_error(
            "resource.not_found",
            "Job was not found",
            false,
            Some(id.clone()),
        )
    })?;
    let project_id = row_string(&row, "project_id")?;
    if !grant.permits_project(&project_id) {
        return Err(domain_error(
            "resource.not_found",
            "Job was not found",
            false,
            Some(id),
        ));
    }
    let input = row_list(&row, "input_refs_json");
    let output = row_list(&row, "output_refs_json");
    let related = input
        .into_iter()
        .chain(output)
        .filter(|value| !value.starts_with("transcription-mode:"))
        .collect::<Vec<_>>();
    Ok(json!({
        "job_id": row_string(&row,"id")?,
        "project_id": project_id,
        "kind": row_string(&row,"type")?,
        "state": row_string(&row,"status")?,
        "created_at": row_string(&row,"created_at")?,
        "updated_at": row_string(&row,"updated_at")?,
        "progress": row.get("jobs.progress").and_then(Value::as_i64).unwrap_or(0),
        "related_object_ids": related,
        "result_summary": Value::Null
    }))
}

fn control_job(
    storage: &genesis_block_native::Storage,
    jobs: &JobEngine,
    grant: &Grant,
    arguments: &Value,
) -> Result<Value, Value> {
    let job_id = required_string(arguments, "job_id")?;
    let action = required_string(arguments, "action")?;
    let idempotency_key = required_string(arguments, "idempotency_key")?;
    if idempotency_key.len() > 128
        || !idempotency_key
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err(domain_error(
            "validation.failed",
            "idempotency_key must contain 1-128 printable ASCII characters",
            false,
            None,
        ));
    }
    let request_hash = sha256(&canonical_json(arguments));
    let key_hash = sha256(idempotency_key.as_bytes());
    if let Some(cached) = find_idempotency(storage, grant, key_hash.as_str())? {
        if cached.get("request_sha256").and_then(Value::as_str) != Some(&request_hash) {
            return Err(domain_error(
                "idempotency.key_reused",
                "Idempotency key was already used for a different request",
                false,
                Some(job_id),
            ));
        }
        match cached.get("operation_state").and_then(Value::as_str) {
            Some("completed") => {
                let result = cached.get("result_json").cloned().ok_or_else(|| {
                    domain_error(
                        "idempotency.outcome_unknown",
                        "The original operation result is unavailable",
                        false,
                        Some(job_id.clone()),
                    )
                })?;
                audit(
                    storage,
                    grant,
                    "fung.job.control",
                    &["job.control"],
                    "idempotency.replay",
                    &object_ids(&result),
                    Some(&key_hash),
                )
                .map_err(|_| {
                    domain_error(
                        "storage.unavailable",
                        "Could not record the local audit event",
                        true,
                        Some(job_id.clone()),
                    )
                })?;
                return Ok(result);
            }
            Some("completed_error") => {
                audit(
                    storage,
                    grant,
                    "fung.job.control",
                    &["job.control"],
                    "idempotency.replay",
                    &[job_id.clone()],
                    Some(&key_hash),
                )
                .map_err(|_| {
                    domain_error(
                        "storage.unavailable",
                        "Could not record the local audit event",
                        true,
                        Some(job_id.clone()),
                    )
                })?;
                return Err(cached.get("result_json").cloned().unwrap_or_else(|| {
                    domain_error(
                        "idempotency.outcome_unknown",
                        "The original operation result is unavailable",
                        false,
                        Some(job_id),
                    )
                }));
            }
            Some("pending") => {
                return reconcile_pending(storage, jobs, grant, &key_hash, &cached);
            }
            _ => {
                return Err(domain_error(
                    "idempotency.outcome_unknown",
                    "The original operation may have been applied; inspect the job before retrying",
                    true,
                    Some(job_id),
                ))
            }
        }
    }
    let source = get_authorized_job(storage, grant, &job_id)?;
    let row_state = row_string(&source, "status").unwrap_or_default();
    if !matches!(action.as_str(), "cancel" | "retry") {
        return Err(domain_error(
            "validation.failed",
            "action must be cancel or retry",
            false,
            Some(job_id),
        ));
    }
    if action == "retry" && !matches!(row_state.as_str(), "failed" | "cancelled") {
        return Err(domain_error(
            "validation.failed",
            "Only failed or cancelled jobs can be retried",
            false,
            Some(job_id),
        ));
    }
    if action == "cancel" && !matches!(row_state.as_str(), "queued" | "running" | "retrying") {
        return Err(domain_error(
            "job.not_cancellable",
            "Job is not queued or running",
            false,
            Some(job_id),
        ));
    }
    let retry_request = if action == "retry" {
        let kind = row_string(&source, "type")
            .ok()
            .and_then(|kind| JobKind::parse(&kind))
            .ok_or_else(|| {
                domain_error(
                    "validation.failed",
                    "Job type is not retryable",
                    false,
                    Some(job_id.clone()),
                )
            })?;
        let refs = row_list(&source, "input_refs_json");
        let recording_id = refs
            .iter()
            .find(|value| !value.starts_with("transcription-mode:"))
            .cloned()
            .ok_or_else(|| {
                domain_error(
                    "validation.failed",
                    "Job has no recording reference",
                    false,
                    Some(job_id.clone()),
                )
            })?;
        let mode = if refs
            .iter()
            .any(|value| value == "transcription-mode:detailed")
        {
            TranscriptionMode::Detailed
        } else {
            TranscriptionMode::General
        };
        let project_id = row_string(&source, "project_id").map_err(|_| {
            domain_error(
                "storage.unavailable",
                "Job data unavailable",
                true,
                Some(job_id.clone()),
            )
        })?;
        Some((kind, project_id, recording_id, mode))
    } else {
        None
    };
    let retry_job_id = retry_request.as_ref().map(|_| Uuid::new_v4().to_string());
    let intent_json = if let Some((kind, project_id, recording_id, mode)) = &retry_request {
        json!({
            "action": "retry",
            "source_job_id": job_id,
            "retry_job_id": retry_job_id,
            "kind": kind.as_str(),
            "project_id": project_id,
            "recording_id": recording_id,
            "transcription_mode": match mode {
                TranscriptionMode::General => "general",
                TranscriptionMode::Detailed => "detailed"
            }
        })
    } else {
        json!({"action":"cancel", "source_job_id":job_id})
    };
    let now = crate::now();
    let idem_id = Uuid::new_v4().to_string();
    genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "product_mcp_idempotency",
            json!({
                "id": idem_id,
                "grant_id": grant.id,
                "tool_name": "fung.job.control",
                "idempotency_key_sha256": key_hash,
                "request_sha256": request_hash,
                "operation_state": "pending",
                "job_id": job_id,
                "intent_json": intent_json,
                "result_json": null,
                "created_at": now,
                "updated_at": now,
                "retain_until": null
            }),
        )],
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Could not record durable operation intent",
            true,
            Some(job_id.clone()),
        )
    })?;
    let (accepted, state, retry_job_id) = if action == "cancel" {
        let outcome = jobs.cancel(&job_id);
        match outcome {
            job_engine::CancelOutcome::Cancelled => (true, "cancelled".to_owned(), None),
            job_engine::CancelOutcome::RequestedWhileRunning => {
                (true, "cancel_requested".to_owned(), None)
            }
            job_engine::CancelOutcome::NotPending => {
                let error = domain_error(
                    "job.not_cancellable",
                    "Job is no longer queued or running",
                    false,
                    Some(job_id.clone()),
                );
                complete_idempotency_error(storage, grant, &key_hash, &job_id, &error)?;
                return Err(error);
            }
        }
    } else {
        let (kind, project_id, recording_id, mode) =
            retry_request.expect("retry input was validated before operation intent");
        let retry_job_id = retry_job_id.expect("retry intent contains a stable job id");
        let new_id = match jobs.enqueue_with_transcription_mode_for_id(
            &retry_job_id,
            kind,
            &project_id,
            Some(&recording_id),
            mode,
        ) {
            Ok(id) => id,
            Err(_) => {
                let error = domain_error(
                    "storage.unavailable",
                    "Could not enqueue the retry",
                    true,
                    Some(job_id.clone()),
                );
                complete_idempotency_error(storage, grant, &key_hash, &job_id, &error)?;
                return Err(error);
            }
        };
        let row = get_authorized_job(storage, grant, &new_id)?;
        let state = row_string(&row, "status").unwrap_or_else(|_| "unknown".to_owned());
        (true, state, Some(new_id))
    };
    let result = json!({"job_id":job_id,"action":action,"accepted":accepted,"state":state,"retry_job_id":retry_job_id});
    complete_idempotency(storage, grant, &key_hash, &job_id, &result)?;
    Ok(result)
}

fn reconcile_pending(
    storage: &genesis_block_native::Storage,
    jobs: &JobEngine,
    grant: &Grant,
    key_hash: &str,
    cached: &Value,
) -> Result<Value, Value> {
    let intent = cached.get("intent_json").ok_or_else(|| {
        domain_error(
            "idempotency.outcome_unknown",
            "The durable operation intent is unavailable; inspect the job before retrying",
            true,
            None,
        )
    })?;
    let action = required_string(intent, "action")?;
    let source_job_id = required_string(intent, "source_job_id")?;
    let result = if action == "retry" {
        let retry_job_id = required_string(intent, "retry_job_id")?;
        let kind_name = required_string(intent, "kind")?;
        let kind = JobKind::parse(&kind_name).ok_or_else(|| {
            domain_error(
                "idempotency.outcome_unknown",
                "The durable retry intent is invalid",
                false,
                Some(source_job_id.clone()),
            )
        })?;
        let project_id = required_string(intent, "project_id")?;
        let recording_id = required_string(intent, "recording_id")?;
        if !grant.permits_project(&project_id) {
            return Err(domain_error(
                "resource.not_found",
                "Job was not found",
                false,
                Some(source_job_id),
            ));
        }
        let mode = match intent.get("transcription_mode").and_then(Value::as_str) {
            Some("general") => TranscriptionMode::General,
            Some("detailed") => TranscriptionMode::Detailed,
            _ => {
                return Err(domain_error(
                    "idempotency.outcome_unknown",
                    "The durable retry intent is invalid",
                    false,
                    Some(source_job_id),
                ));
            }
        };
        jobs.enqueue_with_transcription_mode_for_id(
            &retry_job_id,
            kind,
            &project_id,
            Some(&recording_id),
            mode,
        )
        .map_err(|_| {
            domain_error(
                "storage.unavailable",
                "Could not reconcile the durable retry intent",
                true,
                Some(retry_job_id.clone()),
            )
        })?;
        let row = get_authorized_job(storage, grant, &retry_job_id)?;
        let state = row_string(&row, "status").unwrap_or_else(|_| "unknown".to_owned());
        json!({
            "job_id": source_job_id,
            "action": "retry",
            "accepted": true,
            "state": state,
            "retry_job_id": retry_job_id
        })
    } else if action == "cancel" {
        let source = get_authorized_job(storage, grant, &source_job_id)?;
        let current_state = row_string(&source, "status").unwrap_or_default();
        match current_state.as_str() {
            "cancelled" => json!({
                "job_id": source_job_id,
                "action": "cancel",
                "accepted": true,
                "state": "cancelled",
                "retry_job_id": Value::Null
            }),
            "queued" | "running" | "retrying" => match jobs.cancel(&source_job_id) {
                job_engine::CancelOutcome::Cancelled => json!({
                    "job_id": source_job_id,
                    "action": "cancel",
                    "accepted": true,
                    "state": "cancelled",
                    "retry_job_id": Value::Null
                }),
                job_engine::CancelOutcome::RequestedWhileRunning => json!({
                    "job_id": source_job_id,
                    "action": "cancel",
                    "accepted": true,
                    "state": "cancel_requested",
                    "retry_job_id": Value::Null
                }),
                job_engine::CancelOutcome::NotPending => {
                    return Err(domain_error(
                        "idempotency.outcome_unknown",
                        "Cancellation is still being reconciled; inspect the job and retry this key",
                        true,
                        Some(source_job_id),
                    ));
                }
            },
            terminal_state => json!({
                "job_id": source_job_id,
                "action": "cancel",
                "accepted": false,
                "state": terminal_state,
                "retry_job_id": Value::Null
            }),
        }
    } else {
        return Err(domain_error(
            "idempotency.outcome_unknown",
            "The durable operation intent is invalid",
            false,
            Some(source_job_id),
        ));
    };
    complete_idempotency(storage, grant, key_hash, &source_job_id, &result)?;
    Ok(result)
}

fn get_authorized_job(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    job_id: &str,
) -> Result<Value, Value> {
    let rows = genesis_adapter::query(
        storage,
        "jobs",
        &[
            "id",
            "project_id",
            "type",
            "status",
            "input_refs_json",
            "output_refs_json",
            "created_at",
            "updated_at",
        ],
        vec![genesis_adapter::eq("jobs", "id", json!(job_id))],
        1,
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Job store is unavailable",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    let row = rows.into_iter().next().ok_or_else(|| {
        domain_error(
            "resource.not_found",
            "Job was not found",
            false,
            Some(job_id.to_owned()),
        )
    })?;
    let project_id = row_string(&row, "project_id").map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Job data unavailable",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    if !grant.permits_project(&project_id) {
        return Err(domain_error(
            "resource.not_found",
            "Job was not found",
            false,
            Some(job_id.to_owned()),
        ));
    }
    Ok(row)
}

fn find_idempotency(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    key_hash: &str,
) -> Result<Option<Value>, Value> {
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_idempotency",
        &[
            "id",
            "grant_id",
            "tool_name",
            "idempotency_key_sha256",
            "request_sha256",
            "operation_state",
            "job_id",
            "intent_json",
            "result_json",
            "created_at",
            "updated_at",
            "retain_until",
        ],
        vec![
            genesis_adapter::eq("product_mcp_idempotency", "grant_id", json!(grant.id)),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "tool_name",
                json!("fung.job.control"),
            ),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "idempotency_key_sha256",
                json!(key_hash),
            ),
        ],
        1,
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Idempotency store is unavailable",
            true,
            None,
        )
    })?;
    Ok(rows.into_iter().next().map(|row| {
        let mut output = Map::new();
        for column in [
            "request_sha256",
            "operation_state",
            "intent_json",
            "result_json",
        ] {
            if let Some(value) = row.get(format!("product_mcp_idempotency.{column}")) {
                output.insert(column.to_owned(), value.clone());
            }
        }
        Value::Object(output)
    }))
}

fn complete_idempotency(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    key_hash: &str,
    job_id: &str,
    result: &Value,
) -> Result<(), Value> {
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_idempotency",
        &["id", "request_sha256", "created_at"],
        vec![
            genesis_adapter::eq("product_mcp_idempotency", "grant_id", json!(grant.id)),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "tool_name",
                json!("fung.job.control"),
            ),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "idempotency_key_sha256",
                json!(key_hash),
            ),
        ],
        1,
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Could not finalize durable operation",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    let id = rows
        .first()
        .and_then(|row| row.get("product_mcp_idempotency.id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            domain_error(
                "storage.unavailable",
                "Could not find durable operation intent",
                true,
                Some(job_id.to_owned()),
            )
        })?;
    let updated = crate::now();
    let source = rows.first().expect("id came from first row");
    let request_hash = source
        .get("product_mcp_idempotency.request_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = source
        .get("product_mcp_idempotency.created_at")
        .and_then(Value::as_str)
        .unwrap_or(&updated);
    let retain_until = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    genesis_adapter::commit_rows(
        storage,
        vec![
            genesis_adapter::upsert(
                "product_mcp_idempotency",
                json!({
                "id": id,
                "grant_id": grant.id,
                "tool_name": "fung.job.control",
                "idempotency_key_sha256": key_hash,
                "request_sha256": request_hash,
                "operation_state": "completed",
                "job_id": job_id,
                "intent_json": null,
                "result_json": result,
                "created_at": created_at,
                "updated_at": updated,
                "retain_until": retain_until
                }),
            ),
            genesis_adapter::upsert(
                "product_mcp_audit",
                audit_row(
                    grant,
                    "fung.job.control",
                    &["job.control"],
                    "succeeded",
                    &object_ids(result),
                    Some(key_hash),
                ),
            ),
        ],
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Could not finalize durable operation",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    Ok(())
}

fn complete_idempotency_error(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    key_hash: &str,
    job_id: &str,
    error: &Value,
) -> Result<(), Value> {
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_idempotency",
        &["id", "request_sha256", "created_at"],
        vec![
            genesis_adapter::eq("product_mcp_idempotency", "grant_id", json!(grant.id)),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "tool_name",
                json!("fung.job.control"),
            ),
            genesis_adapter::eq(
                "product_mcp_idempotency",
                "idempotency_key_sha256",
                json!(key_hash),
            ),
        ],
        1,
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Could not finalize durable operation",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    let row = rows.first().ok_or_else(|| {
        domain_error(
            "storage.unavailable",
            "Could not find durable operation intent",
            true,
            Some(job_id.to_owned()),
        )
    })?;
    let id = row
        .get("product_mcp_idempotency.id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            domain_error(
                "storage.unavailable",
                "Could not read durable operation id",
                true,
                Some(job_id.to_owned()),
            )
        })?;
    let request_hash = row
        .get("product_mcp_idempotency.request_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = row
        .get("product_mcp_idempotency.created_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let updated = crate::now();
    let retain_until = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    let outcome = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    genesis_adapter::commit_rows(
        storage,
        vec![
            genesis_adapter::upsert(
                "product_mcp_idempotency",
                json!({
                "id": id,
                "grant_id": grant.id,
                "tool_name": "fung.job.control",
                "idempotency_key_sha256": key_hash,
                "request_sha256": request_hash,
                "operation_state": "completed_error",
                "job_id": job_id,
                "intent_json": null,
                "result_json": error,
                "created_at": created_at,
                "updated_at": updated,
                "retain_until": retain_until
                }),
            ),
            genesis_adapter::upsert(
                "product_mcp_audit",
                audit_row(
                    grant,
                    "fung.job.control",
                    &["job.control"],
                    outcome,
                    &[job_id.to_owned()],
                    Some(key_hash),
                ),
            ),
        ],
    )
    .map_err(|_| {
        domain_error(
            "storage.unavailable",
            "Could not finalize durable operation",
            true,
            Some(job_id.to_owned()),
        )
    })
}

fn audit(
    storage: &genesis_block_native::Storage,
    grant: &Grant,
    tool_name: &str,
    requested_scopes: &[&str],
    outcome: &str,
    object_ids: &[String],
    idempotency_key_hash: Option<&str>,
) -> Result<(), String> {
    genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "product_mcp_audit",
            audit_row(
                grant,
                tool_name,
                requested_scopes,
                outcome,
                object_ids,
                idempotency_key_hash,
            ),
        )],
    )
}

fn audit_row(
    grant: &Grant,
    tool_name: &str,
    requested_scopes: &[&str],
    outcome: &str,
    object_ids: &[String],
    idempotency_key_hash: Option<&str>,
) -> Value {
    json!({
        "id": Uuid::new_v4().to_string(),
        "grant_id": grant.id,
        "tool_name": tool_name,
        "requested_scopes_json": requested_scopes,
        "outcome": outcome,
        "object_ids_json": object_ids,
        "source_revision": null,
        "idempotency_key_sha256": idempotency_key_hash,
        "created_at": crate::now()
    })
}

fn object_ids(result: &Value) -> Vec<String> {
    ["job_id", "retry_job_id"]
        .into_iter()
        .filter_map(|key| result.get(key).and_then(Value::as_str).map(str::to_owned))
        .collect()
}

fn required_string(arguments: &Value, name: &str) -> Result<String, Value> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            domain_error(
                "validation.failed",
                &format!("{name} is required"),
                false,
                None,
            )
        })
}

fn optional_string(arguments: &Value, name: &str) -> Result<Option<String>, Value> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        _ => Err(domain_error(
            "validation.failed",
            &format!("{name} must be a non-empty string or null"),
            false,
            None,
        )),
    }
}

fn row_string(row: &Value, name: &str) -> Result<String, String> {
    row.get(format!("jobs.{name}"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("job field {name} unavailable"))
}

fn row_list(row: &Value, name: &str) -> Vec<String> {
    let Some(value) = row.get(format!("jobs.{name}")) else {
        return Vec::new();
    };
    match value {
        Value::Array(_) => serde_json::from_value(value.clone()).unwrap_or_default(),
        Value::String(raw) => serde_json::from_str(raw).unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobListCursor {
    version: u8,
    grant_id: String,
    project_id: Option<String>,
    offset: u32,
    last_scanned_id: Option<String>,
}

fn encode_job_cursor(
    grant_id: &str,
    project_id: Option<&str>,
    offset: u32,
    last_scanned_id: Option<&str>,
) -> Result<String, ()> {
    use base64::Engine;
    let cursor = JobListCursor {
        version: 1,
        grant_id: grant_id.to_owned(),
        project_id: project_id.map(str::to_owned),
        offset,
        last_scanned_id: last_scanned_id.map(str::to_owned),
    };
    let bytes = serde_json::to_vec(&cursor).map_err(|_| ())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_job_cursor(
    encoded: &str,
    grant_id: &str,
    project_id: Option<&str>,
) -> Result<(u32, Option<String>), ()> {
    use base64::Engine;
    if encoded.len() > MAX_JOB_CURSOR_BYTES {
        return Err(());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| ())?;
    let cursor: JobListCursor = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if cursor.version != 1
        || cursor.grant_id != grant_id
        || cursor.project_id.as_deref() != project_id
        || cursor
            .last_scanned_id
            .as_deref()
            .is_some_and(|id| id.is_empty() || id.len() > 256)
    {
        return Err(());
    }
    Ok((cursor.offset, cursor.last_scanned_id))
}

#[cfg(test)]
fn encode_cursor(created_at: &str, id: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{created_at}\n{id}"))
}

#[cfg(test)]
fn decode_cursor(cursor: &str) -> Result<(String, String), ()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ())?;
    let decoded = String::from_utf8(bytes).map_err(|_| ())?;
    let (created_at, id) = decoded.split_once('\n').ok_or(())?;
    if created_at.is_empty() || id.is_empty() {
        return Err(());
    }
    Ok((created_at.to_owned(), id.to_owned()))
}

fn canonical_json(value: &Value) -> Vec<u8> {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let ordered: BTreeMap<&String, Value> = map
                    .iter()
                    .map(|(key, value)| (key, canonical(value)))
                    .collect();
                let mut result = Map::new();
                for (key, value) in ordered {
                    result.insert(key.clone(), value);
                }
                Value::Object(result)
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_vec(&canonical(value)).unwrap_or_default()
}

fn sha256(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn domain_error(code: &str, message: &str, retryable: bool, resource_id: Option<String>) -> Value {
    let mut error = json!({"code":code,"message":message,"retryable":retryable});
    if let Some(resource_id) = resource_id {
        error["resource_id"] = json!(resource_id);
    }
    error
}

fn domain_result(request: &ParsedRequest, error: Value) -> HandlerReply {
    tool_result(request, error, true)
}

fn tool_result(request: &ParsedRequest, value: Value, is_error: bool) -> HandlerReply {
    let text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned());
    let mut result = json!({"resultType":"complete","content":[{"type":"text","text":text}]});
    if is_error {
        result["isError"] = json!(true);
    } else {
        result["structuredContent"] = value;
    }
    rpc_result(request, result)
}

fn rpc_result(request: &ParsedRequest, result: Value) -> HandlerReply {
    HandlerReply::json(
        200,
        json!({"jsonrpc":"2.0","id":request.request_id,"result":result}),
    )
}

fn protocol_error(request: &ParsedRequest, code: i64, message: &str) -> HandlerReply {
    HandlerReply::json(
        200,
        json!({"jsonrpc":"2.0","id":request.request_id,"error":{"code":code,"message":message}}),
    )
}

fn http_error(status: u16, request: &ParsedRequest, code: i64, message: &str) -> HandlerReply {
    HandlerReply::json(
        status,
        json!({"jsonrpc":"2.0","id":request.request_id,"error":{"code":code,"message":message}}),
    )
}

#[tauri::command]
pub(crate) fn product_mcp_status(state: State<'_, crate::AppState>) -> ServerStatus {
    let runtime = state
        .product_mcp
        .lock()
        .expect("Product MCP runtime mutex poisoned");
    status(&runtime)
}

#[tauri::command]
pub(crate) fn product_mcp_start(
    port: Option<u16>,
    state: State<'_, crate::AppState>,
) -> Result<ServerStatus, String> {
    let mut runtime = state
        .product_mcp
        .lock()
        .expect("Product MCP runtime mutex poisoned");
    start(
        &mut runtime,
        Arc::clone(&state.genesis),
        state.jobs.clone(),
        state.product_mcp_workspace_id.clone(),
        Arc::clone(&state.product_mcp_operations),
        port.unwrap_or(DEFAULT_PORT),
    )
}

#[tauri::command]
pub(crate) fn product_mcp_stop(state: State<'_, crate::AppState>) -> ServerStatus {
    let mut runtime = state
        .product_mcp
        .lock()
        .expect("Product MCP runtime mutex poisoned");
    stop(&mut runtime)
}

#[tauri::command]
pub(crate) fn product_mcp_grants_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<GrantSummary>, String> {
    let rows = genesis_adapter::query_all(
        &state.genesis,
        "product_mcp_grants",
        &[
            "id",
            "workspace_id",
            "client_name",
            "client_version",
            "scopes_json",
            "project_scope_json",
            "all_projects",
            "credential_account",
            "active",
            "expires_at",
            "created_at",
            "revoked_at",
        ],
        vec![],
    )?;
    let mut grants = rows
        .iter()
        .map(grant_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    grants.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(grants.into_iter().map(|grant| grant.summary()).collect())
}

#[tauri::command]
pub(crate) fn product_mcp_grant_create(
    input: GrantInput,
    state: State<'_, crate::AppState>,
) -> Result<CreatedGrant, String> {
    let client_name = input.client_name.trim().to_owned();
    if client_name.is_empty() || client_name.len() > 128 {
        return Err("client name must be 1-128 characters".to_owned());
    }
    if input
        .scopes
        .iter()
        .any(|scope| !SUPPORTED_SCOPES.contains(&scope.as_str()))
    {
        return Err("requested scope is not implemented in this phase".to_owned());
    }
    if !input.all_projects
        && input
            .project_scope
            .iter()
            .any(|project| project.trim().is_empty() || project.len() > 256)
    {
        return Err("project IDs must be non-empty".to_owned());
    }
    let mut projects = input.project_scope.clone();
    projects.sort();
    projects.dedup();
    if input.all_projects {
        projects.clear();
    }
    let id = Uuid::new_v4().to_string();
    let credential_account = format!("{KEYRING_PREFIX}{id}");
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut token = String::from("fung-mcp-");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(token, "{byte:02x}");
    }
    bytes.fill(0);
    let token_hash = sha256(token.as_bytes());
    keyring::Entry::new(KEYRING_SERVICE, &credential_account)
        .and_then(|entry| entry.set_password(&token))
        .map_err(|error| {
            format!("OS Credential Manager could not save the grant token: {error}")
        })?;
    let created_at = crate::now();
    let grant = Grant {
        id: id.clone(),
        client_name: client_name.clone(),
        client_version: None,
        workspace_id: state.product_mcp_workspace_id.clone(),
        scopes: input.scopes,
        project_scope: projects,
        all_projects: input.all_projects,
        credential_account: credential_account.clone(),
        active: true,
        expires_at: None,
        created_at: created_at.clone(),
    };
    let result = genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "product_mcp_grants",
            json!({
                "id":id,"workspace_id":grant.workspace_id,"client_name":grant.client_name,"client_version":null,
                "scopes_json":grant.scopes,"project_scope_json":grant.project_scope,"all_projects":grant.all_projects,
                "token_sha256":token_hash,"credential_account":credential_account,"active":true,"expires_at":null,
                "created_at":created_at,"revoked_at":null
            }),
        )],
    );
    if let Err(error) = result {
        let _ = keyring::Entry::new(KEYRING_SERVICE, &credential_account)
            .and_then(|entry| entry.delete_credential());
        token.zeroize();
        return Err(error);
    }
    Ok(CreatedGrant {
        grant: grant.summary(),
        token,
    })
}

#[tauri::command]
pub(crate) fn product_mcp_grant_revoke(
    grant_id: String,
    state: State<'_, crate::AppState>,
) -> Result<(), String> {
    let grant = grant_by_id(&state.genesis, &grant_id)?;
    if !grant.active {
        return Ok(());
    }
    let mut revoked = grant.clone();
    revoked.active = false;
    let revoked_at = crate::now();
    genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "product_mcp_grants",
            json!({
                "id":revoked.id,"workspace_id":revoked.workspace_id,"client_name":revoked.client_name,
                "client_version":revoked.client_version,"scopes_json":revoked.scopes,"project_scope_json":revoked.project_scope,
                "all_projects":revoked.all_projects,"token_sha256":load_token_hash(&state.genesis,&grant_id)?,
                "credential_account":revoked.credential_account,"active":false,"expires_at":revoked.expires_at,
                "created_at":revoked.created_at,"revoked_at":revoked_at
            }),
        )],
    )?;
    match keyring::Entry::new(KEYRING_SERVICE, &grant.credential_account)
        .and_then(|entry| entry.delete_credential())
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Grant revoked, but OS Credential Manager cleanup failed: {error}"
        )),
    }
}

fn load_token_hash(storage: &genesis_block_native::Storage, id: &str) -> Result<String, String> {
    let rows = genesis_adapter::query(
        storage,
        "product_mcp_grants",
        &["token_sha256"],
        vec![genesis_adapter::eq("product_mcp_grants", "id", json!(id))],
        1,
    )?;
    rows.first()
        .and_then(|row| row.get("product_mcp_grants.token_sha256"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "grant token digest unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};
    use serde_json::Value;
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };

    fn open_storage() -> (tempfile::TempDir, Arc<Storage>) {
        let directory = tempfile::tempdir().expect("temporary Genesis directory");
        let storage = Storage::open(OpenOptions {
            path: directory.path().display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .expect("open Genesis storage");
        genesis_adapter::install(&storage).expect("install current FUNG schema");
        (directory, Arc::new(storage))
    }

    fn insert_grant(
        storage: &Storage,
        id: &str,
        token: &str,
        workspace: &str,
        scopes: &[&str],
        projects: &[&str],
    ) {
        genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "product_mcp_grants",
                json!({
                    "id": id,
                    "workspace_id": workspace,
                    "client_name": "test-client",
                    "client_version": null,
                    "scopes_json": scopes,
                    "project_scope_json": projects,
                    "all_projects": false,
                    "token_sha256": sha256(token.as_bytes()),
                    "credential_account": format!("{KEYRING_PREFIX}{id}"),
                    "active": true,
                    "expires_at": null,
                    "created_at": "2026-10-20T00:00:00Z",
                    "revoked_at": null
                }),
            )],
        )
        .expect("persist local grant fixture");
    }

    fn request(method: &str, id: u64, params: Value, tool_name: Option<&str>) -> ParsedRequest {
        ParsedRequest {
            request_id: Some(json!(id)),
            notification: false,
            method: method.to_owned(),
            params,
            tool_name: tool_name.map(str::to_owned),
            authorization: None,
        }
    }

    fn post(addr: std::net::SocketAddr, token: &str, method: &str, params: Value) -> Value {
        let id = 1;
        let name = params.get("name").and_then(Value::as_str);
        let body = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        let body = serde_json::to_vec(&body).unwrap();
        let mut stream = TcpStream::connect(addr).expect("connect to loopback MCP listener");
        let mut headers = format!(
            "POST /mcp HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nAuthorization: Bearer {token}\r\nMCP-Protocol-Version: {PROTOCOL_VERSION}\r\nMcp-Method: {method}\r\n"
        );
        if let Some(name) = name {
            headers.push_str(&format!("Mcp-Name: {name}\r\n"));
        }
        headers.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ));
        stream.write_all(headers.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (_, response_body) = response.split_once("\r\n\r\n").expect("HTTP body boundary");
        serde_json::from_str(response_body).expect("valid MCP JSON-RPC response")
    }

    #[test]
    fn integrated_loopback_bridge_authenticates_and_lists_grant_scoped_tools() {
        let (_directory, storage) = open_storage();
        let workspace = "workspace-test";
        insert_grant(
            &storage,
            "grant-test",
            "test-token",
            workspace,
            &["workspace.read"],
            &["project-1"],
        );
        let jobs = JobEngine::new(Arc::clone(&storage));
        let operations = Arc::new(Mutex::new(()));
        let auth_storage = Arc::clone(&storage);
        let auth_workspace = workspace.to_owned();
        let handler_storage = Arc::clone(&storage);
        let handler_workspace = workspace.to_owned();
        let handler_jobs = jobs.clone();
        let handler_operations = Arc::clone(&operations);
        let server = ProductMcpServer::start(
            ProductMcpConfig {
                port: 0,
                ..ProductMcpConfig::default()
            },
            move |request| authenticate(&auth_storage, &auth_workspace, request),
            move |request, identity| {
                dispatch(
                    &handler_storage,
                    &handler_jobs,
                    &handler_workspace,
                    &handler_operations,
                    request,
                    identity,
                )
            },
        )
        .expect("start integrated loopback MCP listener");
        let addr = server.local_addr();

        let discovery = post(
            addr,
            "test-token",
            "server/discover",
            json!({"_meta":{"io.modelcontextprotocol/protocolVersion":PROTOCOL_VERSION}}),
        );
        let discovery = &discovery["result"];
        assert_eq!(discovery["resultType"], "complete");
        assert_eq!(discovery["supportedVersions"], json!([PROTOCOL_VERSION]));
        assert_eq!(
            discovery["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "FUNG Desktop"
        );
        assert_eq!(discovery["ttlMs"], 300000);
        assert_eq!(discovery["cacheScope"], "public");

        let catalog = post(
            addr,
            "test-token",
            "tools/list",
            json!({"_meta":{"io.modelcontextprotocol/protocolVersion":PROTOCOL_VERSION}}),
        );
        let catalog = &catalog["result"];
        assert_eq!(catalog["cacheScope"], "private");
        assert_eq!(catalog["ttlMs"], 0);
        let names = catalog["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"fung.job.list"));
        assert!(!names.contains(&"fung.job.control"));

        let health = post(
            addr,
            "test-token",
            "tools/call",
            json!({
                "name":"fung.health", "arguments":{},
                "_meta":{"io.modelcontextprotocol/protocolVersion":PROTOCOL_VERSION}
            }),
        );
        assert_eq!(health["result"]["structuredContent"]["status"], "ready");
        let rejected = post(
            addr,
            "wrong-token",
            "server/discover",
            json!({"_meta":{"io.modelcontextprotocol/protocolVersion":PROTOCOL_VERSION}}),
        );
        assert_eq!(rejected["error"]["code"], -32001);
        drop(server);
    }

    #[test]
    fn unknown_rpc_method_returns_not_found_status() {
        let (_directory, storage) = open_storage();
        let workspace = "workspace-test";
        insert_grant(
            &storage,
            "grant-test",
            "test-token",
            workspace,
            &["workspace.read"],
            &["project-1"],
        );
        let jobs = JobEngine::new(Arc::clone(&storage));
        let operations = Mutex::new(());
        let reply = dispatch(
            &storage,
            &jobs,
            workspace,
            &operations,
            &request("unsupported/method", 3, json!({}), None),
            &GrantIdentity::new("grant-test").unwrap(),
        );
        assert_eq!(reply.status, 404);
        assert_eq!(reply.body.unwrap()["error"]["code"], -32601);
    }

    #[test]
    fn dispatch_accepts_supported_json_rpc_notifications() {
        let (_directory, storage) = open_storage();
        insert_grant(
            &storage,
            "grant-notification",
            "notification-token",
            "workspace-test",
            &[],
            &[],
        );
        let jobs = JobEngine::new(Arc::clone(&storage));
        let request = ParsedRequest {
            request_id: None,
            notification: true,
            method: "ping".to_owned(),
            params: json!({}),
            tool_name: None,
            authorization: None,
        };

        let reply = dispatch(
            &storage,
            &jobs,
            "workspace-test",
            &Mutex::new(()),
            &request,
            &GrantIdentity::new("grant-notification").unwrap(),
        );

        assert_eq!(reply.status, 200);
        assert_eq!(reply.body.unwrap()["result"], json!({}));
    }

    #[test]
    fn application_shutdown_stops_product_mcp_listener() {
        let (_directory, storage) = open_storage();
        let port = {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!((49_152..=65_535).contains(&port));
        let runtime = Mutex::new(Runtime::default());
        start(
            &mut runtime.lock().unwrap(),
            Arc::clone(&storage),
            JobEngine::new(storage),
            "workspace-test".to_owned(),
            Arc::new(Mutex::new(())),
            port,
        )
        .unwrap();

        shutdown(&runtime);

        assert!(!status(&runtime.lock().unwrap()).running);
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    fn job_control_is_project_scoped_and_replays_the_durable_retry_result() {
        let (_directory, storage) = open_storage();
        insert_grant(
            &storage,
            "grant-test",
            "test-token",
            "workspace-test",
            &["workspace.read", "job.control"],
            &["project-1"],
        );
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "projects",
                json!({
                    "id":"project-1","name":"Project 1","storage_path":"relative/project-1",
                    "created_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z"
                }),
            )],
        )
        .unwrap();
        let jobs = JobEngine::new(Arc::clone(&storage));
        let original = jobs
            .enqueue(JobKind::ExportRender, "project-1", Some("recording-1"))
            .unwrap();
        crate::set_job_status(&storage, &original, "failed", None, Some("fixture failure"))
            .unwrap();
        let grant = grant_by_id(&storage, "grant-test").unwrap();
        let identity = GrantIdentity::new("grant-test").unwrap();
        let operations = Mutex::new(());
        let params = json!({"name":"fung.job.control","arguments":{"job_id":original,"action":"retry","idempotency_key":"retry-key-1"}});
        let first = dispatch(
            &storage,
            &jobs,
            "workspace-test",
            &operations,
            &request("tools/call", 1, params.clone(), Some("fung.job.control")),
            &identity,
        );
        let first = first.body.unwrap()["result"]["structuredContent"].clone();
        let second = dispatch(
            &storage,
            &jobs,
            "workspace-test",
            &operations,
            &request("tools/call", 2, params, Some("fung.job.control")),
            &identity,
        );
        let second = second.body.unwrap()["result"]["structuredContent"].clone();
        assert_eq!(first, second);
        assert_eq!(first["accepted"], true);
        let retry_job_id = first["retry_job_id"].as_str().unwrap();
        let all_jobs =
            genesis_adapter::query_all(&storage, "jobs", &["id", "project_id"], vec![]).unwrap();
        assert_eq!(
            all_jobs.len(),
            2,
            "same idempotency key must not enqueue twice"
        );
        let audit = genesis_adapter::query_all(
            &storage,
            "product_mcp_audit",
            &["id", "tool_name", "outcome", "object_ids_json"],
            vec![],
        )
        .unwrap();
        assert_eq!(
            audit.len(),
            2,
            "first execution and replay each leave an audit event"
        );
        assert_eq!(grant.id, "grant-test");
        let replay_audit = audit
            .iter()
            .find(|row| row["product_mcp_audit.outcome"] == "idempotency.replay")
            .expect("replay leaves an audit event");
        assert_eq!(
            replay_audit["product_mcp_audit.object_ids_json"],
            json!([original, retry_job_id])
        );
    }

    #[test]
    fn pending_retry_intents_reconcile_before_or_after_job_enqueue() {
        let (_directory, storage) = open_storage();
        insert_grant(
            &storage,
            "grant-test",
            "test-token",
            "workspace-test",
            &["workspace.read", "job.control"],
            &["project-1"],
        );
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "projects",
                json!({
                    "id":"project-1","name":"Project 1","storage_path":"relative/project-1",
                    "created_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z"
                }),
            )],
        )
        .unwrap();
        let jobs = JobEngine::new(Arc::clone(&storage));
        let original = jobs
            .enqueue(JobKind::ExportRender, "project-1", Some("recording-1"))
            .unwrap();
        crate::set_job_status(&storage, &original, "failed", None, Some("fixture failure"))
            .unwrap();
        let operations = Mutex::new(());
        let identity = GrantIdentity::new("grant-test").unwrap();

        for (index, already_enqueued) in [(0_u64, false), (1_u64, true)] {
            let key = format!("recover-key-{index}");
            let retry_id = Uuid::new_v4().to_string();
            let arguments = json!({
                "job_id": original,
                "action": "retry",
                "idempotency_key": key
            });
            let key_hash = sha256(key.as_bytes());
            let request_hash = sha256(&canonical_json(&arguments));
            genesis_adapter::commit_rows(
                &storage,
                vec![genesis_adapter::upsert(
                    "product_mcp_idempotency",
                    json!({
                        "id": Uuid::new_v4().to_string(),
                        "grant_id":"grant-test",
                        "tool_name":"fung.job.control",
                        "idempotency_key_sha256":key_hash,
                        "request_sha256":request_hash,
                        "operation_state":"pending",
                        "job_id":original,
                        "intent_json":{
                            "action":"retry",
                            "source_job_id":original,
                            "retry_job_id":retry_id,
                            "kind":"export.render",
                            "project_id":"project-1",
                            "recording_id":"recording-1",
                            "transcription_mode":"general"
                        },
                        "result_json":null,
                        "created_at":"2026-10-20T00:00:00Z",
                        "updated_at":"2026-10-20T00:00:00Z",
                        "retain_until":null
                    }),
                )],
            )
            .unwrap();
            if already_enqueued {
                assert_eq!(
                    jobs.enqueue_with_transcription_mode_for_id(
                        &retry_id,
                        JobKind::ExportRender,
                        "project-1",
                        Some("recording-1"),
                        TranscriptionMode::General,
                    )
                    .unwrap(),
                    retry_id
                );
            }

            let reply = dispatch(
                &storage,
                &jobs,
                "workspace-test",
                &operations,
                &request(
                    "tools/call",
                    index + 10,
                    json!({"name":"fung.job.control","arguments":arguments}),
                    Some("fung.job.control"),
                ),
                &identity,
            );
            let result = reply.body.unwrap()["result"]["structuredContent"].clone();
            assert_eq!(result["retry_job_id"], retry_id);
            assert_eq!(result["state"], "queued");
            assert_eq!(result["accepted"], true);
        }

        let rows = genesis_adapter::query_all(&storage, "jobs", &["id"], vec![]).unwrap();
        assert_eq!(
            rows.len(),
            3,
            "each durable retry intent resolves to one job"
        );
        let audits = genesis_adapter::query_all(
            &storage,
            "product_mcp_audit",
            &["object_ids_json", "outcome"],
            vec![],
        )
        .unwrap();
        assert_eq!(audits.len(), 2);
        for audit in audits {
            let object_ids = audit["product_mcp_audit.object_ids_json"]
                .as_array()
                .unwrap();
            assert_eq!(
                object_ids.len(),
                2,
                "retry audit records source and result jobs"
            );
        }
    }

    #[test]
    fn schemas_reject_unknown_fields_and_grants_cannot_cross_projects() {
        assert!(validate_arguments("fung.health", &json!({"secret":"bad"})).is_err());
        assert!(validate_arguments("fung.job.list", &json!({"page_size":101})).is_err());
        assert!(validate_arguments(
            "fung.job.control",
            &json!({"job_id":"j","action":"delete","idempotency_key":"k"})
        )
        .is_err());
        let grant = Grant {
            id: "g".into(),
            client_name: "test".into(),
            client_version: None,
            workspace_id: "w".into(),
            scopes: vec!["workspace.read".into()],
            project_scope: vec!["project-a".into()],
            all_projects: false,
            credential_account: "account".into(),
            active: true,
            expires_at: None,
            created_at: "2026-10-20T00:00:00Z".into(),
        };
        assert!(grant.permits_project("project-a"));
        assert!(!grant.permits_project("project-b"));
        let cursor = encode_cursor("2026-10-20T00:00:00Z", "job-1");
        assert_eq!(
            decode_cursor(&cursor).unwrap(),
            ("2026-10-20T00:00:00Z".into(), "job-1".into())
        );
    }

    #[test]
    fn job_queries_hide_projects_outside_the_exact_grant_binding() {
        let (_directory, storage) = open_storage();
        insert_grant(
            &storage,
            "grant-project-bound",
            "test-token",
            "workspace-test",
            &["workspace.read"],
            &["project-1"],
        );
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "projects",
                json!({
                    "id":"project-2","name":"Hidden project","storage_path":"relative/project-2",
                    "created_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z"
                }),
            )],
        )
        .unwrap();
        let jobs = JobEngine::new(Arc::clone(&storage));
        let hidden_job = jobs
            .enqueue(JobKind::ExportRender, "project-2", Some("recording-secret"))
            .unwrap();
        let grant = grant_by_id(&storage, "grant-project-bound").unwrap();
        let list = list_jobs_for_grant(&storage, &grant, &json!({})).unwrap();
        assert_eq!(list["items"], json!([]));
        let hidden =
            get_job_for_grant(&storage, &grant, &json!({"job_id":hidden_job})).unwrap_err();
        assert_eq!(hidden["code"], "resource.not_found");

        let no_control = Grant {
            scopes: vec!["workspace.read".to_owned()],
            ..grant
        };
        assert!(!is_tool_visible(&no_control, "fung.job.control"));
    }

    #[test]
    fn job_list_bounds_each_scan_and_continues_with_grant_scoped_cursor() {
        let (_directory, storage) = open_storage();
        insert_grant(
            &storage,
            "grant-bounded-list",
            "bounded-list-token",
            "workspace-test",
            &["workspace.read"],
            &["visible-project"],
        );
        genesis_adapter::commit_rows(
            &storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    json!({
                        "id":"hidden-project","name":"Hidden","storage_path":"hidden",
                        "created_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z"
                    }),
                ),
                genesis_adapter::upsert(
                    "projects",
                    json!({
                        "id":"visible-project","name":"Visible","storage_path":"visible",
                        "created_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z"
                    }),
                ),
            ],
        )
        .unwrap();

        let job_row = |id: String, project_id: &str| {
            genesis_adapter::upsert(
                "jobs",
                json!({
                    "id":id,"project_id":project_id,"type":"export.render",
                    "status":"completed","progress":100,"input_refs_json":[],
                    "output_refs_json":[],"provider_id":null,"error_code":null,
                    "error_message":null,"attempt_no":1,"started_at":null,
                    "finished_at":null,"created_at":"2026-10-20T00:00:00Z",
                    "updated_at":"2026-10-20T00:00:00Z"
                }),
            )
        };
        let hidden_jobs = (0..MAX_JOB_SCAN_ROWS_PER_REQUEST)
            .map(|index| job_row(format!("a-hidden-{index:04}"), "hidden-project"))
            .collect();
        genesis_adapter::commit_rows(&storage, hidden_jobs).unwrap();
        genesis_adapter::commit_rows(
            &storage,
            vec![job_row(
                "z-visible-after-bound".to_owned(),
                "visible-project",
            )],
        )
        .unwrap();

        let grant = grant_by_id(&storage, "grant-bounded-list").unwrap();
        let first_page = list_jobs_for_grant(&storage, &grant, &json!({"page_size":10})).unwrap();
        assert_eq!(first_page["items"], json!([]));
        let cursor = first_page["next_cursor"]
            .as_str()
            .expect("scan budget returns a continuation cursor")
            .to_owned();
        assert!(decode_job_cursor(&cursor, "another-grant", None).is_err());

        let second_page =
            list_jobs_for_grant(&storage, &grant, &json!({"page_size":10,"cursor":cursor}))
                .unwrap();
        assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
        assert_eq!(second_page["items"][0]["job_id"], "z-visible-after-bound");
        assert!(second_page["next_cursor"].is_null());
    }
}
