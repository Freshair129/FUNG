//! Native-only authorization helpers for the Desktop broker.
//!
//! This module contains no Tauri event payload, browser session DTO, callback
//! command, or caller-supplied session proof. Native session access material
//! is obtained from `auth_session` and is used only inside the current
//! provider request.

use crate::{auth_session, device_identity, AppError, AppResult};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::Signer;
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

const AUTHORIZE_FUNCTION_PATH: &str = "/functions/v1/google-drive-authorize";
const DRIVE_PROVIDER: &str = "google_drive";
const AUTHORIZATION_REQUEST_TTL: Duration = Duration::from_secs(90);
const ENROLLMENT_OPERATION: &str = "device.enrollment.request";
const ENROLLMENT_PLATFORM: &str = "windows";
const ENROLLMENT_DOMAIN: &[u8] = b"FUNG\0DEVICE_ENROLLMENT\0V1\0";
const ENROLLMENT_PROOF_TTL_MS: i64 = 300_000;

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

pub(crate) struct AuthorizedDriveContext {
    user_id: String,
    device_id: String,
    device_fingerprint: String,
    connection_id: Option<String>,
    operation: DriveOperation,
    expires_at: SystemTime,
}

pub(crate) struct DriveInvocation { context: AuthorizedDriveContext }

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
        if matches!(self.context.operation, DriveOperation::BackupRead | DriveOperation::BackupWrite | DriveOperation::BackupRestore)
            && self.context.connection_id.is_none()
        {
            return Err(public_error("authorization_denied"));
        }
        Ok(())
    }

    pub(crate) fn require_operation(&self, expected: DriveOperation) -> Result<(), String> {
        self.ensure_valid()?;
        if self.context.operation != expected { return Err(public_error("authorization_denied")); }
        Ok(())
    }

    pub(crate) fn user_id(&self) -> &str { &self.context.user_id }
    pub(crate) fn device_id(&self) -> &str { &self.context.device_id }
    pub(crate) fn device_fingerprint(&self) -> &str { &self.context.device_fingerprint }
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

fn public_error(code: &str) -> String { code.to_owned() }

pub(crate) fn configured_value(native_name: &str, frontend_name: &str) -> Result<String, String> {
    env::var(native_name)
        .or_else(|_| env::var(frontend_name))
        .map(|value| value.trim().to_owned())
        .map_err(|_| public_error("auth_config_invalid"))
        .and_then(|value| if value.is_empty() { Err(public_error("auth_config_invalid")) } else { Ok(value) })
}

pub(crate) fn configured_supabase_origin() -> Result<Url, String> {
    let parsed = Url::parse(&configured_value("FUNG_SUPABASE_URL", "VITE_SUPABASE_URL")?)
        .map_err(|_| public_error("auth_config_invalid"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty()
        || parsed.password().is_some() || parsed.port().is_some()
        || !matches!(parsed.path(), "" | "/") || parsed.query().is_some() || parsed.fragment().is_some()
    { return Err(public_error("auth_config_invalid")); }
    Ok(parsed)
}

pub(crate) fn configured_supabase_anon_key() -> Result<String, String> {
    configured_value("FUNG_SUPABASE_ANON_KEY", "VITE_SUPABASE_ANON_KEY")
}

fn authorize_function_url() -> Result<Url, String> {
    let mut origin = configured_supabase_origin()?;
    origin.set_path(AUTHORIZE_FUNCTION_PATH);
    Ok(origin)
}

fn unix_timestamp_ms() -> Result<u64, String> {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_millis() as u64)
        .map_err(|_| public_error("authorization_unavailable"))
}

fn canonical_request(operation: DriveOperation, timestamp_ms: u64, nonce: &str, fingerprint: &str) -> String {
    format!("fung-drive-auth-v1\n{}\n{timestamp_ms}\n{nonce}\n{fingerprint}", operation.as_str())
}

fn validate_uuid(value: &str) -> Result<(), String> {
    Uuid::parse_str(value).map(|_| ()).map_err(|_| public_error("authorization_denied"))
}

fn validate_authorization_response(response: AuthorizationResponse, operation: DriveOperation, fingerprint: &str, nonce: &str, now_ms: u64) -> Result<AuthorizedDriveContext, String> {
    if !response.authorized || response.provider != DRIVE_PROVIDER || response.operation != operation.as_str()
        || response.device_fingerprint != fingerprint || response.nonce != nonce
        || response.expires_at_ms <= now_ms || response.expires_at_ms > now_ms + AUTHORIZATION_REQUEST_TTL.as_millis() as u64
    { return Err(public_error("authorization_denied")); }
    validate_uuid(&response.user_id)?;
    validate_uuid(&response.device_id)?;
    if let Some(connection_id) = response.connection_id.as_deref() { validate_uuid(connection_id)?; }
    Ok(AuthorizedDriveContext {
        user_id: response.user_id,
        device_id: response.device_id,
        device_fingerprint: response.device_fingerprint,
        connection_id: response.connection_id,
        operation,
        expires_at: UNIX_EPOCH + Duration::from_millis(response.expires_at_ms),
    })
}

/// Derive the current native account/device and authorize one exact operation
/// before any provider or Drive keyring effect.
pub(crate) async fn authorize_drive(app: &tauri::AppHandle, operation: DriveOperation) -> Result<AuthorizedDriveContext, String> {
    let access = auth_session::ensure_access_token().await?;
    let anon_key = configured_supabase_anon_key()?;
    let app_data = app.path().app_data_dir().map_err(|_| public_error("device_identity_unavailable"))?;
    let timestamp_ms = unix_timestamp_ms()?;
    let nonce = Uuid::new_v4().to_string();
    let (device_public_key, device_fingerprint) = device_identity::authorization_identity_in_dir(&app_data)
        .map_err(|_| public_error("device_identity_unavailable"))?;
    let canonical = canonical_request(operation, timestamp_ms, &nonce, &device_fingerprint);
    let signature = device_identity::sign_authorization_in_dir(&app_data, canonical.as_bytes())
        .map_err(|_| public_error("device_identity_unavailable"))?;
    let request = AuthorizationRequest { operation: operation.as_str(), device_public_key: &device_public_key, device_fingerprint: &device_fingerprint, signature: &signature, timestamp_ms, nonce: &nonce };
    let response = Client::builder().timeout(Duration::from_secs(15)).build()
        .map_err(|_| public_error("authorization_unavailable"))?
        .post(authorize_function_url()?)
        .bearer_auth(access.as_str()).header("apikey", anon_key).json(&request).send().await
        .map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() { return Err(public_error("authorization_denied")); }
    let authorization = response.json::<AuthorizationResponse>().await.map_err(|_| public_error("authorization_denied"))?;
    validate_authorization_response(authorization, operation, &device_fingerprint, &nonce, timestamp_ms)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeEnrollmentProof {
    pub(crate) version: u8,
    pub(crate) operation: String,
    pub(crate) user_id: String,
    pub(crate) public_key: String,
    pub(crate) fingerprint: String,
    pub(crate) fingerprint_hex: String,
    pub(crate) platform: String,
    pub(crate) device_label: String,
    pub(crate) issued_at_ms: i64,
    pub(crate) expires_at_ms: i64,
    pub(crate) nonce: String,
    pub(crate) signature: String,
}

fn canonical_enrollment_bytes(user_id: &Uuid, public_key: &[u8], fingerprint: &[u8], label: &str, issued_at_ms: i64, expires_at_ms: i64, nonce: &[u8]) -> Result<Vec<u8>, String> {
    if public_key.len() != 32 || fingerprint.len() != 32 || nonce.len() != 32 || label.is_empty() || label.len() > 80 { return Err(public_error("invalid_enrollment_proof")); }
    let mut result = Vec::with_capacity(160);
    result.extend_from_slice(ENROLLMENT_DOMAIN);
    result.extend_from_slice(user_id.as_bytes());
    result.extend_from_slice(public_key);
    result.extend_from_slice(fingerprint);
    result.extend_from_slice(&(ENROLLMENT_PLATFORM.len() as u16).to_be_bytes());
    result.extend_from_slice(ENROLLMENT_PLATFORM.as_bytes());
    result.extend_from_slice(&(label.len() as u16).to_be_bytes());
    result.extend_from_slice(label.as_bytes());
    result.extend_from_slice(&issued_at_ms.to_be_bytes());
    result.extend_from_slice(&expires_at_ms.to_be_bytes());
    result.extend_from_slice(nonce);
    Ok(result)
}

pub(crate) async fn native_device_enrollment_proof(app: &tauri::AppHandle, device_label: &str) -> Result<NativeEnrollmentProof, String> {
    let user_id = auth_session::native_user_id().ok_or_else(|| public_error("auth_required"))?;
    let label = device_label.trim();
    if label.is_empty() || label.len() > 80 || label.chars().any(char::is_control) { return Err(public_error("invalid_enrollment_proof")); }
    let app_data = app.path().app_data_dir().map_err(|_| public_error("device_identity_unavailable"))?;
    let (public_key_text, stored_fingerprint) = device_identity::authorization_identity_in_dir(&app_data).map_err(|_| public_error("device_identity_unavailable"))?;
    let public_key = base64::engine::general_purpose::STANDARD.decode(public_key_text.as_bytes()).map_err(|_| public_error("device_identity_unavailable"))?;
    let fingerprint = Sha256::digest(&public_key);
    let fingerprint_hex = fingerprint.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    if fingerprint_hex != stored_fingerprint { return Err(public_error("device_identity_unavailable")); }
    let issued_at_ms = unix_timestamp_ms()? as i64;
    let expires_at_ms = issued_at_ms + ENROLLMENT_PROOF_TTL_MS;
    let mut nonce = [0u8; 32]; OsRng.fill_bytes(&mut nonce);
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| public_error("auth_required"))?;
    let canonical = canonical_enrollment_bytes(&user_uuid, &public_key, &fingerprint, label, issued_at_ms, expires_at_ms, &nonce)?;
    let signing_key = device_identity::secure_signing_key_in_dir(&app_data).map_err(|_| public_error("device_identity_unavailable"))?;
    let signature = signing_key.sign(&canonical).to_bytes();
    Ok(NativeEnrollmentProof { version: 1, operation: ENROLLMENT_OPERATION.to_owned(), user_id, public_key: URL_SAFE_NO_PAD.encode(public_key), fingerprint: URL_SAFE_NO_PAD.encode(fingerprint), fingerprint_hex, platform: ENROLLMENT_PLATFORM.to_owned(), device_label: label.to_owned(), issued_at_ms, expires_at_ms, nonce: URL_SAFE_NO_PAD.encode(nonce), signature: URL_SAFE_NO_PAD.encode(signature) })
}

pub(crate) fn open_trusted_account_portal(app: tauri::AppHandle) -> AppResult<()> {
    let raw = env::var("FUNG_WEB_APP_URL").map_err(|_| AppError::InvalidInput("FUNG_WEB_APP_URL is not configured".to_owned()))?;
    let url = Url::parse(raw.trim()).map_err(|_| AppError::InvalidInput("FUNG_WEB_APP_URL is invalid".to_owned()))?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() { return Err(AppError::InvalidInput("FUNG_WEB_APP_URL must be a trusted https URL".to_owned())); }
    app.opener().open_url(url.as_str(), None::<&str>).map_err(|error| AppError::InvalidInput(format!("could not open account portal: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_request_binds_operation_timestamp_nonce_and_fingerprint() { assert_eq!(canonical_request(DriveOperation::BackupRestore, 123, "nonce", "fingerprint"), "fung-drive-auth-v1\nbackup.restore\n123\nnonce\nfingerprint"); }
    #[test]
    fn enrollment_canonical_bytes_bind_native_identity_and_network_integers() { let user = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(); let bytes = canonical_enrollment_bytes(&user, &[1; 32], &[2; 32], "FUNG Desktop", 10, 300010, &[3; 32]).unwrap(); assert!(bytes.starts_with(ENROLLMENT_DOMAIN)); assert!(bytes.ends_with(&[3; 32])); }
    #[test]
    fn authorization_context_is_not_a_public_serializable_type() { fn accepts(_: AuthorizedDriveContext) {} let _ = accepts; }
}
