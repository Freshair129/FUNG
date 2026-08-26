---
version: "1.0.0"
created_at: "2026-08-25T16:55:14+07:00,Terra 5.6"
last_update: "2026-08-25T16:55:14+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-implementation-rereview"
  scope: "D-GDA5 Fix2-of-3: Desktop/Tauri native session broker production-path remediation only"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "bcc672decd3ae35cf7875ca2f984a7919aafbe6b"
  approved_candidate_sha256: "B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8"
  fix1_source_commit: "ac688a58cb123f11250f391a67f8d73f0b630325"
  fix1_terra_fail_commit: "0854b9c51ff14fc2095c887c629de1b3de839ac3"
  provenance_correction_commit: "1cb461525756180e0e89ec41e4f6ca896ffcc079"
  implementation_target: "748fb00e13682daefd5751b36665d90dcafbbed8"
  target_parent: "1cb461525756180e0e89ec41e4f6ca896ffcc079"
  luna_implementation_report: "docs/verification/implementation-reports/2026-08-25-native-session-broker-production-path-remediation-implementation-luna-report.md"
  verdict: "FAIL"
  recommendation: "BLOCK D-GDA5 Fix2 acceptance. Remediate the production adapter/race coverage, no-lock drain, per-send lifecycle fence, and production both-domain recovery matrix under approved scope, then obtain a fresh independent review."
---

# Native Session Broker Production-Path Remediation — Terra 5.6 Independent Implementation Re-review, Fix 2

## Verdict

**FAIL / BLOCK.** Fix2 improves the generic lifecycle and keyring material, and
all prescribed local commands pass. It does not close the hard production-path
gates. The registered Browser-loopback listener/callback flow is still a free
production path outside the exercised generic lifecycle adapter; the Drive
disconnect wrapper holds the production lifecycle mutex while it waits for an
active operation that must reacquire that same mutex before its guard can drop;
and resumable upload chunks have no lifecycle-ticket pre/post fence. The
keyring format is materially stronger, but the required deterministic
production startup/restart and both-domain fault matrix is not executed.

This is a local, review-only result. It authorizes no source remediation, push,
PR, merge, provider action, clean-VM action, device action, deployment, release,
or production approval.

## Provenance and scope integrity

| Item | Independent result | Disposition |
|---|---|---|
| Approved candidate | `bcc672decd3ae35cf7875ca2f984a7919aafbe6b` exists and changes only the approved amendment and Luna candidate report. | PASS |
| Candidate bytes | Raw Git blob `951a9115b44582c98456467b7e8125674d7514b9`, 37,406 bytes, hashes to supplied SHA-256 `B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8`. | PASS |
| Candidate document PASS preservation | The candidate PASS path `...terra-rereview-fix1.md` has blob `c1f6f425784e8caa383676eed9a62612e4bef322` both at `baf030d65698ef2c060d10464cd85262706a27dd` and at target `748fb00...`. | PASS |
| Prior review chain | Fix1 source `ac688a58...`, distinct Fix1 FAIL commit `0854b9c...`, technical review `0854b9c...`, and provenance correction `1cb4615...` exist and were read with the original Terra implementation review and Fix1 report. | REVIEWED |
| Fix2 target | `748fb00e13682daefd5751b36665d90dcafbbed8`, parent `1cb461525756180e0e89ec41e4f6ca896ffcc079`, subject `fix(auth): close D-GDA5 fix1 P0 gates`. | REVIEWED |
| Target manifest | Five implementation paths changed: Luna report, `auth_session.rs`, `drive_oauth.rs`, and the two named Node contracts. `lib.rs` is unchanged and was audited for registered command/startup wiring. | BOUNDED |
| Target immutability before this report | The five target paths were clean against `748fb00...`; `git diff --check 748fb00^ 748fb00` passed. | PASS |
| Existing user dirt | Pre-existing modified Desktop docs/plans/review, `BackupPanel.tsx`, `AccountSettings.tsx`, Supabase README, plus untracked RCA/drafts, `.tmp-transcript/`, and Drive CSS were present. None was changed or staged. | PRESERVED |

## Independent findings

### P0-IMP-01 — Registered production adapter graph is not the behavioral graph

**FAIL. Affected gates: D-GDA5-01, D-GDA5-05; AC-GDA5-01, -02, -10.**

The generic `SessionLifecycle` methods are improved and the compiler no longer
reports dead lifecycle-port warnings. That is insufficient for the approved
same-production-path requirement.

Production login calls `begin_login` from the singleton at
`src-tauri/src/auth_session.rs:2234-2246`, then invokes the free
`spawn_listener` at `2252`. That function constructs `NativeListener` and calls
`callback_target` itself at `2042-2075`; the lifecycle's `NativeListener::open`
is a no-op (`1445-1453`). `finish_login` independently takes the callback,
parses it, and invokes `provider.exchange` outside the lifecycle before calling
`take_login_for_exchange` / `complete_login` (`2155-2197`).

The 24 focused behavioral tests construct only
`SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>` at
`3060-3066`. Their `FakeListener::callback_target` always returns `None`
(`2970-2983`) and is never driven by a bound listener/callback. The helpers
directly create pending state and call `FakeProvider::exchange` before
`complete_login` (`3086-3107`). They do not execute the registered loopback
listener, callback ingress, production singleton, or native provider/keyring
ports. `login_expired` also reads `SystemTime` directly (`478-483`) instead of
the injected clock.

The absence of lifecycle-port warnings in the independent `cargo check` is
therefore a positive hygiene result, not behavioral proof of the same registered
adapter/port graph. The Node assertion named “registered adapters and behavioral
tests share production lifecycle entrypoints” is a source-pattern assertion; it
cannot establish the missing execution path.

### P0-IMP-02 — Drive no-lock drain and per-provider fencing are still incomplete

**FAIL. Affected gates: D-GDA5-02, D-GDA5-05; AC-GDA5-01, -03, -10.**

`disconnect_drive` itself sets admission closed and calls `wait_empty`
(`auth_session.rs:801-816`), but the public `drive_disconnect` wrapper holds
`production_lifecycle().lock()` through that call (`1688-1692`). A live provider
operation performs a post-send `drive_check`, which needs the same mutex, before
it can return and drop `DriveOperationGuard`; its `Drop` is what releases the
drain (`drive_oauth.rs:218-222`). Thus this ordering can deadlock: disconnect
waits for the guard, while the guard waits for disconnect to release the mutex.
The focused race avoids the production condition by manually releasing the
drain before `finish_drive_operation` (`auth_session.rs:3351-3379`).

Logout and shutdown are weaker still: they set quiescing and invalidate/clear
Drive state at `631-640` and `669-678` without any `drive_drain.wait_empty()`.
An already admitted provider request can therefore cross the transition while
credential cleanup is running.

Most provider helpers carry `LifecycleTicket` and have pre/post checks, such as
list (`drive_oauth.rs:903-932`), small upload (`1164-1199`), delete
(`1285-1306`), and download request/stream (`1308-1347`). The resumable chunk
loop does not: after the initial session create is fenced (`1209-1223`), each
chunk uses only `invocation.ensure_valid()` before and after `client.put(...)
.send()` (`1241-1254`). There is no `auth_session::drive_check(ticket)` around
those irreversible provider sends. A logout/disconnect can linearize between
that invocation check and a chunk send, and no post-send ticket check rejects
the stale effect. The 24-test focused suite has no race through a registered
Drive command, `upload_resumable_file`, or a real provider port.

### P0-IMP-03 — Keyring source is stronger, but the required production recovery proof is absent

**FAIL (evidence gate). Affected gates: D-GDA5-04, D-GDA5-05; AC-GDA5-05, -10.**

The source now records format/domain/version/exact slot/integrity/content SHA-256
in `CredentialMarker` (`auth_session.rs:882-964`), validates index/registry
integrity and readback (`966-1039`), conditionally restores a marker during
pre-marker compensation (`1150-1208`), and sets `credential_cleanup_failed`
when a post-marker cleanup error is returned by Account or Drive commit
(`505-516`, `769-780`). `load_committed` reads the marker/slot, verifies content,
cleans indexed old slots, and compacts the index before returning credentials
(`1091-1145`). `startup_recover` enumerates the account plus registered Drive
domains before marking startup checked (`1739-1768`). These are material local
source improvements.

They do not satisfy the requested proof. Every fault/restart test uses the fake
in-memory lifecycle. The focused tests cover one account marker/content/index
path and a Drive marker-write failure, but no test executes
`startup_recover()` through the production singleton/keyring for both Account
and registered Drive domains, simulates each crash ordering (staged index,
slot, marker, old-slot deletion, and compact-index write), or proves recovered
state prevents access/publication after a post-marker failure. The only stated
restart test seeds and refreshes a fake account (`3154-3160`); there is no
production Drive-domain restart test. This fails the mandatory actual-entrypoint
and both-domain fault/crash matrix even though the schema is directionally sound.

## Retained P1 controls

| Control | Independent disposition | Evidence |
|---|---|---|
| RecoveryPhrase direct ingress | PASS locally, bounded | Custom `Deserialize` moves the Serde string into `Zeroizing<String>` (`drive_oauth.rs:236-245`); registered restore accepts `RecoveryPhrase` (`1364-1375`). Framework buffer residuals remain external. |
| Account HTTP operation guard | PASS locally, bounded | `AccountOperationGuard` holds ticket/drain (`auth_session.rs:217-237`); native account operations begin it and check it after response parsing (for example `native_post` at `2395+`). This does not cure the Drive defect. |
| `cleanup_failed` terminal state | PASS locally, bounded | Logout/shutdown set `CleanupFailed` on credential cleanup errors (`659-661`, `697-698`); focused terminal test passes. |
| Deny before keyring/provider | PASS locally, bounded | Drive authorization/operation checks precede `begin_drive_work`, refresh-token load, and provider helpers (`drive_oauth.rs:874-896`, `934-943`, `974-992`). |
| Typed IPC / no generic forwarding | PASS, source-bounded | Registered named command inventory remains in `lib.rs:3013-3025,3104-3112`; focused custody/auth contracts pass. |
| Luna AC mapping | PASS | Fix2 Luna report maps AC01 through AC12 in the candidate order, but its P0 PASS claims are rejected by this review. |
| W1 replay authority | PASS locally | Current executable PostgreSQL 17 W1 run passed 8/8. This does not elevate real Edge/RLS/production evidence. |
| Browser/Mobile/panel/secrets | PASS by scope/bounded scan | No Browser, Mobile, or panel source is in target; auth-flow contract passes 8/8. No secret value is printed or asserted by this report; this is not a whole-worktree secret audit. |

## Command evidence

All commands were rerun independently at `748fb00...`; `rustfmt` was invoked
only with `--check` and did not write files. Durations are wall-clock values.

| Required command | Result | Duration and limitation |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS — 12/12; Rust child 24/24 | 2.32 s. Node coverage is chiefly source-pattern; Rust child is fake-port generic lifecycle, not registered listener/Drive command races. |
| `node --test tests/googleDriveContract.test.mjs` | PASS — 6/6 | 0.15 s. Contract/source assertions do not execute provider effects. |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS — 8/8 | 21.18 s; PostgreSQL 17 executable migration/rollback evidence passed. |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS — 8/8 | 0.21 s. Browser/Mobile separation contract only. |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs` | PASS | 0.22 s; check mode only. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS | 1.47 s; 17 warnings, all existing paired-device/device-identity dead code; no lifecycle-port warning. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS — 24/24 | 1.91 s; no actual registered listener/provider/Drive command race. |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS — 404/404 | 24.24 s test runtime; same 17 non-lifecycle warnings. |
| `npm run build` | PASS — 1,764 modules | 8.89 s. |
| `git diff --check 748fb00^ 748fb00` | PASS | 0.05 s. Whitespace integrity is not lifecycle proof. |

## Gate disposition

| Gate | Disposition | Basis |
|---|---|---|
| D-GDA5-01 | FAIL | P0-IMP-01: behavioral coverage does not traverse the registered listener/callback adapter graph or native ports. |
| D-GDA5-02 | FAIL | P0-IMP-02: production disconnect can deadlock its drain, logout/shutdown do not drain, and resumable sends lack ticket checks. |
| D-GDA5-03 | PASS locally | Typed zeroizing `RecoveryPhrase` ingress retained. |
| D-GDA5-04 | FAIL | P0-IMP-03: keyring source is stronger but the required actual production both-domain crash/restart/fault matrix is missing. |
| D-GDA5-05 | FAIL | Required local evidence cannot be accepted while production P0 gates fail. |
| D-GDA5-06 | PASS, provenance only | Candidate commit/hash and prior PASS blob equality independently verified. |
| AC-GDA5-01 | FAIL | Same registered production adapter/port execution is not shown. |
| AC-GDA5-02 | FAIL | No dead lifecycle-port warning remains, but free listener/callback authority is not in behavioral coverage. |
| AC-GDA5-03 | FAIL | No-lock drain and every provider-boundary fence are not implemented/proved. |
| AC-GDA5-04 | PASS locally | Direct typed recovery ingress retained. |
| AC-GDA5-05 | FAIL | Both-domain deterministic production recovery proof is absent. |
| AC-GDA5-06 | PASS locally, retained | Cleanup failure maps to the terminal state in the bounded matrix. |
| AC-GDA5-07 | PASS locally | W1 executable authority rerun is green. |
| AC-GDA5-08 | PASS locally | Closed typed IPC inventory retained. |
| AC-GDA5-09 | PASS by scope | Browser/Mobile/panel paths unchanged; contract remains green. |
| AC-GDA5-10 | FAIL | Passing focused/full suites do not exercise the mandatory registered production race/fault paths. |
| AC-GDA5-11 | OPEN | Local checks do not prove clean-VM keyring, real provider, device/UAT, signing, release, deployment, monitoring, or production state. |
| AC-GDA5-12 | FAIL | Fresh independent Fix2 implementation rereview returns FAIL/BLOCK. |

## Required remediation and external gates

Within the approved lane, remediation must make the registered listener/callback
and native provider/keyring/clock adapters executable through the same
non-test lifecycle entrypoints as the behavioral matrix; release the lifecycle
mutex before waiting for the Drive drain; drain Drive operations before
logout/shutdown cleanup; add a lifecycle ticket pre/post check to every
resumable chunk send; and execute a deterministic production-entrypoint
Account-plus-Drive crash/fault/restart matrix. The fixes require a fresh
approved implementation commit and a new independent review.

Clean Windows VM/OS-keyring proof, real Supabase/Edge/RLS and replay-reservation
evidence, real Google OAuth/list/upload/restore UAT, supported-device UAT,
signing, release, deployment, monitoring, and explicit production approval all
remain open. None is waived by these local results.

## Version Diff

- New, exclusive Fix2 implementation rereview report only.
- Independently verifies candidate bytes/provenance, target scope, retained P1
  controls, required command matrix, and preserved user dirt.
- Blocks Fix2 on P0 production adapter parity, Drive drain/per-send fencing,
  and required production keyring-recovery evidence.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-25 | stable | FAIL/BLOCK: Fix2 local commands pass but registered adapter parity, no-lock Drive drain/per-send fences, and production both-domain recovery proof remain open P0 gates. | recorded by this review commit | Terra 5.6 |
