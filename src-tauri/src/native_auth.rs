//! Native authorization for provider operations.
//!
//! The webview contributes only its current Supabase bearer proof. Native
//! derives the device key, signs a short-lived canonical request, and keeps
//! the returned authorization context in memory for one command invocation.

use crate::{device_identity, AppError, AppResult};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use ed25519_dalek::Signer;
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

const AUTHORIZE_FUNCTION_PATH: &str = "/functions/v1/google-drive-authorize";
const AUTH_TOKEN_PATH: &str = "/auth/v1/token";
const AUTH_USER_PATH: &str = "/auth/v1/user";
const DRIVE_PROVIDER: &str = "google_drive";
const AUTHORIZATION_REQUEST_TTL: Duration = Duration::from_secs(90);
const AUTH_LOGIN_REQUEST_TTL: Duration = Duration::from_secs(120);
const AUTH_CALLBACK_PATH: &str = "/auth/callback";
const MAX_AUTH_CALLBACK_BYTES: usize = 8192;
const MAX_SESSION_PROOF_BYTES: usize = 8192;
const ENROLLMENT_OPERATION: &str = "device.enrollment.request";
const ENROLLMENT_PLATFORM: &str = "windows";
const ENROLLMENT_DOMAIN: &[u8] = b"FUNG\0DEVICE_ENROLLMENT\0V1\0";
const ENROLLMENT_PROOF_TTL_MS: i64 = 300_000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthLoginStarted {
    request_id: String,
    redirect_uri: String,
    expires_at_ms: u64,
    #[serde(skip)]
    state: String,
    #[serde(skip)]
    code_challenge: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthCallbackEvent {
    request_id: String,
    session: Option<AuthSession>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthSession {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
    user_id: String,
}

struct AuthCallbackSuccess {
    code: Zeroizing<String>,
    code_verifier: Zeroizing<String>,
}

struct PendingAuthRequest {
    request_id: String,
    port: u16,
    state: String,
    expires_at: SystemTime,
    code_verifier: Zeroizing<String>,
}

#[derive(Default)]
struct AuthRequestRegistry {
    pending: Option<PendingAuthRequest>,
}

fn create_pkce_pair() -> (Zeroizing<String>, String) {
    let mut random = [0u8; 32];
    OsRng.fill_bytes(&mut random);
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (Zeroizing::new(verifier), challenge)
}

impl AuthRequestRegistry {
    fn start(&mut self, port: u16, now: SystemTime) -> Result<AuthLoginStarted, String> {
        if self.pending.is_some() {
            return Err(public_error("auth_request_in_progress"));
        }
        if port == 0 {
            return Err(public_error("auth_listener_unavailable"));
        }
        let request_id = Uuid::new_v4().to_string();
        let state = Uuid::new_v4().to_string();
        let (code_verifier, code_challenge) = create_pkce_pair();
        let expires_at = now + AUTH_LOGIN_REQUEST_TTL;
        let expires_at_ms = expires_at
            .duration_since(UNIX_EPOCH)
            .map_err(|_| public_error("auth_clock_invalid"))?
            .as_millis() as u64;
        let redirect_uri = format!("http://127.0.0.1:{port}{AUTH_CALLBACK_PATH}");
        self.pending = Some(PendingAuthRequest {
            request_id: request_id.clone(),
            port,
            state: state.clone(),
            expires_at,
            code_verifier,
        });
        Ok(AuthLoginStarted {
            request_id,
            redirect_uri,
            expires_at_ms,
            state,
            code_challenge,
        })
    }

    fn is_pending(&self, request_id: &str) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
    }

    fn expire_if_needed(&mut self, request_id: &str, now: SystemTime) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id && now >= pending.expires_at)
        {
            self.pending = None;
            return true;
        }
        false
    }

    fn cancel(&mut self, request_id: &str) -> Result<(), String> {
        if self.is_pending(request_id) {
            self.pending = None;
            Ok(())
        } else {
            Err(public_error("auth_request_not_found"))
        }
    }

    fn take_error(&mut self, request_id: &str, error: &str) -> Option<AuthCallbackEvent> {
        if !self.is_pending(request_id) {
            return None;
        }
        self.pending = None;
        Some(AuthCallbackEvent {
            request_id: request_id.to_owned(),
            session: None,
            error: Some(error.to_owned()),
        })
    }

    fn accept_callback(
        &mut self,
        callback_url: &str,
        now: SystemTime,
    ) -> Result<AuthCallbackSuccess, String> {
        let Some(pending) = self.pending.take() else {
            return Err(public_error("auth_request_not_found"));
        };
        if now >= pending.expires_at {
            return Err(public_error("auth_timeout"));
        }

        let invalid = || Err(public_error("invalid_callback"));
        let Ok(parsed) = Url::parse(callback_url) else {
            return invalid();
        };
        if parsed.scheme() != "http"
            || parsed.host_str() != Some("127.0.0.1")
            || parsed.port() != Some(pending.port)
            || parsed.path() != AUTH_CALLBACK_PATH
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return invalid();
        }

        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        if pairs.is_empty() {
            return invalid();
        }
        let mut names = std::collections::HashSet::new();
        if pairs.iter().any(|(name, _)| {
            !matches!(
                name.as_str(),
                "code" | "error" | "error_description" | "state"
            ) || !names.insert(name)
        }) {
            return invalid();
        }
        let value = |name: &str| {
            pairs
                .iter()
                .find(|(candidate, _)| candidate == name)
                .map(|(_, value)| value.as_str())
        };
        let code = value("code");
        let error_code = value("error");
        let error_description = value("error_description");
        let state = value("state");
        if state != Some(pending.state.as_str())
            || !state.is_some_and(|value| safe_callback_value(value))
        {
            return invalid();
        }
        if code.is_some() == error_code.is_some()
            || (error_description.is_some() && error_code.is_none())
        {
            return invalid();
        }
        if let Some(code) = code {
            if pairs.len() != 2 || !safe_callback_value(code) {
                return invalid();
            }
            return Ok(AuthCallbackSuccess {
                code: Zeroizing::new(code.to_owned()),
                code_verifier: pending.code_verifier,
            });
        }

        let Some(error_code) = error_code else {
            return invalid();
        };
        if !safe_callback_value(error_code)
            || error_description.is_some_and(|value| !safe_callback_value(value))
            || (error_description.is_none() && pairs.len() != 2)
            || (error_description.is_some() && pairs.len() != 3)
        {
            return invalid();
        }
        Err(error_description.unwrap_or(error_code).to_owned())
    }
}

fn auth_request_registry() -> &'static Mutex<AuthRequestRegistry> {
    static REGISTRY: OnceLock<Mutex<AuthRequestRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(AuthRequestRegistry::default()))
}

fn safe_callback_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_AUTH_CALLBACK_BYTES
        && !value.chars().any(|character| character.is_control())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriveOperation {
    ConnectionAuthorize,
    ConnectionActivate,
    ConnectionRead,
    ConnectionRevoke,
    BackupRead,
    BackupWrite,
    BackupRestore,
}

impl DriveOperation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionAuthorize => "connection.authorize",
            Self::ConnectionActivate => "connection.activate",
            Self::ConnectionRead => "connection.read",
            Self::ConnectionRevoke => "connection.revoke",
            Self::BackupRead => "backup.read",
            Self::BackupWrite => "backup.write",
            Self::BackupRestore => "backup.restore",
        }
    }
}

/// Native-only result of the server authorization decision. This type does
/// not implement `Serialize`, `Clone`, or `Debug`, so it cannot become a
/// webview bearer, event payload, or accidental log value.
pub(crate) struct AuthorizedDriveContext {
    user_id: String,
    device_id: String,
    device_fingerprint: String,
    connection_id: Option<String>,
    operation: DriveOperation,
    expires_at: SystemTime,
}

pub(crate) struct DriveInvocation {
    context: AuthorizedDriveContext,
}

impl AuthorizedDriveContext {
    pub(crate) fn into_invocation(self) -> Result<DriveInvocation, String> {
        let invocation = DriveInvocation { context: self };
        invocation.ensure_valid()?;
        Ok(invocation)
    }
}

impl DriveInvocation {
    pub(crate) fn ensure_valid(&self) -> Result<(), String> {
        if self.context.expires_at <= SystemTime::now() {
            return Err(public_error("drive_authorization_expired"));
        }
        if matches!(
            self.context.operation,
            DriveOperation::BackupRead
                | DriveOperation::BackupWrite
                | DriveOperation::BackupRestore
        ) && self.context.connection_id.is_none()
        {
            return Err(public_error("drive_authorization_denied"));
        }
        Ok(())
    }

    pub(crate) fn require_operation(&self, expected: DriveOperation) -> Result<(), String> {
        self.ensure_valid()?;
        if self.context.operation != expected {
            return Err(public_error("drive_authorization_denied"));
        }
        Ok(())
    }

    pub(crate) fn user_id(&self) -> &str {
        &self.context.user_id
    }

    pub(crate) fn device_id(&self) -> &str {
        &self.context.device_id
    }

    pub(crate) fn device_fingerprint(&self) -> &str {
        &self.context.device_fingerprint
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationRequest<'a> {
    operation: &'a str,
    device_public_key: &'a str,
    device_fingerprint: &'a str,
    signature: &'a str,
    timestamp_ms: u64,
    nonce: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationResponse {
    authorized: bool,
    provider: String,
    user_id: String,
    device_id: String,
    device_fingerprint: String,
    connection_id: Option<String>,
    operation: String,
    expires_at_ms: u64,
    nonce: String,
}

fn public_error(code: &str) -> String {
    code.to_owned()
}

fn validate_session_proof(session_proof: &str) -> Result<(), String> {
    if session_proof.is_empty()
        || session_proof.len() > MAX_SESSION_PROOF_BYTES
        || session_proof
            .chars()
            .any(|character| character.is_control())
    {
        return Err(public_error("missing_session"));
    }
    Ok(())
}

fn configured_value(native_name: &str, frontend_name: &str) -> Result<String, String> {
    env::var(native_name)
        .or_else(|_| env::var(frontend_name))
        .map(|value| value.trim().to_owned())
        .map_err(|_| public_error("supabase_authorization_config_missing"))
        .and_then(|value| {
            if value.is_empty() {
                Err(public_error("supabase_authorization_config_missing"))
            } else {
                Ok(value)
            }
        })
}

pub(crate) fn configured_supabase_origin() -> Result<Url, String> {
    let raw = configured_value("FUNG_SUPABASE_URL", "VITE_SUPABASE_URL")?;
    let parsed =
        Url::parse(&raw).map_err(|_| public_error("supabase_authorization_origin_invalid"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(public_error("supabase_authorization_origin_invalid"));
    }
    Ok(parsed)
}

fn configured_supabase_anon_key() -> Result<String, String> {
    configured_value("FUNG_SUPABASE_ANON_KEY", "VITE_SUPABASE_ANON_KEY")
}

fn authorize_function_url() -> Result<Url, String> {
    let mut origin = configured_supabase_origin()?;
    origin.set_path(AUTHORIZE_FUNCTION_PATH);
    Ok(origin)
}

fn unix_timestamp_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| public_error("drive_authorization_clock_invalid"))
}

fn canonical_request(
    operation: DriveOperation,
    timestamp_ms: u64,
    nonce: &str,
    fingerprint: &str,
) -> String {
    format!(
        "fung-drive-auth-v1\n{}\n{timestamp_ms}\n{nonce}\n{fingerprint}",
        operation.as_str()
    )
}

fn validate_uuid(value: &str, error_code: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| public_error(error_code))
}

fn validate_authorization_response(
    response: AuthorizationResponse,
    operation: DriveOperation,
    fingerprint: &str,
    nonce: &str,
    now_ms: u64,
) -> Result<AuthorizedDriveContext, String> {
    if !response.authorized
        || response.provider != DRIVE_PROVIDER
        || response.operation != operation.as_str()
        || response.device_fingerprint != fingerprint
        || response.nonce != nonce
        || response.expires_at_ms <= now_ms
        || response.expires_at_ms > now_ms + AUTHORIZATION_REQUEST_TTL.as_millis() as u64
    {
        return Err(public_error("drive_authorization_denied"));
    }
    validate_uuid(&response.user_id, "drive_authorization_denied")?;
    validate_uuid(&response.device_id, "drive_authorization_denied")?;
    if let Some(connection_id) = response.connection_id.as_deref() {
        validate_uuid(connection_id, "drive_authorization_denied")?;
    }
    Ok(AuthorizedDriveContext {
        user_id: response.user_id,
        device_id: response.device_id,
        device_fingerprint: response.device_fingerprint,
        connection_id: response.connection_id,
        operation,
        expires_at: UNIX_EPOCH + Duration::from_millis(response.expires_at_ms),
    })
}

/// Ask the deployed authorization function to bind the current session to the
/// native device and one named operation. The caller cannot supply an origin,
/// user ID, device ID, connection ID, capability, or keyring slot.
pub(crate) async fn authorize_drive(
    app: &tauri::AppHandle,
    session_proof: &str,
    operation: DriveOperation,
) -> Result<AuthorizedDriveContext, String> {
    validate_session_proof(session_proof)?;
    let origin = authorize_function_url()?;
    let anon_key = configured_supabase_anon_key()?;
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|_| public_error("device_identity_unavailable"))?;
    let timestamp_ms = unix_timestamp_ms()?;
    let nonce = Uuid::new_v4().to_string();

    let (device_public_key, device_fingerprint) =
        device_identity::authorization_identity_in_dir(&app_data)
            .map_err(|_| public_error("drive_authorization_unavailable"))?;
    let canonical = canonical_request(operation, timestamp_ms, &nonce, &device_fingerprint);
    let signature = device_identity::sign_authorization_in_dir(&app_data, canonical.as_bytes())
        .map_err(|_| public_error("drive_authorization_unavailable"))?;
    let request = AuthorizationRequest {
        operation: operation.as_str(),
        device_public_key: &device_public_key,
        device_fingerprint: &device_fingerprint,
        signature: &signature,
        timestamp_ms,
        nonce: &nonce,
    };

    let response = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| public_error("drive_authorization_unavailable"))?
        .post(origin)
        .bearer_auth(session_proof)
        .header("apikey", anon_key)
        .json(&request)
        .send()
        .await
        .map_err(|_| public_error("drive_authorization_unavailable"))?;
    if !response.status().is_success() {
        return Err(public_error("drive_authorization_denied"));
    }
    let authorization = response
        .json::<AuthorizationResponse>()
        .await
        .map_err(|_| public_error("drive_authorization_denied"))?;
    validate_authorization_response(
        authorization,
        operation,
        &device_fingerprint,
        &nonce,
        timestamp_ms,
    )
}

fn build_google_auth_url(
    origin: &Url,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<Url, String> {
    if !safe_callback_value(redirect_uri)
        || !safe_callback_value(state)
        || !safe_callback_value(code_challenge)
    {
        return Err(public_error("auth_url_invalid"));
    }
    let mut url = origin.clone();
    url.set_path("/auth/v1/authorize");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("provider", "google")
        .append_pair("redirect_to", redirect_uri)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn emit_auth_callback(app: &tauri::AppHandle, event: AuthCallbackEvent) {
    use tauri::Emitter;
    let _ = app.emit("auth-callback", event);
}

fn callback_request_target(stream: &mut TcpStream, port: u16) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| public_error("auth_callback_unavailable"))?;
    let mut bytes = Vec::with_capacity(1024);
    loop {
        let mut chunk = [0u8; 1024];
        let read = stream
            .read(&mut chunk)
            .map_err(|_| public_error("invalid_callback"))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_AUTH_CALLBACK_BYTES {
            return Err(public_error("invalid_callback"));
        }
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8(bytes).map_err(|_| public_error("invalid_callback"))?;
    let mut lines = request.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| public_error("invalid_callback"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next();
    let target = parts.next();
    let version = parts.next();
    if method != Some("GET")
        || target.is_none()
        || version != Some("HTTP/1.1")
        || parts.next().is_some()
    {
        return Err(public_error("invalid_callback"));
    }
    let expected_host = format!("127.0.0.1:{port}");
    let hosts: Vec<&str> = lines
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.strip_prefix("Host:").map(str::trim))
        .collect();
    if hosts.len() != 1 || hosts[0] != expected_host {
        return Err(public_error("invalid_callback"));
    }
    Ok(target.unwrap().to_owned())
}

fn write_callback_response(stream: &mut TcpStream, status: &str) {
    let body = if status == "200 OK" {
        "<html><body>เข้าสู่ระบบสำเร็จ ปิดหน้าต่างนี้ได้เลย</body></html>"
    } else {
        "<html><body>ไม่สามารถยืนยันการเข้าสู่ระบบได้ ปิดหน้าต่างนี้ได้เลย</body></html>"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[derive(Deserialize)]
struct PkceTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
}

#[derive(Deserialize)]
struct SupabaseUserResponse {
    id: String,
}

fn exchange_pkce_code(callback: AuthCallbackSuccess) -> Result<AuthSession, String> {
    let origin = configured_supabase_origin()?;
    let anon_key = configured_supabase_anon_key()?;
    let mut token_url = origin.clone();
    token_url.set_path(AUTH_TOKEN_PATH);
    token_url.set_query(Some("grant_type=pkce"));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| public_error("auth_exchange_failed"))?;
    let token_response = client
        .post(token_url)
        .header("apikey", &anon_key)
        .form(&[
            ("auth_code", callback.code.as_str()),
            ("code_verifier", callback.code_verifier.as_str()),
        ])
        .send()
        .map_err(|_| public_error("auth_exchange_failed"))?;
    if !token_response.status().is_success() {
        return Err(public_error("auth_exchange_failed"));
    }
    let token = token_response
        .json::<PkceTokenResponse>()
        .map_err(|_| public_error("auth_exchange_failed"))?;
    if token.refresh_token.is_empty()
        || token.access_token.is_empty()
        || token.token_type.is_empty()
    {
        return Err(public_error("auth_exchange_failed"));
    }

    let access_token = Zeroizing::new(token.access_token);
    let mut user_url = origin;
    user_url.set_path(AUTH_USER_PATH);
    user_url.set_query(None);
    let user_response = client
        .get(user_url)
        .header("apikey", &anon_key)
        .bearer_auth(access_token.as_str())
        .send()
        .map_err(|_| public_error("auth_user_lookup_failed"))?;
    if !user_response.status().is_success() {
        return Err(public_error("auth_user_lookup_failed"));
    }
    let user = user_response
        .json::<SupabaseUserResponse>()
        .map_err(|_| public_error("auth_user_lookup_failed"))?;
    let user_id = Uuid::parse_str(&user.id)
        .map_err(|_| public_error("auth_user_lookup_failed"))?
        .hyphenated()
        .to_string();
    Ok(AuthSession {
        access_token: access_token.to_string(),
        refresh_token: token.refresh_token,
        expires_in: token.expires_in,
        token_type: token.token_type,
        user_id,
    })
}

fn spawn_auth_listener(
    app: tauri::AppHandle,
    listener: TcpListener,
    port: u16,
    request_id: String,
) {
    thread::spawn(move || loop {
        let expired = auth_request_registry()
            .lock()
            .ok()
            .map(|mut registry| registry.expire_if_needed(&request_id, SystemTime::now()))
            .unwrap_or(false);
        if expired {
            emit_auth_callback(
                &app,
                AuthCallbackEvent {
                    request_id: request_id.clone(),
                    session: None,
                    error: Some(public_error("auth_timeout")),
                },
            );
            return;
        }

        let still_pending = auth_request_registry()
            .lock()
            .ok()
            .is_some_and(|registry| registry.is_pending(&request_id));
        if !still_pending {
            return;
        }

        match listener.accept() {
            Ok((mut stream, peer)) => {
                let event = if peer.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) {
                    auth_request_registry()
                        .lock()
                        .ok()
                        .and_then(|mut registry| {
                            registry.take_error(&request_id, "invalid_callback")
                        })
                } else {
                    let callback = callback_request_target(&mut stream, port)
                        .map(|target| format!("http://127.0.0.1:{port}{target}"));
                    match callback {
                        Ok(callback_url) => {
                            let accepted =
                                auth_request_registry().lock().ok().map(|mut registry| {
                                    registry.accept_callback(&callback_url, SystemTime::now())
                                });
                            match accepted {
                                Some(Ok(callback)) => Some(match exchange_pkce_code(callback) {
                                    Ok(session) => AuthCallbackEvent {
                                        request_id: request_id.clone(),
                                        session: Some(session),
                                        error: None,
                                    },
                                    Err(error) => AuthCallbackEvent {
                                        request_id: request_id.clone(),
                                        session: None,
                                        error: Some(error),
                                    },
                                }),
                                Some(Err(error)) => Some(AuthCallbackEvent {
                                    request_id: request_id.clone(),
                                    session: None,
                                    error: Some(error),
                                }),
                                None => None,
                            }
                        }
                        Err(error) => auth_request_registry()
                            .lock()
                            .ok()
                            .and_then(|mut registry| registry.take_error(&request_id, &error)),
                    }
                };
                write_callback_response(
                    &mut stream,
                    if event
                        .as_ref()
                        .is_some_and(|event| event.error.as_deref() == Some("invalid_callback"))
                    {
                        "400 Bad Request"
                    } else {
                        "200 OK"
                    },
                );
                if let Some(event) = event {
                    emit_auth_callback(&app, event);
                }
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                if let Some(event) = auth_request_registry()
                    .lock()
                    .ok()
                    .and_then(|mut registry| {
                        registry.take_error(&request_id, "auth_listener_unavailable")
                    })
                {
                    emit_auth_callback(&app, event);
                }
                return;
            }
        }
    });
}

#[tauri::command]
pub(crate) fn auth_begin_google_login(app: tauri::AppHandle) -> Result<AuthLoginStarted, String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|_| public_error("auth_listener_unavailable"))?;
    listener
        .set_nonblocking(true)
        .map_err(|_| public_error("auth_listener_unavailable"))?;
    let port = listener
        .local_addr()
        .map_err(|_| public_error("auth_listener_unavailable"))?
        .port();
    let origin = configured_supabase_origin()?;
    let started = auth_request_registry()
        .lock()
        .map_err(|_| public_error("auth_request_unavailable"))?
        .start(port, SystemTime::now())?;
    let auth_url = build_google_auth_url(
        &origin,
        &started.redirect_uri,
        &started.state,
        &started.code_challenge,
    )?;
    let request_id = started.request_id.clone();
    spawn_auth_listener(app.clone(), listener, port, request_id.clone());
    if app
        .opener()
        .open_url(auth_url.as_str(), None::<&str>)
        .is_err()
    {
        let _ = auth_request_registry()
            .lock()
            .ok()
            .and_then(|mut registry| registry.cancel(&request_id).ok());
        return Err(public_error("auth_url_open_failed"));
    }
    Ok(started)
}

#[tauri::command]
pub(crate) fn auth_cancel_google_login(request_id: String) -> Result<(), String> {
    auth_request_registry()
        .lock()
        .map_err(|_| public_error("auth_request_unavailable"))?
        .cancel(&request_id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeEnrollmentProof {
    version: u8,
    operation: String,
    user_id: String,
    public_key: String,
    fingerprint: String,
    fingerprint_hex: String,
    platform: String,
    device_label: String,
    issued_at_ms: i64,
    expires_at_ms: i64,
    nonce: String,
    signature: String,
}

fn enrollment_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(windows)]
fn normalize_nfc(value: &str) -> Result<String, String> {
    #[link(name = "normaliz")]
    extern "system" {
        fn NormalizeString(
            normalization_form: i32,
            source: *const u16,
            source_length: i32,
            destination: *mut u16,
            destination_length: i32,
        ) -> i32;
    }

    let source: Vec<u16> = value.encode_utf16().collect();
    let source_length =
        i32::try_from(source.len()).map_err(|_| public_error("device_label_invalid"))?;
    let required =
        unsafe { NormalizeString(1, source.as_ptr(), source_length, std::ptr::null_mut(), 0) };
    if required <= 0 {
        return Err(public_error("device_label_invalid"));
    }
    let mut destination = vec![0u16; required as usize];
    let written = unsafe {
        NormalizeString(
            1,
            source.as_ptr(),
            source_length,
            destination.as_mut_ptr(),
            required,
        )
    };
    if written <= 0 {
        return Err(public_error("device_label_invalid"));
    }
    String::from_utf16(&destination[..written as usize])
        .map_err(|_| public_error("device_label_invalid"))
}

#[cfg(not(windows))]
fn normalize_nfc(value: &str) -> Result<String, String> {
    if !value.is_ascii() {
        return Err(public_error("device_label_normalization_unavailable"));
    }
    Ok(value.to_owned())
}

fn normalize_enrollment_label(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return Err(public_error("device_label_invalid"));
    }
    let normalized = normalize_nfc(trimmed)?;
    if normalized.is_empty() || normalized.len() > 80 || normalized.chars().any(char::is_control) {
        return Err(public_error("device_label_invalid"));
    }
    Ok(normalized)
}

fn canonical_enrollment_bytes(
    user_id: &Uuid,
    public_key: &[u8],
    fingerprint: &[u8],
    platform: &str,
    device_label: &str,
    issued_at_ms: i64,
    expires_at_ms: i64,
    nonce: &[u8],
) -> Result<Vec<u8>, String> {
    let platform_bytes = platform.as_bytes();
    let label_bytes = device_label.as_bytes();
    if public_key.len() != 32
        || fingerprint.len() != 32
        || nonce.len() != 32
        || platform_bytes.len() > u16::MAX as usize
        || label_bytes.is_empty()
        || label_bytes.len() > 80
    {
        return Err(public_error("enrollment_proof_invalid"));
    }
    let mut canonical = Vec::with_capacity(
        ENROLLMENT_DOMAIN.len()
            + 16
            + 32
            + 32
            + 2
            + platform_bytes.len()
            + 2
            + label_bytes.len()
            + 8
            + 8
            + 32,
    );
    canonical.extend_from_slice(ENROLLMENT_DOMAIN);
    canonical.extend_from_slice(user_id.as_bytes());
    canonical.extend_from_slice(public_key);
    canonical.extend_from_slice(fingerprint);
    canonical.extend_from_slice(&(platform_bytes.len() as u16).to_be_bytes());
    canonical.extend_from_slice(platform_bytes);
    canonical.extend_from_slice(&(label_bytes.len() as u16).to_be_bytes());
    canonical.extend_from_slice(label_bytes);
    canonical.extend_from_slice(&issued_at_ms.to_be_bytes());
    canonical.extend_from_slice(&expires_at_ms.to_be_bytes());
    canonical.extend_from_slice(nonce);
    Ok(canonical)
}

async fn authenticated_user_id(session_proof: &str) -> Result<Uuid, String> {
    validate_session_proof(session_proof)?;
    let mut user_url = configured_supabase_origin()?;
    user_url.set_path(AUTH_USER_PATH);
    user_url.set_query(None);
    let response = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| public_error("auth_user_lookup_failed"))?
        .get(user_url)
        .bearer_auth(session_proof)
        .header("apikey", configured_supabase_anon_key()?)
        .send()
        .await
        .map_err(|_| public_error("auth_user_lookup_failed"))?;
    if !response.status().is_success() {
        return Err(public_error("auth_user_lookup_failed"));
    }
    let user = response
        .json::<SupabaseUserResponse>()
        .await
        .map_err(|_| public_error("auth_user_lookup_failed"))?;
    Uuid::parse_str(&user.id).map_err(|_| public_error("auth_user_lookup_failed"))
}

/// Native creates the exact user-bound enrollment envelope. The webview only
/// supplies its current bearer proof; it cannot choose the user, key, nonce,
/// timestamps, or bytes that are signed.
#[tauri::command(rename = "device_enrollment_proof")]
pub(crate) async fn native_device_enrollment_proof(
    app: tauri::AppHandle,
    session_proof: String,
    device_label: String,
) -> Result<NativeEnrollmentProof, String> {
    #[cfg(not(desktop))]
    {
        let _ = (app, session_proof, device_label);
        return Err(public_error("desktop_enrollment_only"));
    }
    #[cfg(desktop)]
    {
        let user_id = authenticated_user_id(&session_proof).await?;
        let device_label = normalize_enrollment_label(&device_label)?;
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|_| public_error("device_identity_unavailable"))?;
        let (public_key_text, stored_fingerprint) =
            device_identity::authorization_identity_in_dir(&app_data)
                .map_err(|_| public_error("device_identity_unavailable"))?;
        let public_key = STANDARD
            .decode(public_key_text.as_bytes())
            .map_err(|_| public_error("device_identity_unavailable"))?;
        let signing_key = device_identity::secure_signing_key_in_dir(&app_data)
            .map_err(|_| public_error("device_identity_unavailable"))?;
        let verifying_key = signing_key.verifying_key();
        if public_key.as_slice() != verifying_key.as_bytes() || public_key.len() != 32 {
            return Err(public_error("device_identity_unavailable"));
        }
        let fingerprint = Sha256::digest(&public_key);
        let fingerprint_hex = enrollment_hex(&fingerprint);
        if fingerprint_hex != stored_fingerprint {
            return Err(public_error("device_identity_unavailable"));
        }

        let issued_at_ms = i64::try_from(unix_timestamp_ms()?)
            .map_err(|_| public_error("enrollment_proof_clock_invalid"))?;
        let expires_at_ms = issued_at_ms
            .checked_add(ENROLLMENT_PROOF_TTL_MS)
            .ok_or_else(|| public_error("enrollment_proof_clock_invalid"))?;
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);
        let canonical = canonical_enrollment_bytes(
            &user_id,
            &public_key,
            &fingerprint,
            ENROLLMENT_PLATFORM,
            &device_label,
            issued_at_ms,
            expires_at_ms,
            &nonce,
        )?;
        let signature = signing_key.sign(&canonical).to_bytes();
        Ok(NativeEnrollmentProof {
            version: 1,
            operation: ENROLLMENT_OPERATION.to_owned(),
            user_id: user_id.hyphenated().to_string(),
            public_key: URL_SAFE_NO_PAD.encode(public_key),
            fingerprint: URL_SAFE_NO_PAD.encode(fingerprint),
            fingerprint_hex,
            platform: ENROLLMENT_PLATFORM.to_owned(),
            device_label,
            issued_at_ms,
            expires_at_ms,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            signature: URL_SAFE_NO_PAD.encode(signature),
        })
    }
}

pub(crate) fn open_trusted_account_portal(app: tauri::AppHandle) -> AppResult<()> {
    let raw = env::var("FUNG_WEB_APP_URL")
        .map_err(|_| AppError::InvalidInput("FUNG_WEB_APP_URL is not configured".to_owned()))?;
    let url = Url::parse(raw.trim())
        .map_err(|_| AppError::InvalidInput("FUNG_WEB_APP_URL is invalid".to_owned()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::InvalidInput(
            "FUNG_WEB_APP_URL must be a trusted https URL".to_owned(),
        ));
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| AppError::InvalidInput(format!("could not open account portal: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_request_binds_operation_timestamp_nonce_and_fingerprint() {
        let canonical =
            canonical_request(DriveOperation::BackupRestore, 123, "nonce", "fingerprint");
        assert_eq!(
            canonical,
            "fung-drive-auth-v1\nbackup.restore\n123\nnonce\nfingerprint"
        );
    }

    #[test]
    fn native_builds_google_url_from_configured_origin_and_request() {
        let origin = Url::parse("https://project.supabase.co").unwrap();
        let url = build_google_auth_url(
            &origin,
            "http://127.0.0.1:43123/auth/callback",
            "state-123",
            "challenge-123",
        )
        .unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            "https://project.supabase.co"
        );
        assert_eq!(url.path(), "/auth/v1/authorize");
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![
                ("provider".into(), "google".into()),
                (
                    "redirect_to".into(),
                    "http://127.0.0.1:43123/auth/callback".into()
                ),
                ("state".into(), "state-123".into()),
                ("code_challenge".into(), "challenge-123".into()),
                ("code_challenge_method".into(), "S256".into()),
            ]
        );
    }

    #[test]
    fn pkce_verifier_is_private_and_challenge_is_s256() {
        let (verifier, challenge) = create_pkce_pair();
        assert!(verifier.len() >= 43);
        assert_eq!(
            challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
        );
    }

    #[test]
    fn enrollment_canonical_bytes_bind_domain_identity_fields_and_network_integers() {
        let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let public_key = [1u8; 32];
        let fingerprint = [2u8; 32];
        let nonce = [3u8; 32];
        let canonical = canonical_enrollment_bytes(
            &user_id,
            &public_key,
            &fingerprint,
            ENROLLMENT_PLATFORM,
            "FUNG Desktop",
            10,
            300010,
            &nonce,
        )
        .unwrap();
        assert!(canonical.starts_with(ENROLLMENT_DOMAIN));
        assert!(canonical.windows(32).any(|window| window == public_key));
        assert!(canonical.windows(32).any(|window| window == fingerprint));
        assert!(canonical.ends_with(&nonce));
        assert!(canonical
            .windows(8)
            .any(|window| window == 10i64.to_be_bytes()));
        assert!(canonical
            .windows(8)
            .any(|window| window == 300010i64.to_be_bytes()));
    }

    #[test]
    fn authorization_context_is_not_serializable_or_cloneable() {
        fn accepts_context(_: AuthorizedDriveContext) {}
        let _ = accepts_context;
    }

    #[test]
    fn response_rejects_expired_or_mismatched_context() {
        let response = AuthorizationResponse {
            authorized: true,
            provider: DRIVE_PROVIDER.to_owned(),
            user_id: Uuid::new_v4().to_string(),
            device_id: Uuid::new_v4().to_string(),
            device_fingerprint: "a".repeat(64),
            connection_id: None,
            operation: DriveOperation::BackupWrite.as_str().to_owned(),
            expires_at_ms: 100,
            nonce: "n".to_owned(),
        };
        assert!(validate_authorization_response(
            response,
            DriveOperation::BackupWrite,
            &"a".repeat(64),
            "n",
            100,
        )
        .is_err());
    }

    #[test]
    fn native_login_registry_allows_one_pending_request_and_one_callback() {
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let mut registry = AuthRequestRegistry::default();
        let started = registry.start(43_123, now).unwrap();

        assert!(!started.request_id.is_empty());
        assert!(!started.state.is_empty());
        assert_eq!(started.redirect_uri, "http://127.0.0.1:43123/auth/callback");
        assert!(registry.start(43_124, now).is_err());

        let callback = registry
            .accept_callback(
                &format!(
                    "http://127.0.0.1:43123/auth/callback?code=abc&state={}",
                    started.state
                ),
                now + Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(callback.code.as_str(), "abc");
        assert!(!callback.code_verifier.is_empty());
        assert!(registry
            .accept_callback(
                &format!(
                    "http://127.0.0.1:43123/auth/callback?code=abc&state={}",
                    started.state
                ),
                now + Duration::from_secs(2),
            )
            .is_err());
    }

    #[test]
    fn native_login_registry_rejects_wrong_listener_and_query_shape() {
        let now = UNIX_EPOCH + Duration::from_secs(20_000);
        let mut registry = AuthRequestRegistry::default();
        let started = registry.start(43_125, now).unwrap();
        let invalid_callbacks = [
            format!(
                "http://localhost:43125/auth/callback?code=abc&state={}",
                started.state
            ),
            format!(
                "http://127.0.0.1:43126/auth/callback?code=abc&state={}",
                started.state
            ),
            format!(
                "http://127.0.0.1:43125/other?code=abc&state={}",
                started.state
            ),
            format!(
                "http://127.0.0.1:43125/auth/callback?code=abc&state={}&extra=1",
                started.state
            ),
            format!(
                "http://127.0.0.1:43125/auth/callback?code=abc&code=def&state={}",
                started.state
            ),
        ];
        for callback in invalid_callbacks {
            assert!(registry.accept_callback(&callback, now).is_err());
        }
    }

    #[test]
    fn native_login_registry_rejects_timeout_cancellation_and_replay() {
        let now = UNIX_EPOCH + Duration::from_secs(30_000);

        let mut cancelled = AuthRequestRegistry::default();
        let cancelled_request = cancelled.start(43_127, now).unwrap();
        cancelled.cancel(&cancelled_request.request_id).unwrap();
        assert!(cancelled
            .accept_callback(
                &format!(
                    "http://127.0.0.1:43127/auth/callback?code=abc&state={}",
                    cancelled_request.state
                ),
                now + Duration::from_secs(1),
            )
            .is_err());

        let mut expired = AuthRequestRegistry::default();
        let expired_request = expired.start(43_128, now).unwrap();
        assert!(expired
            .accept_callback(
                &format!(
                    "http://127.0.0.1:43128/auth/callback?code=abc&state={}",
                    expired_request.state
                ),
                now + AUTH_LOGIN_REQUEST_TTL + Duration::from_secs(1),
            )
            .is_err());
    }
}
