---
version: "4.0.0b"
created_at: "2026-08-25T04:43:08+07:00,Luna 5.6"
last_update: "2026-08-25T17:30:00+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-report"
  scope: "D-GDA5-01..06; Desktop native session broker production path"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "bcc672decd3ae35cf7875ca2f984a7919aafbe6b"
  approved_candidate_sha256: "B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8"
  target_parent: "748fb00e13682daefd5751b36665d90dcafbbed8"
  prior_terra_fix2_review: "e5155d4c1ed05357c913ebe311dbfbb69e18bb16"
  fix_cycle: "3/3"
  implementation_commit: "not embedded; bound only after the requested commit"
---

# Native Session Broker production-path remediation — Luna 5.6

## Disposition

Fix3-of-3 is implemented within the approved six-path write set. This report
records local implementation evidence only. It does not claim Terra PASS,
production readiness, deployment, release, external authorization, or a
production go/no-go decision.

Fix3 addresses Terra Fix2 P0-IMP-01/02/03:

- the registered listener/callback and injected adapter tests now enter the
  same lifecycle entrypoints used by production commands;
- Drive disconnect, logout, and shutdown quiesce first, release the lifecycle
  mutex before waiting for drains, then perform credential cleanup; every
  resumable chunk send has lifecycle pre/post checks;
- startup recovery is a lifecycle method used by the production startup
  entrypoint and the deterministic Account-plus-Drive fault matrix, including
  staged index, slot, marker, old-slot deletion, and compact-index orderings.

This was the final authorized fix cycle. A remaining Terra FAIL/BLOCK would
require a new amendment; no further implementation cycle is authorized here.

## Provenance and scope

The immutable approved candidate is unchanged:

- commit: `bcc672decd3ae35cf7875ca2f984a7919aafbe6b`
- candidate SHA-256: `B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8`
- implementation baseline: `748fb00e13682daefd5751b36665d90dcafbbed8`
- Terra Fix2 review: `e5155d4c1ed05357c913ebe311dbfbb69e18bb16`

Exact allowed write set:

1. `src-tauri/src/auth_session.rs`
2. `src-tauri/src/drive_oauth.rs`
3. `src-tauri/src/lib.rs` — audited and unchanged
4. `tests/nativeSessionCustody.test.mjs`
5. `tests/googleDriveContract.test.mjs` — audited and unchanged
6. this report

No `native_auth.rs`, Browser, Mobile, UI panel, dependency, configuration,
provider, deployment, release, or external path was changed. Existing user
modified and untracked files were preserved and were not staged.

Final SHA-256 inventory before commit:

| Path | SHA-256 |
|---|---|
| `src-tauri/src/auth_session.rs` | `DDAECDCF57660B4E3C320410DA5395D410069C5423D0DAB331AC0589D63F0FAD` |
| `src-tauri/src/drive_oauth.rs` | `758DC48E76125E034DAD5762330EBA5213D6411696208AA20343E2DAFAB4AEC6` |
| `src-tauri/src/lib.rs` | `0F9CA9D9C63C5FE002CDE794EE75109CCC916609A52893D97B47A4C6C6DED1ED` |
| `tests/nativeSessionCustody.test.mjs` | `A72ABDA500BF8B2B5042259BB674A7DE745DDD49B015DB9F1552F0F79EAF4F04` |
| `tests/googleDriveContract.test.mjs` | `455A98AE39DA29F498D4789A693D54B523E7391C2F4F19580D07501FA7D095BF` |

## Root cause and remediation

### P0-IMP-01 — registered production adapter graph

The production loopback listener previously constructed `NativeListener`
outside the lifecycle object, while behavioral coverage used a listener seam
that returned no callback. Fix3 routes listener callback parsing through the
registered lifecycle listener port. Production login now uses named
`registered_login_begin`, `registered_login_take_for_exchange`, and
`registered_login_complete` entrypoints, and the behavioral harness calls the
same entrypoints with deterministic injected keyring, clock, listener, and
provider ports. Login expiry uses the injected `ClockPort`, not a direct
system-clock read. The generic `LifecycleCore`/`SessionMemory` shortcuts remain
absent.

### P0-IMP-02 — Drive lifecycle ordering and provider fencing

The prior public disconnect held the production mutex while waiting for an
active operation whose post-check and guard release could reacquire it. Fix3
splits Drive transition into `begin_drive_disconnect` and
`finish_drive_disconnect`; the production wrapper quiesces and invalidates
under the lock, releases the lock, waits for the drain, then reacquires the
lock for cleanup. Account logout and shutdown use the same two-phase terminal
transition and drain both account and Drive work before credential cleanup.
Every resumable upload chunk has `drive_check(ticket)` before the provider
send and after it. The race test follows the registered lifecycle transition
ordering and proves that stale work cannot complete the transition early or
deadlock.

### P0-IMP-03 — production-equivalent Account-plus-Drive recovery matrix

`SessionLifecycle::recover_startup` is now the single recovery implementation;
the production `startup_recover` entrypoint delegates to it. Recovery validates
the Account marker/index and enumerates the registered Drive domain registry
through the same keyring port. The deterministic production-equivalent
entrypoint test seeds both domains and exercises staged-index, slot, marker,
old-slot-deletion, and compact-index failure orderings. Any recovery fault
sets `credential_cleanup_failed`, closes admission, clears memory, prevents
Drive admission, and publishes no stale access or connected state. A clean
both-domain restart path also passes.

## Acceptance-criteria mapping

| AC | Fix3 local disposition |
|---|---|
| AC-GDA5-01 | PASS, bounded: registered login/listener and injected adapter paths use one `SessionLifecycle` engine |
| AC-GDA5-02 | PASS, bounded: no duplicate lifecycle authority; clock/listener/provider/keyring ports are exercised through named entrypoints |
| AC-GDA5-03 | PASS, bounded: Drive transition releases mutex before drain and chunk sends have pre/post ticket checks |
| AC-GDA5-04 | PASS locally, bounded: typed zeroizing recovery ingress retained; framework residual custody remains external |
| AC-GDA5-05 | PASS locally, bounded: both-domain startup/restart/fault matrix covers the five named crash orderings |
| AC-GDA5-06 | PASS locally, bounded: logout/shutdown drain before cleanup and preserve terminal cleanup failure behavior |
| AC-GDA5-07 | PASS locally if the required W1 rerun remains green; it is not production Edge/RLS proof |
| AC-GDA5-08 | PASS locally: closed typed IPC inventory retained |
| AC-GDA5-09 | PASS by scope: Browser unchanged and Mobile deferred |
| AC-GDA5-10 | PASS locally only after the complete command matrix below passes |
| AC-GDA5-11 | OPEN: clean Windows VM/OS-keyring, real provider, device/UAT, signing, release, deployment, monitoring, and production evidence |
| AC-GDA5-12 | OPEN until fresh independent Terra implementation review of the focused commit |

## Verification evidence

Results below must be filled from the final clean verification run before the
focused commit is created; no result is inferred from an earlier cycle.

| Required command | Result |
|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS — 12/12; Rust behavioral child 26/26 |
| `node --test tests/googleDriveContract.test.mjs` | PASS — 6/6 |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS — 8/8; external authority boundary remains open |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS — 8/8 |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs` | PASS — check-only |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS — 18 existing warnings; no new lifecycle-port warning |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS — 26/26 |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS — 406/406 |
| `npm run build` | PASS — 1,764 modules |
| `git diff --check` | PASS — scoped implementation diff |

## Limitations and external gates

Local source and executable tests cannot prove clean Windows OS-keyring
behavior, real Supabase/Edge/RLS authorization, real Google OAuth/list/upload/
restore UAT, supported-device behavior, signing, release publication,
deployment, monitoring, or production approval. No real provider call or real
OS credential was used by the deterministic recovery matrix. The existing
GoogleDrivePanel compatibility warning remains out of scope.

## Version Diff

- `3.0.0b -> 4.0.0b`: Fix3-of-3 final authorized remediation. Bound listener,
  clock, login, and recovery behavior to the shared lifecycle; split terminal
  and Drive transitions so drains occur outside the lifecycle mutex; fenced
  every resumable chunk send; added deterministic both-domain recovery fault
  coverage and updated source contracts.
- Preserved the prior P1 controls, typed IPC, Browser/Mobile boundary,
  deny-before-keyring/provider ordering, and cleanup-failure terminal state.
- No push, PR, merge, deploy, release, provider action, or external approval.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 4.0.0b | 2026-08-25 | candidate | Fix3-of-3 final authorized P0 remediation; final verification and Terra review remain required | not embedded before commit | Luna 5.6 |
| 3.0.0b | 2026-08-25 | candidate | Fix2 local report; Terra Fix2 implementation rereview returned FAIL/BLOCK | `748fb00e13682daefd5751b36665d90dcafbbed8` | Luna 5.6 |
| 2.0.0b | 2026-08-25 | candidate | Fix1 local report; Terra Fix1 implementation rereview returned FAIL/BLOCK | `ac688a58cb123f11250f391a67f8d73f0b630325` | Luna 5.6 |
| 1.0.0b | 2026-08-25 | candidate | Initial implementation report for approved D-GDA5-01..06 lane | `fc45d7023804472f7fc4a5d4a05978140e789d7c` | Luna 5.6 |
