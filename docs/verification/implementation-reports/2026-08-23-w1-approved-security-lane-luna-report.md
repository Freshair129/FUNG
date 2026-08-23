---
version: "0.1.0b"
created_at: "2026-08-23T18:00:00+07:00,Luna 5.6"
last_update: "2026-08-23T18:00:00+07:00,Luna 5.6"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "FUNG W1-A-F2 approved security lane"
---

# W1-A-F2 — Luna Approved Security Lane Report

## Status

`DONE_WITH_CONCERNS`

The approved revised Design A lane is implemented and locally verified. The
implementation is fail-closed for native authorization, OS keyring identity
migration, per-operation Drive capabilities, OAuth cancellation, restore
intent, and trusted URL opening. No deployment, push, merge, pull request, or
external message was performed.

## Root-cause map

| Defect | Root cause | Implemented prevention | Evidence |
|---|---|---|---|
| GD-P0-01 / AC-GDA-01..06 | Drive commands accepted frontend `user_id`, `device_id`, and client-side authority; native code had no server-derived operation context. | Added memory-only, non-serializable `AuthorizedDriveContext`; native signs a canonical request with the OS-keyring device key; `google-drive-authorize` derives user/device/connection/scope and every command requests one operation. | `native_auth.rs`, `google-drive-authorize/index.ts`, 5/5 contract tests, full Rust suite. |
| GD-P1-02 / AC-GDA-09 | OAuth cancellation was an independent flag checked before exchange and could race refresh-token persistence. | Added `OAuthTerminalState`; cancellation is linearized against exchange and `begin_commit()` immediately before keyring persistence. | Two OAuth terminal interleaving tests passed; full Rust 377/377. |
| GD-P1-03 / AC-GDA-10 | Generic frontend opener capability allowed arbitrary URL authority. | Removed `opener:allow-open-url`; frontend invokes only native trusted URL commands; native validates Supabase origin, provider, redirect, and configured account portal. | Native URL tests and security contract test passed. |

## Implementation commit

- SHA: `7d6c045c1e6677fd133684945db9719cdef75435`
- Message: `fix: enforce native Google Drive authorization lane`
- Parent: `617eba0172b91205e7ed4cd15c976eebc16b9858` (not amended).
- Report artifact: committed separately after this report was written so the
  implementation SHA remains exact; its final SHA is included in the handoff.

## Exact implementation paths in the implementation commit

1. `.env.example`
2. `src-tauri/capabilities/default.json`
3. `src-tauri/src/backup.rs`
4. `src-tauri/src/device_identity.rs`
5. `src-tauri/src/drive_oauth.rs`
6. `src-tauri/src/lib.rs`
7. `src-tauri/src/native_auth.rs`
8. `src/components/AccountLoginPanel.tsx`
9. `src/components/GoogleDrivePanel.tsx`
10. `src/lib/authFlow.ts`
11. `src/lib/googleDriveFlow.ts`
12. `supabase/functions/google-drive-authorize/index.ts`
13. `supabase/functions/google-drive-metadata/index.ts`
14. `tests/googleDriveContract.test.mjs`

No Cargo/package dependency or authoritative specification file required a
change. The report path is intentionally committed separately.

## Acceptance evidence

| Criterion | Local result | Evidence / boundary |
|---|---|---|
| AC-GDA-01 | PASS | Drive IPC no longer accepts frontend user/device/client authority; native context is the only authority. |
| AC-GDA-02 | PASS locally | Session proof, device ownership, public-key match, and non-revocation are checked in the native authorization function before Drive token keyring/provider operations. Live deployed-function behavior remains external. |
| AC-GDA-03 | PASS locally | Ed25519 PoP, fingerprint digest, canonical operation/timestamp/nonce, expiry, and replay checks are implemented; forged/mismatched responses fail closed. |
| AC-GDA-04 | PASS locally | Provider connection is server-derived, user-owned, active, and exact-scope before backup operations. |
| AC-GDA-05 | PASS locally | `backup.read`, `backup.write`, and `backup.restore` are independently requested and operation-bound. |
| AC-GDA-06 | PASS | Context has no `Serialize`, `Clone`, or `Debug`, validates expiry/operation/connection, and is created per native invocation. |
| AC-GDA-07 | PASS | Restore requires a native archive/target-bound UUID intent, consumes it before provider/keyring access, and cannot replay after consumption or target change. |
| AC-GDA-08 | PASS locally | Browser metadata rejects completion/revocation authority events; only the native PoP path can activate/revoke the Drive connection. |
| AC-GDA-09 | PASS | OAuth callback, exchange, terminal cancellation, and keyring commit transitions are linearized; cancellation interleaving tests pass. |
| AC-GDA-10 | PASS | No frontend arbitrary opener remains in the approved auth/Drive flows; the generic opener capability was removed and native URL validation rejects untrusted origins/providers/redirects. |
| AC-GDA-11 | PASS | Existing crypto, digest, appDataFolder, archive contract, and clean restore tests remain green in the full Rust suite. |
| AC-GDA-12 | PASS locally | Added-diff secret and logging scans are clear; refresh/access tokens and recovery phrases are not persisted in Supabase DTOs/logs. |

## Test-first evidence

Before implementation, the new security contract tests were deliberately run
in RED state:

- `npm run test:google-drive`: 3 passed, 2 failed.
- Expected failures were the absent native authorization module and absent
  `device_identity_keyring` migration contract.

After implementation, the same tests were GREEN at 5/5.

## Verification

| Command/check | Result |
|---|---|
| `npm run test:google-drive` | PASS, 5/5 |
| `npm run test:auth` | PASS, 5/5 |
| `npm run build` | PASS, `tsc` and Vite production build; 1764 modules transformed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS; two existing fixture-helper dead-code warnings (`ensure_identity_in_dir`, `public_key_b64_in_dir`) |
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS, 377/377 Rust tests; 0 doc-tests |
| `deno fmt --check` on both Drive functions | PASS |
| `deno check --node-modules-dir=auto` on both Drive functions | PASS |
| `git diff --check` and staged diff check | PASS |
| Added-diff prohibited-secret scan | CLEAR |
| Added-diff secret-logging scan | CLEAR |
| Staged implementation path audit | PASS, exactly the 14 paths listed above |

## External gates and concerns

1. `google-drive-authorize` and the metadata function were implemented but not
   deployed, as explicitly required. Live Supabase Auth, RLS/grants, the
   `devices.public_key` migration, Edge runtime, and production replay behavior
   remain externally gated.
2. Google OAuth client, consent, loopback redirect, real consent/upload/list/
   download/refresh/revoke, clean-install migration, and Windows Credential
   Manager readback require real environment/device verification.
3. Replay protection uses a short-lived Edge isolate cache plus a durable audit
   correlation lookup. Cross-region concurrent replay behavior still needs
   production control-plane verification; no speculative schema migration was
   added outside the allowlist.
4. Deno verification generated an untracked `deno.lock`; it is outside the
   allowlist and was deliberately left uncommitted. All unrelated dirty and
   untracked paths remain preserved.

## Version Diff

- `new -> 0.1.0b`: recorded the approved W1-A-F2 native authorization,
  keyring-migration, cancellation, restore-intent, and trusted-opener lane.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | need review | W1-A-F2 security lane locally complete with 377/377 Rust tests | `7d6c045` | Luna 5.6 |
