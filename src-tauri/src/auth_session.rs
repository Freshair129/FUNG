//! Desktop native session broker.
//!
//! Refresh credentials are stored only in the OS keyring. Access tokens,
//! callback values, authorization codes, and verifiers never implement a
//! public DTO and are held in `Zeroizing` native memory for their lifetime.

use crate::{device_identity, native_auth, AppState};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "FUNG";
const ACTIVE_SLOT: &str = "desktop-session-active";
const STAGED_SLOT: &str = "desktop-session-staged";
const CALLBACK_PATH: &str = "/auth/callback";
const LOGIN_TTL: Duration = Duration::from_secs(120);
const ACCESS_SKEW_MS: u64 = 30_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionState { SignedOut, LoginPending, Authenticated, Refreshing, RefreshFailed, LogoutPending, CleanupFailed, Shutdown }
impl SessionState { fn as_str(self) -> &'static str { match self { Self::SignedOut => "signed_out", Self::LoginPending => "login_pending", Self::Authenticated => "authenticated", Self::Refreshing => "refreshing", Self::RefreshFailed => "refresh_failed", Self::LogoutPending => "logout_pending", Self::CleanupFailed => "credential_cleanup_failed", Self::Shutdown => "shutdown" } } }

struct PendingLogin {
    request_id: String,
    generation: u64,
    port: u16,
    state: Zeroizing<String>,
    verifier: Zeroizing<String>,
    expires_at: SystemTime,
    callback: Arc<Mutex<Option<Zeroizing<String>>>>,
    cancelled: Arc<AtomicBool>,
}

struct SessionMemory {
    generation: u64,
    state: SessionState,
    startup_checked: bool,
    user_id: Option<String>,
    email: Option<String>,
    access_token: Option<Zeroizing<String>>,
    access_expires_at_ms: Option<u64>,
    pending_login: Option<PendingLogin>,
    refresh_flight: Option<Arc<(Mutex<bool>, Condvar)>>,
}

impl Default for SessionMemory {
    fn default() -> Self { Self { generation: 1, state: SessionState::SignedOut, startup_checked: false, user_id: None, email: None, access_token: None, access_expires_at_ms: None, pending_login: None, refresh_flight: None } }
}

fn memory() -> &'static Mutex<SessionMemory> { static MEMORY: OnceLock<Mutex<SessionMemory>> = OnceLock::new(); MEMORY.get_or_init(|| Mutex::new(SessionMemory::default())) }
fn public_error(code: &str) -> String { code.to_owned() }
fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
fn keyring_entry(slot: &str) -> Result<Entry, String> { Entry::new(KEYRING_SERVICE, slot).map_err(|_| public_error("keyring_unavailable")) }

fn read_secret(slot: &str) -> Result<Option<Zeroizing<String>>, String> {
    match keyring_entry(slot)?.get_password() {
        Ok(value) => { let value = Zeroizing::new(value); if value.trim().is_empty() { Ok(None) } else { Ok(Some(value)) } }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(public_error("keyring_unavailable")),
    }
}

fn delete_secret(slot: &str) -> Result<(), String> {
    match keyring_entry(slot)?.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), Err(_) => Err(public_error("auth_logout_incomplete")) }
}

fn verify_absent(slot: &str) -> Result<(), String> {
    if read_secret(slot)?.is_some() { Err(public_error("auth_logout_incomplete")) } else { Ok(()) }
}

fn commit_refresh_token(token: &Zeroizing<String>) -> Result<(), String> {
    let staged = keyring_entry(STAGED_SLOT)?;
    let active = keyring_entry(ACTIVE_SLOT)?;
    staged.set_password(token.as_str()).map_err(|_| public_error("keyring_unavailable"))?;
    let staged_read = read_secret(STAGED_SLOT)?.ok_or_else(|| public_error("keyring_unavailable"))?;
    if staged_read.as_str() != token.as_str() { return Err(public_error("keyring_unavailable")); }
    active.set_password(staged_read.as_str()).map_err(|_| public_error("keyring_unavailable"))?;
    let active_read = read_secret(ACTIVE_SLOT)?.ok_or_else(|| public_error("keyring_unavailable"))?;
    if active_read.as_str() != token.as_str() { return Err(public_error("keyring_unavailable")); }
    delete_secret(STAGED_SLOT)?;
    verify_absent(STAGED_SLOT)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStatus { pub(crate) state: &'static str, pub(crate) user_id: Option<String>, pub(crate) email: Option<String>, pub(crate) access_expires_at_ms: Option<u64> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginStarted { pub(crate) request_id: String, pub(crate) expires_at_ms: u64 }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cancelled { pub(crate) request_id: String, pub(crate) status: &'static str }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrollmentResult { pub(crate) request_id: String, pub(crate) status: &'static str, pub(crate) authority_state: &'static str }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrollmentStatus { pub(crate) status: String, pub(crate) device_id: Option<String> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingResult { pub(crate) pairing_id: String, pub(crate) display_code: String, pub(crate) expires_at_ms: u64, pub(crate) status: &'static str }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingPeer { pub(crate) id: String, pub(crate) label: String, pub(crate) platform: String, pub(crate) fingerprint: String }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingPoll { pub(crate) status: String, pub(crate) peer: Option<PairingPeer> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReconcileStatus { pub(crate) reconciled: bool, pub(crate) device_id: Option<String> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RevokeResult { pub(crate) device_id: String, pub(crate) status: &'static str }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditRow { pub(crate) event_id: String, pub(crate) event_type: String, pub(crate) created_at: String, pub(crate) device_id: Option<String> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceRow { pub(crate) id: String, pub(crate) label: String, pub(crate) platform: String, pub(crate) authority_state: String, pub(crate) paired_at: Option<String>, pub(crate) revoked_at: Option<String>, pub(crate) endpoint_state: Option<String> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EndpointStatus { pub(crate) status: &'static str, pub(crate) updated_at: Option<String> }

fn deserialize_zeroizing<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where D: Deserializer<'de> { String::deserialize(deserializer).map(Zeroizing::new) }
fn deserialize_optional_zeroizing<'de, D>(deserializer: D) -> Result<Option<Zeroizing<String>>, D::Error>
where D: Deserializer<'de> { Option::<String>::deserialize(deserializer).map(|value| value.map(Zeroizing::new)) }

#[derive(Deserialize)]
struct AuthTokenResponse {
    #[serde(rename = "access_token", deserialize_with = "deserialize_zeroizing")]
    access: Zeroizing<String>,
    #[serde(rename = "refresh_token", default, deserialize_with = "deserialize_optional_zeroizing")]
    refresh: Option<Zeroizing<String>>,
    expires_in: Option<u64>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct AuthUser { id: String, email: Option<String> }
struct SessionMaterial { access: Zeroizing<String>, refresh: Zeroizing<String>, expires_at_ms: u64, user_id: String, email: Option<String> }

fn callback_pair() -> (Zeroizing<String>, String) { let mut bytes = [0u8; 32]; OsRng.fill_bytes(&mut bytes); let verifier = URL_SAFE_NO_PAD.encode(bytes); let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())); (Zeroizing::new(verifier), challenge) }
fn safe_callback_value(value: &str) -> bool { !value.is_empty() && value.len() <= 8192 && !value.chars().any(char::is_control) }

fn callback_from_request(request: &[u8], port: u16) -> Option<Zeroizing<String>> {
    let first = request.split(|byte| *byte == b'\n').next()?.strip_suffix(b"\r").unwrap_or(request);
    let mut fields = first.split(|byte| *byte == b' ');
    if fields.next()? != b"GET" { return None; }
    let target = fields.next()?;
    if !target.starts_with(CALLBACK_PATH.as_bytes()) { return None; }
    let target = std::str::from_utf8(target).ok()?;
    Some(Zeroizing::new(format!("http://127.0.0.1:{port}{target}")))
}

fn parse_callback(raw: &str, pending: &PendingLogin) -> Result<Zeroizing<String>, String> {
    let url = Url::parse(raw).map_err(|_| public_error("auth_callback_invalid"))?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") || url.port() != Some(pending.port) || url.path() != CALLBACK_PATH || url.fragment().is_some() { return Err(public_error("auth_callback_invalid")); }
    let mut names = HashSet::new();
    let mut count = 0usize;
    let mut state = None;
    let mut code = None;
    let mut error = false;
    let mut error_description = false;
    for (name, value) in url.query_pairs() {
        count += 1;
        if !matches!(name.as_ref(), "code" | "error" | "error_description" | "state") || !names.insert(name.as_ref().to_owned()) || !safe_callback_value(value.as_ref()) { return Err(public_error("auth_callback_invalid")); }
        match name.as_ref() {
            "state" => state = Some(value.into_owned()),
            "code" => code = Some(Zeroizing::new(value.into_owned())),
            "error" => error = true,
            "error_description" => error_description = true,
            _ => {}
        }
    }
    if count == 0 || state.as_deref() != Some(pending.state.as_str()) { return Err(public_error("auth_state_mismatch")); }
    if code.is_some() == error || (error_description && !error) { return Err(public_error("auth_callback_invalid")); }
    if let Some(code) = code { if count != 2 { return Err(public_error("auth_callback_invalid")); } return Ok(code); }
    Err(public_error("authorization_denied"))
}

fn spawn_listener(listener: TcpListener, port: u16, callback: Arc<Mutex<Option<Zeroizing<String>>>>, cancelled: Arc<AtomicBool>) {
    thread::spawn(move || {
        let deadline = SystemTime::now() + LOGIN_TTL;
        let _ = listener.set_nonblocking(true);
        loop {
            if cancelled.load(Ordering::Acquire) || SystemTime::now() >= deadline { return; }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut bytes = Zeroizing::new([0u8; 8192]); let count = stream.read(&mut bytes[..]).unwrap_or(0); let callback_url = callback_from_request(&bytes[..count], port);
                    let body = if callback_url.is_some() { "Authentication received. You may close this window." } else { "Authentication rejected." };
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body); let _ = stream.write_all(response.as_bytes());
                    if let Some(value) = callback_url { if let Ok(mut slot) = callback.lock() { *slot = Some(value); } }
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(50)),
                Err(_) => return,
            }
        }
    });
}

async fn auth_user(access: &Zeroizing<String>) -> Result<AuthUser, String> {
    let mut url = native_auth::configured_supabase_origin()?; url.set_path("/auth/v1/user");
    let response = Client::builder().timeout(Duration::from_secs(15)).build().map_err(|_| public_error("auth_refresh_unavailable"))?.get(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).send().await.map_err(|_| public_error("auth_refresh_unavailable"))?;
    if !response.status().is_success() { return Err(public_error("auth_refresh_unavailable")); }
    response.json::<AuthUser>().await.map_err(|_| public_error("auth_refresh_unavailable"))
}

async fn exchange_code(code: Zeroizing<String>, verifier: Zeroizing<String>) -> Result<SessionMaterial, String> {
    let mut url = native_auth::configured_supabase_origin()?; url.set_path("/auth/v1/token"); url.set_query(Some("grant_type=pkce"));
    let response = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|_| public_error("auth_exchange_failed"))?.post(url).header("apikey", native_auth::configured_supabase_anon_key()?).form(&[("code", code.as_str()), ("code_verifier", verifier.as_str())]).send().await.map_err(|_| public_error("auth_exchange_failed"))?;
    if !response.status().is_success() { return Err(public_error("auth_exchange_failed")); }
    let token = response.json::<AuthTokenResponse>().await.map_err(|_| public_error("auth_exchange_failed"))?;
    if token.error.is_some() || token.refresh.is_none() { return Err(public_error("auth_exchange_failed")); }
    let access = token.access; let refresh = token.refresh.unwrap_or_else(|| Zeroizing::new(String::new())); let user = auth_user(&access).await?;
    Ok(SessionMaterial { access, refresh, expires_at_ms: now_ms() + token.expires_in.unwrap_or(3600) * 1000, user_id: user.id, email: user.email })
}

async fn refresh_from_keyring(generation: u64) -> Result<SessionMaterial, String> {
    let old = read_secret(ACTIVE_SLOT)?.ok_or_else(|| public_error("auth_required"))?;
    let mut url = native_auth::configured_supabase_origin()?; url.set_path("/auth/v1/token"); url.set_query(Some("grant_type=refresh_token"));
    let response = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|_| public_error("auth_refresh_unavailable"))?.post(url).header("apikey", native_auth::configured_supabase_anon_key()?).form(&[("refresh_token", old.as_str())]).send().await.map_err(|_| public_error("auth_refresh_unavailable"))?;
    if response.status().as_u16() == 400 { delete_secret(ACTIVE_SLOT)?; verify_absent(ACTIVE_SLOT)?; return Err(public_error("auth_refresh_invalid")); }
    if !response.status().is_success() { return Err(public_error("auth_refresh_unavailable")); }
    let token = response.json::<AuthTokenResponse>().await.map_err(|_| public_error("auth_refresh_unavailable"))?;
    if token.error.is_some() { return Err(public_error("auth_refresh_invalid")); }
    let access = token.access; let refresh = token.refresh.unwrap_or_else(|| Zeroizing::new(old.to_string())); let user = auth_user(&access).await?;
    let current_generation = memory().lock().map_err(|_| public_error("auth_unavailable"))?.generation;
    if current_generation != generation { return Err(public_error("auth_transition_in_progress")); }
    commit_refresh_token(&refresh)?;
    Ok(SessionMaterial { access, refresh, expires_at_ms: now_ms() + token.expires_in.unwrap_or(3600) * 1000, user_id: user.id, email: user.email })
}

fn publish_material(material: SessionMaterial, generation: u64) -> Result<Zeroizing<String>, String> {
    let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?;
    if state.generation != generation || matches!(state.state, SessionState::LogoutPending | SessionState::Shutdown) { return Err(public_error("auth_transition_in_progress")); }
    state.access_token = Some(material.access.clone()); state.access_expires_at_ms = Some(material.expires_at_ms); state.user_id = Some(material.user_id); state.email = material.email; state.state = SessionState::Authenticated; Ok(material.access)
}

async fn ensure_startup() -> Result<(), String> {
    let should_check = { let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; if state.startup_checked { false } else { state.startup_checked = true; true } };
    if should_check && read_secret(ACTIVE_SLOT)?.is_none() { return Err(public_error("auth_required")); }
    Ok(())
}

pub(crate) async fn ensure_access_token() -> Result<Zeroizing<String>, String> {
    ensure_startup().await?;
    loop {
        let (generation, wait) = {
            let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?;
            if matches!(state.state, SessionState::Shutdown | SessionState::LogoutPending | SessionState::CleanupFailed) { return Err(public_error("auth_transition_in_progress")); }
            if state.state == SessionState::Authenticated && state.access_expires_at_ms.is_some_and(|expires| expires > now_ms() + ACCESS_SKEW_MS) { return state.access_token.clone().ok_or_else(|| public_error("auth_refresh_unavailable")); }
            if let Some(flight) = &state.refresh_flight { (state.generation, Some(flight.clone())) } else { let flight = Arc::new((Mutex::new(false), Condvar::new())); state.refresh_flight = Some(flight); state.state = SessionState::Refreshing; (state.generation, None) }
        };
        if let Some(wait) = wait { let _ = tauri::async_runtime::spawn_blocking(move || { let (lock, signal) = &*wait; let mut done = lock.lock().map_err(|_| ())?; while !*done { done = signal.wait(done).map_err(|_| ())?; } Ok::<(), ()>(()) }).await; continue; }
        let result = refresh_from_keyring(generation).await.and_then(|material| publish_material(material, generation));
        let notify = { let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; let notify = state.refresh_flight.take(); if result.is_err() && state.generation == generation && !matches!(state.state, SessionState::Shutdown | SessionState::LogoutPending) { state.access_token = None; state.access_expires_at_ms = None; state.state = if matches!(result.as_ref().err().map(String::as_str), Some("auth_refresh_invalid" | "auth_required")) { SessionState::SignedOut } else { SessionState::RefreshFailed }; } notify };
        if let Some(notify) = notify { let (lock, signal) = &*notify; if let Ok(mut done) = lock.lock() { *done = true; signal.notify_all(); } }
        return result;
    }
}

pub(crate) fn native_access_token() -> Option<Zeroizing<String>> { memory().lock().ok().and_then(|state| state.access_token.clone()) }
pub(crate) fn native_user_id() -> Option<String> { memory().lock().ok().and_then(|state| state.user_id.clone()) }

async fn finish_login(app: AppHandle, request_id: String, generation: u64) {
    loop {
        let maybe = { let state = match memory().lock() { Ok(value) => value, Err(_) => return }; if state.generation != generation || state.pending_login.as_ref().is_none_or(|p| p.request_id != request_id) { return; } state.pending_login.as_ref().and_then(|p| p.callback.lock().ok().and_then(|mut slot| slot.take())) };
        if let Some(raw) = maybe { let pending = { match memory().lock() { Ok(mut state) => state.pending_login.take(), Err(_) => None } }; if let Some(pending) = pending { let result = match parse_callback(raw.as_str(), &pending) { Ok(code) => exchange_code(code, pending.verifier).await, Err(error) => Err(error) }; let outcome = match result { Ok(material) => commit_refresh_token(&material.refresh).and_then(|_| publish_material(material, generation).map(|_| ())), Err(error) => Err(error) }; if outcome.is_err() { if let Ok(mut state) = memory().lock() { if state.generation == generation { state.state = SessionState::SignedOut; state.access_token = None; state.pending_login = None; } } } } return; }
        let expired = memory().lock().ok().and_then(|state| state.pending_login.as_ref().map(|p| SystemTime::now() >= p.expires_at)).unwrap_or(true);
        if expired { if let Ok(mut state) = memory().lock() { state.pending_login = None; if state.generation == generation { state.state = SessionState::SignedOut; } } return; }
        let _ = tauri::async_runtime::spawn_blocking(|| thread::sleep(Duration::from_millis(50))).await;
    }
}

#[tauri::command]
pub(crate) async fn broker_session_login_begin(app: AppHandle) -> Result<LoginStarted, String> {
    let origin = native_auth::configured_supabase_origin()?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| public_error("auth_listener_unavailable"))?;
    let port = listener.local_addr().map_err(|_| public_error("auth_listener_unavailable"))?.port();
    let request_id = Uuid::new_v4().to_string(); let state_value = Zeroizing::new(Uuid::new_v4().to_string()); let (verifier, challenge) = callback_pair(); let callback = Arc::new(Mutex::new(None)); let cancelled = Arc::new(AtomicBool::new(false)); let generation;
    { let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; if state.pending_login.is_some() || state.state == SessionState::LoginPending { return Err(public_error("auth_request_in_progress")); } if matches!(state.state, SessionState::Shutdown | SessionState::LogoutPending | SessionState::CleanupFailed) { return Err(public_error("auth_transition_in_progress")); } state.startup_checked = true; generation = state.generation; state.state = SessionState::LoginPending; state.pending_login = Some(PendingLogin { request_id: request_id.clone(), generation, port, state: state_value.clone(), verifier, expires_at: SystemTime::now() + LOGIN_TTL, callback: callback.clone(), cancelled: cancelled.clone() }); }
    spawn_listener(listener, port, callback, cancelled);
    let mut url = origin; url.set_path("/auth/v1/authorize"); url.query_pairs_mut().append_pair("provider", "google").append_pair("redirect_to", &format!("http://127.0.0.1:{port}{CALLBACK_PATH}")).append_pair("state", state_value.as_str()).append_pair("code_challenge", &challenge).append_pair("code_challenge_method", "S256");
    if app.opener().open_url(url.as_str(), None::<&str>).is_err() { if let Ok(mut state) = memory().lock() { state.pending_login = None; state.state = SessionState::SignedOut; } return Err(public_error("auth_url_open_failed")); }
    tauri::async_runtime::spawn(finish_login(app, request_id.clone(), generation));
    Ok(LoginStarted { request_id, expires_at_ms: now_ms() + LOGIN_TTL.as_millis() as u64 })
}

#[tauri::command]
pub(crate) async fn broker_session_login_cancel(request_id: String) -> Result<Cancelled, String> {
    let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; let pending = state.pending_login.take().ok_or_else(|| public_error("auth_request_not_found"))?; if pending.request_id != request_id { state.pending_login = Some(pending); return Err(public_error("auth_request_not_found")); } pending.cancelled.store(true, Ordering::Release); state.generation = state.generation.wrapping_add(1); state.state = SessionState::SignedOut; Ok(Cancelled { request_id, status: "cancelled" })
}

#[tauri::command]
pub(crate) async fn broker_session_status() -> Result<SessionStatus, String> {
    let has_keyring = read_secret(ACTIVE_SLOT)?.is_some();
    let _ = ensure_startup().await;
    if has_keyring { let _ = ensure_access_token().await; }
    let state = memory().lock().map_err(|_| public_error("auth_unavailable"))?;
    Ok(SessionStatus { state: state.state.as_str(), user_id: state.user_id.clone(), email: state.email.clone(), access_expires_at_ms: state.access_expires_at_ms })
}

#[tauri::command]
pub(crate) async fn broker_session_logout() -> Result<SessionStatus, String> {
    let pending = { let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; if state.state == SessionState::Shutdown { return Ok(SessionStatus { state: "shutdown", user_id: None, email: None, access_expires_at_ms: None }); } state.generation = state.generation.wrapping_add(1); state.state = SessionState::LogoutPending; state.access_token = None; state.access_expires_at_ms = None; state.user_id = None; state.email = None; state.pending_login.take() };
    if let Some(pending) = pending { pending.cancelled.store(true, Ordering::Release); }
    if delete_secret(ACTIVE_SLOT).and_then(|_| verify_absent(ACTIVE_SLOT)).and_then(|_| delete_secret(STAGED_SLOT)).and_then(|_| verify_absent(STAGED_SLOT)).is_err() { if let Ok(mut state) = memory().lock() { state.state = SessionState::CleanupFailed; } return Err(public_error("auth_logout_incomplete")); }
    let mut state = memory().lock().map_err(|_| public_error("auth_unavailable"))?; state.state = SessionState::SignedOut; state.startup_checked = true; Ok(SessionStatus { state: "signed_out", user_id: None, email: None, access_expires_at_ms: None })
}

pub(crate) fn shutdown() { if let Ok(mut state) = memory().lock() { state.generation = state.generation.wrapping_add(1); state.state = SessionState::Shutdown; state.access_token = None; if let Some(pending) = state.pending_login.take() { pending.cancelled.store(true, Ordering::Release); } } let _ = delete_secret(ACTIVE_SLOT).and_then(|_| verify_absent(ACTIVE_SLOT)); let _ = delete_secret(STAGED_SLOT).and_then(|_| verify_absent(STAGED_SLOT)); }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EnrollmentInput { pub(crate) device_label: String }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentRequest<'a> { native_proof: &'a native_auth::NativeEnrollmentProof }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentResponse { request_id: String, status: String }

#[derive(Deserialize)]
struct NativeErrorBody { code: Option<String> }

fn allowed_native_error(code: &str) -> bool {
    matches!(code, "authorization_denied" | "device_not_found" | "enrollment_unavailable" | "invalid_native_proof" | "proof_replayed" | "auth_required")
}

async fn native_post<T: for<'de> Deserialize<'de>>(path: &str, body: impl Serialize) -> Result<T, String> {
    let access = ensure_access_token().await?; let mut url = native_auth::configured_supabase_origin()?; url.set_path(path); let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("authorization_unavailable"))?.post(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).json(&body).send().await.map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() {
        let code = response.json::<NativeErrorBody>().await.ok().and_then(|body| body.code).filter(|code| allowed_native_error(code));
        return Err(public_error(code.as_deref().unwrap_or("authorization_denied")));
    }
    response.json::<T>().await.map_err(|_| public_error("authorization_unavailable"))
}

#[tauri::command]
pub(crate) async fn broker_enrollment_request(app: AppHandle, input: EnrollmentInput) -> Result<EnrollmentResult, String> {
    let proof = native_auth::native_device_enrollment_proof(&app, &input.device_label).await?; let response: EnrollmentResponse = native_post("/functions/v1/device-enrollment", EnrollmentRequest { native_proof: &proof }).await?; if response.status != "pending" { return Err(public_error("enrollment_unavailable")); } Ok(EnrollmentResult { request_id: response.request_id, status: "pending", authority_state: "pending" })
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct DeviceWire { id: String, device_label: Option<String>, platform: String, authority_state: String, registered_at: Option<String>, revoked_at: Option<String>, lan_endpoint: Option<String>, public_key_fingerprint: Option<String> }
async fn device_rows() -> Result<Vec<DeviceWire>, String> { let access = ensure_access_token().await?; let mut url = native_auth::configured_supabase_origin()?; url.set_path("/rest/v1/devices"); url.set_query(Some("select=id,device_label,platform,authority_state,registered_at,revoked_at,lan_endpoint,public_key_fingerprint&order=registered_at.desc")); let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("authorization_unavailable"))?.get(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).send().await.map_err(|_| public_error("authorization_unavailable"))?; if !response.status().is_success() { return Err(public_error("authorization_unavailable")); } response.json::<Vec<DeviceWire>>().await.map_err(|_| public_error("authorization_unavailable")) }

#[tauri::command]
pub(crate) async fn broker_enrollment_status(app: AppHandle) -> Result<EnrollmentStatus, String> { let (_, fingerprint) = current_identity(&app)?; let row = device_rows().await?.into_iter().find(|row| row.public_key_fingerprint.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))); Ok(match row { Some(row) => EnrollmentStatus { status: row.authority_state, device_id: Some(row.id) }, None => EnrollmentStatus { status: "legacy".to_owned(), device_id: None } }) }
#[tauri::command]
pub(crate) async fn broker_device_list() -> Result<Vec<DeviceRow>, String> { Ok(device_rows().await?.into_iter().map(|row| DeviceRow { id: row.id, label: row.device_label.unwrap_or_default(), platform: row.platform, authority_state: row.authority_state, paired_at: row.registered_at, revoked_at: row.revoked_at, endpoint_state: row.lan_endpoint.map(|_| "published".to_owned()) }).collect()) }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointUpdate<'a> { lan_endpoint: &'a str, lan_endpoint_updated_at: &'a str }

#[tauri::command]
pub(crate) async fn broker_device_endpoint_publish(app: AppHandle, state: State<'_, AppState>) -> Result<EndpointStatus, String> {
    let endpoint = crate::fungwire_local_endpoint_native(state).map_err(|_| public_error("fungwire_unavailable"))?;
    let Some(endpoint) = endpoint else { return Ok(EndpointStatus { status: "unavailable", updated_at: None }); };
    let (_, fingerprint) = current_identity(&app)?;
    let device = device_rows().await?.into_iter().find(|row| row.public_key_fingerprint.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))).ok_or_else(|| public_error("device_not_enrolled"))?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?; url.set_path("/rest/v1/devices"); url.set_query(Some(&format!("id=eq.{}", device.id)));
    let updated_at = chrono::Utc::now().to_rfc3339();
    let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("authorization_unavailable"))?.patch(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).header("Prefer", "return=representation").json(&EndpointUpdate { lan_endpoint: &endpoint, lan_endpoint_updated_at: &updated_at }).send().await.map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() { return Err(public_error("authorization_denied")); }
    let updated: Vec<DeviceWire> = response.json().await.map_err(|_| public_error("authorization_unavailable"))?;
    if updated.len() != 1 { return Err(public_error("authorization_denied")); }
    Ok(EndpointStatus { status: "published", updated_at: Some(updated_at) })
}

fn current_identity(app: &AppHandle) -> Result<(String, String), String> { let app_data = app.path().app_data_dir().map_err(|_| public_error("device_identity_unavailable"))?; device_identity::authorization_identity_in_dir(&app_data).map_err(|_| public_error("device_identity_unavailable")) }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairingCreateInput { pub(crate) label: Option<String> }
#[derive(Serialize)]
struct PairingRpc<'a> { p_session_id: &'a str, p_code_hash: &'a str, p_initiator_device_id: &'a str }
#[derive(Deserialize)]
struct PairingSessionWire { id: String, status: String, responder_device_id: Option<String> }

async fn rpc<T: for<'de> Deserialize<'de>>(name: &str, body: impl Serialize) -> Result<T, String> { let access = ensure_access_token().await?; let mut url = native_auth::configured_supabase_origin()?; url.set_path(&format!("/rest/v1/rpc/{name}")); let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("pairing_unavailable"))?.post(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).json(&body).send().await.map_err(|_| public_error("pairing_unavailable"))?; if !response.status().is_success() { return Err(public_error("pairing_unavailable")); } response.json::<T>().await.map_err(|_| public_error("pairing_unavailable")) }

#[tauri::command]
pub(crate) async fn broker_pairing_create(app: AppHandle, input: Option<PairingCreateInput>) -> Result<PairingResult, String> { if input.as_ref().and_then(|value| value.label.as_deref()).is_some_and(|label| label.len() > 80 || label.chars().any(char::is_control)) { return Err(public_error("invalid_input")); } let devices = device_rows().await?; let (_, fingerprint) = current_identity(&app)?; let device = devices.into_iter().find(|row| row.public_key_fingerprint.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))).ok_or_else(|| public_error("device_not_enrolled"))?; let session_id = Uuid::new_v4().to_string(); let mut code_bytes = [0u8; 4]; OsRng.fill_bytes(&mut code_bytes); let code = format!("{:06}", u32::from_be_bytes(code_bytes) % 1_000_000); let digest = Sha256::digest(format!("{session_id}:{code}").as_bytes()); let hash = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>(); let _: Option<String> = rpc("create_pairing_session", PairingRpc { p_session_id: &session_id, p_code_hash: &hash, p_initiator_device_id: &device.id }).await?; Ok(PairingResult { pairing_id: session_id, display_code: code, expires_at_ms: now_ms() + 300_000, status: "waiting" }) }

async fn pairing_row(pairing_id: &str) -> Result<PairingSessionWire, String> { if Uuid::parse_str(pairing_id).is_err() { return Err(public_error("pairing_not_found")); } let access = ensure_access_token().await?; let mut url = native_auth::configured_supabase_origin()?; url.set_path("/rest/v1/pairing_sessions"); url.set_query(Some(&format!("id=eq.{pairing_id}&select=id,status,responder_device_id"))); let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("pairing_unavailable"))?.get(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).send().await.map_err(|_| public_error("pairing_unavailable"))?; if !response.status().is_success() { return Err(public_error("pairing_unavailable")); } response.json::<Vec<PairingSessionWire>>().await.map_err(|_| public_error("pairing_unavailable"))?.into_iter().next().ok_or_else(|| public_error("pairing_not_found")) }

#[tauri::command]
pub(crate) async fn broker_pairing_poll(pairing_id: String) -> Result<PairingPoll, String> { let row = pairing_row(&pairing_id).await?; let peer = if let Some(device_id) = row.responder_device_id { device_rows().await?.into_iter().find(|device| device.id == device_id).map(|device| PairingPeer { id: device.id, label: device.device_label.unwrap_or_default(), platform: device.platform, fingerprint: device.public_key_fingerprint.unwrap_or_default() }) } else { None }; Ok(PairingPoll { status: row.status, peer }) }
#[tauri::command]
pub(crate) async fn broker_pairing_reconcile(app: AppHandle) -> Result<ReconcileStatus, String> { let devices = device_rows().await?; let (_, fingerprint) = current_identity(&app)?; let device_id = devices.into_iter().find(|row| row.public_key_fingerprint.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))).map(|row| row.id); Ok(ReconcileStatus { reconciled: device_id.is_some(), device_id }) }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevokeRequest<'a> { action: &'static str, device_id: &'a str }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevokeWire { device_id: String, authority_state: String }
#[tauri::command]
pub(crate) async fn broker_device_revoke(device_id: String) -> Result<RevokeResult, String> { if Uuid::parse_str(&device_id).is_err() { return Err(public_error("device_not_found")); } let response: RevokeWire = native_post("/functions/v1/device-enrollment", RevokeRequest { action: "revoke", device_id: &device_id }).await?; if response.authority_state != "revoked" { return Err(public_error("authorization_denied")); } Ok(RevokeResult { device_id: response.device_id, status: "revoked" }) }

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct AuditWire { id: String, event_type: String, created_at: String, device_id: Option<String> }
#[tauri::command]
pub(crate) async fn broker_device_audit_list() -> Result<Vec<AuditRow>, String> { let access = ensure_access_token().await?; let mut url = native_auth::configured_supabase_origin()?; url.set_path("/rest/v1/device_audit_events"); url.set_query(Some("select=id,event_type,created_at,device_id&order=created_at.desc&limit=100")); let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| public_error("audit_unavailable"))?.get(url).bearer_auth(access.as_str()).header("apikey", native_auth::configured_supabase_anon_key()?).send().await.map_err(|_| public_error("audit_unavailable"))?; if !response.status().is_success() { return Err(public_error("audit_unavailable")); } Ok(response.json::<Vec<AuditWire>>().await.map_err(|_| public_error("audit_unavailable"))?.into_iter().map(|row| AuditRow { event_id: row.id, event_type: row.event_type, created_at: row.created_at, device_id: row.device_id }).collect()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_rejects_duplicate_or_extra_parameters() { let pending = PendingLogin { request_id: "r".into(), generation: 1, port: 43123, state: Zeroizing::new("s".into()), verifier: Zeroizing::new("v".into()), expires_at: SystemTime::now() + LOGIN_TTL, callback: Arc::new(Mutex::new(None)), cancelled: Arc::new(AtomicBool::new(false)) }; assert!(parse_callback("http://127.0.0.1:43123/auth/callback?code=c&state=s&code=d", &pending).is_err()); assert!(parse_callback("http://127.0.0.1:43123/auth/callback?code=c&state=s", &pending).is_ok()); }
    #[test]
    fn staged_keyring_protocol_names_are_native_only() { assert_eq!(KEYRING_SERVICE, "FUNG"); assert_ne!(ACTIVE_SLOT, STAGED_SLOT); }
}
