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

/// Lists the caller's cloud recordings from the last 30 days (up to 3 pages).
#[tauri::command]
pub(crate) fn zoom_list_recordings() -> AppResult<Vec<ZoomRecordingSummary>> {
    let client_id = client_id_from_env()?;
    let access_token = ensure_fresh_access_token(&client_id).map_err(AppError::InvalidInput)?;
    let from = (chrono::Utc::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
    let mut summaries = Vec::new();
    let mut next_page_token = String::new();
    for _ in 0..3 {
        let path = format!("/users/me/recordings?page_size=30&from={from}&next_page_token={}", url_encode(&next_page_token));
        let page: ZoomRecordingsPage = api_get_json(&access_token, &path).map_err(AppError::InvalidInput)?;
        summaries.extend(page.meetings.iter().map(summarize_meeting));
        if page.next_page_token.is_empty() { break; }
        next_page_token = page.next_page_token;
    }
    summaries.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
