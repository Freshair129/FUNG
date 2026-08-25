---
version: "1.0.0"
created_at: "2026-08-25T16:12:14+07:00,Terra 5.6"
last_update: "2026-08-25T16:12:14+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-implementation-rereview"
  scope: "D-GDA5 fix cycle 1: Desktop/Tauri native session broker production-path remediation only"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "bcc672decd3ae35cf7875ca2f984a7919aafbe6b"
  approved_candidate_sha256: "B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8"
  candidate_terra_pass_commit: "baf030d65698ef2c060d10464cd85262706a27dd"
  prior_failed_implementation: "fc45d7023804472f7fc4a5d4a05978140e789d7c"
  prior_terra_fail_commit: "bf980bb78c8c8303229027870ea1b3f229638541"
  implementation_target: "ac688a58cb123f11250f391a67f8d73f0b630325"
  target_parent: "bf980bb78c8c8303229027870ea1b3f229638541"
  luna_implementation_report: "docs/verification/implementation-reports/2026-08-25-native-session-broker-production-path-remediation-implementation-luna-report.md"
  verdict: "FAIL"
  recommendation: "BLOCK D-GDA5 fix1 acceptance; remediate the remaining P0 production-path, Drive-fencing, and keyring-recovery gaps under approved scope, then obtain a fresh independent review."
---

# Native Session Broker Production-Path Remediation — Terra 5.6 Independent Implementation Re-review, Fix 1

## Verdict

**FAIL / BLOCK.** `ac688a58cb123f11250f391a67f8d73f0b630325` improves the
previous target: it introduces a native singleton of the generic
`SessionLifecycle`, removes the former lifecycle-port compiler warnings, adds a
Drive operation guard, and corrects the Luna AC table. It does not close every
prior P0 gate.

The focused and full local Rust suites pass, but the focused behavioral matrix
still primarily calls `#[cfg(test)]` lifecycle helpers rather than the actual
registered production adapters; the Drive guard is not carried into the
individual list/upload/download provider calls; and the keyring protocol has no
marker/slot integrity proof or complete post-marker/crash recovery matrix for
both credential domains. The successful checks cannot substitute for those
source-backed hard-gate failures.

This review is local and read-only except for this report. It authorizes no
push, PR, merge, deployment, release, provider action, device action, or
production approval.

## Provenance and workspace integrity

| Item | Independent result | Disposition |
|---|---|---|
| Approved candidate | `bcc672decd3ae35cf7875ca2f984a7919aafbe6b` exists and changes exactly the amendment and Luna candidate report. | PASS |
| Candidate bytes | Blob `951a9115b44582c98456467b7e8125674d7514b9`, 37,406 bytes, SHA-256 `B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8`. | PASS — exact supplied value |
| Candidate Terra PASS | `baf030d65698ef2c060d10464cd85262706a27dd` is the document-only PASS at this report path; it did not certify implementation. | PASS, correctly bounded |
| Prior implementation / FAIL | `fc45d702...` and the full Terra FAIL review `bf980bb...` exist; all six prior findings were re-evaluated. | REVIEWED |
| Fix1 target | `ac688a58cb123f11250f391a67f8d73f0b630325`, parent `bf980bb...`, subject `fix(auth): close D-GDA5 implementation review gaps`. | REVIEWED |
| Fix1 manifest | Exactly five paths: Luna implementation report, `auth_session.rs`, `drive_oauth.rs`, and the two named Node contracts. `lib.rs` and `native_auth.rs` are unchanged by Fix1. | PASS, bounded scope |
| Target immutability | Before this report edit, `git diff --quiet ac688a5 --` over the five Fix1 paths returned 0. | PASS |
| Existing user dirt | Modified Desktop docs/plans/review, `BackupPanel.tsx`, `AccountSettings.tsx`, Supabase README; untracked RCA/drafts, `.tmp-transcript/`, and Drive CSS were present before review. They were neither altered nor staged. | PRESERVED |

The candidate requires one engine to own admission, generation/operation IDs,
quiescing, commit fencing, publication, and terminal cleanup; its test harness
must use the same engine entrypoints or registered adapters, not a test-only
success path (candidate lines 101-105 and 127-146). It also requires no Drive
access after disconnect/logout/shutdown linearization and a verified
marker/slot recovery protocol (candidate lines 138, 154-158, and 299-340).

## Independent call-graph and finding disposition

| Prior finding | Fix1 disposition | Independent evidence |
|---|---|---|
| P0-IMP-01 — production lifecycle versus behavioral engine | **FAIL — not closed.** | The registered command inventory is at `src-tauri/src/lib.rs:3013-3025` and `3104-3112`. Production login invokes `begin_login` at `auth_session.rs:2068-2086`, uses the free `spawn_listener` transport at `1876-1920`, and completes via `take_login_for_exchange` / `complete_login` at `1989-2031`; refresh uses `begin_refresh` / `finish_refresh` at `1934-1978`. The 20 focused tests construct `SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>` at `2725-2903`, but their core success, startup, rotation, refresh, and protected paths call `#[cfg(test)]` `begin`, `complete`, `startup`, `rotate_refresh`, `refresh_single_flight`, and `protected` at `297-320`, `474-529`, `531-563`, and `683-699`. Only one test reaches `take_login_for_exchange` / `complete_login` (`3149+`). There is no behavioral execution of the registered adapter graph, Native listener, native HTTP, or Drive provider operation guard. |
| P0-IMP-02 — Drive ticket/guard spans provider operation | **FAIL — not closed.** | `DriveOperationGuard` stores only a ticket and `Drop` calls `drive_finish` (`drive_oauth.rs:206-230`). It is created before the public operations (`934`, `982`, `1362`) and checked only after the whole list/upload/restore result (`942`, `1012`, `1433`). The individual provider functions receive only `DriveInvocation`: async list (`898-924`), blocking list (`1118-1143`), small upload (`1145-1177`), resumable upload (`1179-1258`), delete (`1260-1278`), and download (`1280-1315`). Those functions do not receive or check the lifecycle guard. A disconnect invalidates the ticket and immediately cleans credentials (`auth_session.rs:870-884`), but a list/upload/download started after that invalidation can still reach a provider send before the sole post-operation check rejects its public result. No active-operation drain, per-provider pre/post fence, or behavioral race test covers that provider-effect window. |
| P0-IMP-03 — failure-atomic AccountSession and DriveCredential keyring | **FAIL — not closed.** | The durable index now removes the prior bounded scan (`auth_session.rs:956-1108`) and startup reads Account plus registered Drive domains (`1578-1602`), but the marker is only `v{next}` (`1182`) and parsing plus startup validation establish only a version/index membership and non-empty slot (`950-953`, `1080-1108`). There is no marker slot identifier or integrity metadata/readback proving the referenced credential contents required by the candidate. After marker verification, old-slot deletion or index compaction may return `cleanup_failed` (`1201-1217`); `accept_material` simply propagates `commit_credential` failure (`565-587`) and production `complete_login` propagates it without entering `credential_cleanup_failed` (`424-449`). The focused test names cover staged failure, one Drive marker failure, absence/corrupt marker, pre-marker preservation, and registry enumeration (`2933`, `3070`, `3089`, `3100`, `3124`), but no post-marker cleanup/index-write fault, crash transition, or actual two-domain startup-recovery test. The required all-fault/crash matrix is absent. |
| P1-IMP-04 — recovery phrase ordinary ingress | **PASS locally — closed.** | Registered restore now accepts `RecoveryPhrase`, and custom `Deserialize` directly moves the Serde-created `String` into `Zeroizing<String>` without a clone (`drive_oauth.rs:234-242`, `1333-1342`). The Tauri/Serde buffer and the transient deserializer `String` remain framework-bound residual custody, not a retained ordinary command parameter; the implementation report must not claim forced framework-memory zeroization. |
| P1-IMP-05 — enrollment/device/pairing HTTP lifecycle fence | **PASS locally — closed.** | `AccountOperationGuard` owns an engine ticket (`auth_session.rs:185-205`, `393-422`). `native_post` holds it through HTTP response parse and post-check (`2229-2266`); device list, endpoint publish, RPC, pairing lookup, and audit list follow the same account ticket plus terminal check pattern (`2303-2327`, `2396-2429`, `2462-2487`, `2542-2571`, `2657+`). This fences stale public results; it does not repair the separate Drive provider-effect defect above. |
| P1-IMP-06 — Luna AC mapping | **PASS — corrected.** | Luna’s table at `...implementation-luna-report.md:129-144` maps AC01 single engine, AC02 dead ports, AC03 races, AC04 custody, AC05 keyring, AC06 cleanup, AC07 replay, AC08 typed IPC, AC09 Browser/Mobile, and AC10 local evidence in the candidate’s order. Its PASS claims for AC01/02/03/05/10 are not accepted because the P0 evidence above contradicts them. |

## Required retained controls

| Control | Independent disposition |
|---|---|
| `cleanup_failed` / `credential_cleanup_failed` terminal behavior | Retained for logout/shutdown: `logout` / `shutdown` set `CleanupFailed` on cleanup error at `auth_session.rs:700-771`, and the focused test passes. This does not close P0-IMP-03’s non-terminal post-marker commit failure state. |
| Deny before keyring/provider | Retained in Drive adapters: authorization and operation checks precede refresh-token load/provider calls at `drive_oauth.rs:930-935`, `971-983`, and `1324-1363`; focused custody contract passes. |
| Typed IPC and command inventory | Retained as a closed named `generate_handler!` surface at `src-tauri/src/lib.rs:3013-3025,3104-3112`; no generic auth/Drive forwarding was found in the bounded source/test audit. |
| 50-client replay contract | **UNVERIFIED behaviorally in this run.** The prescribed PostgreSQL authority suite failed before its executable database test became ready, so the one-winner/49-`proof_replayed` contract is not promoted from retained source evidence. |
| Browser/Mobile separation | PASS, scope/source contract only: `tests/authFlow.test.mjs` passes 8/8 and its Browser/Mobile separation case passes; no Browser or Mobile source is in Fix1. |
| GoogleDrivePanel P2 compatibility warning | Preserved out of scope: `GoogleDrivePanel.tsx` is not in Fix1 and no panel source was changed. |
| Secrets | No secret value is disclosed by this report. The bounded target-path audit found no added credential literal. This is not a whole-worktree credential audit. |

## Command evidence

Durations below are independently measured wall times on this workspace. A
passing command is evidence only for the behavior it actually exercised.

| Required command | Result | Duration / limitation |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS — 10/10 | 3.96 s wall; Rust child 20/20. New assertions are source-pattern checks and do not execute registered adapters. |
| `node --test tests/googleDriveContract.test.mjs` | PASS — 6/6 | 0.21 s wall; new Drive-fence assertion is source-pattern evidence. |
| `node --test tests/w1AuthoritySchema.test.mjs` | **FAIL — 7/8** | 65.75 s wall. Executable PostgreSQL 17 authority test timed out waiting for `fung-w1-pg17-18280-1787648867296`; no application conclusion is inferred, but the required 50-client executable evidence is unavailable. |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS — 8/8 | 0.28 s wall. |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs` | PASS | 0.25 s wall; check mode only. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS | 30.07 s wall; 17 warnings, all pre-existing paired-device/device-identity dead code, not lifecycle ports. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS — 20/20 | 2.04 s wall; generic Fake-port matrix, not registered-adapter/provider-race behavior. |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS — 400/400 | 29.90 s wall; 17 warnings. |
| `npm run build` | PASS | 9.92 s wall; TypeScript/Vite, 1,764 modules. |
| `git diff --check ac688a5^ ac688a5` | PASS | 0.07 s wall. |

## D-GDA5 and AC-GDA5 disposition

| Gate | Independent disposition | Basis |
|---|---|---|
| D-GDA5-01 | FAIL | P0-IMP-01: production adapters and the behavioral suite do not use the same entrypoint/port call graph. |
| D-GDA5-02 | FAIL | P0-IMP-02: no active drain or per-provider pre/post lifecycle fence prevents stale Drive provider effects after invalidation. |
| D-GDA5-03 | PASS locally, bounded | P1-IMP-04 recovery phrase is a custom type with a direct move into zeroizing custody; framework residuals remain external/UAT scope. |
| D-GDA5-04 | FAIL | P0-IMP-03: marker/slot integrity and all post-marker/crash/both-domain recovery conditions are not proved or fully represented in lifecycle failure state. |
| D-GDA5-05 | FAIL | The Luna evidence overstates AC01/02/03/05/10, and the required PostgreSQL executable evidence failed to start. |
| D-GDA5-06 | PASS, provenance only | Exact candidate commit/hash and prior document review were independently verified. |
| AC-GDA5-01 | FAIL | Same registered production path and injected behavior are not demonstrated. |
| AC-GDA5-02 | FAIL | The old dead-port warnings are gone, but test-only success/refresh paths remain the dominant behavioral proof. |
| AC-GDA5-03 | FAIL | Drive external provider effects are not fenced/drained across disconnect/logout/shutdown. |
| AC-GDA5-04 | PASS locally | Typed `RecoveryPhrase` command ingress is retained. |
| AC-GDA5-05 | FAIL | Failure-atomic marker/index integrity plus complete both-domain crash/fault evidence is absent. |
| AC-GDA5-06 | PASS locally, retained | Terminal cleanup failure propagation remains tested; it is not a substitute for post-marker failure handling. |
| AC-GDA5-07 | UNVERIFIED | PostgreSQL executable authority test did not become ready. |
| AC-GDA5-08 | PASS, bounded local/source contract | Typed registered command inventory remains closed; no generic forwarding found. |
| AC-GDA5-09 | PASS, scope only | Browser unchanged; Mobile deferred; focused auth contract passed. |
| AC-GDA5-10 | FAIL | Required command set is not wholly green and focused evidence does not exercise the required production race paths. |
| AC-GDA5-11 | OPEN | Local/static checks do not prove provider, VM/keyring, device, release, or production behavior. |
| AC-GDA5-12 | FAIL | This fresh independent implementation review returns FAIL/BLOCK. |

## Remaining scope and external gates

Required code/report remediation remains within the approved native lane and
must be separately reviewed. At minimum it must replace the test-only success
matrix with tests that traverse the same production lifecycle entrypoints and
ports, carry a lifecycle guard/check into every Drive provider boundary (or
provide an equivalent no-lock drain protocol), and make marker/index integrity,
post-marker compensation, crash recovery, and cleanup-failed state exhaustive
for AccountSession and DriveCredential.

Even after those local gates pass, the following remain open: clean Windows VM
and OS-keyring proof; real Supabase/Edge/RLS and replay-reservation evidence;
Google provider OAuth/list/upload/restore UAT; supported-device UAT; signing,
release, deployment, monitoring, and explicit production approval. None is
waived by this review.

## Version Diff

- Replaces the prior document-only PASS occupying this report path with the
  requested independent Fix1 implementation re-review.
- Verifies candidate bytes, Fix1 immutability, exact target manifest, source
  call graph, warning audit, prescribed commands, and preserved user dirt.
- Keeps P1-IMP-04 through P1-IMP-06 closed locally, but records P0-IMP-01,
  P0-IMP-02, and P0-IMP-03 as still blocking.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-25 | stable | FAIL/BLOCK: Fix1 removes prior warnings and corrects reporting, but production/test parity, per-provider Drive fencing, and complete failure-atomic recovery remain P0 blockers. | recorded by this review commit | Terra 5.6 |
