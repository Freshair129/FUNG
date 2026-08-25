---
version: "1.0.0b"
created_at: "2026-08-25T23:14:30+07:00,Luna 5.6"
last_update: "2026-08-25T23:14:30+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-report"
  scope: "D-GDA6-01..06 cycle 1/3; native session broker registered-entrypoint evidence"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "e6722db3f310d82270d3c6879a7749fb15e4f366"
  approved_candidate_sha256: "2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F"
  implementation_source_commit: "ffb6195e2b735a0350238eadb666bcfcbabbefae"
  implementation_commit: "not embedded; bound by the containing focused commit"
  fix_cycle: "1/3"
---

# Native Session Broker registered-entrypoint evidence — Luna 5.6 implementation report

## Disposition

D-GDA6 cycle 1-of-3 is implemented within the approved six-path write set. The
complete required local/static command matrix passes on the final run. This is
a Luna implementation candidate, not an independent Terra implementation PASS
and not production evidence. Fresh Terra review of the focused implementation
commit remains required before D-GDA6 exit.

The implementation converges production registration and deterministic tests
on one non-test typed `RegisteredBrokerEntrypoints<K, C, L, P>` façade. The
production composition root uses `NativeKeyring`, `NativeClock`,
`NativeListener`, and `NativeProvider`; tests inject the same named ports into
that same graph. The Drive matrix holds an actual `DriveOperationGuard` across
the real `upload_resumable_file` provider-send boundary, and normal drain
release occurs only from `DriveOperationGuard::drop`. Startup recovery tests
enter the same non-test `startup_recover` route used by application setup.

## Approval, provenance, and scope

Exact Boss authorization:

```text
approve D-GDA6-01 through D-GDA6-06 — commit e6722db3f310d82270d3c6879a7749fb15e4f366 — SHA-256 2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F
```

- approved candidate commit: `e6722db3f310d82270d3c6879a7749fb15e4f366`
- approved candidate SHA-256: `2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F`
- candidate path: `docs/specs/2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment.md`
- implementation source commit: `ffb6195e2b735a0350238eadb666bcfcbabbefae`
- implementation cycle: `1/3`
- implementation commit: the focused commit containing this report; its hash
  is intentionally not embedded because a commit cannot contain its own hash

Changed paths, and no others:

1. `src-tauri/src/auth_session.rs`
2. `src-tauri/src/drive_oauth.rs`
3. `src-tauri/src/lib.rs`
4. `tests/nativeSessionCustody.test.mjs`
5. `tests/googleDriveContract.test.mjs`
6. `docs/verification/implementation-reports/2026-08-25-native-session-broker-registered-entrypoint-evidence-implementation-luna-report.md`

`src-tauri/src/native_auth.rs`, Browser, Mobile, UI, dependency, lockfile,
configuration, provider, deployment, and release paths were not changed.
Existing user-modified and untracked files were preserved and are not part of
this implementation candidate.

Final pre-commit SHA-256 inventory for the five implementation/source-contract
files:

| Path | SHA-256 |
|---|---|
| `src-tauri/src/auth_session.rs` | `2E506DE9EB31EDED440CC5B3B58EE769001C3C96C2C2AD451EDFD7E9A44FC1DD` |
| `src-tauri/src/drive_oauth.rs` | `4540FDD9EF6A89B63BDB054C4FF4DC827EA5F32ED96AC36FAFE87AC61CE5B56B` |
| `src-tauri/src/lib.rs` | `03477C27679ED7657F4EE316021034044D95BBDBD09C2EA83E51CC1F13875B59` |
| `tests/nativeSessionCustody.test.mjs` | `347875395268C9F00A5212622DA07B9B471F35F0FB5A581E64D1213A420ACD93` |
| `tests/googleDriveContract.test.mjs` | `38C9B5EA1022FBD7E93DB8BC7CEC91C4B413AA5C36FF5D096A7B7A993253F4F8` |

## D-GDA6 decision mapping

| Decision | Cycle 1 local disposition | Evidence |
|---|---|---|
| D-GDA6-01 | IMPLEMENTED; Terra review pending | One non-test typed façade is used by registered production wrappers and deterministic tests. Dead generic lifecycle seams were removed/converged; there is no test-only lifecycle engine. |
| D-GDA6-02 | IMPLEMENTED; Terra review pending | `production_lifecycle` constructs the shared façade with all four native adapters. Tests call the same constructor with port implementations, without a second state/lock/recovery algorithm. |
| D-GDA6-03 | IMPLEMENTED; Terra review pending | Registered-equivalent Drive tests retain a real guard through `upload_resumable_file`, use deterministic send barriers, and cover disconnect/logout/shutdown, denial, stale rejection, drain completion, deadlock bounds, and anti-resurrection. |
| D-GDA6-04 | IMPLEMENTED; Terra review pending | Account and all registered Drive domains enter the shared `startup_recover` composition route. Faults are injected only through `KeyringPort` and cover the required crash/restart matrix. |
| D-GDA6-05 | SATISFIED locally; Terra review pending | Exact compiler, source-contract, behavioral, regression, build, formatting, warning-inventory, and whitespace gates are recorded below. |
| D-GDA6-06 | AUTHORIZATION SATISFIED for cycle 1/3 | Candidate bytes/hash, fresh candidate Terra review, and exact Boss approval preceded implementation. Fresh Terra review of the resulting implementation commit is still required by AC-GDA6-12. |

## AC-GDA6 mapping

| AC | Cycle 1 local disposition |
|---|---|
| AC-GDA6-01 | PASS locally: production wrappers and deterministic tests use one non-test typed façade; no dead generic or test-only mirror authority remains. |
| AC-GDA6-02 | PASS locally: one native-default composition root and same-port deterministic injection use the same entrypoint graph. |
| AC-GDA6-03 | PASS locally: `cargo check` has zero warning attributable to D-GDA6; source inventory shows production callers. The unrelated baseline is itemized below. |
| AC-GDA6-04 | PASS locally: the registered-equivalent Drive matrix owns a real `DriveOperationGuard` through the actual resumable provider-send boundary. |
| AC-GDA6-05 | PASS locally: disconnect, logout, and shutdown release the lifecycle mutex before bounded drain wait, complete without deadlock, and reject pre/post-send stale work. |
| AC-GDA6-06 | PASS locally: winning transitions do not resurrect credential, marker/slot, access, archive publication, connected state, or public success. |
| AC-GDA6-07 | PASS locally: Account and each registered Drive domain execute the same non-test startup composition route with injection only at `KeyringPort`. |
| AC-GDA6-08 | PASS locally: staged index, slot, marker, old-slot deletion, compact index, restart, and post-marker faults fail closed; `NoEntry` is distinct from ambiguity and cleanup uncertainty remains `cleanup_failed`. |
| AC-GDA6-09 | PASS locally/by scope: typed zeroizing recovery ingress, deny-before-keyring/provider, `AccountOperationGuard`, terminal `cleanup_failed`, W1 authority, typed IPC, Browser unchanged, and Mobile deferred are retained. |
| AC-GDA6-10 | PASS locally: all required commands pass on the final run; exact commands, exit status, durations/counts, provenance, paths, and limitations are recorded without secret values. |
| AC-GDA6-11 | OPEN externally: clean Windows VM/real OS keyring, Supabase/Edge/RLS, Google OAuth/provider, device/UAT, signing, release, deployment, monitoring, and production approval are not exercised. |
| AC-GDA6-12 | OPEN for implementation exit: candidate review and exact Boss authorization are satisfied; fresh independent Terra review of this focused implementation commit is pending. Cycle budget used: 1/3. |

## Architecture and behavioral evidence

### One registered composition graph

- `RegisteredBrokerEntrypoints<K, C, L, P>` is compiled outside `#[cfg(test)]`.
- `NativeRegisteredBroker` fixes the production adapters to `NativeKeyring`,
  `NativeClock`, `NativeListener`, and `NativeProvider`.
- `production_lifecycle()` is the singleton production composition root.
- registered login/logout, Drive admission/disconnect, startup setup, and exit
  shutdown delegate into this façade.
- deterministic tests construct the same façade with `FakeKeyring`,
  `FakeClock`, `FakeListener`, and `FakeProvider`; only dependencies differ.

### Drive guard/drop and send-boundary matrix

- `DriveOperationLease` carries shared broker authority into
  `DriveOperationGuard::from_lease`.
- `DriveOperationGuard::drop` is the only normal Drive drain release. Broker
  finish removes the pending operation and does not release the drain again.
- tests pass the real guard into `upload_resumable_file`; deterministic provider
  barriers hold execution at the provider-send boundary.
- disconnect, account logout, and shutdown quiesce before waiting, then reject
  stale pre-send/post-send work and publish no stale archive or connected state.

### Startup recovery matrix

- application setup and deterministic tests call the same non-test
  `startup_recover` façade method.
- tests exercise Account and every registered Drive domain.
- staged-index, slot, marker, old-slot deletion, compact-index, restart, and
  post-marker failure are injected through `KeyringPort`, not internal-map
  mutation.
- ambiguous/fault results fail closed, stale state is not published, verified
  `NoEntry` remains normal absence, and uncertain cleanup remains terminal
  `cleanup_failed`.

## Exact verification evidence

All durations are wall-clock observations from this cycle on the local Windows
checkout. Exit `0` means the exact command passed.

| Exact command | Final result | Duration / count |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, exit 0 | 43.752 s; 12/12 Node tests; nested Rust behavioral gate passed |
| `node --test tests/googleDriveContract.test.mjs` | PASS, exit 0 | 0.333 s; 6/6 |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS, exit 0 | 43.790 s; 8/8; executable PostgreSQL test 41.465 s |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS, exit 0 | 0.321 s; 8/8 |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs` | PASS, exit 0 | 0.871 s on final pre-commit rerun |
| `cargo check --manifest-path src-tauri/Cargo.toml --message-format=short` | PASS, exit 0 | 32.762 s; 18 unrelated/pre-existing warnings, zero D-GDA6 warning |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, exit 0 | 4.119 s; 27/27, 380 filtered out |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS, exit 0 | 36.641 s; 407/407; binary/doc targets 0/0 |
| `npm run build` | PASS, exit 0 | 15.138 s; 1,764 modules transformed |
| `git diff --check -- src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs tests/nativeSessionCustody.test.mjs tests/googleDriveContract.test.mjs` | PASS, exit 0 | 0.059 s on final pre-commit rerun |

### W1 timing variance

The first final-matrix W1 attempt returned 7/8, exit 1, after 75.859 seconds
because the executable PostgreSQL harness did not become ready within its
30-second readiness bound; no W1 assertion or migration test ran in that
subtest. The exact command was rerun alone after the local PostgreSQL cache was
warm and passed 8/8 in 43.790 seconds. Parent read-only corroboration also
reported 8/8 in 36.76 seconds. The final evidence is the worker's own 8/8
rerun; the parent result is corroboration only. This timing variance is not
claimed as production database evidence.

### Scoped rustfmt boundary

The exact required command originally traversed `mod native_auth;` from the
allowed `lib.rs` and failed on pre-existing formatting drift in the unapproved
`src-tauri/src/native_auth.rs`. The scoped solution adds `#[rustfmt::skip]` only
to that module declaration in `lib.rs`. It does not apply a crate-wide skip,
does not suppress rustfmt for any changed D-GDA6 Rust file, and does not weaken
the required command: `auth_session.rs`, `drive_oauth.rs`, and `lib.rs` remain
explicit command arguments and are fully checked. Four pre-existing formatting
drifts in allowed `lib.rs` were formatted. `native_auth.rs` remains byte-for-
byte outside the diff.

## Warning inventory

`cargo check --message-format=short` completed with 18 warnings. None names a
new D-GDA6 façade, entrypoint, adapter, guard, drain, provider-send, or startup
route. The unrelated/pre-existing symbols are:

- `src/lib.rs`: `PairedDeviceInput`, `upsert_paired_device`,
  `list_paired_devices`, `revoke_paired_device`, `paired_device_upsert`,
  `paired_device_list`, `paired_device_revoke`, `fungwire_local_endpoint`.
- `src/auth_session.rs`: `SessionLifecycleState::LogoutPending`. This variant
  exists in source commit `ffb6195e2b735a0350238eadb666bcfcbabbefae`; it was
  not introduced by D-GDA6.
- `src/device_identity.rs`: `ENROLLMENT_PROOF_TTL`, `NativeEnrollmentProof`,
  `enrollment_timestamp_ms`, `validate_device_label`,
  `sign_pending_enrollment_challenge`, `device_enrollment_proof`,
  `ensure_identity_in_dir`, `read_legacy_seed`, `public_key_b64_in_dir`.

## Limitations and external gates

All external gates remain OPEN: clean-install restore on a clean Windows VM,
real OS keyring behavior, real Supabase/Edge/RLS grants, real Google OAuth and
Drive provider transport, Android/device/UAT proof, signing, release,
deployment, monitoring, and production approval. No real keyring write, OAuth
exchange, provider upload, external deployment, or other external action was
performed. Browser remains unchanged and Mobile remains deferred.

The next governed step is fresh independent Terra review of the exact focused
implementation commit. This report does not claim that review has passed.

## Version Diff

- `0.1.0b -> 1.0.0b`: implements the approved D-GDA6 architecture boundary in
  cycle 1/3: one registered façade/composition root, real Drive guard/drop and
  provider-send evidence, shared startup recovery, and bounded command proof.
- Adds a narrow `lib.rs` module-boundary rustfmt exclusion solely for unchanged,
  out-of-scope `native_auth.rs`; all changed D-GDA6 Rust files remain checked.
- Retains typed custody, deny-before-side-effect ordering, W1/IPC boundaries,
  Browser unchanged, Mobile deferred, and all external gates OPEN.
- No push, PR, merge, deploy, release, provider action, or keyring action.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0b | 2026-08-25 | candidate | D-GDA6 cycle 1/3 implemented with complete local matrix; fresh Terra implementation review pending | bound by containing commit | Luna 5.6 |
| 0.1.0b | 2026-08-25 | candidate | Approved exact-byte D-GDA6 amendment; implementation not started | `e6722db3f310d82270d3c6879a7749fb15e4f366` | Luna 5.6 |
