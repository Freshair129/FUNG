---
version: "0.1.0b"
created_at: "2026-08-24T08:24:41+07:00,Luna 5.6"
last_update: "2026-08-24T08:24:41+07:00,Luna 5.6"
status: "unstable"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S2 native account and enrollment boundary"
---

# W1-A-F4-S2 — Luna Native Enrollment Report

## Status

The approved local implementation commit is `1775143a1925e2b4ce25eb2539e3b2ca9474172b`.
Its seven implementation/test paths are clean after commit. The required report
is intentionally committed separately.

Local unit, contract, type-check, build, formatting, diff, static, and secret
scans are green. This report does not claim end-to-end account login,
production enrollment, Terra approval, deployment, or release readiness.

Two follow-up gaps were identified from the committed code but were not changed
after the implementation commit because the worker was instructed to stop
implementation edits:

1. The committed native account URL carries provider, redirect, and state, but
   not a native PKCE challenge/verifier handoff. The existing Supabase client is
   configured for PKCE and `exchangeCodeForSession` requires a stored verifier;
   cached-consent browser login was not verified.
2. The pending-enrollment proof signs a canonical challenge, but the returned
   opaque signature does not carry the issued-at/nonce challenge metadata needed
   for an independent Boss verification ceremony. The Edge endpoint was not
   changed to compensate for this.

These are implementation follow-ups within the already approved S2 files, not
permission to widen scope. They keep this report `unstable` rather than a full
S2 acceptance claim.

## Root-cause mapping

| Finding | Root cause | Local correction in `1775143` |
|---|---|---|
| Browser-controlled OAuth authority | WebView generated the provider URL and called a URL-opening command; the old listener accepted an unbound first request. | Native constructs the configured Google URL, binds `127.0.0.1:0`, owns state/expiry, opens the browser, and emits only a typed request-bound callback. |
| Callback confusion/replay | Callback parsing was not bound to the assigned listener, state, and exact query shape. | `AuthRequestRegistry` and the strict frontend parser reject wrong host/port/path/state, duplicates, additional parameters, timeout, cancellation, and replay. |
| Account/device authority mixing | `AccountLoginPanel` directly mutated `devices` and `device_audit_events` from the authenticated WebView. | The panel obtains a native key proof, reads server truth, and calls only the server-owned pending-enrollment Edge action; it has no device/audit mutation. |
| Plaintext identity migration risk | Keyring and legacy-file lifecycle had no injectable behavior-level failure seam. | Desktop migration uses an `IdentityBackend`, verifies keyring readback before legacy removal, and fails closed on conflict/write/readback/remove failure. |
| Missing behavioral migration evidence | Static source matching could not prove ordering or failure preservation. | `FakeIdentityBackend` records reads, writes, readback, and delete attempts in Rust tests. |

## Exact paths and commits

Implementation commit `1775143a1925e2b4ce25eb2539e3b2ca9474172b` contains only:

- `src-tauri/src/device_identity.rs`
- `src-tauri/src/native_auth.rs`
- `src-tauri/src/lib.rs`
- `src/lib/authFlow.ts`
- `src/lib/authParse.ts`
- `src/components/AccountLoginPanel.tsx`
- `tests/authFlow.test.mjs`

This report is the only path intended for the separate report commit:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s2-native-enrollment-luna-report.md`

Pre-existing dirty/untracked paths were not staged or committed: the desktop
progress/phase-plan/S1 addendum docs, `src/components/BackupPanel.tsx`,
`src/web/AccountSettings.tsx`, `supabase/README.md`, the existing RCA,
`.tmp-transcript/`, the Luna/Terra workflow and Recording2 plan docs, both
approved amendment docs, and `src/components/GoogleDrivePanel.css`.

## TDD evidence

### RED before implementation

- `npm run test:auth` — 4 passed, 2 failed. The old parser did not reject the
  wrong listener/query cases as `invalid_callback`, and the old flow did not
  contain the native begin command.
- `cargo test -j 1 native_auth::tests::native_login_registry --no-fail-fast`
  — failed to compile the newly added RED tests because the implementation
  symbols were intentionally absent: `AuthRequestRegistry`,
  `AUTH_LOGIN_REQUEST_TTL`, `FakeIdentityBackend`, and
  `migrate_identity_with_backend`.

### GREEN after implementation

- `npm run test:auth` — 6 passed, 0 failed.
- Native registry tests — 3 passed, 0 failed.
- Device identity/fake-keyring tests — 7 passed, 0 failed.
- `cargo test -j 1 --no-fail-fast` — 383 library tests passed, 0 failed; all
  binary targets and doc-tests passed with zero failures.
- `npx tsc --noEmit` — passed.
- `npm run build` — passed; Vite production build completed.
- `npm run test:google-drive` — 5 passed, 0 failed.
- `npm run test:device-reconcile` — 6 passed, 0 failed.
- `npm run test:backup-flow` — 17 passed, 0 failed.
- `rustfmt --edition 2021 --check src/device_identity.rs src/native_auth.rs
  src/lib.rs` — passed.
- `git diff --check` — passed for the implementation audit.
- Static AccountLoginPanel device/audit mutation scan — no matches.
- Obsolete arbitrary-opener/old-loopback scan — no matches in committed native
  flow/registration paths.
- Changed-path secret scan — no prohibited credential/private-key patterns.

The GREEN evidence is local-only. No real provider callback or live Supabase
enrollment was run; the PKCE and proof-envelope gaps above remain open.

## Acceptance mapping

| S2 requirement | Result | Evidence |
|---|---|---|
| RED behavioral tests precede implementation | PASS | Auth RED and Rust compile RED above; implementation commit follows them. |
| Native owns URL, listener, state, lifecycle, and opener | PASS locally | Native commands and registration in `native_auth.rs`/`lib.rs`; static flow scan green. End-to-end PKCE handoff remains open. |
| One exact pending request/callback with terminal rejection | PASS locally | 3 native registry tests and 6 strict parser tests. |
| Account session remains separate from device trust | PASS locally | Panel submits only `device-enrollment` `pending`; no `drive_trusted` mutation exists in the panel. |
| No frontend device/audit INSERT/UPDATE/DELETE | PASS | Static scan and auth test pass. |
| OS-keyring Ed25519 identity and canonical proof | PARTIAL | Keyring-backed native command and canonical signature are present; proof metadata/envelope verification was not completed. |
| Migration write/readback/compare/remove ordering | PASS locally | 7 Rust identity tests, including fake write/readback/remove failures and event ordering. |
| Behavioral fake-keyring seam | PASS | `FakeIdentityBackend` is exercised by behavior-level Rust tests. |
| Android/FUNGWIRE/pairing excluded | PASS for this commit | No out-of-scope paths were staged or committed. |
| No live staging/deploy/push/merge/PR/external message | PASS | None were performed. |

## External gates remaining

- Independent Terra S2 security review/re-review. The supplied final S1 Terra
  report was local-gate PASS with Terra `WARN` at report commit `87f2895`.
- Supabase staging deployment, schema/RLS/grant/function preflight, and live
  pending-enrollment/Boss approval evidence.
- Real Google consent, callback, PKCE exchange, Drive authorization, and
  provider upload/download/refresh/revoke.
- Clean-install Windows OS-keyring migration and legacy-file removal proof.
- Physical Android/FUNGWIRE delegation, production signing, release, merge,
  and promotion gates.

No external gate is represented as complete by this local report.

## Version Diff

- `new -> 0.1.0b`: recorded the bounded S2 Luna implementation, local evidence,
  known follow-up gaps, and external-gate boundary.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | unstable | S2 native enrollment local implementation report | `1775143` | Luna 5.6 |
