---
version: "0.1.0b"
created_at: "2026-08-24T08:44:47+07:00,Terra"
last_update: "2026-08-24T08:44:47+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Independent W1-A-F4-S2 Terra review"
  base_commit: "87f28957d35bec2511b9695cfdfacccd075a37d4"
  implementation_commit: "1775143a1925e2b4ce25eb2539e3b2ca9474172b"
  luna_report_commit: "545f111d41336b762765c53bf2d57a892934cd5e"
---

# W1-A-F4-S2 — Independent Terra Security Review

## Verdict

**FAIL — local hard gates 1 and 5 fail.** The implementation must not be
integrated, deployed, pushed, merged, or promoted. These failures are local
implementation defects, not external Google, Supabase, Boss-ceremony, or
release evidence gaps.

I independently reviewed implementation `1775143` against base `87f2895`, the
two approved Google Drive amendments, the final S1 Terra report, and Luna's
report at `545f111`. I did not modify implementation, prior reports, or any
pre-existing dirty/untracked path. No deployment, migration, push, merge, PR,
deletion, or external message was performed.

## Blocking findings

| ID | Priority | Approved hard gate | Result and evidence |
|---|---|---|---|
| P0-01 | P0 | 1 — native owns the complete login URL and lifecycle | **FAIL.** Native binds the exact listener, state, and opener, but `build_google_auth_url` appends only `provider`, `redirect_to`, and `state` (`src-tauri/src/native_auth.rs:525-536`) before native opens that URL (`:704-733`). It creates neither a PKCE `code_challenge` nor a native `code_verifier`. The WebView client is configured with `flowType: "pkce"` (`src/lib/supabase.ts:12-14`) and later performs `exchangeCodeForSession` itself (`src/lib/authFlow.ts:57-83`). A static audit found native `code_challenge=False`, native `code_verifier=False`, WebView PKCE client/exchange both `True`. Thus the native request cannot carry an S256 challenge tied to a native verifier, and the browser-side exchange has no verifier created by this flow. This is exactly Luna's disclosed native-PKCE-handoff gap; it fails hard gate 1 rather than becoming an external-provider warning. |
| P0-02 | P0 | 5 — narrowly typed, native-signed enrollment/rebind proof | **FAIL.** Native signs a canonical pending-enrollment message containing issued-at, expiry, and nonce (`src-tauri/src/device_identity.rs:257-269`), but its public `NativeEnrollmentProof` exposes only public key, fingerprint, opaque proof, and expiry (`:231-238,300-305`). The WebView sends only the opaque proof with key material (`src/components/AccountLoginPanel.tsx:85-93`), not the signed issued-at or nonce. The current Edge handler accepts `nativeProof` solely on a 1..8192-byte length check and forwards it to the RPC (`supabase/functions/device-enrollment/index.ts:149-163`); it has neither those fields nor Ed25519 verification. Therefore neither the server nor a Boss reviewer can reconstruct the canonical bytes, verify possession, validate the proof's original expiry, or bind/replay-check its nonce. This violates hard gate 5 and AC-GDA2-03/D-GDA2-01 locally. It is exactly Luna's proof-envelope gap, not an external ceremony gap. |
| P1-01 | P1 | Test adequacy for the two P0 gates | **WARN, non-blocking only because the P0 defects already fail the review.** `npm run test:auth` passes 6/6, but its source assertions only require the frontend to call `auth_begin_google_login` and not `signInWithOAuth`/arbitrary opener (`tests/authFlow.test.mjs:45-53`); they do not assert a native S256 challenge/verifier handoff. `npm run test:google-drive` passes 5/5, but its PKCE assertion targets the separate Drive connection flow in `drive_oauth.rs` (`tests/googleDriveContract.test.mjs:10-18`), not account login. No test verifies the enrollment proof envelope or Edge signature verification. |
| P2-01 | P2 | Independent Rust execution | **Verification limitation.** `cargo test -j 1 native_auth::tests --no-fail-fast` was started in a clean detached `1775143` worktree. It remained in first-time Tauri dependency compilation for approximately 4.5 minutes and had not reached any test when the user-requested bounded wait expired; it was stopped rather than allowed to run indefinitely. This does not downgrade either source-proven P0 failure to an external gap. |

## Hard-gate ledger

| Gate | Status | Independent evidence |
|---|---|---|
| 1. Native-owned URL/origin/port/state/listener/opener/lifecycle/callback | **FAIL** | P0-01: all listener controls are native, but the native-owned URL omits the required PKCE challenge/verifier handoff. |
| 2. Exact callback validation | Source-supported; Rust execution not completed | The registry checks scheme, `127.0.0.1`, assigned port/path/state, duplicate/additional parameters, expiry, cancellation, and one-shot consumption in `native_auth.rs:128-224`; `npm run test:auth` passed its strict parser cases 6/6. |
| 3. One request/listener/callback across terminal races | Source-supported; Rust execution not completed | A mutex-backed single pending request and terminal consumption are visible in `native_auth.rs:47-224,605-742`. |
| 4. AccountLoginPanel cannot mutate/promote device authority | **PASS locally** | The panel contains no direct `devices` or `device_audit_events` insert/update/delete; the passing auth static test asserts this. It requests only `pending`; it does not write `drive_trusted`. |
| 5. Typed and verifiable native enrollment/rebind proof | **FAIL** | P0-02: signing is native and narrowly invoked, but the signed claims required to verify it never cross the typed boundary and the Edge function does not verify the signature. |
| 6. Keyring migration ordering behavioral tests | Source-supported; Rust execution not completed | The committed fake backend/test seam remains in `device_identity.rs`; the bounded Rust test could not complete. No contrary source evidence found. |
| 7. Session, device trust, and database runtime login stay distinct | **PASS by source review** | The UI uses the Supabase session for the pending request and only reads device authority state; no database-runtime-login change exists in the seven-path diff. |
| 8. Allowed paths and dirty-work preservation | **PASS** | Exact seven implementation paths are within the approved write set; `git diff --check 87f2895..1775143` passed and the pre-review dirty/untracked snapshot was unchanged. |

## Commands and results

| Command / check | Result |
|---|---|
| Isolated checkout | **PASS** — detached worktree at exact `1775143`; `npm ci` completed with 0 vulnerabilities. |
| `npm run test:auth` | **PASS — 6/6.** Coverage is insufficient for P0-01/P0-02 as documented above. |
| `npm run test:google-drive` | **PASS — 5/5.** Its PKCE assertion covers the separate Drive connection flow, not native account login. |
| `npm run test:device-reconcile` | **PASS — 6/6.** |
| `npm run build` | **PASS** — TypeScript plus Vite build; 1,760 modules transformed. |
| `rustfmt --edition 2021 --check src/device_identity.rs src/native_auth.rs src/lib.rs` | **PASS.** |
| `deno check --frozen --node-modules-dir=auto supabase/functions/device-enrollment/index.ts` | **PASS.** |
| `cargo test -j 1 native_auth::tests --no-fail-fast` | **NOT COMPLETED** — first-time isolated Tauri compilation had not reached a test within the bounded wait; stopped on the user's instruction not to wait indefinitely. |
| Native PKCE/proof static audit | **P0 evidence** — native challenge/verifier absent; browser PKCE exchange present; issued-at/nonce absent from proof/UI/Edge; Edge signature verification absent. |
| Changed-path secret scan | **PASS** — no prohibited credential/private-key patterns in the seven changed implementation paths. |

## Changed-path and dirty-worktree audit

`87f2895..1775143` changes exactly these seven approved paths:

- `src-tauri/src/device_identity.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/native_auth.rs`
- `src/components/AccountLoginPanel.tsx`
- `src/lib/authFlow.ts`
- `src/lib/authParse.ts`
- `tests/authFlow.test.mjs`

`git diff --check 87f2895..1775143` passed, and the changed-path audit found
`UNEXPECTED_COUNT=0`. Before this review the worktree already contained the
listed modified/untracked owner paths, including the prior S1 Terra addendum,
both approved amendments, the RCA, `.tmp-transcript/`, and separate UI/docs
work. They were preserved untouched and unstaged. This review adds only this
report.

## Required local remediation before a re-review

1. A fresh Luna fix must generate a per-request PKCE verifier in native code,
   include the matching S256 challenge in the native-owned authorization URL,
   and keep the verifier in the native request lifecycle or an equally trusted
   native exchange boundary. Add behavioral tests for absent, mismatched,
   expired, replayed, and concurrent verifier handling.
2. Define a narrow enrollment-proof envelope that transports every signed
   canonical claim necessary for independent verification (version/action,
   platform, device label/fingerprint, issued-at, expiry, nonce, and
   signature). The server must reconstruct and Ed25519-verify it, validate
   expiry and durable nonce replay before accepting a pending enrollment, and
   produce an operator-verifiable record. Add native, Edge, and adversarial
   tests; do not expose a generic browser signing command or raw private key.
3. Re-run the focused Rust registry/identity tests and all existing auth/Drive
   regressions in a bounded reproducible environment before the next Terra
   review.

## Remaining external gates (after local P0 remediation only)

These remain external only after the two local P0 issues are fixed and pass a
fresh Terra review:

1. Approved staging migration/RLS/function privilege and Data API proof.
2. Live pending-enrollment and database-owner Boss approval ceremony evidence.
3. Google installed-app client configuration, real consent/callback/exchange,
   Drive upload/download/refresh/revoke, and provider-token custody.
4. Clean-install Windows keyring migration/reconnect/restore, Android/FUNGWIRE
   delegation, signing, release, merge, and promotion evidence.

## Version Diff

- `new -> 0.1.0b`: independent S2 review records two local P0 hard-gate
  failures (native PKCE handoff and verifiable enrollment-proof envelope),
  passing scoped checks, the bounded Rust-test limitation, path preservation,
  and retained external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | need review | FAIL: native PKCE verifier handoff and independently verifiable enrollment-proof metadata fail local hard gates. | pending | Terra |
