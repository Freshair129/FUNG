//! Zoom cloud-recording ingestion: OAuth (PKCE), REST pulls, import job.
//! Tokens live ONLY in the OS credential store (keyring). Never persist or
//! log tokens or download URLs.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
}
