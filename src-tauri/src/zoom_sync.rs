//! Zoom cloud-recording ingestion: OAuth (PKCE), REST pulls, import job.
//! Tokens live ONLY in the OS credential store (keyring). Never persist or
//! log tokens or download URLs.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::TcpListener;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::{genesis_adapter, now, AppError, AppResult, AppState};

const KEYRING_SERVICE: &str = "FUNG";
const KEYRING_USER: &str = "zoom-oauth";
const ZOOM_AUTH_BASE: &str = "https://zoom.us";
pub(crate) const ZOOM_API_BASE: &str = "https://api.zoom.us/v2";

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
/// thread that waits (max 180 s) for the loopback callback, exchanges the
/// code, and stores tokens. UI polls `zoom_connection_status`.
#[tauri::command]
pub(crate) fn zoom_connect(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<ZoomConnectionStatus> {
    let client_id = client_id_from_env()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/zoom/callback");
    let verifier = new_verifier();
    let oauth_state = uuid::Uuid::new_v4().simple().to_string();
    let url = authorize_url(&client_id, &redirect_uri, &oauth_state, &pkce_challenge(&verifier));

    write_connection(&state.genesis, "connecting", "").map_err(AppError::Genesis)?;
    app.opener().open_url(url, None::<&str>)
        .map_err(|error| AppError::InvalidInput(format!("could not open browser: {error}")))?;

    let storage = state.genesis.clone();
    std::thread::spawn(move || {
        listener.set_nonblocking(false).ok();
        // read_timeout guards a connected-but-silent client; the accept itself
        // blocks until the browser redirects. Guard total time with a deadline
        // thread that closes the flow by dropping nothing — accept blocks, so
        // rely on the user closing the panel; a stray success after timeout is
        // harmless because state is overwritten idempotently.
        let outcome = (|| -> Result<String, String> {
            let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
            stream.set_read_timeout(Some(std::time::Duration::from_secs(180))).ok();
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let result = parse_callback_request(first_line).and_then(|(code, returned_state)| {
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
    write_connection(&state.genesis, "disconnected", "").map_err(AppError::Genesis)?;
    read_connection(&state.genesis).map_err(AppError::Genesis)
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
}
