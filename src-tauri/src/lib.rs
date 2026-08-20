use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    env,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use thiserror::Error;
use uuid::Uuid;

mod audio_custody;
mod backup;
mod backup_archive;
mod backup_payload;
mod cloud_commands;
mod cloud_config;
mod cloud_executor;
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
mod local_diarization;
mod media_fetch;
mod meeting_intel;
mod mobile;
mod native_recorder;
mod on_device_ai;
mod policy;
mod recovery;
mod speaker_merge;
mod transcript_export;
mod tts_config;
mod tts_executor;
mod zoom_sync;

/// Source-tree fallback used by `tauri dev`. Packaged builds must resolve all
/// worker resources from the installed application's resource directory.
const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

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

fn bundled_whisper_model(runtime: &WhisperRuntime) -> Option<PathBuf> {
    let runtime_root = runtime.python.parent()?.parent()?;
    Some(runtime_root.join("models").join("small"))
}

/// Opens the configured FUNG account portal in the system browser. OAuth is
/// intentionally completed by the hosted web surface; the embedded local
/// runtime never receives a provider client secret.
#[tauri::command]
fn open_external_account_portal(app: tauri::AppHandle) -> AppResult<()> {
    let url = env::var("FUNG_WEB_APP_URL")
        .map_err(|_| AppError::InvalidInput("FUNG_WEB_APP_URL is not configured".to_string()))?;
    let url = url.trim();
    if !url.starts_with("https://") {
        return Err(AppError::InvalidInput(
            "FUNG_WEB_APP_URL must use https".to_string(),
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| AppError::InvalidInput(format!("could not open account portal: {error}")))
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
    pub(crate) genesis: Arc<genesis_block_native::Storage>,
    pub(crate) genesis_path: PathBuf,
    local_api: Mutex<Option<String>>,
    whisper_runtime: WhisperRuntime,
    pub(crate) mobile_gateway: Mutex<Option<mobile::MobileGatewayControl>>,
    pub(crate) fungwire: Mutex<Option<fungwire_server::FungwireServerControl>>,
    pub(crate) live: Mutex<Option<live_meeting::LiveSessionControl>>,
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

#[tauri::command]
fn paired_device_upsert(device: PairedDeviceInput, state: State<'_, AppState>) -> AppResult<()> {
    let conn = paired_devices_connection(&state)?;
    upsert_paired_device(&conn, device)
}

#[tauri::command]
fn paired_device_list(state: State<'_, AppState>) -> AppResult<Vec<PairedDeviceRow>> {
    let conn = paired_devices_connection(&state)?;
    list_paired_devices(&conn)
}

#[tauri::command]
fn paired_device_revoke(id: String, state: State<'_, AppState>) -> AppResult<()> {
    let conn = paired_devices_connection(&state)?;
    revoke_paired_device(&conn, &id)
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
#[tauri::command]
fn fungwire_local_endpoint(state: State<'_, AppState>) -> AppResult<Option<String>> {
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
    conn.execute(
        r#"
        INSERT OR IGNORE INTO model_providers
            (id, label, runtime_location, kind, enabled, config_json, created_at, updated_at)
        VALUES
            ('ollama-summary-intent', 'Ollama / llama.cpp', 'local', 'summary_intent', 1, '{"endpoint":"http://127.0.0.1:11434"}', ?1, ?1),
            ('vllm-summary-intent', 'vLLM', 'local', 'summary_intent', 0, '{"endpoint":"http://127.0.0.1:8000"}', ?1, ?1)
        "#,
        params![inserted_at],
    )?;

    Ok(conn)
}

fn app_state(app: &tauri::App) -> AppResult<AppState> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::MissingAppDataDir)?;
    let legacy_db_path = app_data_dir.join("fung.db");
    let genesis_path = app_data_dir.join("genesisdb");
    let genesis = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
        path: genesis_path.display().to_string(),
        page_cache_mb: Some(64),
        read_only: Some(false),
        vector_dim: Some(384),
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
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":"http://127.0.0.1:11434"},"created_at":seeded_at,"updated_at":seeded_at})),
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"vllm-summary-intent","label":"vLLM","runtime_location":"local","kind":"summary_intent","enabled":false,"config_json":{"endpoint":"http://127.0.0.1:8000"},"created_at":seeded_at,"updated_at":seeded_at})),
    ]).map_err(AppError::Genesis)?;
    let genesis = Arc::new(genesis);
    Ok(AppState {
        data_root: app_data_dir,
        jobs: job_engine::JobEngine::new(Arc::clone(&genesis)),
        genesis,
        genesis_path,
        local_api: Mutex::new(None),
        whisper_runtime: whisper_runtime(app),
        mobile_gateway: Mutex::new(None),
        fungwire: Mutex::new(None),
        live: Mutex::new(None),
        external_mcp: external_mcp_commands::ExternalMcpRuntime::default(),
        external_meeting_tools_enabled: matches!(
            env::var("FUNG_EXTERNAL_MEETING_TOOLS").as_deref(),
            Ok("1")
        ),
    })
}

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> AppResult<Health> {
    let bind = state
        .local_api
        .lock()
        .expect("local api mutex poisoned")
        .clone();

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
    let storage_path = state
        .data_root
        .join("projects")
        .join(&id)
        .display()
        .to_string();
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
        1000,
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
    let mut jobs = genesis_adapter::query(
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
        1000,
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

#[tauri::command]
fn list_transcript_segments(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<TranscriptSegment>> {
    // Resolve speaker display names once per call: query the project's
    // speakers (capped like every other query against this engine) and build
    // an id -> display_name map, rather than a lookup per segment.
    let speaker_rows = genesis_adapter::query(
        &state.genesis,
        "speakers",
        &["id", "display_name"],
        vec![genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id.clone()),
        )],
        1000,
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
    let mut segments = genesis_adapter::query(
        &state.genesis,
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
        vec![genesis_adapter::eq(
            "transcript_segments",
            "project_id",
            serde_json::json!(project_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        let speaker_id = row
            .get("transcript_segments.speaker_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let speaker_name = speaker_id
            .as_ref()
            .and_then(|id| speaker_names.get(id).cloned());
        Ok(TranscriptSegment {
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
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    segments.sort_by_key(|segment| segment.start_ms);
    Ok(segments)
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
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let storage_path = state
        .data_root
        .join("projects")
        .join(&id)
        .display()
        .to_string();
    genesis_adapter::commit_rows(&state.genesis, vec![genesis_adapter::upsert("projects", serde_json::json!({"id":id,"name":default_name,"storage_path":storage_path,"active_recording_id":null,"created_at":timestamp,"updated_at":timestamp}))]).map_err(AppError::Genesis)?;
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

    let registered = genesis_adapter::commit_rows(genesis, vec![
        genesis_adapter::upsert("recordings", serde_json::json!({"id":recording_id,"project_id":project_id,"source":source,"input_path":input_path,"canonical_audio_path":stored_path,"status":"pending","duration_ms":0,"created_at":timestamp,"updated_at":timestamp})),
        // One chunk covering the whole file, so an import is backed up and
        // integrity-checked by exactly the same paths as a live capture.
        // `end_ms` is filled in once transcription reports the duration.
        genesis_adapter::upsert("audio_chunks", serde_json::json!({"id":Uuid::new_v4().to_string(),"recording_id":recording_id,"sequence_no":1,"file_path":stored_path,"start_ms":0,"end_ms":0,"byte_size":custodied.byte_size,"checksum":custodied.sha256,"created_at":timestamp})),
    ]);
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
            let insert_result = (|| -> AppResult<()> {
                let timestamp = now();
                let mut mutations = Vec::new();
                for segment in &output.segments {
                    let seg_timestamp = now();
                    mutations.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({"id":Uuid::new_v4().to_string(),"project_id":project_id,"recording_id":recording_id,"speaker_id":null,"start_ms":segment.start_ms,"end_ms":segment.end_ms,"text":segment.text,"confidence":segment.confidence,"created_at":seg_timestamp,"updated_at":seg_timestamp})));
                }
                mutations.push(genesis_adapter::upsert("recordings", serde_json::json!({"id":recording_id,"project_id":project_id,"source":source,"input_path":input_path,"canonical_audio_path":stored_path,"status":"completed","duration_ms":output.duration_ms,"created_at":timestamp,"updated_at":timestamp})));
                genesis_adapter::commit_rows(genesis, mutations).map_err(AppError::Genesis)?;
                Ok(())
            })();

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
    let staging = state.data_root.join("fetch").join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&staging)
        .map_err(|err| AppError::InvalidInput(format!("could not prepare the fetch directory: {err}")))?;

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
        &["id", "name", "storage_path", "active_recording_id", "created_at"],
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
    if let Some(model) = bundled_whisper_model(runtime) {
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
        // the string "small", which it resolves by downloading from
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
    let profile = transcription_profile()?;

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
        &runtime.script,
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

#[tauri::command]
fn start_local_api(state: State<'_, AppState>) -> AppResult<String> {
    let mut current = state.local_api.lock().expect("local api mutex poisoned");
    if let Some(bind) = current.clone() {
        return Ok(bind);
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    let storage = state.genesis.clone();
    let genesis_path = state.genesis_path.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_api_stream(stream, &storage, &genesis_path),
                Err(_) => break,
            }
        }
    });

    *current = Some(bind.clone());
    Ok(bind)
}

fn handle_api_stream(
    mut stream: TcpStream,
    storage: &genesis_block_native::Storage,
    genesis_path: &std::path::Path,
) {
    let mut buffer = [0; 1024];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first_line = request.lines().next().unwrap_or_default();

    let (status, body) = if first_line.starts_with("GET /health ") {
        (
            "200 OK",
            serde_json::json!({
                "app": "FUNG",
                "version": env!("CARGO_PKG_VERSION"),
                "databasePath": genesis_path.display().to_string(),
                "storageAuthority": "GenesisBlockDB signed WAL",
                "stableFrontier": storage.stable_frontier()
            })
            .to_string(),
        )
    } else {
        (
            "404 Not Found",
            serde_json::json!({
                "error": "not found",
                "available": ["/health"]
            })
            .to_string(),
        )
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

/// One-shot loopback HTTP listener used as a fallback when the `fung://`
/// deep link fails to activate the app (some desktop environments never wire
/// custom protocol handlers correctly). The browser is redirected here after
/// OAuth completes; the first request's full URL is forwarded to the webview
/// via the `auth-callback` event, then the listener thread exits.
#[tauri::command]
fn auth_loopback_listen(app: tauri::AppHandle) -> AppResult<u16> {
    use tauri::Emitter;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_string();
            let body = "<html><body style=\"font-family:sans-serif\"><p>เข้าสู่ระบบสำเร็จ ปิดหน้าต่างนี้ได้เลย</p></body></html>";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let full_url = format!("http://127.0.0.1:{port}{path}");
            let _ = app.emit("auth-callback", full_url);
        }
    });
    Ok(port)
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
            serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":"http://127.0.0.1:11434"},"created_at":seeded_at,"updated_at":seeded_at}),
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
                serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":"http://127.0.0.1:11434"},"created_at":timestamp,"updated_at":timestamp}),
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
    let (chunk_tx, chunk_rx) = std::sync::mpsc::channel();

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
            CaptureEvent::ChunkWriteFailed { channel, error } => {
                report.push_str(&format!(
                    "chunk write failed on {channel}: {error}
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            import_and_transcribe,
            fetch_and_transcribe,
            media_fetch_status,
            media_fetch_consent_set,
            transcript_export::list_export_artifacts,
            audio_integrity_check,
            recovery_scan,
            recovery_recover,
            start_local_api,
            auth_loopback_listen,
            open_external_account_portal,
            paired_device_upsert,
            paired_device_list,
            paired_device_revoke,
            device_identity::device_identity_ensure,
            device_identity::device_public_key,
            zoom_sync::zoom_connect,
            zoom_sync::zoom_connection_status,
            zoom_sync::zoom_disconnect,
            zoom_sync::zoom_list_recordings,
            zoom_sync::zoom_import_recording,
            graph_build::graph_build_start,
            diarization::diarization_status,
            live_meeting::live_meeting_start,
            live_meeting::live_meeting_stop,
            live_meeting::live_meeting_status,
            meeting_intel::meeting_ask,
            meeting_intel::meeting_summaries,
            meeting_intel::generate_meeting_summary,
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
            native_recorder::mobile_native_recorder_start,
            native_recorder::mobile_native_recorder_status,
            native_recorder::mobile_native_recorder_control,
            on_device_ai::mobile_on_device_ai_status,
            mobile::mobile_note_upsert,
            mobile::mobile_relation_upsert,
            mobile::mobile_graph_query,
            mobile::mobile_timeline_query,
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
            fungwire_server::fungwire_server_set_enabled,
            fungwire_server::fungwire_status,
            fungwire_local_endpoint,
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
        .run(|app, event| {
            // Stop taking new work at exit. Anything still queued stays
            // queued in the ledger and is adopted on the next launch —
            // shutdown is not cancellation.
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                if let Some(state) = app.try_state::<AppState>() {
                    state.jobs.shutdown();
                }
            }
        });
}

#[cfg(test)]
mod worker_tests {
    use super::*;

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
                r"C:\Program Files\FUNG\.venv-whisper\models\small"
            ))
        );
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
