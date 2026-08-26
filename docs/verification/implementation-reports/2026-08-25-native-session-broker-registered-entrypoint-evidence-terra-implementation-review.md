---
version: "1.0.0"
created_at: "2026-08-25T23:45:32+07:00,Terra 5.6"
last_update: "2026-08-25T23:45:32+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-review"
  scope: "D-GDA6-01..06; independent implementation review; cycle 1/3"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "e6722db3f310d82270d3c6879a7749fb15e4f366"
  approved_candidate_sha256: "2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F"
  reviewed_implementation_commit: "05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2"
  reviewed_implementation_parent: "ffb6195e2b735a0350238eadb666bcfcbabbefae"
  verdict: "FAIL/BLOCK"
  fix_cycle: "1/3"
---

# Native Session Broker Registered-Entrypoint Evidence — Terra 5.6 Independent Implementation Review, Cycle 1

## Verdict

**FAIL / BLOCK.** The prescribed local command matrix passes at
`05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2`, and the implementation materially
converges the production façade, native adapters, Drive guard/drop, and resumable
provider boundary. It does not yet prove the two mandatory no-test-only-authority
and full-startup-recovery conditions.

Two source-backed P0s remain. First, the new test-only
`RegisteredBrokerEntrypoints::registered_accept_material` at
`src-tauri/src/auth_session.rs:943-948` calls `SessionLifecycle::accept_material`
directly. The rotation, staged-failure, pre-marker, and post-marker tests invoke
that helper at lines 3604, 3619, 3998, and 4032. This bypasses the non-test
registered login begin/ticket/exchange/complete path, so it is a direct
fake-only success authority prohibited by D-GDA6-01 and AC-GDA6-01.

Second, the one test that enters the non-test `startup_recover` façade
(`native_behavioral_registered_startup_recovery_both_domains_fault_matrix`,
lines 3543-3596) covers only five injected cases: staged index, slot, marker,
old-slot deletion, and compact index. It does not execute the required recovery
route matrix for restart after each prior success, post-marker failure, verified
`NoEntry`, or marker-missing/corrupt-target/orphan cleanup. The separate tests
for those states call `begin_refresh`, `registered_accept_material`, or
`shutdown`, not `startup_recover`. This fails D-GDA6-04 and AC-GDA6-07/-08.

This is cycle **1/3**. The exact approved six-path write set can remediate both
findings; no new amendment is required for cycle 2. This review is local/static
only and is not production evidence.

## Provenance and scope integrity

| Check | Independent result | Disposition |
|---|---|---|
| Approved candidate | `e6722db3f310d82270d3c6879a7749fb15e4f366` is an ancestor of the target. The candidate file is unchanged at the target. | PASS |
| Candidate SHA-256 | Current candidate bytes hash to `2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F`. | PASS — exact Boss approval binding retained |
| Candidate Terra review | `ffb6195e2b735a0350238eadb666bcfcbabbefae` records PASS for documentation only. | PASS — correctly bounded, not implementation acceptance |
| Reviewed target | Target `05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2`; parent `ffb6195e2b735a0350238eadb666bcfcbabbefae`. | PASS |
| Target write set | Exactly six approved paths changed: Luna report, `auth_session.rs`, `drive_oauth.rs`, `lib.rs`, `nativeSessionCustody.test.mjs`, and `googleDriveContract.test.mjs`. | PASS |
| Candidate preservation | `git diff --quiet e6722db3... HEAD -- <candidate path>` exits 0. | PASS |
| Diff whitespace | `git diff --check 05e49d7^ 05e49d7` exits 0. | PASS — hygiene only |
| Review isolation | This report path was absent before review. Existing user-modified and untracked paths were observed and not staged or altered. | PASS |

## Independent findings

### P0-IMP-01 — Test-only material acceptance remains a second success authority

**Affected:** D-GDA6-01, D-GDA6-02, D-GDA6-05; AC-GDA6-01, -02, -03, -10.

The non-test graph is real: `RegisteredBrokerEntrypoints` owns the mutex at
lines 892-914; the native singleton constructs it with `NativeKeyring`,
`NativeClock`, `NativeListener`, and `NativeProvider` at lines 2165-2177; and
registered login wrappers use its normal ticketed operations at lines 2611-2714.
The deterministic tests also instantiate that generic façade with the same
named ports at lines 3443-3451. This is a substantial improvement over D-GDA5.

However, `#[cfg(test)] registered_accept_material` is newly added at lines
942-948 and invokes `lifecycle.accept_material(material)` without an admitted
login ticket, pending login, listener callback, provider exchange, or
`registered_login_complete`. The tests named above use it to create successful
or post-marker credential states. It is therefore not merely observation or
port fault injection: it is a test-only state-mutating credential success path
that production wrappers cannot call.

The full Rust run independently reports the retained generic test hooks
`SessionLifecycle::{fail_keyring_at, fail_cleanup, fail_provider_with}` at
lines 742-750 as unused in the test build. Those hooks are not the P0 by
themselves, but confirm that the test seam has not been reduced to dependency
injection plus observation as D-GDA6 requires.

**Cycle-2 remediation within approved paths:** remove
`registered_accept_material` and the dead generic test hooks. Drive the
rotation, staged-write, pre-marker, and post-marker tests through the same
`registered_login_begin` -> `registered_login_take_for_exchange` ->
`registered_login_complete` façade path, with `FakeProvider` and `FakeKeyring`
as the only deterministic dependencies. Extend
`tests/nativeSessionCustody.test.mjs` to reject test-only state-mutating
lifecycle helpers, rather than only selected helper names.

### P0-IMP-02 — Startup-recovery route is shared, but its required fault/restart matrix is incomplete

**Affected:** D-GDA6-04, D-GDA6-05; AC-GDA6-07, -08, -10.

`RegisteredBrokerEntrypoints::startup_recover` is non-test code at lines
1076-1078. Production setup calls `auth_session::startup_recover()` from
`src-tauri/src/lib.rs:2982`; that public function delegates to the same façade
at `auth_session.rs:2197-2199`. The behavioral recovery test calls
`broker.startup_recover()` at lines 3580 and 3595, so the route convergence
itself is present.

The required matrix is nevertheless incomplete. Its local `cases` array at
lines 3544-3550 contains only five stages. The final successful restart is a
single case at lines 3590-3596. It does not run the shared recovery route after
each prior success; does not fault the post-marker cleanup case through that
route; and does not prove normal absence or corrupt/missing-marker/orphan
outcomes through that route. Existing `NoEntry`/corrupt marker checks use
`begin_refresh` (lines 3965-3970), while the post-marker cleanup case uses
`registered_accept_material` (lines 4029-4042). Neither is startup recovery.

`write_keyring_slot` at lines 1259-1266 delegates to `KeyringPort::write` and
does not mutate the fake map directly; that limited setup boundary is
acceptable. The deficiency is coverage and result tracing, not a direct-map
mutation finding.

**Cycle-2 remediation within approved paths:** replace the five-case recovery
test with a table-driven trace for Account and at least two registered Drive
domains. For every listed order, seed/fault only via `FakeKeyring`'s
`KeyringPort` operations, call `startup_recover`, then assert its public result,
terminal state, keyring cleanup/readback result, and no access/connected/public
success. Include staged index, slot, marker, old-slot deletion, compact index,
restart after each prior success, post-marker failure, verified absent marker
and slots, and marker-missing/corrupt/orphan cases. Keep fault labels and slot
identifiers non-secret in the Rust test output/report.

### P1-IMP-01 — Source-contract test does not detect the remaining test-only authority

**Affected:** D-GDA6-05; AC-GDA6-10.

`tests/nativeSessionCustody.test.mjs:185-210` checks selected generic names
and selected `#[cfg(test)]` helper patterns, but not
`registered_accept_material`, `seed_active`, `seed_drive_active`, or a general
test-only state-mutating façade method. The Node contract passes 12/12 while
the P0 helper remains. This is a detection gap, not a replacement for P0-IMP-01.

**Cycle-2 remediation:** add exact negative assertions for the forbidden
state-mutating test-only façade methods and positive assertions that behavioral
credential material flows through ticketed registered-login completion.

### P1-IMP-02 — `#[rustfmt::skip]` is an acceptable scoped boundary, not an implementation bypass

`src-tauri/src/lib.rs:42-43` applies `#[rustfmt::skip]` only to unchanged,
out-of-scope `mod native_auth;`. The exact required rustfmt invocation still
checks all three D-GDA6 Rust files and passed. The attribute neither suppresses
`auth_session.rs`/`drive_oauth.rs` nor changes runtime behavior, while
`native_auth.rs` is outside the six-path authority. It is therefore accepted
for this cycle. A future approved change that touches `native_auth.rs` must
format that module normally; this review does not waive that future obligation.

## Retained local controls

- The production native default composition root and deterministic same-port
  injection are present.
- `DriveOperationGuard::from_lease` at `drive_oauth.rs:212-219` owns the real
  `OperationDrain`; its `Drop` at lines 254-258 is the normal release path.
- `upload_resumable_file` at lines 1348-1404 checks its operation before and
  after provider start and every provider send. The race test holds the real
  guard through the deterministic `ResumableProviderPort::send_chunk` barrier,
  runs disconnect/logout/shutdown, releases no drain manually, and rejects the
  stale post-send result (`auth_session.rs:3844-3907`). This satisfies the
  registered-equivalent direct-façade allowance locally.
- Typed recovery phrase ingress, deny-before-effect checks, cleanup terminal
  states, W1 authority evidence, closed IPC, Browser unchanged, and Mobile
  deferred retain their prior local status. They are not production proof.

## D-GDA6 decision mapping

| Decision | Independent disposition | Evidence |
|---|---|---|
| D-GDA6-01 | FAIL | P0-IMP-01 leaves a test-only credential-success path outside the registered production operation graph. |
| D-GDA6-02 | PASS locally | One non-test generic façade is constructed with native defaults; tests inject the same port types. |
| D-GDA6-03 | PASS locally | Real guard/drop and actual resumable provider-send boundary are exercised with deterministic barriers. |
| D-GDA6-04 | FAIL | Same startup façade is called, but P0-IMP-02 omits required fault/restart/absence cases through that route. |
| D-GDA6-05 | PARTIAL / not accepted | Compiler and command evidence pass, but negative evidence does not prove no test-only lifecycle authority. |
| D-GDA6-06 | PASS for authorization; implementation exit fails | Exact candidate review/approval and cycle budget are valid. This independent review is cycle 1/3 and returns BLOCK. |

## AC-GDA6 mapping

| AC | Independent disposition |
|---|---|
| AC-GDA6-01 | FAIL — P0-IMP-01. |
| AC-GDA6-02 | PASS locally. |
| AC-GDA6-03 | PASS locally — `cargo check` has no D-GDA6 façade/adapter/guard/drain/startup warning; 18 reported warnings are retained baseline items. |
| AC-GDA6-04 | PASS locally under the direct shared-façade allowance. |
| AC-GDA6-05 | PASS locally — source and race test show quiesce before drain wait, actual guard Drop, and bounded completion. |
| AC-GDA6-06 | PASS locally — stale post-send result is rejected and no result is returned. |
| AC-GDA6-07 | FAIL — shared route exists but does not cover all required recovery cases/domains. |
| AC-GDA6-08 | FAIL — post-marker, verified absence, corrupt/missing/orphan, and restart-after-each-success evidence is outside the recovery route. |
| AC-GDA6-09 | PASS locally/by scope. |
| AC-GDA6-10 | PARTIAL / not accepted — commands pass, but source negative evidence is incomplete. |
| AC-GDA6-11 | OPEN externally. |
| AC-GDA6-12 | FAIL for implementation exit — independent implementation review is BLOCK at cycle 1/3. |

## Independent command record

| Exact command | Result | Wall time / count |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, exit 0 | 3.131 s; 12/12 |
| `node --test tests/googleDriveContract.test.mjs` | PASS, exit 0 | 0.798 s; 6/6 |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS, exit 0 | 21.946 s; 8/8 including PostgreSQL authority evidence |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS, exit 0 | 0.770 s; 8/8 |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs` | PASS, exit 0 | 1.698 s |
| `cargo check --manifest-path src-tauri/Cargo.toml --message-format=short` | PASS, exit 0 | 2.064 s; 18 retained baseline warnings; zero D-GDA6 warning |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, exit 0 | 2.178 s; 27/27; 380 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS, exit 0 | 29.553 s; 407/407 |
| `npm run build` | PASS, exit 0 | 8.770 s; 1,764 modules transformed |
| `git diff --check 05e49d7^ 05e49d7` | PASS, exit 0 | 0.566 s |

The focused and full Rust test builds also emit dead-code diagnostics for the
three retained `SessionLifecycle` test helpers identified in P0-IMP-01. They do
not make the prescribed commands fail, but they corroborate that the test-only
seam is not fully converged.

## Local versus external gates

Local/static work above proves only source structure, deterministic-port
behavior, and the stated command results. It did not access a real OS keyring,
Google OAuth or Drive provider, Supabase/Edge/RLS, a clean Windows VM, Android
or device UAT, signing, release publication, deployment, monitoring, or
production approval. All of those gates remain **OPEN**.

No source/test change, push, PR, merge, provider action, keyring action,
deployment, release, or production approval occurred in this review.

## Required disposition

Do not accept or promote `05e49d7b6c5eba7999d0e72ba30d8da5e0dd07f2` as
D-GDA6 complete. A Luna cycle-2 implementation may address P0-IMP-01,
P0-IMP-02, and P1-IMP-01 only within the already approved six paths. It must
then receive a fresh Terra implementation review. The remaining budget is
2/3; if cycle 3 fails, stop and require a new amendment.

## Version Diff

- Adds the independent D-GDA6 cycle-1 implementation review against the exact
  approved candidate bytes and focused target commit.
- Records complete local command evidence and distinguishes it from external
  and production gates.
- Identifies a direct test-only credential-success seam and incomplete shared
  startup-recovery fault evidence as blocking P0s.
- Accepts the narrowly scoped `#[rustfmt::skip]` boundary for unchanged
  out-of-scope `native_auth.rs` without treating it as a runtime or test bypass.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-25 | stable | FAIL/BLOCK: local command matrix passes, but a test-only material-success path and incomplete shared startup-recovery matrix remain; cycle 1/3. | recorded by this review commit | Terra 5.6 |
