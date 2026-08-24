//! Google Drive OAuth and appDataFolder transport.
//!
//! The desktop is the only holder of the Drive refresh token. OAuth state and
//! authorization codes stay in native memory, while the refresh token is
//! stored in the OS credential store. The frontend receives only redacted
//! status and archive metadata.

use crate::backup::{self, BackupJobState, RestoreResult};
use crate::backup_archive::{self, ArchiveManifest};
use crate::filesystem_backup::{self, FilesystemArchiveRecord, FilesystemBackupState};
use crate::native_auth::{self, DriveInvocation, DriveOperation};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::{rngs::OsRng, RngCore};
use reqwest::blocking::{multipart, Client as BlockingClient};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::State;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

pub(crate) const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";

const DRIVE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const DRIVE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const DRIVE_FILES_ENDPOINT: &str = "https://www.googleapis.com/drive/v3/files";
const DRIVE_UPLOAD_ENDPOINT: &str = "https://www.googleapis.com/upload/drive/v3/files";
const KEYRING_SERVICE: &str = "FUNG";
const OAUTH_TTL: Duration = Duration::from_secs(10 * 60);
const DRIVE_CHUNK_SIZE: usize = 8 * 1024 * 1024;
const MAX_DRIVE_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const ARCHIVE_SUFFIX: &str = ".fungbk";
const MANIFEST_SUFFIX: &str = ".manifest.json";
const OAUTH_CALLBACK_PATH: &str = "/oauth/google-drive/callback";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveOAuthStart {
    pub(crate) request_id: String,
    pub(crate) scope: &'static str,
    pub(crate) expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveConnectionStatus {
    pub(crate) connected: bool,
    pub(crate) scope: Option<String>,
    pub(crate) provider: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveCancelled {
    pub(crate) request_id: String,
    pub(crate) status: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveArchiveSummary {
    pub(crate) file_id: String,
    pub(crate) archive_id: String,
    pub(crate) byte_count: u64,
    pub(crate) digest: Option<String>,
    pub(crate) modified_time: Option<String>,
}

#[derive(Clone)]
struct OAuthCallback {
    state: Zeroizing<String>,
    code: Option<Zeroizing<String>>,
    error: Option<String>,
}

#[derive(Clone)]
struct PendingOAuth {
    session_id: String,
    oauth_client_id: String,
    redirect_uri: String,
    state: Zeroizing<String>,
    code_verifier: Zeroizing<String>,
    callback: Arc<Mutex<Option<OAuthCallback>>>,
    terminal: OAuthTerminal,
    expires_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OAuthTerminalState {
    Pending,
    Exchanging,
    Cancelled,
    Committing,
    Completed,
    Failed,
}

#[derive(Clone)]
struct OAuthTerminal {
    state: Arc<Mutex<OAuthTerminalState>>,
}

impl Default for OAuthTerminal {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(OAuthTerminalState::Pending)),
        }
    }
}

impl OAuthTerminal {
    fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .map(|state| *state == OAuthTerminalState::Cancelled)
            .unwrap_or(true)
    }

    fn begin_exchange(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| public_error("drive_oauth_state_unavailable"))?;
        match *state {
            OAuthTerminalState::Pending => {
                *state = OAuthTerminalState::Exchanging;
                Ok(())
            }
            OAuthTerminalState::Cancelled => Err(public_error("drive_oauth_cancelled")),
            OAuthTerminalState::Committing | OAuthTerminalState::Completed => {
                Err(public_error("drive_oauth_completed"))
            }
            OAuthTerminalState::Exchanging | OAuthTerminalState::Failed => {
                Err(public_error("drive_oauth_session_missing"))
            }
        }
    }

    fn begin_commit(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| public_error("drive_oauth_state_unavailable"))?;
        match *state {
            OAuthTerminalState::Exchanging => {
                *state = OAuthTerminalState::Committing;
                Ok(())
            }
            OAuthTerminalState::Cancelled => Err(public_error("drive_oauth_cancelled")),
            OAuthTerminalState::Committing | OAuthTerminalState::Completed => {
                Err(public_error("drive_oauth_completed"))
            }
            OAuthTerminalState::Pending | OAuthTerminalState::Failed => {
                Err(public_error("drive_oauth_session_missing"))
            }
        }
    }

    fn cancel(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| public_error("drive_oauth_state_unavailable"))?;
        match *state {
            OAuthTerminalState::Pending | OAuthTerminalState::Exchanging => {
                *state = OAuthTerminalState::Cancelled;
                Ok(())
            }
            OAuthTerminalState::Cancelled => Ok(()),
            OAuthTerminalState::Committing | OAuthTerminalState::Completed => {
                Err(public_error("drive_oauth_completed"))
            }
            OAuthTerminalState::Failed => Err(public_error("drive_oauth_session_missing")),
        }
    }

    fn mark_completed(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = OAuthTerminalState::Completed;
        }
    }

    fn mark_failed(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = OAuthTerminalState::Failed;
        }
    }
}

#[derive(Default)]
pub(crate) struct DriveOAuthState {
    pending: Mutex<Option<PendingOAuth>>,
}

fn deserialize_zeroizing<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where D: Deserializer<'de> { String::deserialize(deserializer).map(Zeroizing::new) }
fn deserialize_optional_zeroizing<'de, D>(deserializer: D) -> Result<Option<Zeroizing<String>>, D::Error>
where D: Deserializer<'de> { Option::<String>::deserialize(deserializer).map(|value| value.map(Zeroizing::new)) }

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(rename = "access_token", default, deserialize_with = "deserialize_optional_zeroizing")]
    access: Option<Zeroizing<String>>,
    #[serde(rename = "refresh_token", default, deserialize_with = "deserialize_optional_zeroizing")]
    refresh: Option<Zeroizing<String>>,
    scope: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct DriveTokenRecordWrite<'a> {
    #[serde(rename = "refresh_token")]
    value: &'a str,
}
#[derive(Deserialize)]
struct DriveTokenRecord {
    #[serde(rename = "refresh_token", deserialize_with = "deserialize_zeroizing")]
    value: Zeroizing<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFileList {
    files: Vec<DriveFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriveFile {
    id: String,
    name: Option<String>,
    size: Option<String>,
    modified_time: Option<String>,
    app_properties: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DriveManifestEnvelope {
    archive: ArchiveManifest,
    record: FilesystemArchiveRecord,
}

fn public_error(code: &str) -> String {
    code.to_owned()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

fn validate_client_id(client_id: &str) -> Result<(), String> {
    if client_id.trim().is_empty()
        || client_id.len() > 256
        || !client_id.ends_with(".apps.googleusercontent.com")
        || client_id
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(public_error("google_drive_client_id_missing_or_invalid"));
    }
    Ok(())
}

fn validate_archive_id(archive_id: &str) -> Result<(), String> {
    if archive_id.is_empty()
        || archive_id.len() > 128
        || !archive_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(public_error("invalid_archive_id"));
    }
    Ok(())
}

fn validate_drive_file_id(file_id: &str) -> Result<(), String> {
    if file_id.is_empty()
        || file_id.len() > 256
        || !file_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(public_error("invalid_drive_file_id"));
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<(), String> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(public_error("drive_provider_file_invalid"));
    }
    Ok(())
}

fn validate_drive_file_contract(
    file: &DriveFile,
    archive_id: &str,
    kind: &str,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    validate_drive_file_id(&file.id)?;
    validate_archive_id(archive_id)?;
    let suffix = match kind {
        "archive" => ARCHIVE_SUFFIX,
        "manifest" => MANIFEST_SUFFIX,
        _ => return Err(public_error("drive_provider_file_invalid")),
    };
    let expected_name = format!("{archive_id}{suffix}");
    if file.name.as_deref() != Some(expected_name.as_str()) {
        return Err(public_error("drive_provider_file_invalid"));
    }
    let properties = file
        .app_properties
        .as_ref()
        .ok_or_else(|| public_error("drive_provider_file_invalid"))?;
    if properties.get("fungArchiveId").map(String::as_str) != Some(archive_id)
        || properties.get("fungKind").map(String::as_str) != Some(kind)
    {
        return Err(public_error("drive_provider_file_invalid"));
    }
    let digest = properties
        .get("fungDigest")
        .ok_or_else(|| public_error("drive_provider_file_invalid"))?;
    validate_digest(digest)?;
    file.size
        .as_deref()
        .and_then(|size| size.parse::<u64>().ok())
        .ok_or_else(|| public_error("drive_provider_file_invalid"))?;
    if expected_digest.is_some_and(|expected| expected != digest) {
        return Err(public_error("drive_provider_file_invalid"));
    }
    Ok(())
}

fn validate_drive_file_ids(
    files: Vec<DriveFile>,
    error_code: &str,
) -> Result<Vec<DriveFile>, String> {
    for file in &files {
        validate_drive_file_id(&file.id).map_err(|_| public_error(error_code))?;
    }
    Ok(files)
}

fn configured_client_id() -> Result<String, String> {
    let client_id = std::env::var("FUNG_GOOGLE_DRIVE_CLIENT_ID")
        .or_else(|_| std::env::var("VITE_GOOGLE_DRIVE_CLIENT_ID"))
        .map_err(|_| public_error("google_drive_client_id_missing"))?;
    validate_client_id(client_id.trim())?;
    Ok(client_id.trim().to_owned())
}

fn keyring_slot_for(user_id: &str, device_id: &str, device_fingerprint: &str) -> String {
    let digest = Sha256::digest(
        format!("google-drive\0{user_id}\0{device_id}\0{device_fingerprint}").as_bytes(),
    );
    let suffix = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("drive-token-{suffix}")
}

fn keyring_entry(invocation: &DriveInvocation) -> Result<keyring::Entry, String> {
    invocation.ensure_valid()?;
    keyring::Entry::new(
        KEYRING_SERVICE,
        &keyring_slot_for(
            invocation.user_id(),
            invocation.device_id(),
            invocation.device_fingerprint(),
        ),
    )
    .map_err(|_| public_error("drive_keyring_unavailable"))
}

fn save_refresh_token(invocation: &DriveInvocation, refresh_token: Zeroizing<String>) -> Result<(), String> {
    let staged_slot = format!("{}-staged", keyring_slot_for(invocation.user_id(), invocation.device_id(), invocation.device_fingerprint()));
    let active_slot = keyring_slot_for(invocation.user_id(), invocation.device_id(), invocation.device_fingerprint());
    let payload = serde_json::to_string(&DriveTokenRecordWrite { value: refresh_token.as_str() })
        .map_err(|_| public_error("drive_token_storage_failed"))?;
    let staged = keyring::Entry::new(KEYRING_SERVICE, &staged_slot).map_err(|_| public_error("drive_token_storage_failed"))?;
    let active = keyring::Entry::new(KEYRING_SERVICE, &active_slot).map_err(|_| public_error("drive_token_storage_failed"))?;
    staged.set_password(&payload).map_err(|_| public_error("drive_token_storage_failed"))?;
    let staged_read = staged.get_password().map_err(|_| public_error("drive_token_storage_failed"))?;
    if staged_read != payload { return Err(public_error("drive_token_storage_failed")); }
    active.set_password(&staged_read).map_err(|_| public_error("drive_token_storage_failed"))?;
    let active_read = active.get_password().map_err(|_| public_error("drive_token_storage_failed"))?;
    if active_read != payload { return Err(public_error("drive_token_storage_failed")); }
    let _ = staged.delete_credential();
    if staged.get_password().is_ok() { return Err(public_error("drive_token_storage_failed")); }
    Ok(())
}

fn load_refresh_token(invocation: &DriveInvocation) -> Result<Option<Zeroizing<String>>, String> {
    match keyring_entry(invocation)?.get_password() {
        Ok(payload) => {
            let payload = Zeroizing::new(payload);
            let record: DriveTokenRecord = serde_json::from_str(payload.as_str())
                .map_err(|_| public_error("drive_token_storage_invalid"))?;
            if record.value.trim().is_empty() {
                return Err(public_error("drive_token_storage_invalid"));
            }
            Ok(Some(record.value))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(public_error("drive_keyring_unavailable")),
    }
}

fn delete_refresh_token(invocation: &DriveInvocation) -> Result<(), String> {
    match keyring_entry(invocation)?.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => {}, Err(_) => return Err(public_error("drive_token_delete_failed")) }
    if keyring_entry(invocation)?.get_password().is_ok() { return Err(public_error("drive_token_delete_failed")); }
    Ok(())
}

fn exact_scope(scope: &str) -> bool {
    let scopes: Vec<&str> = scope.split_whitespace().collect();
    scopes.len() == 1 && scopes[0] == DRIVE_SCOPE
}

fn async_http_client(timeout: Duration, error_code: &str) -> Result<Client, String> {
    Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|_| public_error(error_code))
}

fn pkce_pair() -> (Zeroizing<String>, String) {
    let mut verifier_bytes = [0u8; 64];
    OsRng.fill_bytes(&mut verifier_bytes);
    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (Zeroizing::new(verifier), challenge)
}

fn random_state() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_authorization_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let mut url = Url::parse(DRIVE_AUTH_ENDPOINT).expect("static Google auth URL");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", DRIVE_SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    url.to_string()
}

fn callback_from_request(request: &[u8]) -> OAuthCallback {
    let first = request.split(|byte| *byte == b'\n').next().unwrap_or_default();
    let first = first.strip_suffix(b"\r").unwrap_or(first);
    let mut fields = first.split(|byte| *byte == b' ');
    let target = fields.nth(1).and_then(|value| std::str::from_utf8(value).ok()).unwrap_or("/");
    let parsed = Url::parse(&format!("http://127.0.0.1{target}"));
    let Ok(parsed) = parsed else {
        return OAuthCallback {
            state: Zeroizing::new(String::new()),
            code: None,
            error: Some("invalid_callback".into()),
        };
    };
    if parsed.path() != OAUTH_CALLBACK_PATH {
        return OAuthCallback {
            state: Zeroizing::new(String::new()),
            code: None,
            error: Some("invalid_callback".into()),
        };
    }
    let mut names = HashSet::new();
    let mut state = None;
    let mut code = None;
    let mut error = None;
    let mut error_description = false;
    for (key, value) in parsed.query_pairs() {
        if !names.insert(key.as_ref().to_owned()) {
            return OAuthCallback { state: Zeroizing::new(String::new()), code: None, error: Some("invalid_callback".into()) };
        }
        match key.as_ref() {
            "state" => state = Some(Zeroizing::new(value.into_owned())),
            "code" => code = Some(Zeroizing::new(value.into_owned())),
            "error" => error = Some("authorization_denied".into()),
            "error_description" => error_description = true,
            _ => return OAuthCallback { state: Zeroizing::new(String::new()), code: None, error: Some("invalid_callback".into()) },
        }
    }
    if state.is_none() || (code.is_some() == error.is_some()) || (error_description && error.is_none()) {
        return OAuthCallback { state: Zeroizing::new(String::new()), code: None, error: Some("invalid_callback".into()) };
    }
    OAuthCallback { state: state.unwrap_or_default(), code, error }
}

fn write_callback_response(stream: &mut TcpStream, success: bool) {
    let body = if success {
        "<html><body style=\"font-family:sans-serif\"><p>เชื่อมต่อ Google Drive สำเร็จ ปิดหน้าต่างนี้ได้เลย</p></body></html>"
    } else {
        "<html><body style=\"font-family:sans-serif\"><p>เชื่อมต่อ Google Drive ไม่สำเร็จ ปิดหน้าต่างนี้ได้เลย</p></body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn remove_pending(state: &DriveOAuthState, session_id: &str) {
    if let Ok(mut pending) = state.pending.lock() {
        if pending
            .as_ref()
            .is_some_and(|entry| entry.session_id == session_id)
        {
            *pending = None;
        }
    }
}

#[tauri::command]
pub(crate) async fn broker_drive_connect_begin(
    app: tauri::AppHandle,
    state: State<'_, DriveOAuthState>,
) -> Result<DriveOAuthStart, String> {
    {
        let pending_guard = state
            .pending
            .lock()
            .map_err(|_| public_error("drive_oauth_state_unavailable"))?;
        if pending_guard.is_some() {
            return Err(public_error("drive_oauth_already_running"));
        }
    }

    let auth =
        native_auth::authorize_drive(&app, DriveOperation::ConnectionAuthorize)
            .await?;
    let _ = auth.into_invocation()?;
    let client_id = configured_client_id()?;
    let mut pending_guard = state
        .pending
        .lock()
        .map_err(|_| public_error("drive_oauth_state_unavailable"))?;
    if pending_guard.is_some() {
        return Err(public_error("drive_oauth_already_running"));
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|_| public_error("drive_oauth_callback_unavailable"))?;
    listener
        .set_nonblocking(true)
        .map_err(|_| public_error("drive_oauth_callback_unavailable"))?;
    let port = listener
        .local_addr()
        .map_err(|_| public_error("drive_oauth_callback_unavailable"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}{OAUTH_CALLBACK_PATH}");
    let (code_verifier, code_challenge) = pkce_pair();
    let oauth_state = Zeroizing::new(random_state());
    let session_id = Uuid::new_v4().to_string();
    let callback = Arc::new(Mutex::new(None));
    let terminal = OAuthTerminal::default();
    let callback_for_thread = Arc::clone(&callback);
    let terminal_for_thread = terminal.clone();
    thread::spawn(move || {
        let deadline = Instant::now() + OAUTH_TTL;
        loop {
            if terminal_for_thread.is_cancelled() || Instant::now() >= deadline {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = Zeroizing::new([0u8; 8192]);
                    let bytes_read = stream.read(&mut buffer[..]).unwrap_or(0);
                    let result = callback_from_request(&buffer[..bytes_read]);
                    write_callback_response(&mut stream, result.code.is_some());
                    if let Ok(mut slot) = callback_for_thread.lock() {
                        *slot = Some(result);
                    }
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    let authorization_url =
        build_authorization_url(&client_id, &redirect_uri, &oauth_state, &code_challenge);
    *pending_guard = Some(PendingOAuth {
        session_id: session_id.clone(),
        oauth_client_id: client_id,
        redirect_uri: redirect_uri.clone(),
        state: oauth_state,
        code_verifier,
        callback,
        terminal,
        expires_at: Instant::now() + OAUTH_TTL,
    });
    if app
        .opener()
        .open_url(authorization_url, None::<&str>)
        .is_err()
    {
        *pending_guard = None;
        return Err(public_error("drive_oauth_open_failed"));
    }
    Ok(DriveOAuthStart { request_id: session_id, scope: DRIVE_SCOPE, expires_at_ms: now_ms() + OAUTH_TTL.as_millis() as u64 })
}

#[tauri::command]
pub(crate) async fn broker_drive_connect_complete(
    session_id: String,
    app: tauri::AppHandle,
    state: State<'_, DriveOAuthState>,
) -> Result<DriveConnectionStatus, String> {
    let pending = state
        .pending
        .lock()
        .map_err(|_| public_error("drive_oauth_state_unavailable"))?
        .as_ref()
        .filter(|entry| entry.session_id == session_id)
        .cloned()
        .ok_or_else(|| public_error("drive_oauth_session_missing"))?;
    let callback = pending.callback.clone();
    let terminal = pending.terminal.clone();
    let expires_at = pending.expires_at;
    let callback_result = tauri::async_runtime::spawn_blocking(move || loop {
        if terminal.is_cancelled() {
            return Err(public_error("drive_oauth_cancelled"));
        }
        if Instant::now() >= expires_at {
            return Err(public_error("drive_oauth_expired"));
        }
        if let Ok(mut slot) = callback.lock() {
            if let Some(result) = slot.take() {
                return Ok(result);
            }
        }
        thread::sleep(Duration::from_millis(50));
    })
    .await
    .map_err(|_| public_error("drive_oauth_callback_failed"))?;
    let callback_result = match callback_result {
        Ok(result) => result,
        Err(error) => {
            remove_pending(&state, &session_id);
            return Err(error);
        }
    };

    if callback_result.state != pending.state {
        remove_pending(&state, &session_id);
        return Err(public_error("drive_oauth_state_mismatch"));
    }
    if callback_result.error.is_some() || callback_result.code.is_none() {
        remove_pending(&state, &session_id);
        return Err(public_error(
            callback_result
                .error
                .as_deref()
                .unwrap_or("authorization_denied"),
        ));
    }
    let code = callback_result.code.unwrap_or_default();
    if let Err(error) = pending.terminal.begin_exchange() {
        remove_pending(&state, &session_id);
        return Err(error);
    }
    let authorization = match native_auth::authorize_drive(&app, DriveOperation::ConnectionAuthorize)
    .await
    {
        Ok(context) => match context.into_invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                pending.terminal.mark_failed();
                remove_pending(&state, &session_id);
                return Err(error);
            }
        },
        Err(error) => {
            pending.terminal.mark_failed();
            remove_pending(&state, &session_id);
            return Err(error);
        }
    };
    let token = match exchange_code(&pending, &authorization, code).await {
        Ok(token) => token,
        Err(error) => {
            pending.terminal.mark_failed();
            remove_pending(&state, &session_id);
            return Err(error);
        }
    };
    if token
        .scope
        .as_deref()
        .is_some_and(|scope| !exact_scope(scope))
        || token.scope.is_none()
    {
        pending.terminal.mark_failed();
        remove_pending(&state, &session_id);
        return Err(public_error("drive_oauth_scope_mismatch"));
    }
    let refresh_token = match token.refresh {
        Some(refresh_token) => refresh_token,
        None => {
            pending.terminal.mark_failed();
            remove_pending(&state, &session_id);
            return Err(public_error("drive_oauth_offline_access_missing"));
        }
    };
    if let Err(error) = pending.terminal.begin_commit() {
        remove_pending(&state, &session_id);
        return Err(error);
    }
    if let Err(_error) = native_auth::authorize_drive(&app, DriveOperation::ConnectionActivate).await {
        pending.terminal.mark_failed();
        remove_pending(&state, &session_id);
        return Err(public_error("drive_connection_activation_failed"));
    }
    if let Err(error) = save_refresh_token(&authorization, refresh_token) {
        pending.terminal.mark_failed();
        remove_pending(&state, &session_id);
        return Err(error);
    }
    pending.terminal.mark_completed();
    remove_pending(&state, &session_id);
    Ok(DriveConnectionStatus {
        connected: true,
        scope: Some(DRIVE_SCOPE.to_owned()),
        provider: "google_drive",
    })
}

async fn exchange_code(
    pending: &PendingOAuth,
    invocation: &DriveInvocation,
    code: Zeroizing<String>,
) -> Result<TokenResponse, String> {
    invocation.ensure_valid()?;
    let response = async_http_client(Duration::from_secs(30), "drive_oauth_token_exchange_failed")?
        .post(DRIVE_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", pending.oauth_client_id.as_str()),
            ("code", code.as_str()),
            ("code_verifier", pending.code_verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", pending.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|_| public_error("drive_oauth_token_exchange_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_oauth_token_exchange_failed"));
    }
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|_| public_error("drive_oauth_token_exchange_failed"))?;
    if token.error.is_some() || token.access.is_none() {
        return Err(public_error("drive_oauth_token_exchange_failed"));
    }
    Ok(token)
}

#[tauri::command]
pub(crate) fn broker_drive_connect_cancel(
    session_id: String,
    state: State<'_, DriveOAuthState>,
) -> Result<DriveCancelled, String> {
    let pending = state
        .pending
        .lock()
        .map_err(|_| public_error("drive_oauth_state_unavailable"))?
        .as_ref()
        .cloned()
        .ok_or_else(|| public_error("drive_oauth_session_missing"))?;
    if pending.session_id != session_id {
        return Err(public_error("drive_oauth_session_mismatch"));
    }
    pending.terminal.cancel()?;
    remove_pending(&state, &session_id);
    Ok(DriveCancelled { request_id: session_id, status: "cancelled" })
}

#[tauri::command]
pub(crate) async fn broker_drive_status(
    app: tauri::AppHandle,
) -> Result<DriveConnectionStatus, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::ConnectionRead)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::ConnectionRead)?;
    Ok(DriveConnectionStatus {
        connected: load_refresh_token(&authorization)?.is_some(),
        scope: Some(DRIVE_SCOPE.to_owned()),
        provider: "google_drive",
    })
}

#[tauri::command]
pub(crate) async fn broker_drive_disconnect(
    app: tauri::AppHandle,
) -> Result<DriveConnectionStatus, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::ConnectionRevoke)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::ConnectionRevoke)?;
    delete_refresh_token(&authorization)?;
    Ok(DriveConnectionStatus {
        connected: false,
        scope: Some(DRIVE_SCOPE.to_owned()),
        provider: "google_drive",
    })
}

async fn access_token(invocation: &DriveInvocation) -> Result<Zeroizing<String>, String> {
    invocation.ensure_valid()?;
    let client_id = configured_client_id()?;
    let refresh_token =
        load_refresh_token(invocation)?.ok_or_else(|| public_error("drive_not_connected"))?;
    let response = async_http_client(Duration::from_secs(30), "drive_token_refresh_failed")?
        .post(DRIVE_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|_| public_error("drive_token_refresh_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_token_refresh_failed"));
    }
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|_| public_error("drive_token_refresh_failed"))?;
    if token.error.is_some() || token.access.is_none() {
        return Err(public_error("drive_token_refresh_failed"));
    }
    let returned_scope = token.scope.as_deref();
    if returned_scope.is_none_or(|scope| !exact_scope(scope)) {
        return Err(public_error("drive_oauth_scope_mismatch"));
    }
    token.access.ok_or_else(|| public_error("drive_token_refresh_failed"))
}

fn drive_query_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

async fn list_files(
    invocation: &DriveInvocation,
    access_token: &str,
    query: &str,
) -> Result<Vec<DriveFile>, String> {
    invocation.ensure_valid()?;
    let response = async_http_client(Duration::from_secs(120), "drive_list_failed")?
        .get(DRIVE_FILES_ENDPOINT)
        .bearer_auth(access_token)
        .query(&[
            ("q", query),
            ("spaces", "appDataFolder"),
            ("fields", "files(id,name,size,modifiedTime,appProperties)"),
            ("pageSize", "1000"),
        ])
        .send()
        .await
        .map_err(|_| public_error("drive_list_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_list_failed"));
    }
    let payload = response
        .json::<DriveFileList>()
        .await
        .map_err(|_| public_error("drive_list_failed"))?;
    validate_drive_file_ids(payload.files, "drive_list_failed")
}

#[tauri::command]
pub(crate) async fn broker_drive_list_archives(
    app: tauri::AppHandle,
) -> Result<Vec<DriveArchiveSummary>, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::BackupRead)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::BackupRead)?;
    let token = access_token(&authorization).await?;
    let files = list_files(
        &authorization,
        token.as_str(),
        "'appDataFolder' in parents and trashed = false and name contains '.fungbk'",
    )
    .await?;
    Ok(files
        .into_iter()
        .filter_map(|file| {
            let name = file.name.as_deref()?;
            let archive_id = name.strip_suffix(ARCHIVE_SUFFIX)?.to_owned();
            if validate_drive_file_contract(&file, &archive_id, "archive", None).is_err() {
                return None;
            }
            Some(DriveArchiveSummary {
                file_id: file.id,
                archive_id,
                byte_count: file
                    .size
                    .as_deref()
                    .and_then(|size| size.parse().ok())
                    ?,
                digest: file
                    .app_properties
                    .as_ref()
                    .and_then(|properties| properties.get("fungDigest").cloned()),
                modified_time: file.modified_time,
            })
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn broker_drive_upload_archive(
    app: tauri::AppHandle,
    archive_id: String,
    fs_state: State<'_, FilesystemBackupState>,
) -> Result<DriveArchiveSummary, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::BackupWrite)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::BackupWrite)?;
    validate_archive_id(&archive_id)?;
    let root = fs_state
        .current_root()
        .ok_or_else(|| public_error("filesystem_backup_root_unavailable"))?;
    let (archive_path, record, manifest) =
        filesystem_backup::verified_archive_details_at_root(&root, &archive_id)
            .map_err(|_| public_error("filesystem_archive_unavailable"))?;
    let token = access_token(&authorization).await?;
    let digest = record.digest.clone();
    let archive_name = format!("{archive_id}{ARCHIVE_SUFFIX}");
    let manifest_name = format!("{archive_id}{MANIFEST_SUFFIX}");
    let manifest_bytes = serde_json::to_vec(&DriveManifestEnvelope {
        archive: manifest,
        record,
    })
    .map_err(|_| public_error("drive_manifest_invalid"))?;
    let expected_size = archive_path
        .metadata()
        .map_err(|_| public_error("filesystem_archive_unavailable"))?
        .len();
    let result = tauri::async_runtime::spawn_blocking(move || {
        authorization.ensure_valid()?;
        upload_archive_files(
            &authorization,
            token.as_str(),
            &archive_path,
            expected_size,
            &archive_name,
            &manifest_name,
            &manifest_bytes,
            &archive_id,
            &digest,
        )
    })
    .await
    .map_err(|_| public_error("drive_upload_failed"))??;
    Ok(result)
}

fn upload_archive_files(
    invocation: &DriveInvocation,
    access_token: &str,
    archive_path: &Path,
    expected_size: u64,
    archive_name: &str,
    manifest_name: &str,
    manifest_bytes: &[u8],
    archive_id: &str,
    digest: &str,
) -> Result<DriveArchiveSummary, String> {
    invocation.ensure_valid()?;
    let client = BlockingClient::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| public_error("drive_upload_failed"))?;
    let query = format!(
        "'appDataFolder' in parents and trashed = false and name = '{}'",
        drive_query_literal(archive_name)
    );
    let existing = blocking_list_files(invocation, &client, access_token, &query)?;
    if let Some(file) = existing.into_iter().next() {
        if validate_drive_file_contract(&file, archive_id, "archive", Some(digest)).is_ok() {
            return summary_from_file(file, archive_id);
        }
        return Err(public_error("drive_archive_already_exists"));
    }

    let manifest_query = format!(
        "'appDataFolder' in parents and trashed = false and name = '{}'",
        drive_query_literal(manifest_name)
    );
    if !blocking_list_files(invocation, &client, access_token, &manifest_query)?.is_empty() {
        return Err(public_error("drive_manifest_already_exists"));
    }

    let manifest_metadata = serde_json::json!({
        "name": manifest_name,
        "parents": ["appDataFolder"],
        "mimeType": "application/json",
        "appProperties": {
            "fungArchiveId": archive_id,
            "fungKind": "manifest",
            "fungDigest": digest
        }
    });
    let manifest_file = upload_small_file(
        invocation,
        &client,
        access_token,
        &manifest_metadata,
        manifest_bytes,
        "application/json",
    )?;
    validate_drive_file_contract(&manifest_file, archive_id, "manifest", Some(digest))?;

    let archive_metadata = serde_json::json!({
        "name": archive_name,
        "parents": ["appDataFolder"],
        "mimeType": "application/octet-stream",
        "appProperties": {
            "fungArchiveId": archive_id,
            "fungKind": "archive",
            "fungDigest": digest
        }
    });
    let archive = match upload_resumable_file(
        invocation,
        &client,
        access_token,
        &archive_metadata,
        archive_path,
        expected_size,
    ) {
        Ok(file) => file,
        Err(error) => {
            let _ = blocking_delete_file(invocation, &client, access_token, &manifest_file.id);
            return Err(error);
        }
    };
    validate_drive_file_contract(&archive, archive_id, "archive", Some(digest))?;
    summary_from_file(archive, archive_id)
}

fn summary_from_file(file: DriveFile, archive_id: &str) -> Result<DriveArchiveSummary, String> {
    let byte_count = file.size.as_deref().and_then(|size| size.parse().ok()).ok_or_else(|| public_error("drive_provider_file_invalid"))?;
    Ok(DriveArchiveSummary {
        file_id: file.id,
        archive_id: archive_id.to_owned(),
        byte_count,
        digest: file
            .app_properties
            .as_ref()
            .and_then(|properties| properties.get("fungDigest").cloned()),
        modified_time: file.modified_time,
    })
}

fn blocking_list_files(
    invocation: &DriveInvocation,
    client: &BlockingClient,
    access_token: &str,
    query: &str,
) -> Result<Vec<DriveFile>, String> {
    invocation.ensure_valid()?;
    let response = client
        .get(DRIVE_FILES_ENDPOINT)
        .bearer_auth(access_token)
        .query(&[
            ("q", query),
            ("spaces", "appDataFolder"),
            ("fields", "files(id,name,size,modifiedTime,appProperties)"),
            ("pageSize", "1000"),
        ])
        .send()
        .map_err(|_| public_error("drive_upload_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_upload_failed"));
    }
    let payload = response
        .json::<DriveFileList>()
        .map_err(|_| public_error("drive_upload_failed"))?;
    validate_drive_file_ids(payload.files, "drive_upload_failed")
}

fn upload_small_file(
    invocation: &DriveInvocation,
    client: &BlockingClient,
    access_token: &str,
    metadata: &serde_json::Value,
    bytes: &[u8],
    mime_type: &str,
) -> Result<DriveFile, String> {
    invocation.ensure_valid()?;
    let metadata_part = multipart::Part::text(metadata.to_string())
        .mime_str("application/json")
        .map_err(|_| public_error("drive_upload_failed"))?;
    let file_part = multipart::Part::bytes(bytes.to_vec())
        .mime_str(mime_type)
        .map_err(|_| public_error("drive_upload_failed"))?;
    let response = client
        .post(format!("{DRIVE_UPLOAD_ENDPOINT}?uploadType=multipart"))
        .bearer_auth(access_token)
        .query(&[("fields", "id,name,size,modifiedTime,appProperties")])
        .multipart(
            multipart::Form::new()
                .part("metadata", metadata_part)
                .part("file", file_part),
        )
        .send()
        .map_err(|_| public_error("drive_upload_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_upload_failed"));
    }
    response
        .json::<DriveFile>()
        .map_err(|_| public_error("drive_upload_failed"))
}

fn upload_resumable_file(
    invocation: &DriveInvocation,
    client: &BlockingClient,
    access_token: &str,
    metadata: &serde_json::Value,
    archive_path: &Path,
    total_size: u64,
) -> Result<DriveFile, String> {
    invocation.ensure_valid()?;
    let start_response = client
        .post(format!("{DRIVE_UPLOAD_ENDPOINT}?uploadType=resumable"))
        .bearer_auth(access_token)
        .header("X-Upload-Content-Type", "application/octet-stream")
        .header("X-Upload-Content-Length", total_size)
        .header("Content-Type", "application/json; charset=UTF-8")
        .body(metadata.to_string())
        .send()
        .map_err(|_| public_error("drive_upload_failed"))?;
    if !start_response.status().is_success() {
        return Err(public_error("drive_upload_failed"));
    }
    let location = start_response
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| public_error("drive_upload_failed"))?
        .to_owned();
    let mut file = File::open(archive_path).map_err(|_| public_error("drive_upload_failed"))?;
    let mut next_offset = 0u64;
    let mut buffer = vec![0u8; DRIVE_CHUNK_SIZE];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| public_error("drive_upload_failed"))?;
        if read == 0 {
            return Err(public_error("drive_upload_failed"));
        }
        let end = next_offset + read as u64 - 1;
        invocation.ensure_valid()?;
        let response = client
            .put(&location)
            .bearer_auth(access_token)
            .header("Content-Length", read)
            .header(
                "Content-Range",
                format!("bytes {next_offset}-{end}/{total_size}"),
            )
            .body(buffer[..read].to_vec())
            .send()
            .map_err(|_| public_error("drive_upload_failed"))?;
        invocation.ensure_valid()?;
        if response.status().is_success() {
            return response
                .json::<DriveFile>()
                .map_err(|_| public_error("drive_upload_failed"));
        }
        if response.status().as_u16() != 308 {
            return Err(public_error("drive_upload_failed"));
        }
        let acknowledged = response
            .headers()
            .get("Range")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit('-').next())
            .and_then(|value| value.parse::<u64>().ok())
            .map(|end| end + 1)
            .unwrap_or(end + 1);
        if acknowledged > end + 1 || acknowledged > total_size {
            return Err(public_error("drive_upload_failed"));
        }
        if acknowledged != end + 1 {
            file.seek(SeekFrom::Start(acknowledged))
                .map_err(|_| public_error("drive_upload_failed"))?;
        }
        next_offset = acknowledged;
        if next_offset >= total_size {
            return Err(public_error("drive_upload_failed"));
        }
    }
}

fn blocking_delete_file(
    invocation: &DriveInvocation,
    client: &BlockingClient,
    access_token: &str,
    file_id: &str,
) -> Result<(), String> {
    invocation.ensure_valid()?;
    validate_drive_file_id(file_id)?;
    let response = client
        .delete(format!("{DRIVE_FILES_ENDPOINT}/{file_id}"))
        .bearer_auth(access_token)
        .send()
        .map_err(|_| public_error("drive_upload_failed"))?;
    if response.status().is_success() || response.status().as_u16() == 404 {
        Ok(())
    } else {
        Err(public_error("drive_upload_failed"))
    }
}

async fn download_file(
    invocation: &DriveInvocation,
    access_token: &str,
    file_id: &str,
) -> Result<Vec<u8>, String> {
    invocation.ensure_valid()?;
    validate_drive_file_id(file_id)?;
    let mut response = async_http_client(Duration::from_secs(120), "drive_download_failed")?
        .get(format!("{DRIVE_FILES_ENDPOINT}/{file_id}"))
        .bearer_auth(access_token)
        .query(&[("alt", "media")])
        .send()
        .await
        .map_err(|_| public_error("drive_download_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_download_failed"));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_DRIVE_DOWNLOAD_BYTES)
    {
        return Err(public_error("drive_archive_too_large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| public_error("drive_download_failed"))?
    {
        invocation.ensure_valid()?;
        if bytes.len() as u64 + chunk.len() as u64 > MAX_DRIVE_DOWNLOAD_BYTES {
            return Err(public_error("drive_archive_too_large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[tauri::command]
pub(crate) async fn broker_drive_restore_intent(
    app: tauri::AppHandle,
    archive_id: String,
    job_state: State<'_, BackupJobState>,
) -> Result<String, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::BackupRestore)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::BackupRestore)?;
    validate_archive_id(&archive_id)?;
    job_state.issue_restore_intent(&archive_id)
}

#[tauri::command]
pub(crate) async fn broker_drive_restore(
    app: tauri::AppHandle,
    file_id: String,
    archive_id: String,
    recovery_phrase: String,
    restore_intent_id: String,
    app_state: State<'_, crate::AppState>,
    job_state: State<'_, BackupJobState>,
) -> Result<RestoreResult, String> {
    let authorization =
        native_auth::authorize_drive(&app, DriveOperation::BackupRestore)
            .await?
            .into_invocation()?;
    authorization.require_operation(DriveOperation::BackupRestore)?;
    validate_archive_id(&archive_id)?;
    validate_drive_file_id(&file_id)?;
    if recovery_phrase.trim().is_empty() {
        return Err(public_error("missing_recovery_phrase"));
    }
    if restore_intent_id.trim().is_empty() || restore_intent_id.len() > 128 {
        return Err(public_error("restore_intent_invalid"));
    }
    let guard = job_state
        .acquire_job()
        .map_err(|_| public_error("backup_job_already_running"))?;
    let restore_parent = job_state.consume_restore_intent(&restore_intent_id, &archive_id)?;
    if restore_parent.starts_with(&app_state.data_root) {
        return Err(public_error("restore_target_must_be_outside_app_data"));
    }
    let token = access_token(&authorization).await?;
    let archive_name = format!("{archive_id}{ARCHIVE_SUFFIX}");
    let archive_query = format!(
        "'appDataFolder' in parents and trashed = false and name = '{}'",
        drive_query_literal(&archive_name)
    );
    let archive_file = list_files(&authorization, token.as_str(), &archive_query)
        .await?
        .into_iter()
        .find(|file| file.id == file_id)
        .ok_or_else(|| public_error("drive_archive_not_found"))?;
    validate_drive_file_contract(&archive_file, &archive_id, "archive", None)?;
    let manifest_name = format!("{archive_id}{MANIFEST_SUFFIX}");
    let manifest_query = format!(
        "'appDataFolder' in parents and trashed = false and name = '{}'",
        drive_query_literal(&manifest_name)
    );
    let manifest_file = list_files(&authorization, token.as_str(), &manifest_query)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| public_error("drive_manifest_not_found"))?;
    validate_drive_file_contract(&manifest_file, &archive_id, "manifest", None)?;
    let manifest_file_id = manifest_file.id.clone();
    let archive_bytes = download_file(&authorization, token.as_str(), &file_id).await?;
    let manifest_bytes = download_file(&authorization, token.as_str(), &manifest_file_id).await?;
    let envelope: DriveManifestEnvelope = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| public_error("drive_manifest_invalid"))?;
    if envelope.record.archive_id != archive_id
        || envelope.archive.archive_id != archive_id
        || envelope.record.digest != envelope.archive.sha256
        || envelope.archive.byte_count != archive_bytes.len() as u64
        || envelope.archive.sha256 != sha256_hex(&archive_bytes)
    {
        return Err(public_error("drive_archive_digest_mismatch"));
    }
    let archive_provider_digest = archive_file
        .app_properties
        .as_ref()
        .and_then(|properties| properties.get("fungDigest"))
        .ok_or_else(|| public_error("drive_archive_digest_mismatch"))?;
    let manifest_provider_digest = manifest_file
        .app_properties
        .as_ref()
        .and_then(|properties| properties.get("fungDigest"))
        .ok_or_else(|| public_error("drive_archive_digest_mismatch"))?;
    if archive_provider_digest != &envelope.archive.sha256
        || manifest_provider_digest != &envelope.archive.sha256
    {
        return Err(public_error("drive_archive_digest_mismatch"));
    }
    let mut manifest = envelope.archive;
    manifest.terminal_state = backup_archive::ENCRYPTED_TERMINAL_STATE.to_owned();
    let envelope = backup_archive::ArchiveEnvelope {
        manifest,
        bytes: archive_bytes,
    };
    let recovery_phrase = Zeroizing::new(recovery_phrase);
    let work_dir = app_state.data_root.join("backup-staging");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let outcome = backup::run_restore_job_from_envelope(
            envelope,
            &restore_parent,
            &work_dir,
            &recovery_phrase,
        );
        drop(guard);
        outcome
    })
    .await
    .map_err(|_| public_error("restore_failed"))?;
    result.map_err(|_| public_error("backup_verification_failed"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_uses_pkce_and_only_drive_appdata_scope() {
        let url = build_authorization_url(
            "123.apps.googleusercontent.com",
            "http://127.0.0.1:4000/oauth/google-drive/callback",
            "state-value",
            "challenge-value",
        );
        let parsed = Url::parse(&url).unwrap();
        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query.get("scope").map(String::as_str), Some(DRIVE_SCOPE));
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            query.get("access_type").map(String::as_str),
            Some("offline")
        );
        assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));
    }

    #[test]
    fn callback_parser_discards_provider_error_details() {
        let callback = callback_from_request(
            b"GET /oauth/google-drive/callback?state=s1&error=access_denied&error_description=secret HTTP/1.1\r\n\r\n",
        );
        assert_eq!(callback.error.as_deref(), Some("authorization_denied"));
        assert_ne!(callback.error.as_deref(), Some("secret"));
    }

    #[test]
    fn callback_parser_keeps_code_and_state() {
        let callback = callback_from_request(
            b"GET /oauth/google-drive/callback?state=s1&code=c1 HTTP/1.1\r\n\r\n",
        );
        assert_eq!(callback.state.as_str(), "s1");
        assert_eq!(callback.code.as_ref().map(|value| value.as_str()), Some("c1"));
        assert!(callback.error.is_none());
    }

    #[test]
    fn callback_parser_rejects_a_non_callback_path() {
        let callback = callback_from_request(b"GET /other?state=s1&code=c1 HTTP/1.1\r\n\r\n");
        assert_eq!(callback.error.as_deref(), Some("invalid_callback"));
        assert!(callback.code.is_none());
    }

    #[test]
    fn scope_is_exact_and_rejects_escalation() {
        assert!(exact_scope(DRIVE_SCOPE));
        assert!(!exact_scope(""));
        assert!(!exact_scope(
            "openid https://www.googleapis.com/auth/drive.appdata"
        ));
        assert!(!exact_scope("https://www.googleapis.com/auth/drive"));
    }

    #[test]
    fn provider_archive_metadata_is_bound_to_app_data_and_digest() {
        let file = DriveFile {
            id: "file-123".to_owned(),
            name: Some("archive-1.fungbk".to_owned()),
            size: Some("42".to_owned()),
            modified_time: None,
            app_properties: Some(std::collections::HashMap::from([
                ("fungArchiveId".to_owned(), "archive-1".to_owned()),
                ("fungKind".to_owned(), "archive".to_owned()),
                ("fungDigest".to_owned(), "a".repeat(64)),
            ])),
        };
        assert!(validate_drive_file_contract(
            &file,
            "archive-1",
            "archive",
            Some("a".repeat(64).as_str()),
        )
        .is_ok());

        let mut wrong_kind = file.clone();
        wrong_kind
            .app_properties
            .as_mut()
            .expect("properties")
            .insert("fungKind".to_owned(), "manifest".to_owned());
        assert!(validate_drive_file_contract(&wrong_kind, "archive-1", "archive", None,).is_err());
    }

    #[test]
    fn keyring_slot_is_stable_without_exposing_identity() {
        let slot = keyring_slot_for("user-a", "device-a", "fingerprint-a");
        assert!(slot.starts_with("drive-token-"));
        assert_eq!(slot.len(), "drive-token-".len() + 64);
        assert!(!slot.contains("user-a"));
        assert_eq!(
            slot,
            keyring_slot_for("user-a", "device-a", "fingerprint-a")
        );
        assert_ne!(
            slot,
            keyring_slot_for("user-b", "device-a", "fingerprint-a")
        );
        assert_ne!(
            slot,
            keyring_slot_for("user-a", "device-b", "fingerprint-a")
        );
    }

    #[test]
    fn oauth_terminal_linearizes_cancel_before_keyring_commit() {
        let terminal = OAuthTerminal::default();
        terminal.begin_exchange().unwrap();
        terminal.cancel().unwrap();
        assert_eq!(
            terminal.begin_commit().unwrap_err(),
            public_error("drive_oauth_cancelled")
        );
    }

    #[test]
    fn oauth_terminal_rejects_cancel_after_commit_begins() {
        let terminal = OAuthTerminal::default();
        terminal.begin_exchange().unwrap();
        terminal.begin_commit().unwrap();
        assert_eq!(
            terminal.cancel().unwrap_err(),
            public_error("drive_oauth_completed")
        );
    }

    #[test]
    fn native_behavioral_drive_callback_state_is_zeroizing_and_terminal_cleanup() {
        let callback = callback_from_request(
            b"GET /oauth/google-drive/callback?state=drive-state&code=drive-code HTTP/1.1\r\n\r\n",
        );
        assert_eq!(callback.state.as_str(), "drive-state");
        assert_eq!(callback.code.as_ref().map(|value| value.as_str()), Some("drive-code"));
        let terminal = OAuthTerminal::default();
        terminal.begin_exchange().unwrap();
        terminal.cancel().unwrap();
        assert!(terminal.is_cancelled());
        drop(callback);
    }

    #[test]
    fn drive_query_literal_escapes_provider_query_delimiters() {
        assert_eq!(drive_query_literal("a'b\\c"), "a\\'b\\\\c");
    }
}
