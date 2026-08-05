//! Zoom cloud-recording ingestion: OAuth (PKCE), REST pulls, import job.
//! Tokens live ONLY in the OS credential store (keyring). Never persist or
//! log tokens or download URLs.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::{genesis_adapter, now, AppError, AppResult, AppState};

const KEYRING_SERVICE: &str = "FUNG";
const KEYRING_USER: &str = "zoom-oauth";
const ZOOM_AUTH_BASE: &str = "https://zoom.us";
pub(crate) const ZOOM_API_BASE: &str = "https://api.zoom.us/v2";

/// Runaway guard only — a real 30-day window never approaches this. Hitting it
/// is reported as an error rather than silently truncating the list.
const MAX_RECORDING_PAGES: usize = 20;

/// Monotonic id of the newest connect attempt. A worker thread whose epoch is
/// no longer current must not write connection state.
static CONNECT_EPOCH: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TokenSet {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    /// Unix seconds after which `access_token` is no longer valid.
    pub(crate) expires_at_epoch: i64,
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSet")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_at_epoch", &self.expires_at_epoch)
            .finish()
    }
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

pub(crate) fn save_tokens(tokens: &TokenSet) -> Result<(), String> {
    let payload = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    keyring_entry()?.set_password(&payload).map_err(|e| e.to_string())
}

pub(crate) fn load_tokens() -> Result<Option<TokenSet>, String> {
    match keyring_entry()?.get_password() {
        Ok(payload) => serde_json::from_str(&payload).map(Some).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn delete_tokens() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// RFC 7636 S256: BASE64URL(SHA256(verifier)) without padding.
pub(crate) fn pkce_challenge(verifier: &str) -> String {
    base64_url_no_pad(&Sha256::digest(verifier.as_bytes()))
}

/// 64 chars from two simple UUIDs — valid PKCE verifier charset, no rand dep.
pub(crate) fn new_verifier() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn url_encode(value: &str) -> String {
    value.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        other => format!("%{other:02X}"),
    }).collect()
}

pub(crate) fn authorize_url(client_id: &str, redirect_uri: &str, state: &str, challenge: &str) -> String {
    format!(
        "{ZOOM_AUTH_BASE}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        url_encode(client_id), url_encode(redirect_uri), url_encode(state), url_encode(challenge)
    )
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

fn token_set_from_response(response: TokenResponse) -> TokenSet {
    TokenSet {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        expires_at_epoch: chrono::Utc::now().timestamp() + response.expires_in,
    }
}

fn post_token_form(form: &[(&str, &str)]) -> Result<TokenSet, String> {
    let response = reqwest::blocking::Client::new()
        .post(format!("{ZOOM_AUTH_BASE}/oauth/token"))
        .form(form)
        .send()
        .map_err(|e| format!("zoom token request failed: {e}"))?;
    if !response.status().is_success() {
        // Body may describe the error; it never contains our secrets.
        return Err(format!("zoom token endpoint returned {}", response.status()));
    }
    response.json::<TokenResponse>().map(token_set_from_response)
        .map_err(|e| format!("zoom token response parse failed: {e}"))
}

pub(crate) fn exchange_code(client_id: &str, code: &str, redirect_uri: &str, verifier: &str) -> Result<TokenSet, String> {
    post_token_form(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", verifier),
    ])
}

pub(crate) fn refresh_tokens(client_id: &str, refresh_token: &str) -> Result<TokenSet, String> {
    post_token_form(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ])
}

/// Returns a currently-valid access token, refreshing (and re-saving) when
/// it expires within 60 seconds. `Err` means the user must reconnect.
pub(crate) fn ensure_fresh_access_token(client_id: &str) -> Result<String, String> {
    let tokens = load_tokens()?.ok_or_else(|| "Zoom is not connected".to_string())?;
    if tokens.expires_at_epoch - chrono::Utc::now().timestamp() > 60 {
        return Ok(tokens.access_token);
    }
    let refreshed = refresh_tokens(client_id, &tokens.refresh_token)?;
    save_tokens(&refreshed)?;
    Ok(refreshed.access_token)
}

pub(crate) fn parse_callback_request(first_line: &str) -> Result<(String, String), String> {
    let path = first_line.strip_prefix("GET ").and_then(|rest| rest.split(' ').next())
        .ok_or_else(|| "not a GET request".to_string())?;
    let query = path.strip_prefix("/zoom/callback?").ok_or_else(|| "unexpected path".to_string())?;
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("code", value)) => code = Some(value.to_string()),
            Some(("state", value)) => state = Some(value.to_string()),
            Some(("error", value)) => return Err(format!("zoom authorization error: {value}")),
            _ => {}
        }
    }
    match (code, state) {
        (Some(code), Some(state)) => Ok((code, state)),
        _ => Err("callback missing code or state".to_string()),
    }
}

/// Waits for one loopback callback connection until `deadline`, returning the
/// connected stream and the request's first line. The listener must already be
/// nonblocking.
fn wait_for_callback(
    listener: &TcpListener,
    deadline: std::time::Instant,
) -> Result<(TcpStream, String), String> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .ok();
                let mut stream = stream;
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                let first_line = request.lines().next().unwrap_or_default().to_string();
                return Ok((stream, first_line));
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(
                        "timed out waiting for the Zoom authorization callback".to_string()
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZoomConnectionStatus {
    pub(crate) status: String,
    pub(crate) account_label: Option<String>,
}

fn write_connection(storage: &genesis_block_native::Storage, status: &str, account_label: &str) -> Result<(), String> {
    let timestamp = now();
    let created_at = genesis_adapter::query(storage, "external_connections", &["created_at"],
        vec![genesis_adapter::eq("external_connections", "id", serde_json::json!("zoom"))], 1)?
        .into_iter().next()
        .and_then(|row| row.get("external_connections.created_at").and_then(serde_json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| timestamp.clone());
    genesis_adapter::commit_rows(storage, vec![genesis_adapter::upsert("external_connections", serde_json::json!({
        "id": "zoom", "provider": "zoom", "account_label": account_label,
        "status": status, "created_at": created_at, "updated_at": timestamp,
    }))])
}

fn read_connection(storage: &genesis_block_native::Storage) -> Result<ZoomConnectionStatus, String> {
    let row = genesis_adapter::query(storage, "external_connections", &["status", "account_label"],
        vec![genesis_adapter::eq("external_connections", "id", serde_json::json!("zoom"))], 1)?
        .into_iter().next();
    Ok(match row {
        Some(row) => ZoomConnectionStatus {
            status: genesis_adapter::string(&row, "external_connections.status")?,
            account_label: row.get("external_connections.account_label").and_then(serde_json::Value::as_str)
                .filter(|label| !label.is_empty()).map(str::to_owned),
        },
        None => ZoomConnectionStatus { status: "disconnected".to_string(), account_label: None },
    })
}

pub(crate) fn client_id_from_env() -> AppResult<String> {
    std::env::var("FUNG_ZOOM_CLIENT_ID")
        .map_err(|_| AppError::InvalidInput("FUNG_ZOOM_CLIENT_ID is not configured".to_string()))
}

fn fetch_account_email(access_token: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Me { email: String }
    let response = reqwest::blocking::Client::new()
        .get(format!("{ZOOM_API_BASE}/users/me"))
        .bearer_auth(access_token)
        .send().map_err(|e| format!("zoom users/me failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("zoom users/me returned {}", response.status()));
    }
    response.json::<Me>().map(|me| me.email).map_err(|e| e.to_string())
}

/// Starts the OAuth flow: opens the system browser and spawns a background
/// thread that waits up to 180 s for the loopback callback, exchanges the
/// code, and stores tokens. The loopback listener is armed nonblocking and is
/// dropped by the worker thread on every exit path (success, auth error, or
/// timeout), so it never outlives the flow. UI polls `zoom_connection_status`.
#[tauri::command]
pub(crate) fn zoom_connect(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<ZoomConnectionStatus> {
    let client_id = client_id_from_env()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener
        .set_nonblocking(true)
        .map_err(|error| AppError::InvalidInput(format!("could not arm the callback listener: {error}")))?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/zoom/callback");
    let verifier = new_verifier();
    let oauth_state = uuid::Uuid::new_v4().simple().to_string();
    let url = authorize_url(&client_id, &redirect_uri, &oauth_state, &pkce_challenge(&verifier));

    let epoch = CONNECT_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    write_connection(&state.genesis, "connecting", "").map_err(AppError::Genesis)?;
    if let Err(error) = app.opener().open_url(url, None::<&str>) {
        CONNECT_EPOCH.fetch_add(1, Ordering::SeqCst);
        write_connection(&state.genesis, "error", "").map_err(AppError::Genesis)?;
        return Err(AppError::InvalidInput(format!("could not open browser: {error}")));
    }

    let storage = state.genesis.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let outcome = (|| -> Result<String, String> {
            let (mut stream, first_line) = wait_for_callback(&listener, deadline)?;
            let result = parse_callback_request(&first_line).and_then(|(code, returned_state)| {
                if returned_state != oauth_state { return Err("oauth state mismatch".to_string()); }
                exchange_code(&client_id, &code, &redirect_uri, &verifier)
            });
            let (body, out) = match result {
                Ok(tokens) => {
                    save_tokens(&tokens)?;
                    let email = fetch_account_email(&tokens.access_token).unwrap_or_default();
                    ("<html><body><h2>FUNG connected to Zoom.</h2>You can close this tab.</body></html>", Ok(email))
                }
                Err(error) => ("<html><body><h2>Zoom connection failed.</h2>Return to FUNG and retry.</body></html>", Err(error)),
            };
            let _ = stream.write_all(format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).as_bytes());
            out
        })();
        if CONNECT_EPOCH.load(Ordering::SeqCst) != epoch {
            // A newer connect attempt (or a disconnect) superseded this one;
            // writing state now would clobber it.
            return;
        }
        let _ = match outcome {
            Ok(email) => write_connection(&storage, "connected", &email),
            Err(_) => write_connection(&storage, "error", ""),
        };
    });

    Ok(ZoomConnectionStatus { status: "connecting".to_string(), account_label: None })
}

#[tauri::command]
pub(crate) fn zoom_connection_status(state: State<'_, AppState>) -> AppResult<ZoomConnectionStatus> {
    let mut status = read_connection(&state.genesis).map_err(AppError::Genesis)?;
    // A "connected" row without stored tokens means the credential was
    // removed out-of-band; surface that truthfully.
    if status.status == "connected" && load_tokens().map_err(AppError::Genesis)?.is_none() {
        status.status = "error".to_string();
    }
    Ok(status)
}

#[tauri::command]
pub(crate) fn zoom_disconnect(state: State<'_, AppState>) -> AppResult<ZoomConnectionStatus> {
    delete_tokens().map_err(AppError::Genesis)?;
    CONNECT_EPOCH.fetch_add(1, Ordering::SeqCst);
    write_connection(&state.genesis, "disconnected", "").map_err(AppError::Genesis)?;
    read_connection(&state.genesis).map_err(AppError::Genesis)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ZoomRecordingFile {
    pub(crate) file_type: String,
    pub(crate) recording_type: Option<String>,
    pub(crate) download_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ZoomParticipantAudioFile {
    pub(crate) file_name: String,
    pub(crate) download_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ZoomMeetingRecording {
    pub(crate) uuid: String,
    pub(crate) topic: String,
    pub(crate) start_time: String,
    pub(crate) duration: i64,
    #[serde(default)]
    pub(crate) recording_files: Vec<ZoomRecordingFile>,
    #[serde(default)]
    pub(crate) participant_audio_files: Option<Vec<ZoomParticipantAudioFile>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ZoomRecordingsPage {
    #[serde(default)]
    pub(crate) next_page_token: String,
    #[serde(default)]
    pub(crate) meetings: Vec<ZoomMeetingRecording>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZoomRecordingSummary {
    pub(crate) uuid: String,
    pub(crate) topic: String,
    pub(crate) start_time: String,
    pub(crate) duration_minutes: i64,
    pub(crate) has_participant_audio: bool,
}

pub(crate) fn summarize_meeting(meeting: &ZoomMeetingRecording) -> ZoomRecordingSummary {
    ZoomRecordingSummary {
        uuid: meeting.uuid.clone(),
        topic: meeting.topic.clone(),
        start_time: meeting.start_time.clone(),
        duration_minutes: meeting.duration,
        has_participant_audio: meeting.participant_audio_files.as_ref().is_some_and(|files| !files.is_empty()),
    }
}

/// Zoom path parameter rule: double-encode UUIDs that contain '/' (or start
/// with '/'), otherwise pass through unchanged.
pub(crate) fn encode_meeting_uuid(uuid: &str) -> String {
    if uuid.contains('/') || uuid.starts_with('/') {
        url_encode(&url_encode(uuid))
    } else {
        uuid.to_string()
    }
}

fn api_get_json<T: serde::de::DeserializeOwned>(access_token: &str, path_and_query: &str) -> Result<T, String> {
    let response = reqwest::blocking::Client::new()
        .get(format!("{ZOOM_API_BASE}{path_and_query}"))
        .bearer_auth(access_token)
        .send().map_err(|e| format!("zoom api request failed: {e}"))?;
    let status = response.status();
    if status.as_u16() == 429 {
        return Err("zoom rate limit hit (429); try again shortly".to_string());
    }
    if !status.is_success() {
        return Err(format!("zoom api returned {status} for {path_and_query}"));
    }
    response.json::<T>().map_err(|e| format!("zoom api response parse failed: {e}"))
}

/// Decides whether paging may continue. Extracted so the runaway-guard policy
/// is testable without an HTTP layer.
fn may_fetch_another_page(pages_fetched: usize, next_page_token: &str) -> bool {
    !next_page_token.is_empty() && pages_fetched < MAX_RECORDING_PAGES
}

/// Lists the caller's cloud recordings from the last 30 days, paging through
/// every result until Zoom stops returning a `next_page_token`. Paging is
/// capped at `MAX_RECORDING_PAGES` purely as a runaway guard — a real 30-day
/// window never approaches it — and hitting that cap is reported as an error
/// rather than silently truncating the list.
#[tauri::command]
pub(crate) fn zoom_list_recordings() -> AppResult<Vec<ZoomRecordingSummary>> {
    let client_id = client_id_from_env()?;
    let access_token = ensure_fresh_access_token(&client_id).map_err(AppError::InvalidInput)?;
    let from = (chrono::Utc::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
    let mut summaries = Vec::new();
    let mut next_page_token = String::new();
    let mut pages_fetched = 0usize;
    let mut complete = false;
    loop {
        let path = format!("/users/me/recordings?page_size=30&from={from}&next_page_token={}", url_encode(&next_page_token));
        let page: ZoomRecordingsPage = api_get_json(&access_token, &path).map_err(AppError::InvalidInput)?;
        summaries.extend(page.meetings.iter().map(summarize_meeting));
        pages_fetched += 1;
        if page.next_page_token.is_empty() {
            complete = true;
            break;
        }
        next_page_token = page.next_page_token;
        if !may_fetch_another_page(pages_fetched, &next_page_token) {
            break;
        }
    }
    if !complete {
        return Err(AppError::InvalidInput(
            "too many Zoom recordings to list in one request; narrow the date range".to_string(),
        ));
    }
    summaries.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(summaries)
}

/// A prior Zoom import of the same meeting, if any.
#[derive(Debug, Clone)]
pub(crate) struct PriorImport {
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    /// The recording's current status; `"completed"` means the import finished.
    pub(crate) recording_status: String,
}

/// Finds a previous Zoom import of `uuid`, so a retry can resume it instead of
/// creating a second project. Returns `None` when the meeting was never imported.
pub(crate) fn find_prior_import(
    storage: &genesis_block_native::Storage,
    uuid: &str,
) -> Result<Option<PriorImport>, String> {
    let Some(row) = genesis_adapter::query(
        storage,
        "external_imports",
        &["project_id", "recording_id", "provider", "external_uuid"],
        vec![genesis_adapter::eq("external_imports", "external_uuid", serde_json::json!(uuid))],
        10,
    )?
    .into_iter()
    .find(|row| {
        row.get("external_imports.provider").and_then(serde_json::Value::as_str) == Some("zoom")
    }) else {
        return Ok(None);
    };
    let project_id = genesis_adapter::string(&row, "external_imports.project_id")?;
    let recording_id = genesis_adapter::string(&row, "external_imports.recording_id")?;
    let recording_status = genesis_adapter::query(
        storage,
        "recordings",
        &["status"],
        vec![genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id))],
        1,
    )?
    .into_iter()
    .next()
    .map(|row| genesis_adapter::string(&row, "recordings.status"))
    .transpose()?
    .unwrap_or_else(|| "pending".to_string());
    Ok(Some(PriorImport { project_id, recording_id, recording_status }))
}

/// Windows-safe single path component: replaces separator/reserved chars.
pub(crate) fn sanitize_component(value: &str) -> String {
    value.chars().map(|c| match c {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '=' => '_',
        other => other,
    }).collect()
}

/// Streams `url` to `dest`, resuming with a Range request when a partial
/// file exists. Never log `url` — it is credential-bearing.
pub(crate) fn download_to_file(access_token: &str, url: &str, dest: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let existing = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url).bearer_auth(access_token);
    if existing > 0 { request = request.header("Range", format!("bytes={existing}-")); }
    // reqwest::Error's Display embeds the request URL when one is set (as it
    // is here); strip it before formatting so the credential-bearing
    // download URL never reaches a job event or error string.
    let mut response = request.send().map_err(|e| format!("zoom download failed: {}", e.without_url()))?;
    let status = response.status();
    let append = status.as_u16() == 206;
    if !status.is_success() {
        return Err(format!("zoom download returned {status}"));
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true).write(true).append(append).truncate(!append)
        .open(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut response, &mut file).map_err(|e| format!("zoom download write failed: {e}"))?;
    Ok(())
}

/// Imports one Zoom cloud recording end-to-end: download → transcribe →
/// speaker attribution → graph build. Runs on a background thread; UI polls
/// `list_jobs`.
#[tauri::command]
pub(crate) fn zoom_import_recording(meeting_uuid: String, state: State<'_, AppState>) -> AppResult<crate::Job> {
    let client_id = client_id_from_env()?;
    let prior = find_prior_import(&state.genesis, &meeting_uuid).map_err(AppError::Genesis)?;
    if prior.as_ref().is_some_and(|import| import.recording_status == "completed") {
        return Err(AppError::InvalidInput("recording is already imported".to_string()));
    }
    let resuming = prior.is_some();
    let (project_id, recording_id) = match &prior {
        Some(import) => (import.project_id.clone(), import.recording_id.clone()),
        None => (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string()),
    };
    let access_token = ensure_fresh_access_token(&client_id).map_err(AppError::InvalidInput)?;
    let meeting: ZoomMeetingRecording =
        api_get_json(&access_token, &format!("/meetings/{}/recordings", encode_meeting_uuid(&meeting_uuid)))
            .map_err(AppError::InvalidInput)?;

    let job_id = uuid::Uuid::new_v4().to_string();
    let timestamp = now();
    let storage_path = state.data_root.join("projects").join(&project_id).display().to_string();
    let base_dir = state.data_root.join("projects").join(&project_id).join("zoom").join(sanitize_component(&meeting_uuid));
    let mixed_path = base_dir.join("mixed.m4a");

    let mut seed = Vec::new();
    if !resuming {
        seed.push(genesis_adapter::upsert("projects", serde_json::json!({"id": project_id, "name": meeting.topic, "storage_path": storage_path, "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})));
        seed.push(genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": "import", "input_path": null, "canonical_audio_path": mixed_path.display().to_string(), "status": "pending", "duration_ms": 0, "created_at": timestamp, "updated_at": timestamp})));
        // Recorded upfront so a concurrent second call finds it immediately.
        seed.push(genesis_adapter::upsert("external_imports", serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "project_id": project_id,
            "provider": "zoom",
            "external_uuid": meeting_uuid,
            "recording_id": recording_id,
            "payload_json": {"topic": meeting.topic},
            "created_at": timestamp,
        })));
    }
    seed.push(genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "zoom.import", "status": "running", "progress": 0, "input_refs_json": [meeting_uuid], "output_refs_json": [recording_id], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": timestamp, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})));
    seed.push(genesis_adapter::upsert("job_events", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": if resuming { "resuming zoom recording download" } else { "downloading zoom recording" }, "created_at": timestamp})));
    genesis_adapter::commit_rows(&state.genesis, seed).map_err(AppError::Genesis)?;

    let ctx = ImportContext {
        storage: state.genesis.clone(),
        whisper: state.whisper_runtime_clone(),
        job_id: job_id.clone(),
        project_id: project_id.clone(),
        recording_id: recording_id.clone(),
        meeting_uuid: meeting_uuid.clone(),
        meeting_topic: meeting.topic.clone(),
        base_dir,
        mixed_path,
    };
    std::thread::spawn(move || run_import_worker(ctx, meeting, access_token));

    Ok(crate::Job {
        id: job_id, project_id, job_type: "zoom.import".to_string(), status: "running".to_string(),
        progress: 0, input_refs: vec![meeting_uuid], output_refs: vec![recording_id],
        provider_id: None, error_code: None, error_message: None,
        started_at: Some(timestamp.clone()), finished_at: None,
        created_at: timestamp.clone(), updated_at: timestamp,
    })
}

pub(crate) struct ImportContext {
    pub(crate) storage: std::sync::Arc<genesis_block_native::Storage>,
    pub(crate) whisper: crate::WhisperRuntime,
    pub(crate) job_id: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) meeting_uuid: String,
    pub(crate) meeting_topic: String,
    pub(crate) base_dir: std::path::PathBuf,
    pub(crate) mixed_path: std::path::PathBuf,
}

/// Phase 1 of the worker: downloads. Task 6/7 extend this with processing.
fn run_import_worker(ctx: ImportContext, meeting: ZoomMeetingRecording, access_token: String) {
    let result = (|| -> Result<Vec<(String, std::path::PathBuf)>, String> {
        let mixed = meeting.recording_files.iter()
            .find(|f| f.recording_type.as_deref() == Some("audio_only") && f.file_type.eq_ignore_ascii_case("M4A"))
            .or_else(|| meeting.recording_files.iter().find(|f| f.file_type.eq_ignore_ascii_case("MP4")))
            .ok_or_else(|| "no downloadable audio/video file on this recording".to_string())?;
        download_to_file(&access_token, &mixed.download_url, &ctx.mixed_path)?;
        let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "running", Some(15), None);
        let mut participants = Vec::new();
        if let Some(files) = &meeting.participant_audio_files {
            for (index, file) in files.iter().enumerate() {
                let display_name = file.file_name.strip_prefix("Audio only - ").unwrap_or(&file.file_name).to_string();
                let dest = ctx.base_dir.join("participants").join(format!("{index}-{}.m4a", sanitize_component(&display_name)));
                download_to_file(&access_token, &file.download_url, &dest)?;
                participants.push((display_name, dest));
            }
        }
        let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "running", Some(30), None);
        Ok(participants)
    })();

    match result {
        Ok(participants) => run_processing_pipeline(ctx, participants),
        Err(message) => { let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "failed", None, Some(&message)); }
    }
}

/// Path A (>=2 participant audio files): transcribe each participant file
/// separately for perfect attribution, merge by time, then persist. Path B
/// (mixed audio only) is implemented in Task 7. Either way the `zoom.import`
/// job only reports `completed` once attribution has actually persisted, and
/// `persist_attribution` is what flips the recording row to `completed`.
fn run_processing_pipeline(ctx: ImportContext, participants: Vec<(String, std::path::PathBuf)>) {
    let outcome = (|| -> Result<(), String> {
        if participants.len() >= 2 {
            // Path A: perfect attribution from per-participant files.
            let total = participants.len() as i64;
            let mut outputs = Vec::new();
            for (index, (display_name, path)) in participants.iter().enumerate() {
                let storage = ctx.storage.clone();
                let job_id = ctx.job_id.clone();
                let base = 30 + (index as i64 * 55) / total;
                let span = 55 / total;
                let output = crate::run_transcription(&ctx.whisper, &path.display().to_string(), move |pct| {
                    let _ = crate::set_job_status(&storage, &job_id, "running", Some(base + pct * span / 100), None);
                })?;
                outputs.push((display_name.clone(), output));
            }
            let duration_ms = outputs.iter().map(|(_, output)| output.duration_ms).max().unwrap_or(0);
            let merged = crate::speaker_merge::merge_participant_outputs(outputs);
            let turns = crate::speaker_merge::group_turns(&merged, 1_500);
            crate::speaker_merge::persist_attribution(&ctx.storage, &ctx.project_id, &ctx.recording_id, "local", "faster-whisper per-participant", &merged, &turns, duration_ms)?;
        } else {
            run_mixed_audio_path(&ctx)?; // Path B — implemented in Task 7.
        }
        Ok(())
    })();
    match outcome {
        Ok(()) => {
            let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "completed", Some(100), None);
            crate::graph_build::start_graph_build(ctx.storage.clone(), ctx.project_id.clone(), ctx.recording_id.clone(), ctx.meeting_topic.clone());
        }
        Err(message) => { let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "failed", None, Some(&message)); }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiarizeTurn {
    pub(crate) speaker_key: String,
    pub(crate) display_name: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiarizeOutput {
    pub(crate) duration_ms: i64,
    pub(crate) turns: Vec<DiarizeTurn>,
}

/// Path B: whisper on the mixed file, then local pyannote diarization.
/// Diarization failure downgrades gracefully: transcript persists without
/// speakers and the job still completes (spec rule).
fn run_mixed_audio_path(ctx: &ImportContext) -> Result<(), String> {
    let mixed = ctx.mixed_path.display().to_string();
    let (storage, job_id) = (ctx.storage.clone(), ctx.job_id.clone());
    let whisper_output = crate::run_transcription(&ctx.whisper, &mixed, move |pct| {
        let _ = crate::set_job_status(&storage, &job_id, "running", Some(30 + pct * 35 / 100), None);
    })?;
    let unassigned: Vec<crate::speaker_merge::AttributedSegment> = whisper_output.segments.iter().map(|segment| crate::speaker_merge::AttributedSegment {
        speaker_key: None, display_name: None,
        start_ms: segment.start_ms, end_ms: segment.end_ms,
        text: segment.text.clone(), confidence: segment.confidence,
    }).collect();

    let (storage, job_id) = (ctx.storage.clone(), ctx.job_id.clone());
    match crate::run_diarization(&ctx.whisper, &mixed, move |pct| {
        let _ = crate::set_job_status(&storage, &job_id, "running", Some(65 + pct * 30 / 100), None);
    }) {
        Ok(diarize) => {
            let assigned = crate::speaker_merge::assign_by_overlap(&unassigned, &diarize.turns);
            let turns: Vec<crate::speaker_merge::SpeakerTurn> = diarize.turns.iter().map(|turn| crate::speaker_merge::SpeakerTurn {
                speaker_key: turn.speaker_key.clone(), display_name: turn.display_name.clone(),
                start_ms: turn.start_ms, end_ms: turn.end_ms, confidence: turn.confidence, overlap: false,
            }).collect();
            crate::speaker_merge::persist_attribution(&ctx.storage, &ctx.project_id, &ctx.recording_id, "local", "pyannote/speaker-diarization-3.1", &assigned, &turns, whisper_output.duration_ms)
        }
        Err(message) => {
            // Transcript must survive without diarization.
            crate::speaker_merge::persist_attribution(&ctx.storage, &ctx.project_id, &ctx.recording_id, "local", "faster-whisper (no diarization)", &unassigned, &[], whisper_output.duration_ms)?;
            let timestamp = now();
            // The transcript is already durable at this point; an audit-log
            // write failure here must not fail the import, so this is
            // best-effort rather than `?`.
            let _ = genesis_adapter::commit_rows(&ctx.storage, vec![genesis_adapter::upsert("job_events", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "job_id": ctx.job_id, "status": "running", "message": format!("diarization unavailable: {message}"), "created_at": timestamp}))]);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-zoom-test-{}", uuid::Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(), page_cache_mb: Some(16), read_only: Some(false), vector_dim: Some(4),
        }).unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    #[test]
    fn diarize_output_parses_worker_json() {
        let raw = r#"{"durationMs": 9000, "turns": [{"speakerKey": "s:0", "displayName": "Speaker 1", "startMs": 0, "endMs": 2500, "confidence": null}]}"#;
        let output: DiarizeOutput = serde_json::from_str(raw).unwrap();
        assert_eq!(output.turns.len(), 1);
        assert_eq!(output.turns[0].display_name, "Speaker 1");
    }

    #[test]
    fn prior_import_reports_recording_status_for_resume_decisions() {
        let (path, storage) = open_storage();
        assert!(find_prior_import(&storage, "uuid-1").unwrap().is_none());
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("external_imports", serde_json::json!({"id":"i1","project_id":"p1","provider":"zoom","external_uuid":"uuid-1","recording_id":"r1","payload_json":{},"created_at":"t"})),
        ]).unwrap();

        let prior = find_prior_import(&storage, "uuid-1").unwrap().expect("prior import");
        assert_eq!(prior.project_id, "p1");
        assert_eq!(prior.recording_id, "r1");
        // An unfinished import must be resumable, not rejected.
        assert_eq!(prior.recording_status, "pending");

        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"completed","duration_ms":10,"created_at":"t","updated_at":"t2"})),
        ]).unwrap();
        let finished = find_prior_import(&storage, "uuid-1").unwrap().expect("prior import");
        assert_eq!(finished.recording_status, "completed");

        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn prior_import_ignores_other_providers_and_other_meetings() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("external_imports", serde_json::json!({"id":"i1","project_id":"p1","provider":"other","external_uuid":"uuid-1","recording_id":"r1","payload_json":{},"created_at":"t"})),
        ]).unwrap();
        assert!(find_prior_import(&storage, "uuid-1").unwrap().is_none(), "non-zoom provider must not match");
        assert!(find_prior_import(&storage, "uuid-2").unwrap().is_none());
        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn sanitize_component_keeps_paths_safe() {
        assert_eq!(sanitize_component("abc//slash=="), "abc__slash__");
        assert_eq!(sanitize_component("Audio only - Boss"), "Audio only - Boss");
        assert_eq!(sanitize_component("a<b>:c|?*\\/"), "a_b__c_____");
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        // RFC 7636 appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(pkce_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn authorize_url_carries_pkce_and_state() {
        let url = authorize_url("client123", "http://127.0.0.1:4567/zoom/callback", "st4te", "chall");
        assert!(url.starts_with("https://zoom.us/oauth/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=client123"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4567%2Fzoom%2Fcallback"));
        assert!(url.contains("state=st4te"));
        assert!(url.contains("code_challenge=chall"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn token_set_debug_never_exposes_secrets() {
        let tokens = TokenSet {
            access_token: "secret-access".to_string(),
            refresh_token: "secret-refresh".to_string(),
            expires_at_epoch: 123,
        };
        let rendered = format!("{tokens:?}");
        assert!(!rendered.contains("secret-access"));
        assert!(!rendered.contains("secret-refresh"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn callback_parser_extracts_code_and_state() {
        let line = "GET /zoom/callback?code=abc123&state=xyz HTTP/1.1";
        assert_eq!(parse_callback_request(line).unwrap(), ("abc123".to_string(), "xyz".to_string()));
        // Order-independent.
        let line2 = "GET /zoom/callback?state=xyz&code=abc123 HTTP/1.1";
        assert_eq!(parse_callback_request(line2).unwrap(), ("abc123".to_string(), "xyz".to_string()));
    }

    #[test]
    fn callback_parser_rejects_denials_and_junk() {
        assert!(parse_callback_request("GET /zoom/callback?error=access_denied HTTP/1.1").is_err());
        assert!(parse_callback_request("GET /favicon.ico HTTP/1.1").is_err());
    }

    #[test]
    fn wait_for_callback_returns_request_line_from_loopback_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || {
            use std::io::Write as _;
            let mut stream = TcpStream::connect(addr).unwrap();
            stream
                .write_all(b"GET /zoom/callback?code=c&state=s HTTP/1.1\r\nHost: x\r\n\r\n")
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(300));
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (_stream, first_line) = wait_for_callback(&listener, deadline).unwrap();
        assert_eq!(first_line, "GET /zoom/callback?code=c&state=s HTTP/1.1");
        client.join().unwrap();
    }

    #[test]
    fn wait_for_callback_times_out_without_a_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
        let error = wait_for_callback(&listener, deadline).unwrap_err();
        assert!(error.contains("timed out"));
    }

    const RECORDINGS_FIXTURE: &str = r#"{
      "next_page_token": "",
      "meetings": [
        {
          "uuid": "abc//slash==",
          "topic": "Weekly sync",
          "start_time": "2026-08-01T09:00:00Z",
          "duration": 42,
          "recording_files": [
            {"file_type": "M4A", "recording_type": "audio_only", "download_url": "https://zoom.us/rec/dl/mixed"},
            {"file_type": "MP4", "recording_type": "shared_screen_with_speaker_view", "download_url": "https://zoom.us/rec/dl/video"}
          ],
          "participant_audio_files": [
            {"file_name": "Audio only - Boss", "download_url": "https://zoom.us/rec/dl/p1"},
            {"file_name": "Audio only - ATHER", "download_url": "https://zoom.us/rec/dl/p2"}
          ]
        },
        {
          "uuid": "plainuuid",
          "topic": "1:1",
          "start_time": "2026-08-02T10:00:00Z",
          "duration": 15,
          "recording_files": [
            {"file_type": "M4A", "recording_type": "audio_only", "download_url": "https://zoom.us/rec/dl/only"}
          ]
        }
      ]
    }"#;

    #[test]
    fn recordings_page_parses_and_summarizes() {
        let page: ZoomRecordingsPage = serde_json::from_str(RECORDINGS_FIXTURE).unwrap();
        let summaries: Vec<ZoomRecordingSummary> = page.meetings.iter().map(summarize_meeting).collect();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].uuid, "abc//slash==");
        assert_eq!(summaries[0].duration_minutes, 42);
        assert!(summaries[0].has_participant_audio);
        assert!(!summaries[1].has_participant_audio);
    }

    #[test]
    fn meeting_uuid_is_double_encoded_when_it_contains_slashes() {
        // Zoom requires double URL-encoding for UUIDs containing '/' or '//'.
        assert_eq!(encode_meeting_uuid("abc//slash=="), "abc%252F%252Fslash%253D%253D");
        assert_eq!(encode_meeting_uuid("plainuuid"), "plainuuid");
    }

    #[test]
    fn paging_continues_past_three_pages_and_stops_at_the_runaway_guard() {
        // An empty token always ends paging.
        assert!(!may_fetch_another_page(1, ""));
        // Page 4 and beyond must still be fetched — the old 3-page cap was a
        // silent truncation bug.
        assert!(may_fetch_another_page(3, "tok"));
        assert!(may_fetch_another_page(10, "tok"));
        // The runaway guard is the only stop condition besides an empty token.
        assert!(!may_fetch_another_page(MAX_RECORDING_PAGES, "tok"));
    }
}
