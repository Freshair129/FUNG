---
version: "2.0.0b"
created_at: "2026-08-25T23:14:30+07:00,Luna 5.6"
last_update: "2026-08-26T00:27:00+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-report"
  scope: "D-GDA6-01..06 cycle 2/3; registered native session broker evidence"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "e6722db3f310d82270d3c6879a7749fb15e4f366"
  approved_candidate_sha256: "2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F"
  implementation_source_commit: "05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2"
  terra_cycle1_review_commit: "26507132f23a44d9b674b19ed1e336505b67edae"
  implementation_commit: "not embedded; bound by the containing focused commit"
  fix_cycle: "2/3"
  terra_status: "pending fresh independent implementation review"
---

# Native Session Broker registered-entrypoint evidence — Luna 5.6 implementation report

## Disposition

D-GDA6 cycle 2-of-3 is implemented within the approved six-path write set. The
final required local/static command matrix passes. This is a Luna implementation
candidate only: fresh independent Terra review of the focused commit remains
required, and no external or production gate is claimed complete.

Cycle 2 addresses Terra findings P0-IMP-01, P0-IMP-02, and P1-IMP-01 from
review commit `26507132f23a44d9b674b19ed1e336505b67edae`. The production
Drive guard/drop and resumable provider-send implementation was preserved; no
Drive source was changed.

## Authority and provenance

Exact authorization used for this implementation:

```text
approve D-GDA6-01 through D-GDA6-06 — commit e6722db3f310d82270d3c6879a7749fb15e4f366 — SHA-256 2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F
```

- Approved candidate commit: `e6722db3f310d82270d3c6879a7749fb15e4f366`
- Approved candidate SHA-256: `2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F`
- Cycle-1 implementation: `05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2`
- Terra cycle-1 FAIL/BLOCK: `26507132f23a44d9b674b19ed1e336505b67edae`
- Cycle-2 base before edits: `26507132f23a44d9b674b19ed1e336505b67edae`
- Implementation commit: bound by the containing focused commit because a
  commit cannot embed its own hash

The approved future write set remains:

1. `src-tauri/src/auth_session.rs`
2. `src-tauri/src/drive_oauth.rs`
3. `src-tauri/src/lib.rs`
4. `tests/nativeSessionCustody.test.mjs`
5. `tests/googleDriveContract.test.mjs`
6. this Luna implementation report

Actual cycle-2 changed paths are only:

1. `src-tauri/src/auth_session.rs`
2. `tests/nativeSessionCustody.test.mjs`
3. this Luna implementation report

The other three allowed source/contract paths were inspected and left
byte-stable. Existing user-owned modified and untracked paths were preserved,
unstaged, and untouched.

## Terra finding disposition

### P0-IMP-01 — removed second credential-success authority

`RegisteredBrokerEntrypoints::registered_accept_material` was removed. The
dead generic `SessionLifecycle::{fail_keyring_at, fail_cleanup,
fail_provider_with}` hooks and their façade wrappers were removed as well. The
test-only success seeding helpers were also removed so no test façade can write
authenticated credential state directly.

Rotation, staged-write, pre-marker, and post-marker tests now use the same
ticketed path:

```text
registered_login_begin
  -> registered_login_take_for_exchange
  -> registered_login_complete
```

Deterministic behavior is injected through shared `FakeKeyring` and
`FakeProvider` port instances. Keyring seed, fault, deletion, verification, and
readback operations use those ports directly; the registered broker façade is
not used as a state-mutating test control surface. Drive credential setup uses
the normal `begin_drive_work` plus `DriveOperationGuard` plus `commit_drive`
ticketed operation.

### P0-IMP-02 — complete startup-recovery route matrix

`native_behavioral_registered_startup_recovery_both_domains_fault_matrix` now
calls the non-test `RegisteredBrokerEntrypoints::startup_recover` route for
each of three registered domains: Account (`desktop-session`), `drive-alpha`,
and `drive-beta`.

The route-backed matrix contains seven fault cases for each domain:

- staged index
- missing referenced slot
- corrupt marker
- old-slot deletion
- compact-index write
- post-marker cleanup/absence verification
- corrupt referenced target

It also contains verified `NoEntry` and marker-missing/orphan cleanup success
cases for each domain. Each success case invokes the same startup route again
on the shared keyring state, producing 12 restart invocations in total. Every
case records a non-secret case label/domain/result/state, asserts the public
result and terminal state, clears the injected fault through the fake port for
readback, and asserts no access/connected/public success on the failure path.
No test mutates the fake keyring map directly.

Final route counts are 33 `startup_recover` calls: 21 fault-path calls and 12
success/restart calls across the three domains.

### P1-IMP-01 — stronger negative and positive contract evidence

`tests/nativeSessionCustody.test.mjs` now rejects the removed credential-success
façade and all removed test-only state-mutating helper names, including the
selected seed/fault helpers. The behavioral contract also requires the Rust
output to contain the ticketed-login rotation test and the full registered
startup-recovery matrix test. This keeps source checks as negative guardrails
while using the behavioral suite for the positive route proof.

## Retained controls and non-regression boundary

- Native production composition remains `NativeKeyring`, `NativeClock`,
  `NativeListener`, and `NativeProvider` through the existing composition root.
- `DriveOperationGuard` ownership and `Drop` release remain unchanged.
- Existing resumable provider-send pre/post ticket checks and real barrier race
  evidence remain unchanged and pass.
- Typed zeroizing recovery ingress, deny-before-keyring/provider ordering,
  `AccountOperationGuard`, terminal cleanup failure, W1 authority, and closed
  typed IPC remain in force.
- `src-tauri/src/native_auth.rs` remains outside the write set and unchanged.
- Browser source/behavior is unchanged; Mobile remains deferred.
- No provider, keyring, OAuth, deployment, release, PR, merge, or external
  action was performed.

## D-GDA6 decision mapping

| Decision | Cycle-2 local disposition | Evidence |
|---|---|---|
| D-GDA6-01 | Implemented locally; Terra pending | One non-test façade remains the production/test route; direct material-acceptance façade and dead generic hooks are absent. |
| D-GDA6-02 | Preserved and implemented locally; Terra pending | Tests inject shared fake ports only; no second lifecycle engine or state-mutating broker helper remains. |
| D-GDA6-03 | Preserved and passed locally; Terra pending | Drive source was unchanged; focused behavioral suite retains real guard/drop and resumable send barrier evidence. |
| D-GDA6-04 | Implemented locally; Terra pending | Account plus `drive-alpha` and `drive-beta` use the same non-test `startup_recover` route for 33 matrix calls. |
| D-GDA6-05 | Passed locally; Terra pending | Required source, negative, behavioral, full regression, format, build, and diff gates pass; 18 compiler warnings are unchanged baseline. |
| D-GDA6-06 | Cycle-2 authorization and scope satisfied | Exact Boss approval preceded implementation; cycle budget is 2/3; fresh Terra implementation review remains required. |

## AC-GDA6 mapping

| Acceptance criterion | Cycle-2 local disposition |
|---|---|
| AC-GDA6-01 | PASS locally: registered production wrappers and behavioral tests use one non-test façade; removed direct material-acceptance and generic test authority. |
| AC-GDA6-02 | PASS locally: one native-default composition root and same-port deterministic injection remain in use. |
| AC-GDA6-03 | PASS locally: cargo check reports no D-GDA6 warning; the only `auth_session.rs` warning is the pre-existing `LogoutPending` baseline variant. |
| AC-GDA6-04 | PASS locally/preserved: real `DriveOperationGuard`, Drop release, and actual resumable provider-send barrier remain covered. |
| AC-GDA6-05 | PASS locally/preserved: disconnect, logout, and shutdown drain/fence behavior remains green. |
| AC-GDA6-06 | PASS locally: failure cases publish no credential/access/connected/public success; cleanup/readback is asserted through fake ports. |
| AC-GDA6-07 | PASS locally: Account and two registered Drive domains enter the same non-test `startup_recover` route. |
| AC-GDA6-08 | PASS locally: staged index, slot, marker, old-slot deletion, compact index, post-marker, corrupt target, NoEntry, orphan cleanup, and restart cases are route-backed and fail closed where required. |
| AC-GDA6-09 | PASS locally/by scope: custody, ordering, cleanup failure, W1, IPC, Browser, and Mobile boundaries are retained. |
| AC-GDA6-10 | PASS locally: required Node/Rust/cargo/rustfmt/build/diff commands pass with exact counts below. |
| AC-GDA6-11 | OPEN externally: clean VM, real OS keyring, Supabase/Edge/RLS, Google OAuth/provider, device/UAT, signing, release, deployment, monitoring, and production approval remain unrun. |
| AC-GDA6-12 | OPEN for implementation exit: fresh independent Terra review of the cycle-2 focused commit is pending; two of three fix cycles are used. |

## Exact verification evidence

Durations below are local Windows observations from the final post-fix matrix.
Exit `0` means the command passed.

| Exact command | Final result | Duration / count |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, exit 0 | 34.888 s; 12/12 Node tests; nested Rust behavioral gate 27/27 |
| `node --test tests/googleDriveContract.test.mjs` | PASS, exit 0 | 0.102 s; 6/6 |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS, exit 0 | 19.630 s; 8/8; executable PostgreSQL evidence 19.014 s |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS, exit 0 | 0.170 s; 8/8 |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs` | PASS, exit 0 | 1.015 s |
| `cargo check --manifest-path src-tauri/Cargo.toml --message-format=short` | PASS, exit 0 | 4.022 s; 18 unchanged baseline warnings; zero D-GDA6 warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, exit 0 | 1.825 s command; 27/27 behavioral tests |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS, exit 0 | 407/407 library tests; 22.41 s library suite; zero-test binary/doc targets passed |
| `npm run build` | PASS, exit 0 | 9.60 s Vite build; 1,764 modules transformed |
| `git diff --check -- src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs tests/nativeSessionCustody.test.mjs tests/googleDriveContract.test.mjs` | PASS, exit 0 | no whitespace errors |

The first post-conversion focused run failed five assertions because tests
still assumed the old authenticated-in-memory shortcut and counted shutdown's
expected fake keyring deletes as zero effects. Those tests were converted to
restart/port observations, then the final focused, Node, and full matrices
passed. This intermediate failure is not included as final acceptance.

## Warning inventory

The final non-test `cargo check` reports 18 existing warnings. They remain
outside this D-GDA6 slice:

- `src/lib.rs`: `PairedDeviceInput`, `upsert_paired_device`,
  `list_paired_devices`, `revoke_paired_device`, `paired_device_upsert`,
  `paired_device_list`, `paired_device_revoke`, `fungwire_local_endpoint`.
- `src/auth_session.rs`: `SessionLifecycleState::LogoutPending`.
- `src/device_identity.rs`: `ENROLLMENT_PROOF_TTL`, `NativeEnrollmentProof`,
  `enrollment_timestamp_ms`, `validate_device_label`,
  `sign_pending_enrollment_challenge`, `device_enrollment_proof`,
  `ensure_identity_in_dir`, `read_legacy_seed`, `public_key_b64_in_dir`.

No D-GDA6 façade, entrypoint, adapter, guard, drain, provider-send, or startup
symbol is named by the final compiler output. The test build repeats 10 of
those baseline warnings; it adds no D-GDA6 warning.

## Source hash inventory before the containing fix commit

| Path | SHA-256 |
|---|---|
| `src-tauri/src/auth_session.rs` | `9DB1FA7B40D1A394714AB26DE3BD09E253103EFE967B9AE21D87A40950762AC3` |
| `src-tauri/src/drive_oauth.rs` | `4540FDD9EF6A89B63BDB054C4FF4DC827EA5F32ED96AC36FAFE87AC61CE5B56B` |
| `src-tauri/src/lib.rs` | `03477C27679ED7657F4EE316021034044D95BBDBD09C2EA83E51CC1F13875B59` |
| `tests/nativeSessionCustody.test.mjs` | `57C869159BB5E1A08D6AB4A84B57CEBFBEE3386B7A9404ADE8D2BDC964F52697` |
| `tests/googleDriveContract.test.mjs` | `38C9B5EA1022FBD7E93DB8BC7CEC91C4B413AA5C36FF5D096A7B7A993253F4F8` |

## Limitations and external gates

This report proves local/static source reachability, deterministic injected-port
behavior, registered-equivalent Drive evidence retained from cycle 1, and the
required automated matrix only. It does not prove a real OS keyring on a clean
Windows VM, real Supabase/Edge/RLS grants, real Google OAuth/Drive transport,
device/UAT, signing, release, deployment, monitoring, or production approval.
All such gates remain OPEN. No secret values are recorded here.

Fresh Terra implementation review of the containing cycle-2 commit is the next
governed gate. If Terra fails cycle 2, one fix cycle remains. If cycle 3 fails,
the implementation must stop and require a new amendment; this report does not
authorize a fourth cycle or any scope expansion.

## Version Diff

- `1.0.0b` → `2.0.0b`: cycle-2 remediation of Terra P0-IMP-01,
  P0-IMP-02, and P1-IMP-01.
- Removed direct test-only credential-success and dead generic lifecycle fault
  seams; moved deterministic controls to shared fake ports.
- Rebuilt the startup recovery matrix through the non-test route for Account
  and two registered Drive domains, including fault, absence, orphan, and
  restart evidence.
- Strengthened Node negative/positive contract evidence and retained cycle-1
  Drive guard/send proof.
- No external action, push, PR, merge, deployment, release, or provider/keyring
  action occurred.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 2.0.0b | 2026-08-26 | candidate | D-GDA6 cycle 2/3 implemented; final local matrix green; fresh Terra review pending | bound by containing commit | Luna 5.6 |
| 1.0.0b | 2026-08-25 | candidate | D-GDA6 cycle 1/3 implemented; Terra cycle-1 review returned FAIL/BLOCK | `05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2` | Luna 5.6 |
| 0.1.0b | 2026-08-25 | candidate | Approved exact-byte D-GDA6 amendment; implementation not started | `e6722db3f310d82270d3c6879a7749fb15e4f366` | Luna 5.6 |
