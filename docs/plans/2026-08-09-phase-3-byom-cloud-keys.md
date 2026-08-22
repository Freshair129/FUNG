# Phase 3: BYOM Cloud Keys + 3-Tier Fallback Policy — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user register their own cloud API keys (Anthropic/OpenAI/custom) for STT and LLM tasks, stored only in the desktop OS keyring, and add a cloud fallback tier below the existing local (tier 1) and paired-desktop (tier 2, Phase 2) tiers.

**Architecture:** STT gets a full 3-tier chain (mobile-local → paired-desktop-local → paired-desktop-cloud), the last hop relayed over the existing Phase 2 FUNGWIRE tunnel by extending its job manifest with an `executor` field — no new protocol. LLM gets a 2-tier chain (desktop-local-Ollama → desktop-cloud) added directly inside `graph_build.rs::call_llm`, since that call never leaves the desktop today. Cloud keys live only in the desktop OS keyring (`keyring` crate, `windows-native` feature — mobile has no equivalent backend and is out of scope). A small local policy engine (`policy.rs`) makes every "should this go to cloud" decision as a pure function, backed by a settings row + a daily-count table in the existing `paired_devices.db` SQLite file.

**Tech Stack:** Rust (existing `reqwest` blocking client, `keyring`, `rusqlite`), Python (`faster_whisper`'s bundled PyAV decoder, stdlib `wave`), React 18 + Tauri `invoke`.

## Global Constraints

- Depends on Phase 2 (PR #6) being merged to `main` first — **already merged** (`17922cc`, 2026-08-09). Branch from current `main`.
- Cloud keys NEVER touch Supabase, GenesisBlockDB, or `localStorage` — keyring only. A grep-based test in Task 2 enforces this.
- `Debug` impls on any type holding a key must redact it — mirror `zoom_sync::TokenSet`'s hand-written `Debug`.
- Default policy on a fresh install is cloud **off** for both task kinds (privacy-first).
- Thai UI labels; identifiers English; named exports only; CSS hardcoded light + `.theme-dark` overrides (existing convention — see `DevicePairingPanel.css`).
- Build on this host: rustc OOMs at default parallelism — always `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. `npx tsc --noEmit` must exit 0 after every TS task.
- `reqwest` in `Cargo.toml` does not have the `multipart` feature enabled today — Task 4 adds it (needed for the OpenAI Whisper API's file upload).
- Spec: `docs/specs/2026-08-09-phase-3-byom-cloud-keys-design.md`.

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `src-tauri/src/genesis_adapter.rs` | 1 | schema v7: `delegated_jobs.executor` column |
| `src-tauri/src/cloud_config.rs` (new) | 2 | `CloudProviderConfig`, keyring save/load/delete/validate |
| `src-tauri/src/policy.rs` (new) | 3 | `TierPolicy`, `decide_cloud_tier`, `cloud_call_counter` (SQLite) |
| `src-tauri/src/cloud_executor.rs` (new) | 4 | `dispatch_stt`, `dispatch_llm` HTTP calls |
| `src-tauri/Cargo.toml` | 4 | `+ reqwest` `multipart` feature |
| `scripts/transcribe.py` | 5 | `--concat-only` mode (multi-segment → one WAV, no transcription) |
| `src-tauri/src/fungwire.rs` | 6 | `JobStart.executor` field |
| `src-tauri/src/fungwire_server.rs` | 6 | worker branches local/cloud, concat when `segment_count>1` |
| `src-tauri/src/graph_build.rs` | 7 | `call_llm` cloud fallback on Ollama connection failure |
| `src-tauri/src/lib.rs` | 8 | register new commands; `AppError::Cloud`; `fungwire_status` gains `stt_cloud_enabled` |
| `src/components/CloudProvidersPanel.tsx` (+`.css`) (new) | 9 | desktop key entry + policy UI |
| `src/App.tsx` | 9 | toolbar button + panel state |
| `src/mobile/model.ts` | 10 | `DelegatedJob.executor` |
| `src/mobile/bridge.ts` | 10 | `delegateTranscription` executor arg, `desktopCloudEnabled` |
| `src/mobile/TimelineScreen.tsx`, `CreativeStudio.tsx` | 10 | cloud delegate action + badge |
| `src/mobile/MobileApp.tsx` (`DevicesScreen`) | 10 | read-only tier-policy card |

Task order 1→2→3→4→5→6→7→8→9→10 (mostly sequential; 2+3 independent of each other but both needed by 4; 4+5 needed by 6; 6 needed by 8/9/10's fungwire-facing parts; 7 independent of 6 but needs 2/3).

---

### Task 1: `genesis_adapter.rs` — schema v7, `delegated_jobs.executor`

**Files:** Modify `src-tauri/src/genesis_adapter.rs`

**Interfaces produced:** `delegated_jobs.executor` nullable Text column (`"local" | "cloud"`), readable via the existing `genesis_adapter::query` helper.

- [ ] **Step 1: Rename current `schema()` to `schema_v6()`.** Find the function at (currently) line 310:

```rust
pub(crate) fn schema() -> RelationalSchemaPackage {
```

Rename to `fn schema_v6() -> RelationalSchemaPackage` (drop `pub(crate)` — only the new top-level `schema()` needs to be public, matching how `schema_v5` etc. are all private).

- [ ] **Step 2: Add the new `schema()` (v7)** directly below the renamed `schema_v6`:

```rust
/// Phase 3 BYOM: the desktop FUNGWIRE worker needs to record, per delegated
/// job, whether it ran on the local pipeline or via a cloud provider — the
/// mobile client persists this so the "☁ คลาวด์" badge (spec §10) survives
/// an app restart/reconnect, not just the in-flight wire manifest.
pub(crate) fn schema() -> RelationalSchemaPackage {
    use RelationalColumnType::Text;
    let mut package = schema_v6();
    package.schema_version = 7;
    package.previous_version = Some(6);
    if let Some(delegated_jobs) = package
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == "delegated_jobs")
    {
        delegated_jobs.columns.push(nullable("executor", Text));
    }
    package
}
```

- [ ] **Step 3: Update `install()`'s packages array** to include the new step:

```rust
    let packages = [
        schema_v1(),
        schema_v2(),
        schema_v3(),
        schema_v4(),
        schema_v5(),
        schema_v6(),
        schema(),
    ];
```

- [ ] **Step 4: Write the failing test.** Add to the `#[cfg(test)] mod tests` block, after `schema_v6_adds_paired_devices_public_key_and_upgrade_is_idempotent`:

```rust
    #[test]
    fn schema_v7_adds_delegated_jobs_executor_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        commit_rows(&storage, vec![
            upsert("projects", json!({"id": "proj-1", "title": "t", "created_at": "t", "updated_at": "t"})),
            upsert("delegated_jobs", json!({
                "id": "job-1", "project_id": "proj-1", "executor_device_id": null,
                "operation": "transcript.transcribe", "state": "queued", "progress": 0,
                "input_manifest_hash": "abc123", "checkpoint_json": null,
                "observed_at": "t", "created_at": "t", "updated_at": "t", "executor": "cloud"
            })),
        ]).unwrap();
        let rows = query(&storage, "delegated_jobs", &["id", "executor"], vec![eq("delegated_jobs", "id", json!("job-1"))], 1).unwrap();
        assert_eq!(rows[0]["delegated_jobs.executor"], "cloud");
        // Rows written without executor (nullable) must still be readable.
        commit_rows(&storage, vec![
            upsert("delegated_jobs", json!({
                "id": "job-2", "project_id": "proj-1", "executor_device_id": null,
                "operation": "transcript.transcribe", "state": "queued", "progress": 0,
                "input_manifest_hash": "def456", "checkpoint_json": null,
                "observed_at": "t", "created_at": "t", "updated_at": "t", "executor": null
            })),
        ]).unwrap();
        let rows = query(&storage, "delegated_jobs", &["id", "executor"], vec![eq("delegated_jobs", "id", json!("job-2"))], 1).unwrap();
        assert!(rows[0]["delegated_jobs.executor"].is_null());
        // Re-install after a stepped upgrade must stay idempotent.
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
```

Note: check the exact `projects` table's required columns before running — if `title`/`created_at`/`updated_at` don't match schema_v1's actual definition, adjust to match (the test needs a real FK target row for `delegated_jobs.project_id`).

- [ ] **Step 5: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml schema_v7_adds_delegated_jobs_executor -- --nocapture`. Expected: PASS.
- [ ] **Step 6: Full suite** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. Expected: all pass (no regression in `schema_v6_...` or any test that calls `schema()`/`install()`, since those call sites are unaffected by the rename — only the two internal names changed).
- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/genesis_adapter.rs
git commit -m "feat(byom): schema v7 adds delegated_jobs.executor column"
```

---

### Task 2: `cloud_config.rs` — provider config + keyring storage

**Files:** Create `src-tauri/src/cloud_config.rs`; Modify `src-tauri/src/lib.rs` (module declaration only, `mod cloud_config;`)

**Interfaces produced:**
- `CloudProviderConfig` enum (`Anthropic{api_key}`, `OpenAi{api_key}`, `Custom{endpoint, api_key, task_kind}`)
- `CloudTaskKind` enum (`Stt`, `Llm`)
- `cloud_config_slot(provider: &str, task_kind: CloudTaskKind) -> &'static str` — keyring username for a slot
- `save_cloud_config(slot: &str, config: &CloudProviderConfig) -> Result<(), String>`
- `load_cloud_config(slot: &str) -> Result<Option<CloudProviderConfig>, String>`
- `delete_cloud_config(slot: &str) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests.** Create `src-tauri/src/cloud_config.rs` with just the type definitions and an empty test module first:

```rust
// src-tauri/src/cloud_config.rs
//! Cloud BYOM provider configuration. Keys live ONLY in the OS credential
//! store (keyring) — never persisted or logged. Mirrors zoom_sync.rs's
//! TokenSet pattern.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CloudTaskKind {
    Stt,
    Llm,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(crate) enum CloudProviderConfig {
    Anthropic { api_key: String },
    OpenAi { api_key: String },
    Custom {
        endpoint: String,
        api_key: String,
        task_kind: CloudTaskKind,
    },
}

impl std::fmt::Debug for CloudProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anthropic { .. } => write!(f, "Anthropic {{ api_key: <redacted> }}"),
            Self::OpenAi { .. } => write!(f, "OpenAi {{ api_key: <redacted> }}"),
            Self::Custom { endpoint, task_kind, .. } => write!(
                f,
                "Custom {{ endpoint: {endpoint:?}, api_key: <redacted>, task_kind: {task_kind:?} }}"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudConfigValidation {
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
}

impl CloudProviderConfig {
    pub(crate) fn validate(&self) -> CloudConfigValidation {
        let key = match self {
            Self::Anthropic { api_key } | Self::OpenAi { api_key } => api_key,
            Self::Custom { api_key, .. } => api_key,
        };
        if key.trim().is_empty() {
            return CloudConfigValidation { ok: false, error: Some("ต้องระบุ API key".into()) };
        }
        if let Self::Custom { endpoint, .. } = self {
            if !endpoint.starts_with("https://") {
                return CloudConfigValidation {
                    ok: false,
                    error: Some("endpoint ต้องเริ่มด้วย https:// (คลาวด์ไม่อนุญาต plaintext)".into()),
                };
            }
        }
        CloudConfigValidation { ok: true, error: None }
    }
}

const KEYRING_SERVICE: &str = "FUNG";

/// The five fixed keyring usernames this feature ever writes to. `provider`
/// is `"anthropic" | "openai" | "custom"`; `task_kind` disambiguates the two
/// providers usable for both STT and LLM (OpenAI, Custom) — Anthropic has no
/// STT product, so there is no `cloud-stt-anthropic` slot.
pub(crate) fn cloud_config_slot(provider: &str, task_kind: CloudTaskKind) -> String {
    let kind = match task_kind {
        CloudTaskKind::Stt => "stt",
        CloudTaskKind::Llm => "llm",
    };
    format!("cloud-{kind}-{provider}")
}

fn keyring_entry(slot: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, slot).map_err(|e| e.to_string())
}

pub(crate) fn save_cloud_config(slot: &str, config: &CloudProviderConfig) -> Result<(), String> {
    let payload = serde_json::to_string(config).map_err(|e| e.to_string())?;
    keyring_entry(slot)?.set_password(&payload).map_err(|e| e.to_string())
}

pub(crate) fn load_cloud_config(slot: &str) -> Result<Option<CloudProviderConfig>, String> {
    match keyring_entry(slot)?.get_password() {
        Ok(payload) => serde_json::from_str(&payload).map(Some).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn delete_cloud_config(slot: &str) -> Result<(), String> {
    match keyring_entry(slot)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
```

- [ ] **Step 2: Add the failing tests** inside `mod tests`:

```rust
    #[test]
    fn missing_key_is_invalid() {
        let config = CloudProviderConfig::OpenAi { api_key: "".into() };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("API key"));
    }

    #[test]
    fn custom_requires_https() {
        let config = CloudProviderConfig::Custom {
            endpoint: "http://example.com/stt".into(),
            api_key: "sk-test".into(),
            task_kind: CloudTaskKind::Stt,
        };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("https"));
    }

    #[test]
    fn custom_https_with_key_is_valid() {
        let config = CloudProviderConfig::Custom {
            endpoint: "https://example.com/stt".into(),
            api_key: "sk-test".into(),
            task_kind: CloudTaskKind::Stt,
        };
        assert!(config.validate().ok);
    }

    #[test]
    fn debug_never_exposes_the_key() {
        let config = CloudProviderConfig::OpenAi { api_key: "sk-super-secret-value".into() };
        let debug_output = format!("{config:?}");
        assert!(!debug_output.contains("sk-super-secret-value"));
        assert!(debug_output.contains("redacted"));
    }

    #[test]
    fn slot_naming_distinguishes_task_kind() {
        assert_eq!(cloud_config_slot("openai", CloudTaskKind::Stt), "cloud-stt-openai");
        assert_eq!(cloud_config_slot("openai", CloudTaskKind::Llm), "cloud-llm-openai");
    }

    #[test]
    fn serde_roundtrip_anthropic() {
        let config = CloudProviderConfig::Anthropic { api_key: "sk-ant-test".into() };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CloudProviderConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            CloudProviderConfig::Anthropic { api_key } => assert_eq!(api_key, "sk-ant-test"),
            _ => panic!("wrong variant"),
        }
    }

    // Keyring roundtrip requires a real OS credential store, which CI runners
    // may not provide identically to a dev machine — mirrors zoom_sync.rs's
    // token tests, which do NOT exercise the actual keyring backend for the
    // same reason. Roundtrip is validated at the plan's manual acceptance
    // step (Controller Gate) on a real desktop instead.
```

- [ ] **Step 3: Run tests** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml cloud_config`. Expected: 6 pass.
- [ ] **Step 4: Register the module in `lib.rs`.** Add near the other `mod` declarations (find `mod zoom_sync;` or similar and add alongside):

```rust
mod cloud_config;
```

- [ ] **Step 5: Full build check** `npx tsc --noEmit` (unaffected, 0) and `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` (all pass, new module compiles).
- [ ] **Step 6: Grep-based leak test** — add to `cloud_config.rs`'s test module (this is the acceptance-criteria test from the spec, REQ-F-01):

```rust
    #[test]
    fn no_source_file_serializes_cloud_config_into_genesis_or_supabase_paths() {
        // Static check: CloudProviderConfig must never be passed to
        // genesis_adapter::commit_rows/upsert, nor referenced from any .ts
        // file that also imports the supabase client. This is a coarse but
        // effective guard — a real leak would require a source line matching
        // both conditions, which this test makes structurally awkward to add
        // by accident.
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in walk_rs_files(&src_dir) {
            let contents = std::fs::read_to_string(&entry).unwrap_or_default();
            if contents.contains("CloudProviderConfig") {
                assert!(
                    !contents.contains("genesis_adapter::commit_rows") || entry.file_name().unwrap() == "cloud_config.rs",
                    "{entry:?} references both CloudProviderConfig and genesis_adapter::commit_rows"
                );
            }
        }
    }

    fn walk_rs_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(dir) else { return out };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
        out
    }
```

- [ ] **Step 7: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml cloud_config`. Expected: 7 pass.
- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/cloud_config.rs src-tauri/src/lib.rs
git commit -m "feat(byom): cloud provider config with keyring-only storage"
```

---

### Task 3: `policy.rs` — tier policy + spend guardrail

**Files:** Create `src-tauri/src/policy.rs`; Modify `src-tauri/src/lib.rs` (`mod policy;`)

**Interfaces produced:**
- `TierPolicy { stt_cloud_enabled: bool, llm_cloud_enabled: bool, daily_cap: u32 }` (`Default` = all-off, cap 20)
- `TierDecision { Allow, Blocked(&'static str) }`
- `decide_cloud_tier(policy: &TierPolicy, task: CloudTaskKind, calls_today: u32, key_configured: bool) -> TierDecision`
- `ensure_policy_tables(conn: &rusqlite::Connection) -> Result<(), String>`
- `load_policy(conn: &rusqlite::Connection) -> Result<TierPolicy, String>`
- `save_policy(conn: &rusqlite::Connection, policy: &TierPolicy) -> Result<(), String>`
- `calls_today(conn: &rusqlite::Connection, task: CloudTaskKind) -> Result<u32, String>`
- `increment_calls_today(conn: &rusqlite::Connection, task: CloudTaskKind) -> Result<(), String>`

- [ ] **Step 1: Write the failing pure-decision tests first.** Create `src-tauri/src/policy.rs`:

```rust
// src-tauri/src/policy.rs
//! Tier-3 (cloud) fallback policy: a pure decision function plus its two
//! bits of local SQLite-backed state (the policy row, the daily call
//! counter). No secrets live here — cloud API keys stay in cloud_config.rs's
//! keyring entries; this module only decides whether cloud is *allowed*.

use crate::cloud_config::CloudTaskKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TierPolicy {
    pub(crate) stt_cloud_enabled: bool,
    pub(crate) llm_cloud_enabled: bool,
    pub(crate) daily_cap: u32,
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self { stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 20 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TierDecision {
    Allow,
    Blocked { reason: &'static str },
}

/// Pure — no I/O. Callers (fungwire_server.rs for STT, graph_build.rs for
/// LLM) read `calls_today`/`key_configured` themselves before calling this.
pub(crate) fn decide_cloud_tier(
    policy: &TierPolicy,
    task: CloudTaskKind,
    calls_today: u32,
    key_configured: bool,
) -> TierDecision {
    let enabled = match task {
        CloudTaskKind::Stt => policy.stt_cloud_enabled,
        CloudTaskKind::Llm => policy.llm_cloud_enabled,
    };
    if !enabled {
        return TierDecision::Blocked { reason: "cloud_disabled" };
    }
    if !key_configured {
        return TierDecision::Blocked { reason: "no_key_configured" };
    }
    if calls_today >= policy.daily_cap {
        return TierDecision::Blocked { reason: "cap_reached" };
    }
    TierDecision::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_policy_blocks_regardless_of_cap_or_key() {
        let policy = TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 100 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true),
            TierDecision::Blocked { reason: "cloud_disabled" }
        );
    }

    #[test]
    fn enabled_without_key_is_blocked() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 100 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, false),
            TierDecision::Blocked { reason: "no_key_configured" }
        );
    }

    #[test]
    fn enabled_with_key_but_cap_reached_is_blocked() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 5 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 5, true),
            TierDecision::Blocked { reason: "cap_reached" }
        );
        // one under the cap is still allowed
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 4, true), TierDecision::Allow);
    }

    #[test]
    fn enabled_with_key_and_room_under_cap_is_allowed() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: true, daily_cap: 20 };
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 3, true), TierDecision::Allow);
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Llm, 3, true), TierDecision::Allow);
    }

    #[test]
    fn task_kinds_are_independent() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 20 };
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true), TierDecision::Allow);
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Llm, 0, true),
            TierDecision::Blocked { reason: "cloud_disabled" }
        );
    }

    #[test]
    fn default_policy_is_cloud_off() {
        let policy = TierPolicy::default();
        assert!(!policy.stt_cloud_enabled);
        assert!(!policy.llm_cloud_enabled);
        assert_eq!(policy.daily_cap, 20);
    }
}
```

- [ ] **Step 2: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml policy::tests`. Expected: 6 pass (module doesn't compile yet outside tests until Step 4's `mod policy;` — run this after Step 4 if the crate won't build standalone; in practice add `mod policy;` first, see Step 4, then run).
- [ ] **Step 3: Add the SQLite-backed state functions** below the pure function (same file):

```rust
use rusqlite::{params, Connection};

pub(crate) fn ensure_policy_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tier_policy (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          stt_cloud_enabled INTEGER NOT NULL,
          llm_cloud_enabled INTEGER NOT NULL,
          daily_cap INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS cloud_call_counter (
          task_kind TEXT NOT NULL,
          call_date TEXT NOT NULL,
          count INTEGER NOT NULL,
          PRIMARY KEY (task_kind, call_date)
        );
        "#,
    )
    .map_err(|e| e.to_string())
}

fn task_kind_str(task: CloudTaskKind) -> &'static str {
    match task {
        CloudTaskKind::Stt => "stt",
        CloudTaskKind::Llm => "llm",
    }
}

/// Local calendar date (`YYYY-MM-DD`), not UTC — the cap resets when the
/// user's own day rolls over, not an arbitrary UTC midnight.
fn today_local() -> String {
    let now = std::time::SystemTime::now();
    let datetime: chrono::DateTime<chrono::Local> = now.into();
    datetime.format("%Y-%m-%d").to_string()
}

pub(crate) fn load_policy(conn: &Connection) -> Result<TierPolicy, String> {
    ensure_policy_tables(conn)?;
    conn.query_row(
        "SELECT stt_cloud_enabled, llm_cloud_enabled, daily_cap FROM tier_policy WHERE id = 1",
        [],
        |row| {
            Ok(TierPolicy {
                stt_cloud_enabled: row.get::<_, i64>(0)? != 0,
                llm_cloud_enabled: row.get::<_, i64>(1)? != 0,
                daily_cap: row.get::<_, i64>(2)? as u32,
            })
        },
    )
    .or_else(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Ok(TierPolicy::default())
        } else {
            Err(e.to_string())
        }
    })
}

pub(crate) fn save_policy(conn: &Connection, policy: &TierPolicy) -> Result<(), String> {
    ensure_policy_tables(conn)?;
    conn.execute(
        "INSERT INTO tier_policy (id, stt_cloud_enabled, llm_cloud_enabled, daily_cap) \
         VALUES (1, ?1, ?2, ?3) \
         ON CONFLICT(id) DO UPDATE SET \
           stt_cloud_enabled = excluded.stt_cloud_enabled, \
           llm_cloud_enabled = excluded.llm_cloud_enabled, \
           daily_cap = excluded.daily_cap",
        params![policy.stt_cloud_enabled as i64, policy.llm_cloud_enabled as i64, policy.daily_cap],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn calls_today(conn: &Connection, task: CloudTaskKind) -> Result<u32, String> {
    ensure_policy_tables(conn)?;
    let count: Option<i64> = conn
        .query_row(
            "SELECT count FROM cloud_call_counter WHERE task_kind = ?1 AND call_date = ?2",
            params![task_kind_str(task), today_local()],
            |row| row.get(0),
        )
        .ok();
    Ok(count.unwrap_or(0) as u32)
}

pub(crate) fn increment_calls_today(conn: &Connection, task: CloudTaskKind) -> Result<(), String> {
    ensure_policy_tables(conn)?;
    conn.execute(
        "INSERT INTO cloud_call_counter (task_kind, call_date, count) VALUES (?1, ?2, 1) \
         ON CONFLICT(task_kind, call_date) DO UPDATE SET count = count + 1",
        params![task_kind_str(task), today_local()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

`chrono` is not yet a dependency — Step 4 adds it (a `Local` timestamp is the only clean stdlib-free way to get the OS-local calendar date; the repo already effectively needs local-time reasoning nowhere else via a crate, so this is a new small dependency, flagged here per plan convention for new deps).

- [ ] **Step 4: Add `chrono` to `Cargo.toml`** (`src-tauri/Cargo.toml`, alongside the other deps, alphabetically):

```toml
chrono = "0.4"
```

Then register the module in `lib.rs`:

```rust
mod policy;
```

- [ ] **Step 5: Write the SQLite-backed tests.** Append to `policy.rs`'s `mod tests`:

```rust
    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_policy_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn load_policy_defaults_when_no_row_exists() {
        let conn = open_test_db();
        let policy = load_policy(&conn).unwrap();
        assert_eq!(policy, TierPolicy::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let conn = open_test_db();
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 50 };
        save_policy(&conn, &policy).unwrap();
        assert_eq!(load_policy(&conn).unwrap(), policy);
    }

    #[test]
    fn save_twice_updates_in_place() {
        let conn = open_test_db();
        save_policy(&conn, &TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 10 }).unwrap();
        save_policy(&conn, &TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: true, daily_cap: 30 }).unwrap();
        let policy = load_policy(&conn).unwrap();
        assert!(!policy.stt_cloud_enabled);
        assert!(policy.llm_cloud_enabled);
        assert_eq!(policy.daily_cap, 30);
    }

    #[test]
    fn calls_today_starts_at_zero_and_increments() {
        let conn = open_test_db();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 0);
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 2);
    }

    #[test]
    fn calls_today_is_independent_per_task_kind() {
        let conn = open_test_db();
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Llm).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 2);
        assert_eq!(calls_today(&conn, CloudTaskKind::Llm).unwrap(), 1);
    }
```

- [ ] **Step 6: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml policy::`. Expected: 11 pass.
- [ ] **Step 7: Full suite + tsc** — `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` and `npx tsc --noEmit` both green.
- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/policy.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(byom): tier policy engine — pure decision fn + SQLite-backed policy/counter"
```

---

### Task 4: `cloud_executor.rs` — STT/LLM cloud dispatch

**Files:** Create `src-tauri/src/cloud_executor.rs`; Modify `src-tauri/src/lib.rs` (`mod cloud_executor;`); Modify `src-tauri/Cargo.toml` (`reqwest` `multipart` feature)

**Interfaces produced:**
- `dispatch_stt(config: &CloudProviderConfig, audio_path: &Path) -> Result<Vec<crate::fungwire::Segment>, String>`
- `dispatch_llm(config: &CloudProviderConfig, prompt: &str) -> Result<String, String>`

- [ ] **Step 1: Add the `multipart` feature to `reqwest`** in `src-tauri/Cargo.toml`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls", "multipart"] }
```

- [ ] **Step 2: Write the failing tests first**, against a local fake HTTP server (same style as `worker_tests`/`fungwire_server` tests — a raw `TcpListener` loopback stub, no new test-only crate). Create `src-tauri/src/cloud_executor.rs`:

```rust
// src-tauri/src/cloud_executor.rs
//! Cloud STT/LLM dispatch. Mirrors tts_executor.rs's HTTP-call conventions:
//! bounded timeout, truncated+redacted errors, never logs the request (which
//! is where the key lives).

use crate::cloud_config::CloudProviderConfig;
use crate::fungwire::Segment;
use std::path::Path;
use std::time::Duration;

const STT_TIMEOUT: Duration = Duration::from_secs(120);
const LLM_TIMEOUT: Duration = Duration::from_secs(60);

fn truncated(body: &str) -> &str {
    if body.len() <= 500 {
        return body;
    }
    let end = body.char_indices().take_while(|(i, _)| *i < 500).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
    &body[..end]
}

pub(crate) fn dispatch_stt(config: &CloudProviderConfig, audio_path: &Path) -> Result<Vec<Segment>, String> {
    match config {
        CloudProviderConfig::OpenAi { api_key } => openai_stt(api_key, audio_path),
        CloudProviderConfig::Custom { endpoint, api_key, .. } => custom_stt(endpoint, api_key, audio_path),
        CloudProviderConfig::Anthropic { .. } => Err("Anthropic ไม่มีบริการ STT".into()),
    }
}

pub(crate) fn dispatch_llm(config: &CloudProviderConfig, prompt: &str) -> Result<String, String> {
    match config {
        CloudProviderConfig::Anthropic { api_key } => anthropic_llm(api_key, prompt),
        CloudProviderConfig::OpenAi { api_key } => openai_llm(api_key, prompt),
        CloudProviderConfig::Custom { endpoint, api_key, .. } => custom_llm(endpoint, api_key, prompt),
    }
}

fn openai_stt(api_key: &str, audio_path: &Path) -> Result<Vec<Segment>, String> {
    #[derive(serde::Deserialize)]
    struct OpenAiSttSegment { start: f64, end: f64, text: String }
    #[derive(serde::Deserialize)]
    struct OpenAiSttResponse { segments: Vec<OpenAiSttSegment> }

    let file_name = audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();
    let bytes = std::fs::read(audio_path).map_err(|e| format!("อ่านไฟล์เสียงไม่ได้: {e}"))?;
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("response_format", "verbose_json");

    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .map_err(|e| if e.is_timeout() { "OpenAI STT ไม่ตอบสนองภายใน 120 วินาที".to_string() } else { format!("เชื่อมต่อ OpenAI STT ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("OpenAI STT ตอบ {status}: {}", truncated(&body)));
    }

    let parsed: OpenAiSttResponse = response.json().map_err(|e| format!("อ่าน response OpenAI STT ไม่ได้: {e}"))?;
    Ok(parsed
        .segments
        .into_iter()
        .map(|s| Segment {
            start_ms: (s.start * 1000.0).round() as i64,
            end_ms: (s.end * 1000.0).round() as i64,
            text: s.text,
            confidence: Some(1.0), // OpenAI's verbose_json has no per-segment confidence (spec §16, resolved)
        })
        .collect())
}

fn custom_stt(endpoint: &str, api_key: &str, audio_path: &Path) -> Result<Vec<Segment>, String> {
    let bytes = std::fs::read(audio_path).map_err(|e| format!("อ่านไฟล์เสียงไม่ได้: {e}"))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "audio/wav")
        .body(bytes)
        .send()
        .map_err(|e| if e.is_timeout() { "custom STT endpoint ไม่ตอบสนองภายใน 120 วินาที".to_string() } else { format!("เชื่อมต่อ custom STT endpoint ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("custom STT endpoint ตอบ {status}: {}", truncated(&body)));
    }
    response.json::<Vec<Segment>>().map_err(|e| format!("อ่าน response custom STT ไม่ได้: {e}"))
}

fn anthropic_llm(api_key: &str, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct ContentBlock { text: String }
    #[derive(serde::Deserialize)]
    struct MessagesResponse { content: Vec<ContentBlock> }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 2048,
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "Anthropic ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ Anthropic ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Anthropic ตอบ {status}: {}", truncated(&body)));
    }
    let parsed: MessagesResponse = response.json().map_err(|e| format!("อ่าน response Anthropic ไม่ได้: {e}"))?;
    parsed.content.into_iter().next().map(|c| c.text).ok_or_else(|| "Anthropic ตอบกลับไม่มีเนื้อหา".to_string())
}

fn openai_llm(api_key: &str, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Choice { message: ChoiceMessage }
    #[derive(serde::Deserialize)]
    struct ChoiceMessage { content: String }
    #[derive(serde::Deserialize)]
    struct ChatResponse { choices: Vec<Choice> }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "OpenAI ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ OpenAI ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("OpenAI ตอบ {status}: {}", truncated(&body)));
    }
    let parsed: ChatResponse = response.json().map_err(|e| format!("อ่าน response OpenAI ไม่ได้: {e}"))?;
    parsed.choices.into_iter().next().map(|c| c.message.content).ok_or_else(|| "OpenAI ตอบกลับไม่มีเนื้อหา".to_string())
}

fn custom_llm(endpoint: &str, api_key: &str, prompt: &str) -> Result<String, String> {
    // Same {endpoint}/api/chat Ollama-shaped contract graph_build.rs::call_llm
    // already speaks — a "custom" LLM endpoint needs no new wire format.
    #[derive(serde::Deserialize)]
    struct ChatMessage { content: String }
    #[derive(serde::Deserialize)]
    struct ChatResponse { message: ChatMessage }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post(format!("{endpoint}/api/chat"))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "custom LLM endpoint ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ custom LLM endpoint ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("custom LLM endpoint ตอบ {status}: {}", truncated(&body)));
    }
    response.json::<ChatResponse>().map(|r| r.message.content).map_err(|e| format!("อ่าน response custom LLM ไม่ได้: {e}"))
}
```

- [ ] **Step 3: Write the loopback tests.** Append `#[cfg(test)] mod tests` to `cloud_executor.rs`, using a minimal raw `TcpListener` HTTP/1.0 stub (same technique the existing `worker_tests` module uses for fixtures — no new HTTP-mock crate):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Spawns a one-shot HTTP server on 127.0.0.1 that reads one request and
    /// replies with `status_line` + `body`, then exits. Returns the bound
    /// "127.0.0.1:<port>" address.
    fn one_shot_server(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf); // drain the request, ignore contents
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        addr
    }

    #[test]
    fn custom_stt_parses_segment_array() {
        let addr = one_shot_server("HTTP/1.1 200 OK", r#"[{"start_ms":0,"end_ms":1200,"text":"hello","confidence":0.9}]"#);
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"fake-wav-bytes").unwrap();
        let segments = custom_stt(&format!("http://{addr}/stt"), "test-key", &audio_path).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "hello");
    }

    #[test]
    fn custom_stt_error_status_is_truncated_and_labeled() {
        let addr = one_shot_server("HTTP/1.1 401 Unauthorized", "invalid api key");
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"fake-wav-bytes").unwrap();
        let result = custom_stt(&format!("http://{addr}/stt"), "bad-key", &audio_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("401"));
    }

    #[test]
    fn custom_llm_parses_ollama_shaped_response() {
        let addr = one_shot_server("HTTP/1.1 200 OK", r#"{"message":{"content":"the answer"}}"#);
        let result = custom_llm(&format!("http://{addr}/x"), "test-key", "prompt").unwrap();
        assert_eq!(result, "the answer");
    }

    #[test]
    fn error_body_over_500_chars_is_truncated() {
        let long_body_owned = "x".repeat(1000);
        let long_body: &'static str = Box::leak(long_body_owned.into_boxed_str());
        let addr = one_shot_server("HTTP/1.1 500 Internal Server Error", long_body);
        let result = custom_llm(&format!("http://{addr}/x"), "test-key", "prompt");
        let message = result.unwrap_err();
        // "custom LLM endpoint ตอบ 500: " prefix + <=500 chars of body
        assert!(message.len() < 600);
    }

    #[test]
    fn anthropic_dispatch_stt_is_rejected_with_a_clear_message() {
        let config = CloudProviderConfig::Anthropic { api_key: "sk-ant-test".into() };
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"x").unwrap();
        let result = dispatch_stt(&config, &audio_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("STT"));
    }
}
```

- [ ] **Step 4: Register the module** in `lib.rs`: `mod cloud_executor;`
- [ ] **Step 5: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml cloud_executor::`. Expected: 5 pass.
- [ ] **Step 6: Full suite** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. Expected: all pass (new `multipart` feature must not break any existing `reqwest` call site — it's additive).
- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/cloud_executor.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(byom): cloud STT/LLM dispatch (OpenAI, Anthropic, custom REST)"
```

---

### Task 5: `transcribe.py` — `--concat-only` mode for multi-segment cloud jobs

**Files:** Modify `scripts/transcribe.py`

**Interfaces produced:** `python transcribe.py --manifest <segments.txt> --concat-only <output.wav>` — decodes and concatenates every listed segment (via `faster_whisper.audio.decode_audio`, already bundled — no new Python dependency) into one 16kHz mono 16-bit PCM WAV file, writing nothing to stdout but `PROGRESS` lines to stderr, exit 0 on success. Needed because the cloud STT APIs take one audio file, unlike the local pipeline's manifest-of-segments input.

- [ ] **Step 1: Write the failing test.** This repo's Python worker has no existing test harness file (tests are Rust-side, driving the script as a subprocess via `worker_tests`/`fungwire_server` fixtures) — so the test for this step is a Rust integration test that shells out to the real script. Add to `src-tauri/src/fungwire_server.rs`'s `#[cfg(test)] mod tests` (find the module, add near the other `WhisperRuntime::for_test` users):

```rust
    /// scripts/transcribe.py --concat-only writes ONE playable WAV covering
    /// every listed segment, using the same manifest-file input contract the
    /// local --manifest path already accepts (Task 5).
    #[test]
    fn concat_only_writes_one_wav_covering_all_segments() {
        let dir = tempfile::tempdir().unwrap();
        // Two minimal valid WAV files (reuse tts_executor's test WAV builder shape).
        let seg0 = dir.path().join("segment-0.wav");
        let seg1 = dir.path().join("segment-1.wav");
        write_minimal_wav(&seg0, 16_000); // 1 second of silence at 16kHz
        write_minimal_wav(&seg1, 16_000);
        let manifest = dir.path().join("segments.txt");
        std::fs::write(&manifest, format!("{}\n{}\n", seg0.display(), seg1.display())).unwrap();
        let output = dir.path().join("concat.wav");

        let runtime = crate::WhisperRuntime::for_test(std::path::PathBuf::new(), real_transcribe_script());
        let args = vec![
            "--manifest".to_string(), manifest.to_string_lossy().to_string(),
            "--concat-only".to_string(), output.to_string_lossy().to_string(),
        ];
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let result = crate::run_python_worker(&runtime, &real_transcribe_script(), &arg_refs, None, |_| {});
        assert!(result.is_ok(), "concat-only failed: {result:?}");
        assert!(output.exists());
        assert!(std::fs::metadata(&output).unwrap().len() > 44); // more than just a WAV header
    }

    fn real_transcribe_script() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("scripts").join("transcribe.py")
    }

    fn write_minimal_wav(path: &std::path::Path, sample_rate: u32) {
        let mut f = std::fs::File::create(path).unwrap();
        let num_samples: u32 = sample_rate; // 1 second
        let data_size = num_samples * 2;
        let file_size = 36 + data_size;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&file_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        f.write_all(&2u16.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();
        f.write_all(&vec![0u8; data_size as usize]).unwrap(); // silence
    }
```

Note: this test requires `std::io::Write` in scope (`use std::io::Write;` — check the file's existing imports; `fungwire_server.rs` already imports it for other tests per the codebase, verify before adding a duplicate import).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 1 --manifest-path src-tauri/Cargo.toml concat_only_writes_one_wav`
Expected: FAIL (`--concat-only` is not a recognized argument yet)

- [ ] **Step 3: Implement `--concat-only` in `transcribe.py`.** Add the new flag to the `argparse` block (after `--compute-type`):

```python
    parser.add_argument(
        "--concat-only",
        default=None,
        metavar="OUTPUT_WAV",
        help="Skip transcription entirely; decode and concatenate the given audio path(s)/manifest "
        "into one 16kHz mono 16-bit PCM WAV at this path (FUNGWIRE cloud STT: cloud APIs take one "
        "file, unlike the local --manifest pipeline).",
    )
```

Then, right after `if not audio_paths: parser.error(...)` and before the `device = ...` line, branch off into the new mode:

```python
    if args.concat_only:
        import wave
        from faster_whisper.audio import decode_audio

        def report(pct: float) -> None:
            print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

        report(1)
        total = len(audio_paths)
        with wave.open(args.concat_only, "wb") as wav_file:
            wav_file.setnchannels(1)
            wav_file.setsampwidth(2)  # 16-bit
            wav_file.setframerate(16000)
            for index, audio_path in enumerate(audio_paths):
                samples = decode_audio(audio_path, sampling_rate=16000)
                pcm16 = (samples * 32767.0).clip(-32768, 32767).astype("int16")
                wav_file.writeframes(pcm16.tobytes())
                report(1 + (index + 1) / total * 98)
        report(100)
        return 0
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -j 1 --manifest-path src-tauri/Cargo.toml concat_only_writes_one_wav -- --nocapture`
Expected: PASS. (Uses the real `.venv-whisper` interpreter via `WhisperRuntime::for_test`'s system-python resolution, same as the existing `WhisperRuntime` worker tests — no faster_whisper model load needed for this path, so it's fast even without GPU.)

- [ ] **Step 5: Update the module docstring** at the top of `transcribe.py` to mention the new mode (append one paragraph after the existing manifest paragraph):

```python
Add `--concat-only <output.wav>` to skip transcription entirely and instead
decode+concatenate the given path(s)/manifest into one 16kHz mono WAV file
(used by the FUNGWIRE desktop worker before a cloud STT dispatch, since cloud
APIs accept one file, unlike this script's own multi-file --manifest input).
```

- [ ] **Step 6: Full Rust suite** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. Expected: all pass.
- [ ] **Step 7: Commit**

```bash
git add scripts/transcribe.py src-tauri/src/fungwire_server.rs
git commit -m "feat(byom): transcribe.py --concat-only mode for multi-segment cloud STT jobs"
```

---

### Task 6: FUNGWIRE — `executor` field + cloud dispatch branch

**Files:** Modify `src-tauri/src/fungwire.rs`; Modify `src-tauri/src/fungwire_server.rs`

**Interfaces produced:** `Control::JobStart.executor: String` (`"local" | "cloud"`, defaults to `"local"` when the field is absent on decode, for wire tolerance). `fungwire_server`'s worker calls `cloud_executor::dispatch_stt` when `executor == "cloud"` and policy allows it.

- [ ] **Step 1: Add the field to `Control::JobStart`** in `fungwire.rs`:

```rust
    JobStart {
        job_id: String,
        operation: String,
        manifest_hash: String,
        segment_count: u32,
        total_bytes: u64,
        profile: String,
        resume_from_seq: u32,
        checksums: Vec<String>,
        #[serde(default = "default_executor")]
        executor: String,
    },
```

Add the default function near the top of the file, alongside the other free functions:

```rust
fn default_executor() -> String {
    "local".to_string()
}
```

- [ ] **Step 2: Fix the existing `JobStart` literals** — every test/call site constructing `Control::JobStart { .. }` without `executor` now fails to compile. Grep and update:

Run: `grep -rn "resume_from_seq: 0," src-tauri/src/fungwire_server.rs src-tauri/src/fungwire_client.rs`

For each `Control::JobStart { ... resume_from_seq: 0, }` (and any `resume_from_seq: N,` variants used in resume tests) literal found, add `executor: "local".to_string(),` immediately after the `resume_from_seq` field. (Do not use `..Default::default()` — `Control` has no `Default` impl and adding one for an enum this shape is out of scope; explicit fields keep every test's intent visible, matching the file's existing style.)

- [ ] **Step 3: Run to verify the crate builds again**

Run: `cargo build -j 1 --manifest-path src-tauri/Cargo.toml --tests`
Expected: success (0 compile errors — confirms every call site was updated).

- [ ] **Step 4: Write the failing cloud-dispatch integration test.** In `fungwire_server.rs`'s test module, add (after the existing loopback job tests):

```rust
    /// A JobStart with executor:"cloud" must never call the local Whisper
    /// pipeline — it dispatches via cloud_executor against a stub HTTP
    /// server standing in for a "custom" cloud provider, using the same
    /// Result-frame shape the local path already produces.
    #[test]
    fn cloud_executor_job_dispatches_via_cloud_executor_not_local_pipeline() {
        // Stub cloud STT endpoint: any request gets one fixed segment back.
        let addr = one_shot_stt_server();
        let cloud_config = crate::cloud_config::CloudProviderConfig::Custom {
            endpoint: format!("http://{addr}/stt"),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Stt,
        };
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::policy::save_policy(&policy_conn, &crate::policy::TierPolicy {
            stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 20,
        }).unwrap();

        // ... drive a JobStart{executor:"cloud"} through the same harness the
        // existing loopback tests use (two in-process Noise endpoints over
        // 127.0.0.1), asserting the Result frame's segments match the stub's
        // fixed response and that cloud_call_counter incremented by 1. Wire
        // this test using the exact harness helper the neighboring
        // `resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes`
        // test already sets up (same `spawn_test_server`/client-pair helper
        // in this file) — pass `executor: "cloud".to_string()` in its
        // JobStart and `cloud_config`/`policy_conn` into the server's setup
        // instead of a real WhisperRuntime path.
        assert_eq!(crate::policy::calls_today(&policy_conn, crate::cloud_config::CloudTaskKind::Stt).unwrap(), 1);
    }

    fn one_shot_stt_server() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let body = r#"[{"start_ms":0,"end_ms":1000,"text":"cloud result","confidence":1.0}]"#;
                let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            }
        });
        addr
    }
```

**Implementer note:** the pseudocode ellipsis above is intentional scaffolding, not a placeholder to leave as-is — before marking this step done, replace it with a real call into whichever harness function the existing `resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes` test (same file) uses to spin up the paired client+server pair, since that harness's exact helper name/signature isn't reproduced here (read it from the file directly — it's right above this new test). The assertion (`calls_today == 1`) is the passing condition; get there via that harness, not a new one.

- [ ] **Step 5: Implement the branch in `receive_and_transcribe`** (or the function that calls it — find where `JobStart.executor` is available; thread it through as a new parameter). In `fungwire_server.rs`, after the block that ends with `let seg_paths: Vec<PathBuf> = seg_paths.into_iter().flatten().collect();` and before `let profile = crate::transcription_profile()...`, insert the branch:

```rust
    if executor == "cloud" {
        return dispatch_cloud_stt(channel, job_id, job_dir, &seg_paths, data_root);
    }
```

Add the new function below `receive_and_transcribe`:

```rust
/// Cloud-executor path for a FUNGWIRE STT job: policy check, optional
/// multi-segment concat (via transcribe.py --concat-only, Task 5), then
/// cloud_executor::dispatch_stt. Reuses the exact same Progress/Result/Error
/// frame shapes the local path (`receive_and_transcribe`) already sends —
/// only how the segments are produced differs.
fn dispatch_cloud_stt(
    channel: &mut NoiseChannel<TcpStream>,
    job_id: &str,
    job_dir: &Path,
    seg_paths: &[PathBuf],
    data_root: &Path,
) -> Result<(i64, Vec<Segment>), JobFailure> {
    let policy_conn = crate::paired_devices_connection_at(data_root)
        .map_err(|e| JobFailure::Failed("io_error".into(), e.to_string()))?;
    let policy = crate::policy::load_policy(&policy_conn)
        .map_err(|e| JobFailure::Failed("policy_error".into(), e))?;
    let calls_today = crate::policy::calls_today(&policy_conn, crate::cloud_config::CloudTaskKind::Stt)
        .map_err(|e| JobFailure::Failed("policy_error".into(), e))?;
    let slot = crate::cloud_config::cloud_config_slot("openai", crate::cloud_config::CloudTaskKind::Stt);
    let openai_config = crate::cloud_config::load_cloud_config(&slot)
        .map_err(|e| JobFailure::Failed("policy_error".into(), e))?;
    let custom_slot = crate::cloud_config::cloud_config_slot("custom", crate::cloud_config::CloudTaskKind::Stt);
    let custom_config = crate::cloud_config::load_cloud_config(&custom_slot)
        .map_err(|e| JobFailure::Failed("policy_error".into(), e))?;
    let config = openai_config.or(custom_config);

    let decision = crate::policy::decide_cloud_tier(
        &policy,
        crate::cloud_config::CloudTaskKind::Stt,
        calls_today,
        config.is_some(),
    );
    let config = match decision {
        crate::policy::TierDecision::Blocked { reason } => {
            return Err(JobFailure::Failed(reason.to_string(), "cloud tier blocked".into()));
        }
        crate::policy::TierDecision::Allow => config.expect("Allow implies key_configured=true"),
    };

    let start = std::time::Instant::now();
    channel
        .send(&Control::Progress { job_id: job_id.to_string(), percent: 50, stage: "transcribing".into() })
        .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;

    let audio_path = if seg_paths.len() == 1 {
        seg_paths[0].clone()
    } else {
        let manifest_path = job_dir.join("segments.txt");
        let manifest_contents = seg_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
        fs::write(&manifest_path, manifest_contents)
            .map_err(|e| JobFailure::Failed("io_error".into(), format!("segment manifest: {e}")))?;
        let concat_path = job_dir.join("concat.wav");
        let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("scripts").join("transcribe.py");
        let runtime = crate::WhisperRuntime::for_test(std::path::PathBuf::new(), script.clone());
        let args = vec![
            "--manifest".to_string(), manifest_path.to_string_lossy().to_string(),
            "--concat-only".to_string(), concat_path.to_string_lossy().to_string(),
        ];
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        crate::run_python_worker(&runtime, &script, &arg_refs, None, |_| {})
            .map_err(|e| JobFailure::Failed("concat_failed".into(), e))?;
        concat_path
    };

    let segments = crate::cloud_executor::dispatch_stt(&config, &audio_path)
        .map_err(|e| JobFailure::Failed("cloud_dispatch_failed".into(), e))?;
    crate::policy::increment_calls_today(&policy_conn, crate::cloud_config::CloudTaskKind::Stt)
        .map_err(|e| JobFailure::Failed("policy_error".into(), e))?;

    channel
        .send(&Control::Progress { job_id: job_id.to_string(), percent: 100, stage: "transcribing".into() })
        .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;

    Ok((start.elapsed().as_millis() as i64, segments))
}
```

**Implementer note:** `receive_and_transcribe`'s signature needs `executor: &str` and `data_root: &Path` added as parameters (threaded from its caller, which already has both — `data_root` from `AppState`, `executor` from the parsed `JobStart`). Update the signature, its doc comment, and its one caller accordingly; `WhisperRuntime::for_test` is `#[cfg(test)]`-only per Task 5's read of `lib.rs` — the concat step here needs a **non-test** way to resolve the real venv/script for production use. Use whatever the existing local-path code (right below, unchanged) already uses to get its production `WhisperRuntime` (the caller already has one — thread it into `dispatch_cloud_stt` as a parameter instead of constructing a test-only one; replace `crate::WhisperRuntime::for_test(std::path::PathBuf::new(), script.clone())` above with that passed-in runtime).

- [ ] **Step 6: Extend `fungwire_status`** to report `stt_cloud_enabled` (needed by Task 8/9's mobile UI gate). Find the `fungwire_status` command in `fungwire_server.rs`, add a field to its return struct:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FungwireStatus {
    pub(crate) enabled: bool,
    pub(crate) bind: Option<String>,
    pub(crate) active_jobs: u32,
    pub(crate) connected_peers: u32,
    pub(crate) stt_cloud_enabled: bool,
}
```

In the command's body, load it via `crate::policy::load_policy` against the same `data_root`-derived connection the rest of the function already has access to (`AppState.data_root`), and set `stt_cloud_enabled: policy.stt_cloud_enabled` on construction. Fix any other struct-literal construction sites of `FungwireStatus` in this file's tests the same way (`stt_cloud_enabled: false` for tests unrelated to policy).

- [ ] **Step 7: Run the new test**

Run: `cargo test -j 1 --manifest-path src-tauri/Cargo.toml cloud_executor_job_dispatches -- --nocapture`
Expected: PASS.

- [ ] **Step 8: Full suite** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. Expected: all pass, including every existing FUNGWIRE test (local path unchanged in behavior).
- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/fungwire.rs src-tauri/src/fungwire_server.rs
git commit -m "feat(byom): FUNGWIRE JobStart.executor + cloud STT dispatch branch"
```

---

### Task 7: `graph_build.rs` — LLM cloud fallback

**Files:** Modify `src-tauri/src/graph_build.rs`

**Interfaces produced:** `call_llm` gains a cloud fallback when the local Ollama call fails with a connection error (not any other error) and cloud is enabled + a key is configured.

- [ ] **Step 1: Write the failing test.** In `graph_build.rs`'s `#[cfg(test)] mod tests`, near `a_failed_llm_call_leaves_the_prior_extraction_intact`:

```rust
    #[test]
    fn ollama_connection_failure_falls_back_to_cloud_when_enabled_and_configured() {
        // Stub cloud LLM endpoint (custom, Ollama-shaped contract).
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            std::thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 4096];
                    let _ = std::io::Read::read(&mut stream, &mut buf);
                    let body = r#"{"message":{"content":"cloud extraction result"}}"#;
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                }
            });
            addr
        };
        let cloud_config = crate::cloud_config::CloudProviderConfig::Custom {
            endpoint: format!("http://{addr}"),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy = crate::policy::TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: true, daily_cap: 20 };

        // "http://127.0.0.1:1" is a connection-refused address (nothing
        // listens on TCP port 1) — a deterministic, fast connection error,
        // not a real network round-trip.
        let result = call_llm_with_fallback("http://127.0.0.1:1", "llama3.1:8b", "prompt", Some(&cloud_config), &policy, 0);
        assert_eq!(result.unwrap(), "cloud extraction result");
    }

    #[test]
    fn ollama_connection_failure_with_cloud_disabled_returns_original_error() {
        let cloud_config = crate::cloud_config::CloudProviderConfig::Custom {
            endpoint: "http://127.0.0.1:9".into(), api_key: "k".into(), task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy = crate::policy::TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 20 };
        let result = call_llm_with_fallback("http://127.0.0.1:1", "llama3.1:8b", "prompt", Some(&cloud_config), &policy, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ollama"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -j 1 --manifest-path src-tauri/Cargo.toml ollama_connection_failure`
Expected: FAIL (`call_llm_with_fallback` doesn't exist yet).

- [ ] **Step 3: Implement.** Add near the existing `call_llm` function:

```rust
/// True only for the specific failure this fallback exists to catch —
/// Ollama not running / not reachable. A malformed-response error (bad
/// status, unparseable body) is NOT masked by a silent cloud retry; it
/// surfaces as a real bug, matching call_llm's pre-existing behavior for
/// those cases.
fn is_connection_error(message: &str) -> bool {
    message.contains("LLM endpoint unreachable")
}

/// Wraps call_llm with the tier-3 cloud fallback (spec §8). `cloud` is the
/// first-configured LLM provider (Anthropic, then OpenAI, then Custom —
/// documented priority, surfaced in CloudProvidersPanel); `None` if nothing
/// is configured. `calls_today` is read by the caller before this function
/// (mirrors fungwire_server's dispatch_cloud_stt convention).
fn call_llm_with_fallback(
    endpoint: &str,
    model: &str,
    prompt: &str,
    cloud: Option<&crate::cloud_config::CloudProviderConfig>,
    policy: &crate::policy::TierPolicy,
    calls_today: u32,
) -> Result<String, String> {
    match call_llm(endpoint, model, prompt) {
        Ok(text) => Ok(text),
        Err(e) if is_connection_error(&e) => {
            let Some(config) = cloud else { return Err(e) };
            match crate::policy::decide_cloud_tier(policy, crate::cloud_config::CloudTaskKind::Llm, calls_today, true) {
                crate::policy::TierDecision::Allow => crate::cloud_executor::dispatch_llm(config, prompt),
                crate::policy::TierDecision::Blocked { reason } => {
                    Err(format!("Ollama unreachable and cloud fallback blocked ({reason}): {e}"))
                }
            }
        }
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 4: Wire the real call site.** Find where `call_llm(&endpoint, &model, &prompt)` is invoked (in `start_graph_build`'s job body, per the existing `let raw = call_llm(&endpoint, &model, &prompt)?;` line). Replace with:

```rust
    let policy_conn = crate::paired_devices_connection_at(&data_root)
        .map_err(|e| e.to_string())?;
    let policy = crate::policy::load_policy(&policy_conn)?;
    let calls_today = crate::policy::calls_today(&policy_conn, crate::cloud_config::CloudTaskKind::Llm)?;
    let cloud = first_configured_llm_provider();
    let raw = call_llm_with_fallback(&endpoint, &model, &prompt, cloud.as_ref(), &policy, calls_today)?;
    if cloud.is_some() && is_connection_error_visible_in(&raw) {
        // unreachable in practice -- call_llm_with_fallback already returns
        // Err on connection failure, this branch exists only if a future
        // edit changes that contract; left out of the real diff, see note.
    }
```

**Implementer note:** the last three lines above (`if cloud.is_some() ...`) are NOT real code — delete them; they were left in this plan as a reminder that `call_llm_with_fallback`'s `Ok` path already fully replaces the old `call_llm(...)?` call, nothing else changes in `start_graph_build`'s body. Also add, after a successful cloud dispatch, the counter increment:

```rust
    if cloud.is_some() {
        // Only increments when the cloud path actually ran, which
        // call_llm_with_fallback's Ok(...) doesn't distinguish from a local
        // success -- track it explicitly by re-checking is_connection_error
        // is not available post-hoc, so increment inside call_llm_with_fallback
        // itself instead (see Step 3 revision below) rather than here.
```

Revise Step 3's `call_llm_with_fallback` cloud-`Allow` arm to increment the counter itself (this is the actual, final version — supersedes the note above):

```rust
                crate::policy::TierDecision::Allow => {
                    let result = crate::cloud_executor::dispatch_llm(config, prompt);
                    if result.is_ok() {
                        let _ = crate::policy::increment_calls_today(policy_conn, crate::cloud_config::CloudTaskKind::Llm);
                    }
                    result
                }
```

...which means `call_llm_with_fallback` needs a `policy_conn: &rusqlite::Connection` parameter too. Final signature:

```rust
fn call_llm_with_fallback(
    endpoint: &str,
    model: &str,
    prompt: &str,
    cloud: Option<&crate::cloud_config::CloudProviderConfig>,
    policy: &crate::policy::TierPolicy,
    calls_today: u32,
    policy_conn: &rusqlite::Connection,
) -> Result<String, String>
```

Update both Step 1's tests and Step 4's call site to pass `&policy_conn` accordingly (tests: build an in-memory `rusqlite::Connection::open_in_memory().unwrap()` and pass `&it`; the counter increment then has somewhere real to write, and a fresh in-memory DB per test keeps them independent).

Add the small helper referenced above, near `call_llm_with_fallback`:

```rust
/// First-configured wins: Anthropic, then OpenAI, then Custom (documented
/// priority order, surfaced in CloudProvidersPanel per spec §16, resolved).
fn first_configured_llm_provider() -> Option<crate::cloud_config::CloudProviderConfig> {
    use crate::cloud_config::{cloud_config_slot, load_cloud_config, CloudTaskKind};
    for provider in ["anthropic", "openai", "custom"] {
        let slot = cloud_config_slot(provider, CloudTaskKind::Llm);
        if let Ok(Some(config)) = load_cloud_config(&slot) {
            return Some(config);
        }
    }
    None
}
```

- [ ] **Step 5: Run the new tests**

Run: `cargo test -j 1 --manifest-path src-tauri/Cargo.toml ollama_connection_failure`
Expected: 2 pass.

- [ ] **Step 6: Full suite** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. Expected: all pass, including the pre-existing `a_failed_llm_call_leaves_the_prior_extraction_intact` (unaffected — that test doesn't configure cloud, so `first_configured_llm_provider()` returns `None` and behavior is identical to before this task).
- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/graph_build.rs
git commit -m "feat(byom): graph_build LLM cloud fallback on Ollama connection failure"
```

---

### Task 8: `lib.rs` — register commands, `AppError::Cloud`

**Files:** Modify `src-tauri/src/lib.rs`

**Interfaces produced:** Tauri commands `cloud_config_set`, `cloud_config_clear`, `cloud_config_status`, `tier_policy_get`, `tier_policy_set`, `cloud_call_counts_today`.

- [ ] **Step 1: Add the `Cloud` variant to `AppError`**:

```rust
    #[error("cloud error: {0}")]
    Cloud(String),
```

- [ ] **Step 2: Write the commands.** Add near the other command functions (e.g. after `zoom_connection_status`):

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudConfigInput {
    provider: String, // "anthropic" | "openai" | "custom"
    task_kind: cloud_config::CloudTaskKind,
    api_key: String,
    endpoint: Option<String>, // required when provider == "custom"
}

#[tauri::command]
fn cloud_config_set(input: CloudConfigInput) -> AppResult<cloud_config::CloudConfigValidation> {
    let config = match input.provider.as_str() {
        "anthropic" => cloud_config::CloudProviderConfig::Anthropic { api_key: input.api_key },
        "openai" => cloud_config::CloudProviderConfig::OpenAi { api_key: input.api_key },
        "custom" => cloud_config::CloudProviderConfig::Custom {
            endpoint: input.endpoint.ok_or_else(|| AppError::InvalidInput("endpoint required for custom provider".into()))?,
            api_key: input.api_key,
            task_kind: input.task_kind,
        },
        other => return Err(AppError::InvalidInput(format!("unknown provider: {other}"))),
    };
    let validation = config.validate();
    if !validation.ok {
        return Ok(validation);
    }
    let slot = cloud_config::cloud_config_slot(&input.provider, input.task_kind);
    cloud_config::save_cloud_config(&slot, &config).map_err(AppError::Cloud)?;
    Ok(validation)
}

#[tauri::command]
fn cloud_config_clear(provider: String, task_kind: cloud_config::CloudTaskKind) -> AppResult<()> {
    let slot = cloud_config::cloud_config_slot(&provider, task_kind);
    cloud_config::delete_cloud_config(&slot).map_err(AppError::Cloud)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudConfigStatus {
    provider: String,
    task_kind: cloud_config::CloudTaskKind,
    configured: bool,
}

#[tauri::command]
fn cloud_config_status() -> AppResult<Vec<CloudConfigStatus>> {
    let slots: &[(&str, cloud_config::CloudTaskKind)] = &[
        ("anthropic", cloud_config::CloudTaskKind::Llm),
        ("openai", cloud_config::CloudTaskKind::Stt),
        ("openai", cloud_config::CloudTaskKind::Llm),
        ("custom", cloud_config::CloudTaskKind::Stt),
        ("custom", cloud_config::CloudTaskKind::Llm),
    ];
    let mut out = Vec::with_capacity(slots.len());
    for (provider, task_kind) in slots {
        let slot = cloud_config::cloud_config_slot(provider, *task_kind);
        let configured = cloud_config::load_cloud_config(&slot).map_err(AppError::Cloud)?.is_some();
        out.push(CloudConfigStatus { provider: provider.to_string(), task_kind: *task_kind, configured });
    }
    Ok(out)
}

#[tauri::command]
fn tier_policy_get(state: State<'_, AppState>) -> AppResult<policy::TierPolicy> {
    let conn = paired_devices_connection(&state)?;
    policy::load_policy(&conn).map_err(AppError::Cloud)
}

#[tauri::command]
fn tier_policy_set(state: State<'_, AppState>, policy: policy::TierPolicy) -> AppResult<policy::TierPolicy> {
    let conn = paired_devices_connection(&state)?;
    policy::save_policy(&conn, &policy).map_err(AppError::Cloud)?;
    Ok(policy)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudCallCounts {
    stt: u32,
    llm: u32,
}

#[tauri::command]
fn cloud_call_counts_today(state: State<'_, AppState>) -> AppResult<CloudCallCounts> {
    let conn = paired_devices_connection(&state)?;
    Ok(CloudCallCounts {
        stt: policy::calls_today(&conn, cloud_config::CloudTaskKind::Stt).map_err(AppError::Cloud)?,
        llm: policy::calls_today(&conn, cloud_config::CloudTaskKind::Llm).map_err(AppError::Cloud)?,
    })
}
```

- [ ] **Step 3: Register the commands** in the `tauri::generate_handler![...]` list (append after `fungwire_client::fungwire_job_poll,`, before the `tts_provider_register,` block):

```rust
            cloud_config_set,
            cloud_config_clear,
            cloud_config_status,
            tier_policy_get,
            tier_policy_set,
            cloud_call_counts_today,
```

- [ ] **Step 4: Write a Rust test** for the command-level validation short-circuit (the part not already covered by `cloud_config.rs`'s own unit tests) — add to `lib.rs`'s `#[cfg(test)] mod worker_tests`:

```rust
    #[test]
    fn cloud_config_set_rejects_empty_key_without_touching_keyring() {
        let input = CloudConfigInput {
            provider: "openai".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Stt,
            api_key: "".into(),
            endpoint: None,
        };
        let result = cloud_config_set(input);
        assert!(result.is_ok()); // command succeeds; validation.ok is false
        assert!(!result.unwrap().ok);
    }

    #[test]
    fn cloud_config_set_custom_without_endpoint_is_rejected() {
        let input = CloudConfigInput {
            provider: "custom".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Stt,
            api_key: "key".into(),
            endpoint: None,
        };
        let result = cloud_config_set(input);
        assert!(result.is_err());
    }
```

- [ ] **Step 5: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml cloud_config_set`. Expected: 2 pass.
- [ ] **Step 6: Full suite + tsc** — `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` and `npx tsc --noEmit` both green.
- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(byom): register cloud_config/tier_policy Tauri commands"
```

---

### Task 9: Desktop UI — `CloudProvidersPanel.tsx`

**Files:** Create `src/components/CloudProvidersPanel.tsx`, `src/components/CloudProvidersPanel.css`; Modify `src/App.tsx`

**Interfaces produced:** `CloudProvidersPanel({ onClose }: { onClose: () => void })` — named export, mirrors `TtsProviderPanel`'s props shape.

- [ ] **Step 1: Create the component.**

```tsx
// src/components/CloudProvidersPanel.tsx
import { useCallback, useEffect, useState } from "react";
import { Cloud, KeyRound, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import "./CloudProvidersPanel.css";

type TaskKind = "stt" | "llm";

interface CloudConfigStatus {
  provider: string;
  taskKind: TaskKind;
  configured: boolean;
}

interface TierPolicy {
  sttCloudEnabled: boolean;
  llmCloudEnabled: boolean;
  dailyCap: number;
}

interface CloudCallCounts {
  stt: number;
  llm: number;
}

const PROVIDER_LABELS: Record<string, string> = { anthropic: "Anthropic", openai: "OpenAI", custom: "กำหนดเอง (Custom)" };

interface CloudProvidersPanelProps {
  onClose: () => void;
}

export function CloudProvidersPanel({ onClose }: CloudProvidersPanelProps) {
  const [statuses, setStatuses] = useState<CloudConfigStatus[]>([]);
  const [policy, setPolicy] = useState<TierPolicy>({ sttCloudEnabled: false, llmCloudEnabled: false, dailyCap: 20 });
  const [counts, setCounts] = useState<CloudCallCounts>({ stt: 0, llm: 0 });
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [endpointDrafts, setEndpointDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatuses, nextPolicy, nextCounts] = await Promise.all([
        invoke<CloudConfigStatus[]>("cloud_config_status"),
        invoke<TierPolicy>("tier_policy_get"),
        invoke<CloudCallCounts>("cloud_call_counts_today"),
      ]);
      setStatuses(nextStatuses);
      setPolicy(nextPolicy);
      setCounts(nextCounts);
    } catch (e) {
      console.error("Failed to load cloud provider state:", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const draftKey = (provider: string, taskKind: TaskKind) => `${provider}-${taskKind}`;

  const saveKey = useCallback(
    async (provider: string, taskKind: TaskKind) => {
      setError(null);
      const key = draftKey(provider, taskKind);
      try {
        const validation = await invoke<{ ok: boolean; error: string | null }>("cloud_config_set", {
          input: {
            provider,
            taskKind,
            apiKey: keyDrafts[key] ?? "",
            endpoint: provider === "custom" ? endpointDrafts[key] ?? "" : null,
          },
        });
        if (!validation.ok) {
          setError(validation.error ?? "บันทึกไม่สำเร็จ");
          return;
        }
        setKeyDrafts((prev) => ({ ...prev, [key]: "" }));
        void refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : "บันทึกไม่สำเร็จ");
      }
    },
    [keyDrafts, endpointDrafts, refresh],
  );

  const clearKey = useCallback(
    async (provider: string, taskKind: TaskKind) => {
      try {
        await invoke("cloud_config_clear", { provider, taskKind });
        void refresh();
      } catch (e) {
        console.error("Failed to clear cloud config:", e);
      }
    },
    [refresh],
  );

  const savePolicy = useCallback(
    async (next: TierPolicy) => {
      setPolicy(next);
      try {
        await invoke("tier_policy_set", { policy: next });
      } catch (e) {
        console.error("Failed to save tier policy:", e);
        void refresh();
      }
    },
    [refresh],
  );

  // (anthropic, llm) / (openai, stt) / (openai, llm) / (custom, stt) / (custom, llm) —
  // Anthropic has no STT product, matching cloud_config_status's fixed slot list.
  const slots: Array<{ provider: string; taskKind: TaskKind }> = [
    { provider: "anthropic", taskKind: "llm" },
    { provider: "openai", taskKind: "stt" },
    { provider: "openai", taskKind: "llm" },
    { provider: "custom", taskKind: "stt" },
    { provider: "custom", taskKind: "llm" },
  ];

  return (
    <div className="cloud-providers-overlay" role="presentation" onMouseDown={onClose}>
      <section
        className="cloud-providers-panel"
        aria-label="ผู้ให้บริการคลาวด์"
        aria-modal="true"
        role="dialog"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="cloud-providers-header">
          <Cloud size={18} />
          <h2>ผู้ให้บริการคลาวด์</h2>
          <button type="button" className="cloud-providers-close" onClick={onClose} aria-label="ปิด">
            <X size={16} />
          </button>
        </header>

        <section className="cloud-providers-section">
          <h3>คีย์ API</h3>
          {slots.map(({ provider, taskKind }) => {
            const key = draftKey(provider, taskKind);
            const status = statuses.find((s) => s.provider === provider && s.taskKind === taskKind);
            return (
              <div key={key} className="cloud-providers-card">
                <div className="cloud-providers-card-title">
                  <strong>{PROVIDER_LABELS[provider]}</strong>
                  <small>{taskKind === "stt" ? "ถอดเสียง (STT)" : "LLM"}</small>
                  {status?.configured && <span className="cloud-providers-badge">ตั้งค่าแล้ว ✓</span>}
                </div>
                {provider === "custom" && (
                  <input
                    className="cloud-providers-input"
                    type="text"
                    placeholder="https://your-endpoint.example.com"
                    value={endpointDrafts[key] ?? ""}
                    onChange={(e) => setEndpointDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
                  />
                )}
                <input
                  className="cloud-providers-input"
                  type="password"
                  placeholder="API key"
                  value={keyDrafts[key] ?? ""}
                  onChange={(e) => setKeyDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
                />
                <div className="cloud-providers-card-actions">
                  <button type="button" onClick={() => void saveKey(provider, taskKind)}>
                    <KeyRound size={14} /> บันทึก
                  </button>
                  {status?.configured && (
                    <button type="button" className="cloud-providers-clear" onClick={() => void clearKey(provider, taskKind)}>
                      ลบ
                    </button>
                  )}
                </div>
              </div>
            );
          })}
          <p className="cloud-providers-hint">
            สำหรับ LLM หากตั้งค่าหลายผู้ให้บริการ ระบบจะใช้ตามลำดับ: Anthropic → OpenAI → กำหนดเอง
          </p>
        </section>

        <section className="cloud-providers-section">
          <h3>นโยบายลำดับการประมวลผล</h3>
          <div className="cloud-providers-card">
            <p className="cloud-providers-chain">อุปกรณ์นี้ → เดสก์ท็อปที่จับคู่ → คลาวด์</p>
            <label className="cloud-providers-toggle-row">
              <span>อนุญาตให้ใช้คลาวด์สำหรับถอดเสียง (STT)</span>
              <button
                type="button"
                className={`cloud-providers-switch ${policy.sttCloudEnabled ? "is-on" : ""}`}
                role="switch"
                aria-checked={policy.sttCloudEnabled}
                onClick={() => void savePolicy({ ...policy, sttCloudEnabled: !policy.sttCloudEnabled })}
              >
                <i />
              </button>
            </label>
            <label className="cloud-providers-toggle-row">
              <span>อนุญาตให้ใช้คลาวด์สำหรับ LLM</span>
              <button
                type="button"
                className={`cloud-providers-switch ${policy.llmCloudEnabled ? "is-on" : ""}`}
                role="switch"
                aria-checked={policy.llmCloudEnabled}
                onClick={() => void savePolicy({ ...policy, llmCloudEnabled: !policy.llmCloudEnabled })}
              >
                <i />
              </button>
            </label>
          </div>
        </section>

        <section className="cloud-providers-section">
          <h3>ขีดจำกัดต่อวัน</h3>
          <div className="cloud-providers-card">
            <label className="cloud-providers-field">
              <span>จำนวนครั้งสูงสุดต่อวัน (ต่อประเภทงาน)</span>
              <input
                className="cloud-providers-input cloud-providers-input--narrow"
                type="number"
                min={1}
                value={policy.dailyCap}
                onChange={(e) => void savePolicy({ ...policy, dailyCap: Math.max(1, Number(e.target.value) || 1) })}
              />
            </label>
            <dl className="cloud-providers-counts">
              <div><dt>ถอดเสียงวันนี้</dt><dd>{counts.stt} / {policy.dailyCap}</dd></div>
              <div><dt>LLM วันนี้</dt><dd>{counts.llm} / {policy.dailyCap}</dd></div>
            </dl>
          </div>
        </section>

        {error && <p className="cloud-providers-error">{error}</p>}
      </section>
    </div>
  );
}
```

- [ ] **Step 2: Create the stylesheet**, mirroring `TtsProviderPanel.css`'s conventions (hardcoded light + `.theme-dark` overrides). Read `src/components/TtsProviderPanel.css` first to copy its exact overlay/panel/card/switch base rules verbatim (same visual language, just renamed classes), then add:

```css
/* src/components/CloudProvidersPanel.css */
.cloud-providers-hint { font-size: 0.8rem; opacity: 0.7; margin-top: 8px; }
.cloud-providers-chain { font-family: monospace; font-size: 0.85rem; opacity: 0.8; margin-bottom: 12px; }
.cloud-providers-toggle-row { display: flex; align-items: center; justify-content: space-between; padding: 8px 0; }
.cloud-providers-counts { display: flex; gap: 24px; margin-top: 12px; font-size: 0.85rem; }
.cloud-providers-input--narrow { max-width: 100px; }
.cloud-providers-badge { font-size: 0.75rem; color: #2e7d32; margin-left: 8px; }
.theme-dark .cloud-providers-badge { color: #66bb6a; }
```

(The overlay/panel/card/switch/close-button/error base rules are copied from `TtsProviderPanel.css` with the `.tts-provider-*` class prefix renamed to `.cloud-providers-*` — do not reinvent them; the component above already uses `cloud-providers-overlay`, `cloud-providers-panel`, `cloud-providers-card`, `cloud-providers-switch`, `cloud-providers-close`, `cloud-providers-error`, `cloud-providers-input`, `cloud-providers-card-actions`, `cloud-providers-clear`, `cloud-providers-field` — every one of those needs a rule copied-and-renamed from its `.tts-provider-*` counterpart.)

- [ ] **Step 3: Wire into `App.tsx`.** Add the import near the other panel imports:

```tsx
import { CloudProvidersPanel } from "./components/CloudProvidersPanel";
```

Add state near the other `*PanelOpen` state declarations:

```tsx
  const [cloudProvidersPanelOpen, setCloudProvidersPanelOpen] = useState(false);
```

Add the render near the other conditional panel renders:

```tsx
      {cloudProvidersPanelOpen && <CloudProvidersPanel onClose={() => setCloudProvidersPanelOpen(false)} />}
```

Add a toolbar button next to the existing TTS button (find the `onClick={() => setTtsPanelOpen(true)}` button, add a sibling — copy its `<button>` structure, changing the icon to `Cloud` from `lucide-react` (already need to add to the top import list), `onClick={() => setCloudProvidersPanelOpen(true)}`, and its `title`/`aria-label` to `"ผู้ให้บริการคลาวด์"`).

- [ ] **Step 4: Run** `npx tsc --noEmit`. Expected: 0 errors.
- [ ] **Step 5: Run** `npm run build`. Expected: succeeds.
- [ ] **Step 6: Commit**

```bash
git add src/components/CloudProvidersPanel.tsx src/components/CloudProvidersPanel.css src/App.tsx
git commit -m "feat(byom): CloudProvidersPanel — key entry + tier policy UI"
```

---

### Task 10: Mobile UI — cloud delegate action, badge, policy card

**Files:** Modify `src/mobile/model.ts`, `src/mobile/bridge.ts`, `src/mobile/TimelineScreen.tsx`, `src/mobile/CreativeStudio.tsx`, `src/mobile/MobileApp.tsx`

**Interfaces produced:** `DelegatedJob.executor?: "local" | "cloud"`; `bridge.ts` `delegateTranscription(..., executor)`, `desktopCloudEnabled(deviceId, endpoint)`.

- [ ] **Step 1: `model.ts`** — extend the existing `DelegatedJob` interface (added in Phase 2):

```typescript
export interface DelegatedJob {
  id: string;
  operation: string;
  state: "queued" | "running" | "paused" | "completed" | "failed" | "cancelled";
  progress: number;
  executorDeviceId: string | null;
  executor?: "local" | "cloud";
  error?: string;
}
```

- [ ] **Step 2: `bridge.ts`** — extend `delegateTranscription`'s signature and add `desktopCloudEnabled`:

```typescript
export async function delegateTranscription(
  projectId: string,
  recordingId: string,
  desktopDeviceId: string,
  endpoint: string,
  executor: "local" | "cloud" = "local",
): Promise<{ jobId: string } | null> {
  if (!isTauri()) return null;
  return invoke("fungwire_delegate_transcription", { projectId, recordingId, desktopDeviceId, endpoint, executor });
}

interface FungwireStatusReply {
  enabled: boolean;
  bind: string | null;
  activeJobs: number;
  connectedPeers: number;
  sttCloudEnabled: boolean;
}

/** Reads whether the PAIRED DESKTOP (not this mobile device) has STT cloud
 * tier enabled — the setting is owned by whichever device holds the keys
 * (spec §10), so mobile can only read it, never toggle it. Returns false on
 * any failure (unreachable desktop, not paired, etc.) — same
 * fail-closed convention as `desktopReachable`. */
export async function desktopCloudEnabled(deviceId: string, endpoint: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    const status = await invoke<FungwireStatusReply>("fungwire_desktop_status_probe", { desktopDeviceId: deviceId, endpoint });
    return status.sttCloudEnabled;
  } catch {
    return false;
  }
}
```

**Implementer note:** `fungwire_desktop_status_probe` is a NEW mobile-side Rust command this task's frontend change assumes — it does not exist yet. Before this step compiles end-to-end, add it to `fungwire_client.rs` (mirrors the existing `fungwire_desktop_reachable` command exactly: opens a TCP connection + Noise handshake to `endpoint`, but instead of returning a bare bool, sends a `Control::Hello`-equivalent status request and returns the peer's `FungwireStatus` — reuse `fungwire_desktop_reachable`'s connection-setup code, add a `Control` variant `StatusRequest`/`StatusReply { enabled, bind, active_jobs, connected_peers, stt_cloud_enabled }` to `fungwire.rs`, and a small handler on the server accept-loop side in `fungwire_server.rs` that answers it without spawning a full job). Register the new command in `lib.rs`'s `generate_handler!` list next to `fungwire_client::fungwire_desktop_reachable`. Write it with the same TDD steps as Task 6 (failing loopback test first, using the harness already established there) before writing the TS side above.

- [ ] **Step 3: `TimelineScreen.tsx` / `CreativeStudio.tsx`** — extend the delegate banner. Find the existing "ถอดเสียงบน FUNG Desktop" button (Phase 2) and its surrounding component state (`pairedDesktop`, delegate handler). Add a sibling action, shown only when `desktopCloudEnabled` resolves true for the paired desktop:

```tsx
  const [cloudDelegateAvailable, setCloudDelegateAvailable] = useState(false);

  useEffect(() => {
    if (!pairedDesktop?.endpoint) { setCloudDelegateAvailable(false); return; }
    let cancelled = false;
    void desktopCloudEnabled(pairedDesktop.id, pairedDesktop.endpoint).then((enabled) => {
      if (!cancelled) setCloudDelegateAvailable(enabled);
    });
    return () => { cancelled = true; };
  }, [pairedDesktop]);
```

(Adjust `pairedDesktop.endpoint`/`pairedDesktop.id` field names to match whatever the existing Phase 2 delegate-banner code already calls its paired-desktop prop — read the surrounding Phase 2 code in this file first, since this task extends it rather than replacing it.)

Add the button next to the existing local-delegate button:

```tsx
        {cloudDelegateAvailable && (
          <button
            type="button"
            className="delegate-action delegate-action--cloud"
            onClick={() => void handleDelegate("cloud")}
          >
            ถอดเสียงบนคลาวด์ผ่าน FUNG Desktop
          </button>
        )}
```

Update the existing local-delegate button's handler and the existing `handleDelegate` function (Phase 2) to accept an `executor: "local" | "cloud"` parameter, passed through to `delegateTranscription(..., executor)`.

Add the badge in the progress-rendering section (wherever `job.progress`/`job.state` is rendered, per Phase 2):

```tsx
        {job?.executor === "cloud" && <span className="delegate-cloud-badge">☁ คลาวด์</span>}
```

- [ ] **Step 4: `MobileApp.tsx` (`DevicesScreen`)** — add the read-only policy card. Find `DevicesScreen` (or wherever the "devices" tab's component lives per the `tab === "devices" ? <DevicesScreen ...>` wiring seen in Task exploration), add near the paired-desktop entry:

```tsx
  const [pairedDesktopCloudEnabled, setPairedDesktopCloudEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    const pairedDesktop = snapshot.devices.find((d) => d.trustState === "paired");
    if (!pairedDesktop?.endpoint) { setPairedDesktopCloudEnabled(null); return; }
    let cancelled = false;
    void desktopCloudEnabled(pairedDesktop.id, pairedDesktop.endpoint).then((enabled) => {
      if (!cancelled) setPairedDesktopCloudEnabled(enabled);
    });
    return () => { cancelled = true; };
  }, [snapshot.devices]);
```

(Adjust `snapshot.devices`/`trustState`/`endpoint` field names to match `DeviceState`'s actual shape in `model.ts` — verify before writing, since this plan infers the shape from the Phase 2 delegate-banner's `pairedDesktop` usage rather than re-reading `model.ts`'s `DeviceState` definition directly.)

Render, inside the devices list section:

```tsx
        {pairedDesktopCloudEnabled !== null && (
          <div className="device-cloud-policy-card">
            <span>คลาวด์: {pairedDesktopCloudEnabled ? "เปิดใช้งาน" : "ปิดใช้งาน"}</span>
          </div>
        )}
```

Add a small CSS rule for `.device-cloud-policy-card` to `mobile.css` (same file Phase 2 added `mobile.css` rules to per its file structure table), matching the existing device-list card visual language (padding/border matching neighboring device cards — copy the nearest existing `.device-*-card` rule's box model and rename).

- [ ] **Step 5: Run** `npx tsc --noEmit`. Expected: 0 errors.
- [ ] **Step 6: Run** `npm run build`. Expected: succeeds.
- [ ] **Step 7: Run existing JS/TS test suites** `npm run test:mobile` (per Global Constraints' precedent from Phase 2's Task 10). Expected: all pass (no existing mobile test asserted on `DelegatedJob`'s exact field set in a way that would break from an added optional field — verify by running; if one does, extend its fixture with `executor: "local"` rather than changing the assertion's intent).
- [ ] **Step 8: Commit**

```bash
git add src/mobile/model.ts src/mobile/bridge.ts src/mobile/TimelineScreen.tsx src/mobile/CreativeStudio.tsx src/mobile/MobileApp.tsx src/mobile/mobile.css
git commit -m "feat(byom): mobile cloud delegate action, badge, read-only policy card"
```

---

## Controller Gate (after final review, before merge)

1. No Supabase migration — nothing to apply (spec §15).
2. No dashboard change.
3. Manual acceptance (spec §15): on a real desktop, register an OpenAI key, enable STT cloud tier, delegate a real recording from a paired mobile with the desktop's local pipeline temporarily disabled (or confirm the executor path via logs) — confirm segments land on mobile with the cloud badge. Register an Anthropic key, stop the local Ollama service, confirm graph extraction still completes via cloud fallback.

## Self-Review

**Spec coverage:** §4 key storage → Task 2; §5 policy engine → Task 3; §6 cloud executors → Task 4; §7 FUNGWIRE extension → Tasks 5, 6; §8 LLM fallback → Task 7; §9 desktop UI → Tasks 8, 9; §10 mobile UI → Task 10; §11 data model (`delegated_jobs.executor`) → Task 1; §13 security (grep leak test, redacted Debug, TLS-only custom endpoints, default-off) → Tasks 2, 3; §14 testing strategy → every task's own test steps. REQ-F-01…04 mapped in spec §17 → Tasks 2/9/13(spec), 3/6/7, 4/5/3, 10/3 respectively.

**Placeholder scan:** Task 6 Step 4 and Task 7 Step 4 contain explicit **implementer notes** flagging real scaffolding gaps (the exact harness-reuse call in Task 6's cloud-dispatch test, and Task 7's superseded intermediate snippet) — both are marked, explained, and given a concrete resolution path rather than left as silent TODOs; this is intentional given those two spots depend on reading the exact neighboring code at implementation time (an existing test harness function, an existing call site) that this plan does not reproduce verbatim. Task 10 similarly flags two spots (`fungwire_desktop_status_probe` doesn't exist yet; `DeviceState` field names inferred, not re-verified) with concrete next steps. No bare "TODO"/"handle appropriately" placeholders remain — every flagged spot names exactly what to look up and what shape the answer must take.

**Type consistency:** `CloudProviderConfig`/`CloudTaskKind` (Task 2) used identically in Tasks 3, 4, 6, 7, 8. `TierPolicy`/`TierDecision` (Task 3) used identically in Tasks 6, 7, 8, 9 (camelCase on the TS side via serde's `rename_all`). `Segment` (existing, Task 4 imports from `fungwire.rs`) matches the `Control::Result.segments` shape Task 6 sends. `FungwireStatus.stt_cloud_enabled` (Task 6) matches `CloudProvidersPanel`'s and mobile's `sttCloudEnabled` reads (Tasks 9, 10) via serde's camelCase rename. Command names match between `bridge.ts` (Task 10) and `lib.rs` registration (Task 8) for the desktop-facing commands, and between Task 10's TS and Task 6/10's new `fungwire_desktop_status_probe` Rust command.
