use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    env,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};
use tauri::{Manager, State};
#[cfg(desktop)]
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;
use uuid::Uuid;

mod audio_custody;
mod audio_export;
mod auth_session;
mod backup;
mod backup_archive;
mod backup_payload;
mod cloud_commands;
mod cloud_config;
mod cloud_executor;
mod desktop_playback;
mod device_identity;
mod diarization;
mod external_mcp;
mod external_mcp_commands;
mod external_mcp_transport;
mod filesystem_backup;
mod fungwire;
mod fungwire_client;
mod fungwire_server;
mod genesis_adapter;
mod graph_build;
mod job_engine;
mod live_meeting;
mod live_transcript;
mod local_api;
mod local_diarization;
mod media_fetch;
mod meeting_adapter;
mod meeting_agent;
mod meeting_delivery;
mod meeting_intel;
mod meeting_intelligence_runtime;
mod meeting_intelligence_schema;
mod meeting_knowledge;
mod mobile;
#[rustfmt::skip]
mod native_auth;
mod native_recorder;
mod on_device_ai;
mod policy;
mod recording_output;
mod recording_review;
mod recovery;
mod speaker_merge;
mod transcript_export;
mod tts_config;
mod tts_executor;
mod zoom_sync;

/// Source-tree fallback used by `tauri dev`. Packaged builds must resolve all
/// worker resources from the installed application's resource directory.
const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Default local Ollama endpoint. Seeded into the `model_providers` row
/// `ollama-summary-intent` on first run (both the raw-SQL bootstrap and the
/// genesis JSON upsert below) and used by `graph_build::llm_provider_config`
/// as the fallback when that row's `config_json` has no `endpoint` key.
/// Single source of truth so the seed and the fallback cannot drift apart
/// (issue #36).
pub(crate) const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
/// Default local vLLM endpoint, seeded into the `model_providers` row
/// `vllm-summary-intent` alongside the Ollama row (disabled by default).
pub(crate) const DEFAULT_VLLM_ENDPOINT: &str = "http://127.0.0.1:8000";
/// Default local model name used by `graph_build::llm_provider_config` when
/// a `model_providers` row's `config_json` has no `model` key.
pub(crate) const DEFAULT_OLLAMA_MODEL: &str = "llama3.1:8b";

const THAI_CANDIDATE_PROFILE: &str = "thai-large-candidate";
const THAI_CANDIDATE_MODEL: &str = "whisper-th-large-combined";
const THAI_CANDIDATE_RUNTIME: &str = ".venv-whisper-transformers-candidate";

#[derive(Clone)]
pub(crate) struct WhisperRuntime {
    pub(crate) python: PathBuf,
    pub(crate) script: PathBuf,
    pub(crate) cuda_bin: PathBuf,
}

impl WhisperRuntime {
    /// Test-only constructor pointing at explicit python/script paths,
    /// bypassing the packaged-vs-source-tree resolution `whisper_runtime`
    /// performs (there is no `tauri::App` to resolve a resource dir from in
    /// unit tests). Used by the FUNGWIRE job-loop tests to point the worker
    /// at a stub script instead of the real faster-whisper pipeline, while
    /// still exercising the real `run_python_worker` subprocess plumbing.
    ///
    /// `python` is accepted for source-compat with existing callers (who
    /// still build the bundled `.venv-whisper` path) but is intentionally
    /// NOT used: that path exists on dev machines but not on CI runners
    /// (which never install the FUNG app bundle), which made these tests
    /// dev-only (see CI failure `FUNG Python runtime is missing at
    /// D:\a\FUNG\FUNG\.venv-whisper\...`). Instead we resolve a real system
    /// python via `resolve_test_python`, which is present on both dev
    /// machines and CI runners, falling back to the bundled venv locally
    /// when no system python is on PATH. On Windows the Python Launcher
    /// (`py.exe`) is accepted as the system interpreter as well.
    #[cfg(test)]
    pub(crate) fn for_test(_python: PathBuf, script: PathBuf) -> Self {
        Self {
            python: resolve_test_python(),
            script,
            cuda_bin: PathBuf::new(),
        }
    }
}

/// Resolves an absolute path to a system Python interpreter for use by
/// `WhisperRuntime::for_test`. Dependency-free: shells out to `where`
/// (Windows) or `which` (unix) rather than pulling in a crate, since this is
/// test-only plumbing. Falls back to the bundled `.venv-whisper` interpreter
/// (present in local dev checkouts, absent on CI) if no system interpreter
/// resolves, so local test runs are unaffected either way. Windows also
/// probes the standard `py.exe` launcher when `python.exe` is not on PATH.
#[cfg(test)]
fn resolve_test_python() -> PathBuf {
    let (finder, names): (&str, &[&str]) = if cfg!(windows) {
        ("where", &["python", "python3", "py"])
    } else {
        ("which", &["python3", "python"])
    };

    for name in names {
        let Ok(output) = Command::new(finder).arg(name).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let candidate = PathBuf::from(line.trim());
            // Skip the Windows Store's python.exe alias stub: it exists on
            // disk (so `.exists()` passes) but launches the Store instead of
            // an interpreter when executed.
            let is_store_stub = candidate
                .components()
                .any(|c| c.as_os_str().eq_ignore_ascii_case("WindowsApps"));
            if !is_store_stub && candidate.exists() {
                return candidate;
            }
        }
    }

    // Fallback: bundled venv python (present in local dev checkouts).
    source_root()
        .join(".venv-whisper")
        .join("Scripts")
        .join("python.exe")
}

fn source_root() -> PathBuf {
    PathBuf::from(PROJECT_ROOT)
        .parent()
        .expect("src-tauri has a parent directory")
        .to_path_buf()
}

fn whisper_runtime(app: &tauri::App) -> WhisperRuntime {
    let source_root = source_root();
    let packaged_root = app.path().resource_dir().ok();
    let root = packaged_root
        .filter(|path| {
            path.join(".venv-whisper")
                .join("Scripts")
                .join("python.exe")
                .exists()
        })
        .unwrap_or(source_root);

    WhisperRuntime {
        python: root
            .join(".venv-whisper")
            .join("Scripts")
            .join("python.exe"),
        script: root.join("scripts").join("transcribe.py"),
        cuda_bin: root.join("runtime").join("cuda12").join("bin"),
    }
}

pub(crate) const REQUIRED_CUDA_DLLS: [&str; 4] = [
    "cudart64_12.dll",
    "cublas64_12.dll",
    "cublasLt64_12.dll",
    "cudnn64_9.dll",
];

pub(crate) fn transcription_profile() -> Result<String, String> {
    let configured = env::var("FUNG_TRANSCRIPTION_PROFILE").ok();
    transcription_profile_from(configured.as_deref())
}

fn transcription_profile_from(configured: Option<&str>) -> Result<String, String> {
    let profile = configured.unwrap_or("cpu").to_string();
    match profile.as_str() {
        "gpu" | "cpu" => Ok(profile),
        _ => Err(format!(
            "invalid FUNG_TRANSCRIPTION_PROFILE '{profile}'; use 'gpu' or 'cpu'"
        )),
    }
}

// Candidate-only profile selection remains exposed for the release contract
// while the production transcription path continues to use its existing API.
#[allow(dead_code)]
pub(crate) fn whisper_model_name() -> Result<&'static str, String> {
    let configured = env::var("FUNG_WHISPER_MODEL_PROFILE").ok();
    whisper_model_name_from(configured.as_deref())
}

fn whisper_model_name_from(configured: Option<&str>) -> Result<&'static str, String> {
    match configured.unwrap_or("turbo") {
        "turbo" => Ok("large-v3-turbo"),
        "medium" => Ok("medium"),
        THAI_CANDIDATE_PROFILE => Ok(THAI_CANDIDATE_MODEL),
        "large-v3" | "reference" => Err(
            "large-v3 is qualification-only; run the reference worker explicitly instead of selecting it in the desktop profile".to_string(),
        ),
        profile => Err(format!(
            "invalid FUNG_WHISPER_MODEL_PROFILE '{profile}'; use 'turbo', 'medium', or '{THAI_CANDIDATE_PROFILE}'"
        )),
    }
}

fn whisper_model_backend_from(configured: Option<&str>) -> Result<&'static str, String> {
    match configured.unwrap_or("turbo") {
        THAI_CANDIDATE_PROFILE => Ok("transformers"),
        "turbo" | "medium" => Ok("faster-whisper"),
        "large-v3" | "reference" => Err(
            "large-v3 is qualification-only; run the reference worker explicitly instead of selecting it in the desktop profile".to_string(),
        ),
        profile => Err(format!(
            "invalid FUNG_WHISPER_MODEL_PROFILE '{profile}'; use 'turbo', 'medium', or '{THAI_CANDIDATE_PROFILE}'"
        )),
    }
}

fn whisper_worker_script_for_profile(
    runtime: &WhisperRuntime,
    profile: &str,
    live: bool,
) -> Result<PathBuf, String> {
    if profile == THAI_CANDIDATE_PROFILE {
        let scripts_dir = runtime
            .script
            .parent()
            .ok_or_else(|| "scripts directory not found".to_string())?;
        return Ok(scripts_dir.join(if live {
            "transcribe_transformers_live.py"
        } else {
            "transcribe_transformers.py"
        }));
    }
    if live {
        let scripts_dir = runtime
            .script
            .parent()
            .ok_or_else(|| "scripts directory not found".to_string())?;
        return Ok(scripts_dir.join("transcribe_live.py"));
    }
    Ok(runtime.script.clone())
}

pub(crate) fn whisper_worker_script(
    runtime: &WhisperRuntime,
    live: bool,
) -> Result<PathBuf, String> {
    let configured = env::var("FUNG_WHISPER_MODEL_PROFILE").ok();
    let profile = configured.as_deref().unwrap_or("turbo");
    whisper_model_backend_from(configured.as_deref())?;
    whisper_worker_script_for_profile(runtime, profile, live)
}

fn bundled_whisper_model_for_profile(runtime: &WhisperRuntime, profile: &str) -> Option<PathBuf> {
    let runtime_root = runtime.python.parent()?.parent()?;
    let model = whisper_model_name_from(Some(profile)).ok()?;
    let model_root = if profile == THAI_CANDIDATE_PROFILE {
        runtime_root.parent()?.join(THAI_CANDIDATE_RUNTIME)
    } else {
        runtime_root.to_path_buf()
    };
    Some(model_root.join("models").join(model))
}

fn bundled_whisper_model(runtime: &WhisperRuntime) -> Option<PathBuf> {
    let configured = env::var("FUNG_WHISPER_MODEL_PROFILE").ok();
    bundled_whisper_model_for_profile(runtime, configured.as_deref().unwrap_or("turbo"))
}

pub(crate) fn require_bundled_whisper_model(runtime: &WhisperRuntime) -> Result<PathBuf, String> {
    let configured = env::var("FUNG_WHISPER_MODEL_PROFILE").ok();
    let model = whisper_model_name_from(configured.as_deref())?;
    let model_path = bundled_whisper_model(runtime).ok_or_else(|| {
        "FUNG Whisper runtime layout is invalid; the bundled Python path has no runtime root"
            .to_string()
    })?;
    if !model_path.is_dir() {
        let staging_hint = if configured.as_deref() == Some(THAI_CANDIDATE_PROFILE) {
            "scripts/stage_whisper_transformers_candidate.ps1"
        } else {
            "scripts/stage_whisper_runtime.ps1"
        };
        return Err(format!(
            "FUNG Whisper model '{model}' is missing at {}. Stage it with {staging_hint}.",
            model_path.display()
        ));
    }
    Ok(model_path)
}

fn child_compatible_whisper_model_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(path) = path.to_str() {
        if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            let bytes = rest.as_bytes();
            if bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/')
            {
                return PathBuf::from(rest);
            }
        }
    }

    path
}

fn worker_whisper_model_env_path(runtime: &WhisperRuntime) -> Option<PathBuf> {
    bundled_whisper_model(runtime).map(child_compatible_whisper_model_path)
}

/// Opens the configured FUNG account portal through the native trusted URL
/// registry. The webview never supplies the destination.
#[tauri::command]
fn open_external_account_portal(app: tauri::AppHandle) -> AppResult<()> {
    native_auth::open_trusted_account_portal(app)
}

/// Opens this project's Google authorize URL for the mobile login flow. The
/// webview supplies only the PKCE code challenge; every other part of the
/// URL is fixed natively.
#[tauri::command]
fn auth_open_google_authorize(app: tauri::AppHandle, code_challenge: String) -> AppResult<()> {
    native_auth::open_google_authorize(app, &code_challenge)
}

/// Exchanges the mobile login's PKCE code for a session natively so the
/// webview never performs network egress. Returns the tokens for supabase-js
/// to adopt in the webview (the mobile session-custody model).
#[tauri::command]
async fn auth_exchange_google_code(
    code: String,
    code_verifier: String,
) -> AppResult<native_auth::MobileSession> {
    native_auth::exchange_google_code(&code, &code_verifier).await
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountPortalStatus {
    pub(crate) status: &'static str,
}

#[tauri::command(rename = "account_portal_open")]
fn account_portal_open(app: tauri::AppHandle) -> AppResult<AccountPortalStatus> {
    native_auth::open_trusted_account_portal(app)?;
    Ok(AccountPortalStatus { status: "opened" })
}

#[tauri::command(rename = "broker_fungwire_status")]
fn broker_fungwire_status(
    state: State<'_, AppState>,
) -> AppResult<fungwire_server::FungwireStatus> {
    fungwire_server::fungwire_status(state)
}

#[tauri::command(rename = "broker_fungwire_set_enabled")]
fn broker_fungwire_set_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<fungwire_server::FungwireStatus> {
    fungwire_server::fungwire_server_set_enabled(enabled, state)
}

#[derive(Debug, Error)]
enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("app data directory is not available")]
    MissingAppDataDir,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("GenesisBlockDB error: {0}")]
    Genesis(String),
    #[error("TTS error: {0}")]
    Tts(String),
    #[error("cloud error: {0}")]
    Cloud(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

type AppResult<T> = Result<T, AppError>;

pub(crate) struct AppState {
    pub(crate) data_root: PathBuf,
    pub(crate) recording_output: Arc<Mutex<recording_output::RecordingOutputManager>>,
    pub(crate) genesis: Arc<genesis_block_native::Storage>,
    pub(crate) genesis_path: PathBuf,
    pub(crate) meeting_owner_session: Mutex<Option<genesis_adapter::NativeOwnerUnlockSession>>,
    pub(crate) meeting_intelligence:
        Mutex<meeting_intelligence_runtime::MeetingIntelligenceRuntime>,
    pub(crate) local_api: Mutex<Option<local_api::LocalApiControl>>,
    whisper_runtime: WhisperRuntime,
    pub(crate) mobile_gateway: Mutex<Option<mobile::MobileGatewayControl>>,
    pub(crate) fungwire: Mutex<Option<fungwire_server::FungwireServerControl>>,
    pub(crate) live: Arc<Mutex<Option<live_meeting::LiveSessionControl>>>,
    pub(crate) review_registry: Mutex<recording_review::ReviewRegistry>,
    pub(crate) playback: desktop_playback::PlaybackManager,
    pub(crate) native_capture: Arc<recording_review::NativeCaptureGuard>,
    pub(crate) main_window_owner: u128,
    /// The durable job queue. Cloneable handle, not a lock: the worker owns
    /// its own state, so a command that enqueues never blocks behind a job.
    pub(crate) jobs: job_engine::JobEngine,
    pub(crate) external_mcp: external_mcp_commands::ExternalMcpRuntime,
    pub(crate) external_meeting_tools_enabled: bool,
}

impl AppState {
    pub(crate) fn whisper_runtime_clone(&self) -> WhisperRuntime {
        self.whisper_runtime.clone()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    app: String,
    version: String,
    database_path: String,
    sqlite_wal: bool,
    genesis_path: String,
    genesis_stable_frontier: u64,
    storage_authority: String,
    local_api: LocalApiHealth,
    /// Jobs waiting or retrying. A non-zero depth with nothing visible in
    /// the UI is the signal that the worker is stuck, which is exactly the
    /// state the old fire-and-forget threads could not report at all.
    pending_jobs: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalApiHealth {
    running: bool,
    bind: Option<String>,
    /// Set only while the opt-in LAN listener is on (`set_local_api_lan`).
    lan_bind: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    name: String,
    storage_path: String,
    active_recording_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) project_id: String,
    #[serde(rename = "type")]
    pub(crate) job_type: String,
    pub(crate) status: String,
    pub(crate) progress: i64,
    pub(crate) input_refs: Vec<String>,
    pub(crate) output_refs: Vec<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptSegment {
    id: String,
    project_id: String,
    recording_id: String,
    speaker_id: Option<String>,
    speaker_name: Option<String>,
    start_ms: i64,
    end_ms: i64,
    text: String,
    confidence: Option<f64>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WhisperSegment {
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WhisperOutput {
    pub(crate) duration_ms: i64,
    pub(crate) segments: Vec<WhisperSegment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelProvider {
    id: String,
    label: String,
    runtime_location: String,
    kind: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

/// Desktop-local record of a paired mobile device. Persisted in a dedicated
/// SQLite WAL file (`paired_devices.db`) rather than the legacy `fung.db` —
/// `fung.db` is a one-time-import source consumed by
/// `genesis_adapter::import_legacy_sqlite`, which matches tables purely by
/// name against GenesisBlockDB's own schema (which happens to also define a
/// table named `paired_devices`, for an unrelated mobile-side capability
/// concept). Sharing that file/name would let the legacy importer sweep rows
/// out of this table using the wrong column set. See Task 5 report for
/// details.
#[cfg(test)]
#[derive(Debug, Deserialize)]
pub(crate) struct PairedDeviceInput {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) platform: String,
    pub(crate) fingerprint: String,
    pub(crate) pairing_session_id: String,
    pub(crate) public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PairedDeviceRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) platform: String,
    pub(crate) fingerprint: String,
    pub(crate) paired_at: String,
    pub(crate) revoked_at: Option<String>,
    pub(crate) pairing_session_id: String,
    pub(crate) public_key: Option<String>,
}

fn ensure_paired_devices_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS paired_devices (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          platform TEXT NOT NULL,
          fingerprint TEXT NOT NULL,
          paired_at TEXT NOT NULL,
          revoked_at TEXT,
          pairing_session_id TEXT NOT NULL,
          public_key TEXT
        );
        "#,
    )?;
    // CREATE TABLE IF NOT EXISTS is a no-op on a paired_devices.db written
    // before this task, so a fresh column definition above never reaches an
    // existing table. SQLite has no ADD COLUMN IF NOT EXISTS, so probe via
    // PRAGMA table_info and ALTER only when the column is actually missing.
    let has_public_key = conn
        .prepare("PRAGMA table_info(paired_devices)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|column_name| column_name == "public_key");
    if !has_public_key {
        conn.execute("ALTER TABLE paired_devices ADD COLUMN public_key TEXT", [])?;
    }
    Ok(())
}

#[cfg(test)]
fn upsert_paired_device(conn: &Connection, device: PairedDeviceInput) -> AppResult<()> {
    conn.execute(
        r#"
        INSERT INTO paired_devices (id, name, platform, fingerprint, paired_at, revoked_at, pairing_session_id, public_key)
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            revoked_at = NULL,
            public_key = excluded.public_key
        "#,
        params![
            device.id,
            device.name,
            device.platform,
            device.fingerprint,
            now(),
            device.pairing_session_id,
            device.public_key,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
fn list_paired_devices(conn: &Connection) -> AppResult<Vec<PairedDeviceRow>> {
    let mut statement = conn.prepare(
        "SELECT id, name, platform, fingerprint, paired_at, revoked_at, pairing_session_id, public_key \
         FROM paired_devices ORDER BY paired_at DESC",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(PairedDeviceRow {
                id: row.get(0)?,
                name: row.get(1)?,
                platform: row.get(2)?,
                fingerprint: row.get(3)?,
                paired_at: row.get(4)?,
                revoked_at: row.get(5)?,
                pairing_session_id: row.get(6)?,
                public_key: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
fn revoke_paired_device(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE paired_devices SET revoked_at = ?1 WHERE id = ?2",
        params![now(), id],
    )?;
    Ok(())
}

/// Opens (creating if needed) the `paired_devices.db` that lives under
/// `dir` (an app data directory). Split out from `paired_devices_connection`
/// so the FUNGWIRE server (Task 6) can look up a peer using only the app
/// data path it already has, without needing a full `AppState`/`State<'_,_>`
/// (which isn't available off the Tauri command dispatch path, e.g. inside a
/// TCP accept-loop worker thread).
fn paired_devices_connection_at(dir: &std::path::Path) -> AppResult<Connection> {
    let db_path = dir.join("paired_devices.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    ensure_paired_devices_table(&conn)?;
    Ok(conn)
}

fn paired_devices_connection(state: &AppState) -> AppResult<Connection> {
    paired_devices_connection_at(&state.data_root)
}

/// Looks up a single paired, non-revoked device by id. Used by the FUNGWIRE
/// server's pre-Noise peer check (Task 6): the responder must know which
/// peer's static key to expect, and must refuse unknown or revoked devices
/// before spending any handshake work on them. Returns `Ok(None)` rather
/// than an error for "no such active pairing" — that's an expected, routine
/// outcome the caller turns into a rejected connection, not a failure.
pub(crate) fn lookup_paired_peer(
    app_data: &std::path::Path,
    device_id: &str,
) -> AppResult<Option<PairedDeviceRow>> {
    let conn = paired_devices_connection_at(app_data)?;
    let mut statement = conn.prepare(
        "SELECT id, name, platform, fingerprint, paired_at, revoked_at, pairing_session_id, public_key \
         FROM paired_devices WHERE id = ?1 AND revoked_at IS NULL",
    )?;
    let mut rows = statement.query_map(params![device_id], |row| {
        Ok(PairedDeviceRow {
            id: row.get(0)?,
            name: row.get(1)?,
            platform: row.get(2)?,
            fingerprint: row.get(3)?,
            paired_at: row.get(4)?,
            revoked_at: row.get(5)?,
            pairing_session_id: row.get(6)?,
            public_key: row.get(7)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Best-effort LAN-routable IPv4 for this machine, found without any network
/// traffic or external dependency: `connect`ing a UDP socket to a public
/// address just makes the OS pick a local route/interface (no packet is
/// actually sent for UDP `connect`), and `local_addr()` reads that choice
/// back. Returns `None` on any failure (no route, no network, sandboxed
/// environment, etc.) or if the resolved address is loopback/non-IPv4 —
/// callers must treat `None` as "endpoint unknown", not an error.
pub(crate) fn primary_lan_ipv4() -> Option<String> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?; // no packet sent; just sets the local addr
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
        _ => None,
    }
}

/// Desktop-side publisher half of Task 9: reports `"<lan-ip>:<port>"` for the
/// FUNGWIRE server Task 6 binds via `fungwire_server_set_enabled`, so the
/// frontend (Task 10) can write it to Supabase `devices.lan_endpoint` for
/// mobile to resolve. Returns `Ok(None)` — not an error — whenever the
/// server isn't currently bound or the LAN IP can't be determined; the
/// stored bind is `"0.0.0.0:PORT"` (unroutable), so the concrete port is
/// combined with `primary_lan_ipv4()` rather than returned as-is.
pub(crate) fn fungwire_local_endpoint_native(
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let bind = {
        let guard = state.fungwire.lock().expect("fungwire mutex poisoned");
        match guard.as_ref() {
            Some(control) => control.bind.clone(),
            None => return Ok(None),
        }
    };

    let port = match bind.rsplit_once(':') {
        Some((_, port)) => port,
        None => return Ok(None),
    };

    Ok(primary_lan_ipv4().map(|ip| format!("{ip}:{port}")))
}

pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
#[allow(dead_code)]
fn init_database(db_path: PathBuf) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            storage_path TEXT NOT NULL,
            active_recording_id TEXT,
            archived_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS recordings (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            source TEXT NOT NULL CHECK (source IN ('microphone', 'import')),
            input_path TEXT,
            canonical_audio_path TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'recording', 'paused', 'completed', 'failed')),
            duration_ms INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audio_chunks (
            id TEXT PRIMARY KEY,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            sequence_no INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            byte_size INTEGER NOT NULL DEFAULT 0,
            checksum TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(recording_id, sequence_no)
        );

        CREATE TABLE IF NOT EXISTS audio_layers (
            id TEXT PRIMARY KEY,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('original', 'cleaned', 'noise_reduced_export', 'selected_clip', 'voice', 'music', 'noise')),
            file_path TEXT NOT NULL,
            source_chunk_id TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS speakers (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            confidence REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, key)
        );

        CREATE TABLE IF NOT EXISTS transcript_segments (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            speaker_id TEXT REFERENCES speakers(id) ON DELETE SET NULL,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            text TEXT NOT NULL,
            confidence REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS model_providers (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            runtime_location TEXT NOT NULL CHECK (runtime_location IN ('local', 'lan', 'cloud')),
            kind TEXT NOT NULL CHECK (kind IN ('transcription', 'diarization', 'cleanup', 'separation', 'summary_intent', 'tts')),
            enabled INTEGER NOT NULL DEFAULT 1,
            config_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS model_runs (
            id TEXT PRIMARY KEY,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            provider_id TEXT NOT NULL REFERENCES model_providers(id),
            model_name TEXT NOT NULL,
            task_kind TEXT NOT NULL,
            runtime_location TEXT NOT NULL CHECK (runtime_location IN ('local', 'lan', 'cloud')),
            input_ref TEXT NOT NULL,
            output_ref TEXT NOT NULL,
            parameters_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            type TEXT NOT NULL CHECK (type IN ('recording.capture', 'recording.recover', 'audio.cleanup', 'audio.separate', 'transcript.transcribe', 'transcript.diarize', 'summary.generate', 'intent.infer', 'export.render', 'zoom.import', 'graph.build')),
            status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'paused', 'completed', 'failed', 'retrying', 'cancelled')),
            progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100),
            input_refs_json TEXT NOT NULL DEFAULT '[]',
            output_refs_json TEXT NOT NULL DEFAULT '[]',
            provider_id TEXT REFERENCES model_providers(id),
            error_code TEXT,
            error_message TEXT,
            attempt_no INTEGER NOT NULL DEFAULT 1,
            started_at TEXT,
            finished_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS job_events (
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
            status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'paused', 'completed', 'failed', 'retrying', 'cancelled')),
            message TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS summaries (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('whole_story', 'timeline', 'decisions_actions', 'speaker')),
            content TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL DEFAULT '[]',
            model_run_id TEXT NOT NULL REFERENCES model_runs(id),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS intent_inferences (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE CASCADE,
            label TEXT NOT NULL,
            confidence REAL NOT NULL,
            evidence_refs_json TEXT NOT NULL DEFAULT '[]',
            model_run_id TEXT NOT NULL REFERENCES model_runs(id),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS export_artifacts (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('wav', 'mp3', 'txt', 'srt', 'vtt', 'json')),
            file_path TEXT NOT NULL,
            source_layer_id TEXT REFERENCES audio_layers(id) ON DELETE SET NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audit_events (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            event_type TEXT NOT NULL,
            actor TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tts_test_results (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL REFERENCES model_providers(id) ON DELETE CASCADE,
            status TEXT NOT NULL CHECK (status IN ('ok', 'error')),
            latency_ms INTEGER,
            sample_audio_path TEXT,
            error_message TEXT,
            tested_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS uq_jobs_active_recording_capture
          ON jobs(project_id, type)
          WHERE type = 'recording.capture'
            AND status IN ('queued', 'running', 'paused', 'retrying');

        CREATE INDEX IF NOT EXISTS idx_job_events_job_id ON job_events(job_id, created_at);
        "#,
    )?;

    mobile::init_schema(&conn)?;

    let inserted_at = now();
    let seed_model_providers_sql = format!(
        r#"
        INSERT OR IGNORE INTO model_providers
            (id, label, runtime_location, kind, enabled, config_json, created_at, updated_at)
        VALUES
            ('ollama-summary-intent', 'Ollama / llama.cpp', 'local', 'summary_intent', 1, '{{"endpoint":"{DEFAULT_OLLAMA_ENDPOINT}"}}', ?1, ?1),
            ('vllm-summary-intent', 'vLLM', 'local', 'summary_intent', 0, '{{"endpoint":"{DEFAULT_VLLM_ENDPOINT}"}}', ?1, ?1)
        "#
    );
    conn.execute(&seed_model_providers_sql, params![inserted_at])?;

    Ok(conn)
}

fn app_state(app: &tauri::App) -> AppResult<AppState> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::MissingAppDataDir)?;
    let default_output_root = app
        .path()
        .document_dir()
        .map_err(|_| AppError::InvalidInput("Documents directory is not available".to_string()))?
        .join("fung");
    let recording_output =
        recording_output::RecordingOutputManager::load(app_data_dir.clone(), default_output_root)
            .map_err(AppError::InvalidInput)?;
    let legacy_db_path = app_data_dir.join("fung.db");
    let genesis_path = app_data_dir.join("genesisdb");
    let genesis = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
        path: genesis_path.display().to_string(),
        page_cache_mb: Some(64),
        read_only: Some(false),
        vector_dim: Some(384),
        retention: None,
    })
    .map_err(|error| AppError::Genesis(error.to_string()))?;
    genesis_adapter::install(&genesis).map_err(AppError::Genesis)?;
    let legacy_marker = genesis_path.join("legacy-fung-sqlite-import-v1.complete");
    if legacy_db_path.is_file() && !legacy_marker.is_file() {
        genesis_adapter::import_legacy_sqlite(&genesis, &legacy_db_path)
            .map_err(AppError::Genesis)?;
        std::fs::write(&legacy_marker, now())?;
    }
    let seeded_at = now();
    genesis_adapter::commit_rows(&genesis, vec![
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":DEFAULT_OLLAMA_ENDPOINT},"created_at":seeded_at,"updated_at":seeded_at})),
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"vllm-summary-intent","label":"vLLM","runtime_location":"local","kind":"summary_intent","enabled":false,"config_json":{"endpoint":DEFAULT_VLLM_ENDPOINT},"created_at":seeded_at,"updated_at":seeded_at})),
    ]).map_err(AppError::Genesis)?;
    let genesis = Arc::new(genesis);
    Ok(AppState {
        data_root: app_data_dir,
        recording_output: Arc::new(Mutex::new(recording_output)),
        jobs: job_engine::JobEngine::new(Arc::clone(&genesis)),
        genesis,
        genesis_path,
        meeting_owner_session: Mutex::new(None),
        meeting_intelligence: Mutex::new(
            meeting_intelligence_runtime::MeetingIntelligenceRuntime::default(),
        ),
        local_api: Mutex::new(None),
        whisper_runtime: whisper_runtime(app),
        mobile_gateway: Mutex::new(None),
        fungwire: Mutex::new(None),
        live: Arc::new(Mutex::new(None)),
        review_registry: Mutex::new(recording_review::ReviewRegistry::default()),
        playback: desktop_playback::PlaybackManager::new(),
        native_capture: Arc::new(recording_review::NativeCaptureGuard::default()),
        main_window_owner: recording_review::random_owner_id(),
        external_mcp: external_mcp_commands::ExternalMcpRuntime::default(),
        external_meeting_tools_enabled: matches!(
            env::var("FUNG_EXTERNAL_MEETING_TOOLS").as_deref(),
            Ok("1")
        ),
    })
}

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> AppResult<Health> {
    let (bind, lan_bind) = {
        let guard = state.local_api.lock().expect("local api mutex poisoned");
        (
            guard.as_ref().map(|control| control.bind.clone()),
            guard
                .as_ref()
                .and_then(|control| control.lan.as_ref().map(|lan| lan.bind.clone())),
        )
    };

    Ok(Health {
        app: "FUNG".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_path: state.genesis_path.display().to_string(),
        sqlite_wal: true,
        genesis_path: state.genesis_path.display().to_string(),
        genesis_stable_frontier: state.genesis.stable_frontier(),
        storage_authority: "GenesisBlockDB signed WAL".to_string(),
        local_api: LocalApiHealth {
            running: bind.is_some(),
            bind,
            lan_bind,
        },
        pending_jobs: state.jobs.queue_depth(),
    })
}

#[tauri::command]
fn create_project(name: String, state: State<'_, AppState>) -> AppResult<Project> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "project name is required".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let output_root = state
        .recording_output
        .lock()
        .expect("recording output mutex poisoned")
        .ensure_current_writable()
        .map_err(AppError::InvalidInput)?;
    let storage_path = output_root.join("projects").join(&id).display().to_string();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("projects", serde_json::json!({"id": id, "name": trimmed, "storage_path": storage_path, "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": id, "project_id": id, "entity_type": "project", "entity_id": id, "label": trimmed, "position_x": 50.0, "position_y": 17.0, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("audit_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": id, "event_type": "project.created", "actor": "user", "payload_json": {}, "created_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

    Ok(Project {
        id,
        name: trimmed.to_string(),
        storage_path,
        active_recording_id: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    let mut projects = genesis_adapter::query(
        &state.genesis,
        "projects",
        &[
            "id",
            "name",
            "storage_path",
            "active_recording_id",
            "created_at",
            "updated_at",
        ],
        vec![],
        genesis_adapter::ROW_CAP,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        Ok(Project {
            id: genesis_adapter::string(&row, "projects.id").map_err(AppError::Genesis)?,
            name: genesis_adapter::string(&row, "projects.name").map_err(AppError::Genesis)?,
            storage_path: genesis_adapter::string(&row, "projects.storage_path")
                .map_err(AppError::Genesis)?,
            active_recording_id: row
                .get("projects.active_recording_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            created_at: genesis_adapter::string(&row, "projects.created_at")
                .map_err(AppError::Genesis)?,
            updated_at: genesis_adapter::string(&row, "projects.updated_at")
                .map_err(AppError::Genesis)?,
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    projects.sort_by_key(|project| {
        std::cmp::Reverse((project.updated_at.clone(), project.created_at.clone()))
    });
    Ok(projects)
}

/// Queues a job and returns the row that will run.
///
/// The old version accepted any type string and wrote a `queued` row. Since
/// nothing consumed the queue, twelve UI buttons filed rows that sat until
/// the next launch terminalised them — a spinner that meant nothing. It now
/// refuses a type the engine has no handler for, so an unsupported action
/// fails at the moment the user takes it rather than pretending to work.
///
/// Returns the existing job when an equivalent one is already pending, which
/// is why the response carries the id rather than assuming a fresh one.
#[tauri::command]
fn create_job(
    job_type: String,
    project_id: Option<String>,
    recording_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Job> {
    let kind = job_engine::JobKind::parse(&job_type).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "'{}' is not a job this build can run",
            job_type.trim()
        ))
    })?;
    let project_id =
        project_id.ok_or_else(|| AppError::InvalidInput("job needs a project".to_string()))?;
    let recording_id = recording_id.filter(|value| !value.trim().is_empty());
    let id = state
        .jobs
        .enqueue(kind, &project_id, recording_id.as_deref())
        .map_err(AppError::Genesis)?;
    job_by_id(&state.genesis, &id)
}

/// Asks the engine to stop a job. The outcome distinguishes "it will not
/// run" from "it is already running and cannot be interrupted", because
/// reporting both as success is how a cancel button comes to mean nothing.
#[tauri::command]
fn cancel_job(job_id: String, state: State<'_, AppState>) -> AppResult<job_engine::CancelOutcome> {
    Ok(state.jobs.cancel(&job_id))
}

/// The job types this build can actually run, for a UI that would otherwise
/// have to hard-code the list and drift from it.
#[tauri::command]
fn runnable_job_types() -> Vec<&'static str> {
    job_engine::JobKind::ALL
        .into_iter()
        .map(job_engine::JobKind::as_str)
        .collect()
}

fn job_by_id(storage: &genesis_block_native::Storage, job_id: &str) -> AppResult<Job> {
    genesis_adapter::query(
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
            "provider_id",
            "error_code",
            "error_message",
            "started_at",
            "finished_at",
            "created_at",
            "updated_at",
        ],
        vec![genesis_adapter::eq("jobs", "id", serde_json::json!(job_id))],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("job not found".to_string()))
    .and_then(job_from_row)
}

#[tauri::command]
fn list_jobs(state: State<'_, AppState>) -> AppResult<Vec<Job>> {
    // Read whole before sorting: with more than one engine page of jobs, a
    // single capped read could miss the newest rows entirely.
    let mut jobs = genesis_adapter::query_all(
        &state.genesis,
        "jobs",
        &[
            "id",
            "project_id",
            "type",
            "status",
            "progress",
            "input_refs_json",
            "output_refs_json",
            "provider_id",
            "error_code",
            "error_message",
            "started_at",
            "finished_at",
            "created_at",
            "updated_at",
        ],
        vec![],
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(job_from_row)
    .collect::<AppResult<Vec<_>>>()?;
    jobs.sort_by_key(|job| std::cmp::Reverse(job.created_at.clone()));
    jobs.truncate(30);
    Ok(jobs)
}

fn job_from_row(row: serde_json::Value) -> AppResult<Job> {
    let json_list = |key: &str| {
        row.get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default()
    };
    let optional = |key: &str| {
        row.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    Ok(Job {
        id: genesis_adapter::string(&row, "jobs.id").map_err(AppError::Genesis)?,
        project_id: genesis_adapter::string(&row, "jobs.project_id").map_err(AppError::Genesis)?,
        job_type: genesis_adapter::string(&row, "jobs.type").map_err(AppError::Genesis)?,
        status: genesis_adapter::string(&row, "jobs.status").map_err(AppError::Genesis)?,
        progress: genesis_adapter::integer(&row, "jobs.progress").map_err(AppError::Genesis)?,
        input_refs: json_list("jobs.input_refs_json"),
        output_refs: json_list("jobs.output_refs_json"),
        provider_id: optional("jobs.provider_id"),
        error_code: optional("jobs.error_code"),
        error_message: optional("jobs.error_message"),
        started_at: optional("jobs.started_at"),
        finished_at: optional("jobs.finished_at"),
        created_at: genesis_adapter::string(&row, "jobs.created_at").map_err(AppError::Genesis)?,
        updated_at: genesis_adapter::string(&row, "jobs.updated_at").map_err(AppError::Genesis)?,
    })
}

#[tauri::command]
fn list_model_providers(state: State<'_, AppState>) -> AppResult<Vec<ModelProvider>> {
    let mut providers = genesis_adapter::query(
        &state.genesis,
        "model_providers",
        &[
            "id",
            "label",
            "runtime_location",
            "kind",
            "enabled",
            "created_at",
            "updated_at",
        ],
        vec![],
        500,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        Ok(ModelProvider {
            id: genesis_adapter::string(&row, "model_providers.id").map_err(AppError::Genesis)?,
            label: genesis_adapter::string(&row, "model_providers.label")
                .map_err(AppError::Genesis)?,
            runtime_location: genesis_adapter::string(&row, "model_providers.runtime_location")
                .map_err(AppError::Genesis)?,
            kind: genesis_adapter::string(&row, "model_providers.kind")
                .map_err(AppError::Genesis)?,
            enabled: row
                .get("model_providers.enabled")
                .and_then(|value| {
                    value
                        .as_bool()
                        .or_else(|| value.as_i64().map(|number| number != 0))
                })
                .unwrap_or(false),
            created_at: genesis_adapter::string(&row, "model_providers.created_at")
                .map_err(AppError::Genesis)?,
            updated_at: genesis_adapter::string(&row, "model_providers.updated_at")
                .map_err(AppError::Genesis)?,
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    providers.sort_by_key(|provider| (provider.runtime_location.clone(), provider.label.clone()));
    Ok(providers)
}

fn model_provider_row_enabled(row: &serde_json::Value) -> bool {
    row.get("model_providers.enabled")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_i64().map(|number| number != 0))
        })
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsRegisterInput {
    label: String,
    config_json: String, // JSON string of tts_config::TtsProviderConfig
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsRegisterOutput {
    provider_id: String,
    validation: tts_config::TtsValidation,
}

#[tauri::command]
fn tts_provider_register(
    input: TtsRegisterInput,
    state: State<'_, AppState>,
) -> AppResult<TtsRegisterOutput> {
    let config: tts_config::TtsProviderConfig = serde_json::from_str(&input.config_json)
        .map_err(|e| AppError::InvalidInput(format!("config ไม่ถูกรูปแบบ: {e}")))?;

    let validation = config.validate();
    if !validation.ok {
        return Ok(TtsRegisterOutput {
            provider_id: String::new(),
            validation,
        });
    }

    let id = Uuid::new_v4().to_string();
    let timestamp = now();

    genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "model_providers",
            serde_json::json!({
                "id": id,
                "label": input.label,
                "runtime_location": "local",
                "kind": "tts",
                "enabled": true,
                "config_json": input.config_json,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        )],
    )
    .map_err(AppError::Genesis)?;

    Ok(TtsRegisterOutput {
        provider_id: id,
        validation,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsUpdateInput {
    provider_id: String,
    label: Option<String>,
    config_json: Option<String>,
}

#[tauri::command]
fn tts_provider_update(
    input: TtsUpdateInput,
    state: State<'_, AppState>,
) -> AppResult<tts_config::TtsValidation> {
    // If config_json is provided, validate it before persisting anything.
    if let Some(ref config_json) = input.config_json {
        let config: tts_config::TtsProviderConfig = serde_json::from_str(config_json)
            .map_err(|e| AppError::InvalidInput(format!("config ไม่ถูกรูปแบบ: {e}")))?;
        let validation = config.validate();
        if !validation.ok {
            return Ok(validation);
        }
    }

    let timestamp = now();

    let rows = genesis_adapter::query(
        &state.genesis,
        "model_providers",
        &[
            "id",
            "label",
            "runtime_location",
            "kind",
            "enabled",
            "config_json",
            "created_at",
            "updated_at",
        ],
        vec![genesis_adapter::eq(
            "model_providers",
            "id",
            serde_json::json!(input.provider_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?;

    let row = rows
        .first()
        .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบ provider: {}", input.provider_id)))?;

    let label = match input.label {
        Some(label) => label,
        None => genesis_adapter::string(row, "model_providers.label").map_err(AppError::Genesis)?,
    };
    let config_json = match input.config_json {
        Some(config_json) => config_json,
        None => genesis_adapter::string(row, "model_providers.config_json")
            .map_err(AppError::Genesis)?,
    };
    let runtime_location = genesis_adapter::string(row, "model_providers.runtime_location")
        .map_err(AppError::Genesis)?;
    let kind = genesis_adapter::string(row, "model_providers.kind").map_err(AppError::Genesis)?;
    let enabled = model_provider_row_enabled(row);
    let created_at =
        genesis_adapter::string(row, "model_providers.created_at").map_err(AppError::Genesis)?;

    genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "model_providers",
            serde_json::json!({
                "id": input.provider_id,
                "label": label,
                "runtime_location": runtime_location,
                "kind": kind,
                "enabled": enabled,
                "config_json": config_json,
                "created_at": created_at,
                "updated_at": timestamp,
            }),
        )],
    )
    .map_err(AppError::Genesis)?;

    Ok(tts_config::TtsValidation {
        ok: true,
        error: None,
        warnings: vec![],
    })
}

#[tauri::command]
fn tts_provider_toggle(
    provider_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<bool> {
    let timestamp = now();

    let rows = genesis_adapter::query(
        &state.genesis,
        "model_providers",
        &[
            "id",
            "label",
            "runtime_location",
            "kind",
            "enabled",
            "config_json",
            "created_at",
            "updated_at",
        ],
        vec![genesis_adapter::eq(
            "model_providers",
            "id",
            serde_json::json!(provider_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?;

    let row = rows
        .first()
        .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบ provider: {provider_id}")))?;

    let label = genesis_adapter::string(row, "model_providers.label").map_err(AppError::Genesis)?;
    let runtime_location = genesis_adapter::string(row, "model_providers.runtime_location")
        .map_err(AppError::Genesis)?;
    let kind = genesis_adapter::string(row, "model_providers.kind").map_err(AppError::Genesis)?;
    let config_json =
        genesis_adapter::string(row, "model_providers.config_json").map_err(AppError::Genesis)?;
    let created_at =
        genesis_adapter::string(row, "model_providers.created_at").map_err(AppError::Genesis)?;

    genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "model_providers",
            serde_json::json!({
                "id": provider_id,
                "label": label,
                "runtime_location": runtime_location,
                "kind": kind,
                "enabled": enabled,
                "config_json": config_json,
                "created_at": created_at,
                "updated_at": timestamp,
            }),
        )],
    )
    .map_err(AppError::Genesis)?;

    Ok(true)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsTestOutput {
    status: String, // "ok" or "error"
    latency_ms: Option<u64>,
    audio_path: Option<String>,
    message: Option<String>,
}

#[tauri::command]
fn tts_provider_test(
    provider_id: String,
    test_text: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<TtsTestOutput> {
    let text = test_text.unwrap_or_else(|| "ทดสอบระบบเสียง".into());

    let rows = genesis_adapter::query(
        &state.genesis,
        "model_providers",
        &["id", "config_json"],
        vec![
            genesis_adapter::eq("model_providers", "id", serde_json::json!(provider_id)),
            genesis_adapter::eq("model_providers", "kind", serde_json::json!("tts")),
        ],
        1,
    )
    .map_err(AppError::Genesis)?;

    let row = rows
        .first()
        .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบ TTS provider: {provider_id}")))?;

    let config_str =
        genesis_adapter::string(row, "model_providers.config_json").map_err(AppError::Genesis)?;
    let config: tts_config::TtsProviderConfig = serde_json::from_str(&config_str)
        .map_err(|e| AppError::InvalidInput(format!("config ไม่ถูกรูปแบบ: {e}")))?;

    let temp_dir = std::env::temp_dir().join("fung-tts");
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("สร้าง temp dir ไม่ได้: {e}"),
        ))
    })?;

    let request = tts_executor::TtsSynthesisRequest {
        text,
        ref_audio: None,
        ref_text: None,
    };

    let (status, latency_ms, audio_path, message) =
        match tts_executor::dispatch(&config, &request, &temp_dir) {
            Ok(result) => (
                "ok".to_string(),
                Some(result.latency_ms),
                Some(result.audio_path.display().to_string()),
                None,
            ),
            Err(e) => ("error".to_string(), None, None, Some(e)),
        };

    // Record the test result; a failure to persist it should not fail the whole test.
    let timestamp = now();
    let test_id = Uuid::new_v4().to_string();
    let _ = genesis_adapter::commit_rows(
        &state.genesis,
        vec![genesis_adapter::upsert(
            "tts_test_results",
            serde_json::json!({
                "id": test_id,
                "provider_id": provider_id,
                "status": status,
                "latency_ms": latency_ms,
                "sample_audio_path": audio_path,
                "error_message": message,
                "tested_at": timestamp,
            }),
        )],
    );

    Ok(TtsTestOutput {
        status,
        latency_ms,
        audio_path,
        message,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsSynthesizeInput {
    text: String,
    provider_id: Option<String>,
    ref_audio: Option<String>,
    ref_text: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsSynthesizeOutput {
    audio_path: String,
    latency_ms: u64,
}

#[tauri::command]
fn tts_synthesize_text(
    input: TtsSynthesizeInput,
    state: State<'_, AppState>,
) -> AppResult<TtsSynthesizeOutput> {
    // Locate the config for the requested (or first enabled) TTS provider.
    let config_str = if let Some(pid) = &input.provider_id {
        let rows = genesis_adapter::query(
            &state.genesis,
            "model_providers",
            &["config_json"],
            vec![
                genesis_adapter::eq("model_providers", "id", serde_json::json!(pid)),
                genesis_adapter::eq("model_providers", "kind", serde_json::json!("tts")),
                genesis_adapter::eq("model_providers", "enabled", serde_json::json!(true)),
            ],
            1,
        )
        .map_err(AppError::Genesis)?;
        rows.first()
            .map(|row| genesis_adapter::string(row, "model_providers.config_json"))
            .transpose()
            .map_err(AppError::Genesis)?
            .ok_or_else(|| AppError::InvalidInput(format!("TTS provider '{pid}' ไม่พร้อมใช้งาน")))?
    } else {
        let rows = genesis_adapter::query(
            &state.genesis,
            "model_providers",
            &["config_json"],
            vec![
                genesis_adapter::eq("model_providers", "kind", serde_json::json!("tts")),
                genesis_adapter::eq("model_providers", "enabled", serde_json::json!(true)),
            ],
            1,
        )
        .map_err(AppError::Genesis)?;
        rows.first()
            .map(|row| genesis_adapter::string(row, "model_providers.config_json"))
            .transpose()
            .map_err(AppError::Genesis)?
            .ok_or_else(|| {
                AppError::InvalidInput("ยังไม่ได้ลงทะเบียน TTS provider — ไปตั้งค่าที่ Settings".to_string())
            })?
    };

    let config: tts_config::TtsProviderConfig = serde_json::from_str(&config_str)
        .map_err(|e| AppError::InvalidInput(format!("config ผิดพลาด: {e}")))?;

    let temp_dir = std::env::temp_dir().join("fung-tts");
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("สร้าง temp dir ไม่ได้: {e}"),
        ))
    })?;

    let request = tts_executor::TtsSynthesisRequest {
        text: input.text,
        ref_audio: input.ref_audio.map(std::path::PathBuf::from),
        ref_text: input.ref_text,
    };

    let result = tts_executor::dispatch(&config, &request, &temp_dir).map_err(AppError::Tts)?;

    Ok(TtsSynthesizeOutput {
        audio_path: result.audio_path.display().to_string(),
        latency_ms: result.latency_ms,
    })
}

/// A recording's transcript, read whole.
///
/// The read pages past the engine's single-query row ceiling via
/// `genesis_adapter::query_all`, so `segments` is always the complete
/// transcript. The `capped`/`cap`/`capped_recording_ids` fields date from
/// when the engine had no offset and a long recording could only be
/// truncated; they are kept so the frontend contract does not change shape,
/// and they now always report "nothing missing".
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptView {
    segments: Vec<TranscriptSegment>,
    /// Always false: the read pages until the last short page.
    capped: bool,
    /// The engine's single-read page size, kept for contract stability.
    cap: u32,
    /// Always empty, kept for contract stability.
    capped_recording_ids: Vec<String>,
}

#[tauri::command]
fn list_transcript_segments(
    project_id: String,
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<TranscriptView> {
    transcript_view(&state.genesis, &project_id, &recording_id)
}

#[tauri::command]
fn meeting_transcript_snapshot(
    project_id: String,
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingTranscriptSnapshot> {
    genesis_adapter::meeting_transcript_snapshot(&state.genesis, &project_id, &recording_id)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn replay_meeting_events(
    project_id: String,
    cursor: meeting_intelligence_schema::MeetingReplayCursor,
    limit: u32,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingReplayPage> {
    genesis_adapter::replay_meeting_events(
        &state.genesis,
        &project_id,
        &cursor.recording_id,
        &cursor,
        limit,
    )
    .map_err(AppError::Genesis)
}

#[tauri::command]
fn correct_meeting_utterance(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingRevisionRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::CommittedMeetingEvent> {
    let attempt = genesis_adapter::begin_meeting_commit(
        &state.genesis,
        &format!("meeting-correction::{}", request.revision.id),
        &now(),
    )
    .map_err(AppError::Genesis)?;
    let committed = genesis_adapter::revise_meeting_transcript(&state.genesis, &attempt, &request)
        .map_err(AppError::Genesis)?;
    let cursor = meeting_intelligence_schema::MeetingReplayCursor {
        recording_id: request.scope.recording_id.clone(),
        after_cursor: committed.cursor.saturating_sub(1),
    };
    if let Ok(page) = genesis_adapter::replay_meeting_events(
        &state.genesis,
        &request.scope.project_id,
        &request.scope.recording_id,
        &cursor,
        8,
    ) {
        use tauri::Emitter;
        for event in page.events {
            let _ = app.emit("meeting-transcript-event", event);
        }
    }
    Ok(committed)
}

#[tauri::command]
fn meeting_adapter_capabilities() -> meeting_adapter::CapabilityReport {
    meeting_adapter::MeetingTransport::probe_capabilities(
        &meeting_adapter::UnconfiguredMeetingTransport,
    )
}

#[tauri::command]
fn meeting_local_owner_provision(state: State<'_, AppState>) -> AppResult<String> {
    genesis_adapter::provision_native_local_owner_vault(&state.genesis).map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_local_owner_vault_options(
    state: State<'_, AppState>,
) -> AppResult<Vec<serde_json::Value>> {
    genesis_adapter::native_local_owner_vault_options(&state.genesis).map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_local_owner_unlock(
    vault_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let session =
        genesis_adapter::unlock_native_local_owner_selected(&state.genesis, vault_id.as_deref())
            .map_err(AppError::Genesis)?;
    let mut current = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    if let Some(previous) = current.take() {
        previous.lock();
    }
    state
        .meeting_intelligence
        .lock()
        .expect("meeting intelligence mutex poisoned")
        .clear_sensitive();
    *current = Some(session);
    Ok(())
}

#[tauri::command]
fn meeting_local_owner_lock(state: State<'_, AppState>) -> AppResult<()> {
    let mut current = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    if let Some(session) = current.take() {
        session.lock();
    }
    state
        .meeting_intelligence
        .lock()
        .expect("meeting intelligence mutex poisoned")
        .clear_sensitive();
    Ok(())
}

#[tauri::command]
fn meeting_knowledge_collection_create(
    request: meeting_intelligence_schema::MeetingKnowledgeCollectionCreateRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingKnowledgeCollectionCreateResult> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::create_meeting_knowledge_collection(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_knowledge_collections_list(
    project_id: String,
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<meeting_intelligence_schema::MeetingKnowledgeCollectionSummary>> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::list_meeting_knowledge_collections(
        &state.genesis,
        session,
        &project_id,
        &recording_id,
    )
    .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_knowledge_set_selection(
    request: meeting_intelligence_schema::MeetingKnowledgeSelectionCommand,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingKnowledgeSelectionReceipt> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::set_meeting_knowledge_selection(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
async fn meeting_knowledge_import_selected(
    app: tauri::AppHandle,
    collection_id: String,
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingKnowledgeImportResult> {
    let session = {
        let owner = state
            .meeting_owner_session
            .lock()
            .expect("meeting owner session mutex poisoned");
        owner
            .as_ref()
            .cloned()
            .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?
    };
    let storage = Arc::clone(&state.genesis);
    let python = state.whisper_runtime.python.clone();
    let scripts = state
        .whisper_runtime
        .script
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Genesis("KNOWLEDGE_PARSER_RUNTIME_UNAVAILABLE".to_string()))?;
    #[cfg(not(debug_assertions))]
    let pdf_parser_runtime = app
        .path()
        .resource_dir()
        .ok()
        .map(|resource_dir| resource_dir.join("knowledge-parser-runtime"));
    #[cfg(debug_assertions)]
    let pdf_parser_runtime =
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.knowledge-parser-runtime"));
    #[cfg(desktop)]
    let selected = {
        let (sender, receiver) = std::sync::mpsc::channel();
        app.dialog()
            .file()
            .add_filter("Text, Markdown and PDF", &["txt", "md", "pdf"])
            .pick_file(move |selection| {
                let path = selection.and_then(|value| value.into_path().ok());
                let _ = sender.send(path);
            });
        tauri::async_runtime::spawn_blocking(move || {
            receiver
                .recv_timeout(std::time::Duration::from_secs(300))
                .ok()
                .flatten()
        })
        .await
        .ok()
        .flatten()
    };
    #[cfg(mobile)]
    let selected: Option<PathBuf> = {
        let _ = &app;
        None
    };
    let selected = selected
        .ok_or_else(|| AppError::Genesis("KNOWLEDGE_FILE_SELECTION_CANCELLED".to_string()))?;
    genesis_adapter::import_selected_knowledge_document(
        &storage,
        &session,
        &collection_id,
        &project_id,
        &selected,
        &python,
        &scripts.join("extract_knowledge.py"),
        pdf_parser_runtime.as_deref(),
    )
    .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_knowledge_metric_save(
    request: meeting_intelligence_schema::MeetingKnowledgeMetricSaveRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingKnowledgeMetricSaveResult> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::save_meeting_knowledge_metric(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_knowledge_metric_compute(
    request: meeting_intelligence_schema::MeetingKnowledgeMetricComputeRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingKnowledgeMetricComputeResult> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::compute_meeting_knowledge_metric(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_list(
    request: meeting_intelligence_schema::MeetingPeopleListRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingPeopleSnapshot> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::list_meeting_people(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_profile_create(
    request: meeting_intelligence_schema::MeetingPeopleProfileCreateRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingPeopleProfile> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::create_meeting_people_profile(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_profile_update(
    request: meeting_intelligence_schema::MeetingPeopleProfileUpdateRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingPeopleProfile> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::update_meeting_people_profile(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_profile_archive(
    request: meeting_intelligence_schema::MeetingPeopleProfileArchiveRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::archive_meeting_people_profile(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_link_propose(
    request: meeting_intelligence_schema::MeetingPeopleLinkProposalRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingPeopleLink> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::propose_meeting_people_link(&state.genesis, session, &request)
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_link_confirm(
    request: meeting_intelligence_schema::MeetingPeopleLinkMutationRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::mutate_meeting_people_link(&state.genesis, session, &request, "confirm")
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_link_reject(
    request: meeting_intelligence_schema::MeetingPeopleLinkMutationRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::mutate_meeting_people_link(&state.genesis, session, &request, "reject")
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_people_link_unlink(
    request: meeting_intelligence_schema::MeetingPeopleLinkMutationRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let session = owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::mutate_meeting_people_link(&state.genesis, session, &request, "unlink")
        .map_err(AppError::Genesis)
}

fn meeting_agent_capability(
    ready: bool,
    reason: Option<&str>,
) -> meeting_intelligence_schema::MeetingAgentCapability {
    meeting_intelligence_schema::MeetingAgentCapability {
        readiness: if ready { "ready" } else { "blocked" }.to_string(),
        reason_code: if ready {
            None
        } else {
            reason.map(ToOwned::to_owned)
        },
    }
}

fn meeting_agent_status_snapshot(
    state: &AppState,
    selection: &meeting_intelligence_schema::MeetingAgentSelection,
) -> AppResult<(
    meeting_intelligence_schema::MeetingAgentStatus,
    i64,
    Vec<String>,
)> {
    let transcript = genesis_adapter::meeting_transcript_snapshot(
        &state.genesis,
        &selection.project_id,
        &selection.recording_id,
    )
    .ok();
    let owner_session = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned();
    let policy = if let Some(session) = owner_session.as_ref() {
        genesis_adapter::meeting_agent_policy_snapshot(
            &state.genesis,
            &selection.project_id,
            &selection.recording_id,
            Some(&session.owner_scope()),
        )
        .map_err(AppError::Genesis)?
    } else {
        None
    };
    let persisted_revision = policy
        .as_ref()
        .and_then(|row| row.get("meeting_agent_grants.expected_revision"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let selected_state = if let Some(session) = owner_session.as_ref() {
        genesis_adapter::meeting_knowledge_selection_state(
            &state.genesis,
            session,
            &selection.project_id,
            &selection.recording_id,
        )
        .ok()
    } else {
        None
    };
    let (collection_revision, selected_collection_ids) = selected_state.unwrap_or((0, Vec::new()));
    let transcript_ready = transcript
        .as_ref()
        .is_some_and(|snapshot| !snapshot.utterances.is_empty());
    let collections = if let Some(session) = owner_session.as_ref() {
        genesis_adapter::list_meeting_knowledge_collections(
            &state.genesis,
            session,
            &selection.project_id,
            &selection.recording_id,
        )
        .ok()
    } else {
        None
    };
    let readable_selected = !selected_collection_ids.is_empty()
        && collections.as_ref().is_some_and(|items| {
            items.iter().any(|item| {
                item.selected
                    && item.readable
                    && selected_collection_ids.contains(&item.collection_id)
            })
        })
        && collections.as_ref().is_some_and(|items| {
            selected_collection_ids.iter().all(|collection_id| {
                items
                    .iter()
                    .any(|item| item.collection_id == *collection_id && item.readable)
            })
        });
    let owner_ready = owner_session.is_some();
    let mut blockers = Vec::new();
    if !owner_ready {
        blockers.push("LOCAL_OWNER_LOCKED".to_string());
    }
    if !transcript_ready {
        blockers.push("TRANSCRIPT_UNAVAILABLE".to_string());
    }
    if !readable_selected {
        blockers.push(if selected_collection_ids.is_empty() {
            "KNOWLEDGE_SELECTION_REQUIRED".to_string()
        } else {
            "KNOWLEDGE_READ_UNAVAILABLE".to_string()
        });
    }
    let mut mode = "off".to_string();
    let mut agent_state = "stopped".to_string();
    let mut expires_at = None;
    let mut allowed_topics = Vec::new();
    if let Some(row) = policy.as_ref() {
        mode = row
            .get("meeting_agent_grants.mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("off")
            .to_string();
        if let Some(expiry) = row
            .get("meeting_agent_grants.expires_at")
            .and_then(serde_json::Value::as_str)
        {
            expires_at = Some(expiry.to_string());
        }
        if let Some(topics) = row
            .get("meeting_agent_grants.capabilities_json")
            .and_then(|value| value.get("allowedTopics"))
            .and_then(serde_json::Value::as_array)
        {
            allowed_topics = topics
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .collect();
        }
        if row
            .get("meeting_agent_grants.state")
            .and_then(serde_json::Value::as_str)
            == Some("active")
        {
            agent_state = "paused".to_string();
            blockers.push("RESTART_REQUIRES_REENABLE".to_string());
        } else {
            agent_state = match row
                .get("meeting_agent_grants.state")
                .and_then(serde_json::Value::as_str)
            {
                Some("paused") => "paused",
                Some("revoked") => "blocked",
                _ => "stopped",
            }
            .to_string();
        }
    }
    let revision = {
        let mut runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let current = runtime.session(
            &selection.project_id,
            &selection.recording_id,
            persisted_revision,
        );
        if current.enabled_this_process {
            mode = current.mode.clone();
            agent_state = current.state.clone();
            let revision = current.revision.max(persisted_revision);
            expires_at = current.expires_at.clone();
            allowed_topics = current.allowed_topics.clone();
            blockers.retain(|blocker| blocker != "RESTART_REQUIRES_REENABLE");
            revision
        } else {
            persisted_revision
        }
    };
    if expires_at.as_deref().is_some_and(|expiry| {
        chrono::DateTime::parse_from_rfc3339(expiry)
            .map(|parsed| parsed.with_timezone(&chrono::Utc) <= chrono::Utc::now())
            .unwrap_or(true)
    }) {
        mode = "off".to_string();
        agent_state = "expired".to_string();
        blockers.retain(|blocker| blocker != "RESTART_REQUIRES_REENABLE");
        blockers.push("MEETING_AGENT_GRANT_EXPIRED".to_string());
    }
    let status = meeting_intelligence_schema::MeetingAgentStatus {
        project_id: selection.project_id.clone(),
        recording_id: selection.recording_id.clone(),
        revision,
        mode,
        state: agent_state,
        expires_at,
        blockers,
        allowed_topics,
        local_agent: meeting_agent_capability(owner_ready, Some("LOCAL_OWNER_LOCKED")),
        transcript_read: meeting_agent_capability(transcript_ready, Some("TRANSCRIPT_UNAVAILABLE")),
        knowledge_read: meeting_agent_capability(
            readable_selected,
            Some(if selected_collection_ids.is_empty() {
                "KNOWLEDGE_SELECTION_REQUIRED"
            } else {
                "KNOWLEDGE_READ_UNAVAILABLE"
            }),
        ),
        external_join: meeting_intelligence_schema::MeetingAgentCapability {
            readiness: "unavailable".to_string(),
            reason_code: Some("PROVIDER_UNCONFIGURED".to_string()),
        },
        external_media_read: meeting_intelligence_schema::MeetingAgentCapability {
            readiness: "unavailable".to_string(),
            reason_code: Some("PROVIDER_UNCONFIGURED".to_string()),
        },
        external_chat_send: meeting_intelligence_schema::MeetingAgentCapability {
            readiness: "unavailable".to_string(),
            reason_code: Some("PROVIDER_UNCONFIGURED".to_string()),
        },
        external_link_send: meeting_intelligence_schema::MeetingAgentCapability {
            readiness: "unavailable".to_string(),
            reason_code: Some("PROVIDER_UNCONFIGURED".to_string()),
        },
        external_file_upload: meeting_intelligence_schema::MeetingAgentCapability {
            readiness: "unavailable".to_string(),
            reason_code: Some("PROVIDER_UNCONFIGURED".to_string()),
        },
    };
    Ok((status, collection_revision, selected_collection_ids))
}

#[tauri::command]
fn meeting_agent_preflight(
    selection: meeting_intelligence_schema::MeetingAgentSelection,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentPreflight> {
    let (status, collection_revision, selected_collection_ids) =
        meeting_agent_status_snapshot(&state, &selection)?;
    Ok(meeting_intelligence_schema::MeetingAgentPreflight {
        status,
        collection_revision,
        selected_collection_ids,
        local_limits: meeting_intelligence_schema::MeetingAgentLocalLimits {
            max_active_runs: 1,
            max_retrieval_attempts: 3,
            trigger_expiry_ms: 30_000,
            max_runs_per_minute: 2,
            max_runs_per_hour: 20,
        },
    })
}

#[tauri::command]
fn meeting_agent_status(
    selection: meeting_intelligence_schema::MeetingAgentSelection,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    meeting_agent_status_snapshot(&state, &selection).map(|(status, _, _)| status)
}

fn emit_meeting_agent_status(
    app: &tauri::AppHandle,
    status: &meeting_intelligence_schema::MeetingAgentStatus,
) {
    if let Ok(payload) = serde_json::to_value(status) {
        use tauri::Emitter;
        let _ = app.emit("meeting-agent-status", payload);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_meeting_agent_runtime(
    state: &AppState,
    project_id: &str,
    recording_id: &str,
    revision: u64,
    mode: &str,
    agent_state: &str,
    expires_at: Option<String>,
    allowed_topics: Vec<String>,
    enabled: bool,
) {
    let mut runtime = state
        .meeting_intelligence
        .lock()
        .expect("meeting intelligence mutex poisoned");
    let session = runtime.session(project_id, recording_id, revision);
    session.revision = revision;
    session.mode = mode.to_string();
    session.state = agent_state.to_string();
    session.expires_at = expires_at;
    session.allowed_topics = allowed_topics;
    session.enabled_this_process = enabled;
    session.active_run_id = None;
    if !enabled {
        session.drafts.clear();
        session.deliveries.clear();
        session.run_times.clear();
    }
}

struct MeetingAgentRunLease<'a> {
    state: &'a AppState,
    project_id: String,
    recording_id: String,
    run_id: String,
}

impl Drop for MeetingAgentRunLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut runtime) = self.state.meeting_intelligence.lock() {
            if let Some(session) = runtime
                .sessions
                .get_mut(&(self.project_id.clone(), self.recording_id.clone()))
            {
                if session.active_run_id.as_deref() == Some(self.run_id.as_str()) {
                    session.active_run_id = None;
                }
            }
        }
    }
}

#[tauri::command]
fn meeting_agent_start(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentStartRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    if !matches!(request.mode.as_str(), "observe" | "draft") {
        return Err(AppError::InvalidInput(
            "MEETING_AGENT_MODE_INVALID".to_string(),
        ));
    }
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (before, _, selected) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != before.revision {
        return Err(AppError::Genesis(
            "STALE_MEETING_AGENT_REVISION".to_string(),
        ));
    }
    if before.blockers.iter().any(|blocker| {
        blocker != "RESTART_REQUIRES_REENABLE" && blocker != "MEETING_AGENT_GRANT_EXPIRED"
    }) || (request.mode == "draft"
        && (before.local_agent.readiness != "ready"
            || before.transcript_read.readiness != "ready"
            || before.knowledge_read.readiness != "ready"))
    {
        return Err(AppError::Genesis(
            "MEETING_AGENT_PREFLIGHT_BLOCKED".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let (revision, _, _, expires_at) = genesis_adapter::commit_meeting_agent_policy(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        request.expected_revision,
        &request.mode,
        "active",
        &[],
        None,
    )
    .map_err(AppError::Genesis)?;
    let mode = request.mode.clone();
    let agent_state = if mode == "observe" {
        "observing"
    } else {
        "drafting"
    };
    update_meeting_agent_runtime(
        &state,
        &request.project_id,
        &request.recording_id,
        revision,
        &mode,
        agent_state,
        expires_at,
        Vec::new(),
        true,
    );
    let _ = selected;
    let (status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    emit_meeting_agent_status(&app, &status);
    Ok(status)
}

#[tauri::command]
fn meeting_agent_set_policy(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentPolicyRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    if !matches!(request.mode.as_str(), "off" | "observe" | "draft") {
        return Err(AppError::InvalidInput(
            "MEETING_AGENT_MODE_INVALID".to_string(),
        ));
    }
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (before, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != before.revision {
        return Err(AppError::Genesis(
            "STALE_MEETING_AGENT_REVISION".to_string(),
        ));
    }
    if request.mode == "draft"
        && (before.local_agent.readiness != "ready"
            || before.transcript_read.readiness != "ready"
            || before.knowledge_read.readiness != "ready")
    {
        return Err(AppError::Genesis(
            "MEETING_AGENT_PREFLIGHT_BLOCKED".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let next_state = if request.mode == "off" {
        "stopped"
    } else {
        "active"
    };
    let (revision, _, _, expires_at) = genesis_adapter::commit_meeting_agent_policy(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        request.expected_revision,
        &request.mode,
        next_state,
        &request.allowed_topics,
        request.expires_at.as_deref(),
    )
    .map_err(AppError::Genesis)?;
    let agent_state = match request.mode.as_str() {
        "observe" => "observing",
        "draft" => "drafting",
        _ => "stopped",
    };
    let enabled = request.mode != "off";
    update_meeting_agent_runtime(
        &state,
        &request.project_id,
        &request.recording_id,
        revision,
        &request.mode,
        agent_state,
        expires_at,
        request.allowed_topics,
        enabled,
    );
    let (status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    emit_meeting_agent_status(&app, &status);
    Ok(status)
}

#[tauri::command]
fn meeting_agent_pause(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentMutationRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    meeting_agent_set_running_state(app, request, &state, "paused")
}

#[tauri::command]
fn meeting_agent_stop(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentMutationRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    meeting_agent_set_running_state(app, request, &state, "stopped")
}

fn meeting_agent_set_running_state(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentMutationRequest,
    state: &AppState,
    next_state: &str,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (before, _, _) = meeting_agent_status_snapshot(state, &selection)?;
    if request.expected_revision != before.revision {
        return Err(AppError::Genesis(
            "STALE_MEETING_AGENT_REVISION".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let mode = if next_state == "stopped" {
        "off".to_string()
    } else {
        before.mode.clone()
    };
    let topics = before.allowed_topics.clone();
    let expiry = before.expires_at.clone();
    let (revision, _, _, next_expiry) = genesis_adapter::commit_meeting_agent_policy(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        request.expected_revision,
        &mode,
        next_state,
        &topics,
        expiry.as_deref(),
    )
    .map_err(AppError::Genesis)?;
    update_meeting_agent_runtime(
        state,
        &request.project_id,
        &request.recording_id,
        revision,
        &mode,
        next_state,
        next_expiry,
        topics,
        next_state == "paused",
    );
    let (status, _, _) = meeting_agent_status_snapshot(state, &selection)?;
    emit_meeting_agent_status(&app, &status);
    Ok(status)
}

#[tauri::command]
fn meeting_agent_ask(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentAskRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentPrivateDraft> {
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (before, _, selected) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != before.revision {
        return Err(AppError::Genesis(
            "STALE_MEETING_AGENT_REVISION".to_string(),
        ));
    }
    if before.mode != "draft"
        || !matches!(before.state.as_str(), "drafting" | "observing")
        || before.local_agent.readiness != "ready"
        || before.transcript_read.readiness != "ready"
        || before.knowledge_read.readiness != "ready"
    {
        return Err(AppError::Genesis(
            "MEETING_AGENT_DRAFT_NOT_AUTHORIZED".to_string(),
        ));
    }
    if request.question.trim().is_empty()
        || request.question.chars().count() > 2_000
        || request.transcript_cursor < 0
        || selected.is_empty()
        || request.collection_ids.len() != selected.len()
        || request
            .collection_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != request.collection_ids.len()
        || selected
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != request.collection_ids.iter().cloned().collect()
    {
        return Err(AppError::InvalidInput(
            "MEETING_AGENT_ASK_INVALID".to_string(),
        ));
    }
    if before.expires_at.as_deref().is_some_and(|expiry| {
        chrono::DateTime::parse_from_rfc3339(expiry)
            .map(|parsed| parsed.with_timezone(&chrono::Utc) <= chrono::Utc::now())
            .unwrap_or(true)
    }) {
        return Err(AppError::Genesis("MEETING_AGENT_GRANT_EXPIRED".to_string()));
    }
    let transcript = genesis_adapter::meeting_transcript_snapshot(
        &state.genesis,
        &request.project_id,
        &request.recording_id,
    )
    .map_err(AppError::Genesis)?;
    if transcript.high_watermark != request.transcript_cursor {
        return Err(AppError::Genesis(
            "MEETING_TRANSCRIPT_CURSOR_STALE".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let account_guard = auth_session::account_begin_operation().map_err(AppError::Genesis)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    {
        let mut runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let session = runtime.session(&request.project_id, &request.recording_id, before.revision);
        if !session.enabled_this_process || session.mode != "draft" || session.state == "paused" {
            return Err(AppError::Genesis(
                "MEETING_AGENT_RESTART_REQUIRES_REENABLE".to_string(),
            ));
        }
        if session.active_run_id.is_some() {
            return Err(AppError::Genesis(
                "MEETING_AGENT_RUN_ALREADY_ACTIVE".to_string(),
            ));
        }
        let now = std::time::Instant::now();
        session.run_times.retain(|started| {
            now.saturating_duration_since(*started) < std::time::Duration::from_secs(3600)
        });
        let runs_last_minute = session
            .run_times
            .iter()
            .filter(|started| {
                now.saturating_duration_since(**started) < std::time::Duration::from_secs(60)
            })
            .count();
        if runs_last_minute >= 2 || session.run_times.len() >= 20 {
            return Err(AppError::Genesis(
                "MEETING_AGENT_LOCAL_RATE_LIMIT".to_string(),
            ));
        }
        session.run_times.push(now);
        session.active_run_id = Some(run_id.clone());
    }
    let _run_lease = MeetingAgentRunLease {
        state: state.inner(),
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
        run_id: run_id.clone(),
    };
    let search = genesis_adapter::search_selected_meeting_knowledge(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.collection_ids,
        &request.question,
    )
    .map_err(AppError::Genesis)?;
    if search.evidence.is_empty() {
        use tauri::Emitter;
        let _ = app.emit(
            "meeting-agent-policy-blocked",
            serde_json::json!({
                "projectId": request.project_id,
                "recordingId": request.recording_id,
                "code": "MEETING_KNOWLEDGE_NO_EVIDENCE",
                "reason": "No selected source contains a matching excerpt.",
                "sourceCursor": request.transcript_cursor,
            }),
        );
        let _ = genesis_adapter::commit_meeting_agent_run_event(
            &state.genesis,
            &owner,
            &request.project_id,
            &request.recording_id,
            &request.request_id,
            "draft_blocked",
            request.transcript_cursor,
            serde_json::json!({"evidence": []}),
        );
        return Err(AppError::Genesis(
            "MEETING_KNOWLEDGE_NO_EVIDENCE".to_string(),
        ));
    }
    let mut draft_text = String::from("จากเอกสารที่เลือก พบข้อความที่เกี่ยวข้อง:\n\n");
    let mut citations = Vec::new();
    let mut evidence_refs = Vec::new();
    for hit in search.evidence.iter().take(8) {
        let excerpt = hit.excerpt.trim();
        if excerpt.is_empty() {
            continue;
        }
        draft_text.push('“');
        draft_text.push_str(excerpt);
        draft_text.push_str("”\n\n");
        let locator = match &hit.citation.locator {
            crate::meeting_knowledge::CitationLocator::TextSpan {
                start_line,
                end_line,
                start_char,
                end_char,
            } => format!("บรรทัด {start_line}-{end_line} · อักขระ {start_char}-{end_char}"),
            crate::meeting_knowledge::CitationLocator::PdfPage {
                page_number,
                start_char,
                end_char,
            } => format!("หน้า {page_number} · อักขระ {start_char}-{end_char}"),
            crate::meeting_knowledge::CitationLocator::SpreadsheetCellRange {
                sheet_ref,
                range,
            } => {
                format!("ชีต {sheet_ref} · {range}")
            }
            crate::meeting_knowledge::CitationLocator::TranscriptRange {
                recording_id,
                start_ms,
                end_ms,
                ..
            } => format!("{recording_id} · {start_ms}-{end_ms} ms"),
        };
        citations.push(meeting_intelligence_schema::MeetingAgentCitation {
            document_id: hit.citation.document_id.clone(),
            version_id: hit.citation.document_version_id.clone(),
            locator: locator.clone(),
            label: format!(
                "เอกสาร {}",
                hit.citation.document_id.chars().take(8).collect::<String>()
            ),
        });
        evidence_refs.push(serde_json::json!({
            "collectionId": hit.citation.collection_id,
            "documentId": hit.citation.document_id,
            "versionId": hit.citation.document_version_id,
            "documentVersionNumber": hit.citation.document_version_number,
            "sourceVersion": hit.citation.source_version,
            "contentSha256": hit.citation.content_sha256,
            "aclRevision": hit.citation.acl_revision,
            "readGrantId": hit.citation.read_grant_id,
            "locator": locator,
            "locatorData": serde_json::to_value(&hit.citation.locator).unwrap_or(serde_json::Value::Null),
        }));
    }
    if citations.is_empty() || draft_text.chars().count() > 12_000 {
        return Err(AppError::Genesis(
            "MEETING_AGENT_DRAFT_SIZE_INVALID".to_string(),
        ));
    }
    let (latest_status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    if latest_status.revision != before.revision
        || latest_status.mode != "draft"
        || latest_status.state == "paused"
    {
        return Err(AppError::Genesis("MEETING_AGENT_RUN_CANCELLED".to_string()));
    }
    let latest_transcript = genesis_adapter::meeting_transcript_snapshot(
        &state.genesis,
        &request.project_id,
        &request.recording_id,
    )
    .map_err(AppError::Genesis)?;
    if latest_transcript.high_watermark != request.transcript_cursor {
        return Err(AppError::Genesis(
            "MEETING_TRANSCRIPT_CURSOR_STALE".to_string(),
        ));
    }
    {
        let runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let still_active = runtime
            .sessions
            .get(&(request.project_id.clone(), request.recording_id.clone()))
            .is_some_and(|session| session.active_run_id.as_deref() == Some(run_id.as_str()));
        if !still_active {
            return Err(AppError::Genesis("MEETING_AGENT_RUN_CANCELLED".to_string()));
        }
    }
    let draft_hash = format!("{:x}", sha2::Sha256::digest(request.request_id.as_bytes()));
    let draft_id = format!("draft-{}", &draft_hash[..32]);
    let timestamp = chrono::Utc::now();
    let draft = meeting_intelligence_schema::MeetingAgentPrivateDraft {
        draft_id: draft_id.clone(),
        revision: 1,
        text: draft_text.clone(),
        citations,
        based_on_transcript_cursor: request.transcript_cursor,
        expires_at: (timestamp + chrono::Duration::minutes(10)).to_rfc3339(),
        state: "private".to_string(),
    };
    let revisions = transcript
        .utterances
        .iter()
        .filter_map(|row| {
            Some(serde_json::json!({
                "utteranceId": row.get("utterance_id")?,
                "revisionId": row.get("revision_id"),
                "revision": row.get("revision")?,
            }))
        })
        .collect::<Vec<_>>();
    genesis_adapter::persist_private_meeting_agent_draft(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        before.revision,
        &request.collection_ids,
        &request.request_id,
        &draft_id,
        &draft_text,
        &draft.expires_at,
        request.transcript_cursor,
        serde_json::json!(revisions),
        serde_json::json!(evidence_refs),
    )
    .map_err(AppError::Genesis)?;
    let current_owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned");
    let active_owner = current_owner
        .as_ref()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let _publication_fence = active_owner
        .fence_sensitive_publication(&state.genesis, &owner)
        .map_err(AppError::Genesis)?;
    account_guard
        .with_account_lifecycle_fence(owner.account_lifecycle_witness(), || {
            {
                let mut runtime = state
                    .meeting_intelligence
                    .lock()
                    .expect("meeting intelligence mutex poisoned");
                let session =
                    runtime.session(&request.project_id, &request.recording_id, before.revision);
                if !session.enabled_this_process
                    || session.mode != "draft"
                    || session.state == "paused"
                    || session.active_run_id.as_deref() != Some(run_id.as_str())
                {
                    return Err("MEETING_AGENT_RUN_CANCELLED".to_string());
                }
                session.state = "drafting".to_string();
                session.active_run_id = None;
                session.drafts.insert(draft_id, draft.clone());
            }
            use tauri::Emitter;
            let _ = app.emit(
                "meeting-agent-draft",
                serde_json::json!({
                    "projectId": request.project_id,
                    "recordingId": request.recording_id,
                    "draft": draft,
                }),
            );
            Ok(draft)
        })
        .map_err(AppError::Genesis)
}

#[tauri::command]
fn meeting_agent_preview_delivery(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentDeliveryPreviewRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentDeliveryPreview> {
    if request.scope != "local_preview_only" {
        return Err(AppError::InvalidInput(
            "MEETING_DELIVERY_SCOPE_INVALID".to_string(),
        ));
    }
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != status.revision
        || status.mode != "draft"
        || !status.blockers.is_empty()
    {
        return Err(AppError::Genesis(
            "MEETING_DELIVERY_PREFLIGHT_BLOCKED".to_string(),
        ));
    }
    let (payload, draft_expires_at) = {
        let runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let session = runtime
            .sessions
            .get(&(request.project_id.clone(), request.recording_id.clone()))
            .ok_or_else(|| AppError::Genesis("MEETING_AGENT_DRAFT_UNAVAILABLE".to_string()))?;
        if !session.enabled_this_process {
            return Err(AppError::Genesis(
                "MEETING_AGENT_RESTART_REQUIRES_REENABLE".to_string(),
            ));
        }
        let draft = session
            .drafts
            .get(&request.draft_id)
            .filter(|draft| draft.revision == request.draft_revision && draft.state == "private")
            .filter(|draft| {
                chrono::DateTime::parse_from_rfc3339(&draft.expires_at)
                    .map(|expiry| expiry.with_timezone(&chrono::Utc) > chrono::Utc::now())
                    .unwrap_or(false)
            })
            .ok_or_else(|| AppError::Genesis("MEETING_AGENT_DRAFT_STALE".to_string()))?;
        (
            serde_json::json!({"text": draft.text, "citations": draft.citations}),
            draft.expires_at.clone(),
        )
    };
    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|_| AppError::Genesis("MEETING_DELIVERY_PAYLOAD_INVALID".to_string()))?;
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let preview = genesis_adapter::persist_local_meeting_delivery_preview(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        status.revision,
        &request.draft_id,
        request.draft_revision,
        &draft_expires_at,
        &payload_bytes,
    )
    .map_err(AppError::Genesis)?;
    {
        let mut runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let session = runtime.session(&request.project_id, &request.recording_id, status.revision);
        if !session.enabled_this_process
            || session.mode != "draft"
            || session.state == "paused"
            || session.revision != status.revision
        {
            return Err(AppError::Genesis(
                "MEETING_DELIVERY_PREVIEW_CANCELLED".to_string(),
            ));
        }
        let draft = session
            .drafts
            .get(&request.draft_id)
            .filter(|draft| draft.revision == request.draft_revision && draft.state == "private")
            .filter(|draft| {
                chrono::DateTime::parse_from_rfc3339(&draft.expires_at)
                    .map(|expiry| expiry.with_timezone(&chrono::Utc) > chrono::Utc::now())
                    .unwrap_or(false)
            })
            .ok_or_else(|| AppError::Genesis("MEETING_AGENT_DRAFT_STALE".to_string()))?;
        let _ = draft;
        session.deliveries.insert(
            preview.intent_id.clone(),
            meeting_intelligence_runtime::RuntimeDelivery {
                preview: preview.clone(),
                draft_id: request.draft_id,
                draft_revision: request.draft_revision,
                expires_at: std::time::Instant::now() + std::time::Duration::from_secs(300),
            },
        );
    }
    use tauri::Emitter;
    let _ = app.emit(
        "meeting-delivery-status",
        serde_json::json!({
            "projectId": request.project_id,
            "recordingId": request.recording_id,
            "intentId": preview.intent_id,
            "state": preview.state,
            "revision": status.revision,
            "externalDispatchAvailable": false,
        }),
    );
    Ok(preview)
}

#[tauri::command]
fn meeting_agent_approve_delivery(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentDeliveryApprovalRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentDeliveryPreview> {
    if request.scope != "local_preview_only" {
        return Err(AppError::InvalidInput(
            "MEETING_DELIVERY_SCOPE_INVALID".to_string(),
        ));
    }
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != status.revision || status.mode != "draft" {
        return Err(AppError::Genesis(
            "MEETING_DELIVERY_APPROVAL_STALE".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let preview = {
        let runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let session = runtime
            .sessions
            .get(&(request.project_id.clone(), request.recording_id.clone()))
            .ok_or_else(|| AppError::Genesis("MEETING_DELIVERY_PREVIEW_UNAVAILABLE".to_string()))?;
        if !session.enabled_this_process
            || session.mode != "draft"
            || session.state == "paused"
            || session.revision != status.revision
        {
            return Err(AppError::Genesis(
                "MEETING_DELIVERY_APPROVAL_STALE".to_string(),
            ));
        }
        let delivery = session
            .deliveries
            .get(&request.intent_id)
            .filter(|delivery| delivery.preview.payload_hash == request.approved_payload_hash)
            .filter(|delivery| delivery.preview.state == "awaiting_approval")
            .filter(|delivery| delivery.expires_at > std::time::Instant::now())
            .ok_or_else(|| AppError::Genesis("MEETING_DELIVERY_APPROVAL_STALE".to_string()))?;
        let draft = session
            .drafts
            .get(&delivery.draft_id)
            .filter(|draft| draft.revision == delivery.draft_revision && draft.state == "private")
            .filter(|draft| {
                chrono::DateTime::parse_from_rfc3339(&draft.expires_at)
                    .map(|expiry| expiry.with_timezone(&chrono::Utc) > chrono::Utc::now())
                    .unwrap_or(false)
            })
            .ok_or_else(|| AppError::Genesis("MEETING_DELIVERY_DRAFT_CHANGED".to_string()))?;
        let _ = draft;
        delivery.preview.clone()
    };
    let persisted_approved = genesis_adapter::approve_local_meeting_delivery_preview(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        status.revision,
        &preview.intent_id,
        &request.approved_payload_hash,
    )
    .map_err(AppError::Genesis)?;
    let runtime_approved = {
        let mut runtime = state
            .meeting_intelligence
            .lock()
            .expect("meeting intelligence mutex poisoned");
        let session = runtime.session(&request.project_id, &request.recording_id, status.revision);
        if !session.enabled_this_process
            || session.mode != "draft"
            || session.state == "paused"
            || session.revision != status.revision
            || session.expires_at.as_deref().is_some_and(|expiry| {
                chrono::DateTime::parse_from_rfc3339(expiry)
                    .map(|parsed| parsed.with_timezone(&chrono::Utc) <= chrono::Utc::now())
                    .unwrap_or(true)
            })
        {
            return Err(AppError::Genesis(
                "MEETING_DELIVERY_APPROVAL_STALE".to_string(),
            ));
        }
        let delivery = session
            .deliveries
            .get_mut(&request.intent_id)
            .ok_or_else(|| AppError::Genesis("MEETING_DELIVERY_PREVIEW_UNAVAILABLE".to_string()))?;
        if delivery.preview.payload_hash != request.approved_payload_hash
            || delivery.expires_at <= std::time::Instant::now()
        {
            return Err(AppError::Genesis(
                "MEETING_DELIVERY_APPROVAL_STALE".to_string(),
            ));
        }
        delivery.preview.state = "approved_local_only".to_string();
        delivery.preview.clone()
    };
    if persisted_approved.intent_id != runtime_approved.intent_id
        || persisted_approved.payload_hash != runtime_approved.payload_hash
        || persisted_approved.state != runtime_approved.state
    {
        return Err(AppError::Genesis(
            "MEETING_DELIVERY_APPROVAL_STATE_MISMATCH".to_string(),
        ));
    }
    use tauri::Emitter;
    let _ = app.emit(
        "meeting-delivery-status",
        serde_json::json!({
            "projectId": request.project_id,
            "recordingId": request.recording_id,
            "intentId": persisted_approved.intent_id,
            "state": persisted_approved.state,
            "revision": status.revision,
            "externalDispatchAvailable": false,
        }),
    );
    Ok(persisted_approved)
}

#[tauri::command]
fn meeting_agent_revoke(
    app: tauri::AppHandle,
    request: meeting_intelligence_schema::MeetingAgentRevokeRequest,
    state: State<'_, AppState>,
) -> AppResult<meeting_intelligence_schema::MeetingAgentStatus> {
    if request.reason.trim().is_empty() || request.reason.len() > 256 {
        return Err(AppError::InvalidInput(
            "MEETING_AGENT_REVOKE_REASON_INVALID".to_string(),
        ));
    }
    let selection = meeting_intelligence_schema::MeetingAgentSelection {
        project_id: request.project_id.clone(),
        recording_id: request.recording_id.clone(),
    };
    let (before, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    if request.expected_revision != before.revision {
        return Err(AppError::Genesis(
            "STALE_MEETING_AGENT_REVISION".to_string(),
        ));
    }
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    let (revision, _, _, _) = genesis_adapter::commit_meeting_agent_policy(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        request.expected_revision,
        "off",
        "revoked",
        &[],
        None,
    )
    .map_err(AppError::Genesis)?;
    genesis_adapter::commit_meeting_agent_run_event(
        &state.genesis,
        &owner,
        &request.project_id,
        &request.recording_id,
        &request.request_id,
        "revoked",
        -1,
        serde_json::json!({"reason": request.reason}),
    )
    .map_err(AppError::Genesis)?;
    update_meeting_agent_runtime(
        &state,
        &request.project_id,
        &request.recording_id,
        revision,
        "off",
        "blocked",
        None,
        Vec::new(),
        false,
    );
    let (status, _, _) = meeting_agent_status_snapshot(&state, &selection)?;
    emit_meeting_agent_status(&app, &status);
    Ok(status)
}

#[tauri::command]
fn meeting_agent_history(
    selection: meeting_intelligence_schema::MeetingAgentSelection,
    limit: u32,
    state: State<'_, AppState>,
) -> AppResult<Vec<meeting_intelligence_schema::MeetingAgentHistoryEntry>> {
    let owner = state
        .meeting_owner_session
        .lock()
        .expect("meeting owner session mutex poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Genesis("LOCAL_OWNER_LOCKED".to_string()))?;
    genesis_adapter::list_meeting_agent_runs(
        &state.genesis,
        &owner,
        &selection.project_id,
        &selection.recording_id,
        limit,
    )
    .map_err(AppError::Genesis)
}

#[tauri::command]
fn correct_transcript_segment(
    project_id: String,
    recording_id: String,
    segment_id: String,
    corrected_text: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    correct_transcript_segment_in_storage(
        &state.genesis,
        &project_id,
        &recording_id,
        &segment_id,
        &corrected_text,
    )
}

/// Applies a user correction to one recording-scoped transcript segment.
///
/// The current segment remains the source used by later review/export paths;
/// the accepted refinement proposal preserves the before/after text and the
/// audit event ties that correction to the exact recording and segment. All
/// three writes share one Genesis transaction so a visible correction cannot
/// exist without its local audit trail.
fn correct_transcript_segment_in_storage(
    genesis: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    segment_id: &str,
    corrected_text: &str,
) -> AppResult<()> {
    let corrected_text = corrected_text.trim();
    if corrected_text.is_empty() {
        return Err(AppError::InvalidInput(
            "ข้อความ transcript ต้องไม่ว่าง".to_string(),
        ));
    }
    let revision_id = Uuid::new_v4().to_string();
    let attempt = genesis_adapter::begin_meeting_commit(
        genesis,
        &format!("meeting-correction::{revision_id}"),
        &now(),
    )
    .map_err(AppError::Genesis)?;

    let row = genesis_adapter::query(
        genesis,
        "transcript_segments",
        &[
            "id",
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "text",
            "confidence",
            "created_at",
        ],
        vec![
            genesis_adapter::eq(
                "transcript_segments",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_segments",
                "recording_id",
                serde_json::json!(recording_id),
            ),
            genesis_adapter::eq("transcript_segments", "id", serde_json::json!(segment_id)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| {
        AppError::InvalidInput(format!(
            "ไม่พบ transcript segment {segment_id} ในการบันทึก {recording_id}"
        ))
    })?;

    let original_text =
        genesis_adapter::string(&row, "transcript_segments.text").map_err(AppError::Genesis)?;
    if original_text == corrected_text {
        return Ok(());
    }

    if correct_v2_utterance_in_storage(
        genesis,
        project_id,
        recording_id,
        segment_id,
        corrected_text,
        &revision_id,
        &attempt,
    )? {
        return Ok(());
    }

    let timestamp = now();
    let proposal_id = Uuid::new_v4().to_string();
    let policy = "manual_user_correction";
    let mutations = vec![
        genesis_adapter::upsert(
            "transcript_segments",
            serde_json::json!({
                "id": segment_id,
                "project_id": project_id,
                "recording_id": recording_id,
                "speaker_id": row.get("transcript_segments.speaker_id").cloned().unwrap_or(serde_json::Value::Null),
                "start_ms": genesis_adapter::integer(&row, "transcript_segments.start_ms").map_err(AppError::Genesis)?,
                "end_ms": genesis_adapter::integer(&row, "transcript_segments.end_ms").map_err(AppError::Genesis)?,
                "text": corrected_text,
                "confidence": row.get("transcript_segments.confidence").cloned().unwrap_or(serde_json::Value::Null),
                "created_at": genesis_adapter::string(&row, "transcript_segments.created_at").map_err(AppError::Genesis)?,
                "updated_at": timestamp.clone(),
            }),
        ),
        genesis_adapter::upsert(
            "transcript_refinement_proposals",
            serde_json::json!({
                "id": proposal_id.clone(),
                "project_id": project_id,
                "transcript_segment_id": segment_id,
                "original_text": original_text,
                "proposed_text": corrected_text,
                "policy": policy,
                "model_run_id": null,
                "status": "accepted",
                "reviewed_at": timestamp.clone(),
                "created_at": timestamp.clone(),
                "updated_at": timestamp.clone(),
            }),
        ),
        genesis_adapter::upsert(
            "audit_events",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": project_id,
                "event_type": "transcript.segment.corrected",
                "actor": "user",
                "payload_json": {
                    "recordingId": recording_id,
                    "segmentId": segment_id,
                    "proposalId": proposal_id,
                    "policy": policy,
                },
                "created_at": timestamp,
            }),
        ),
    ];

    genesis_adapter::commit_rows(genesis, mutations).map_err(AppError::Genesis)
}

fn correct_v2_utterance_in_storage(
    genesis: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    utterance_id: &str,
    corrected_text: &str,
    revision_id: &str,
    attempt: &meeting_intelligence_schema::MeetingCommitAttempt,
) -> AppResult<bool> {
    let projection = genesis_adapter::query(
        genesis,
        "transcript_projection",
        &["revision_id"],
        vec![
            genesis_adapter::eq(
                "transcript_projection",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_projection",
                "recording_id",
                serde_json::json!(recording_id),
            ),
            genesis_adapter::eq(
                "transcript_projection",
                "utterance_id",
                serde_json::json!(utterance_id),
            ),
        ],
        1,
    )
    .map_err(AppError::Genesis)?;
    let Some(projection) = projection.into_iter().next() else {
        return Ok(false);
    };
    let current_revision_id =
        genesis_adapter::string(&projection, "transcript_projection.revision_id")
            .map_err(AppError::Genesis)?;
    let current = genesis_adapter::query(
        genesis,
        "transcript_revisions",
        &[
            "id",
            "revision",
            "raw_text",
            "effective_text",
            "language",
            "confidence",
            "start_ms",
            "end_ms",
            "model_run_id",
        ],
        vec![genesis_adapter::eq(
            "transcript_revisions",
            "id",
            serde_json::json!(&current_revision_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::Genesis("current transcript revision is missing".to_string()))?;
    let event = genesis_adapter::query(
        genesis,
        "transcript_event_log",
        &["payload_json"],
        vec![genesis_adapter::eq(
            "transcript_event_log",
            "revision_id",
            serde_json::json!(&current_revision_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::Genesis("current transcript revision event is missing".to_string()))?;
    let payload = event
        .get("transcript_event_log.payload_json")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let scope_value = payload.get("scope").cloned().ok_or_else(|| {
        AppError::Genesis("current transcript revision scope is missing".to_string())
    })?;
    let scope: meeting_intelligence_schema::MeetingScope = serde_json::from_value(scope_value)
        .map_err(|_| {
            AppError::Genesis("current transcript revision scope is invalid".to_string())
        })?;
    if scope.project_id != project_id || scope.recording_id != recording_id {
        return Err(AppError::Genesis(
            "current transcript revision scope does not match selection".to_string(),
        ));
    }
    let current_revision = genesis_adapter::integer(&current, "transcript_revisions.revision")
        .map_err(AppError::Genesis)?;
    let next_revision = current_revision
        .checked_add(1)
        .ok_or_else(|| AppError::Genesis("transcript revision is exhausted".to_string()))?;
    let revision = meeting_intelligence_schema::TranscriptRevisionInput {
        id: revision_id.to_string(),
        utterance_id: utterance_id.to_string(),
        revision: next_revision,
        supersedes_revision: Some(current_revision),
        expected_revision: Some(current_revision),
        origin: meeting_intelligence_schema::TranscriptOrigin::Human,
        raw_text: genesis_adapter::string(&current, "transcript_revisions.raw_text")
            .map_err(AppError::Genesis)?,
        effective_text: corrected_text.to_string(),
        language: current
            .get("transcript_revisions.language")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        confidence: current
            .get("transcript_revisions.confidence")
            .and_then(serde_json::Value::as_f64),
        start_ms: genesis_adapter::integer(&current, "transcript_revisions.start_ms")
            .map_err(AppError::Genesis)?,
        end_ms: genesis_adapter::integer(&current, "transcript_revisions.end_ms")
            .map_err(AppError::Genesis)?,
        model_run_id: current
            .get("transcript_revisions.model_run_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        review_state: "reviewed".to_string(),
    };
    let request = meeting_intelligence_schema::MeetingRevisionRequest { scope, revision };
    genesis_adapter::revise_meeting_transcript(genesis, attempt, &request)
        .map_err(AppError::Genesis)?;
    Ok(true)
}

/// The body of [`list_transcript_segments`], taking the storage handle rather
/// than Tauri state so the read is testable without an app.
fn transcript_view(
    genesis: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> AppResult<TranscriptView> {
    // Resolve speaker display names once per call: query the project's
    // speakers (capped like every other query against this engine) and build
    // an id -> display_name map, rather than a lookup per segment.
    let speaker_rows = genesis_adapter::query(
        genesis,
        "speakers",
        &["id", "display_name"],
        vec![genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        genesis_adapter::ROW_CAP,
    )
    .map_err(AppError::Genesis)?;
    let speaker_names: std::collections::HashMap<String, String> = speaker_rows
        .into_iter()
        .filter_map(|row| {
            Some((
                row.get("speakers.id")?.as_str()?.to_string(),
                row.get("speakers.display_name")?.as_str()?.to_string(),
            ))
        })
        .collect();

    // Resolve the selected recording inside the selected project before
    // reading segments. The id is globally unique, but checking ownership
    // here keeps the bridge recording-scoped at the project boundary too.
    let recording_exists = genesis_adapter::query(
        genesis,
        "recordings",
        &["id"],
        vec![
            genesis_adapter::eq("recordings", "project_id", serde_json::json!(project_id)),
            genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?;
    if recording_exists.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "ไม่พบการบันทึก {recording_id} ในโปรเจกต์ {project_id}"
        )));
    }

    let mut segments: Vec<TranscriptSegment> = Vec::new();

    let rows = genesis_adapter::query_all(
        genesis,
        "transcript_segments",
        &[
            "id",
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "text",
            "confidence",
            "created_at",
        ],
        vec![
            genesis_adapter::eq(
                "transcript_segments",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_segments",
                "recording_id",
                serde_json::json!(recording_id),
            ),
        ],
    )
    .map_err(AppError::Genesis)?;
    for row in rows {
        let speaker_id = row
            .get("transcript_segments.speaker_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let speaker_name = speaker_id
            .as_ref()
            .and_then(|id| speaker_names.get(id).cloned());
        segments.push(TranscriptSegment {
            id: genesis_adapter::string(&row, "transcript_segments.id")
                .map_err(AppError::Genesis)?,
            project_id: genesis_adapter::string(&row, "transcript_segments.project_id")
                .map_err(AppError::Genesis)?,
            recording_id: genesis_adapter::string(&row, "transcript_segments.recording_id")
                .map_err(AppError::Genesis)?,
            speaker_id,
            speaker_name,
            start_ms: genesis_adapter::integer(&row, "transcript_segments.start_ms")
                .map_err(AppError::Genesis)?,
            end_ms: genesis_adapter::integer(&row, "transcript_segments.end_ms")
                .map_err(AppError::Genesis)?,
            text: genesis_adapter::string(&row, "transcript_segments.text")
                .map_err(AppError::Genesis)?,
            confidence: row
                .get("transcript_segments.confidence")
                .and_then(serde_json::Value::as_f64),
            created_at: genesis_adapter::string(&row, "transcript_segments.created_at")
                .map_err(AppError::Genesis)?,
        });
    }

    segments.sort_by_key(|segment| segment.start_ms);
    Ok(TranscriptView {
        // `query_all` pages past the engine's single-read ceiling, so the
        // read is always whole now. The fields stay for frontend contract
        // stability; they are truthfully never set.
        capped: false,
        cap: genesis_adapter::ROW_CAP,
        capped_recording_ids: Vec::new(),
        segments,
    })
}

pub(crate) fn set_job_status(
    storage: &genesis_block_native::Storage,
    job_id: &str,
    status: &str,
    progress: Option<i64>,
    error_message: Option<&str>,
) -> AppResult<()> {
    let timestamp = now();
    let row = genesis_adapter::query(
        storage,
        "jobs",
        &[
            "project_id",
            "type",
            "status",
            "progress",
            "input_refs_json",
            "output_refs_json",
            "provider_id",
            "error_code",
            "error_message",
            "attempt_no",
            "started_at",
            "finished_at",
            "created_at",
        ],
        vec![genesis_adapter::eq("jobs", "id", serde_json::json!(job_id))],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("job not found".to_string()))?;
    let optional = |key: &str| row.get(key).cloned().unwrap_or(serde_json::Value::Null);
    let started_at = if status == "running" && optional("jobs.started_at").is_null() {
        serde_json::Value::String(timestamp.clone())
    } else {
        optional("jobs.started_at")
    };
    let finished_at = if matches!(status, "completed" | "failed") {
        serde_json::Value::String(timestamp.clone())
    } else {
        optional("jobs.finished_at")
    };
    genesis_adapter::commit_rows(storage, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id":job_id,"project_id":genesis_adapter::string(&row,"jobs.project_id").map_err(AppError::Genesis)?,"type":genesis_adapter::string(&row,"jobs.type").map_err(AppError::Genesis)?,"status":status,"progress":progress.unwrap_or(genesis_adapter::integer(&row,"jobs.progress").map_err(AppError::Genesis)?),"input_refs_json":optional("jobs.input_refs_json"),"output_refs_json":optional("jobs.output_refs_json"),"provider_id":optional("jobs.provider_id"),"error_code":optional("jobs.error_code"),"error_message":error_message.map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),"attempt_no":genesis_adapter::integer(&row,"jobs.attempt_no").map_err(AppError::Genesis)?,"started_at":started_at,"finished_at":finished_at,"created_at":genesis_adapter::string(&row,"jobs.created_at").map_err(AppError::Genesis)?,"updated_at":timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id":Uuid::new_v4().to_string(),"job_id":job_id,"status":status,"message":error_message.unwrap_or(status),"created_at":timestamp})),
    ]).map_err(AppError::Genesis)?;
    Ok(())
}

/// Re-runs the interrupted-recording scan. Cheap by construction — directory
/// listings and row comparisons, no hashing — so the UI can call it freely.
#[tauri::command]
fn recovery_scan(state: State<'_, AppState>) -> AppResult<recovery::RecoveryReport> {
    recovery::scan(&state.genesis).map_err(AppError::Genesis)
}

/// What recovering one recording achieved: what was adopted back into the
/// ledger, and what text was produced for it.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveredRecording {
    adopted: recovery::RecoveryOutcome,
    transcript: live_meeting::GapFillOutcome,
}

/// Adopts a recording's orphaned audio into the ledger, then transcribes
/// whatever text it is still missing.
///
/// Adoption alone leaves a recovered recording showing chunks with no words —
/// audio that is safe and unreadable at the same time — so the two run as one
/// user action. Transcription failure does not fail the recovery: the audio is
/// already durable, and a recording with a partial transcript is a truthful
/// state as long as it is reported.
#[tauri::command]
async fn recovery_recover(
    recording_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<RecoveredRecording> {
    let storage = Arc::clone(&state.genesis);
    let runtime = state.whisper_runtime_clone();
    tauri::async_runtime::spawn_blocking(move || {
        let adopted =
            recovery::recover_recording(&storage, &recording_id).map_err(AppError::Genesis)?;
        let project_id = genesis_adapter::capture(&storage, &recording_id)
            .map_err(AppError::Genesis)?
            .project_id;
        let transcript = live_meeting::fill_transcript_gaps(
            &app,
            &storage,
            &runtime,
            &project_id,
            &recording_id,
        );
        Ok(RecoveredRecording {
            adopted,
            transcript,
        })
    })
    .await
    .map_err(|_| AppError::InvalidInput("recovery task did not complete".to_string()))?
}

/// A project's own storage root, which is where its audio belongs.
fn project_storage_path(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> AppResult<PathBuf> {
    let rows = genesis_adapter::query(
        storage,
        "projects",
        &["storage_path"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?;
    let path = rows
        .first()
        .and_then(|row| row.get("projects.storage_path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบที่เก็บของโปรเจกต์ {project_id}")))?;
    Ok(PathBuf::from(path))
}

/// Re-reads every audio chunk of a project and reports what is still present
/// and unchanged. Chunks found under the project's current root with a
/// matching digest have their rows repaired, so moving a project folder stops
/// being silent data loss.
#[tauri::command]
fn audio_integrity_check(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<audio_custody::AudioIntegrityReport> {
    let report = audio_custody::verify_project_audio(&state.genesis, &project_id)
        .map_err(|error| AppError::InvalidInput(error.to_string()))?;
    // A failed integrity check is a finding about the user's data, not a
    // transient UI state: record it so it survives the window closing.
    if !report.is_clean() {
        let timestamp = now();
        let _ = genesis_adapter::commit_rows(
            &state.genesis,
            vec![genesis_adapter::upsert(
                "audit_events",
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "project_id": project_id,
                    "event_type": "audio_integrity.incomplete",
                    "actor": "user",
                    "payload_json": {
                        "checked": report.checked,
                        "missing": report.missing,
                        "modified": report.modified,
                    },
                    "created_at": timestamp,
                }),
            )],
        );
    }
    Ok(report)
}

/// Imports an audio/video file, transcribes it locally with faster-whisper,
/// and writes the resulting segments into `transcript_segments`. Runs the
/// Python worker on a background thread so the UI can keep polling job
/// progress via `list_jobs` instead of blocking on the whole file.
#[tauri::command]
fn import_and_transcribe(
    file_path: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Job> {
    let default_name = PathBuf::from(&file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported session".to_string());
    let project_id = resolve_or_create_project(&state, project_id, &default_name)?;
    let job = create_import_job(&state.genesis, &project_id, &file_path)?;

    let genesis = state.genesis.clone();
    let runtime = state.whisper_runtime.clone();
    let job_id = job.id.clone();
    let worker_project_id = project_id.clone();
    thread::spawn(move || {
        run_import_pipeline(
            &genesis,
            &runtime,
            &worker_project_id,
            &job_id,
            "import",
            &file_path,
            &PathBuf::from(&file_path),
            ImportProgress::whole_job(),
        );
    });

    Ok(job)
}

/// Returns the project to import into, creating one named `default_name` when
/// the caller named none. Shared by file import and URL ingest so that a
/// fetched recording lands in a project the same way a dragged-in one does.
fn resolve_or_create_project(
    state: &AppState,
    project_id: Option<String>,
    default_name: &str,
) -> AppResult<String> {
    if let Some(id) = project_id {
        return Ok(id);
    }
    let output_root = state
        .recording_output
        .lock()
        .expect("recording output mutex poisoned")
        .ensure_current_writable()
        .map_err(AppError::InvalidInput)?;
    create_project_named(&state.genesis, &output_root, default_name)
}

/// Creates a project whose storage lives under `<storage_root>/projects/<id>`
/// and returns its id. The Tauri-state-free half of
/// [`resolve_or_create_project`], shared with the loopback API's upload
/// route (`local_api::import_recording`), which has no `AppState`.
pub(crate) fn create_project_named(
    genesis: &genesis_block_native::Storage,
    storage_root: &std::path::Path,
    name: &str,
) -> AppResult<String> {
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let storage_path = storage_root
        .join("projects")
        .join(&id)
        .display()
        .to_string();
    genesis_adapter::commit_rows(genesis, vec![genesis_adapter::upsert("projects", serde_json::json!({"id":id,"name":name,"storage_path":storage_path,"active_recording_id":null,"created_at":timestamp,"updated_at":timestamp}))]).map_err(AppError::Genesis)?;
    Ok(id)
}

/// The slice of a job's 0–100 progress bar that transcription owns.
///
/// A plain file import is transcription and nothing else, so it owns all of
/// it. A URL ingest spends real time downloading first, and a bar that sat at
/// zero for a five-minute fetch and then jumped would be reporting the wrong
/// thing — so the fetch owns the head of the bar and transcription is scaled
/// into what remains.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImportProgress {
    floor: i64,
}

impl ImportProgress {
    fn whole_job() -> Self {
        Self { floor: 0 }
    }

    fn after_fetch() -> Self {
        Self {
            floor: FETCH_PROGRESS_SHARE,
        }
    }

    /// Maps a worker's own 0–100 into this slice.
    fn scale(self, pct: i64) -> i64 {
        self.floor + pct * (100 - self.floor) / 100
    }
}

/// How much of a URL-ingest job's progress bar the download owns. Fetching is
/// bandwidth-bound and transcription is compute-bound, so no split is right
/// for every recording; a third is close enough to keep the bar moving
/// honestly in both halves.
const FETCH_PROGRESS_SHARE: i64 = 35;

/// Files the `transcript.transcribe` row that the UI polls, before any slow
/// work starts.
///
/// Created up front, not after the audio lands: a URL ingest can spend
/// minutes downloading, and a job that does not exist yet is a job the user
/// cannot see, cancel, or find again after closing the panel.
fn create_import_job(
    genesis: &Arc<genesis_block_native::Storage>,
    project_id: &str,
    input_ref: &str,
) -> AppResult<Job> {
    let job_id = Uuid::new_v4().to_string();
    let timestamp = now();
    genesis_adapter::commit_rows(genesis, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id":job_id,"project_id":project_id,"type":"transcript.transcribe","status":"running","progress":0,"input_refs_json":[input_ref],"output_refs_json":[],"provider_id":null,"error_code":null,"error_message":null,"attempt_no":1,"started_at":timestamp,"finished_at":null,"created_at":timestamp,"updated_at":timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id":Uuid::new_v4().to_string(),"job_id":job_id,"status":"running","message":"running","created_at":timestamp})),
    ]).map_err(AppError::Genesis)?;

    Ok(Job {
        id: job_id,
        project_id: project_id.to_string(),
        job_type: "transcript.transcribe".to_string(),
        status: "running".to_string(),
        progress: 0,
        input_refs: vec![input_ref.to_string()],
        output_refs: Vec::new(),
        provider_id: None,
        error_code: None,
        error_message: None,
        started_at: Some(timestamp.clone()),
        finished_at: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

/// Takes custody of `source_file`, transcribes it, and writes the segments,
/// terminalising `job_id` either way. Runs on a worker thread and never
/// returns an error: a failure is recorded on the job, which is the only
/// place anyone will look for it.
///
/// `source` is the value written to `recordings.source` (`import` for a file
/// the user picked, `url` for one `media_fetch` pulled in) and `input_path`
/// is what that recording came from — a filesystem path or the resolved URL.
/// Both are recorded rather than inferred so that, months later, a recording
/// can say where it came from without anyone re-deriving it.
///
/// Everything after custody is identical for both, deliberately: a fetched
/// recording is backed up, integrity-checked, and recovered by exactly the
/// same paths as a local one, because it is the same kind of thing once it
/// has landed.
#[allow(clippy::too_many_arguments)]
fn finalize_import_success(
    genesis: &Arc<genesis_block_native::Storage>,
    project_id: &str,
    recording_id: &str,
    chunk_id: &str,
    source: &str,
    input_path: &str,
    stored_path: &str,
    byte_size: i64,
    checksum: &str,
    created_at: &str,
    output: &WhisperOutput,
) -> AppResult<()> {
    let project = genesis_adapter::query(
        genesis,
        "projects",
        &["id", "name", "storage_path", "created_at"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบโปรเจกต์ {project_id}")))?;
    let project_name =
        genesis_adapter::string(&project, "projects.name").map_err(AppError::Genesis)?;
    let project_storage_path =
        genesis_adapter::string(&project, "projects.storage_path").map_err(AppError::Genesis)?;
    let project_created_at =
        genesis_adapter::string(&project, "projects.created_at").map_err(AppError::Genesis)?;
    let finished_at = now();
    let mut mutations = Vec::new();
    for segment in &output.segments {
        let segment_timestamp = now();
        mutations.push(genesis_adapter::upsert(
            "transcript_segments",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": project_id,
                "recording_id": recording_id,
                "speaker_id": null,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "text": segment.text,
                "confidence": segment.confidence,
                "created_at": segment_timestamp,
                "updated_at": segment_timestamp,
            }),
        ));
    }
    mutations.extend([
        genesis_adapter::upsert(
            "recordings",
            serde_json::json!({
                "id": recording_id,
                "project_id": project_id,
                "source": source,
                "input_path": input_path,
                "canonical_audio_path": stored_path,
                "status": "completed",
                "duration_ms": output.duration_ms,
                "created_at": created_at,
                "updated_at": finished_at,
            }),
        ),
        genesis_adapter::upsert(
            "audio_chunks",
            serde_json::json!({
                "id": chunk_id,
                "recording_id": recording_id,
                "sequence_no": 1,
                "file_path": stored_path,
                "start_ms": 0,
                "end_ms": output.duration_ms,
                "byte_size": byte_size,
                "checksum": checksum,
                "created_at": created_at,
                "transcribed_at": finished_at,
            }),
        ),
        genesis_adapter::upsert(
            "projects",
            serde_json::json!({
                "id": project_id,
                "name": project_name,
                "storage_path": project_storage_path,
                "active_recording_id": recording_id,
                "created_at": project_created_at,
                "updated_at": finished_at,
            }),
        ),
    ]);
    genesis_adapter::commit_rows(genesis, mutations).map_err(AppError::Genesis)
}

#[allow(clippy::too_many_arguments)]
fn run_import_pipeline(
    genesis: &Arc<genesis_block_native::Storage>,
    runtime: &WhisperRuntime,
    project_id: &str,
    job_id: &str,
    source: &str,
    input_path: &str,
    source_file: &std::path::Path,
    progress: ImportProgress,
) {
    let recording_id = Uuid::new_v4().to_string();
    run_import_pipeline_as(
        genesis,
        runtime,
        project_id,
        job_id,
        source,
        input_path,
        source_file,
        progress,
        &recording_id,
    );
}

/// [`run_import_pipeline`] with the recording id chosen by the caller. The
/// loopback API's upload route hands the id back to the browser in its
/// `202` before the worker has written anything, so the page can poll
/// `/recordings/{id}/transcript` for precisely this import.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_import_pipeline_as(
    genesis: &Arc<genesis_block_native::Storage>,
    runtime: &WhisperRuntime,
    project_id: &str,
    job_id: &str,
    source: &str,
    input_path: &str,
    source_file: &std::path::Path,
    progress: ImportProgress,
    recording_id: &str,
) {
    let recording_id = recording_id.to_string();
    let timestamp = now();

    // Take custody before anything depends on this audio. Until this existed
    // the ledger recorded the user's own path, so moving or deleting their
    // file invalidated a recording that still reported `completed`.
    let storage_root = match project_storage_path(genesis, project_id) {
        Ok(root) => root,
        Err(err) => {
            let _ = set_job_status(genesis, job_id, "failed", None, Some(&err.to_string()));
            return;
        }
    };
    let custodied =
        match audio_custody::take_custody_of_import(&storage_root, &recording_id, source_file) {
            Ok(custodied) => custodied,
            Err(error) => {
                let _ = set_job_status(genesis, job_id, "failed", None, Some(&error.to_string()));
                return;
            }
        };
    let stored_path = custodied.stored_path.display().to_string();
    let chunk_id = Uuid::new_v4().to_string();

    let registered = genesis_adapter::commit_rows(
        genesis,
        vec![
            genesis_adapter::upsert(
                "recordings",
                serde_json::json!({"id":recording_id,"project_id":project_id,"source":source,"input_path":input_path,"canonical_audio_path":stored_path,"status":"pending","duration_ms":0,"created_at":timestamp,"updated_at":timestamp}),
            ),
            // One chunk covering the whole file, so an import is backed up and
            // integrity-checked by exactly the same paths as a live capture.
            // `end_ms` is filled in once transcription reports the duration.
            genesis_adapter::upsert(
                "audio_chunks",
                serde_json::json!({"id":chunk_id,"recording_id":recording_id,"sequence_no":1,"file_path":stored_path,"start_ms":0,"end_ms":0,"byte_size":custodied.byte_size,"checksum":custodied.sha256,"created_at":timestamp,"transcribed_at":null}),
            ),
        ],
    );
    if let Err(err) = registered {
        let _ = set_job_status(genesis, job_id, "failed", None, Some(&err.to_string()));
        return;
    }

    let progress_storage = genesis.clone();
    let progress_job_id = job_id.to_string();
    let outcome = run_transcription(runtime, &stored_path, move |pct| {
        let _ = set_job_status(
            &progress_storage,
            &progress_job_id,
            "running",
            Some(progress.scale(pct)),
            None,
        );
    });

    match outcome {
        Ok(output) => {
            let insert_result = finalize_import_success(
                genesis,
                project_id,
                &recording_id,
                &chunk_id,
                source,
                input_path,
                &stored_path,
                custodied.byte_size,
                &custodied.sha256,
                &timestamp,
                &output,
            );

            match insert_result {
                Ok(()) => {
                    let _ = set_job_status(genesis, job_id, "completed", Some(100), None);
                }
                Err(err) => {
                    let _ = set_job_status(genesis, job_id, "failed", None, Some(&err.to_string()));
                }
            }
        }
        Err(message) => {
            let _ = set_job_status(genesis, job_id, "failed", None, Some(&message));
        }
    }
}

/// Fetches the audio behind a URL and transcribes it, as one job.
///
/// The consent check is here, at the command boundary, and not inside
/// `media_fetch::fetch`: a refusal must reach the user as a refusal — with
/// the reason and the next step — rather than as a worker that failed.
#[tauri::command]
fn fetch_and_transcribe(
    url: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Job> {
    // Checked before anything is created, so a refused fetch leaves no
    // project, no job row, and no trace of a URL that was never fetched.
    let readiness = media_fetch_readiness(&state)?;
    if !readiness.available {
        return Err(AppError::InvalidInput(
            readiness
                .detail
                .unwrap_or_else(|| "ยังดึงสื่อจากอินเทอร์เน็ตไม่ได้".to_string()),
        ));
    }
    let url = media_fetch::require_http_url(&url)
        .map_err(AppError::InvalidInput)?
        .to_string();

    // The download lands here first, not in the project: custody is what
    // moves it in, and a fetch that fails halfway must not leave a partial
    // file inside a project's audio tree looking like a recording.
    let staging = state
        .data_root
        .join("fetch")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&staging).map_err(|err| {
        AppError::InvalidInput(format!("could not prepare the fetch directory: {err}"))
    })?;

    let project_id = resolve_or_create_project(&state, project_id, &url)?;
    let job = create_import_job(&state.genesis, &project_id, &url)?;

    let genesis = state.genesis.clone();
    let runtime = state.whisper_runtime.clone();
    let job_id = job.id.clone();
    let worker_project_id = project_id.clone();

    thread::spawn(move || {
        let progress_storage = genesis.clone();
        let progress_job_id = job_id.clone();
        let fetched = media_fetch::fetch(&runtime, &url, &staging, move |pct| {
            let _ = set_job_status(
                &progress_storage,
                &progress_job_id,
                "running",
                Some(pct * FETCH_PROGRESS_SHARE / 100),
                None,
            );
        });

        match fetched {
            Ok(media) => {
                // What the fetch actually reached, on the job it happened
                // under. The URL alone does not say which extractor served
                // it or how long the media turned out to be, and after the
                // staging directory is gone this row is the only record.
                record_fetch_provenance(&genesis, &job_id, &media);
                // The project was created before the title was known, so it
                // is named now — the URL was only ever a placeholder.
                rename_placeholder_project(&genesis, &worker_project_id, &url, &media.title);
                run_import_pipeline(
                    &genesis,
                    &runtime,
                    &worker_project_id,
                    &job_id,
                    "url",
                    &media.webpage_url,
                    std::path::Path::new(&media.path),
                    ImportProgress::after_fetch(),
                );
            }
            Err(message) => {
                let _ = set_job_status(&genesis, &job_id, "failed", None, Some(&message));
            }
        }

        // Custody copied what it needed; the staging copy is redundant either
        // way, and on the failure path it is a partial download nothing
        // should ever read.
        let _ = std::fs::remove_dir_all(&staging);
    });

    Ok(job)
}

/// Writes what the fetch resolved to onto the job's event trail: which
/// extractor served it, and how long the source said it was.
///
/// A job event rather than a new column, because this is a fact about one
/// attempt rather than about the recording — a second fetch of the same URL
/// months later may well be served by a different extractor.
fn record_fetch_provenance(
    genesis: &Arc<genesis_block_native::Storage>,
    job_id: &str,
    media: &media_fetch::FetchedMedia,
) {
    let extractor = if media.extractor.is_empty() {
        "unknown"
    } else {
        &media.extractor
    };
    let _ = genesis_adapter::commit_rows(
        genesis,
        vec![genesis_adapter::upsert(
            "job_events",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "job_id": job_id,
                "status": "running",
                "message": format!(
                    "fetched via {extractor} ({} ms reported by source)",
                    media.duration_ms
                ),
                "created_at": now(),
            }),
        )],
    );
}

/// Replaces a project name that is still the placeholder URL with the fetched
/// title. Leaves a name the user chose, or one an earlier fetch already set,
/// alone — this only ever cleans up after itself.
fn rename_placeholder_project(
    genesis: &Arc<genesis_block_native::Storage>,
    project_id: &str,
    placeholder: &str,
    title: &str,
) {
    if title.trim().is_empty() {
        return;
    }
    let Ok(rows) = genesis_adapter::query(
        genesis,
        "projects",
        &[
            "id",
            "name",
            "storage_path",
            "active_recording_id",
            "created_at",
        ],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    ) else {
        return;
    };
    let Some(row) = rows.into_iter().next() else {
        return;
    };
    if row.get("projects.name").and_then(|value| value.as_str()) != Some(placeholder) {
        return;
    }
    let timestamp = now();
    let _ = genesis_adapter::commit_rows(
        genesis,
        vec![genesis_adapter::upsert(
            "projects",
            serde_json::json!({
                "id": project_id,
                "name": title,
                "storage_path": row.get("projects.storage_path"),
                "active_recording_id": row.get("projects.active_recording_id"),
                "created_at": row.get("projects.created_at"),
                "updated_at": timestamp,
            }),
        )],
    );
}

/// Probes the URL-ingest installation and reads the stored consent flag.
fn media_fetch_readiness(state: &AppState) -> AppResult<media_fetch::MediaFetchReadiness> {
    let conn = paired_devices_connection(state)?;
    let consent = policy::media_fetch_consent(&conn).map_err(AppError::InvalidInput)?;
    Ok(media_fetch::probe(&state.whisper_runtime, consent))
}

/// Reports whether this installation can fetch media from a URL, and why not
/// when it cannot.
#[tauri::command]
fn media_fetch_status(state: State<'_, AppState>) -> AppResult<media_fetch::MediaFetchReadiness> {
    media_fetch_readiness(&state)
}

/// Grants or revokes permission for FUNG to fetch media from the internet,
/// and reports the resulting readiness so the caller does not have to ask
/// again to find out what is still missing.
#[tauri::command]
fn media_fetch_consent_set(
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<media_fetch::MediaFetchReadiness> {
    let conn = paired_devices_connection(&state)?;
    policy::set_media_fetch_consent(&conn, enabled).map_err(AppError::InvalidInput)?;
    media_fetch_readiness(&state)
}

/// Cap on the non-`PROGRESS` stderr tail captured for a worker's error
/// message. Chatty workers (torch/pyannote log warnings on every run) can
/// otherwise dump megabytes of noise into a `job_events` row; the real
/// failure text is almost always right before the process exits, so the
/// *tail* is the part worth keeping.
const STDERR_TAIL_CAP_BYTES: usize = 8192;

/// Appends `line` (plus a newline) to `buffer`, then drops whole lines from
/// the front until `buffer` is back at or under `STDERR_TAIL_CAP_BYTES` —
/// never splitting a line, and always keeping the most recently written
/// (i.e. most relevant) text.
pub(crate) fn append_bounded(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push('\n');
    while buffer.len() > STDERR_TAIL_CAP_BYTES {
        match buffer.find('\n') {
            Some(newline_index) => {
                buffer.drain(..=newline_index);
            }
            None => break,
        }
    }
}

/// Runs a python worker from the whisper venv and returns its stdout after a
/// zero exit. `PROGRESS <pct>` stderr lines stream through `on_progress`;
/// other stderr lines are collected into the error message on failure.
/// `path_prefix`, when set, is prepended to the child's PATH — used by
/// `run_transcription` to expose the bundled CUDA DLLs; harmless to omit for
/// workers (like diarize) that don't need it.
pub(crate) fn run_python_worker(
    runtime: &WhisperRuntime,
    script: &std::path::Path,
    args: &[&str],
    path_prefix: Option<&std::path::Path>,
    hf_home: Option<&std::path::Path>,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<String, String> {
    if !runtime.python.exists() {
        return Err(format!(
            "FUNG Python runtime is missing at {}. Reinstall the FUNG application bundle.",
            runtime.python.display(),
        ));
    }
    if !script.exists() {
        return Err(format!(
            "FUNG worker script is missing at {}. Reinstall the FUNG application bundle.",
            script.display(),
        ));
    }

    let mut command = Command::new(&runtime.python);
    command.arg(script).args(args);
    if let Some(model) = worker_whisper_model_env_path(runtime) {
        command.env("FUNG_WHISPER_MODEL", model);
    }
    match hf_home {
        // Only the worker that needs it gets a redirected cache.
        Some(hf_home) => {
            command.env("HF_HOME", hf_home);
        }
        // A worker handed no Hugging Face cache has no business reaching the
        // hub, so the environment says so instead of a comment.
        //
        // The transcription worker loads a bundled model *by path*. If that
        // path ever stops resolving -- a partial install, a runtime layout
        // change, the script run by hand -- faster-whisper's own default is
        // a model name, which it resolves by downloading from
        // huggingface.co. That would turn the one pass this product's
        // local-first claim rests on into a silent network fetch, on the
        // machine of someone who chose FUNG precisely so their audio would
        // not leave it. Offline makes the same condition a legible error.
        None => {
            command.env("HF_HUB_OFFLINE", "1");
        }
    }

    if let Some(prefix) = path_prefix {
        let inherited_path = env::var_os("PATH").unwrap_or_default();
        let child_path = env::join_paths([prefix.as_os_str(), inherited_path.as_os_str()])
            .map_err(|err| format!("could not compose FUNG GPU runtime PATH: {err}"))?;
        command.env("PATH", child_path);
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to launch worker: {err}"))?;

    let stderr = child.stderr.take().expect("stderr was piped");
    let stderr_tail = Arc::new(Mutex::new(String::new()));
    let tail = stderr_tail.clone();
    let progress_thread = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("PROGRESS ") {
                if let Ok(pct) = rest.trim().parse::<i64>() {
                    on_progress(pct);
                }
            } else if let Ok(mut tail) = tail.lock() {
                append_bounded(&mut tail, &line);
            }
        }
    });

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut raw_output = String::new();
    stdout
        .read_to_string(&mut raw_output)
        .map_err(|err| format!("failed to read worker output: {err}"))?;

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for worker: {err}"))?;
    let _ = progress_thread.join();

    if !status.success() {
        let tail = stderr_tail.lock().map(|t| t.clone()).unwrap_or_default();
        return Err(format!("worker exited with {status}: {}", tail.trim()));
    }

    Ok(raw_output)
}

/// Runs the faster-whisper worker script and blocks until it exits,
/// reporting `PROGRESS <pct>` lines from stderr via `on_progress` as they
/// arrive. Intended to run off the main thread (see `import_and_transcribe`).
/// The GPU/CUDA DLL check and profile selection are whisper-specific and stay
/// here rather than in the generic `run_python_worker`.
pub(crate) fn run_transcription(
    runtime: &WhisperRuntime,
    file_path: &str,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<WhisperOutput, String> {
    require_bundled_whisper_model(runtime)?;
    let profile = transcription_profile()?;
    let worker_script = whisper_worker_script(runtime, false)?;

    let path_prefix = if profile == "gpu" {
        let missing: Vec<&str> = REQUIRED_CUDA_DLLS
            .iter()
            .copied()
            .filter(|dll| !runtime.cuda_bin.join(dll).is_file())
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "FUNG GPU runtime is incomplete at {} (missing {}). Reinstall the FUNG GPU bundle or select FUNG_TRANSCRIPTION_PROFILE=cpu.",
                runtime.cuda_bin.display(),
                missing.join(", ")
            ));
        }
        Some(runtime.cuda_bin.as_path())
    } else {
        None
    };

    let raw_output = run_python_worker(
        runtime,
        &worker_script,
        &[file_path, "--profile", &profile],
        path_prefix,
        // Transcription loads a bundled model by path. Passing `None`
        // both leaves the cache alone and pins the worker offline -- see
        // `run_python_worker`.
        None,
        on_progress,
    )?;

    serde_json::from_str::<WhisperOutput>(raw_output.trim())
        .map_err(|err| format!("failed to parse transcription output: {err}"))
}

/// Runs the pyannote diarization worker script. Path B calls this after
/// transcribing the mixed file; a failure here must never take down the
/// transcript (see `zoom_sync::run_mixed_audio_path`).
/// Runs the local diarization worker over one audio file.
///
/// `data_root` is threaded through only to locate FUNG's Hugging Face cache:
/// the gated pipeline is fetched once into the app's own directory rather
/// than the user's global `~/.cache/huggingface`, so a local-first install
/// keeps its weights with the rest of its data.
pub(crate) fn run_diarization(
    runtime: &WhisperRuntime,
    data_root: &std::path::Path,
    file_path: &str,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<zoom_sync::DiarizeOutput, String> {
    // Answer from the filesystem before paying for a subprocess. Without
    // this the only signal a missing dependency produced was the worker's
    // own `MODEL_ACCESS ...` line, several seconds and one process later.
    let readiness = diarization::probe(runtime, data_root);
    if let Some(blocker) = readiness.blocker {
        return Err(format!("{}: {}", blocker.code(), blocker.detail()));
    }

    let script = diarization::worker_script(runtime)
        .ok_or_else(|| "could not resolve the diarization worker path".to_string())?;
    let raw = run_python_worker(
        runtime,
        &script,
        &[file_path],
        None,
        Some(&diarization::hf_home(data_root)),
        on_progress,
    )?;
    serde_json::from_str(raw.trim())
        .map_err(|err| format!("failed to parse diarization output: {err}"))
}

/// Starts (or reports) the loopback API and returns its connect URL. Idempotent
/// so the Settings panel can re-read the URL without rotating the token; the
/// routes and the token policy live in `local_api`.
#[tauri::command]
fn start_local_api(state: State<'_, AppState>) -> AppResult<local_api::LocalApiInfo> {
    Ok(local_api::start(&state)?)
}

/// Opt-in LAN sharing of the local API so a phone's browser on the same
/// Wi-Fi can open the desktop-served recordings page (QR in Settings ›
/// Runtime). Off by default, stopped on demand; see `local_api::set_lan`.
#[tauri::command]
fn set_local_api_lan(
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<local_api::LocalApiInfo> {
    Ok(local_api::set_lan(&state, enabled)?)
}

/// Temporary diagnostic used by `bin/dbcheck.rs` only — remove together with
/// that binary once the v8 migration issue is resolved.
#[doc(hidden)]
pub fn __debug_db_probe(path: &str) -> Result<String, String> {
    let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
        path: path.to_string(),
        page_cache_mb: Some(16),
        read_only: Some(false),
        vector_dim: Some(384),
        retention: None,
    })
    .map_err(|error| format!("open failed: {error}"))?;

    let mut report = String::new();

    // Rows most likely to hold a string inside a Json-typed column.
    match genesis_adapter::query(
        &storage,
        "model_providers",
        &["id", "kind", "config_json"],
        vec![],
        100,
    ) {
        Ok(rows) => {
            report.push_str(&format!("model_providers rows: {}\n", rows.len()));
            for row in rows {
                let id = row
                    .get("model_providers.id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let kind = row
                    .get("model_providers.kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let config = row.get("model_providers.config_json");
                let type_name = match config {
                    Some(serde_json::Value::String(_)) => "STRING",
                    Some(serde_json::Value::Object(_)) => "object",
                    Some(serde_json::Value::Array(_)) => "array",
                    Some(serde_json::Value::Null) | None => "null/missing",
                    _ => "other",
                };
                report.push_str(&format!("  {id} ({kind}) config_json = {type_name}\n"));
            }
        }
        Err(error) => report.push_str(&format!("model_providers query failed: {error}\n")),
    }

    report.push_str("running install()...\n");
    match genesis_adapter::install(&storage) {
        Ok(()) => report.push_str("install OK\n"),
        Err(error) => report.push_str(&format!("install FAILED: {error}\n")),
    }

    if let Some(legacy) = std::env::args().nth(2) {
        report.push_str(&format!("importing legacy sqlite {legacy} ...\n"));
        match genesis_adapter::import_legacy_sqlite(&storage, std::path::Path::new(&legacy)) {
            Ok(count) => report.push_str(&format!("legacy import OK: {count} rows\n")),
            Err(error) => report.push_str(&format!("legacy import FAILED: {error}\n")),
        }
    }

    let seeded_at = now();
    let seed = genesis_adapter::commit_rows(
        &storage,
        vec![genesis_adapter::upsert(
            "model_providers",
            serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":DEFAULT_OLLAMA_ENDPOINT},"created_at":seeded_at,"updated_at":seeded_at}),
        )],
    );
    match seed {
        Ok(()) => report.push_str("seed OK\n"),
        Err(error) => report.push_str(&format!("seed FAILED: {error}\n")),
    }

    Ok(report)
}

/// Headless end-to-end smoke of the Live Meeting pipeline on REAL hardware:
/// capture (mic + WASAPI loopback) → durable chunk ledger → persistent
/// whisper worker → transcript segments → summary (Ollama) → Markdown export.
/// Used by `bin/live_smoke.rs`; also the LM-01 acceptance harness, so keep it
/// in sync with the command-path behavior in `live_meeting.rs`.
#[doc(hidden)]
pub fn __debug_live_smoke(
    work_dir: &str,
    capture_secs: u64,
    language: Option<String>,
) -> Result<String, String> {
    use live_meeting::{
        spawn_capture_thread, CaptureEvent, ChannelKind, LiveWorker, CHANNEL_MIC, CHANNEL_SYSTEM,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    let mut report = String::new();
    let dir = PathBuf::from(work_dir);
    let chunks_dir = dir.join("chunks");
    std::fs::create_dir_all(&chunks_dir).map_err(|error| error.to_string())?;

    let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
        path: dir.join("genesisdb").display().to_string(),
        page_cache_mb: Some(32),
        read_only: Some(false),
        vector_dim: Some(384),
        retention: None,
    })
    .map_err(|error| format!("open storage: {error}"))?;
    genesis_adapter::install(&storage)?;

    let project_id = "smoke-project";
    let recording_id = "smoke-recording";
    let timestamp = now();
    genesis_adapter::commit_rows(
        &storage,
        vec![
            genesis_adapter::upsert(
                "model_providers",
                serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":DEFAULT_OLLAMA_ENDPOINT},"created_at":timestamp,"updated_at":timestamp}),
            ),
            genesis_adapter::upsert(
                "projects",
                serde_json::json!({"id":project_id,"name":"Live smoke","storage_path":dir.display().to_string(),"active_recording_id":null,"created_at":timestamp,"updated_at":timestamp}),
            ),
            genesis_adapter::upsert(
                "speakers",
                serde_json::json!({"id":format!("{project_id}::speaker::me"),"project_id":project_id,"key":"me","display_name":"เรา","confidence":null,"created_at":timestamp,"updated_at":timestamp}),
            ),
            genesis_adapter::upsert(
                "speakers",
                serde_json::json!({"id":format!("{project_id}::speaker::them"),"project_id":project_id,"key":"them","display_name":"อีกฝ่าย","confidence":null,"created_at":timestamp,"updated_at":timestamp}),
            ),
        ],
    )?;
    let mut capture_record = live_meeting::start_desktop_capture(
        &storage,
        project_id,
        recording_id,
        &chunks_dir.display().to_string(),
        &timestamp,
        language.as_deref(),
    )?;

    // Progress must survive a killed process: append every stage to a file.
    let report_path = dir.join("smoke-report.txt");
    let note = |report: &mut String, line: &str| {
        report.push_str(line);
        report.push('\n');
        let _ = std::fs::write(&report_path, report.as_bytes());
        eprintln!("[smoke] {line}");
    };

    // Whisper worker FIRST: its first run may download the model, and that
    // wait must not silently extend the capture window.
    let root = source_root();
    let runtime = WhisperRuntime {
        python: root
            .join(".venv-whisper")
            .join("Scripts")
            .join("python.exe"),
        script: root.join("scripts").join("transcribe.py"),
        cuda_bin: root.join("runtime").join("cuda12").join("bin"),
    };
    let mut worker = LiveWorker::spawn(&runtime, language.as_deref())?;
    worker.wait_ready()?;
    note(&mut report, "whisper live worker: ready");

    let stop = Arc::new(AtomicBool::new(false));
    let (chunk_tx, chunk_rx) = std::sync::mpsc::sync_channel(16);

    if capture_secs == 0 {
        // Inject mode: treat prepared WAV files in {work_dir}/inject as mic
        // chunks — proves ledger→whisper→segments→summary deterministically,
        // independent of room acoustics and device quirks.
        let inject_dir = dir.join("inject");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&inject_dir)
            .map_err(|error| format!("inject dir missing: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().map(|ext| ext == "wav").unwrap_or(false))
            .collect();
        files.sort();
        let mut cursor_ms: i64 = 0;
        for path in files {
            let reader = hound::WavReader::open(&path).map_err(|error| error.to_string())?;
            let spec = reader.spec();
            let duration_ms = (reader.duration() as i64) * 1000 / spec.sample_rate.max(1) as i64;
            let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
            let checksum = {
                use sha2::Digest;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&bytes);
                format!("{:x}", hasher.finalize())
            };
            let _ = chunk_tx.send(CaptureEvent::Chunk(live_meeting::RawChunk {
                channel: CHANNEL_MIC,
                chunk_id: Uuid::new_v4().to_string(),
                file_path: path.display().to_string(),
                start_ms: cursor_ms,
                end_ms: cursor_ms + duration_ms,
                byte_size: bytes.len() as i64,
                checksum,
            }));
            cursor_ms += duration_ms;
        }
        note(&mut report, "inject mode: queued prepared WAV chunks");
    } else {
        let mic = spawn_capture_thread(
            ChannelKind::Mic,
            CHANNEL_MIC,
            None,
            stop.clone(),
            chunk_tx.clone(),
            chunks_dir.clone(),
        );
        match &mic {
            Ok(ready) => note(&mut report, &format!("mic device: {}", ready.device_name)),
            Err(error) => note(&mut report, &format!("mic UNAVAILABLE: {error}")),
        }
        let system = spawn_capture_thread(
            ChannelKind::SystemLoopback,
            CHANNEL_SYSTEM,
            None,
            stop.clone(),
            chunk_tx.clone(),
            chunks_dir.clone(),
        );
        match &system {
            Ok(ready) => note(
                &mut report,
                &format!("loopback device: {}", ready.device_name),
            ),
            Err(error) => note(&mut report, &format!("loopback UNAVAILABLE: {error}")),
        }
        if mic.is_err() && system.is_err() {
            return Err(format!("no capture channel available\n{report}"));
        }
        note(&mut report, &format!("capturing for {capture_secs}s..."));
        std::thread::sleep(Duration::from_secs(capture_secs));
        stop.store(true, Ordering::SeqCst);
    }
    drop(chunk_tx);

    let mut chunk_count = 0usize;
    let mut segment_count = 0usize;
    let mut samples: Vec<String> = Vec::new();
    // Mirrors the coordinator: two interleaved channel timelines mean the
    // last-written chunk is not necessarily the longest one.
    let mut max_end_ms: i64 = 0;
    while let Ok(event) = chunk_rx.recv() {
        let chunk = match event {
            CaptureEvent::Chunk(chunk) => chunk,
            CaptureEvent::ChunkWriteFailed {
                channel,
                start_ms,
                end_ms,
                error,
            } => {
                max_end_ms = max_end_ms.max(end_ms);
                report.push_str(&format!(
                    "chunk write failed on {channel} {start_ms}..{end_ms} ms: {error}
"
                ));
                continue;
            }
            CaptureEvent::SourceGap {
                channel,
                start_ms,
                end_ms,
                reason,
            } => {
                max_end_ms = max_end_ms.max(end_ms);
                report.push_str(&format!(
                    "capture gap on {channel} {start_ms}..{end_ms} ms ({reason})
"
                ));
                continue;
            }
            CaptureEvent::StreamFailed { channel, error } => {
                report.push_str(&format!(
                    "stream fault on {channel}: {error}
"
                ));
                continue;
            }
        };
        let chunk_timestamp = now();
        max_end_ms = max_end_ms.max(chunk.end_ms);
        capture_record = genesis_adapter::append_capture_chunk(
            &storage,
            &capture_record,
            genesis_adapter::AudioChunk {
                id: &chunk.chunk_id,
                file_path: &chunk.file_path,
                start_ms: chunk.start_ms,
                end_ms: chunk.end_ms,
                byte_size: chunk.byte_size,
                checksum: &chunk.checksum,
                timestamp: &chunk_timestamp,
            },
        )?;
        chunk_count += 1;
        match worker.transcribe_chunk(&chunk) {
            Ok(response) => {
                if let Some(error) = response.error {
                    report.push_str(&format!("chunk {} error: {error}\n", chunk.chunk_id));
                    continue;
                }
                let speaker_key = if chunk.channel == CHANNEL_MIC {
                    "me"
                } else {
                    "them"
                };
                let mut mutations = Vec::new();
                for segment in &response.segments {
                    let seg_timestamp = now();
                    mutations.push(genesis_adapter::upsert(
                        "transcript_segments",
                        serde_json::json!({
                            "id": Uuid::new_v4().to_string(),
                            "project_id": project_id,
                            "recording_id": recording_id,
                            "speaker_id": format!("{project_id}::speaker::{speaker_key}"),
                            "start_ms": chunk.start_ms + segment.start_ms,
                            "end_ms": chunk.start_ms + segment.end_ms,
                            "text": segment.text,
                            "confidence": segment.confidence,
                            "created_at": seg_timestamp,
                            "updated_at": seg_timestamp,
                        }),
                    ));
                    segment_count += 1;
                    if samples.len() < 10 {
                        samples.push(format!("[{}] {}", chunk.channel, segment.text));
                    }
                }
                if !mutations.is_empty() {
                    genesis_adapter::commit_rows(&storage, mutations)?;
                }
            }
            Err(error) => report.push_str(&format!("worker failure: {error}\n")),
        }
    }
    worker.shutdown();
    capture_record.duration_ms = capture_record.duration_ms.max(max_end_ms);
    genesis_adapter::finish_capture(&storage, &capture_record, &now())?;

    note(
        &mut report,
        &format!(
            "chunks: {chunk_count} (ledger duration {} ms), segments: {segment_count}",
            capture_record.duration_ms
        ),
    );
    for sample in samples.clone() {
        note(&mut report, &format!("  {sample}"));
    }

    note(&mut report, "generating summaries (Ollama)...");
    match meeting_intel::summarize_and_export(&storage, project_id, recording_id) {
        Ok(export_path) => note(
            &mut report,
            &format!("summary + export: OK -> {export_path}"),
        ),
        Err(error) => note(&mut report, &format!("summary + export FAILED: {error}")),
    }

    Ok(report)
}

/// Parses `KEY=value` lines from a `.env` file's contents. Blank lines and
/// lines starting with `#` are skipped; surrounding whitespace and a single
/// pair of wrapping double quotes on the value are trimmed. A line whose
/// value is empty (`KEY=`, the repo's committed template) is skipped rather
/// than emitted as `("KEY", "")`: `native_auth::configured_value` already
/// treats an empty value as unconfigured, and leaving the key genuinely
/// unset — instead of set-but-empty — keeps that the same for any other
/// future reader that checks presence without also checking emptiness. This
/// is the pure, testable half of `.env` loading — no filesystem or
/// process-environment access happens here.
fn parse_dotenv(contents: &str) -> Vec<(String, String)> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(value);
            if value.is_empty() {
                return None;
            }
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Loads `.env` into the process environment at startup so the native
/// broker's `env::var("FUNG_SUPABASE_URL")`-style reads (see
/// `native_auth::configured_value`) see the same project configuration the
/// Vite frontend already reads automatically. `tauri dev` runs `cargo run`
/// with the working directory at `src-tauri`, one level below the repo root
/// where `.env` lives, so this walks up from the current directory looking
/// for it — the same convention `dotenvy`/`dotenv` tooling uses. A missing
/// file is not an error: packaged installs are expected to supply real
/// OS/environment configuration instead. Variables already set in the process
/// environment are never overwritten, so a real deployment value always wins
/// over whatever is checked into `.env`.
fn load_dotenv() {
    let mut dir = env::current_dir().ok();
    while let Some(candidate) = dir {
        let path = candidate.join(".env");
        if path.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                for (key, value) in parse_dotenv(&contents) {
                    if env::var_os(&key).is_none() {
                        // SAFETY: called once, synchronously, as the very
                        // first statement in `run()` before Tauri spawns any
                        // thread, so no concurrent env reader can observe a
                        // torn write.
                        unsafe { env::set_var(key, value) };
                    }
                }
            }
            break;
        }
        dir = candidate.parent().map(|parent| parent.to_path_buf());
    }
}

#[cfg(test)]
mod dotenv_tests {
    use super::parse_dotenv;

    #[test]
    fn parse_dotenv_reads_simple_assignment() {
        assert_eq!(
            parse_dotenv("VITE_SUPABASE_URL=https://example.supabase.co"),
            vec![(
                "VITE_SUPABASE_URL".to_owned(),
                "https://example.supabase.co".to_owned()
            )]
        );
    }

    #[test]
    fn parse_dotenv_skips_blank_lines_and_comments() {
        let contents = "\n# a comment\nFUNG_SUPABASE_URL=https://example.supabase.co\n\n";
        assert_eq!(
            parse_dotenv(contents),
            vec![(
                "FUNG_SUPABASE_URL".to_owned(),
                "https://example.supabase.co".to_owned()
            )]
        );
    }

    #[test]
    fn parse_dotenv_skips_empty_values_like_the_checked_in_env() {
        // Regression for the desktop login flow: a blank `KEY=` line (the
        // repo's committed `.env` template) must not become `("KEY", "")`
        // that would then satisfy `env::var_os` and mask the real
        // `auth_config_invalid` failure with something even more confusing.
        assert_eq!(
            parse_dotenv("VITE_SUPABASE_URL=\nVITE_SUPABASE_ANON_KEY="),
            Vec::<(String, String)>::new()
        );
    }

    #[test]
    fn parse_dotenv_trims_wrapping_quotes_and_whitespace() {
        assert_eq!(
            parse_dotenv("  FUNG_SUPABASE_ANON_KEY = \"anon-key-value\"  "),
            vec![(
                "FUNG_SUPABASE_ANON_KEY".to_owned(),
                "anon-key-value".to_owned()
            )]
        );
    }

    #[test]
    fn parse_dotenv_ignores_malformed_lines_without_an_equals_sign() {
        assert_eq!(parse_dotenv("not-a-valid-line"), Vec::new());
    }
}

fn shutdown_native_state(state: &AppState) {
    live_meeting::shutdown(state);
    state.playback.shutdown();
    state
        .review_registry
        .lock()
        .expect("review registry mutex poisoned")
        .clear();
    state.jobs.shutdown();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    load_dotenv();
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(native_recorder::init())
        .plugin(on_device_ai::init())
        .setup(|app| {
            let state = app_state(app)?;
            // Recovery runs before the window is usable so a crashed session
            // is never presented as a healthy one. Detection only: stale jobs
            // are terminalized and interrupted recordings are recorded, while
            // adopting orphaned audio stays an explicit user action.
            match recovery::scan(&state.genesis) {
                Ok(report) => {
                    if report.needs_attention() || report.stale_jobs_failed > 0 {
                        eprintln!(
                            "[recovery] {} interrupted recording(s), {} stale job(s) closed",
                            report.interrupted.len(),
                            report.stale_jobs_failed
                        );
                    }
                }
                // A failed scan must not stop the app from opening — the user
                // still needs access to their existing projects.
                Err(error) => eprintln!("[recovery] startup scan failed: {error}"),
            }
            app.manage(state);
            app.manage(filesystem_backup::FilesystemBackupState::default());
            app.manage(backup::BackupJobState::default());
            if let Err(error) = auth_session::startup_recover() {
                eprintln!("[auth-session] deterministic startup recovery failed: {error}");
            }

            // The worker starts only now: its handlers reach back into
            // AppState, so it must not run before the state is managed.
            // Adoption follows, picking up whatever the last run left
            // queued or was still running when it exited.
            let engine = app.state::<AppState>().jobs.clone();
            engine.start_worker(app.handle().clone());
            match engine.adopt_pending() {
                Ok(0) => {}
                Ok(count) => eprintln!("[jobs] resumed {count} pending job(s)"),
                // A failed adoption leaves the rows exactly as they were, so
                // the next launch tries again — but it must not be silent,
                // because until then that work is stalled.
                Err(error) => eprintln!("[jobs] could not resume pending jobs: {error}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            create_project,
            list_projects,
            create_job,
            cancel_job,
            runnable_job_types,
            list_jobs,
            list_model_providers,
            list_transcript_segments,
            meeting_transcript_snapshot,
            replay_meeting_events,
            correct_meeting_utterance,
            meeting_adapter_capabilities,
            meeting_local_owner_provision,
            meeting_local_owner_vault_options,
            meeting_local_owner_unlock,
            meeting_local_owner_lock,
            meeting_knowledge_collection_create,
            meeting_knowledge_collections_list,
            meeting_knowledge_set_selection,
            meeting_knowledge_import_selected,
            meeting_knowledge_metric_save,
            meeting_knowledge_metric_compute,
            meeting_people_list,
            meeting_people_profile_create,
            meeting_people_profile_update,
            meeting_people_profile_archive,
            meeting_people_link_propose,
            meeting_people_link_confirm,
            meeting_people_link_reject,
            meeting_people_link_unlink,
            meeting_agent_preflight,
            meeting_agent_start,
            meeting_agent_pause,
            meeting_agent_stop,
            meeting_agent_set_policy,
            meeting_agent_ask,
            meeting_agent_preview_delivery,
            meeting_agent_approve_delivery,
            meeting_agent_revoke,
            meeting_agent_status,
            meeting_agent_history,
            correct_transcript_segment,
            import_and_transcribe,
            fetch_and_transcribe,
            media_fetch_status,
            media_fetch_consent_set,
            transcript_export::list_export_artifacts,
            audio_integrity_check,
            recovery_scan,
            recovery_recover,
            start_local_api,
            set_local_api_lan,
            open_external_account_portal,
            auth_open_google_authorize,
            auth_exchange_google_code,
            account_portal_open,
            auth_session::broker_session_login_begin,
            auth_session::broker_session_login_cancel,
            auth_session::broker_session_status,
            auth_session::broker_session_logout,
            auth_session::broker_enrollment_request,
            auth_session::broker_enrollment_status,
            auth_session::broker_device_list,
            auth_session::broker_pairing_create,
            auth_session::broker_pairing_poll,
            auth_session::broker_pairing_reconcile,
            auth_session::broker_device_revoke,
            auth_session::broker_device_audit_list,
            auth_session::broker_device_endpoint_publish,
            device_identity::device_identity_ensure,
            device_identity::device_public_key,
            zoom_sync::zoom_connect,
            zoom_sync::zoom_connection_status,
            zoom_sync::zoom_disconnect,
            zoom_sync::zoom_list_recordings,
            zoom_sync::zoom_import_recording,
            graph_build::graph_build_start,
            diarization::diarization_status,
            recording_output::recording_output_get,
            recording_output::recording_output_set,
            recording_output::recording_output_reset,
            live_meeting::live_capture_devices,
            live_meeting::live_meeting_start,
            live_meeting::live_meeting_stop,
            live_meeting::live_meeting_status,
            recording_review::desktop_recordings_list,
            recording_review::desktop_recordings_release,
            recording_review::desktop_recording_get,
            meeting_intel::meeting_ask,
            meeting_intel::meeting_ask_recording,
            meeting_intel::meeting_summaries,
            meeting_intel::generate_meeting_summary,
            desktop_playback::desktop_playback_open,
            desktop_playback::desktop_playback_control,
            desktop_playback::desktop_playback_status,
            desktop_playback::desktop_playback_close,
            external_mcp_commands::external_connectors_list,
            external_mcp_commands::external_connector_register,
            external_mcp_commands::external_connector_disconnect,
            external_mcp_commands::meeting_tool_suggest,
            external_mcp_commands::meeting_tool_execute,
            external_mcp_commands::meeting_tool_cancel,
            external_mcp_commands::meeting_tool_revoke,
            external_mcp_commands::meeting_tool_runs_list,
            mobile::mobile_capture_start,
            mobile::mobile_capture_append_segment,
            mobile::mobile_capture_reconcile_native,
            mobile::mobile_capture_finish,
            mobile::mobile_capture_playback_segment,
            mobile::mobile_capture_playback_manifest,
            native_recorder::mobile_native_recorder_start,
            native_recorder::mobile_native_recorder_status,
            native_recorder::mobile_native_recorder_level,
            native_recorder::mobile_native_recorder_control,
            on_device_ai::mobile_on_device_ai_status,
            mobile::mobile_note_upsert,
            mobile::mobile_relation_upsert,
            mobile::mobile_graph_query,
            mobile::mobile_timeline_query,
            mobile::mobile_recordings_query,
            mobile::mobile_diarization_start,
            mobile::mobile_processing_job_start,
            mobile::mobile_diarization_import,
            mobile::mobile_speaker_rename,
            mobile::mobile_speaker_turn_split,
            mobile::mobile_speaker_merge,
            mobile::mobile_speaker_turn_confirm,
            mobile::mobile_story_create,
            mobile::mobile_story_query,
            mobile::mobile_story_clip_move,
            mobile::mobile_story_clip_split,
            mobile::mobile_story_clip_trim,
            mobile::mobile_story_undo,
            mobile::mobile_story_redo,
            mobile::mobile_model_packages_query,
            mobile::mobile_refinement_review,
            mobile::mobile_effect_chain_update,
            mobile::mobile_voice_profiles_query,
            mobile::mobile_agent_voice_grant_set,
            mobile::mobile_agent_voice_stop,
            mobile::mobile_pairing_complete,
            mobile::mobile_voice_parse,
            mobile::mobile_mcp_set_enabled,
            broker_fungwire_set_enabled,
            broker_fungwire_status,
            fungwire_client::fungwire_desktop_reachable,
            fungwire_client::fungwire_desktop_status_probe,
            fungwire_client::fungwire_delegate_transcription,
            fungwire_client::fungwire_job_poll,
            cloud_commands::cloud_config_set,
            cloud_commands::cloud_config_clear,
            cloud_commands::cloud_config_status,
            cloud_commands::tier_policy_get,
            cloud_commands::tier_policy_set,
            cloud_commands::cloud_call_counts_today,
            backup::backup_status,
            backup::backup_list_archives,
            backup::backup_generate_recovery_phrase,
            backup::backup_run,
            backup::backup_restore,
            backup::backup_restore_select_target,
            filesystem_backup::filesystem_backup_select_root,
            tts_provider_register,
            tts_provider_update,
            tts_provider_toggle,
            tts_provider_test,
            tts_synthesize_text
        ])
        .build(tauri::generate_context!())
        .expect("error while running FUNG")
        .run(|app, event| match event {
            // Stop taking new work at exit. Anything still queued stays
            // queued in the ledger and is adopted on the next launch —
            // shutdown is not cancellation.
            tauri::RunEvent::ExitRequested { .. } => {
                if let Err(error_code) = auth_session::shutdown() {
                    eprintln!("{error_code}");
                }
                if let Some(state) = app.try_state::<AppState>() {
                    shutdown_native_state(&state);
                }
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } if label == "main" => {
                if let Some(state) = app.try_state::<AppState>() {
                    shutdown_native_state(&state);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod transcript_view_tests {
    use super::*;

    fn open_storage() -> (PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-view-cap-test-{}", Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    /// `recordings_of` recordings, each holding `segments_each` segments.
    fn seed(storage: &genesis_block_native::Storage, recordings_of: usize, segments_each: i64) {
        let mut rows = vec![genesis_adapter::upsert(
            "projects",
            serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"}),
        )];
        for recording in 0..recordings_of {
            let recording_id = format!("r{recording}");
            rows.push(genesis_adapter::upsert("recordings", serde_json::json!({"id":recording_id,"project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"completed","duration_ms":0,"created_at":"t","updated_at":"t"})));
            for index in 0..segments_each {
                rows.push(genesis_adapter::upsert(
                    "transcript_segments",
                    serde_json::json!({
                        "id": format!("{recording_id}-s{index}"), "project_id": "p1",
                        "recording_id": recording_id, "speaker_id": null,
                        "start_ms": index * 1000, "end_ms": index * 1000 + 900,
                        "text": format!("บรรทัด {index}"), "confidence": 0.9,
                        "created_at": "t", "updated_at": "t",
                    }),
                ));
            }
        }
        // The engine caps a mutation batch at 1000 operations, so commit in
        // pages. Writing is not what these tests are about.
        for page in rows.chunks(500) {
            genesis_adapter::commit_rows(storage, page.to_vec()).unwrap();
        }
    }

    #[test]
    fn a_selected_recording_is_read_without_other_recordings() {
        // The review surface names one recording, so the other recordings in
        // the project must not enter its transcript or completeness state.
        let (path, storage) = open_storage();
        seed(&storage, 5, 400);

        let view = transcript_view(&storage, "p1", "r3").unwrap();
        assert_eq!(view.segments.len(), 400);
        assert!(view
            .segments
            .iter()
            .all(|segment| segment.recording_id == "r3"));
        assert!(!view.capped, "the selected recording is below the ceiling");
        assert!(view.capped_recording_ids.is_empty());

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_selected_recording_past_the_ceiling_is_read_whole() {
        // The engine now takes an offset, so a recording longer than one
        // engine page is read in full — ordered, complete, and unflagged.
        let (path, storage) = open_storage();
        seed(&storage, 2, genesis_adapter::ROW_CAP as i64 + 100);

        let view = transcript_view(&storage, "p1", "r1").unwrap();
        assert!(!view.capped, "a paged read has nothing to warn about");
        assert!(view.capped_recording_ids.is_empty());
        assert_eq!(view.segments.len(), genesis_adapter::ROW_CAP as usize + 100);
        // Whole and in playback order: every page landed, none twice.
        assert!(view
            .segments
            .windows(2)
            .all(|pair| pair[0].start_ms <= pair[1].start_ms));
        let ids: std::collections::HashSet<&str> = view
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect();
        assert_eq!(ids.len(), view.segments.len(), "no duplicated page rows");

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn an_intact_transcript_reports_nothing_to_warn_about() {
        // A warning that shows on a healthy transcript trains people to
        // ignore it.
        let (path, storage) = open_storage();
        seed(&storage, 1, 120);

        let view = transcript_view(&storage, "p1", "r0").unwrap();
        assert_eq!(view.segments.len(), 120);
        assert!(!view.capped);
        assert!(view.capped_recording_ids.is_empty());

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn transcript_view_rejects_a_recording_owned_by_another_project() {
        let (path, storage) = open_storage();
        seed(&storage, 1, 1);

        let error = transcript_view(&storage, "different-project", "r0").unwrap_err();

        assert!(error.to_string().contains("ไม่พบการบันทึก"));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;

    #[test]
    fn whisper_model_profiles_default_to_turbo_and_reject_reference_in_desktop() {
        assert_eq!(whisper_model_name_from(None).unwrap(), "large-v3-turbo");
        assert_eq!(
            whisper_model_name_from(Some("turbo")).unwrap(),
            "large-v3-turbo"
        );
        assert_eq!(whisper_model_name_from(Some("medium")).unwrap(), "medium");
        assert_eq!(
            whisper_model_name_from(Some(THAI_CANDIDATE_PROFILE)).unwrap(),
            THAI_CANDIDATE_MODEL
        );
        assert_eq!(
            whisper_model_backend_from(Some(THAI_CANDIDATE_PROFILE)).unwrap(),
            "transformers"
        );
        assert!(whisper_model_name_from(Some("reference")).is_err());
        assert!(whisper_model_name_from(Some("small")).is_err());
    }

    #[test]
    fn operational_profiles_select_persistent_live_worker() {
        let runtime = WhisperRuntime {
            python: PathBuf::new(),
            script: PathBuf::from("scripts").join("transcribe.py"),
            cuda_bin: PathBuf::new(),
        };

        let live_scripts: Vec<_> = ["turbo", "medium"]
            .map(|profile| whisper_worker_script_for_profile(&runtime, profile, true).unwrap())
            .into();
        assert_eq!(
            live_scripts,
            vec![PathBuf::from("scripts").join("transcribe_live.py"); 2]
        );
    }

    #[test]
    fn operational_profiles_preserve_injected_batch_script() {
        let runtime = WhisperRuntime {
            python: PathBuf::new(),
            script: PathBuf::from("fixtures").join("fake_transcribe.py"),
            cuda_bin: PathBuf::new(),
        };

        for profile in ["turbo", "medium"] {
            assert_eq!(
                whisper_worker_script_for_profile(&runtime, profile, false).unwrap(),
                runtime.script
            );
        }
    }

    #[test]
    fn thai_candidate_uses_separate_model_root_and_workers() {
        let runtime = WhisperRuntime {
            python: PathBuf::from(r"C:\Program Files\FUNG\.venv-whisper\Scripts\python.exe"),
            script: PathBuf::from(r"C:\Program Files\FUNG\scripts\transcribe.py"),
            cuda_bin: PathBuf::new(),
        };

        assert_eq!(
            bundled_whisper_model_for_profile(&runtime, THAI_CANDIDATE_PROFILE),
            Some(PathBuf::from(
                r"C:\Program Files\FUNG\.venv-whisper-transformers-candidate\models\whisper-th-large-combined"
            ))
        );
        assert_eq!(
            whisper_worker_script_for_profile(&runtime, THAI_CANDIDATE_PROFILE, false).unwrap(),
            PathBuf::from(r"C:\Program Files\FUNG\scripts\transcribe_transformers.py")
        );
        assert_eq!(
            whisper_worker_script_for_profile(&runtime, THAI_CANDIDATE_PROFILE, true).unwrap(),
            PathBuf::from(r"C:\Program Files\FUNG\scripts\transcribe_transformers_live.py")
        );
        assert_eq!(
            whisper_worker_script_for_profile(&runtime, "turbo", false).unwrap(),
            runtime.script
        );
    }

    #[test]
    fn public_release_defaults_to_cpu_and_keeps_explicit_gpu_override() {
        assert_eq!(transcription_profile_from(None).unwrap(), "cpu");
        assert_eq!(transcription_profile_from(Some("gpu")).unwrap(), "gpu");
        assert!(transcription_profile_from(Some("auto")).is_err());
    }

    #[test]
    fn portable_model_is_resolved_next_to_the_bundled_python_runtime() {
        let runtime = WhisperRuntime {
            python: PathBuf::from(r"C:\Program Files\FUNG\.venv-whisper\Scripts\python.exe"),
            script: PathBuf::from(r"C:\Program Files\FUNG\scripts\transcribe.py"),
            cuda_bin: PathBuf::new(),
        };

        assert_eq!(
            bundled_whisper_model(&runtime),
            Some(PathBuf::from(
                r"C:\Program Files\FUNG\.venv-whisper\models\large-v3-turbo"
            ))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_drive_verbatim_model_path_is_child_compatible() {
        assert_eq!(
            child_compatible_whisper_model_path(PathBuf::from(r"\\?\C:\folder\model",)),
            PathBuf::from(r"C:\folder\model")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_verbatim_unc_model_path_is_child_compatible() {
        assert_eq!(
            child_compatible_whisper_model_path(PathBuf::from(r"\\?\UNC\server\share\model",)),
            PathBuf::from(r"\\server\share\model")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_unsupported_verbatim_namespace_remains_unchanged() {
        let path = PathBuf::from(r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\model");
        assert_eq!(child_compatible_whisper_model_path(path.clone()), path);
    }

    #[test]
    fn ordinary_model_path_remains_unchanged() {
        let path = PathBuf::from(r"C:\folder\model");
        assert_eq!(child_compatible_whisper_model_path(path.clone()), path);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_model_path_remains_unchanged() {
        let path = PathBuf::from(r"\\?\C:\folder\model");
        assert_eq!(child_compatible_whisper_model_path(path.clone()), path);
    }

    #[test]
    fn worker_uses_child_compatible_bundled_model_path() {
        let runtime = WhisperRuntime {
            python: if cfg!(windows) {
                PathBuf::from(r"\\?\C:\Program Files\FUNG\.venv-whisper\Scripts\python.exe")
            } else {
                PathBuf::from("/opt/FUNG/.venv-whisper/Scripts/python")
            },
            script: PathBuf::from(r"C:\Program Files\FUNG\scripts\transcribe.py"),
            cuda_bin: PathBuf::new(),
        };

        let model = worker_whisper_model_env_path(&runtime).expect("bundled model path");
        if cfg!(windows) {
            assert_eq!(
                model,
                PathBuf::from(r"C:\Program Files\FUNG\.venv-whisper\models\large-v3-turbo")
            );
        } else {
            assert_eq!(
                model,
                PathBuf::from("/opt/FUNG/.venv-whisper/models/large-v3-turbo")
            );
        }
    }

    #[test]
    fn append_bounded_caps_length_and_keeps_latest_line() {
        let mut buffer = String::new();
        for i in 0..2000 {
            append_bounded(&mut buffer, &format!("torch warning line {i}"));
        }
        assert!(
            buffer.len() <= STDERR_TAIL_CAP_BYTES + 128,
            "buffer must stay bounded near the cap, got {} bytes",
            buffer.len()
        );
        assert!(
            buffer.contains("torch warning line 1999"),
            "must retain the most recently appended line"
        );
        assert!(
            !buffer.contains("torch warning line 0\n"),
            "oldest lines must be dropped once the cap is exceeded"
        );
    }

    #[test]
    fn primary_lan_ipv4_never_panics_and_is_non_loopback_or_none() {
        // Sandboxed/CI runners often have no route to the public internet, so
        // this must tolerate `None` — it must never panic, and any `Some`
        // must not be the loopback address.
        if let Some(ip) = primary_lan_ipv4() {
            let parsed: std::net::Ipv4Addr = ip.parse().expect("must be a valid IPv4 string");
            assert!(!parsed.is_loopback(), "must not report loopback: {ip}");
        }
    }
}

#[cfg(test)]
mod import_handoff_tests {
    use super::*;

    fn open_storage() -> (PathBuf, genesis_block_native::Storage) {
        let path =
            std::env::temp_dir().join(format!("fung-import-handoff-test-{}", Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    #[test]
    fn finalize_import_success_persists_import_handoff() {
        let (path, storage) = open_storage();
        let genesis = Arc::new(storage);
        genesis_adapter::commit_rows(
            &genesis,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "project-1",
                        "name": "Boss review",
                        "storage_path": "C:/fung/projects/project-1",
                        "active_recording_id": "previous-recording",
                        "created_at": "project-created",
                        "updated_at": "project-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "recording-1",
                        "project_id": "project-1",
                        "source": "import",
                        "input_path": "D:/incoming/meeting.m4a",
                        "canonical_audio_path": "C:/fung/projects/project-1/recording-1/audio.m4a",
                        "status": "pending",
                        "duration_ms": 0,
                        "created_at": "recording-created",
                        "updated_at": "recording-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({
                        "id": "chunk-1",
                        "recording_id": "recording-1",
                        "sequence_no": 1,
                        "file_path": "C:/fung/projects/project-1/recording-1/audio.m4a",
                        "start_ms": 0,
                        "end_ms": 0,
                        "byte_size": 9876,
                        "checksum": "sha-before",
                        "created_at": "chunk-created",
                        "transcribed_at": null,
                    }),
                ),
            ],
        )
        .unwrap();

        let output = WhisperOutput {
            duration_ms: 4321,
            segments: vec![WhisperSegment {
                start_ms: 100,
                end_ms: 900,
                text: "hello".to_string(),
                confidence: Some(0.91),
            }],
        };
        finalize_import_success(
            &genesis,
            "project-1",
            "recording-1",
            "chunk-1",
            "import",
            "D:/incoming/meeting.m4a",
            "C:/fung/projects/project-1/recording-1/audio.m4a",
            9876,
            "sha-before",
            "recording-created",
            &output,
        )
        .unwrap();

        let project = genesis_adapter::query(
            &genesis,
            "projects",
            &["name", "storage_path", "active_recording_id", "created_at"],
            vec![genesis_adapter::eq(
                "projects",
                "id",
                serde_json::json!("project-1"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(project["projects.name"], "Boss review");
        assert_eq!(
            project["projects.storage_path"],
            "C:/fung/projects/project-1"
        );
        assert_eq!(project["projects.active_recording_id"], "recording-1");
        assert_eq!(project["projects.created_at"], "project-created");

        let recording = genesis_adapter::query(
            &genesis,
            "recordings",
            &["status", "duration_ms", "canonical_audio_path"],
            vec![genesis_adapter::eq(
                "recordings",
                "id",
                serde_json::json!("recording-1"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(recording["recordings.status"], "completed");
        assert_eq!(recording["recordings.duration_ms"], 4321);
        assert_eq!(
            recording["recordings.canonical_audio_path"],
            "C:/fung/projects/project-1/recording-1/audio.m4a"
        );

        let chunk = genesis_adapter::query(
            &genesis,
            "audio_chunks",
            &[
                "file_path",
                "checksum",
                "byte_size",
                "sequence_no",
                "end_ms",
                "transcribed_at",
            ],
            vec![genesis_adapter::eq(
                "audio_chunks",
                "id",
                serde_json::json!("chunk-1"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(
            chunk["audio_chunks.file_path"],
            "C:/fung/projects/project-1/recording-1/audio.m4a"
        );
        assert_eq!(chunk["audio_chunks.checksum"], "sha-before");
        assert_eq!(chunk["audio_chunks.byte_size"], 9876);
        assert_eq!(chunk["audio_chunks.sequence_no"], 1);
        assert_eq!(chunk["audio_chunks.end_ms"], 4321);
        assert!(chunk["audio_chunks.transcribed_at"].as_str().is_some());

        drop(genesis);
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod transcript_correction_tests {
    use super::*;

    fn open_storage() -> (PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!(
            "fung-transcript-correction-test-{}",
            Uuid::new_v4()
        ));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    fn seed_segment(storage: &genesis_block_native::Storage) {
        genesis_adapter::commit_rows(
            storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "p1",
                        "name": "Meeting",
                        "storage_path": "C:/fung/projects/p1",
                        "active_recording_id": "r1",
                        "created_at": "project-created",
                        "updated_at": "project-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "r1",
                        "project_id": "p1",
                        "source": "import",
                        "input_path": "D:/incoming/meeting.wav",
                        "canonical_audio_path": "C:/fung/projects/p1/r1/meeting.wav",
                        "status": "completed",
                        "duration_ms": 1000,
                        "created_at": "recording-created",
                        "updated_at": "recording-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "transcript_segments",
                    serde_json::json!({
                        "id": "s1",
                        "project_id": "p1",
                        "recording_id": "r1",
                        "speaker_id": null,
                        "start_ms": 100,
                        "end_ms": 900,
                        "text": "ข้อความเดิม",
                        "confidence": 0.8,
                        "created_at": "segment-created",
                        "updated_at": "segment-updated",
                    }),
                ),
            ],
        )
        .unwrap();
    }

    #[test]
    fn manual_correction_updates_segment_and_records_provenance() {
        let (path, storage) = open_storage();
        seed_segment(&storage);

        correct_transcript_segment_in_storage(&storage, "p1", "r1", "s1", "ข้อความแก้ไข").unwrap();

        let segment = genesis_adapter::query(
            &storage,
            "transcript_segments",
            &["text", "updated_at"],
            vec![genesis_adapter::eq(
                "transcript_segments",
                "id",
                serde_json::json!("s1"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(segment["transcript_segments.text"], "ข้อความแก้ไข");
        assert_ne!(segment["transcript_segments.updated_at"], "segment-updated");

        let proposal = genesis_adapter::query(
            &storage,
            "transcript_refinement_proposals",
            &[
                "transcript_segment_id",
                "original_text",
                "proposed_text",
                "policy",
                "status",
            ],
            vec![genesis_adapter::eq(
                "transcript_refinement_proposals",
                "transcript_segment_id",
                serde_json::json!("s1"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(
            proposal["transcript_refinement_proposals.original_text"],
            "ข้อความเดิม"
        );
        assert_eq!(
            proposal["transcript_refinement_proposals.proposed_text"],
            "ข้อความแก้ไข"
        );
        assert_eq!(
            proposal["transcript_refinement_proposals.policy"],
            "manual_user_correction"
        );
        assert_eq!(
            proposal["transcript_refinement_proposals.status"],
            "accepted"
        );

        let audit = genesis_adapter::query(
            &storage,
            "audit_events",
            &["event_type", "payload_json"],
            vec![genesis_adapter::eq(
                "audit_events",
                "event_type",
                serde_json::json!("transcript.segment.corrected"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(
            audit["audit_events.event_type"],
            "transcript.segment.corrected"
        );
        assert_eq!(audit["audit_events.payload_json"]["recordingId"], "r1");
        assert_eq!(audit["audit_events.payload_json"]["segmentId"], "s1");

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn manual_correction_rejects_empty_text_and_wrong_recording() {
        let (path, storage) = open_storage();
        seed_segment(&storage);

        let empty =
            correct_transcript_segment_in_storage(&storage, "p1", "r1", "s1", "  ").unwrap_err();
        assert!(empty.to_string().contains("ต้องไม่ว่าง"));

        let wrong_recording =
            correct_transcript_segment_in_storage(&storage, "p1", "missing", "s1", "ใหม่")
                .unwrap_err();
        assert!(wrong_recording
            .to_string()
            .contains("ไม่พบ transcript segment"));

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod paired_device_tests {
    use super::*;

    fn test_storage() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        ensure_paired_devices_table(&conn).expect("create paired_devices table");
        conn
    }

    #[test]
    fn paired_device_roundtrip() {
        let storage = test_storage();
        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-1".into(),
                name: "Pixel".into(),
                platform: "android".into(),
                fingerprint: "ab".repeat(32),
                pairing_session_id: "sess-1".into(),
                public_key: Some("cGVlci1wdWJsaWMta2V5LWJhc2U2NA==".into()),
            },
        )
        .unwrap();

        let rows = list_paired_devices(&storage).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Pixel");
        assert_eq!(rows[0].platform, "android");
        assert!(rows[0].revoked_at.is_none());
        assert_eq!(
            rows[0].public_key.as_deref(),
            Some("cGVlci1wdWJsaWMta2V5LWJhc2U2NA==")
        );

        revoke_paired_device(&storage, "dev-1").unwrap();
        let rows = list_paired_devices(&storage).unwrap();
        assert!(rows[0].revoked_at.is_some());
    }

    #[test]
    fn ensure_paired_devices_table_backfills_public_key_column_on_legacy_db() {
        // Simulates a paired_devices.db created before this task: the table
        // exists but has no public_key column. ensure_paired_devices_table
        // must ALTER it in place rather than relying on CREATE TABLE IF NOT
        // EXISTS (which is a no-op once the table already exists).
        let storage = Connection::open_in_memory().expect("open in-memory sqlite");
        storage
            .execute_batch(
                r#"
                CREATE TABLE paired_devices (
                  id TEXT PRIMARY KEY,
                  name TEXT NOT NULL,
                  platform TEXT NOT NULL,
                  fingerprint TEXT NOT NULL,
                  paired_at TEXT NOT NULL,
                  revoked_at TEXT,
                  pairing_session_id TEXT NOT NULL
                );
                "#,
            )
            .unwrap();

        ensure_paired_devices_table(&storage).expect("backfill public_key column");

        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-legacy".into(),
                name: "Legacy Pixel".into(),
                platform: "android".into(),
                fingerprint: "cd".repeat(32),
                pairing_session_id: "sess-legacy".into(),
                public_key: Some("bGVnYWN5LXB1YmxpYy1rZXk=".into()),
            },
        )
        .unwrap();
        let rows = list_paired_devices(&storage).unwrap();
        assert_eq!(
            rows[0].public_key.as_deref(),
            Some("bGVnYWN5LXB1YmxpYy1rZXk=")
        );
    }

    #[test]
    fn upsert_revives_a_revoked_device_and_keeps_original_pairing_session() {
        let storage = test_storage();
        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-1".into(),
                name: "Pixel".into(),
                platform: "android".into(),
                fingerprint: "ab".repeat(32),
                pairing_session_id: "sess-1".into(),
                public_key: None,
            },
        )
        .unwrap();
        revoke_paired_device(&storage, "dev-1").unwrap();

        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-1".into(),
                name: "Pixel 9".into(),
                platform: "android".into(),
                fingerprint: "ab".repeat(32),
                pairing_session_id: "sess-2".into(),
                public_key: None,
            },
        )
        .unwrap();

        let rows = list_paired_devices(&storage).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Pixel 9");
        assert!(rows[0].revoked_at.is_none());
        assert_eq!(
            rows[0].pairing_session_id, "sess-1",
            "re-pairing revives the row but does not overwrite the original pairing_session_id"
        );
    }

    #[test]
    fn list_is_ordered_newest_paired_first() {
        let storage = test_storage();
        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-1".into(),
                name: "First".into(),
                platform: "android".into(),
                fingerprint: "aa".repeat(32),
                pairing_session_id: "sess-1".into(),
                public_key: None,
            },
        )
        .unwrap();
        storage
            .execute(
                "UPDATE paired_devices SET paired_at = '2020-01-01T00:00:00Z' WHERE id = 'dev-1'",
                [],
            )
            .unwrap();
        upsert_paired_device(
            &storage,
            PairedDeviceInput {
                id: "dev-2".into(),
                name: "Second".into(),
                platform: "ios".into(),
                fingerprint: "bb".repeat(32),
                pairing_session_id: "sess-2".into(),
                public_key: None,
            },
        )
        .unwrap();

        let rows = list_paired_devices(&storage).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].id, "dev-2",
            "most recently paired device is listed first"
        );
    }
}
