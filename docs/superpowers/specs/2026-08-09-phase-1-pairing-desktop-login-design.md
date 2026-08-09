# Phase 1: Device Pairing + Desktop/Mobile Login — Design Spec

| Field | Value |
|---|---|
| Date | 2026-08-09 |
| Status | draft — pending Boss review |
| Sub-projects | B (pairing) + D (desktop login) + E-core (mobile login, pulled in 2026-08-09) |
| Master plan | `2026-08-09-fung-master-implementation-plan.md` Phase 1 (REQ-B-01…09, + REQ-E-01 moved here) |
| Parent spec | `2026-08-08-auth-web-hybrid-subproject-a-design.md` (Sub-project A, shipped) |

## 1. Overview & Scope

**Goal:** A mobile device and a desktop can pair with each other through a verified 6-digit code brokered by Supabase, after both sign in with the same Google account. Pairing produces a durable, revocable trust relationship stored locally on both sides (cloud for handshake, local for runtime).

**In scope:**
- Google login on desktop Tauri app (system browser + `fung://` deep link, loopback fallback)
- Google login on mobile Tauri app (system browser + deep link — same machinery)
- Device identity: per-device keypair, fingerprint registered in `devices` table
- `pairing_sessions` table + `confirm_pairing` RPC (hash-verified, attempt-limited, 5-min expiry)
- Cloud discovery of the user's desktops + manual endpoint entry as fallback
- Replacement of the fake `mobile_pair_desktop` proof with verified pairing
- Revocation (both directions) + audit events
- UI: desktop pairing panel, mobile Devices tab rework, web Dashboard paired-devices tile

**Out of scope (parked):**
- FUNGWIRE tunnel, job delegation (Phase 2) — the `endpoint` field is stored but unused
- BYOM keys, tier policy (Phase 3)
- Cloud storage, account-settings unification on mobile (Phase 4)
- iOS

## 2. Locked Decisions

| Question | Decision | Date |
|---|---|---|
| Scope | Pairing only; D pulled into B | 2026-08-09 |
| Handshake | 6-digit code, generated + shown on desktop, entered on mobile | 2026-08-09 |
| Discovery | Cloud primary (list user's desktops), manual endpoint fallback | 2026-08-09 |
| Verification | Via Supabase (`pairing_sessions` + RPC) | 2026-08-09 |
| Desktop OAuth | System browser + deep link | 2026-08-09 |
| Data model | Hybrid: cloud for handshake, local for runtime | 2026-08-09 |
| Pairing broker | Approach 1: dedicated `pairing_sessions` table (approved via master plan REQ-B-03) | 2026-08-09 |
| Mobile auth | Pulled into Phase 1 — cloud-brokered verification requires mobile session (RLS) | 2026-08-09 |

## 3. Architecture Overview

```
┌────────────── Desktop (Tauri, Windows) ──────────────┐
│ React: LoginPanel / DevicePairingPanel               │
│   └─ src/lib/authFlow.ts  (shared PKCE via           │
│      supabase-js signInWithOAuth + deep-link event)  │
│ Rust: deep-link plugin, device_identity_* commands   │
│   └─ ed25519 keypair → private key in OS keyring     │
│   └─ paired_devices (desktop SQLite WAL)             │
└──────────┬───────────────────────────────────────────┘
           │ anon-key HTTPS (RLS-scoped)
   ┌───────▼──────── Supabase ────────────────┐
   │ devices (existing) — one row per device  │
   │ pairing_sessions (new) — 5-min TTL       │
   │ confirm_pairing() RPC — atomic verify    │
   │ device_audit_events (new)                │
   └───────▲──────────────────────────────────┘
           │ anon-key HTTPS (RLS-scoped)
┌──────────┴──────── Mobile (Tauri, Android) ──────────┐
│ React: DevicesScreen rework (discover → code entry)  │
│   └─ same src/lib/authFlow.ts                        │
│ Rust: deep-link plugin (intent-filter),              │
│   mobile_pair_desktop REPLACED by verified path      │
│   └─ paired_devices (GenesisBlockDB) + cloud link    │
└──────────────────────────────────────────────────────┘
```

Key simplification: **auth runs in TypeScript on both surfaces** using the existing `supabase-js` client (`src/lib/supabase.ts` from Sub-project A). Rust's role is limited to: registering the deep-link scheme, forwarding the callback URL to the webview, device keypair + secure storage, and local pairing persistence. No Rust-side OAuth implementation.

## 4. Auth Flow (both surfaces)

New shared module `src/lib/authFlow.ts`:

1. `signInWithGoogle()` calls `supabase.auth.signInWithOAuth({ provider: "google", options: { redirectTo: "fung://auth/callback", skipBrowserRedirect: true } })` → gets the authorize URL.
2. Opens the URL in the **system browser** via `@tauri-apps/plugin-opener` (never in the webview — Google blocks embedded webviews).
3. User completes Google login; Supabase redirects to `fung://auth/callback?code=...`.
4. The OS routes the deep link to the app (tauri-plugin-deep-link). Rust forwards the URL to the webview as an event.
5. `authFlow.ts` handles the event: `supabase.auth.exchangeCodeForSession(code)` (PKCE verifier lives in supabase-js storage). Session persisted by supabase-js.
6. UI updates via `onAuthStateChange` (same pattern as Sub-project A's `AuthGuard`).

**Desktop specifics (Windows):** `fung://` scheme registered by `tauri-plugin-deep-link` at install/first-run. **Fallback (risk R4):** if the deep link does not arrive within 120 s, the UI offers "ลองวิธีสำรอง" which re-runs the flow with `redirectTo: http://127.0.0.1:{port}/auth/callback` served by a one-shot loopback listener (pattern proven in `zoom_sync.rs`). Both redirect URLs must be whitelisted in the Supabase dashboard (§12).

**Mobile specifics (Android):** the deep-link plugin's manifest intent-filter registers `fung://`. The browser flow uses the system browser via the opener plugin. supabase-js persists the session in the webview's localStorage (app-private on Android). Keystore-backed hardening is deferred (§17).

**Session storage note:** supabase-js sessions live in webview localStorage on both surfaces. This is app-private storage on both platforms. Auth tokens do NOT go into the OS keyring in this phase; the keyring holds only the device private key. (Keyring-backed session storage is a hardening item, §17.)

## 5. Device Identity

New Rust module `src-tauri/src/device_identity.rs`:

- `device_identity_ensure() -> DeviceIdentity { fingerprint: String, created: bool }`
  - Generates an ed25519 keypair on first call (new deps: `ed25519-dalek = "2"`, `rand = "0.8"`).
  - Private key: Phase 1 stores the device private key as a base64 file in the app-data dir on both platforms; OS keyring / Android Keystore hardening is deferred (backlog).
  - Fingerprint = lowercase hex `sha256(public_key_bytes)` (64 chars — satisfies the existing `devices_fingerprint_length` 16–255 constraint).
- The keypair is **not used for signing in Phase 1**; it exists so the `devices` row is anchored to hardware from day one, and Phase 2's tunnel handshake signs with it.

**Registration (TypeScript, after login):** upsert into `devices` via supabase-js:
`{ user_id, device_label, platform: "windows" | "android", public_key_fingerprint }` — the existing unique constraint `(user_id, public_key_fingerprint)` makes this idempotent. The returned `devices.id` is cached locally (desktop: SQLite `app_meta`; mobile: Genesis `notes`-adjacent meta — see §8). `last_seen_at` is updated on each app start (existing update grant covers it).

Web Dashboard rows (from Sub-project A) have no fingerprint — web is NOT a pairable device in this design; only `windows`/`android` platform rows appear in pairing UIs.

## 6. Pairing Broker (Supabase)

Migration `supabase/migrations/20260809000000_pairing_sessions.sql`:

```sql
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
```

**Code handling:** the 6-digit code is generated on the desktop (crypto RNG, 000000–999999, zero-padded). Only `code_hash = encode(sha256((session_id || ':' || code)::bytea), 'hex')` is stored — the session id acts as salt, so identical codes hash differently. The plaintext code exists only on the desktop screen and in the user's head.

**Atomic confirmation** — RPC in the same migration:

```sql
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
```

`security invoker` — RLS still applies, so only the session owner's authenticated devices can call it. Expired-session cleanup: sessions are tiny; a `delete where expires_at < now() - interval '1 day'` statement runs opportunistically whenever a new session is created (no pg_cron dependency).

**Audit** — same migration:

```sql
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

Event types written by clients: `pairing_session_created`, `pairing_confirmed`, `pairing_locked`, `pairing_expired`, `device_revoked`, `device_registered`.

## 7. Pairing Flows

**Desktop initiates (happy path):**
1. Desktop (logged in, device registered) → DevicePairingPanel → "จับคู่อุปกรณ์ใหม่".
2. Generate code, insert `pairing_sessions` row, show code + 5-minute countdown + "รอการยืนยันจากมือถือ…".
3. Desktop polls its session row every 2 s while the dialog is open (no Realtime dependency).
4. Mobile (logged in) → Devices tab → "จับคู่กับ Desktop" → sees list of the user's desktop devices (query: `devices` where `platform = 'windows'` and `revoked_at is null`) → picks one → enters the 6-digit code.
5. Mobile calls `confirm_pairing(session_id, code, my_device_id)`. The session to target: mobile queries the newest `pending` session whose `initiator_device_id` = the chosen desktop.
6. On `'confirmed'`: both sides persist locally (§8) and write audit events. Desktop dialog flips to "จับคู่สำเร็จ ✓"; mobile shows the desktop as paired.
7. On `'wrong_code'` → "รหัสไม่ถูกต้อง (เหลือ N ครั้ง)"; `'locked'` → session dead, desktop must create a new one; `'expired'` → prompt desktop to regenerate.

**Manual endpoint fallback:** the mobile sheet keeps an "ใส่ที่อยู่เอง" field (IP:port) for when cloud discovery shows nothing (e.g. desktop registered under a different label). The endpoint is stored in the local pairing record for Phase 2's tunnel. **Verification always requires Supabase** — documented limitation; there is no offline pairing in Phase 1.

## 8. Local Persistence (hybrid model)

**Desktop paired devices are stored in a dedicated `paired_devices.db` SQLite file, NOT the main WAL db, because GenesisBlockDB defines its own `paired_devices` table and the legacy importer matches tables by name.** Concretely: `import_legacy_sqlite` walks a source SQLite file and imports tables by name match; if the desktop's own pairing table lived in the same WAL db (or shared a file with a GenesisBlockDB-schema database) under the name `paired_devices`, the importer would treat it as GenesisBlockDB data and attempt to import/merge it incorrectly. To avoid that collision, the desktop pairing table lives in its own file, created idempotently at startup:

```sql
CREATE TABLE IF NOT EXISTS paired_devices (
  id TEXT PRIMARY KEY,              -- cloud devices.id of the PEER (mobile)
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  paired_at TEXT NOT NULL,
  revoked_at TEXT,
  pairing_session_id TEXT NOT NULL
);
```

**Mobile (GenesisBlockDB `paired_devices`, existing table):** rows now written ONLY after `confirm_pairing` returns `'confirmed'`. Column mapping: `id` = cloud `devices.id` of the peer desktop; `endpoint` = manual/entered endpoint or `''`; `trust_state` = `'paired'`; `pairing_proof_hash` = the `pairing_session_id` (repurposed as the verified-session reference — the old fake sha256 proof is gone); `capabilities_json` = `'[]'` until Phase 2 capability exchange. The Tauri command `mobile_pair_desktop(name, endpoint, pairing_code)` is **replaced** by `mobile_pairing_complete(peer_device_id, name, endpoint, pairing_session_id)` — invoked by the frontend only after RPC confirmation. The unverified path is deleted.

**Cached self-identity:** each side stores its own cloud `devices.id` + fingerprint (desktop: a small `app_meta` key-value table, same startup-idempotent pattern; mobile: Genesis `mobile_recording_checkpoints`-style meta row is NOT abused — instead the value goes in localStorage via the existing snapshot store, which already persists device state).

## 9. Revocation

- **From web Dashboard or desktop:** delete the peer's `devices` row (delete grant exists; FK cascades clean up `pairing_sessions`), write `device_revoked` audit event, flip local `paired_devices.revoked_at` / `trust_state = 'revoked'`.
- **Propagation:** on every app start (and on opening the Devices/pairing UI), each side re-checks that (a) its own `devices` row still exists and (b) each locally-paired peer's row still exists. A missing row → local record marked revoked, UI shows "ถูกยกเลิกการจับคู่". No push channel in Phase 1 — one-refresh convergence, matching the master plan's acceptance criterion.
- Local unpair (mobile "ลบอุปกรณ์") deletes the local record AND the cloud `pairing_sessions` linkage is left to expire; the peer discovers on next refresh.

## 10. UI Inventory

| Surface | Component | Change |
|---|---|---|
| Desktop | `src/components/AccountLoginPanel.tsx` (new) | Google login state, device label editing, logout — follows `ExternalAccountPanel` visual pattern |
| Desktop | `src/components/DevicePairingPanel.tsx` (new) | Paired list, "จับคู่อุปกรณ์ใหม่" dialog (code + countdown + status), revoke |
| Desktop | `src/App.tsx` | Two new settings entries wiring the panels (same conditional-render pattern as `TtsProviderPanel`) |
| Mobile | `src/mobile/MobileApp.tsx` DevicesScreen | Rework: login gate → discovery list → code entry sheet → paired states (paired/unreachable/revoked) |
| Mobile | `src/mobile/bridge.ts` | `pairDesktop` replaced by `pairingComplete`; add deep-link event listener hookup |
| Web | `src/web/Dashboard.tsx` | "อุปกรณ์ที่จับคู่" tile → live list from `devices` + revoke button |
| Shared | `src/lib/authFlow.ts` (new) | signInWithGoogle / deep-link handling / loopback fallback trigger |

All UI labels Thai; identifiers English; CSS = hardcoded light + `.theme-dark` overrides; named exports only (Sub-project A conventions carry over as global constraints).

## 11. New Dependencies

| Dep | Where | Why |
|---|---|---|
| `tauri-plugin-deep-link` v2 | Cargo + capability config | `fung://` scheme, both platforms |
| `ed25519-dalek = "2"`, `rand = "0.8"` | Cargo | device keypair |
| (none) | npm | supabase-js + opener plugin already present |

## 12. Manual Configuration (Boss)

1. Supabase dashboard → Auth → URL Configuration → add redirect URLs: `fung://auth/callback` and `http://127.0.0.1:*/auth/callback` (or the specific fallback port range chosen at plan time).
2. No Google Cloud Console change (same OAuth client via Supabase).
3. Apply migration `20260809000000_pairing_sessions.sql` to project `nqnrvqnijzovkrhxslfp`.

## 13. Security Checklist

| Item | Mechanism |
|---|---|
| Code never stored/transmitted in plaintext to DB | sha256(session_id ‖ ':' ‖ code), salt = session id |
| Brute force | 5 attempts → `locked`; 6-digit space + 5-min TTL |
| Replay | session single-use (`status` transitions one-way), TTL |
| Cross-user access | RLS user-scoped on all three tables; RPC is `security invoker` |
| Key material | Phase 1 stores the device private key as a base64 file in the app-data dir on both platforms; OS keyring / Android Keystore hardening is deferred (backlog). Never serialized to any DB or log. |
| Anon key only | unchanged — no service role anywhere client-side |
| Webview OAuth ban | system browser only (opener plugin) |

## 14. Testing Strategy

- **Rust:** unit tests for keypair generate/load round-trip, fingerprint format, desktop `paired_devices` CRUD, `mobile_pairing_complete` (Genesis test schema).
- **SQL:** the migration file is exercised by a plan-time test script hitting a shadow schema — at minimum, hash-match logic is mirrored in a Rust unit test to guarantee the client-side hash computation matches the SQL expression (same input vectors).
- **TS:** authFlow unit-testable pure parts (URL parsing of deep-link callback) via `node --test` (pattern exists in `tests/`).
- **Manual acceptance (per master plan):** fresh desktop login E2E; pairing happy path; wrong-code ×5 lock; 5-min expiry; revoke propagation both directions.

## 15. Risks & Open Items (carried into plan)

1. **R4 deep link on Windows** — mitigated by loopback fallback; spike task first in Sprint S1.
2. **Genesis additive columns** — avoided: no schema change needed (repurposed `pairing_proof_hash` + existing columns). If Phase 2 needs `cloud_device_id` as a real column, that's a Genesis migration task then.
3. **`devices` update grant excludes `revoked_at`** — revocation uses row DELETE (grant exists). If soft-revoke is later preferred, a migration must extend the column grant.
4. **supabase-js session in localStorage** — accepted for Phase 1; keyring/Keystore-backed session storage deferred (hardening backlog).
5. **Polling (2 s) instead of Realtime** — deliberate: avoids a Realtime dependency (master plan R7); revisit only if UX demands.

## 16. File Inventory

**New:** `src/lib/authFlow.ts` · `src/components/AccountLoginPanel.tsx` (+css) · `src/components/DevicePairingPanel.tsx` (+css) · `src-tauri/src/device_identity.rs` · `supabase/migrations/20260809000000_pairing_sessions.sql`
**Modified:** `src/App.tsx` · `src/mobile/MobileApp.tsx` · `src/mobile/bridge.ts` · `src/mobile/model.ts` · `src/web/Dashboard.tsx` · `src-tauri/src/lib.rs` (commands + deep-link init + sqlite table) · `src-tauri/src/mobile.rs` (replace pair command) · `src-tauri/Cargo.toml` · `src-tauri/tauri.conf.json` / capabilities (deep-link) · `schemas/sqlite-wal-v1.sql`
**Deleted behavior:** unverified `mobile_pair_desktop` path.
