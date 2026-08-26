---
version: "1.0.0"
created_at: "2026-08-24T21:12:00+07:00,Terra 5.6"
last_update: "2026-08-24T21:12:00+07:00,Terra 5.6"
status: "complete"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-code-review"
  scope: "Desktop/Tauri Native Session Broker; review only"
  risk: "HIGH"
  reviewed_commit: "cd6ceef87d4b0754b17ab04b2c463a942ba978c5"
  reviewed_ref: "codex/backlog-truth-sync"
  verdict: "FAIL"
  integration_recommendation: "BLOCK"
---

# Native Session Broker — Terra 5.6 Code Review

## Verdict

**FAIL — BLOCK INTEGRATION.** Commit `cd6ceef87d4b0754b17ab04b2c463a942ba978c5`
does not meet the two mandatory evidence gates AC-GDA4-07 and AC-GDA4-08.
It also has source-level security/contract defects in OAuth-state zeroization
and typed IPC. A green build or static source scan does not substitute for
these gates.

No code, test, configuration, deployment, or external state was modified by
this review. Pre-existing unrelated dirty and untracked work was preserved.

## Commit provenance and review inputs

| Item | Evidence |
|---|---|
| Reviewed implementation | `cd6ceef87d4b0754b17ab04b2c463a942ba978c5` — `feat(auth): add native desktop session broker` |
| Parent | `cd6ceef^` |
| Candidate design | `docs/specs/2026-08-24-native-session-broker-amendment.md` v0.2.0b |
| Approval record | `docs/verification/implementation-reports/2026-08-24-native-session-broker-approval-record.md` v0.1.0b |
| Writer report independently checked, not inherited | `docs/verification/implementation-reports/2026-08-24-native-session-broker-luna-implementation.md` v0.1.2b |
| Scope | Desktop/Tauri only; browser unchanged; Mobile deferred |

## Command evidence

| Command | Result | Independent evidence |
|---|---|---|
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS | 8 passed, 0 failed |
| `node --test tests/googleDriveContract.test.mjs` | PASS | 5 passed, 0 failed |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS, insufficient | 8 passed, including PostgreSQL 17 execution; the concurrent scenario is only two clients, not 50 |
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, insufficient | 8 passed; all custody/lifecycle checks are source-text regex assertions rather than native behavioral tests |
| `npm run build` | PASS | TypeScript and Vite build completed; 1,764 modules transformed |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS with warnings | Exit 0; 21 warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS with warnings | 381 library tests passed, 0 failed; no required end-to-end native lifecycle matrix was present |
| `git diff --check cd6ceef^ cd6ceef` | PASS | No whitespace errors |

## AC-GDA4 acceptance review

| AC | Result | Independent assessment |
|---|---|---|
| AC-GDA4-01 | WARN | Static review finds redacted public DTOs and no obvious Desktop token event/storage path. This is not a runtime proof for every event/log/provider path, and AC-GDA4-02 has a concrete custody defect. |
| AC-GDA4-02 | FAIL | OAuth request state is held in ordinary `String` values, contrary to the zeroizing callback/state requirement; terminal-path behavior is not executed. |
| AC-GDA4-03 | WARN | Source implements keyring staging/readback, generation, and a refresh-flight mechanism, but startup/restart, rotation failures, single-flight, cleanup, shutdown, and stale completions lack executable native evidence. |
| AC-GDA4-04 | FAIL | The Desktop adapter exposes generic `Record<string, unknown>` command arguments, not per-operation typed IPC. Browser/Mobile static separation is otherwise preserved. |
| AC-GDA4-05 | WARN | Account and pairing consumers use broker names, and Drive uses fixed broker command names; however Drive remains on the generic `InvokeFn` surface rather than the required typed adapter, so exact typed consumer parity is not proven. |
| AC-GDA4-06 | PASS (local/static) | `src/web/AuthGuard.tsx` and Mobile's existing Supabase/auth-flow adapter remain present; build passed. This makes no Mobile readiness claim. |
| AC-GDA4-07 | FAIL | Required 50 simultaneous identical PostgreSQL clients are absent. The executable test creates exactly two clients and expects one winner plus one replay loser; it does not prove 49 `proof_replayed` losers or zero mutation by every loser. |
| AC-GDA4-08 | FAIL | Required native behavioral lifecycle coverage is absent. The passing custody test reads source files and uses regex presence/absence checks; it does not execute success, startup/restart, rotation, failure, single-flight, denial, malformed callback, timeout, cancel, exchange failure, logout, shutdown, cleanup failure, or stale generation. |
| AC-GDA4-09 | FAIL | Build/static checks are locally green, but the mandatory AC-GDA4-07/-08 gates and this independent review fail. External provider, clean-machine/keyring, device, signing/release, and production gates also remain open. |

## Findings

### P0-NSB-01 — 50-client replay proof is missing

**Status: FAIL.** `tests/w1AuthoritySchema.test.mjs:470-473` creates only two
simultaneous `runPsqlAsync` invocations. Its only cardinality assertions at
`tests/w1AuthoritySchema.test.mjs:478-486` require one success and one replay.
The approved AC requires at least 50 identical simultaneous clients, exactly
one winner, 49 `proof_replayed` losers, and zero loser mutation. A passing
two-client test is explicitly insufficient.

**Required disposition:** replace this with an executable 50-or-more-client
PostgreSQL test that records all 49 replay outcomes and verifies that no loser
created or mutated an enrollment/authorization/grant/reservation outcome.

### P0-NSB-02 — Native lifecycle evidence is static, not behavioral

**Status: FAIL.** `tests/nativeSessionCustody.test.mjs:23-34` only reads
`auth_session.rs` and checks word presence/absence. Its purported lifecycle
test at `tests/nativeSessionCustody.test.mjs:121-137` likewise consists solely
of `assert.match`/`assert.doesNotMatch` source scans. It never constructs a
keyring, callback listener, provider response, cancellation, timeout, stale
generation, or shutdown race.

This fails the mandatory AC-GDA4-08 behavioral matrix, including success,
startup/restart, rotation/failure, single-flight, denial, malformed callback,
timeout, cancel, exchange failure, logout, shutdown, cleanup failure, and
stale-generation completion.

**Required disposition:** add executable native tests with injectable
keyring/clock/listener/HTTP seams and assert redacted public outcomes plus
secret disposal on every terminal path.

### P1-NSB-03 — OAuth state is not zeroized

**Status: FAIL.** The native auth callback parser stores request state in an
ordinary `Option<String>` (`src-tauri/src/auth_session.rs:175`) and assigns the
callback value with `value.into_owned()` (`src-tauri/src/auth_session.rs:183`).
The Drive flow also keeps callback and pending OAuth state as ordinary strings:
`src-tauri/src/drive_oauth.rs:80-95` declares `OAuthCallback.state: String`
and `PendingOAuth.state: String`.

The approved boundary requires callback/request state, code, verifier, access,
and provider secrets to be zeroized on every terminal path. Dropping ordinary
`String` values is not zeroization; the current static suite does not detect
this distinction.

**Required disposition:** use a zeroizing byte/string representation from
callback receipt through validation and terminal cleanup, including both
auth-session and Drive OAuth state.

### P1-NSB-04 — IPC is closed by command name but not typed per operation

**Status: FAIL.** `src/lib/desktopSessionBroker.ts:39` defines the IPC surface
as `args?: Record<string, unknown>`, and `src/lib/desktopSessionBroker.ts:64-65`
forwards that arbitrary record directly. `src/components/AccountLoginPanel.tsx:16`
recreates the same generic forwarding signature. Drive bypasses the broker
type entirely by importing the generic `InvokeFn` (`src/lib/googleDriveFlow.ts:1`)
and invoking commands directly (`src/lib/googleDriveFlow.ts:24-49`).

The command-name union is a useful partial restriction, but it is not the
required typed, closed per-operation IPC contract and leaves exact input/output
shape enforcement unproven.

**Required disposition:** expose discriminated, per-command request/result
types (or dedicated operation functions) and route Drive through that adapter;
remove the generic record/invoke forwarding surface from Desktop consumers.

### P2-NSB-01 — Native review signal remains noisy

**Status: WARN.** `cargo check` and `cargo test` pass but report 21 warnings,
including the unused `app` parameter in
`src-tauri/src/auth_session.rs:281` and now-unreachable legacy local pairing
functions such as `src-tauri/src/lib.rs:539-551`. This is not the integration
blocker, but it obscures the broker's verification signal and should be
cleaned only after the P0/P1 remediation is designed and approved.

## Security-boundary assessment

- **OS-keyring-only refresh custody:** WARN — source uses keyring staging and
  readback, but lifecycle/cleanup paths are not behaviorally proven.
- **Zeroized native secret custody:** FAIL — ordinary auth and Drive OAuth
  state strings violate the requested state/callback zeroization boundary.
- **No WebView/log/storage/file/Genesis serialization:** WARN — targeted
  static scans passed and no direct Desktop token DTO/event was found; this
  remains unproven for runtime paths without the required behavioral tests.
- **Closed IPC / legacy retirement:** FAIL — fixed broker names exist, but the
  argument transport remains generic rather than exact typed operations.
- **Drive deny-before-secret/provider ordering:** WARN — source orders
  `authorize_drive` before keyring/provider work for the inspected Drive
  operations (for example `src-tauri/src/drive_oauth.rs:814-839` and
  `911-923`), but the mandatory negative behavioral evidence is missing.
- **Desktop/browser/Mobile boundary:** PASS locally/static — Desktop panels no
  longer import the browser auth adapter; browser and deferred Mobile paths
  remain available. This does not prove device behavior or promote Mobile.

## Integration recommendation

**BLOCK.** Do not integrate, merge, push, deploy, release, or promote this
implementation until P0-NSB-01 and P0-NSB-02 pass, the P1 defects are fixed
and independently reviewed, and the acceptance table can be re-evidenced.

## External gates still open

1. Real Supabase/Edge/RLS authority verification, including denial-before-
   keyring/provider effects and replay behavior in the target environment.
2. Real Google installed-app OAuth, refresh/revocation, Drive appDataFolder,
   and digest/target-bound restore UAT.
3. Clean Windows VM/keyring startup, rotation, logout, shutdown, stale-
   completion, and cleanup-readback evidence.
4. Supported-device UAT, signing, release artifact/publication verification,
   and explicit production go/no-go approval.

## Version diff

- `new -> 1.0.0`: independent review of `cd6ceef`; records the two mandatory
  hard-gate failures, zeroization and IPC contract defects, local command
  evidence, AC disposition, and a blocking integration recommendation.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-24 | complete | Independent code review: FAIL; integration blocked by AC-GDA4-07/-08 and P1 custody/IPC defects. | this commit | Terra 5.6 |
