//! Native authorization for provider operations.
//!
//! The webview contributes only its current Supabase bearer proof. Native
//! derives the device key, signs a short-lived canonical request, and keeps
//! the returned authorization context in memory for one command invocation.

use crate::{device_identity, AppError, AppResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

const AUTHORIZE_FUNCTION_PATH: &str = "/functions/v1/google-drive-authorize";
const DRIVE_PROVIDER: &str = "google_drive";
const AUTHORIZATION_REQUEST_TTL: Duration = Duration::from_secs(90);
const MAX_SESSION_PROOF_BYTES: usize = 8192;

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

fn trusted_loopback_redirect(value: &str) -> bool {
    if value == "fung://auth/callback" {
        return true;
    }
    let Ok(parsed) = Url::parse(value) else {
        return false;
    };
    parsed.scheme() == "http"
        && parsed.host_str() == Some("127.0.0.1")
        && parsed.port().is_some_and(|port| port != 0)
        && parsed.path() == "/auth/callback"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

pub(crate) fn validate_trusted_auth_url(candidate: &str, origin: &Url) -> Result<(), String> {
    let parsed = Url::parse(candidate).map_err(|_| public_error("auth_url_untrusted"))?;
    if !same_origin(&parsed, origin)
        || parsed.path() != "/auth/v1/authorize"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(public_error("auth_url_untrusted"));
    }
    let provider = parsed
        .query_pairs()
        .find(|(key, _)| key == "provider")
        .map(|(_, value)| value.into_owned());
    let redirect = parsed
        .query_pairs()
        .find(|(key, _)| key == "redirect_to")
        .map(|(_, value)| value.into_owned());
    if provider.as_deref() != Some("google")
        || !redirect
            .as_deref()
            .is_some_and(|value| trusted_loopback_redirect(value))
    {
        return Err(public_error("auth_url_untrusted"));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn open_trusted_auth_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let origin = configured_supabase_origin()?;
    validate_trusted_auth_url(&url, &origin)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| public_error("auth_url_open_failed"))
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
    fn trusted_auth_url_rejects_foreign_origin_and_provider() {
        let origin = Url::parse("https://project.supabase.co").unwrap();
        let trusted = "https://project.supabase.co/auth/v1/authorize?provider=google&redirect_to=http%3A%2F%2F127.0.0.1%3A43123%2Fauth%2Fcallback";
        assert!(validate_trusted_auth_url(trusted, &origin).is_ok());
        assert!(validate_trusted_auth_url(
            "https://evil.example/auth/v1/authorize?provider=google&redirect_to=fung%3A%2F%2Fauth%2Fcallback",
            &origin,
        )
        .is_err());
        assert!(validate_trusted_auth_url(
            "https://project.supabase.co/auth/v1/authorize?provider=github&redirect_to=fung%3A%2F%2Fauth%2Fcallback",
            &origin,
        )
        .is_err());
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
}
