---
version: "1.0.0"
created_at: "2026-08-26T00:28:03+07:00,Terra 5.6"
last_update: "2026-08-26T00:28:03+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-review"
  scope: "D-GDA6-01..06; independent implementation re-review; cycle 2/3"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "e6722db3f310d82270d3c6879a7749fb15e4f366"
  approved_candidate_sha256: "2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F"
  reviewed_implementation_commit: "0a07c0b628bfaf6569f8f506213e56d314470dd0"
  reviewed_implementation_parent: "26507132f23a44d9b674b19ed1e336505b67edae"
  verdict: "FAIL/BLOCK"
  fix_cycle: "2/3"
---

# Native Session Broker Registered-Entrypoint Evidence — Terra 5.6 Independent Implementation Re-review, Cycle 2

## Verdict

**FAIL / BLOCK.** All prescribed local commands pass at
`0a07c0b628bfaf6569f8f506213e56d314470dd0`. Cycle 2 closes the former
test-only credential-success authority and improves the recovery test into a
table over Account, `drive-alpha`, and `drive-beta`. It does not yet prove the
full recovery/restart/readback matrix required by D-GDA6-04.

The remaining P0 is evidence, not a request for a different production
lifecycle engine. In the table-driven test, every fault row calls
`RegisteredBrokerEntrypoints::startup_recover` once only
(`src-tauri/src/auth_session.rs:3694-3712`). A second startup route is used
only for the two normal-absence rows (`:3715-3730`). Thus the corrupt-marker
row (`:3612-3614`) has no restart-after-failure/re-read proof, despite the
candidate requiring that an ambiguous marker's restart reread all material.

Further, `assert_recovery_terminal` prints only case, domain, result class,
and lifecycle state (`:3663-3676`), then reads the target marker and index but
discards both values (`:3688-3690`). It does not assert the required slot or
orphan cleanup/readback outcome for fault rows, nor record the required
operation, fault point, marker/index/slot identifiers, cleanup proof, and
public-publication outcome. Passing source tests cannot substitute for those
missing behavioral assertions. This leaves AC-GDA6-07/-08 and the evidence
portion of AC-GDA6-10 unaccepted.

This is fix cycle **2/3**. A bounded cycle 3 may remediate this one P0 within
the approved six paths. Cycle 3 is final; a further failure requires a new
amendment. This verdict is local/static only and is not production evidence.

## Provenance and scope integrity

| Check | Independent result | Disposition |
|---|---|---|
| Approved candidate ancestry | `git merge-base --is-ancestor e6722db3... 0a07c0b6...` exits 0. | PASS |
| Candidate SHA-256 | Candidate bytes hash to `2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F`. | PASS — exact Boss binding retained |
| Candidate preservation | `git diff --quiet e6722db3... 0a07c0b6... -- <candidate>` exits 0. | PASS |
| Reviewed target | Target `0a07c0b628bfaf6569f8f506213e56d314470dd0`; parent `26507132f23a44d9b674b19ed1e336505b67edae`. | PASS |
| Cycle-2 write set | Only Luna report, `src-tauri/src/auth_session.rs`, and `tests/nativeSessionCustody.test.mjs` changed. All are inside the six approved paths. | PASS |
| Diff whitespace | `git diff --check 0a07c0b^ 0a07c0b` exits 0. | PASS |
| Review isolation | This review writes only its own report path. Existing user changes were neither staged nor altered. | PASS |

Before this review write, the preserved user-owned worktree paths were the
same modified/untracked set outside this review: the Desktop/Phase-4/W1
documents, BackupPanel, AccountSettings, Supabase README, RCA/transcript
artifacts, prior workflow/spec documents, and GoogleDrivePanel CSS. No source,
test, candidate, provider, credential, or external system action occurred in
this review.

## Independent findings

### P0-IMP-02-R2 — Recovery matrix lacks mandatory restart and cleanup/readback evidence

**Affected:** D-GDA6-04, D-GDA6-05; AC-GDA6-06, -07, -08, -10.

The positive convergence is real. The test fixture creates the same
`RegisteredBrokerEntrypoints` façade with `FakeKeyring`, `FakeClock`,
`FakeListener`, and `FakeProvider` (`src-tauri/src/auth_session.rs:3379-3391`).
Its seed creates Account material through
`registered_login_begin -> registered_login_take_for_exchange ->
registered_login_complete` (`:3420-3455`), and Drive material through a
ticketed `DriveOperationGuard` plus `commit_drive` (`:3457-3484`).
`FakeKeyring` setup/read/delete helpers delegate to the `KeyringPort` methods,
not its internal map (`:3172-3189`), so there is no direct-map-mutation
finding.

The matrix also now includes all three target-domain labels and its first pass
does enter the same non-test `startup_recover` façade (`:3591-3712`). It covers
staged index, missing slot, corrupt marker, old-slot deletion, compact-index,
post-marker, and corrupt-target rows, plus verified-NoEntry and
marker-missing-orphan rows.

It still falls short of the approved row semantics:

1. The candidate requires restart after each prior success and specifically
   requires ambiguous-marker restart reread proof. The fault loop performs no
   restart at all (`:3704-3712`). The only restart loop is limited to
   `VerifiedNoEntry` and `MarkerMissingOrphan` (`:3715-3730`). The test has no
   same-route restart after `Marker`, `CorruptTarget`, or the other fault rows;
   it also has no explicit valid persisted Account-plus-two-Drive restart row
   in this matrix.
2. The stated terminal helper establishes no access and no connected state,
   but its postcondition does not assert whether marker/index/slot/orphan
   material was deleted, retained under an explicit terminal-cleanup state, or
   read back correctly. Its two `read_slot` calls are assigned to `_`
   (`:3688-3690`). There is no `verify_slot_absent` assertion for slot 1 or
   orphan slot 99 in the fault rows.
3. The prescribed no-secret operation trace requires domain, operation, fault
   point, marker/index/slot identifiers, result class, cleanup proof, and
   public outcome. The actual trace is only
   `recovery case=<label> domain=<domain> result=<class> state=<state>`
   (`:3669-3676`). The Luna report's claim that 33 route calls prove the full
   matrix therefore overstates what the behavioral assertions demonstrate.

The underlying production route is not being faulted here: the deficiency is
the deterministic proof. `recover_startup` correctly uses `load_committed` for
Account and the domain registry (`:804-834`), and `load_committed` contains
port-level deletion/readback logic (`:1487-1543`), but source inspection is not
an executed recovery-matrix assertion.

**Final cycle-3 remediation, within the approved six paths:**

1. In `src-tauri/src/auth_session.rs`, make each table row declare expected
   first/restart result, exact port-visible marker/index/versioned-slot
   readback, and whether a terminal cleanup state is expected. After clearing
   any injected fault, construct a fresh façade and invoke
   `startup_recover` again for every required restart row, including corrupt
   marker; add a valid persisted Account + `drive-alpha` + `drive-beta` row.
2. Assert cleanup/readback through `KeyringPort`/`FakeKeyring` operations only:
   marker, index, current slot, and orphan slot must be present only when the
   row's terminal condition explicitly permits it; otherwise assert verified
   absence. Assert the public result, terminal state, no access, no connected
   state, and no public success on both first and restart calls.
3. Emit a non-secret trace containing the required operation/fault/slot and
   cleanup-result fields. Update the Node source contract only if needed to
   bind the strengthened named behavioral matrix, and correct the Luna report
   and AC mapping. Do not alter Drive source, `lib.rs`, the candidate, or any
   user-owned path unless an approved need arises.

### P0-IMP-01 — Closed in cycle 2

`registered_accept_material` is absent. The former generic fault/seeding
helpers are absent from the session source, and the Node contract rejects their
names at `tests/nativeSessionCustody.test.mjs:217-230`. Rotation now enters the
registered ticketed flow through `login_with_registered_ticket`
(`src-tauri/src/auth_session.rs:3735-3750`), whose helper invokes the required
begin/take/complete façade sequence (`:3420-3455`). `accept_material` remains
only an engine-internal action reached from normal login completion and refresh
processing (`:501`, `:537-564`, `:641`); no test façade exposes it.

**Disposition:** PASS locally.

### P1-IMP-01 — Closed in cycle 2

The Node contract rejects the former state-mutating helper names and preserves
positive checks for the registered login entrypoints
(`tests/nativeSessionCustody.test.mjs:195-235`). Its behavioral Rust subtest
passes 27/27, so source checking remains supplemental to the runtime evidence.

**Disposition:** PASS locally.

## Retained local controls

- Native composition continues to use `NativeKeyring`, `NativeClock`,
  `NativeListener`, and `NativeProvider` (`src-tauri/src/auth_session.rs:2019-2030`).
- `DriveOperationGuard::Drop` remains the normal drain release
  (`src-tauri/src/drive_oauth.rs:206-258`), and `upload_resumable_file` retains
  its per-send ticket check (`:1348-1404`). The focused behavioral command
  passes the disconnect/logout/shutdown provider-send barrier test.
- The scoped `rustfmt` command passes. `cargo check` reports 18 retained
  baseline warnings; none is a D-GDA6 façade, adapter, guard, drain, or startup
  method warning. `SessionLifecycleState::LogoutPending` at
  `auth_session.rs:47` is the retained pre-existing warning in the changed
  source file and is not introduced by cycle 2.
- Typed recovery-phrase ingress, deny-before-effect behavior, terminal cleanup,
  W1 authority, closed IPC, Browser unchanged, and Mobile deferred retain their
  prior local status. They are not production proof.

## D-GDA6 decision mapping

| Decision | Independent disposition | Evidence |
|---|---|---|
| D-GDA6-01 | PASS locally | The second test-only credential-success authority is removed; tested material flow is registered-ticketed. |
| D-GDA6-02 | PASS locally | One native-default composition root and same-port deterministic injection remain. |
| D-GDA6-03 | PASS locally | Real guard/drop and actual resumable provider-send barrier remain covered. |
| D-GDA6-04 | FAIL | Recovery test calls the shared façade but lacks required restart/readback proof. |
| D-GDA6-05 | PARTIAL / not accepted | Commands and warning boundary pass, but recovery trace and AC claim do not prove the required bounded evidence. |
| D-GDA6-06 | PASS for authority; implementation exit blocked | Exact approval and cycle budget are valid; this independent cycle-2 review blocks promotion. |

## AC-GDA6 mapping

| AC | Independent disposition |
|---|---|
| AC-GDA6-01 | PASS locally. |
| AC-GDA6-02 | PASS locally. |
| AC-GDA6-03 | PASS locally — zero D-GDA6 warning; 18 itemized baseline warnings remain. |
| AC-GDA6-04 | PASS locally — retained Drive guard/drop and resumable-send race evidence. |
| AC-GDA6-05 | PASS locally — disconnect, logout, and shutdown barrier evidence remains green. |
| AC-GDA6-06 | PARTIAL / not accepted — provider race is covered, but recovery cleanup/readback is not asserted for every required row. |
| AC-GDA6-07 | FAIL — route convergence exists but restart evidence is not complete for Account plus both Drive domains. |
| AC-GDA6-08 | FAIL — fault/absence rows run, but restart, slot/orphan cleanup, and required readback proof are incomplete. |
| AC-GDA6-09 | PASS locally/by scope. |
| AC-GDA6-10 | PARTIAL / not accepted — all commands pass but the required recovery evidence/trace is incomplete. |
| AC-GDA6-11 | OPEN externally. |
| AC-GDA6-12 | FAIL for implementation exit — independent re-review blocks at cycle 2/3. |

## Independent command record

| Exact command | Result | Wall time / count |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, exit 0 | 1.963 s; 12/12; nested behavioral Rust gate 27/27 |
| `node --test tests/googleDriveContract.test.mjs` | PASS, exit 0 | 0.154 s; 6/6 |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS, exit 0 | 20.318 s; 8/8 including PostgreSQL authority evidence |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS, exit 0 | 0.198 s; 8/8 |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs` | PASS, exit 0 | 0.756 s |
| `cargo check --manifest-path src-tauri/Cargo.toml --message-format=short` | PASS, exit 0 | 1.345 s; 18 retained baseline warnings; zero D-GDA6 warning |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, exit 0 | 1.700 s; 27/27; 380 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS, exit 0 | 28.035 s; 407/407 library tests; zero-test binary/doc targets passed |
| `npm run build` | PASS, exit 0 | 4.986 s; 1,764 modules transformed |
| `git diff --check 0a07c0b^ 0a07c0b` | PASS, exit 0 | 0.036 s |

## Local versus external gates

This re-review proves only source structure and deterministic local behavior.
It did not access a real OS keyring, Google OAuth or Drive provider,
Supabase/Edge/RLS, a clean Windows VM, Android/device UAT, signing, release
publication, deployment, monitoring, or production approval. All remain
**OPEN**.

No push, PR, merge, provider action, keyring action, deployment, release, or
production approval occurred in this review.

## Required disposition

Do not accept or promote `0a07c0b628bfaf6569f8f506213e56d314470dd0` as
D-GDA6 complete. A final Luna cycle-3 implementation may address only
P0-IMP-02-R2 and the resulting report/contract evidence within the already
approved six paths, followed by a fresh Terra re-review. If cycle 3 fails,
stop and require a new amendment.

## Version Diff

- Adds an independent cycle-2 implementation re-review against the exact
  approved D-GDA6 candidate and focused implementation commit.
- Confirms P0-IMP-01 and P1-IMP-01 are closed locally and retained Drive
  controls pass.
- Blocks on a source-backed recovery matrix gap: missing restart and asserted
  port-level cleanup/readback evidence for required fault rows.
- Keeps all external and production gates explicitly open.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-26 | stable | FAIL/BLOCK: local commands pass and test-only credential authority is closed, but the required startup-recovery restart/readback matrix remains incomplete; cycle 2/3. | recorded by this review commit | Terra 5.6 |
