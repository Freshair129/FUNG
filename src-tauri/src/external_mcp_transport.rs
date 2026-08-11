//! Bounded stdio transport for approved read-only external MCP tools.
//! @req FR-109, FR-114, NFR-105
//! @tested src-tauri/src/external_mcp_transport.rs#tests

use crate::external_mcp::{ConnectorCapability, ExternalMcpErrorCode};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MAX_MCP_MESSAGE_BYTES: usize = 256 * 1024;
const MAX_MCP_STDERR_BYTES: u64 = 64 * 1024;
const MAX_MCP_EXECUTION: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllowedStdioTool {
    pub(crate) name: String,
    pub(crate) capability: ConnectorCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StdioConnectorConfig {
    pub(crate) connector_id: String,
    pub(crate) executable: PathBuf,
    pub(crate) arguments: Vec<String>,
    pub(crate) allowed_tools: Vec<AllowedStdioTool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExternalExecutionLimits {
    pub(crate) timeout: Duration,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExternalCancellation(Arc<AtomicBool>);

impl ExternalCancellation {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpCallOutput {
    pub(crate) payload: Value,
    pub(crate) source_refs: Vec<String>,
}

struct StdioProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: mpsc::Receiver<Result<String, ExternalMcpErrorCode>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
}

impl StdioProcess {
    fn send(&mut self, message: &Value) -> Result<(), ExternalMcpErrorCode> {
        let encoded =
            serde_json::to_vec(message).map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
        if encoded.len() > MAX_MCP_MESSAGE_BYTES || encoded.contains(&b'\n') {
            return Err(ExternalMcpErrorCode::ResultTooLarge);
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
        stdin
            .write_all(&encoded)
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)
    }

    fn response(
        &self,
        expected_id: i64,
        deadline: Instant,
        cancellation: &ExternalCancellation,
    ) -> Result<Value, ExternalMcpErrorCode> {
        loop {
            if cancellation.is_cancelled() {
                return Err(ExternalMcpErrorCode::ToolCancelled);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(ExternalMcpErrorCode::ToolTimeout);
            }
            let wait = deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(10));
            let line = match self.responses.recv_timeout(wait) {
                Ok(line) => line?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ExternalMcpErrorCode::ConnectorUnhealthy)
                }
            };
            let message: Value = serde_json::from_str(line.trim_end())
                .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
            if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
            }
            if message.get("method").is_some() {
                if message.get("id").is_some() {
                    return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
                }
                continue;
            }
            if message.get("id").and_then(Value::as_i64) != Some(expected_id)
                || message.get("error").is_some()
            {
                return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
            }
            return message
                .get("result")
                .cloned()
                .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy);
        }
    }

    fn cleanup(mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for StdioProcess {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

pub(crate) fn validate_stdio_config(
    config: &StdioConnectorConfig,
) -> Result<(), ExternalMcpErrorCode> {
    if !is_safe_identifier(&config.connector_id)
        || !config.executable.is_absolute()
        || !config.executable.is_file()
        || config.arguments.len() > 32
        || config.arguments.iter().any(|argument| {
            argument.len() > 1_024 || argument.chars().any(|character| character == '\0')
        })
        || config.allowed_tools.is_empty()
        || config.allowed_tools.len() > 32
        || config
            .allowed_tools
            .iter()
            .any(|tool| !is_safe_identifier(&tool.name))
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }

    let mut names = config
        .allowed_tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    Ok(())
}

pub(crate) fn validate_stdio_tool(
    config: &StdioConnectorConfig,
    tool_name: &str,
    capability: ConnectorCapability,
) -> Result<(), ExternalMcpErrorCode> {
    validate_stdio_config(config)?;
    config
        .allowed_tools
        .iter()
        .any(|tool| tool.name == tool_name && tool.capability == capability)
        .then_some(())
        .ok_or(ExternalMcpErrorCode::CapabilityDenied)
}

fn spawn_stdio(config: &StdioConnectorConfig) -> Result<StdioProcess, ExternalMcpErrorCode> {
    validate_stdio_config(config)?;
    let mut command = Command::new(&config.executable);
    command
        .args(&config.arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let stdin = child
        .stdin
        .take()
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let (sender, responses) = mpsc::channel();
    let stdout_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let read = match reader
                .by_ref()
                .take((MAX_MCP_MESSAGE_BYTES + 1) as u64)
                .read_line(&mut line)
            {
                Ok(read) => read,
                Err(_) => {
                    let _ = sender.send(Err(ExternalMcpErrorCode::ConnectorUnhealthy));
                    return;
                }
            };
            if read == 0 {
                return;
            }
            if read > MAX_MCP_MESSAGE_BYTES || !line.ends_with('\n') {
                let _ = sender.send(Err(ExternalMcpErrorCode::ResultTooLarge));
                return;
            }
            if sender.send(Ok(line)).is_err() {
                return;
            }
        }
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut bounded = Vec::new();
        let _ = stderr.take(MAX_MCP_STDERR_BYTES).read_to_end(&mut bounded);
        drop(bounded);
    });
    Ok(StdioProcess {
        child,
        stdin: Some(stdin),
        responses,
        stdout_thread: Some(stdout_thread),
        stderr_thread: Some(stderr_thread),
    })
}

fn execute_stdio_tool_inner(
    config: &StdioConnectorConfig,
    tool_name: &str,
    arguments: &Value,
    limits: ExternalExecutionLimits,
    cancellation: &ExternalCancellation,
) -> Result<McpCallOutput, ExternalMcpErrorCode> {
    validate_stdio_tool(
        config,
        tool_name,
        config
            .allowed_tools
            .iter()
            .find(|tool| tool.name == tool_name)
            .map(|tool| tool.capability)
            .ok_or(ExternalMcpErrorCode::CapabilityDenied)?,
    )?;
    if limits.timeout.is_zero() || !arguments.is_object() {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let deadline = Instant::now() + limits.timeout.min(MAX_MCP_EXECUTION);
    let mut process = spawn_stdio(config)?;

    process.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name":"FUNG","version":env!("CARGO_PKG_VERSION")}
        }
    }))?;
    let initialized = process.response(1, deadline, cancellation)?;
    if initialized.get("protocolVersion").and_then(Value::as_str) != Some(MCP_PROTOCOL_VERSION)
        || initialized
            .get("capabilities")
            .and_then(|value| value.get("tools"))
            .is_none()
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    process.send(&serde_json::json!({
        "jsonrpc":"2.0",
        "method":"notifications/initialized"
    }))?;
    process.send(&serde_json::json!({
        "jsonrpc":"2.0",
        "id":2,
        "method":"tools/list",
        "params":{}
    }))?;
    let listed = process.response(2, deadline, cancellation)?;
    let tools = listed
        .get("tools")
        .and_then(Value::as_array)
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let advertised = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(tool_name))
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    if advertised
        .pointer("/execution/taskSupport")
        .and_then(Value::as_str)
        == Some("required")
        || advertised
            .get("inputSchema")
            .and_then(Value::as_object)
            .is_none()
    {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }

    process.send(&serde_json::json!({
        "jsonrpc":"2.0",
        "id":3,
        "method":"tools/call",
        "params":{"name":tool_name,"arguments":arguments}
    }))?;
    let called = process.response(3, deadline, cancellation)?;
    if called.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(ExternalMcpErrorCode::ConnectorUnhealthy);
    }
    let payload = called
        .get("structuredContent")
        .or_else(|| called.get("content"))
        .cloned()
        .ok_or(ExternalMcpErrorCode::ConnectorUnhealthy)?;
    let source_refs = called
        .get("sourceRefs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let value = item.as_str().ok_or(ExternalMcpErrorCode::ResultUnsafe)?;
                    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control)
                    {
                        return Err(ExternalMcpErrorCode::ResultUnsafe);
                    }
                    Ok(value.to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if source_refs.len() > 64 {
        return Err(ExternalMcpErrorCode::ResultUnsafe);
    }

    process.cleanup();
    Ok(McpCallOutput {
        payload,
        source_refs,
    })
}

pub(crate) fn execute_stdio_tool(
    config: &StdioConnectorConfig,
    tool_name: &str,
    capability: ConnectorCapability,
    arguments: &Value,
    limits: ExternalExecutionLimits,
    cancellation: &ExternalCancellation,
) -> Result<McpCallOutput, ExternalMcpErrorCode> {
    validate_stdio_tool(config, tool_name, capability)?;
    execute_stdio_tool_inner(config, tool_name, arguments, limits, cancellation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    fn fixture_executable() -> PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .join("test-fixtures");
                std::fs::create_dir_all(&output_dir).expect("create fixture output directory");
                let executable = output_dir.join(if cfg!(windows) {
                    "fake-external-mcp.exe"
                } else {
                    "fake-external-mcp"
                });
                let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join("fixtures")
                    .join("fake_external_mcp.rs");
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

    fn valid_config() -> StdioConnectorConfig {
        StdioConnectorConfig {
            connector_id: "connector-1".into(),
            executable: std::env::current_exe().expect("absolute test executable"),
            arguments: vec!["--fixture".into()],
            allowed_tools: vec![AllowedStdioTool {
                name: "search_documents".into(),
                capability: ConnectorCapability::DocumentsSearch,
            }],
        }
    }

    fn fixture_config(mode: &str) -> StdioConnectorConfig {
        StdioConnectorConfig {
            connector_id: "connector-fixture".into(),
            executable: fixture_executable(),
            arguments: vec![mode.into()],
            allowed_tools: vec![AllowedStdioTool {
                name: "search_documents".into(),
                capability: ConnectorCapability::DocumentsSearch,
            }],
        }
    }

    #[test]
    fn stdio_config_requires_absolute_executable_and_exact_semantic_tool_allowlist() {
        let config = valid_config();
        assert_eq!(validate_stdio_config(&config), Ok(()));
        assert_eq!(
            validate_stdio_tool(
                &config,
                "search_documents",
                ConnectorCapability::DocumentsSearch,
            ),
            Ok(())
        );

        let mut relative = valid_config();
        relative.executable = "fixture-mcp".into();
        assert_eq!(
            validate_stdio_config(&relative),
            Err(ExternalMcpErrorCode::ConnectorUnhealthy)
        );
        assert_eq!(
            validate_stdio_tool(
                &config,
                "delete_documents",
                ConnectorCapability::DocumentsSearch,
            ),
            Err(ExternalMcpErrorCode::CapabilityDenied)
        );
        assert_eq!(
            validate_stdio_tool(
                &config,
                "search_documents",
                ConnectorCapability::CrmCustomerStatusRead,
            ),
            Err(ExternalMcpErrorCode::CapabilityDenied)
        );
    }

    #[test]
    fn stdio_adapter_initializes_lists_and_calls_only_the_exact_allowed_tool() {
        let output = execute_stdio_tool(
            &fixture_config("write-advertised"),
            "search_documents",
            ConnectorCapability::DocumentsSearch,
            &serde_json::json!({"query":"contract"}),
            ExternalExecutionLimits {
                timeout: Duration::from_secs(2),
            },
            &ExternalCancellation::new(),
        )
        .expect("approved fixture call");

        assert_eq!(
            output.payload,
            serde_json::json!({
                "items": [{
                    "title": "Approved contract",
                    "location": "kb://documents/42"
                }]
            })
        );
        assert_eq!(output.source_refs, vec!["kb://documents/42"]);
    }

    #[test]
    fn stdio_adapter_timeout_and_cancel_are_bounded_and_cleanup_the_child() {
        let slow_config = fixture_config("slow");
        let started = Instant::now();
        assert_eq!(
            execute_stdio_tool(
                &slow_config,
                "search_documents",
                ConnectorCapability::DocumentsSearch,
                &serde_json::json!({"query":"contract"}),
                ExternalExecutionLimits {
                    timeout: Duration::from_millis(80),
                },
                &ExternalCancellation::new(),
            ),
            Err(ExternalMcpErrorCode::ToolTimeout)
        );
        assert!(started.elapsed() < Duration::from_secs(2));

        let cancellation = ExternalCancellation::new();
        let cancel_from_thread = cancellation.clone();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            cancel_from_thread.cancel();
        });
        assert_eq!(
            execute_stdio_tool(
                &slow_config,
                "search_documents",
                ConnectorCapability::DocumentsSearch,
                &serde_json::json!({"query":"contract"}),
                ExternalExecutionLimits {
                    timeout: Duration::from_secs(2),
                },
                &cancellation,
            ),
            Err(ExternalMcpErrorCode::ToolCancelled)
        );
        canceller.join().expect("canceller joins");
    }
}
