# Phase 1: Device Pairing + Desktop/Mobile Login — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Verified 6-digit-code device pairing between mobile and desktop, brokered by Supabase, with Google login on both Tauri surfaces.

**Architecture:** Auth runs in TypeScript on both surfaces (shared `src/lib/authFlow.ts`, supabase-js PKCE + system browser + `fung://` deep link, loopback fallback on desktop). Rust owns: deep-link plugin, device ed25519 keypair, local pairing persistence (desktop SQLite / mobile GenesisBlockDB). Supabase owns the handshake (`pairing_sessions` + `confirm_pairing` RPC). See spec: `docs/superpowers/specs/2026-08-09-phase-1-pairing-desktop-login-design.md`.

**Tech Stack:** React 18, supabase-js v2, Tauri v2, tauri-plugin-deep-link, ed25519-dalek, rusqlite, GenesisBlockDB.

## Global Constraints

- UI labels Thai; code identifiers English; named exports only; CSS = hardcoded light values + `.theme-dark .prefix-*` overrides (no CSS custom properties for colors)
- Anon key only in frontend; no service role key anywhere client-side
- The 6-digit code is NEVER stored or transmitted in plaintext to the DB — only `sha256(session_id || ':' || code)` hex
- Client computes `code_hash` with WebCrypto; SQL recomputes with `sha256()` — the formula strings must match character-for-character: `${sessionId}:${code}`
- No Supabase Realtime — desktop polls its session row every 2 s
- Device private key never appears in any DB, log, serialized struct, or frontend payload — fingerprint only
- Pairing device rows are `platform IN ('windows','android')` — web Dashboard rows are not pairable
- `devices` column grants: INSERT (user_id, device_label, platform, public_key_fingerprint); UPDATE (device_label, last_seen_at) ONLY — never upsert whole rows (conflict-update on non-granted columns fails 42501); revoke = DELETE row
- Supabase project ref: `nqnrvqnijzovkrhxslfp`; migration applies ONLY at controller/Boss gate, never by an implementer
- `npx tsc --noEmit` must exit 0 after every task; run `cargo test --manifest-path src-tauri/Cargo.toml <module>` focused per task, full suite before each commit that touches Rust

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `supabase/migrations/20260809000000_pairing_sessions.sql` | 1 | pairing_sessions + confirm_pairing RPC + device_audit_events |
| `src-tauri/src/device_identity.rs` (new) | 2 | ed25519 keypair, keyring/file storage, fingerprint |
| `src-tauri/src/lib.rs` | 2,3,5 | command registration, deep-link init, loopback listener, desktop paired_devices |
| `src-tauri/Cargo.toml`, `tauri.conf.json`, `capabilities/default.json` | 2,3 | deps + plugin config |
| `src/lib/authFlow.ts` (new) + `tests/authFlow.test.mjs` | 4 | login flow, callback parsing, listener |
| `src/lib/supabase.ts` | 4 | add `flowType: "pkce"` |
| `schemas/sqlite-wal-v1.sql` | 5 | desktop paired_devices table |
| `src/components/AccountLoginPanel.tsx` (+`.css`) | 6 | desktop login + device registration |
| `src/components/DevicePairingPanel.tsx` (+`.css`) | 7 | pairing dialog, paired list, revoke |
| `src/App.tsx` | 6,7 | wire both panels (TtsProviderPanel pattern) |
| `src-tauri/src/mobile.rs` | 8 | replace mobile_pair_desktop → mobile_pairing_complete |
| `src/mobile/MobileApp.tsx`, `bridge.ts`, `model.ts`, `mobileStore.ts` | 9 | DevicesScreen rework |
| `src/web/Dashboard.tsx` | 10 | live paired-devices tile |

Task order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10. (4 needs 3's event contract; 6/7 need 4+5; 9 needs 4+8; 10 independent after 1.)

---

### Task 1: Supabase migration — pairing_sessions + confirm_pairing + device_audit_events

**Files:** Create `supabase/migrations/20260809000000_pairing_sessions.sql`

**Interfaces produced:** table `public.pairing_sessions`, RPC `public.confirm_pairing(p_session_id uuid, p_code text, p_responder_device_id uuid) returns text` (values: `confirmed|wrong_code|locked|expired|cancelled|not_found`), table `public.device_audit_events`.

- [x] **Step 1: Write the migration file** — exact content (copied from spec §6, single source of truth for the SQL):

```sql
-- Pairing sessions: short-lived brokered handshakes between two of a user's devices.
create table if not exists public.pairing_sessions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  initiator_device_id uuid not null references public.devices (id) on delete cascade,
  responder_device_id uuid references public.devices (id) on delete set null,
  code_hash text not null,
  status text not null default 'pending'
    check (status in ('pending','confirmed','expired','cancelled','locked')),
  attempt_count integer not null default 0,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '5 minutes',
  confirmed_at timestamptz
);

alter table public.pairing_sessions enable row level security;

create policy "pairing_sessions_select_own" on public.pairing_sessions
  for select using ((select auth.uid()) = user_id);
create policy "pairing_sessions_insert_own" on public.pairing_sessions
  for insert with check ((select auth.uid()) = user_id);
create policy "pairing_sessions_update_own" on public.pairing_sessions
  for update using ((select auth.uid()) = user_id);

grant select, insert, update on public.pairing_sessions to authenticated;

create index pairing_sessions_user_pending_idx
  on public.pairing_sessions (user_id, created_at desc)
  where status = 'pending';

-- Atomic code verification with attempt limiting. security invoker: RLS applies.
create or replace function public.confirm_pairing(
  p_session_id uuid,
  p_code text,
  p_responder_device_id uuid
) returns text
language plpgsql
security invoker
as $$
declare
  v_session public.pairing_sessions%rowtype;
begin
  select * into v_session from public.pairing_sessions
    where id = p_session_id for update;

  if not found then return 'not_found'; end if;
  if v_session.status = 'locked' then return 'locked'; end if;
  if v_session.status <> 'pending' then return v_session.status; end if;
  if v_session.expires_at < now() then
    update public.pairing_sessions set status = 'expired' where id = p_session_id;
    return 'expired';
  end if;

  if v_session.code_hash = encode(sha256((p_session_id::text || ':' || p_code)::bytea), 'hex') then
    update public.pairing_sessions
      set status = 'confirmed', confirmed_at = now(),
          responder_device_id = p_responder_device_id
      where id = p_session_id;
    return 'confirmed';
  end if;

  update public.pairing_sessions
    set attempt_count = attempt_count + 1,
        status = case when attempt_count + 1 >= 5 then 'locked' else status end
    where id = p_session_id;
  return case when v_session.attempt_count + 1 >= 5 then 'locked' else 'wrong_code' end;
end;
$$;

-- Device lifecycle audit trail (user-scoped).
create table if not exists public.device_audit_events (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  device_id uuid,
  event_type text not null check (char_length(event_type) between 1 and 60),
  metadata jsonb not null default '{}',
  created_at timestamptz not null default now()
);
alter table public.device_audit_events enable row level security;
create policy "device_audit_select_own" on public.device_audit_events
  for select using ((select auth.uid()) = user_id);
create policy "device_audit_insert_own" on public.device_audit_events
  for insert with check ((select auth.uid()) = user_id);
grant select, insert on public.device_audit_events to authenticated;
```

- [x] **Step 2: Sanity checks** — confirm: every `create table` is `if not exists`; both tables have RLS enabled + policies before grants; the function is `security invoker`; the hash expression is exactly `encode(sha256((p_session_id::text || ':' || p_code)::bytea), 'hex')`.
- [x] **Step 3: Do NOT apply the migration** — applying to `nqnrvqnijzovkrhxslfp` happens at the controller gate after final review.
- [x] **Step 4: Commit** — `git add supabase/migrations/20260809000000_pairing_sessions.sql && git commit -m "feat(pairing): add pairing_sessions, confirm_pairing RPC, and device audit migration"`

---

### Task 2: Device identity (Rust) — keypair, fingerprint, storage

**Files:** Create `src-tauri/src/device_identity.rs`; Modify `src-tauri/Cargo.toml` (deps), `src-tauri/src/lib.rs` (module + command registration)

**Interfaces produced:** Tauri command `device_identity_ensure() -> { fingerprint: String, created: bool }` (both surfaces invoke it).

- [x] **Step 1: Add deps to `src-tauri/Cargo.toml` [dependencies]:**

```toml
ed25519-dalek = "2"
rand = "0.8"
```

- [x] **Step 2: Write failing tests first** in `device_identity.rs` `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_and_reloads_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_identity_in_dir(dir.path()).unwrap();
        assert!(first.created);
        let second = ensure_identity_in_dir(dir.path()).unwrap();
        assert!(!second.created);
        assert_eq!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn fingerprint_is_64_hex_chars() {
        let dir = tempfile::tempdir().unwrap();
        let id = ensure_identity_in_dir(dir.path()).unwrap();
        assert_eq!(id.fingerprint.len(), 64);
        assert!(id.fingerprint.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
```

- [x] **Step 3: Run** `cargo test --manifest-path src-tauri/Cargo.toml device_identity` — expect FAIL (functions not defined).
- [x] **Step 4: Implement** `device_identity.rs`:

```rust
use std::fs;
use std::path::Path;

use base64::Engine;
use ed25519_dalek::SigningKey;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{AppError, AppResult};

const KEY_FILE: &str = "device_identity.key";

#[derive(Debug, Clone, Serialize)]
pub struct DeviceIdentity {
    pub fingerprint: String,
    pub created: bool,
}

fn fingerprint_of(signing_key: &SigningKey) -> String {
    let public = signing_key.verifying_key();
    let digest = Sha256::digest(public.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// File-backed identity storage inside the app data dir. On Windows the file
/// lives in %APPDATA%; on Android inside the app-private files dir. Keystore /
/// keyring hardening is a Phase 1 backlog item (spec §15.4).
pub fn ensure_identity_in_dir(dir: &Path) -> AppResult<DeviceIdentity> {
    fs::create_dir_all(dir)
        .map_err(|e| AppError::Internal(format!("identity dir: {e}")))?;
    let path = dir.join(KEY_FILE);
    if path.exists() {
        let encoded = fs::read_to_string(&path)
            .map_err(|e| AppError::Internal(format!("identity read: {e}")))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| AppError::Internal(format!("identity decode: {e}")))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| AppError::Internal("identity key length".into()))?;
        let key = SigningKey::from_bytes(&arr);
        return Ok(DeviceIdentity { fingerprint: fingerprint_of(&key), created: false });
    }
    let key = SigningKey::generate(&mut rand::rngs::OsRng);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key.to_bytes());
    fs::write(&path, encoded)
        .map_err(|e| AppError::Internal(format!("identity write: {e}")))?;
    Ok(DeviceIdentity { fingerprint: fingerprint_of(&key), created: true })
}

#[tauri::command]
pub fn device_identity_ensure(app: tauri::AppHandle) -> AppResult<DeviceIdentity> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app data dir: {e}")))?;
    ensure_identity_in_dir(&dir)
}
```

Adjust `AppError` variant names to whatever `lib.rs` actually defines (read it — do not invent a new variant if an equivalent internal/storage variant exists).

- [x] **Step 5:** `mod device_identity;` in `lib.rs` + add `device_identity::device_identity_ensure` to `generate_handler![]`.
- [x] **Step 6: Run** the module tests — expect PASS. Then full `cargo test` + `npx tsc --noEmit`.
- [x] **Step 7: Commit** — `feat(pairing): add device identity keypair with fingerprint`

---

### Task 3: Deep link plugin + loopback fallback (Rust)

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`; npm: add `@tauri-apps/plugin-deep-link`

**Interfaces produced:** deep link `fung://` delivered to webview via the plugin's `onOpenUrl` JS API; Tauri command `auth_loopback_listen() -> u16` (port) which emits event `"auth-callback"` (payload: full callback URL string) when the browser hits it.

- [x] **Step 1:** `npm install @tauri-apps/plugin-deep-link` and add `tauri-plugin-deep-link = "2"` to Cargo dependencies.
- [x] **Step 2:** Configure the scheme. In `src-tauri/tauri.conf.json` add to `plugins`:

```json
"deep-link": {
  "desktop": {
    "schemes": ["fung"]
  }
}
```

For Android the plugin reads mobile config at build time — consult the plugin's README (`https://v2.tauri.app/plugin/deep-linking/`) for the current mobile custom-scheme syntax and add it accordingly; the scheme must be `fung`. If mobile config genuinely cannot express a custom scheme (docs indicate app-links/host-based only), report DONE_WITH_CONCERNS stating exactly what the docs say — do NOT improvise a manifest hack.

- [x] **Step 3:** Register plugin in the builder in `lib.rs` (`.plugin(tauri_plugin_deep_link::init())`) and add the plugin permission (`"deep-link:default"`) to `src-tauri/capabilities/default.json`.
- [x] **Step 4:** Implement the loopback fallback command in `lib.rs` (pattern: `start_local_api`):

```rust
#[tauri::command]
fn auth_loopback_listen(app: tauri::AppHandle) -> AppResult<u16> {
    use std::io::{Read, Write};
    use tauri::Emitter;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| AppError::Internal(format!("loopback bind: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Internal(format!("loopback addr: {e}")))?
        .port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_string();
            let body = "<html><body style=\"font-family:sans-serif\"><p>เข้าสู่ระบบสำเร็จ ปิดหน้าต่างนี้ได้เลย</p></body></html>";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let full_url = format!("http://127.0.0.1:{port}{path}");
            let _ = app.emit("auth-callback", full_url);
        }
    });
    Ok(port)
}
```

Register in `generate_handler![]`. One-shot by design (thread exits after first request).

- [x] **Step 5:** Unit test the path-parsing seam if extracted; otherwise `cargo build` + full `cargo test` green is the gate (network accept loop is covered by manual acceptance).
- [x] **Step 6:** `npx tsc --noEmit` + `npm run build` green (new npm package must not break the web build — the deep-link JS import happens only behind dynamic `import()` in Task 4).
- [x] **Step 7: Commit** — `feat(pairing): register fung:// deep link and loopback auth fallback`

---

### Task 4: Shared auth flow (TypeScript)

**Files:** Create `src/lib/authFlow.ts`, `tests/authFlow.test.mjs`; Modify `src/lib/supabase.ts`, `package.json` (test script)

**Interfaces produced:** `parseAuthCallbackUrl(url)`, `beginGoogleLogin(redirectTo?)`, `completeFromCallbackUrl(url)`, `listenForAuthCallback(onDone)`, `beginLoopbackFallbackLogin()`, `hashPairingCode(sessionId, code)` (WebCrypto sha256 hex — also used by Task 7).

- [x] **Step 1:** Enable PKCE in `src/lib/supabase.ts`:

```typescript
export const supabase = createClient(supabaseUrl ?? "", supabaseAnonKey ?? "", {
  auth: { flowType: "pkce" },
});
```

**Web regression note (must appear in your report):** the web landing/AuthCallback flow (Sub-project A) currently relies on implicit-flow hash tokens. With `flowType: "pkce"`, supabase-js's `detectSessionInUrl` handles the `?code=` exchange automatically on the callback page, so `AuthCallback.tsx`'s `getSession()` pattern keeps working — verify by reading `src/web/AuthCallback.tsx` and stating in your report why it still works (or flag DONE_WITH_CONCERNS if you find it doesn't).

- [x] **Step 2: Write failing test** `tests/authFlow.test.mjs` (mirror the import pattern of `tests/captureOrchestration.test.mjs`):

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import { parseAuthCallbackUrl } from "../src/lib/authParse.ts";

test("parses code from deep link url", () => {
  const r = parseAuthCallbackUrl("fung://auth/callback?code=abc123");
  assert.equal(r.code, "abc123");
  assert.equal(r.error, null);
});

test("parses error", () => {
  const r = parseAuthCallbackUrl("fung://auth/callback?error=access_denied&error_description=denied");
  assert.equal(r.code, null);
  assert.equal(r.error, "denied");
});

test("parses loopback url", () => {
  const r = parseAuthCallbackUrl("http://127.0.0.1:49213/auth/callback?code=xyz");
  assert.equal(r.code, "xyz");
});

test("rejects garbage", () => {
  assert.equal(parseAuthCallbackUrl("not a url").code, null);
});
```

Pure parsing lives in `src/lib/authParse.ts` (no supabase import — keeps the node test dependency-free):

```typescript
export interface AuthCallbackResult {
  code: string | null;
  error: string | null;
}

export function parseAuthCallbackUrl(url: string): AuthCallbackResult {
  try {
    const normalized = url.startsWith("fung://")
      ? url.replace("fung://", "https://fung.local/")
      : url;
    const parsed = new URL(normalized);
    const error =
      parsed.searchParams.get("error_description") ?? parsed.searchParams.get("error");
    return { code: parsed.searchParams.get("code"), error };
  } catch {
    return { code: null, error: "invalid_url" };
  }
}
```

Add script `"test:auth": "node --test --experimental-strip-types tests/authFlow.test.mjs"` and append it to the CI frontend job? NO — CI edits are out of scope for this task; instead note it for Task 10's wrap-up commit. Run: `npm run test:auth` — expect FAIL first (file missing), then PASS after implementing.

- [x] **Step 3: Implement `src/lib/authFlow.ts`:**

```typescript
import { supabase } from "./supabase";
import { parseAuthCallbackUrl } from "./authParse";

const DEEP_LINK_REDIRECT = "fung://auth/callback";

export async function beginGoogleLogin(redirectTo: string = DEEP_LINK_REDIRECT): Promise<void> {
  const { data, error } = await supabase.auth.signInWithOAuth({
    provider: "google",
    options: { redirectTo, skipBrowserRedirect: true },
  });
  if (error) throw error;
  if (!data?.url) throw new Error("ไม่ได้รับ URL สำหรับเข้าสู่ระบบ");
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(data.url);
}

export async function beginLoopbackFallbackLogin(): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  const port = await invoke<number>("auth_loopback_listen");
  await beginGoogleLogin(`http://127.0.0.1:${port}/auth/callback`);
}

export async function completeFromCallbackUrl(url: string): Promise<void> {
  const { code, error } = parseAuthCallbackUrl(url);
  if (error) throw new Error(error);
  if (!code) throw new Error("missing_code");
  const { error: exchangeError } = await supabase.auth.exchangeCodeForSession(code);
  if (exchangeError) throw exchangeError;
}

/** Wires BOTH callback channels (deep link + loopback event). Returns cleanup. */
export async function listenForAuthCallback(
  onDone: (err: string | null) => void,
): Promise<() => void> {
  const cleanups: Array<() => void> = [];
  try {
    const { onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
    const un = await onOpenUrl((urls) => {
      const target = urls.find((u) => u.includes("/auth/callback") || u.startsWith("fung://auth"));
      if (!target) return;
      completeFromCallbackUrl(target)
        .then(() => onDone(null))
        .catch((e) => onDone(e instanceof Error ? e.message : String(e)));
    });
    cleanups.push(un);
  } catch {
    // plugin unavailable (e.g. web preview) — loopback listener below still applies
  }
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<string>("auth-callback", (event) => {
      completeFromCallbackUrl(event.payload)
        .then(() => onDone(null))
        .catch((e) => onDone(e instanceof Error ? e.message : String(e)));
    });
    cleanups.push(un);
  } catch {
    // not in Tauri
  }
  return () => cleanups.forEach((fn) => fn());
}

/** sha256(`${sessionId}:${code}`) lowercase hex — MUST match the SQL expression in confirm_pairing. */
export async function hashPairingCode(sessionId: string, code: string): Promise<string> {
  const data = new TextEncoder().encode(`${sessionId}:${code}`);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
```

- [x] **Step 4:** Add a hash test to `tests/authFlow.test.mjs` cross-checking `hashPairingCode`'s formula against node:crypto (same input → same hex), importing from a pure helper if needed; WebCrypto exists in Node ≥ 20 as `globalThis.crypto`, so testing `hashPairingCode` directly works. Vector: `hashPairingCode("11111111-1111-1111-1111-111111111111", "123456")` must equal node `createHash("sha256").update("11111111-1111-1111-1111-111111111111:123456").digest("hex")`.
- [x] **Step 5:** `npm run test:auth` PASS · `npx tsc --noEmit` 0 · `npm run build` green.
- [x] **Step 6: Commit** — `feat(auth): shared PKCE auth flow with deep link + loopback callback handling`

---

### Task 5: Desktop paired_devices persistence (Rust)

**Files:** Modify `schemas/sqlite-wal-v1.sql`, `src-tauri/src/lib.rs`

**Interfaces produced:** commands `paired_device_upsert(device: PairedDeviceInput)`, `paired_device_list() -> Vec<PairedDeviceRow>`, `paired_device_revoke(id: String)`.

- [x] **Step 1:** Append to `schemas/sqlite-wal-v1.sql`:

```sql
CREATE TABLE IF NOT EXISTS paired_devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  paired_at TEXT NOT NULL,
  revoked_at TEXT,
  pairing_session_id TEXT NOT NULL
);
```

First READ how `lib.rs` initializes the SQLite schema (search for `sqlite-wal-v1` / `include_str!` / `CREATE TABLE`). If the schema file is executed wholesale at startup, the append is sufficient; if tables are created individually in code, add the same `CREATE TABLE IF NOT EXISTS` to that code path too. State which mechanism you found in your report.

- [x] **Step 2: Failing tests first** (in `lib.rs` tests module or alongside the storage impl, using the existing test-storage pattern):

```rust
#[test]
fn paired_device_roundtrip() {
    let storage = test_storage(); // reuse existing helper pattern in lib.rs tests
    upsert_paired_device(&storage, PairedDeviceInput {
        id: "dev-1".into(), name: "Pixel".into(), platform: "android".into(),
        fingerprint: "ab".repeat(32), pairing_session_id: "sess-1".into(),
    }).unwrap();
    let rows = list_paired_devices(&storage).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].revoked_at.is_none());
    revoke_paired_device(&storage, "dev-1").unwrap();
    let rows = list_paired_devices(&storage).unwrap();
    assert!(rows[0].revoked_at.is_some());
}
```

- [x] **Step 3:** Run focused test → FAIL. Implement the three inner functions + `#[tauri::command]` wrappers (serde structs `PairedDeviceInput { id, name, platform, fingerprint, pairing_session_id }`, `PairedDeviceRow { id, name, platform, fingerprint, paired_at, revoked_at, pairing_session_id }`; `paired_at`/`revoked_at` = RFC3339 via `chrono::Utc::now().to_rfc3339()`). Upsert = `INSERT ... ON CONFLICT(id) DO UPDATE SET name=excluded.name, revoked_at=NULL`. Register all three commands.
- [x] **Step 4:** Focused test PASS → full `cargo test` → `npx tsc --noEmit`.
- [x] **Step 5: Commit** — `feat(pairing): desktop paired_devices storage and commands`

---

### Task 6: Desktop AccountLoginPanel

**Files:** Create `src/components/AccountLoginPanel.tsx`, `src/components/AccountLoginPanel.css`; Modify `src/App.tsx`

**Interfaces:** consumes `authFlow.ts` (Task 4), `device_identity_ensure` (Task 2). Produces: logged-in session + registered device + `localStorage["fung.device.id"]` (own cloud device id — Task 7/9 read this).

- [x] **Step 1: Component** — complete code:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import type { Session } from "@supabase/supabase-js";
import { LogIn, LogOut, MonitorSmartphone, RefreshCw } from "lucide-react";
import { supabase } from "../lib/supabase";
import {
  beginGoogleLogin,
  beginLoopbackFallbackLogin,
  listenForAuthCallback,
} from "../lib/authFlow";
import { invoke } from "@tauri-apps/api/core";
import "./AccountLoginPanel.css";

interface DeviceIdentity {
  fingerprint: string;
  created: boolean;
}

const DEVICE_ID_KEY = "fung.device.id";
const FALLBACK_DELAY_MS = 120_000;

export function AccountLoginPanel() {
  const [session, setSession] = useState<Session | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showFallback, setShowFallback] = useState(false);
  const [deviceLabel, setDeviceLabel] = useState("FUNG Desktop");
  const [registered, setRegistered] = useState(false);
  const fallbackTimer = useRef<number | null>(null);

  useEffect(() => {
    void supabase.auth.getSession().then(({ data }) => setSession(data.session ?? null));
    const { data: sub } = supabase.auth.onAuthStateChange((_e, s) => setSession(s));
    let cleanup: (() => void) | undefined;
    void listenForAuthCallback((err) => {
      setBusy(false);
      setShowFallback(false);
      if (fallbackTimer.current) window.clearTimeout(fallbackTimer.current);
      setError(err ? `เข้าสู่ระบบไม่สำเร็จ: ${err}` : null);
    }).then((fn) => { cleanup = fn; });
    return () => {
      sub.subscription.unsubscribe();
      cleanup?.();
      if (fallbackTimer.current) window.clearTimeout(fallbackTimer.current);
    };
  }, []);

  // Register this device once per session.
  useEffect(() => {
    if (!session || registered) return;
    let cancelled = false;
    void (async () => {
      try {
        const identity = await invoke<DeviceIdentity>("device_identity_ensure");
        const { data: existing, error: selErr } = await supabase
          .from("devices")
          .select("id, device_label")
          .eq("public_key_fingerprint", identity.fingerprint)
          .maybeSingle();
        if (selErr) throw selErr;
        let deviceId = existing?.id as string | undefined;
        if (!deviceId) {
          const { data: inserted, error: insErr } = await supabase
            .from("devices")
            .insert({
              user_id: session.user.id,
              device_label: deviceLabel,
              platform: "windows",
              public_key_fingerprint: identity.fingerprint,
            })
            .select("id")
            .single();
          if (insErr) throw insErr;
          deviceId = inserted.id as string;
          await supabase.from("device_audit_events").insert({
            user_id: session.user.id,
            device_id: deviceId,
            event_type: "device_registered",
            metadata: { platform: "windows" },
          });
        } else {
          await supabase
            .from("devices")
            .update({ last_seen_at: new Date().toISOString() })
            .eq("id", deviceId);
        }
        if (!cancelled && deviceId) {
          localStorage.setItem(DEVICE_ID_KEY, deviceId);
          setRegistered(true);
        }
      } catch (e) {
        if (!cancelled) {
          console.error("Device registration failed:", e);
          setError("ลงทะเบียนอุปกรณ์ไม่สำเร็จ");
        }
      }
    })();
    return () => { cancelled = true; };
  }, [session, registered, deviceLabel]);

  const handleLogin = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await beginGoogleLogin();
      fallbackTimer.current = window.setTimeout(() => setShowFallback(true), FALLBACK_DELAY_MS);
    } catch (e) {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleFallback = useCallback(async () => {
    setError(null);
    try {
      await beginLoopbackFallbackLogin();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleLogout = useCallback(async () => {
    await supabase.auth.signOut();
    setRegistered(false);
  }, []);

  return (
    <section className="account-login-panel" aria-label="บัญชี FUNG">
      <header className="account-login-header">
        <MonitorSmartphone size={18} />
        <h3>บัญชี FUNG</h3>
      </header>
      {session ? (
        <div className="account-login-signed-in">
          <p className="account-login-email">{session.user.email}</p>
          <label className="account-login-label">
            ชื่ออุปกรณ์นี้
            <input
              value={deviceLabel}
              onChange={(e) => setDeviceLabel(e.target.value)}
              maxLength={120}
            />
          </label>
          <p className="account-login-status">
            {registered ? "อุปกรณ์ลงทะเบียนแล้ว ✓" : "กำลังลงทะเบียนอุปกรณ์…"}
          </p>
          <button type="button" className="account-login-btn" onClick={handleLogout}>
            <LogOut size={15} /> ออกจากระบบ
          </button>
        </div>
      ) : (
        <div className="account-login-signed-out">
          <button type="button" className="account-login-btn" onClick={handleLogin} disabled={busy}>
            <LogIn size={15} /> {busy ? "รอการยืนยันในเบราว์เซอร์…" : "เข้าสู่ระบบด้วย Google"}
          </button>
          {showFallback && (
            <button type="button" className="account-login-btn account-login-btn-secondary" onClick={handleFallback}>
              <RefreshCw size={15} /> ลองวิธีสำรอง (loopback)
            </button>
          )}
        </div>
      )}
      {error && <p className="account-login-error">{error}</p>}
    </section>
  );
}
```

- [x] **Step 2: CSS** `AccountLoginPanel.css` — follow `ExternalAccountPanel`'s visual vocabulary (read its css for exact color values), classes prefixed `account-login-`, hardcoded light values + `.theme-dark .account-login-*` overrides. Style: header row, labeled input, primary/secondary buttons, error in `#b3261e`-family red with dark override.
- [x] **Step 3: Wire into `src/App.tsx`** exactly the way `TtsProviderPanel` is wired (find its import, its open-state, its settings entry/button, its conditional render — replicate all four for `AccountLoginPanel` with a Thai menu label "บัญชี & อุปกรณ์"). Do not restructure anything else.
- [x] **Step 4:** `npx tsc --noEmit` 0 · `npm run build` green.
- [x] **Step 5: Commit** — `feat(auth): desktop account login panel with device registration`

---

### Task 7: Desktop DevicePairingPanel

**Files:** Create `src/components/DevicePairingPanel.tsx`, `src/components/DevicePairingPanel.css`; Modify `src/App.tsx` (render inside/below AccountLoginPanel's settings surface — same pattern)

**Interfaces:** consumes `hashPairingCode` (Task 4), `paired_device_*` commands (Task 5), `localStorage["fung.device.id"]` (Task 6), Supabase `pairing_sessions`/`devices`/`device_audit_events` (Task 1).

- [x] **Step 1: Component** — complete code:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { Link2, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { supabase } from "../lib/supabase";
import { hashPairingCode } from "../lib/authFlow";
import "./DevicePairingPanel.css";

interface PairedDeviceRow {
  id: string;
  name: string;
  platform: string;
  fingerprint: string;
  paired_at: string;
  revoked_at: string | null;
  pairing_session_id: string;
}

type PairingState =
  | { kind: "idle" }
  | { kind: "waiting"; sessionId: string; code: string; expiresAt: number }
  | { kind: "confirmed"; peerName: string }
  | { kind: "error"; message: string };

const DEVICE_ID_KEY = "fung.device.id";
const POLL_MS = 2000;

function generateCode(): string {
  const buf = new Uint32Array(1);
  crypto.getRandomValues(buf);
  return String(buf[0] % 1_000_000).padStart(6, "0");
}

export function DevicePairingPanel() {
  const [paired, setPaired] = useState<PairedDeviceRow[]>([]);
  const [pairing, setPairing] = useState<PairingState>({ kind: "idle" });
  const [now, setNow] = useState(Date.now());
  const pollTimer = useRef<number | null>(null);

  const refreshLocal = useCallback(async () => {
    try {
      setPaired(await invoke<PairedDeviceRow[]>("paired_device_list"));
    } catch (e) {
      console.error("Failed to list paired devices:", e);
    }
  }, []);

  useEffect(() => {
    void refreshLocal();
  }, [refreshLocal]);

  // Revocation propagation: verify each local peer still exists in the cloud.
  useEffect(() => {
    void (async () => {
      const local = await invoke<PairedDeviceRow[]>("paired_device_list").catch(() => []);
      const active = local.filter((d) => !d.revoked_at);
      if (active.length === 0) return;
      const { data, error } = await supabase
        .from("devices")
        .select("id")
        .in("id", active.map((d) => d.id));
      if (error) { console.error("Revocation check failed:", error); return; }
      const alive = new Set((data ?? []).map((r) => r.id as string));
      for (const d of active) {
        if (!alive.has(d.id)) await invoke("paired_device_revoke", { id: d.id });
      }
      void refreshLocal();
    })();
  }, [refreshLocal]);

  // Countdown tick while waiting.
  useEffect(() => {
    if (pairing.kind !== "waiting") return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [pairing.kind]);

  const stopPolling = useCallback(() => {
    if (pollTimer.current) {
      window.clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
  }, []);

  const startPairing = useCallback(async () => {
    const myDeviceId = localStorage.getItem(DEVICE_ID_KEY);
    const { data: sessionData } = await supabase.auth.getSession();
    const userId = sessionData.session?.user.id;
    if (!myDeviceId || !userId) {
      setPairing({ kind: "error", message: "ต้องเข้าสู่ระบบและลงทะเบียนอุปกรณ์ก่อน" });
      return;
    }
    const sessionId = crypto.randomUUID();
    const code = generateCode();
    const codeHash = await hashPairingCode(sessionId, code);
    // Opportunistic cleanup of stale sessions (spec §6).
    await supabase
      .from("pairing_sessions")
      .delete()
      .lt("expires_at", new Date(Date.now() - 86_400_000).toISOString());
    const { error } = await supabase.from("pairing_sessions").insert({
      id: sessionId,
      user_id: userId,
      initiator_device_id: myDeviceId,
      code_hash: codeHash,
    });
    if (error) {
      setPairing({ kind: "error", message: `สร้างรหัสไม่สำเร็จ: ${error.message}` });
      return;
    }
    await supabase.from("device_audit_events").insert({
      user_id: userId,
      device_id: myDeviceId,
      event_type: "pairing_session_created",
      metadata: { session_id: sessionId },
    });
    setPairing({ kind: "waiting", sessionId, code, expiresAt: Date.now() + 5 * 60_000 });
    pollTimer.current = window.setInterval(() => void poll(sessionId, userId), POLL_MS);
  }, []);

  const poll = useCallback(
    async (sessionId: string, userId: string) => {
      const { data, error } = await supabase
        .from("pairing_sessions")
        .select("status, responder_device_id")
        .eq("id", sessionId)
        .single();
      if (error) { console.error("Pairing poll failed:", error); return; }
      if (data.status === "confirmed" && data.responder_device_id) {
        stopPolling();
        const { data: peer } = await supabase
          .from("devices")
          .select("id, device_label, platform, public_key_fingerprint")
          .eq("id", data.responder_device_id)
          .single();
        if (peer) {
          await invoke("paired_device_upsert", {
            device: {
              id: peer.id,
              name: peer.device_label,
              platform: peer.platform,
              fingerprint: peer.public_key_fingerprint,
              pairing_session_id: sessionId,
            },
          });
          await supabase.from("device_audit_events").insert({
            user_id: userId,
            device_id: peer.id,
            event_type: "pairing_confirmed",
            metadata: { session_id: sessionId },
          });
          setPairing({ kind: "confirmed", peerName: peer.device_label });
          void refreshLocal();
        }
      } else if (data.status === "locked" || data.status === "expired") {
        stopPolling();
        setPairing({
          kind: "error",
          message: data.status === "locked" ? "ใส่รหัสผิดครบ 5 ครั้ง — สร้างรหัสใหม่" : "รหัสหมดอายุ — สร้างรหัสใหม่",
        });
      }
    },
    [refreshLocal, stopPolling],
  );

  useEffect(() => () => stopPolling(), [stopPolling]);

  const revoke = useCallback(
    async (row: PairedDeviceRow) => {
      const { data: sessionData } = await supabase.auth.getSession();
      const userId = sessionData.session?.user.id;
      const { error } = await supabase.from("devices").delete().eq("id", row.id);
      if (error) { console.error("Cloud revoke failed:", error); }
      await invoke("paired_device_revoke", { id: row.id });
      if (userId) {
        await supabase.from("device_audit_events").insert({
          user_id: userId,
          device_id: row.id,
          event_type: "device_revoked",
          metadata: { name: row.name },
        });
      }
      void refreshLocal();
    },
    [refreshLocal],
  );

  const remainingMs = pairing.kind === "waiting" ? Math.max(0, pairing.expiresAt - now) : 0;

  return (
    <section className="device-pairing-panel" aria-label="อุปกรณ์ที่จับคู่">
      <header className="device-pairing-header">
        <Link2 size={18} />
        <h3>อุปกรณ์ที่จับคู่</h3>
      </header>

      <ul className="device-pairing-list">
        {paired.length === 0 && <li className="device-pairing-empty">ยังไม่มีอุปกรณ์ที่จับคู่</li>}
        {paired.map((d) => (
          <li key={d.id} className={d.revoked_at ? "device-pairing-item device-pairing-item-revoked" : "device-pairing-item"}>
            <div>
              <strong>{d.name}</strong>
              <small>{d.platform} · {d.revoked_at ? "ถูกยกเลิกการจับคู่" : "จับคู่แล้ว"}</small>
            </div>
            {!d.revoked_at && (
              <button type="button" className="device-pairing-revoke" onClick={() => void revoke(d)} aria-label={`ยกเลิก ${d.name}`}>
                <Trash2 size={15} />
              </button>
            )}
          </li>
        ))}
      </ul>

      {pairing.kind === "waiting" ? (
        <div className="device-pairing-code-box">
          <p>ใส่รหัสนี้บนมือถือของคุณ</p>
          <strong className="device-pairing-code">{pairing.code}</strong>
          <p className="device-pairing-countdown">
            หมดอายุใน {Math.floor(remainingMs / 60000)}:{String(Math.floor((remainingMs % 60000) / 1000)).padStart(2, "0")} นาที
          </p>
          <p className="device-pairing-hint">รอการยืนยันจากมือถือ…</p>
        </div>
      ) : (
        <button type="button" className="device-pairing-start" onClick={() => void startPairing()}>
          <RefreshCw size={15} /> จับคู่อุปกรณ์ใหม่
        </button>
      )}
      {pairing.kind === "confirmed" && (
        <p className="device-pairing-success">จับคู่กับ {pairing.peerName} สำเร็จ ✓</p>
      )}
      {pairing.kind === "error" && <p className="device-pairing-error">{pairing.message}</p>}
    </section>
  );
}
```

- [x] **Step 2: CSS** — `device-pairing-` prefix, big monospace 6-digit code display (letter-spacing), light values + `.theme-dark` overrides.
- [x] **Step 3:** Wire into `App.tsx` on the same settings surface as AccountLoginPanel (render below it).
- [x] **Step 4:** `npx tsc --noEmit` 0 · `npm run build` green.
- [x] **Step 5: Commit** — `feat(pairing): desktop pairing panel with code display, polling, and revoke`

---

### Task 8: Mobile pairing command (Rust) — replace the fake path

**Files:** Modify `src-tauri/src/mobile.rs`, `src-tauri/src/lib.rs` (handler list)

**Interfaces produced:** `mobile_pairing_complete(peer_device_id, name, endpoint, pairing_session_id)`; **removed:** `mobile_pair_desktop`.

- [x] **Step 1:** Read the current `mobile_pair_desktop` (mobile.rs ~line 800) and its tests. Write a failing test for the new command's inner function:

```rust
#[test]
fn pairing_complete_upserts_verified_row() {
    // reuse the existing genesis test-state helper used by mobile_pair_desktop's tests
    let state = test_state();
    pairing_complete_inner(&state, "cloud-dev-1", "FUNG Desktop", "192.168.1.20:8765", "sess-uuid-1").unwrap();
    // query paired_devices: id == "cloud-dev-1", trust_state == "paired",
    // pairing_proof_hash == "sess-uuid-1", endpoint == "192.168.1.20:8765"
}
```

- [x] **Step 2:** Run → FAIL. Implement `pairing_complete_inner` + `#[tauri::command] mobile_pairing_complete`: upsert Genesis `paired_devices` row keyed by `id = peer_device_id` with `name`, `endpoint` (may be empty string), `trust_state = "paired"`, `pairing_proof_hash = pairing_session_id`, `capabilities_json = "[]"`, timestamps — mirroring the row shape the old function wrote, minus the fake sha256 proof. Input validation: `peer_device_id` and `pairing_session_id` non-empty; name 1..=120 chars.
- [x] **Step 3:** Delete `mobile_pair_desktop` (function + its tests + handler registration) and register `mobile_pairing_complete`. Any other references (grep `mobile_pair_desktop` across repo — bridge.ts updated in Task 9; if bridge still references it after this task, that's expected mid-branch breakage ONLY if tsc fails — check: bridge.ts calls `invoke("mobile_pair_desktop")` as a string, tsc will NOT fail; note the dangling string for Task 9 in your report).
- [x] **Step 4:** Focused tests PASS → full `cargo test` → commit: `feat(pairing): replace unverified mobile_pair_desktop with verified mobile_pairing_complete`

---

### Task 9: Mobile DevicesScreen rework

**Files:** Modify `src/mobile/MobileApp.tsx` (DevicesScreen), `src/mobile/bridge.ts`, `src/mobile/model.ts`, `src/mobile/mobileStore.ts`

**Interfaces:** consumes `authFlow.ts`, `device_identity_ensure`, `mobile_pairing_complete`, `confirm_pairing` RPC, `localStorage["fung.device.id"]` (own id, set here for mobile).

- [x] **Step 1: model.ts** — extend `DeviceState.trustState` union with `"revoked"`; add optional `cloudDeviceId?: string` and `pairingSessionId?: string` fields.
- [x] **Step 2: mobileStore.ts** — replace `pairDevice(snapshot, name, endpoint)` with:

```typescript
export function upsertPairedDevice(
  snapshot: MobileSnapshot,
  device: { cloudDeviceId: string; name: string; endpoint: string; pairingSessionId: string },
): MobileSnapshot {
  const rest = snapshot.devices.filter((d) => d.cloudDeviceId !== device.cloudDeviceId);
  return {
    ...snapshot,
    devices: [
      ...rest,
      {
        id: device.cloudDeviceId,
        cloudDeviceId: device.cloudDeviceId,
        name: device.name,
        endpoint: device.endpoint,
        trustState: "paired",
        capabilities: [],
        pairingSessionId: device.pairingSessionId,
        lastSeenAt: new Date().toISOString(),
      },
    ],
  };
}

export function markDeviceRevoked(snapshot: MobileSnapshot, cloudDeviceId: string): MobileSnapshot {
  return {
    ...snapshot,
    devices: snapshot.devices.map((d) =>
      d.cloudDeviceId === cloudDeviceId ? { ...d, trustState: "revoked" } : d,
    ),
  };
}
```

Keep `removeDevice` as-is. Fix any existing callers of `pairDevice` (grep).

- [x] **Step 3: bridge.ts** — replace `pairDesktop` with:

```typescript
export async function pairingComplete(
  peerDeviceId: string,
  name: string,
  endpoint: string,
  pairingSessionId: string,
): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_pairing_complete", { peerDeviceId, name, endpoint, pairingSessionId });
}

export async function deviceIdentityEnsure(): Promise<{ fingerprint: string; created: boolean } | null> {
  if (!isTauri()) return null;
  return invoke("device_identity_ensure");
}
```

- [x] **Step 4: DevicesScreen rework** in `MobileApp.tsx`. Read the current DevicesScreen (~lines 463–505) first. New behavior (complete flow spec — adapt JSX to the file's existing sheet/list idioms and CSS classes; keep the visual container structure):
  1. **No session** → card with "เข้าสู่ระบบด้วย Google เพื่อจับคู่อุปกรณ์" button → `beginGoogleLogin()` + `listenForAuthCallback` (register listener once in the screen's mount effect). Note: NO loopback fallback on mobile (deep link is native there).
  2. **Session, not registered** → auto-register android device (same select-then-insert flow as Task 6's effect, `platform: "android"`, label default "FUNG Mobile") → store own id in `localStorage["fung.device.id"]`.
  3. **Paired list** from `snapshot.devices` with trust-state chips: จับคู่แล้ว / ไม่ตอบสนอง / ถูกยกเลิก. Revocation check on screen focus: `select id from devices in (cloudIds)` → missing → `markDeviceRevoked` + save snapshot.
  4. **"จับคู่กับ Desktop"** opens sheet: fetch desktops (`devices` where `platform = "windows"`, `revoked_at is null` — RLS scopes to own rows); radio list + optional "ที่อยู่ (ไม่บังคับ)" endpoint field (placeholder `192.168.1.20:8765`) + 6-digit code input (numeric, `inputMode="numeric"`, `maxLength 6`).
  5. Submit → find newest pending session: `from("pairing_sessions").select("id").eq("initiator_device_id", chosen.id).eq("status","pending").order("created_at",{ascending:false}).limit(1).maybeSingle()` — if none: "ยังไม่มีรหัสจาก Desktop — กด 'จับคู่อุปกรณ์ใหม่' บนเครื่องนั้นก่อน". Else `rpc("confirm_pairing", { p_session_id, p_code: code, p_responder_device_id: myDeviceId })`.
  6. Handle RPC result: `confirmed` → `bridge.pairingComplete(...)` + `upsertPairedDevice` + save + audit insert (`pairing_confirmed`) + close sheet; `wrong_code` → "รหัสไม่ถูกต้อง ลองใหม่"; `locked` → "ใส่รหัสผิดครบ 5 ครั้ง — สร้างรหัสใหม่บน Desktop"; `expired` → "รหัสหมดอายุ".
- [x] **Step 5:** `npx tsc --noEmit` 0 · `npm run build` green · `npm run test:mobile` still 4/4.
- [x] **Step 6: Commit** — `feat(pairing): mobile login gate, desktop discovery, and verified code entry`

---

### Task 10: Web Dashboard live devices tile + CI test hookup

**Files:** Modify `src/web/Dashboard.tsx`, `.github/workflows/ci.yml`

- [x] **Step 1:** In `Dashboard.tsx`, replace the placeholder "อุปกรณ์ที่จับคู่" tile with a live list: on load (inside the existing `load()`), also `from("devices").select("id, device_label, platform, last_seen_at").is("revoked_at", null).order("registered_at", { ascending: false })` (log error per existing pattern). Render rows (label + platform + last-seen relative time) inside the tile; per-row "ยกเลิก" button → `from("devices").delete().eq("id", id)` + audit insert (`device_revoked`) + reload. Keep tile styling classes; add minimal new classes in `Dashboard.css` with `.theme-dark` overrides if needed.
- [x] **Step 2:** Append `- run: npm run test:auth` to the CI frontend job (after `test:design-system`).
- [x] **Step 3:** `npx tsc --noEmit` 0 · `npm run build` green.
- [x] **Step 4: Commit** — `feat(pairing): live device list with revoke on web dashboard; run auth tests in CI`

---

## Controller Gate (after final review, before merge)

1. Apply migration `20260809000000_pairing_sessions.sql` to `nqnrvqnijzovkrhxslfp` (Supabase MCP `apply_migration`) — with Boss confirmation.
2. Boss: Supabase dashboard → Auth → URL Configuration → add `fung://auth/callback` (+ loopback pattern per Task 3 outcome).
3. Manual acceptance run (spec §14): desktop login E2E, pairing happy path, wrong-code ×5 → locked, expiry, revoke both directions.

## Self-Review

**Spec coverage:** §4 auth → Tasks 3,4,6; §5 identity → Task 2; §6 broker → Task 1; §7 flows → Tasks 7,9; §8 persistence → Tasks 5,8,9; §9 revocation → Tasks 7,9,10; §10 UI → 6,7,9,10; §13 security → constraints + Tasks 1,4; §14 testing → per-task steps + gate.
**Placeholders:** none — every code step has complete code or an exact behavioral contract with the pattern file named.
**Type consistency:** `PairedDeviceRow` (Task 5 Rust ⇄ Task 7 TS) field names match (serde snake_case default); `DeviceIdentity` (Task 2 ⇄ Tasks 6/9); `hashPairingCode` formula = SQL formula (Task 1 ⇄ Task 4 test vector); RPC arg names `p_session_id/p_code/p_responder_device_id` (Task 1 ⇄ Task 9).
**Known deviation from spec:** self device-id cache uses localStorage on BOTH surfaces (spec §8 said desktop `app_meta` SQLite) — simpler, consistent, and the value is non-secret; noted here as the governing decision.
