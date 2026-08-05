# Zoom Meeting Ingestion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** FUNG Desktop connects to a user's Zoom account, imports cloud recordings, produces speaker-attributed transcripts (per-participant audio files when available, local pyannote diarization as fallback), and builds a knowledge graph (structural + LLM-extracted) in GenesisBlockDB.

**Architecture:** Everything runs inside the existing Tauri desktop runtime. A new `zoom_sync` Rust module handles OAuth (PKCE, loopback callback) and REST pulls; downloads feed the existing faster-whisper worker; a new `speaker_merge` module attributes speakers; a new `graph_build` module writes graph nodes/edges via `genesis_adapter`. Spec: `docs/superpowers/specs/2026-08-05-zoom-meeting-ingestion-design.md`.

**Tech Stack:** Rust (sync style, `thread::spawn`, `reqwest::blocking`), keyring (Windows Credential Manager), Python faster-whisper + pyannote-audio workers, React/TS frontend, GenesisBlockDB via `genesis_adapter`.

## Global Constraints

- **Local-first:** audio, transcripts, and graph data never leave the machine. OAuth tokens live ONLY in Windows Credential Manager via the `keyring` crate — never in GenesisBlockDB, never in logs, never in files.
- **No async Rust:** this codebase uses sync commands + `thread::spawn` (see `import_and_transcribe` in `src-tauri/src/lib.rs:616`). Use `reqwest::blocking`, never tokio/async fn.
- **All persistence through `genesis_adapter`:** `upsert`/`commit_rows`/`query`/`eq`/`delete`. Never open SQLite directly in production paths.
- **Job pattern:** long work = row in `jobs` + `job_events`, status updates via `set_job_status` (`src-tauri/src/lib.rs:592`). New job types: `zoom.import`, `graph.build`.
- **Python worker contract** (same as `scripts/transcribe.py`): `PROGRESS <0-100>` lines on stderr, one JSON object on stdout, UTF-8 reconfigured.
- **Zoom app config:** client ID comes from env `FUNG_ZOOM_CLIENT_ID` (public PKCE app — no client secret anywhere).
- **Never log** Zoom `download_url` values or `Authorization` headers (they carry credentials).
- Diarization being unavailable must never block transcript availability (job still completes; event notes the gap).
- Frontend build check is `npm run build` (tsc + vite). Rust check is `cargo test --manifest-path src-tauri/Cargo.toml`.

## File Structure

| File | Responsibility |
| --- | --- |
| `src-tauri/Cargo.toml` | Add `reqwest`, `keyring`, `base64` deps |
| `src-tauri/src/genesis_adapter.rs` | Schema v4: `external_connections`, `external_imports` tables |
| `src-tauri/src/zoom_sync.rs` (new) | OAuth PKCE + token store + Zoom REST + `zoom.import` job |
| `src-tauri/src/speaker_merge.rs` (new) | Pure merge/attribution logic + persistence of speakers/segments/turns |
| `src-tauri/src/graph_build.rs` (new) | Structural graph + LLM extraction + `graph.build` job |
| `src-tauri/src/lib.rs` | Register modules/commands; make `WhisperOutput`/`run_transcription` reusable; generic python worker runner |
| `scripts/diarize.py` (new) | pyannote-audio diarization worker |
| `src/tauri.ts` | TS wrappers for the 6 new commands |
| `src/components/ZoomPanel.tsx` + `.css` (new) | Connect + recordings list + import UI |
| `src/App.tsx` | Mount ZoomPanel + trigger button |
| `docs/Desktop/ZOOM_INTEGRATION_SETUP.md` (new) | Zoom app creation, scopes, pyannote/HF setup |

---

### Task 1: Dependencies + Genesis schema v4

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/genesis_adapter.rs`
- Modify: `src-tauri/src/lib.rs:338` (test-schema CHECK parity)

**Interfaces:**
- Produces: tables `external_connections` (id, provider, account_label, status, created_at, updated_at) and `external_imports` (id, project_id, provider, external_uuid, recording_id, payload_json, created_at); crates `reqwest::blocking`, `keyring`, `base64` available to later tasks.

- [ ] **Step 1: Add dependencies to `src-tauri/Cargo.toml`**

Append to `[dependencies]`:

```toml
base64 = "0.22"
keyring = { version = "3", features = ["windows-native"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
```

- [ ] **Step 2: Write the failing schema test**

In `src-tauri/src/genesis_adapter.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn schema_v4_adds_external_tables_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        // v4 tables accept rows through the normal adapter path.
        commit_rows(&storage, vec![
            upsert("external_connections", json!({"id": "zoom", "provider": "zoom", "account_label": "user@example.com", "status": "connected", "created_at": "t", "updated_at": "t"})),
        ]).unwrap();
        let rows = query(&storage, "external_connections", &["id", "status"], vec![eq("external_connections", "id", json!("zoom"))], 1).unwrap();
        assert_eq!(rows[0]["external_connections.status"], "connected");
        // Re-install after a stepped upgrade must stay idempotent (mirrors existing v1->v3 test).
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml schema_v4 -- --nocapture`
Expected: FAIL — `external_connections` is not a registered table (relational mutation rejected).

- [ ] **Step 4: Implement schema v4**

In `src-tauri/src/genesis_adapter.rs`: rename the current `pub(crate) fn schema()` to `fn schema_v3()` (keep its body, still setting `schema_version = 3`). Add below it:

```rust
pub(crate) fn schema() -> RelationalSchemaPackage {
    use RelationalColumnType::{Json, Text};
    let mut package = schema_v3();
    package.schema_version = 4;
    package.previous_version = Some(3);
    package.tables.extend([
        table(
            "external_connections",
            vec![
                required("id", Text),
                required("provider", Text),
                required("account_label", Text),
                required("status", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "external_imports",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("provider", Text),
                required("external_uuid", Text),
                required("recording_id", Text),
                required("payload_json", Json),
                required("created_at", Text),
            ],
            vec![fk("project_id", "projects"), fk("recording_id", "recordings")],
            vec![],
        ),
    ]);
    package
}
```

Note: the existing test `install_is_idempotent_after_a_prior_schema_upgrade` registers `schema_v1()`, `schema_v2()`, then `schema()` — it now exercises v1→v2→v4 which remains valid because `schema()` chains `previous_version` correctly. Also update the `priority` closure in `import_legacy_sqlite` — add `"external_connections"` to the priority-2 arm and `"external_imports"` to the priority-5 arm.

- [ ] **Step 5: Test-schema parity in `src-tauri/src/lib.rs`**

In `init_database` (test-only sqlite mirror), extend the `jobs.type` CHECK list at line ~338 to include the new job types:

```sql
type TEXT NOT NULL CHECK (type IN ('recording.capture', 'recording.recover', 'audio.cleanup', 'audio.separate', 'transcript.transcribe', 'transcript.diarize', 'summary.generate', 'intent.infer', 'export.render', 'zoom.import', 'graph.build')),
```

- [ ] **Step 6: Run tests to verify pass**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml genesis_adapter`
Expected: PASS (all existing genesis tests + the new one).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/genesis_adapter.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): add http/keyring deps and genesis schema v4 external tables"
```

### Task 2: `zoom_sync.rs` core — token store, PKCE, token endpoints

**Files:**
- Create: `src-tauri/src/zoom_sync.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod zoom_sync;` next to the other `mod` lines)

**Interfaces:**
- Produces: `pub(crate) struct TokenSet { access_token: String, refresh_token: String, expires_at_epoch: i64 }`; `save_tokens(&TokenSet) -> Result<(), String>`; `load_tokens() -> Result<Option<TokenSet>, String>`; `delete_tokens() -> Result<(), String>`; `pkce_challenge(&str) -> String`; `authorize_url(client_id, redirect_uri, state, challenge) -> String`; `exchange_code(client_id, code, redirect_uri, verifier) -> Result<TokenSet, String>`; `ensure_fresh_access_token(client_id) -> Result<String, String>`.

- [ ] **Step 1: Write failing PKCE + URL tests**

Create `src-tauri/src/zoom_sync.rs` with just the test module first:

```rust
//! Zoom cloud-recording ingestion: OAuth (PKCE), REST pulls, import job.
//! Tokens live ONLY in the OS credential store (keyring). Never persist or
//! log tokens or download URLs.

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
}
```

Add `mod zoom_sync;` in `src-tauri/src/lib.rs` under the existing `mod on_device_ai;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: FAIL to compile — `pkce_challenge` / `authorize_url` not found.

- [ ] **Step 3: Implement token store + PKCE + endpoints**

Add above the test module in `src-tauri/src/zoom_sync.rs`:

```rust
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KEYRING_SERVICE: &str = "FUNG";
const KEYRING_USER: &str = "zoom-oauth";
const ZOOM_AUTH_BASE: &str = "https://zoom.us";
pub(crate) const ZOOM_API_BASE: &str = "https://api.zoom.us/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TokenSet {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    /// Unix seconds after which `access_token` is no longer valid.
    pub(crate) expires_at_epoch: i64,
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
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/zoom_sync.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): PKCE helpers, keyring token store, token endpoints"
```

### Task 3: OAuth loopback flow + connect/status/disconnect commands

**Files:**
- Modify: `src-tauri/src/zoom_sync.rs`
- Modify: `src-tauri/src/lib.rs` (register 3 commands in `tauri::generate_handler!`)

**Interfaces:**
- Consumes: Task 2 token functions; `crate::genesis_adapter::{upsert, commit_rows, query, eq, string}`; `crate::{AppState, AppError, AppResult, now}`.
- Produces: commands `zoom_connect() -> ZoomConnectionStatus`, `zoom_connection_status() -> ZoomConnectionStatus`, `zoom_disconnect() -> ZoomConnectionStatus` where `ZoomConnectionStatus { status: String, account_label: Option<String> }` (serde camelCase; `status` ∈ `disconnected|connecting|connected|error`); helper `parse_callback_request(&str) -> Result<(String, String), String>` returning `(code, state)`.

- [ ] **Step 1: Write failing callback-parser tests**

Add to the `tests` module in `zoom_sync.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync::tests::callback`
Expected: FAIL to compile — `parse_callback_request` not found.

- [ ] **Step 3: Implement parser, connection persistence, and commands**

Add to `zoom_sync.rs`:

```rust
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::TcpListener;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{genesis_adapter, now, AppError, AppResult, AppState};

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
```

Register in `src-tauri/src/lib.rs` inside `tauri::generate_handler![` (after `open_external_account_portal,`):

```rust
            zoom_sync::zoom_connect,
            zoom_sync::zoom_connection_status,
            zoom_sync::zoom_disconnect,
```

- [ ] **Step 4: Run tests + build**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: PASS (4 tests). Full compile succeeds.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/zoom_sync.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): OAuth loopback connect/status/disconnect commands"
```

### Task 4: List recordings — API models + `zoom_list_recordings`

**Files:**
- Modify: `src-tauri/src/zoom_sync.rs`
- Modify: `src-tauri/src/lib.rs` (register command)

**Interfaces:**
- Consumes: `ensure_fresh_access_token`, `client_id_from_env`.
- Produces: command `zoom_list_recordings() -> Vec<ZoomRecordingSummary>` with `ZoomRecordingSummary { uuid: String, topic: String, start_time: String, duration_minutes: i64, has_participant_audio: bool }` (serde camelCase); internal `ZoomMeetingRecording { uuid, topic, start_time, duration, recording_files: Vec<ZoomRecordingFile>, participant_audio_files: Option<Vec<ZoomParticipantAudioFile>> }`, `ZoomRecordingFile { file_type: String, recording_type: Option<String>, download_url: String }`, `ZoomParticipantAudioFile { file_name: String, download_url: String }`; helper `encode_meeting_uuid(&str) -> String`.

- [ ] **Step 1: Write failing parse/encode tests**

Add to `tests` in `zoom_sync.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: FAIL to compile — missing types.

- [ ] **Step 3: Implement models, summarize, encode, and the command**

Add to `zoom_sync.rs`:

```rust
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
```

Register `zoom_sync::zoom_list_recordings,` in `generate_handler!` after `zoom_sync::zoom_disconnect,`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/zoom_sync.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): recordings list command with fixture-tested API models"
```

### Task 5: `zoom.import` job — download + idempotency

**Files:**
- Modify: `src-tauri/src/zoom_sync.rs`
- Modify: `src-tauri/src/lib.rs` (register command; make `import` helpers reusable)

**Interfaces:**
- Consumes: Task 4 models; `crate::{set_job_status}` — change its visibility in `lib.rs` from `fn` to `pub(crate) fn` (same for `now` if not already).
- Produces: command `zoom_import_recording(meeting_uuid: String) -> Job`; helpers `find_existing_import(storage, uuid) -> Result<Option<String>, String>` (returns recording_id), `sanitize_component(&str) -> String`, `download_to_file(access_token, url, dest) -> Result<(), String>` (resumes via HTTP Range when a partial file exists). Downloaded layout: `<data_root>/projects/<project_id>/zoom/<safe_uuid>/mixed.m4a` and `participants/<i>-<safe_name>.m4a`. After download it records `external_imports` + flips `recordings.status`, then runs Task 6/7 processing in the same worker thread.

- [ ] **Step 1: Write failing tests (idempotency + sanitize)**

Add to `tests` in `zoom_sync.rs` (mirror the temp-storage pattern from `genesis_adapter::tests::open`):

```rust
    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-zoom-test-{}", uuid::Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(), page_cache_mb: Some(16), read_only: Some(false), vector_dim: Some(4),
        }).unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    #[test]
    fn import_idempotency_finds_prior_recording_by_uuid() {
        let (path, storage) = open_storage();
        assert_eq!(find_existing_import(&storage, "uuid-1").unwrap(), None);
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("external_imports", serde_json::json!({"id":"i1","project_id":"p1","provider":"zoom","external_uuid":"uuid-1","recording_id":"r1","payload_json":{},"created_at":"t"})),
        ]).unwrap();
        assert_eq!(find_existing_import(&storage, "uuid-1").unwrap(), Some("r1".to_string()));
        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn sanitize_component_keeps_paths_safe() {
        assert_eq!(sanitize_component("abc//slash=="), "abc__slash__");
        assert_eq!(sanitize_component("Audio only - Boss"), "Audio only - Boss");
        assert_eq!(sanitize_component("a<b>:c|?*\\/"), "a_b__c_____");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: FAIL to compile — helpers not found.

- [ ] **Step 3: Implement helpers + import command**

In `src-tauri/src/lib.rs` change `fn set_job_status(` to `pub(crate) fn set_job_status(`. Then add to `zoom_sync.rs`:

```rust
pub(crate) fn find_existing_import(storage: &genesis_block_native::Storage, uuid: &str) -> Result<Option<String>, String> {
    Ok(genesis_adapter::query(storage, "external_imports", &["recording_id", "provider", "external_uuid"],
        vec![genesis_adapter::eq("external_imports", "external_uuid", serde_json::json!(uuid))], 10)?
        .into_iter()
        .find(|row| row.get("external_imports.provider").and_then(serde_json::Value::as_str) == Some("zoom"))
        .and_then(|row| row.get("external_imports.recording_id").and_then(serde_json::Value::as_str).map(str::to_owned)))
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
    let mut response = request.send().map_err(|e| format!("zoom download failed: {e}"))?;
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
    if find_existing_import(&state.genesis, &meeting_uuid).map_err(AppError::Genesis)?.is_some() {
        return Err(AppError::InvalidInput("recording is already imported".to_string()));
    }
    let access_token = ensure_fresh_access_token(&client_id).map_err(AppError::InvalidInput)?;
    let meeting: ZoomMeetingRecording =
        api_get_json(&access_token, &format!("/meetings/{}/recordings", encode_meeting_uuid(&meeting_uuid)))
            .map_err(AppError::InvalidInput)?;

    let project_id = uuid::Uuid::new_v4().to_string();
    let recording_id = uuid::Uuid::new_v4().to_string();
    let job_id = uuid::Uuid::new_v4().to_string();
    let timestamp = now();
    let storage_path = state.data_root.join("projects").join(&project_id).display().to_string();
    let base_dir = state.data_root.join("projects").join(&project_id).join("zoom").join(sanitize_component(&meeting_uuid));
    let mixed_path = base_dir.join("mixed.m4a");

    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("projects", serde_json::json!({"id": project_id, "name": meeting.topic, "storage_path": storage_path, "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": "import", "input_path": null, "canonical_audio_path": mixed_path.display().to_string(), "status": "pending", "duration_ms": 0, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "zoom.import", "status": "running", "progress": 0, "input_refs_json": [meeting_uuid], "output_refs_json": [recording_id], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": timestamp, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": "downloading zoom recording", "created_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

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
        let timestamp = now();
        genesis_adapter::commit_rows(&ctx.storage, vec![
            genesis_adapter::upsert("external_imports", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "project_id": ctx.project_id, "provider": "zoom", "external_uuid": ctx.meeting_uuid, "recording_id": ctx.recording_id, "payload_json": {"topic": ctx.meeting_topic, "participantFiles": participants.len()}, "created_at": timestamp})),
        ])?;
        Ok(participants)
    })();

    match result {
        Ok(participants) => run_processing_pipeline(ctx, participants),
        Err(message) => { let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "failed", None, Some(&message)); }
    }
}

/// Placeholder until Task 6/7 wire transcription + attribution + graph.
fn run_processing_pipeline(ctx: ImportContext, _participants: Vec<(String, std::path::PathBuf)>) {
    let _ = crate::set_job_status(&ctx.storage, &ctx.job_id, "completed", Some(100), None);
}
```

In `src-tauri/src/lib.rs` add an accessor on `AppState` (below the struct):

```rust
impl AppState {
    pub(crate) fn whisper_runtime_clone(&self) -> WhisperRuntime {
        self.whisper_runtime.clone()
    }
}
```

and make `struct WhisperRuntime` + `struct Job` and Job's fields `pub(crate)` so `zoom_sync` can construct/return them. Register `zoom_sync::zoom_import_recording,` in `generate_handler!`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml zoom_sync`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/zoom_sync.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): import job with resumable downloads and uuid idempotency"
```

### Task 6: `speaker_merge.rs` — Path A merge + persistence

**Files:**
- Create: `src-tauri/src/speaker_merge.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod speaker_merge;`; make `WhisperOutput`, `WhisperSegment` and their fields `pub(crate)`; make `run_transcription` `pub(crate)`)
- Modify: `src-tauri/src/zoom_sync.rs` (`run_processing_pipeline` Path A)

**Interfaces:**
- Consumes: `crate::{WhisperOutput, WhisperSegment, run_transcription, set_job_status, now}`; `genesis_adapter`.
- Produces:
  - `pub(crate) struct AttributedSegment { pub speaker_key: Option<String>, pub display_name: Option<String>, pub start_ms: i64, pub end_ms: i64, pub text: String, pub confidence: Option<f64> }`
  - `merge_participant_outputs(Vec<(String, WhisperOutput)>) -> Vec<AttributedSegment>` — speaker_key = `p:<lowercased display name>`; sorted by (start_ms, end_ms).
  - `pub(crate) struct SpeakerTurn { pub speaker_key: String, pub display_name: String, pub start_ms: i64, pub end_ms: i64, pub confidence: Option<f64>, pub overlap: bool }`
  - `group_turns(&[AttributedSegment], gap_ms: i64) -> Vec<SpeakerTurn>` — merges adjacent same-speaker segments with gap ≤ gap_ms; `overlap = true` when the turn time-intersects any turn of another speaker.
  - `persist_attribution(storage, project_id, recording_id, runtime_location: &str, model_name: &str, segments: &[AttributedSegment], turns: &[SpeakerTurn], duration_ms: i64) -> Result<(), String>` — writes `speakers` (reuse-by-key like `mobile_diarization_import` at `src-tauri/src/mobile.rs:1047`), `transcript_segments` (with `speaker_id`), `speaker_turns` (status `proposed`), one `model_providers` row id `fung-desktop-attribution` (kind `diarization`, runtime_location as given), one `model_runs` row (task_kind `diarization`), and flips `recordings.status` to `completed` with `duration_ms`.

- [ ] **Step 1: Write failing merge/grouping tests**

Create `src-tauri/src/speaker_merge.rs` starting with tests:

```rust
//! Speaker attribution: merge per-participant whisper outputs (Path A) or
//! align diarization turns with a mixed transcript (Path B), then persist
//! speakers/segments/turns through genesis_adapter.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WhisperOutput, WhisperSegment};

    fn seg(start: i64, end: i64, text: &str) -> WhisperSegment {
        WhisperSegment { start_ms: start, end_ms: end, text: text.to_string(), confidence: Some(0.9) }
    }

    #[test]
    fn merge_interleaves_participants_by_time() {
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(0, 2_000, "hello"), seg(6_000, 8_000, "bye")] }),
            ("ATHER".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(2_500, 5_000, "hi")] }),
        ]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].display_name.as_deref(), Some("Boss"));
        assert_eq!(merged[1].display_name.as_deref(), Some("ATHER"));
        assert_eq!(merged[1].speaker_key.as_deref(), Some("p:ather"));
        assert_eq!(merged[2].text, "bye");
    }

    #[test]
    fn group_turns_merges_within_gap_and_flags_overlap() {
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(0, 2_000, "a"), seg(2_800, 4_000, "b"), seg(9_000, 9_500, "c")] }),
            ("ATHER".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(3_500, 6_000, "x")] }),
        ]);
        let turns = group_turns(&merged, 1_500);
        // Boss: [0..4000] (gap 800 <= 1500 merges) and [9000..9500]; ATHER: [3500..6000].
        assert_eq!(turns.len(), 3);
        let boss_first = turns.iter().find(|t| t.speaker_key == "p:boss" && t.start_ms == 0).unwrap();
        assert_eq!(boss_first.end_ms, 4_000);
        assert!(boss_first.overlap, "intersects ATHER 3500..6000");
        let boss_second = turns.iter().find(|t| t.speaker_key == "p:boss" && t.start_ms == 9_000).unwrap();
        assert!(!boss_second.overlap);
    }

    #[test]
    fn persist_attribution_reuses_speaker_keys_and_links_segments() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 4_000, segments: vec![seg(0, 2_000, "a")] }),
        ]);
        let turns = group_turns(&merged, 1_500);
        persist_attribution(&storage, "p1", "r1", "local", "faster-whisper per-participant", &merged, &turns, 4_000).unwrap();
        // Run twice: speaker row must be reused, not duplicated.
        persist_attribution(&storage, "p1", "r1", "local", "faster-whisper per-participant", &merged, &turns, 4_000).unwrap();
        let speakers = crate::genesis_adapter::query(&storage, "speakers", &["id", "key"],
            vec![crate::genesis_adapter::eq("speakers", "project_id", serde_json::json!("p1"))], 10).unwrap();
        assert_eq!(speakers.len(), 1);
        let segments = crate::genesis_adapter::query(&storage, "transcript_segments", &["id", "speaker_id"],
            vec![crate::genesis_adapter::eq("transcript_segments", "project_id", serde_json::json!("p1"))], 10).unwrap();
        assert!(segments.iter().all(|row| row.get("transcript_segments.speaker_id").and_then(serde_json::Value::as_str).is_some()));
        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-merge-test-{}", uuid::Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(), page_cache_mb: Some(16), read_only: Some(false), vector_dim: Some(4),
        }).unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }
}
```

Add `mod speaker_merge;` to `src-tauri/src/lib.rs` and make `WhisperOutput`/`WhisperSegment` + fields + `run_transcription` `pub(crate)`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml speaker_merge`
Expected: FAIL to compile — functions/types missing.

- [ ] **Step 3: Implement merge, grouping, persistence**

Add above the tests in `speaker_merge.rs`:

```rust
use crate::{genesis_adapter, now, WhisperOutput};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct AttributedSegment {
    pub(crate) speaker_key: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct SpeakerTurn {
    pub(crate) speaker_key: String,
    pub(crate) display_name: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) confidence: Option<f64>,
    pub(crate) overlap: bool,
}

pub(crate) fn merge_participant_outputs(outputs: Vec<(String, WhisperOutput)>) -> Vec<AttributedSegment> {
    let mut merged: Vec<AttributedSegment> = outputs.into_iter().flat_map(|(display_name, output)| {
        let key = format!("p:{}", display_name.trim().to_lowercase());
        output.segments.into_iter().map(move |segment| AttributedSegment {
            speaker_key: Some(key.clone()),
            display_name: Some(display_name.clone()),
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            text: segment.text,
            confidence: segment.confidence,
        }).collect::<Vec<_>>()
    }).collect();
    merged.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
    merged
}

pub(crate) fn group_turns(segments: &[AttributedSegment], gap_ms: i64) -> Vec<SpeakerTurn> {
    let mut turns: Vec<SpeakerTurn> = Vec::new();
    for segment in segments {
        let (Some(key), Some(name)) = (&segment.speaker_key, &segment.display_name) else { continue };
        let extend = turns.iter_mut().rev()
            .find(|turn| &turn.speaker_key == key)
            .filter(|turn| segment.start_ms - turn.end_ms <= gap_ms && segment.start_ms >= turn.start_ms);
        match extend {
            Some(turn) => {
                turn.end_ms = turn.end_ms.max(segment.end_ms);
                turn.confidence = match (turn.confidence, segment.confidence) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            None => turns.push(SpeakerTurn {
                speaker_key: key.clone(), display_name: name.clone(),
                start_ms: segment.start_ms, end_ms: segment.end_ms,
                confidence: segment.confidence, overlap: false,
            }),
        }
    }
    // Overlap pass: a turn overlaps when it intersects a different speaker's turn.
    let snapshot = turns.clone();
    for turn in &mut turns {
        turn.overlap = snapshot.iter().any(|other|
            other.speaker_key != turn.speaker_key
                && other.start_ms < turn.end_ms
                && turn.start_ms < other.end_ms);
    }
    turns.sort_by_key(|turn| (turn.start_ms, turn.end_ms));
    turns
}

/// Persists speakers (reused by key), transcript segments, proposed speaker
/// turns and diarization provenance, then marks the recording completed.
/// Deletes this recording's previously-persisted segments/proposed turns
/// first so a re-run replaces rather than duplicates.
pub(crate) fn persist_attribution(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    runtime_location: &str,
    model_name: &str,
    segments: &[AttributedSegment],
    turns: &[SpeakerTurn],
    duration_ms: i64,
) -> Result<(), String> {
    let timestamp = now();
    let mut mutations = Vec::new();
    for row in genesis_adapter::query(storage, "transcript_segments", &["id"],
        vec![genesis_adapter::eq("transcript_segments", "recording_id", serde_json::json!(recording_id))], 5000)? {
        mutations.push(genesis_adapter::delete("transcript_segments", &genesis_adapter::string(&row, "transcript_segments.id")?));
    }
    for row in genesis_adapter::query(storage, "speaker_turns", &["id", "status"],
        vec![genesis_adapter::eq("speaker_turns", "recording_id", serde_json::json!(recording_id))], 5000)? {
        if row.get("speaker_turns.status").and_then(serde_json::Value::as_str) == Some("proposed") {
            mutations.push(genesis_adapter::delete("speaker_turns", &genesis_adapter::string(&row, "speaker_turns.id")?));
        }
    }

    let provider_id = "fung-desktop-attribution";
    let model_run_id = Uuid::new_v4().to_string();
    mutations.push(genesis_adapter::upsert("model_providers", serde_json::json!({"id": provider_id, "label": "FUNG Desktop attribution", "runtime_location": runtime_location, "kind": "diarization", "enabled": true, "config_json": {}, "created_at": timestamp, "updated_at": timestamp})));
    mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": provider_id, "model_name": model_name, "task_kind": "diarization", "runtime_location": runtime_location, "input_ref": recording_id, "output_ref": format!("speaker-turns:{recording_id}"), "parameters_json": {}, "created_at": timestamp})));

    // Reuse speakers by key (same contract as mobile_diarization_import).
    let existing = genesis_adapter::query(storage, "speakers", &["id", "key", "created_at"],
        vec![genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id))], 500)?;
    let mut key_to_id = std::collections::HashMap::new();
    for row in &existing {
        if let (Some(key), Some(id)) = (
            row.get("speakers.key").and_then(serde_json::Value::as_str),
            row.get("speakers.id").and_then(serde_json::Value::as_str),
        ) { key_to_id.insert(key.to_string(), id.to_string()); }
    }
    let mut ensure_speaker = |key: &str, display_name: &str, confidence: Option<f64>, mutations: &mut Vec<_>| -> String {
        if let Some(id) = key_to_id.get(key) { return id.clone(); }
        let id = Uuid::new_v4().to_string();
        mutations.push(genesis_adapter::upsert("speakers", serde_json::json!({"id": id, "project_id": project_id, "key": key, "display_name": display_name, "confidence": confidence, "created_at": timestamp, "updated_at": timestamp})));
        key_to_id.insert(key.to_string(), id.clone());
        id
    };

    for segment in segments {
        let speaker_id = match (&segment.speaker_key, &segment.display_name) {
            (Some(key), Some(name)) => serde_json::json!(ensure_speaker(key, name, segment.confidence, &mut mutations)),
            _ => serde_json::Value::Null,
        };
        mutations.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": segment.start_ms, "end_ms": segment.end_ms, "text": segment.text, "confidence": segment.confidence, "created_at": timestamp, "updated_at": timestamp})));
    }
    for turn in turns {
        let speaker_id = ensure_speaker(&turn.speaker_key, &turn.display_name, turn.confidence, &mut mutations);
        mutations.push(genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": turn.start_ms, "end_ms": turn.end_ms, "confidence": turn.confidence, "status": "proposed", "model_run_id": model_run_id, "overlap": turn.overlap, "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }
    let recording = genesis_adapter::query(storage, "recordings", &["source", "input_path", "canonical_audio_path", "created_at"],
        vec![genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id))], 1)?
        .into_iter().next().ok_or_else(|| "recording not found".to_string())?;
    mutations.push(genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": genesis_adapter::string(&recording, "recordings.source")?, "input_path": recording.get("recordings.input_path").cloned().unwrap_or(serde_json::Value::Null), "canonical_audio_path": genesis_adapter::string(&recording, "recordings.canonical_audio_path")?, "status": "completed", "duration_ms": duration_ms, "created_at": genesis_adapter::string(&recording, "recordings.created_at")?, "updated_at": timestamp})));
    genesis_adapter::commit_rows(storage, mutations)
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml speaker_merge`
Expected: PASS (3 tests).

- [ ] **Step 5: Wire Path A into the import worker**

Replace the placeholder `run_processing_pipeline` in `zoom_sync.rs`:

```rust
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
```

Until Tasks 7/8 exist, add temporary stubs so this compiles, replaced by those tasks:

```rust
// zoom_sync.rs — replaced in Task 7:
fn run_mixed_audio_path(_ctx: &ImportContext) -> Result<(), String> {
    Err("mixed-audio path not yet implemented".to_string())
}
```

```rust
// lib.rs gets `mod graph_build;` in Task 8. For THIS task create
// src-tauri/src/graph_build.rs containing only the stub below and add
// `mod graph_build;` to lib.rs now:
pub(crate) fn start_graph_build(
    _storage: std::sync::Arc<genesis_block_native::Storage>,
    _project_id: String,
    _recording_id: String,
    _meeting_label: String,
) {}
```

- [ ] **Step 6: Run full test suite + commit**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml`
Expected: PASS.

```bash
git add src-tauri/src/speaker_merge.rs src-tauri/src/zoom_sync.rs src-tauri/src/graph_build.rs src-tauri/src/lib.rs
git commit -m "feat(zoom): per-participant transcription merge with speaker attribution"
```

### Task 7: `diarize.py` + Path B (mixed-audio fallback)

**Files:**
- Create: `scripts/diarize.py`
- Modify: `src-tauri/src/lib.rs` (generalize worker runner)
- Modify: `src-tauri/src/speaker_merge.rs` (`assign_by_overlap`)
- Modify: `src-tauri/src/zoom_sync.rs` (real `run_mixed_audio_path`)

**Interfaces:**
- Consumes: `WhisperRuntime` (python venv path); Task 6 `persist_attribution`.
- Produces: `crate::run_diarization(&WhisperRuntime, file_path, on_progress) -> Result<DiarizeOutput, String>` with `pub(crate) struct DiarizeOutput { duration_ms: i64, turns: Vec<DiarizeTurn> }`, `pub(crate) struct DiarizeTurn { speaker_key: String, display_name: String, start_ms: i64, end_ms: i64, confidence: Option<f64> }` (serde camelCase); `speaker_merge::assign_by_overlap(&[AttributedSegment], &[DiarizeTurn]) -> Vec<AttributedSegment>`.

- [ ] **Step 1: Write `scripts/diarize.py`**

Same worker contract as `transcribe.py` (PROGRESS on stderr, one JSON on stdout):

```python
"""Local speaker-diarization worker for FUNG, backed by pyannote-audio.

Invoked by the Rust backend as a subprocess:

    python diarize.py <audio_path> [--model pyannote/speaker-diarization-3.1]

The pyannote pipeline weights are gated on Hugging Face: the user must
accept the model license and expose a token via FUNG_HF_TOKEN (or HF_TOKEN)
for the FIRST download; afterwards the cached model works offline.
"""

import argparse
import json
import os
import sys


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Diarize an audio file locally.")
    parser.add_argument("audio_path")
    parser.add_argument("--model", default="pyannote/speaker-diarization-3.1")
    args = parser.parse_args()

    def report(pct: float) -> None:
        print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

    report(1)
    try:
        import torch
        from pyannote.audio import Pipeline
    except ImportError as error:
        print(f"MODEL_ACCESS pyannote-audio is not installed: {error}", file=sys.stderr, flush=True)
        return 3

    token = os.environ.get("FUNG_HF_TOKEN") or os.environ.get("HF_TOKEN")
    try:
        pipeline = Pipeline.from_pretrained(args.model, use_auth_token=token)
    except Exception as error:  # gated model, missing token, offline first run
        print(f"MODEL_ACCESS could not load {args.model}: {error}", file=sys.stderr, flush=True)
        return 3

    if torch.cuda.is_available():
        pipeline.to(torch.device("cuda"))

    report(10)
    diarization = pipeline(args.audio_path)
    report(90)

    turns = []
    labels = []
    for segment, _track, label in diarization.itertracks(yield_label=True):
        if label not in labels:
            labels.append(label)
        index = labels.index(label)
        turns.append({
            "speakerKey": f"s:{index}",
            "displayName": f"Speaker {index + 1}",
            "startMs": round(segment.start * 1000),
            "endMs": round(segment.end * 1000),
            "confidence": None,
        })
    duration_ms = max((turn["endMs"] for turn in turns), default=0)

    report(100)
    print(json.dumps({"durationMs": duration_ms, "turns": turns}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

One-time environment setup (document only — done by the user/UAT, not CI):

```bash
D:\FUNG\.venv-whisper\Scripts\pip.exe install pyannote.audio
```

- [ ] **Step 2: Write failing Rust tests (fixture parse + overlap assignment)**

In `src-tauri/src/lib.rs` `#[cfg(test)]` area (or a `mod tests` in `speaker_merge.rs` — put them beside the code they test):

`speaker_merge.rs` tests:

```rust
    #[test]
    fn assign_by_overlap_picks_dominant_turn_and_leaves_gaps_unassigned() {
        let segments = vec![
            AttributedSegment { speaker_key: None, display_name: None, start_ms: 0, end_ms: 2_000, text: "a".into(), confidence: None },
            AttributedSegment { speaker_key: None, display_name: None, start_ms: 2_000, end_ms: 4_000, text: "b".into(), confidence: None },
            AttributedSegment { speaker_key: None, display_name: None, start_ms: 8_000, end_ms: 9_000, text: "c".into(), confidence: None },
        ];
        let turns = vec![
            crate::zoom_sync::DiarizeTurn { speaker_key: "s:0".into(), display_name: "Speaker 1".into(), start_ms: 0, end_ms: 2_500, confidence: None },
            crate::zoom_sync::DiarizeTurn { speaker_key: "s:1".into(), display_name: "Speaker 2".into(), start_ms: 2_500, end_ms: 5_000, confidence: None },
        ];
        let assigned = assign_by_overlap(&segments, &turns);
        assert_eq!(assigned[0].speaker_key.as_deref(), Some("s:0"));
        assert_eq!(assigned[1].speaker_key.as_deref(), Some("s:1")); // 1500ms overlap beats 500ms
        assert_eq!(assigned[2].speaker_key, None); // no overlap → unassigned
    }
```

`zoom_sync.rs` test:

```rust
    #[test]
    fn diarize_output_parses_worker_json() {
        let raw = r#"{"durationMs": 9000, "turns": [{"speakerKey": "s:0", "displayName": "Speaker 1", "startMs": 0, "endMs": 2500, "confidence": null}]}"#;
        let output: DiarizeOutput = serde_json::from_str(raw).unwrap();
        assert_eq!(output.turns.len(), 1);
        assert_eq!(output.turns[0].display_name, "Speaker 1");
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml assign_by_overlap diarize_output`
Expected: FAIL to compile.

- [ ] **Step 4: Implement runner generalization + assignment + Path B**

In `src-tauri/src/lib.rs`, extract the subprocess plumbing of `run_transcription` into a reusable runner (keep `run_transcription` signature; it becomes a thin wrapper):

```rust
/// Runs a python worker from the whisper venv and returns its stdout after a
/// zero exit. `PROGRESS <pct>` stderr lines stream through `on_progress`;
/// other stderr lines are collected into the error message on failure.
pub(crate) fn run_python_worker(
    runtime: &WhisperRuntime,
    script: &std::path::Path,
    args: &[&str],
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<String, String> {
    if !runtime.python.exists() {
        return Err(format!("FUNG Python runtime is missing at {}. Reinstall the FUNG application bundle.", runtime.python.display()));
    }
    if !script.exists() {
        return Err(format!("FUNG worker script is missing at {}. Reinstall the FUNG application bundle.", script.display()));
    }
    let mut command = Command::new(&runtime.python);
    command.arg(script).args(args);
    // (move the existing GPU PATH/CUDA-check block here unchanged, applied for
    // transcribe; for diarize the same PATH extension is harmless)
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .map_err(|err| format!("failed to launch worker: {err}"))?;
    let stderr = child.stderr.take().expect("stderr was piped");
    let stderr_tail = Arc::new(Mutex::new(String::new()));
    let tail = stderr_tail.clone();
    let progress_thread = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("PROGRESS ") {
                if let Ok(pct) = rest.trim().parse::<i64>() { on_progress(pct); }
            } else if let Ok(mut tail) = tail.lock() {
                tail.push_str(&line); tail.push('\n');
            }
        }
    });
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut raw_output = String::new();
    stdout.read_to_string(&mut raw_output).map_err(|err| format!("failed to read worker output: {err}"))?;
    let status = child.wait().map_err(|err| format!("failed to wait for worker: {err}"))?;
    let _ = progress_thread.join();
    if !status.success() {
        let tail = stderr_tail.lock().map(|t| t.clone()).unwrap_or_default();
        return Err(format!("worker exited with {status}: {}", tail.trim()));
    }
    Ok(raw_output)
}
```

`run_transcription` then becomes: profile/CUDA checks (unchanged, kept there), `run_python_worker(runtime, &runtime.script, &[file_path, "--profile", &profile], on_progress)` + `serde_json::from_str::<WhisperOutput>`. Add `run_diarization`:

```rust
pub(crate) fn run_diarization(
    runtime: &WhisperRuntime,
    file_path: &str,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<zoom_sync::DiarizeOutput, String> {
    let script = runtime.script.parent().expect("scripts dir").join("diarize.py");
    let raw = run_python_worker(runtime, &script, &[file_path], on_progress)?;
    serde_json::from_str(raw.trim()).map_err(|err| format!("failed to parse diarization output: {err}"))
}
```

In `zoom_sync.rs` add the output structs:

```rust
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
```

In `speaker_merge.rs` add:

```rust
/// Assigns each transcript segment the diarization turn with the largest
/// time overlap; segments overlapping nothing stay unassigned (speaker null).
pub(crate) fn assign_by_overlap(
    segments: &[AttributedSegment],
    turns: &[crate::zoom_sync::DiarizeTurn],
) -> Vec<AttributedSegment> {
    segments.iter().map(|segment| {
        let best = turns.iter()
            .map(|turn| (turn, (segment.end_ms.min(turn.end_ms) - segment.start_ms.max(turn.start_ms)).max(0)))
            .filter(|(_, overlap)| *overlap > 0)
            .max_by_key(|(_, overlap)| *overlap)
            .map(|(turn, _)| turn);
        AttributedSegment {
            speaker_key: best.map(|turn| turn.speaker_key.clone()),
            display_name: best.map(|turn| turn.display_name.clone()),
            ..segment.clone()
        }
    }).collect()
}
```

Replace the Task 6 stub `run_mixed_audio_path` in `zoom_sync.rs`:

```rust
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
            genesis_adapter::commit_rows(&ctx.storage, vec![genesis_adapter::upsert("job_events", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "job_id": ctx.job_id, "status": "running", "message": format!("diarization unavailable: {message}"), "created_at": timestamp}))])?;
            Ok(())
        }
    }
}
```

- [ ] **Step 5: Run tests to verify pass, commit**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml`
Expected: PASS (all).

```bash
git add scripts/diarize.py src-tauri/src/lib.rs src-tauri/src/speaker_merge.rs src-tauri/src/zoom_sync.rs
git commit -m "feat(zoom): pyannote diarization worker and mixed-audio fallback path"
```

### Task 8: `graph_build.rs` — structural graph + LLM extraction

**Files:**
- Modify: `src-tauri/src/graph_build.rs` (replace Task 6 stub)
- Modify: `src-tauri/src/lib.rs` (register `graph_build_start` command)

**Evidence mapping (from spec §7):** `graph_edges` can only reference `graph_nodes` (FK), so evidence lives in the edge's `provenance_json`: every extracted node connects to the meeting node with an edge whose `provenance_json = {"actor":"ai","modelRunId":...,"evidenceSegmentIds":[...],"confidence":...}` and `epistemic_status = "ai_proposed"`. Structural edges use `epistemic_status = "confirmed"`, `provenance_json = {"actor":"system"}`. Extraction ids are prefixed `gx:{recording_id}:` / `gxe:{recording_id}:` so a re-run can find and delete stale ones (query by project, filter by prefix in Rust — genesis filters are equality-only).

**Interfaces:**
- Consumes: `genesis_adapter`; `model_providers` row `ollama-summary-intent` (`config_json.endpoint`, optional `config_json.model`, default model `llama3.1:8b`); transcript segments of the recording.
- Produces: `start_graph_build(storage: Arc<Storage>, project_id: String, recording_id: String, meeting_label: String)` (spawns thread + `graph.build` job); command `graph_build_start(project_id: String, recording_id: String) -> Job` for manual retry; pure fns `det_node_id(recording_id, kind, label) -> String`, `structural_mutations(...)`, `extraction_mutations(...)`, `stale_extraction_ids(rows: &[serde_json::Value], recording_id: &str) -> Vec<String>`, `parse_extraction(&str) -> Extraction`.

- [ ] **Step 1: Write failing tests**

Replace stub file content, starting with:

```rust
//! Knowledge-graph builder: deterministic structural layer plus best-effort
//! LLM extraction (Topic/Decision/ActionItem/Mention) with evidence links to
//! transcript segments, persisted via genesis_adapter.

#[cfg(test)]
mod tests {
    use super::*;

    const EXTRACTION_FIXTURE: &str = r#"{
      "topics": [{"label": "Q3 roadmap", "evidence": [0, 2], "confidence": 0.8}],
      "decisions": [{"label": "Ship zoom import in August", "evidence": [2], "confidence": 0.7}],
      "actionItems": [{"label": "Boss drafts the release note", "owner": "p:boss", "evidence": [3], "confidence": 0.9}],
      "mentions": [{"label": "GenesisBlockDB", "kind": "project", "evidence": [1], "confidence": 0.6}]
    }"#;

    #[test]
    fn extraction_parses_with_tolerant_defaults() {
        let extraction = parse_extraction(EXTRACTION_FIXTURE).unwrap();
        assert_eq!(extraction.topics.len(), 1);
        assert_eq!(extraction.action_items[0].owner.as_deref(), Some("p:boss"));
        // Missing arrays default to empty instead of failing.
        let sparse = parse_extraction(r#"{"topics": []}"#).unwrap();
        assert!(sparse.decisions.is_empty());
    }

    #[test]
    fn det_node_ids_are_stable_and_recording_scoped() {
        let a = det_node_id("rec-1", "topic", "Q3 roadmap");
        assert_eq!(a, det_node_id("rec-1", "topic", "Q3 roadmap"));
        assert_ne!(a, det_node_id("rec-2", "topic", "Q3 roadmap"));
        assert!(a.starts_with("gx:rec-1:"));
    }

    #[test]
    fn stale_ids_filter_matches_only_this_recordings_extractions() {
        let rows = vec![
            serde_json::json!({"graph_nodes.id": "gx:rec-1:aaaa"}),
            serde_json::json!({"graph_nodes.id": "gx:rec-2:bbbb"}),
            serde_json::json!({"graph_nodes.id": "meeting:rec-1"}),
            serde_json::json!({"graph_nodes.id": "some-note"}),
        ];
        assert_eq!(stale_extraction_ids(&rows, "graph_nodes.id", "gx:rec-1:"), vec!["gx:rec-1:aaaa".to_string()]);
    }

    #[test]
    fn extraction_mutations_carry_evidence_in_edge_provenance() {
        let extraction = parse_extraction(EXTRACTION_FIXTURE).unwrap();
        let segment_ids = vec!["s0".to_string(), "s1".to_string(), "s2".to_string(), "s3".to_string()];
        let mutations = extraction_mutations("p1", "rec-1", "run-1", &extraction, &segment_ids, "t");
        // 4 entities → 4 nodes + 4 edges.
        assert_eq!(mutations.len(), 8);
        let edge = mutations.iter().find_map(|m| {
            (m.table == "graph_edges").then(|| m.values.clone())
        }).unwrap();
        let provenance: serde_json::Value = serde_json::from_str(edge["provenance_json"].as_str().unwrap()).unwrap();
        assert_eq!(provenance["actor"], "ai");
        assert!(provenance["evidenceSegmentIds"].as_array().unwrap().iter().all(|v| v.as_str().unwrap().starts_with('s')));
        assert_eq!(edge["epistemic_status"], "ai_proposed");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml graph_build`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

Above the tests:

```rust
use crate::{genesis_adapter, now};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ExtractedItem {
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) owner: Option<String>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) evidence: Vec<usize>,
    #[serde(default)]
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Extraction {
    #[serde(default)] pub(crate) topics: Vec<ExtractedItem>,
    #[serde(default)] pub(crate) decisions: Vec<ExtractedItem>,
    #[serde(default)] pub(crate) action_items: Vec<ExtractedItem>,
    #[serde(default)] pub(crate) mentions: Vec<ExtractedItem>,
}

pub(crate) fn parse_extraction(raw: &str) -> Result<Extraction, String> {
    serde_json::from_str(raw).map_err(|e| format!("extraction parse failed: {e}"))
}

pub(crate) fn det_node_id(recording_id: &str, kind: &str, label: &str) -> String {
    let digest = Sha256::digest(format!("{kind}\u{1}{}", label.trim().to_lowercase()).as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("gx:{recording_id}:{hex}")
}

/// Ids of previously-extracted rows for this recording (prefix match done in
/// Rust because genesis filters are equality-only). `column` is e.g.
/// "graph_nodes.id"; `prefix` is `gx:{recording_id}:` or `gxe:{recording_id}:`.
pub(crate) fn stale_extraction_ids(rows: &[serde_json::Value], column: &str, prefix: &str) -> Vec<String> {
    rows.iter()
        .filter_map(|row| row.get(column).and_then(serde_json::Value::as_str))
        .filter(|id| id.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// Structural layer: meeting node, speaker nodes, spoke_in + part_of edges.
pub(crate) fn structural_mutations(
    project_id: &str,
    recording_id: &str,
    meeting_label: &str,
    speakers: &[(String, String)], // (speaker_id, display_name)
    timestamp: &str,
) -> Vec<genesis_block_native::RelationalRowMutation> {
    let meeting_node = format!("meeting:{recording_id}");
    let system_provenance = "{\"actor\":\"system\"}";
    let mut mutations = vec![
        genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": meeting_node, "project_id": project_id, "entity_type": "meeting", "entity_id": recording_id, "label": meeting_label, "position_x": 50.0, "position_y": 50.0, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("graph_edges", serde_json::json!({"id": format!("edge:{meeting_node}:part_of"), "project_id": project_id, "source_node_id": meeting_node, "target_node_id": project_id, "predicate": "part_of", "epistemic_status": "confirmed", "provenance_json": system_provenance, "created_at": timestamp, "updated_at": timestamp})),
    ];
    for (speaker_id, display_name) in speakers {
        let speaker_node = format!("speaker:{speaker_id}");
        mutations.push(genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": speaker_node, "project_id": project_id, "entity_type": "speaker", "entity_id": speaker_id, "label": display_name, "position_x": 30.0, "position_y": 70.0, "created_at": timestamp, "updated_at": timestamp})));
        mutations.push(genesis_adapter::upsert("graph_edges", serde_json::json!({"id": format!("edge:{speaker_node}:spoke_in:{recording_id}"), "project_id": project_id, "source_node_id": speaker_node, "target_node_id": meeting_node, "predicate": "spoke_in", "epistemic_status": "confirmed", "provenance_json": system_provenance, "created_at": timestamp, "updated_at": timestamp})));
    }
    mutations
}

pub(crate) fn extraction_mutations(
    project_id: &str,
    recording_id: &str,
    model_run_id: &str,
    extraction: &Extraction,
    segment_ids: &[String],
    timestamp: &str,
) -> Vec<genesis_block_native::RelationalRowMutation> {
    let meeting_node = format!("meeting:{recording_id}");
    let mut mutations = Vec::new();
    let groups: [(&str, &Vec<ExtractedItem>); 4] = [
        ("topic", &extraction.topics),
        ("decision", &extraction.decisions),
        ("action_item", &extraction.action_items),
        ("mention", &extraction.mentions),
    ];
    for (kind, items) in groups {
        for item in items {
            if item.label.trim().is_empty() { continue; }
            let node_id = det_node_id(recording_id, kind, &item.label);
            let evidence: Vec<&str> = item.evidence.iter()
                .filter_map(|index| segment_ids.get(*index).map(String::as_str))
                .collect();
            let provenance = serde_json::json!({
                "actor": "ai",
                "modelRunId": model_run_id,
                "evidenceSegmentIds": evidence,
                "confidence": item.confidence,
                "owner": item.owner,
                "kind": item.kind,
            }).to_string();
            mutations.push(genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": node_id, "project_id": project_id, "entity_type": kind, "entity_id": node_id, "label": item.label, "position_x": 70.0, "position_y": 30.0, "created_at": timestamp, "updated_at": timestamp})));
            mutations.push(genesis_adapter::upsert("graph_edges", serde_json::json!({"id": format!("gxe:{recording_id}:{}", &node_id[node_id.len() - 16..]), "project_id": project_id, "source_node_id": node_id, "target_node_id": meeting_node, "predicate": "extracted_from", "epistemic_status": "ai_proposed", "provenance_json": provenance, "created_at": timestamp, "updated_at": timestamp})));
        }
    }
    mutations
}
```

Then the LLM call + job driver (same file):

```rust
const EXTRACTION_PROMPT_HEADER: &str = r#"You are a meeting-analysis assistant. From the numbered transcript below, extract entities as STRICT JSON with this exact shape (no prose, no markdown):
{"topics":[{"label":"...","evidence":[segment numbers],"confidence":0.0}],
 "decisions":[{"label":"...","evidence":[...],"confidence":0.0}],
 "actionItems":[{"label":"who does what by when","owner":"speaker name or null","evidence":[...],"confidence":0.0}],
 "mentions":[{"label":"...","kind":"person|project|organization|other","evidence":[...],"confidence":0.0}]}
Labels must be in the transcript's language (Thai stays Thai). Evidence lists the segment numbers that support each item. Use [] when a category has nothing.
Transcript:
"#;

fn llm_provider_config(storage: &genesis_block_native::Storage) -> Result<(String, String), String> {
    let row = genesis_adapter::query(storage, "model_providers", &["config_json", "enabled"],
        vec![genesis_adapter::eq("model_providers", "id", serde_json::json!("ollama-summary-intent"))], 1)?
        .into_iter().next().ok_or_else(|| "summary/intent model provider is not configured".to_string())?;
    let config = row.get("model_providers.config_json").and_then(serde_json::Value::as_str)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or(serde_json::Value::Null);
    let endpoint = config.get("endpoint").and_then(serde_json::Value::as_str)
        .unwrap_or("http://127.0.0.1:11434").to_string();
    let model = config.get("model").and_then(serde_json::Value::as_str)
        .unwrap_or("llama3.1:8b").to_string();
    Ok((endpoint, model))
}

fn call_llm(endpoint: &str, model: &str, prompt: &str) -> Result<String, String> {
    #[derive(Deserialize)] struct ChatMessage { content: String }
    #[derive(Deserialize)] struct ChatResponse { message: ChatMessage }
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build().map_err(|e| e.to_string())?
        .post(format!("{endpoint}/api/chat"))
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
            "format": "json",
        }))
        .send().map_err(|e| format!("LLM endpoint unreachable at {endpoint}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("LLM endpoint returned {}", response.status()));
    }
    response.json::<ChatResponse>().map(|r| r.message.content).map_err(|e| e.to_string())
}

/// Spawns the graph.build job for a recording whose transcript is ready.
pub(crate) fn start_graph_build(
    storage: std::sync::Arc<genesis_block_native::Storage>,
    project_id: String,
    recording_id: String,
    meeting_label: String,
) {
    let job_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let seeded = genesis_adapter::commit_rows(&storage, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "graph.build", "status": "running", "progress": 0, "input_refs_json": [recording_id], "output_refs_json": [], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": timestamp, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": "building knowledge graph", "created_at": timestamp})),
    ]);
    if seeded.is_err() { return; }
    std::thread::spawn(move || {
        let outcome = run_graph_build(&storage, &project_id, &recording_id, &meeting_label, &job_id);
        let _ = match outcome {
            Ok(()) => crate::set_job_status(&storage, &job_id, "completed", Some(100), None),
            Err(message) => crate::set_job_status(&storage, &job_id, "failed", None, Some(&message)),
        };
    });
}

fn run_graph_build(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    meeting_label: &str,
    job_id: &str,
) -> Result<(), String> {
    // 1) Structural layer (always succeeds independently of the LLM).
    let mut segment_rows = genesis_adapter::query(storage, "transcript_segments", &["id", "start_ms", "text", "speaker_id"],
        vec![genesis_adapter::eq("transcript_segments", "recording_id", serde_json::json!(recording_id))], 5000)?;
    segment_rows.sort_by_key(|row| row.get("transcript_segments.start_ms").and_then(serde_json::Value::as_i64).unwrap_or(0));
    let speaker_rows = genesis_adapter::query(storage, "speakers", &["id", "display_name"],
        vec![genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id))], 500)?;
    let speakers: Vec<(String, String)> = speaker_rows.iter().filter_map(|row| Some((
        row.get("speakers.id")?.as_str()?.to_string(),
        row.get("speakers.display_name")?.as_str()?.to_string(),
    ))).collect();
    let timestamp = now();
    genesis_adapter::commit_rows(storage, structural_mutations(project_id, recording_id, meeting_label, &speakers, &timestamp))?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(20), None);

    // 2) Replace old extraction for this recording (idempotent re-run).
    let node_rows = genesis_adapter::query(storage, "graph_nodes", &["id"],
        vec![genesis_adapter::eq("graph_nodes", "project_id", serde_json::json!(project_id))], 5000)?;
    let edge_rows = genesis_adapter::query(storage, "graph_edges", &["id"],
        vec![genesis_adapter::eq("graph_edges", "project_id", serde_json::json!(project_id))], 5000)?;
    let mut cleanup = Vec::new();
    for id in stale_extraction_ids(&edge_rows, "graph_edges.id", &format!("gxe:{recording_id}:")) {
        cleanup.push(genesis_adapter::delete("graph_edges", &id));
    }
    for id in stale_extraction_ids(&node_rows, "graph_nodes.id", &format!("gx:{recording_id}:")) {
        cleanup.push(genesis_adapter::delete("graph_nodes", &id));
    }
    if !cleanup.is_empty() { genesis_adapter::commit_rows(storage, cleanup)?; }

    // 3) LLM extraction (best-effort by design, but a failure fails the JOB so
    //    the user can retry — structural graph above is already committed).
    let segment_ids: Vec<String> = segment_rows.iter()
        .filter_map(|row| row.get("transcript_segments.id").and_then(serde_json::Value::as_str).map(str::to_owned))
        .collect();
    let mut prompt = String::from(EXTRACTION_PROMPT_HEADER);
    for (index, row) in segment_rows.iter().enumerate() {
        let text = row.get("transcript_segments.text").and_then(serde_json::Value::as_str).unwrap_or_default();
        prompt.push_str(&format!("[{index}] {text}\n"));
    }
    let (endpoint, model) = llm_provider_config(storage)?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(40), None);
    let raw = call_llm(&endpoint, &model, &prompt)?;
    let extraction = parse_extraction(&raw)?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(80), None);

    let model_run_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let mut mutations = vec![
        genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": "ollama-summary-intent", "model_name": model, "task_kind": "graph_extraction", "runtime_location": "local", "input_ref": recording_id, "output_ref": format!("graph:{recording_id}"), "parameters_json": {"endpoint": endpoint}, "created_at": timestamp})),
    ];
    mutations.extend(extraction_mutations(project_id, recording_id, &model_run_id, &extraction, &segment_ids, &timestamp));
    genesis_adapter::commit_rows(storage, mutations)
}

/// Manual retry surface for a failed/never-run graph build.
#[tauri::command]
pub(crate) fn graph_build_start(
    project_id: String,
    recording_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> crate::AppResult<()> {
    let label = genesis_adapter::query(&state.genesis, "projects", &["name"],
        vec![genesis_adapter::eq("projects", "id", serde_json::json!(project_id))], 1)
        .map_err(crate::AppError::Genesis)?
        .into_iter().next()
        .and_then(|row| row.get("projects.name").and_then(serde_json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "Meeting".to_string());
    start_graph_build(state.genesis.clone(), project_id, recording_id, label);
    Ok(())
}
```

Register `graph_build::graph_build_start,` in `generate_handler!`. Note `extraction_mutations`' edge-id slice `&node_id[node_id.len()-16..]` — node ids end with 16 hex chars, so this is the per-entity hash suffix.

- [ ] **Step 4: Run tests to verify pass, commit**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml graph_build`
Expected: PASS (4 tests). Then full `cargo test` passes.

```bash
git add src-tauri/src/graph_build.rs src-tauri/src/lib.rs
git commit -m "feat(graph): structural + LLM-extracted knowledge graph with evidence provenance"
```

### Task 9: Frontend — TS wrappers + ZoomPanel + App wiring

**Files:**
- Modify: `src/tauri.ts`
- Create: `src/components/ZoomPanel.tsx`
- Create: `src/components/ZoomPanel.css`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: the 6 Rust commands from Tasks 3–5 and 8.
- Produces: `zoomConnect/zoomConnectionStatus/zoomDisconnect/zoomListRecordings/zoomImportRecording` wrappers; `<ZoomPanel onClose={() => void} />` modal.

- [ ] **Step 1: Add wrappers + types to `src/tauri.ts`**

Append:

```ts
export type ZoomConnectionStatus = {
  status: "disconnected" | "connecting" | "connected" | "error";
  accountLabel: string | null;
};

export type ZoomRecordingSummary = {
  uuid: string;
  topic: string;
  startTime: string;
  durationMinutes: number;
  hasParticipantAudio: boolean;
};

const zoomOffline: ZoomConnectionStatus = { status: "disconnected", accountLabel: null };

export async function zoomConnect(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_connect");
}

export async function zoomConnectionStatus(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_connection_status");
}

export async function zoomDisconnect(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_disconnect");
}

export async function zoomListRecordings(): Promise<ZoomRecordingSummary[]> {
  if (!canInvoke()) return [];
  return invoke<ZoomRecordingSummary[]>("zoom_list_recordings");
}

export async function zoomImportRecording(meetingUuid: string): Promise<Job> {
  if (!canInvoke()) throw new Error("Zoom import requires the desktop app.");
  return invoke<Job>("zoom_import_recording", { meetingUuid });
}
```

- [ ] **Step 2: Create `src/components/ZoomPanel.tsx`**

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import {
  zoomConnect,
  zoomConnectionStatus,
  zoomDisconnect,
  zoomImportRecording,
  zoomListRecordings,
  type ZoomConnectionStatus,
  type ZoomRecordingSummary,
} from "../tauri";
import "./ZoomPanel.css";

const STATUS_LABEL: Record<ZoomConnectionStatus["status"], string> = {
  disconnected: "ยังไม่ได้เชื่อมต่อ",
  connecting: "กำลังเชื่อมต่อ… ยืนยันใน browser",
  connected: "เชื่อมต่อแล้ว",
  error: "ต้องเชื่อมต่อใหม่",
};

export function ZoomPanel({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<ZoomConnectionStatus>({ status: "disconnected", accountLabel: null });
  const [recordings, setRecordings] = useState<ZoomRecordingSummary[]>([]);
  const [busyUuid, setBusyUuid] = useState<string | null>(null);
  const [importedUuids, setImportedUuids] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await zoomConnectionStatus());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    pollRef.current = window.setInterval(() => void refreshStatus(), 2000);
    return () => {
      if (pollRef.current !== null) window.clearInterval(pollRef.current);
    };
  }, [refreshStatus]);

  useEffect(() => {
    if (status.status !== "connected") return;
    zoomListRecordings().then(setRecordings).catch((err) => setError(String(err)));
  }, [status.status]);

  const handleConnect = async () => {
    setError(null);
    try {
      setStatus(await zoomConnect());
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDisconnect = async () => {
    setError(null);
    try {
      setStatus(await zoomDisconnect());
      setRecordings([]);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleImport = async (uuid: string) => {
    setBusyUuid(uuid);
    setError(null);
    try {
      await zoomImportRecording(uuid);
      setImportedUuids((prev) => new Set(prev).add(uuid));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusyUuid(null);
    }
  };

  return (
    <div className="zoom-panel-backdrop" role="dialog" aria-label="Zoom import">
      <div className="zoom-panel">
        <header className="zoom-panel-header">
          <h2>Import from Zoom</h2>
          <button type="button" onClick={onClose} aria-label="Close">×</button>
        </header>
        <div className="zoom-panel-status">
          <span data-status={status.status}>{STATUS_LABEL[status.status]}</span>
          {status.accountLabel && <span className="zoom-panel-account">{status.accountLabel}</span>}
          {status.status === "connected" ? (
            <button type="button" onClick={handleDisconnect}>Disconnect</button>
          ) : (
            <button type="button" onClick={handleConnect} disabled={status.status === "connecting"}>
              Connect Zoom
            </button>
          )}
        </div>
        {error && <p className="zoom-panel-error">{error}</p>}
        {status.status === "connected" && (
          <ul className="zoom-panel-list">
            {recordings.length === 0 && <li className="zoom-panel-empty">ไม่พบ cloud recording ใน 30 วันที่ผ่านมา</li>}
            {recordings.map((recording) => (
              <li key={recording.uuid}>
                <div>
                  <strong>{recording.topic}</strong>
                  <span>
                    {new Date(recording.startTime).toLocaleString()} · {recording.durationMinutes} นาที ·{" "}
                    {recording.hasParticipantAudio ? "เสียงแยกรายคน ✓" : "เสียงรวม (แยกผู้พูดด้วย AI)"}
                  </span>
                </div>
                <button
                  type="button"
                  disabled={busyUuid === recording.uuid || importedUuids.has(recording.uuid)}
                  onClick={() => void handleImport(recording.uuid)}
                >
                  {importedUuids.has(recording.uuid) ? "Imported ✓" : busyUuid === recording.uuid ? "Importing…" : "Import"}
                </button>
              </li>
            ))}
          </ul>
        )}
        <p className="zoom-panel-note">
          ไฟล์เสียงและ transcript ประมวลผลและเก็บในเครื่องนี้เท่านั้น ติดตามความคืบหน้าได้จากรายการ Jobs
        </p>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create `src/components/ZoomPanel.css`**

Follow the visual language of `src/components/ExternalAccountPanel.css` (read it first and reuse its backdrop/panel tokens). Baseline:

```css
.zoom-panel-backdrop {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  background: rgba(10, 12, 10, 0.55);
  z-index: 60;
}

.zoom-panel {
  width: min(560px, 92vw);
  max-height: 80vh;
  overflow-y: auto;
  border-radius: 16px;
  padding: 20px;
  background: var(--surface, #101410);
  color: var(--text, #e8ece8);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.45);
}

.zoom-panel-header { display: flex; justify-content: space-between; align-items: center; }
.zoom-panel-status { display: flex; gap: 12px; align-items: center; margin: 12px 0; flex-wrap: wrap; }
.zoom-panel-status [data-status="connected"] { color: #7fd18a; }
.zoom-panel-status [data-status="error"] { color: #e08a8a; }
.zoom-panel-error { color: #e08a8a; font-size: 0.85rem; }
.zoom-panel-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
.zoom-panel-list li { display: flex; justify-content: space-between; gap: 12px; align-items: center; padding: 10px 12px; border-radius: 10px; background: rgba(255, 255, 255, 0.04); }
.zoom-panel-list li div { display: grid; gap: 2px; }
.zoom-panel-list li span { font-size: 0.8rem; opacity: 0.75; }
.zoom-panel-empty { opacity: 0.7; }
.zoom-panel-note { font-size: 0.75rem; opacity: 0.6; margin-top: 14px; }
```

- [ ] **Step 4: Wire into `src/App.tsx`**

1. Add import next to the ExternalAccountPanel import (line 46): `import { ZoomPanel } from "./components/ZoomPanel";`
2. Locate the existing `accountPanelOpen` state declaration (search `accountPanelOpen`) and add beside it: `const [zoomPanelOpen, setZoomPanelOpen] = useState(false);`
3. Next to the ExternalAccountPanel render (line ~933) add: `{zoomPanelOpen && <ZoomPanel onClose={() => setZoomPanelOpen(false)} />}`
4. Find the button/control that sets `setAccountPanelOpen(true)` and add an adjacent trigger with the same classes:

```tsx
<button type="button" className="/* same className as the account trigger */" onClick={() => setZoomPanelOpen(true)}>
  <Cloud size={14} aria-hidden />
  Zoom
</button>
```

(`Cloud` is already imported from lucide-react at `src/App.tsx:8`.)

- [ ] **Step 5: Verify build, commit**

Run: `npm run build`
Expected: tsc + vite succeed with no type errors.

```bash
git add src/tauri.ts src/components/ZoomPanel.tsx src/components/ZoomPanel.css src/App.tsx
git commit -m "feat(zoom): desktop panel for connect, list, and import"
```

### Task 10: Setup doc + final validation

**Files:**
- Create: `docs/Desktop/ZOOM_INTEGRATION_SETUP.md`

**Interfaces:** none (docs + verification only).

- [ ] **Step 1: Write `docs/Desktop/ZOOM_INTEGRATION_SETUP.md`**

```markdown
# Zoom Integration Setup

## 1. Create the Zoom OAuth app (once per distribution)

1. https://marketplace.zoom.us → Develop → Build App → **General App**.
2. App type: **User-managed**. Enable **PKCE**; no client secret is used by FUNG.
3. Redirect URL allow-list must include loopback: `http://127.0.0.1` (Zoom
   allows loopback redirect with any port for PKCE apps).
4. Scopes (least privilege):
   - `cloud_recording:read:list_user_recordings`
   - `cloud_recording:read:recording`
   - `user:read:user`
5. Copy the **Client ID** and set it for the desktop runtime:

    ```powershell
    [Environment]::SetEnvironmentVariable("FUNG_ZOOM_CLIENT_ID", "<client id>", "User")
    ```

## 2. Recommended Zoom account settings

- Settings → Recording → enable **Record a separate audio file of each
  participant** — gives FUNG exact speaker attribution (Path A). Without it
  FUNG falls back to on-device diarization (Path B, anonymous Speaker 1/2/3).

## 3. Local diarization model (Path B only)

1. `D:\FUNG\.venv-whisper\Scripts\pip.exe install pyannote.audio`
2. Accept the model licenses on Hugging Face while signed in:
   - https://huggingface.co/pyannote/speaker-diarization-3.1
   - https://huggingface.co/pyannote/segmentation-3.0
3. Create a read token (https://huggingface.co/settings/tokens) and set
   `FUNG_HF_TOKEN` the same way as above. The token is needed only for the
   first model download; afterwards diarization runs fully offline.

## 4. Knowledge-graph extraction model

`graph.build` calls the local LLM configured on the `ollama-summary-intent`
provider (default endpoint `http://127.0.0.1:11434`, default model
`llama3.1:8b`). Install via Ollama: `ollama pull llama3.1:8b`. Thai-heavy
meetings work better with a Thai-capable model; set `config_json.model` on
the provider row to override.

## 5. Privacy invariants

- OAuth tokens: Windows Credential Manager only (service `FUNG`, entry
  `zoom-oauth`). Never in GenesisBlockDB, logs, or files.
- Audio, transcripts, and graph data never leave this machine.
```

- [ ] **Step 2: Full validation**

Run: `cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml`
Expected: PASS (all suites: genesis_adapter, zoom_sync, speaker_merge, graph_build, existing tests).

Run: `npm run build`
Expected: success.

Run: `npm run test:mobile`
Expected: PASS (unchanged behavior — regression check).

- [ ] **Step 3: Manual UAT checklist (real Zoom account)**

1. Set `FUNG_ZOOM_CLIENT_ID`, launch `npm run desktop`, open Zoom panel → Connect → browser consent → panel shows connected + email.
2. Meeting A (separate audio files ON): import → job runs → transcript shows real participant names; speakers renameable; graph has meeting/speaker/topic nodes.
3. Meeting B (separate audio files OFF): import → Path B → Speaker 1/2/3 labels; if pyannote not installed, transcript still appears and job event notes "diarization unavailable".
4. Re-import Meeting A → rejected with "already imported".
5. Disconnect → token gone from Credential Manager (verify in Windows Credential Manager UI); reconnect works.

- [ ] **Step 4: Commit**

```bash
git add docs/Desktop/ZOOM_INTEGRATION_SETUP.md
git commit -m "docs(zoom): integration setup guide and UAT checklist"
```

