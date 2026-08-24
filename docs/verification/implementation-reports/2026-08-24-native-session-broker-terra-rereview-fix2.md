---
version: "1.0.0b"
created_at: "2026-08-24T19:05:00+07:00,Terra 5.6"
last_update: "2026-08-24T19:05:00+07:00,Terra 5.6"
status: "need review"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-code-review"
  scope: "Desktop/Tauri native session broker fix2; review only"
  risk: "HIGH"
  reviewed_initial_commit: "cd6ceef87d4b0754b17ab04b2c463a942ba978c5"
  reviewed_fail_report_commit: "7a51ee1"
  reviewed_fix_commit: "abb7a329258e4a2b94c0bc4406e9a2904e7da591"
  verdict: "FAIL"
  integration_recommendation: "BLOCK"
---

# Native Session Broker Fix2 — Terra 5.6 Independent Re-review

## Verdict

**FAIL — BLOCK INTEGRATION.** Fix commit
`abb7a329258e4a2b94c0bc4406e9a2904e7da591` closes the real PostgreSQL
50-client proof and replaces the generic IPC forwarding surface. It does not
close the required native behavioral or zeroizing-custody gates. The 14 Rust
cases execute a separate `#[cfg(test)] BehavioralBroker` and fake types rather
than the production broker functions or injected seams used by them. In
addition, production callback and Drive refresh-token handling still creates
ordinary secret-bearing `String` values, and shutdown discards keyring-cleanup
failures.

This review changed no code, tests, prior reports, specifications, ledger, or
unrelated path. Pre-existing dirty and untracked work was preserved.

## Inputs and immutable provenance

| Item | Evidence |
|---|---|
| Approved design | `docs/specs/2026-08-24-native-session-broker-amendment.md` v0.2.0b; current SHA-256 is `41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4`, matching the approval record. |
| Approval record | `docs/verification/implementation-reports/2026-08-24-native-session-broker-approval-record.md:27-40`; D-GDA4-01..05 are bound to that exact hash. |
| Approval Terra review | `docs/verification/implementation-reports/2026-08-24-native-session-broker-approval-terra-review.md:24-33,119-125`; it authorizes bounded local implementation only, not integration. |
| Initial implementation | `cd6ceef87d4b0754b17ab04b2c463a942ba978c5` — `feat(auth): add native desktop session broker`. |
| Initial independent FAIL | `docs/verification/implementation-reports/2026-08-24-native-session-broker-terra-code-review.md` at `7a51ee1`; mandatory findings and dispositions are at lines 71-135. |
| Re-reviewed fix | `abb7a329258e4a2b94c0bc4406e9a2904e7da591` — `fix(auth): close native broker review gaps`; this was `HEAD` during review. |
| Updated Luna implementation report | `docs/verification/implementation-reports/2026-08-24-native-session-broker-luna-implementation.md` v0.1.3b, lines 82-160; its closure claim was independently re-tested rather than inherited. |

Scope remains Desktop/Tauri. The fix diff contains no `src/web/*` or
`src/mobile/*` paths; Browser behavior is unchanged and Mobile remains
deferred.

## Independent commands and results

| Command | Result | Independent result |
|---|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS | 8/8, 0 skipped. Docker/PostgreSQL 17 executed; the concurrency test was not skipped. |
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, insufficient | 9/9. Its Rust child process reports 14 passing tests, but the asserted test path is not the production path; see P0-NSB-02. |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS | 8/8. |
| `node --test tests/googleDriveContract.test.mjs` | PASS | 5/5. |
| `npm run build` | PASS | TypeScript/Vite build; 1,764 modules transformed. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS with WARN | Exit 0; 21 compiler warnings. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, insufficient | 14/14 named tests pass, but all auth lifecycle cases are test-only model logic. |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS with WARN | 395 library tests passed, 0 failed; warnings remain non-failing. |
| `git diff --check cd6ceef abb7a329258e4a2b94c0bc4406e9a2904e7da591` | PASS | No whitespace error. |

## Original finding dispositions

| Finding | Disposition | Exact evidence |
|---|---|---|
| P0-NSB-01 — 50-client replay proof missing | **PASS** | `tests/w1AuthoritySchema.test.mjs:470-493` launches `Array.from({ length: 50 })` `runPsqlAsync` PostgreSQL clients with one captured timestamp; lines 474-492 require exactly one exit-0 winner, 49 `proof_replayed` outcomes, and no other result. Lines 495-511 inspect the required outcome state and require `1|1|0|0|0|0|0` for enrollment request, proof reservation, devices, connection, grant, authorization reservation, and decision. The independent Node run passed 8/8 with Docker available. |
| P0-NSB-02 — native lifecycle evidence static, not behavioral | **FAIL — hard gate remains open** | `tests/nativeSessionCustody.test.mjs:144-162` runs the `native_behavioral_` filter only. The selected auth tests define a separate `BehavioralBroker`, `FakeKeyring`, `FakeProvider`, `FakeListener`, and `FakeClock` under `auth_session.rs:454-518`; they do not call production `broker_session_login_begin` (lines 291-303), `finish_login` (281-289), `ensure_access_token` (261-275), `commit_refresh_token` (86-97), or `shutdown` (327). The fakes are not injected into those production functions. Thus passing `native_behavioral_*` tests simulate disconnected lifecycle logic and cannot establish production terminal cleanup, cancellation, refresh, or generation behavior. |
| P1-NSB-03 — OAuth state zeroization | **FAIL** | The field types are improved (`Zeroizing<String>` in `auth_session.rs:42-45`, `drive_oauth.rs:80-95`), but receipt/custody is not end-to-end zeroizing. Production auth constructs an ordinary callback URL containing the callback query at `auth_session.rs:160-167`; Drive does the same with `Url::parse(&format!(...{target}))` at `drive_oauth.rs:476-482`. Drive keyring persistence creates ordinary refresh-token copies in `payload`, `staged_read`, and `active_read` at `drive_oauth.rs:390-405`. These values contain the refresh token and are not wrapped in `Zeroizing`; the test at lines 1561-1572 merely drops an `OAuthCallback` and does not observe these paths. |
| P1-NSB-04 — generic IPC and Drive bypass | **PASS (local/static), P2 WARN** | `src/lib/desktopSessionBroker.ts:31-84` now exposes dedicated per-operation functions and has no `Record<string, unknown>` forwarding. Account and pairing panels import/call those functions (`AccountLoginPanel.tsx:3-9,22-54`; `DevicePairingPanel.tsx:3-11,53-136`); Drive flow calls the typed adapter (`googleDriveFlow.ts:2-14,28-62`). `GoogleDrivePanel.tsx` still imports and passes a legacy `invoke` argument (`:49,59,106,123,134,157,162,182`) to ignored `unknown` compatibility parameters in `googleDriveFlow.ts:28-62`. It is not forwarded to a command, so this is not the original generic-bypass defect, but it weakens the exact typed-consumer signal and should be removed only under a separately approved write-set expansion. |

## Additional defect found in fix2

| Finding | Severity | Evidence and effect |
|---|---|---|
| NSB-R2-01 — production shutdown suppresses credential-cleanup failure | P1 / FAIL | `auth_session.rs:327` sets `Shutdown`, then deliberately discards both `delete_secret(...).and_then(verify_absent(...))` results with `let _ =`. Candidate §2.2 requires an unprovable shutdown cleanup to be reported as a cleanup concern, not treated as successful cleanup. The test-only `BehavioralBroker::shutdown` at lines 515-516 returns a redacted failure, but production shutdown cannot report one. |

## AC-GDA4-01..09 re-evaluation

| AC | Result | Independent assessment |
|---|---|---|
| AC-GDA4-01 | WARN | Public DTOs in `desktopSessionBroker.ts:14-29`, `auth_session.rs:99-137`, and `drive_oauth.rs:47-78` are redacted; the static suite finds no Desktop token DTO/event. Full production log, provider-buffer, and storage observation is not established locally. |
| AC-GDA4-02 | **FAIL** | Callback receipt creates ordinary secret-bearing strings (`auth_session.rs:160-167`; `drive_oauth.rs:476-482`), and Drive refresh persistence/readback keeps ordinary token strings (`drive_oauth.rs:390-405`). The disconnected behavioral model cannot cure or prove these production paths. |
| AC-GDA4-03 | **FAIL** | Login/logout production staging uses readback (`auth_session.rs:86-97,319-324`), but `shutdown` discards deletion/readback failure (`:327`). Startup, rotation, single-flight, and stale-generation evidence comes from a model not wired to production code (`:454-548`), so the required production behavior is not proven. |
| AC-GDA4-04 | PASS (local/static) | Dedicated operation functions replace `Record<string, unknown>` forwarding (`desktopSessionBroker.ts:31-84`); legacy secret-bearing commands are absent from the registered desktop inventory (`lib.rs:3101-3109` and custody test lines 39-79). Browser/Mobile paths were not modified. |
| AC-GDA4-05 | PASS (local/static), P2 WARN | Account, pairing, and Drive calls resolve through the typed adapter; Drive authority is requested before keyring/provider effects in the inspected production paths (`drive_oauth.rs:559-563,696-752,814-850,911-970`). The retained ignored legacy `invoke` parameter prevents a fully clean exact-consumer proof. |
| AC-GDA4-06 | PASS (local/static) | The fix diff has no Browser or Mobile paths; `tests/nativeSessionCustody.test.mjs:107-114` confirms their separate adapters remain present. No Mobile readiness claim is made. |
| AC-GDA4-07 | **PASS** | Executed PostgreSQL 17 proof meets the exact 50-client/winner/replay/no-loser-mutation contract at `w1AuthoritySchema.test.mjs:470-511`. |
| AC-GDA4-08 | **FAIL — hard gate** | Fourteen named tests pass, but the lifecycle scenarios run separate fake state-machine code, not production behavior or production-injected fakes (`auth_session.rs:454-548`). Several required production transitions are therefore not exercised. |
| AC-GDA4-09 | **FAIL** | AC-GDA4-02/-03/-08 fail, and the required independent review is FAIL. Clean build/test results do not substitute for those hard gates. |

## Security-boundary assessment

| Boundary | Result | Evidence |
|---|---|---|
| Native/WebView secret non-serialization | WARN | No direct token event/DTO/storage path was found in the reviewed Desktop adapter, but runtime provider/log-buffer proof remains open. |
| Zeroizing callback/code/verifier/token custody | FAIL | Ordinary callback-URL and Drive refresh-token `String` copies remain at the production locations cited in P1-NSB-03. |
| Keyring rotation, delete, and cleanup | FAIL | Production Drive serialization/readbacks are ordinary `String`s; production shutdown ignores keyring cleanup failure at `auth_session.rs:327`. |
| Closed typed IPC / legacy retirement | PASS (local/static) | Dedicated broker functions and command inventory replace the former generic record/invoke surface; the ignored Google Drive compatibility argument is a P2 warning, not a forwarded bypass. |
| Drive deny-before-secret/provider ordering | WARN | Source order is correct in the inspected operations, but its negative behavioral proof is test-only model logic; real Edge/RLS/provider evidence remains external. |
| Browser unchanged / Mobile deferred | PASS (local/static) | No Browser/Mobile fix2 diff; their separate source adapters remain available. |

## Integration recommendation

**BLOCK.** Do not integrate, merge, push, deploy, release, or promote
`abb7a329258e4a2b94c0bc4406e9a2904e7da591` until all of the following are
independently re-reviewed as a newly approved fix:

1. Replace the test-only lifecycle model with production code that accepts
   injectable keyring/clock/listener/provider seams, and make the 14 required
   cases exercise those production paths with terminal zeroization assertions.
2. Remove ordinary callback-query and refresh-token custody copies, including
   the Drive keyring serialization/readbacks, or introduce a reviewed
   zeroizing byte/string boundary for them.
3. Make production shutdown preserve/report cleanup failure according to the
   candidate's fail-closed contract.

The P2 compatibility warning may remain only once the P0/P1 hard gates are
closed and it no longer obscures typed-path correctness.

## External gates retained

1. Real Supabase/Edge/RLS authorization, denial-before-keyring/provider,
   durable replay, grant/revocation, and audit evidence in the target
   environment.
2. Real installed-app Google OAuth, refresh/revocation, appDataFolder,
   digest-bound upload/restore, and cancellation UAT.
3. Clean Windows VM OS-keyring startup, rotation, logout, shutdown, stale
   completion, and cleanup-readback proof.
4. Supported-device UAT; signing, release artifact/publication, merge,
   deployment, promotion, and explicit production go/no-go approval.

## Version diff

`new -> 1.0.0b`: fresh independent fix2 re-review of `abb7a329`; records the
passing live PostgreSQL 17 50-client proof, the invalid disconnected native
behavioral matrix, remaining zeroizing/shutdown defects, all AC-GDA4-01..09
dispositions, and a blocking integration recommendation.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0b | 2026-08-24 | need review | Independent fix2 re-review: P0 replay proof passes; P0 behavioral, P1 zeroizing, and shutdown cleanup gates fail; integration blocked. | pending | Terra 5.6 |
