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

mod cloud_config;
mod cloud_executor;
mod device_identity;
mod fungwire;
mod fungwire_client;
mod fungwire_server;
mod genesis_adapter;
mod graph_build;
mod mobile;
mod native_recorder;
mod on_device_ai;
mod policy;
mod speaker_merge;
mod tts_config;
mod tts_executor;
mod zoom_sync;

/// Source-tree fallback used by `tauri dev`. Packaged builds must resolve all
/// worker resources from the installed application's resource directory.
const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Clone)]
pub(crate) struct WhisperRuntime {
    python: PathBuf,
    pub(crate) script: PathBuf,
    cuda_bin: PathBuf,
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
    /// when no system python is on PATH.
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
/// resolves, so local test runs are unaffected either way.
#[cfg(test)]
fn resolve_test_python() -> PathBuf {
    let (finder, names): (&str, &[&str]) = if cfg!(windows) {
        ("where", &["python", "python3"])
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

const REQUIRED_CUDA_DLLS: [&str; 4] = [
    "cudart64_12.dll",
    "cublas64_12.dll",
    "cublasLt64_12.dll",
    "cudnn64_9.dll",
];

fn transcription_profile() -> Result<String, String> {
    let profile = env::var("FUNG_TRANSCRIPTION_PROFILE").unwrap_or_else(|_| "gpu".to_string());
    match profile.as_str() {
        "gpu" | "cpu" => Ok(profile),
        _ => Err(format!(
            "invalid FUNG_TRANSCRIPTION_PROFILE '{profile}'; use 'gpu' or 'cpu'"
        )),
    }
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
        genesis_adapter::import_legacy_sqlite(&genesis, &legacy_db_path).map_err(AppError::Genesis)?;
        std::fs::write(&legacy_marker, now())?;
    }
    let seeded_at = now();
    genesis_adapter::commit_rows(&genesis, vec![
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"ollama-summary-intent","label":"Ollama / llama.cpp","runtime_location":"local","kind":"summary_intent","enabled":true,"config_json":{"endpoint":"http://127.0.0.1:11434"},"created_at":seeded_at,"updated_at":seeded_at})),
        genesis_adapter::upsert("model_providers", serde_json::json!({"id":"vllm-summary-intent","label":"vLLM","runtime_location":"local","kind":"summary_intent","enabled":false,"config_json":{"endpoint":"http://127.0.0.1:8000"},"created_at":seeded_at,"updated_at":seeded_at})),
    ]).map_err(AppError::Genesis)?;
    Ok(AppState {
        data_root: app_data_dir,
        genesis: Arc::new(genesis),
        genesis_path,
        local_api: Mutex::new(None),
        whisper_runtime: whisper_runtime(app),
        mobile_gateway: Mutex::new(None),
        fungwire: Mutex::new(None),
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
    let mut projects = genesis_adapter::query(&state.genesis, "projects", &["id", "name", "storage_path", "active_recording_id", "created_at", "updated_at"], vec![], 1000).map_err(AppError::Genesis)?.into_iter().map(|row| Ok(Project { id: genesis_adapter::string(&row, "projects.id").map_err(AppError::Genesis)?, name: genesis_adapter::string(&row, "projects.name").map_err(AppError::Genesis)?, storage_path: genesis_adapter::string(&row, "projects.storage_path").map_err(AppError::Genesis)?, active_recording_id: row.get("projects.active_recording_id").and_then(serde_json::Value::as_str).map(str::to_owned), created_at: genesis_adapter::string(&row, "projects.created_at").map_err(AppError::Genesis)?, updated_at: genesis_adapter::string(&row, "projects.updated_at").map_err(AppError::Genesis)? })).collect::<AppResult<Vec<_>>>()?;
    projects.sort_by_key(|project| std::cmp::Reverse((project.updated_at.clone(), project.created_at.clone())));
    Ok(projects)
}

#[tauri::command]
fn create_job(
    job_type: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Job> {
    let trimmed = job_type.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("job type is required".to_string()));
    }
    let project_id = project_id.ok_or_else(|| {
        AppError::InvalidInput("project id is required for stateful jobs".to_string())
    })?;

    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id": id, "project_id": project_id, "type": trimmed, "status": "queued", "progress": 0, "input_refs_json": [], "output_refs_json": [], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": null, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "job_id": id, "status": "queued", "message": "Job queued", "created_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

    Ok(Job {
        id,
        project_id,
        job_type: trimmed.to_string(),
        status: "queued".to_string(),
        progress: 0,
        input_refs: Vec::new(),
        output_refs: Vec::new(),
        provider_id: None,
        error_code: None,
        error_message: None,
        started_at: None,
        finished_at: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

#[tauri::command]
fn list_jobs(state: State<'_, AppState>) -> AppResult<Vec<Job>> {
    let mut jobs = genesis_adapter::query(&state.genesis, "jobs", &["id", "project_id", "type", "status", "progress", "input_refs_json", "output_refs_json", "provider_id", "error_code", "error_message", "started_at", "finished_at", "created_at", "updated_at"], vec![], 1000).map_err(AppError::Genesis)?.into_iter().map(job_from_row).collect::<AppResult<Vec<_>>>()?;
    jobs.sort_by_key(|job| std::cmp::Reverse(job.created_at.clone())); jobs.truncate(30); Ok(jobs)
}

fn job_from_row(row: serde_json::Value) -> AppResult<Job> {
    let json_list = |key: &str| row.get(key).and_then(serde_json::Value::as_str).and_then(|value| serde_json::from_str(value).ok()).unwrap_or_default();
    let optional = |key: &str| row.get(key).and_then(serde_json::Value::as_str).map(str::to_owned);
    Ok(Job { id: genesis_adapter::string(&row, "jobs.id").map_err(AppError::Genesis)?, project_id: genesis_adapter::string(&row, "jobs.project_id").map_err(AppError::Genesis)?, job_type: genesis_adapter::string(&row, "jobs.type").map_err(AppError::Genesis)?, status: genesis_adapter::string(&row, "jobs.status").map_err(AppError::Genesis)?, progress: genesis_adapter::integer(&row, "jobs.progress").map_err(AppError::Genesis)?, input_refs: json_list("jobs.input_refs_json"), output_refs: json_list("jobs.output_refs_json"), provider_id: optional("jobs.provider_id"), error_code: optional("jobs.error_code"), error_message: optional("jobs.error_message"), started_at: optional("jobs.started_at"), finished_at: optional("jobs.finished_at"), created_at: genesis_adapter::string(&row, "jobs.created_at").map_err(AppError::Genesis)?, updated_at: genesis_adapter::string(&row, "jobs.updated_at").map_err(AppError::Genesis)? })
}

#[tauri::command]
fn list_model_providers(state: State<'_, AppState>) -> AppResult<Vec<ModelProvider>> {
    let mut providers = genesis_adapter::query(&state.genesis, "model_providers", &["id", "label", "runtime_location", "kind", "enabled", "created_at", "updated_at"], vec![], 500).map_err(AppError::Genesis)?.into_iter().map(|row| Ok(ModelProvider { id: genesis_adapter::string(&row, "model_providers.id").map_err(AppError::Genesis)?, label: genesis_adapter::string(&row, "model_providers.label").map_err(AppError::Genesis)?, runtime_location: genesis_adapter::string(&row, "model_providers.runtime_location").map_err(AppError::Genesis)?, kind: genesis_adapter::string(&row, "model_providers.kind").map_err(AppError::Genesis)?, enabled: row.get("model_providers.enabled").and_then(|value| value.as_bool().or_else(|| value.as_i64().map(|number| number != 0))).unwrap_or(false), created_at: genesis_adapter::string(&row, "model_providers.created_at").map_err(AppError::Genesis)?, updated_at: genesis_adapter::string(&row, "model_providers.updated_at").map_err(AppError::Genesis)? })).collect::<AppResult<Vec<_>>>()?;
    providers.sort_by_key(|provider| (provider.runtime_location.clone(), provider.label.clone())); Ok(providers)
}

fn model_provider_row_enabled(row: &serde_json::Value) -> bool {
    row.get("model_providers.enabled")
        .and_then(|value| value.as_bool().or_else(|| value.as_i64().map(|number| number != 0)))
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
        &["id", "label", "runtime_location", "kind", "enabled", "config_json", "created_at", "updated_at"],
        vec![genesis_adapter::eq("model_providers", "id", serde_json::json!(input.provider_id))],
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
        None => genesis_adapter::string(row, "model_providers.config_json").map_err(AppError::Genesis)?,
    };
    let runtime_location = genesis_adapter::string(row, "model_providers.runtime_location").map_err(AppError::Genesis)?;
    let kind = genesis_adapter::string(row, "model_providers.kind").map_err(AppError::Genesis)?;
    let enabled = model_provider_row_enabled(row);
    let created_at = genesis_adapter::string(row, "model_providers.created_at").map_err(AppError::Genesis)?;

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

    Ok(tts_config::TtsValidation { ok: true, error: None, warnings: vec![] })
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
        &["id", "label", "runtime_location", "kind", "enabled", "config_json", "created_at", "updated_at"],
        vec![genesis_adapter::eq("model_providers", "id", serde_json::json!(provider_id))],
        1,
    )
    .map_err(AppError::Genesis)?;

    let row = rows
        .first()
        .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบ provider: {provider_id}")))?;

    let label = genesis_adapter::string(row, "model_providers.label").map_err(AppError::Genesis)?;
    let runtime_location = genesis_adapter::string(row, "model_providers.runtime_location").map_err(AppError::Genesis)?;
    let kind = genesis_adapter::string(row, "model_providers.kind").map_err(AppError::Genesis)?;
    let config_json = genesis_adapter::string(row, "model_providers.config_json").map_err(AppError::Genesis)?;
    let created_at = genesis_adapter::string(row, "model_providers.created_at").map_err(AppError::Genesis)?;

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

    let config_str = genesis_adapter::string(row, "model_providers.config_json").map_err(AppError::Genesis)?;
    let config: tts_config::TtsProviderConfig = serde_json::from_str(&config_str)
        .map_err(|e| AppError::InvalidInput(format!("config ไม่ถูกรูปแบบ: {e}")))?;

    let temp_dir = std::env::temp_dir().join("fung-tts");
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        AppError::Io(std::io::Error::new(e.kind(), format!("สร้าง temp dir ไม่ได้: {e}")))
    })?;

    let request = tts_executor::TtsSynthesisRequest {
        text,
        ref_audio: None,
        ref_text: None,
    };

    let (status, latency_ms, audio_path, message) = match tts_executor::dispatch(&config, &request, &temp_dir) {
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

    Ok(TtsTestOutput { status, latency_ms, audio_path, message })
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
        AppError::Io(std::io::Error::new(e.kind(), format!("สร้าง temp dir ไม่ได้: {e}")))
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
    let speaker_rows = genesis_adapter::query(&state.genesis, "speakers", &["id", "display_name"],
        vec![genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id.clone()))], 1000).map_err(AppError::Genesis)?;
    let speaker_names: std::collections::HashMap<String, String> = speaker_rows.into_iter().filter_map(|row| {
        Some((row.get("speakers.id")?.as_str()?.to_string(), row.get("speakers.display_name")?.as_str()?.to_string()))
    }).collect();
    let mut segments = genesis_adapter::query(&state.genesis, "transcript_segments", &["id", "project_id", "recording_id", "speaker_id", "start_ms", "end_ms", "text", "confidence", "created_at"], vec![genesis_adapter::eq("transcript_segments", "project_id", serde_json::json!(project_id))], 1000).map_err(AppError::Genesis)?.into_iter().map(|row| {
        let speaker_id = row.get("transcript_segments.speaker_id").and_then(serde_json::Value::as_str).map(str::to_owned);
        let speaker_name = speaker_id.as_ref().and_then(|id| speaker_names.get(id).cloned());
        Ok(TranscriptSegment {
            id: genesis_adapter::string(&row, "transcript_segments.id").map_err(AppError::Genesis)?,
            project_id: genesis_adapter::string(&row, "transcript_segments.project_id").map_err(AppError::Genesis)?,
            recording_id: genesis_adapter::string(&row, "transcript_segments.recording_id").map_err(AppError::Genesis)?,
            speaker_id,
            speaker_name,
            start_ms: genesis_adapter::integer(&row, "transcript_segments.start_ms").map_err(AppError::Genesis)?,
            end_ms: genesis_adapter::integer(&row, "transcript_segments.end_ms").map_err(AppError::Genesis)?,
            text: genesis_adapter::string(&row, "transcript_segments.text").map_err(AppError::Genesis)?,
            confidence: row.get("transcript_segments.confidence").and_then(serde_json::Value::as_f64),
            created_at: genesis_adapter::string(&row, "transcript_segments.created_at").map_err(AppError::Genesis)?,
        })
    }).collect::<AppResult<Vec<_>>>()?;
    segments.sort_by_key(|segment| segment.start_ms); Ok(segments)
}

pub(crate) fn set_job_status(
    storage: &genesis_block_native::Storage,
    job_id: &str,
    status: &str,
    progress: Option<i64>,
    error_message: Option<&str>,
) -> AppResult<()> {
    let timestamp = now();
    let row = genesis_adapter::query(storage, "jobs", &["project_id", "type", "status", "progress", "input_refs_json", "output_refs_json", "provider_id", "error_code", "error_message", "attempt_no", "started_at", "finished_at", "created_at"], vec![genesis_adapter::eq("jobs", "id", serde_json::json!(job_id))], 1).map_err(AppError::Genesis)?.into_iter().next().ok_or_else(|| AppError::InvalidInput("job not found".to_string()))?;
    let optional = |key: &str| row.get(key).cloned().unwrap_or(serde_json::Value::Null);
    let started_at = if status == "running" && optional("jobs.started_at").is_null() { serde_json::Value::String(timestamp.clone()) } else { optional("jobs.started_at") };
    let finished_at = if matches!(status, "completed" | "failed") { serde_json::Value::String(timestamp.clone()) } else { optional("jobs.finished_at") };
    genesis_adapter::commit_rows(storage, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id":job_id,"project_id":genesis_adapter::string(&row,"jobs.project_id").map_err(AppError::Genesis)?,"type":genesis_adapter::string(&row,"jobs.type").map_err(AppError::Genesis)?,"status":status,"progress":progress.unwrap_or(genesis_adapter::integer(&row,"jobs.progress").map_err(AppError::Genesis)?),"input_refs_json":optional("jobs.input_refs_json"),"output_refs_json":optional("jobs.output_refs_json"),"provider_id":optional("jobs.provider_id"),"error_code":optional("jobs.error_code"),"error_message":error_message.map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),"attempt_no":genesis_adapter::integer(&row,"jobs.attempt_no").map_err(AppError::Genesis)?,"started_at":started_at,"finished_at":finished_at,"created_at":genesis_adapter::string(&row,"jobs.created_at").map_err(AppError::Genesis)?,"updated_at":timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id":Uuid::new_v4().to_string(),"job_id":job_id,"status":status,"message":error_message.unwrap_or(status),"created_at":timestamp})),
    ]).map_err(AppError::Genesis)?;
    Ok(())
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
    let project_id = match project_id {
        Some(id) => id,
        None => {
            let id = Uuid::new_v4().to_string();
            let timestamp = now();
            let name = PathBuf::from(&file_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Imported session".to_string());
            let storage_path = state
                .data_root
                .join("projects")
                .join(&id)
                .display()
                .to_string();
            genesis_adapter::commit_rows(&state.genesis, vec![genesis_adapter::upsert("projects", serde_json::json!({"id":id,"name":name,"storage_path":storage_path,"active_recording_id":null,"created_at":timestamp,"updated_at":timestamp}))]).map_err(AppError::Genesis)?;
            id
        }
    };

    let recording_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let job_id = Uuid::new_v4().to_string();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("recordings", serde_json::json!({"id":recording_id,"project_id":project_id,"source":"import","input_path":file_path,"canonical_audio_path":file_path,"status":"pending","duration_ms":0,"created_at":timestamp,"updated_at":timestamp})),
        genesis_adapter::upsert("jobs", serde_json::json!({"id":job_id,"project_id":project_id,"type":"transcript.transcribe","status":"running","progress":0,"input_refs_json":[file_path],"output_refs_json":[],"provider_id":null,"error_code":null,"error_message":null,"attempt_no":1,"started_at":timestamp,"finished_at":null,"created_at":timestamp,"updated_at":timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id":Uuid::new_v4().to_string(),"job_id":job_id,"status":"running","message":"running","created_at":timestamp})),
    ]).map_err(AppError::Genesis)?;

    let worker_storage = state.genesis.clone();
    let worker_job_id = job_id.clone();
    let worker_project_id = project_id.clone();
    let worker_recording_id = recording_id.clone();
    let worker_file_path = file_path.clone();
    let worker_runtime = state.whisper_runtime.clone();

    thread::spawn(move || {
        let progress_storage = worker_storage.clone();
        let progress_job_id = worker_job_id.clone();
        let outcome = run_transcription(&worker_runtime, &worker_file_path, move |pct| {
            let _ = set_job_status(&progress_storage, &progress_job_id, "running", Some(pct), None);
        });

        match outcome {
            Ok(output) => {
                let insert_result = (|| -> AppResult<()> {
                    let timestamp = now();
                    let mut mutations = Vec::new();
                    for segment in &output.segments {
                        let seg_timestamp = now();
                        mutations.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({"id":Uuid::new_v4().to_string(),"project_id":worker_project_id,"recording_id":worker_recording_id,"speaker_id":null,"start_ms":segment.start_ms,"end_ms":segment.end_ms,"text":segment.text,"confidence":segment.confidence,"created_at":seg_timestamp,"updated_at":seg_timestamp})));
                    }
                    mutations.push(genesis_adapter::upsert("recordings", serde_json::json!({"id":worker_recording_id,"project_id":worker_project_id,"source":"import","input_path":worker_file_path,"canonical_audio_path":worker_file_path,"status":"completed","duration_ms":output.duration_ms,"created_at":timestamp,"updated_at":timestamp})));
                    genesis_adapter::commit_rows(&worker_storage, mutations).map_err(AppError::Genesis)?;
                    Ok(())
                })();

                match insert_result {
                    Ok(()) => {
                        let _ = set_job_status(&worker_storage, &worker_job_id, "completed", Some(100), None);
                    }
                    Err(err) => {
                        let _ = set_job_status(
                            &worker_storage,
                            &worker_job_id,
                            "failed",
                            None,
                            Some(&err.to_string()),
                        );
                    }
                }
            }
            Err(message) => {
                let _ = set_job_status(&worker_storage, &worker_job_id, "failed", None, Some(&message));
            }
        }
    });

    Ok(Job {
        id: job_id,
        project_id,
        job_type: "transcript.transcribe".to_string(),
        status: "running".to_string(),
        progress: 0,
        input_refs: vec![file_path],
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
fn append_bounded(buffer: &mut String, line: &str) {
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
        on_progress,
    )?;

    serde_json::from_str::<WhisperOutput>(raw_output.trim())
        .map_err(|err| format!("failed to parse transcription output: {err}"))
}

/// Runs the pyannote diarization worker script. Path B calls this after
/// transcribing the mixed file; a failure here must never take down the
/// transcript (see `zoom_sync::run_mixed_audio_path`).
pub(crate) fn run_diarization(
    runtime: &WhisperRuntime,
    file_path: &str,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<zoom_sync::DiarizeOutput, String> {
    let script = runtime.script.parent().expect("scripts dir").join("diarize.py");
    let raw = run_python_worker(runtime, &script, &[file_path], None, on_progress)?;
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

fn handle_api_stream(mut stream: TcpStream, storage: &genesis_block_native::Storage, genesis_path: &std::path::Path) {
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
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            create_project,
            list_projects,
            create_job,
            list_jobs,
            list_model_providers,
            list_transcript_segments,
            import_and_transcribe,
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
            fungwire_client::fungwire_delegate_transcription,
            fungwire_client::fungwire_job_poll,
            tts_provider_register,
            tts_provider_update,
            tts_provider_toggle,
            tts_provider_test,
            tts_synthesize_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running FUNG");
}

#[cfg(test)]
mod worker_tests {
    use super::*;

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
        match primary_lan_ipv4() {
            Some(ip) => {
                let parsed: std::net::Ipv4Addr = ip.parse().expect("must be a valid IPv4 string");
                assert!(!parsed.is_loopback(), "must not report loopback: {ip}");
            }
            None => {}
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
        assert_eq!(rows[0].public_key.as_deref(), Some("bGVnYWN5LXB1YmxpYy1rZXk="));
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
        assert_eq!(rows[0].id, "dev-2", "most recently paired device is listed first");
    }
}
