# Phase 3: BYOM Cloud Keys + 3-Tier Fallback Policy — Design Spec

| Field | Value |
|---|---|
| Date | 2026-08-09 |
| Status | approved 2026-08-09 |
| Sub-project | Phase 3 (Sub-project F) — master plan REQ-F-01…04 |
| Depends on | Phase 2 (FUNGWIRE tunnel + job delegation), PR #6 — merged 2026-08-09 |
| Master plan | `2026-08-09-fung-master-implementation-plan.md` Phase 3 |

## 1. Overview & Scope

**Goal:** Let a user register their own cloud API keys (Anthropic/OpenAI/custom) for speech-to-text and LLM tasks, stored only in the desktop OS keyring, and add a third fallback tier — cloud — below the local (tier 1) and paired-desktop (tier 2, Phase 2) tiers already in place. Privacy default is cloud **off**.

**In scope:**
- Cloud key registration UI (desktop only) and OS-keyring storage, mirroring `zoom_sync.rs`'s token pattern.
- STT 3-tier chain: mobile-local (`admit_task`) → paired-desktop-local (Phase 2 FUNGWIRE) → paired-desktop-cloud (new, relayed over the same FUNGWIRE channel).
- LLM 2-tier chain: desktop-local-Ollama (existing `graph_build.rs`) → desktop-cloud (new, direct — no FUNGWIRE involved, since this call never leaves the desktop today).
- Per-task-kind (STT, LLM) on/off toggle for the cloud tier, plus a daily call-count cap per task kind.
- Cloud executors: OpenAI Whisper API + custom REST for STT; Anthropic Messages API + OpenAI Chat Completions + custom REST for LLM.
- Desktop UI: new standalone `CloudProvidersPanel.tsx` (own toolbar button, mirrors `TtsProviderPanel.tsx`).
- Mobile UI: read-only tier-policy status section on the existing `DevicesScreen` ("อุปกรณ์" tab); cloud-tier badge on the existing Phase 2 delegate/progress UI when a job actually used cloud.

**Out of scope (parked):**
- Mobile-held cloud keys / mobile-initiated cloud calls independent of a paired desktop — mobile has no OS-keyring backend today (`keyring` crate is `windows-native`-only in `Cargo.toml`); adding Android Keystore support is a larger, separate effort. A mobile user with no paired, reachable desktop has no tier-3 fallback in this phase.
- TTS in the tier-policy engine — BYOM TTS (`tts_config.rs`/`tts_executor.rs`) remains its own separate system, untouched. (Resolves an inconsistency in the master plan: REQ-F-02 lists TTS as a task kind, REQ-F-03's cloud executor only covers STT/LLM; this spec follows REQ-F-03.)
- On-device mobile LLM (wiring `on_device_ai.rs`'s `AiTaskKind::Llm` to a real llama.cpp call) — nothing today calls it; building it just to make LLM's tier count match STT's is out of scope.
- Full tier reordering / per-tier disable — the chain order is fixed (local → desktop → cloud); the only user-configurable knob is whether tier 3 is allowed at all, per task kind.
- Estimated-dollar spend caps — the cap is a simple daily request count, not a cost estimate (no provider price table to maintain/go stale).
- FUNGWIRE cloud relay / NAT traversal — parked since Phase 2 (master plan §11); cloud tier here still requires mobile+desktop to be on the same LAN and paired, same as tier 2.

## 2. Locked Decisions

| Question | Decision | Date |
|---|---|---|
| Where do cloud keys live | Desktop OS keyring only, never Supabase, never mobile | 2026-08-09 |
| How mobile reaches cloud tier | Relayed over the existing Phase 2 FUNGWIRE tunnel — desktop executes the cloud call with its own key | 2026-08-09 |
| Task kinds in scope | STT + LLM only; TTS excluded (stays on its existing separate BYOM system) | 2026-08-09 |
| STT cloud providers | OpenAI Whisper API + custom REST | 2026-08-09 |
| LLM cloud providers | Anthropic Messages API + OpenAI Chat Completions + custom REST | 2026-08-09 |
| Policy configurability | Fixed 3-tier order; per-task-kind on/off toggle for tier 3 only, plus provider choice | 2026-08-09 |
| Spend guardrail | Daily request-count cap per task kind (not a dollar estimate) | 2026-08-09 |
| Desktop UI placement | New standalone `CloudProvidersPanel`, own toolbar button | 2026-08-09 |
| LLM tier shape | 2 effective tiers (local Ollama → cloud) — no FUNGWIRE involvement, matches what `graph_build.rs` already does today | 2026-08-09 |
| STT tier-3 delivery | Reuse the Phase 2 `delegated_jobs`/FUNGWIRE job protocol; add an `executor: "local" \| "cloud"` field rather than a parallel job system | 2026-08-09 |

## 3. Architecture Overview

```
┌───────────────────────── Mobile (Tauri, Android) ─────────────────────────┐
│ React: TimelineScreen delegate banner — adds "ถอดเสียงบนคลาวด์ผ่าน FUNG    │
│   Desktop" alongside the existing Phase 2 "ถอดเสียงบน FUNG Desktop"        │
│ React: DevicesScreen — read-only tier-policy status section                │
│ bridge.ts: delegateTranscription(..., executor) — extended, not replaced   │
│ Rust: fungwire_client.rs — unchanged wire protocol, new executor field     │
└───────────┬─────────────────────────────────────────────────────────────────┘
            │  existing Noise-encrypted FUNGWIRE channel (Phase 2, unchanged)
            ▼
┌───────────┴───────────────────── Desktop (Tauri, Windows) ─────────────────┐
│ Rust: fungwire_server.rs — worker branches on job.executor:                │
│   "local" → existing transcribe.py path (Phase 2, unchanged)               │
│   "cloud" → cloud_executor::dispatch_stt (NEW)                             │
│ Rust: policy.rs (NEW) — pure decision fn: tier availability × cap × toggle │
│ Rust: cloud_config.rs (NEW) — keyring-backed provider config, mirrors      │
│   tts_config.rs                                                            │
│ Rust: cloud_executor.rs (NEW) — dispatch_stt / dispatch_llm, mirrors       │
│   tts_executor.rs                                                          │
│ Rust: graph_build.rs — call_llm gains a cloud fallback on Ollama           │
│   connection failure (existing local-Ollama call is unconditional today)  │
│ React: CloudProvidersPanel.tsx (NEW) — key entry, tier-3 toggles, cap,     │
│   today's call count                                                       │
│ SQLite (desktop, existing WAL DB): + tier_policy, + cloud_call_counter     │
└──────────────────────────────────────────────────────────────────────────┘
```

**No Supabase changes.** Cloud keys and policy are local-only state; nothing here needs a migration.

## 4. Cloud Key Storage

### 4.1 `cloud_config.rs` (new, mirrors `tts_config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(crate) enum CloudProviderConfig {
    Anthropic { api_key: String },
    OpenAi { api_key: String },
    Custom { endpoint: String, api_key: String, task_kind: CloudTaskKind },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CloudTaskKind { Stt, Llm }
```

Validation (`validate()`, same shape as `TtsProviderConfig::validate`): `api_key` non-empty; `Custom.endpoint` starts with `https://` (cloud calls are never plaintext, unlike the LAN-only `is_private_ip` allowance in `tts_config.rs`); `Debug` never includes `api_key` (hand-written impl, redacted — same pattern as `zoom_sync::TokenSet`).

### 4.2 Storage

One keyring entry per provider slot (`FUNG` service, users `cloud-stt-openai`, `cloud-stt-custom`, `cloud-llm-anthropic`, `cloud-llm-openai`, `cloud-llm-custom`), each holding the serialized `CloudProviderConfig` JSON — same `save_/load_/delete_` triplet as `zoom_sync.rs::save_tokens`. Keeping providers as separate entries (rather than one blob) means deleting one key never risks corrupting another, and matches "which providers are configured" being a simple existence check per entry.

## 5. Policy Engine

### 5.1 `policy.rs` (new)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TierPolicy {
    pub(crate) stt_cloud_enabled: bool,
    pub(crate) llm_cloud_enabled: bool,
    pub(crate) daily_cap: u32, // applies per task kind
}
```

Stored in the desktop's existing SQLite WAL DB as a single-row settings table `tier_policy` (not a secret — no keyring). Default row on first run: `{ stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 20 }` — privacy-first default per REQ-F-04.

### 5.2 Decision function

```rust
pub(crate) enum TierDecision { Allow, Blocked(&'static str) } // reasons: "cloud_disabled" | "cap_reached" | "no_key_configured"

pub(crate) fn decide_cloud_tier(
    policy: &TierPolicy,
    task: CloudTaskKind,
    calls_today: u32,
    key_configured: bool,
) -> TierDecision
```

Pure function, no I/O — unit-testable as a plain matrix (enabled × cap-state × key-configured → decision). `calls_today` and `key_configured` are read by the caller (`fungwire_server.rs` for STT, `graph_build.rs` for LLM) before invoking this.

### 5.3 Spend guardrail

`cloud_call_counter` table: `(task_kind TEXT, call_date TEXT, count INTEGER)`, primary key `(task_kind, call_date)`. On a successful cloud dispatch, `UPSERT ... count = count + 1` for today's date (local desktop date, `YYYY-MM-DD`). `calls_today(task_kind)` queries today's row (missing row = 0). No cleanup job needed — old rows are tiny and harmless; a future retention pass can prune them if it ever matters.

## 6. Cloud Executors

### 6.1 `cloud_executor.rs` (new, mirrors `tts_executor.rs`)

```rust
pub(crate) fn dispatch_stt(config: &CloudProviderConfig, audio_path: &Path) -> Result<Vec<Segment>, String>
pub(crate) fn dispatch_llm(config: &CloudProviderConfig, prompt: &str) -> Result<String, String>
```

- **STT / OpenAI:** `POST https://api.openai.com/v1/audio/transcriptions`, multipart form (`file`, `model=whisper-1`, `response_format=verbose_json` for segment timestamps), `Authorization: Bearer <key>`. Maps response segments to the same `{ start_ms, end_ms, text, confidence }` shape Phase 2's `Result` frame already uses (confidence defaults to `1.0` — the API doesn't return one).
- **STT / Custom:** same shape as `tts_executor::exec_rest_api` — POST audio bytes + `Authorization` header, expects a JSON segment array back (documented contract, since "custom" is user-defined).
- **LLM / Anthropic:** `POST https://api.anthropic.com/v1/messages`, `x-api-key` header, `anthropic-version` header, single user-turn request matching `graph_build.rs`'s existing prompt (same JSON-object response contract it already parses).
- **LLM / OpenAI:** `POST https://api.openai.com/v1/chat/completions`, `Authorization: Bearer`, same prompt.
- **LLM / Custom:** same Ollama-shaped `{endpoint}/api/chat` contract `graph_build.rs::call_llm` already speaks — a "custom" LLM endpoint is just a different `endpoint`/`model`, no new code path needed for that case.

Timeout: 30s for STT per-request is too short for a real recording — use 120s (still bounded, matches the existing `run_python_worker` job-level timeout expectations, not the short interactive `tts_executor::TIMEOUT`). LLM cloud calls: 60s (shorter than local Ollama's 600s, since cloud APIs are expected to respond faster than a local unaccelerated model).

Error handling: identical redacted/truncated pattern to `tts_executor::exec_rest_api` (status + first 500 chars of body, key never included in any error string — tested).

## 7. FUNGWIRE Extension (STT tier 3)

- `fungwire.rs` `JobStart` control frame gains one field: `executor: "local" | "cloud"` (default `"local"` if absent, for wire compatibility — though Phase 2 and Phase 3 ship together in practice, this keeps the frame forward-tolerant).
- `fungwire_server.rs` worker, step 4 (previously "always `run_python_worker(transcribe.py)`"): branches on `executor`.
  - `"local"` → unchanged Phase 2 path.
  - `"cloud"` → call `policy::decide_cloud_tier(..., CloudTaskKind::Stt, ...)`. `Blocked(reason)` → `Error{code: reason}` frame, job fails fast (mirrors Phase 2's existing `Error{code:"transcribe_failed"}` failure path — no new client-side state machine). `Allow` → `cloud_executor::dispatch_stt`, map its `Segment`s into the same `Result` frame Phase 2 already sends, increment `cloud_call_counter`.
- Everything else — manifest hash verification, chunk transfer, resume, cancel, progress streaming, revocation re-check — is unchanged and reused as-is; `Progress{stage:"transcribing"}` is still emitted (single event before the cloud call, then on completion, since cloud APIs don't offer the incremental `PROGRESS <pct>` lines `transcribe.py` does).

**Mobile UX:** the existing Phase 2 delegate banner (`TimelineScreen`/`CreativeStudio`) gains a second action, shown only when `stt_cloud_enabled` is true on the paired desktop *and* that desktop is reachable (read via a new `fungwire_status` field, §9) — **"ถอดเสียงบนคลาวด์ผ่าน FUNG Desktop"**. Both actions call the same `delegateTranscription`, now taking an `executor` argument.

## 8. LLM Cloud Fallback (desktop-only)

`graph_build.rs::call_llm` today makes an unconditional local-Ollama call. Extension:

```rust
fn call_llm(endpoint: &str, model: &str, prompt: &str, cloud: Option<&CloudProviderConfig>) -> Result<String, String> {
    match call_ollama(endpoint, model, prompt) {
        Ok(text) => Ok(text),
        Err(e) if is_connection_error(&e) => {
            let Some(config) = cloud else { return Err(e) };
            match policy::decide_cloud_tier(&policy, CloudTaskKind::Llm, calls_today, true) {
                TierDecision::Allow => cloud_executor::dispatch_llm(config, prompt),
                TierDecision::Blocked(reason) => Err(format!("Ollama unreachable and cloud fallback blocked: {reason}")),
            }
        }
        Err(e) => Err(e), // non-connection errors (bad response, etc.) are NOT masked by a fallback
    }
}
```

`is_connection_error` distinguishes "Ollama isn't running" (the actual fallback trigger — the exact failure mode this tier exists for) from a malformed-response error, which should surface as a real bug rather than silently retry against a different provider. `llm_provider_config` (existing, reads the `model_providers` table) is unchanged; the cloud config comes from `cloud_config::load_llm_config()` (tries Anthropic entry, then OpenAI, then Custom — first configured wins; which one is "first" is exposed in `CloudProvidersPanel` as an explicit priority note, not left implicit to the user).

## 9. Desktop UI — `CloudProvidersPanel.tsx`

New component, mirrors `TtsProviderPanel.tsx`'s structure and `DevicePairingPanel.tsx`'s Thai-labeled toggle-switch pattern. Own toolbar button (next to the existing TTS button). Sections:

1. **ผู้ให้บริการคลาวด์ (Cloud providers)** — one card per provider slot (Anthropic, OpenAI, Custom×2 for STT/LLM): masked API-key input, save/clear buttons calling new Tauri commands `cloud_config_set`/`cloud_config_clear`/`cloud_config_status` (status returns only "configured: bool", never the key itself — mirrors `zoom_connection_status`'s never-return-the-secret convention).
2. **นโยบายลำดับการประมวลผล (Tier policy)** — per task kind (STT, LLM): toggle switch "อนุญาตให้ใช้คลาวด์" (allow cloud), read-only display of the fixed chain ("อุปกรณ์นี้ → เดสก์ท็อปที่จับคู่ → คลาวด์").
3. **ขีดจำกัดต่อวัน (Daily cap)** — number input (`daily_cap`), today's count per task kind (read-only, from `cloud_call_counter`).

New Tauri commands: `cloud_config_set`, `cloud_config_clear`, `cloud_config_status`, `tier_policy_get`, `tier_policy_set`, `cloud_call_counts_today`.

`fungwire_status` (existing Phase 2 command) gains one field: `stt_cloud_enabled: bool`, so mobile can decide whether to show the cloud delegate action without a separate round-trip.

## 10. Mobile UI

- `DevicesScreen` ("อุปกรณ์" tab): a small read-only card under the paired-desktop entry — "คลาวด์: เปิดใช้งาน" / "คลาวด์: ปิดใช้งาน" (reflecting the paired desktop's `fungwire_status().stt_cloud_enabled`). Mobile cannot toggle this — the setting is owned by whichever device holds the keys, consistent with §2's "desktop owns keys and policy" decision. No new screen; this is one card, same visual language as the existing device list.
- Delegate banner (`TimelineScreen`): second action button as described in §7, plus once a job's `executor` field on its `delegated_jobs`/`DelegatedJob` record is `"cloud"`, the progress UI shows a small "☁ คลาวด์" badge instead of nothing — purely informational, no new interaction.

## 11. Data Model Changes

| Store | Change |
|---|---|
| Desktop OS keyring | + up to 5 new entries (`cloud-stt-openai`, `cloud-stt-custom`, `cloud-llm-anthropic`, `cloud-llm-openai`, `cloud-llm-custom`) |
| Desktop SQLite WAL DB | + `tier_policy` (single row), + `cloud_call_counter` (`task_kind`, `call_date`, `count`) |
| Mobile Genesis `delegated_jobs` | + `executor TEXT` column (`"local" \| "cloud"`, nullable, defaults to `"local"` on existing rows) — persisted so the cloud badge (§10) survives app restart/reconnect, not just carried in the in-flight wire manifest |
| Supabase | **no change** — cloud keys/policy never leave the desktop |

## 12. New Components

**Rust (new files):**
- `src-tauri/src/cloud_config.rs` — `CloudProviderConfig`, keyring save/load/delete/validate (§4).
- `src-tauri/src/cloud_executor.rs` — `dispatch_stt`, `dispatch_llm` (§6).
- `src-tauri/src/policy.rs` — `TierPolicy`, `decide_cloud_tier`, `cloud_call_counter` read/increment (§5).

**Rust (modified):**
- `fungwire.rs` — `JobStart.executor` field.
- `fungwire_server.rs` — worker branch on `executor` (§7).
- `graph_build.rs` — `call_llm` cloud fallback (§8).
- `lib.rs` — register new commands (§9); `fungwire_status` gains `stt_cloud_enabled`.
- `genesis_adapter.rs` — schema upgrade adding `delegated_jobs.executor` (idempotent, same pattern as the existing `schema_v6_adds_paired_devices_public_key_and_upgrade_is_idempotent` upgrade).

**Frontend (new):**
- `src/components/CloudProvidersPanel.tsx` (+ `.css`) — §9.

**Frontend (modified):**
- `src/App.tsx` — new toolbar button + panel-open state.
- `src/mobile/model.ts` — `DelegatedJob.executor?: "local" | "cloud"`.
- `src/mobile/bridge.ts` — `delegateTranscription` gains an `executor` argument; new `desktopCloudEnabled` read.
- `src/mobile/TimelineScreen.tsx` / `CreativeStudio.tsx` — second delegate action + cloud badge (§7, §10).
- `src/mobile/MobileApp.tsx` (`DevicesScreen`) — read-only tier-policy card (§10).

## 13. Security

| Concern | Mechanism |
|---|---|
| Keys never leave desktop | Keyring-only storage; grep-based test asserts no `CloudProviderConfig`/key material appears in any `serde_json::to_string` call site that touches GenesisBlockDB, Supabase, or `localStorage` |
| Keys never logged | Hand-written redacted `Debug`, same as `zoom_sync::TokenSet`; error strings from `cloud_executor` are truncated response bodies, never the request (which is where the key lives) |
| Default is off | `TierPolicy::default()` = both toggles `false`; a fresh install makes zero cloud network calls without explicit opt-in (proven by a network-call-count test with default policy) |
| Cloud calls are TLS-only | `Custom.endpoint` validation requires `https://` (stricter than `tts_config`'s LAN-allowing `is_private_ip`, since these are inherently leaving the local network) |
| Spend runaway | Daily cap blocks further calls once hit; blocked calls fail fast with a clear reason, never silently retry |
| Revoked/unpaired mobile can't trigger cloud spend | STT cloud dispatch happens inside the same Phase 2 FUNGWIRE worker, which already re-checks `paired_devices.revoked_at` on every `JobStart` — unchanged, inherited for free |

**Residual (documented):** if a user's desktop OS-level account is compromised, the OS keyring's protection is only as strong as the OS session (same residual risk `zoom_sync.rs` already documents for OAuth tokens — not new to this phase). Audio/prompt content sent to a cloud provider is inherently outside FUNG's local-first boundary once tier 3 is opted into — this is the explicit, user-chosen trade-off the feature exists to offer, not a defect.

## 14. Testing Strategy

- **`cloud_config.rs`:** keyring roundtrip per provider variant; validation (empty key, non-`https://` custom endpoint); `Debug` never contains the real key (`assert!(!format!("{:?}", config).contains(&real_key))`, same pattern as `zoom_sync::token_set_debug_never_exposes_secrets`).
- **`cloud_executor.rs`:** dispatch against a local fake HTTP server (`tiny_http` or a raw `TcpListener` stub, matching the existing `worker_tests` style) for both success and error-status responses; timeout behavior; error-string truncation and redaction.
- **`policy.rs`:** pure matrix test over `{enabled, disabled} × {cap not reached, cap reached} × {key configured, not configured}` → correct `TierDecision` for both task kinds — no I/O, fast (mirrors `on_device_ai::admit_task`'s existing test style).
- **`fungwire_server.rs`:** extend the existing loopback integration test with an `executor: "cloud"` job hitting a fake local HTTP server standing in for OpenAI's endpoint; assert the same `Result` frame shape as the local-executor test.
- **`graph_build.rs`:** extend the existing `a_failed_llm_call_leaves_the_prior_extraction_intact` test with a cloud-configured case — Ollama connection failure + cloud enabled + key configured → cloud path taken, extraction succeeds; Ollama connection failure + cloud disabled → original failure behavior unchanged.
- **Acceptance (master plan REQ-F-01…04):**
  - Grep proves no API key ever serialized to GenesisBlockDB/Supabase/localStorage.
  - Policy matrix test (above) covers "3 tiers × availability combinations → correct executor chosen."
  - Fresh install: default policy is cloud-off for both task kinds; a test asserts zero outbound calls to any cloud host without explicit opt-in.

## 15. Controller Gate (after final review, before merge)

1. No Supabase migration — nothing to apply.
2. No dashboard change.
3. Manual acceptance: on a real desktop, register an OpenAI key, enable STT cloud tier, delegate a real recording from a paired mobile with the desktop's local pipeline temporarily disabled (or simply confirm the executor path via logs) — confirm segments land on mobile with the cloud badge. Register an Anthropic key, stop the local Ollama service, confirm graph extraction still completes via cloud fallback.

## 16. Resolved (was: Open Questions for Spec Review)

Boss approved 2026-08-09 — resolved as originally proposed, no design changes needed:

- **LLM provider priority when multiple are configured:** confirmed "first configured wins" (Anthropic → OpenAI → Custom), order surfaced in `CloudProvidersPanel` — no explicit-picker UI added.
- **STT segment confidence from OpenAI:** confirmed default `1.0` (not `null`/omitted) — matches `Segment.confidence`'s existing type (non-optional `f64` from the Phase 2 pipeline).
- **Cloud timeouts:** confirmed fixed 120s (STT) / 60s (LLM), not user-configurable in v1 — same fixed-timeout convention as `tts_executor::TIMEOUT`.

## 17. Requirement Traceability

| Master-plan REQ | Section |
|---|---|
| REQ-F-01 register cloud API keys, keyring-only, never Supabase | §4, §9, §13 |
| REQ-F-02 policy engine: local → paired desktop → cloud, per task kind | §5, §7, §8 |
| REQ-F-03 cloud executor for STT/LLM + spend guardrails | §6, §5.3 |
| REQ-F-04 mobile settings surface for tier policy, privacy default | §10, §5.1 |
