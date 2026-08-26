---
version: "1.0.0"
created_at: "2026-08-25T17:32:30+07:00,Terra 5.6"
last_update: "2026-08-25T17:32:30+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-review"
  scope: "D-GDA5-01..06; Fix3-of-3 independent implementation review"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "bcc672decd3ae35cf7875ca2f984a7919aafbe6b"
  approved_candidate_sha256: "B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8"
  reviewed_implementation_commit: "837b04476b720553997719b9be71da9470029d6e"
  reviewed_implementation_parent: "e5155d4c1ed05357c913ebe311dbfbb69e18bb16"
  verdict: "FAIL/BLOCK"
  fix_cycle: "3/3 exhausted"
---

# Native Session Broker Production-Path Remediation — Terra 5.6 Independent Implementation Re-review, Fix 3

## Verdict

**FAIL / BLOCK.** All prescribed local commands pass at
`837b04476b720553997719b9be71da9470029d6e`, and Fix3 materially improves the
source-level Drive transition and recovery implementation. It does not close
the mandatory actual-entrypoint evidence gate.

`cargo check` independently reports that the non-test `SessionLifecycle`
methods `logout`, `shutdown`, and `disconnect_drive` are unused in the
production build. The focused behavioral suite uses those exact methods on a
`SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>` instead
of the registered production command route. The Drive race proof likewise does
not execute a registered `broker_drive_*` command, `DriveOperationGuard`, or a
resumable provider send. These are source-backed P0 evidence failures against
the approved single-live-engine and production-route requirements.

This is Fix3-of-3. The authorized implementation-fix cycles are exhausted. A
new approved amendment is required before any further source or test change.

## Provenance and scope integrity

| Check | Independent result | Disposition |
|---|---|---|
| Approved candidate commit | `bcc672decd3ae35cf7875ca2f984a7919aafbe6b` resolves. | PASS |
| Candidate bytes | Candidate and target both contain blob `951a9115b44582c98456467b7e8125674d7514b9`; SHA-256 is `B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8`. | PASS — exact approval binding retained |
| Candidate Terra document review | Prior document-only PASS commit `baf030d65698ef2c060d10464cd85262706a27dd` remains a candidate review, not implementation acceptance. | PASS, correctly bounded |
| Reviewed target | Target `837b04476b720553997719b9be71da9470029d6e` has parent `e5155d4c1ed05357c913ebe311dbfbb69e18bb16`. | PASS |
| Target manifest | Four permitted paths changed: Luna report, `auth_session.rs`, `drive_oauth.rs`, and `nativeSessionCustody.test.mjs`. `lib.rs` and `googleDriveContract.test.mjs` are unchanged; no unapproved implementation path is in the target diff. | PASS — bounded |
| Candidate preservation | The amendment blob is identical at candidate and target. | PASS |
| Review-path cleanliness | The Fix3 review path was clean before this review. Existing user-modified and untracked files were observed and left untouched. | PASS |
| Diff whitespace | `git diff --check 837b044^ 837b044` exits 0. | PASS — hygiene only |

## Independent findings

### P0-IMP-01 — Behavioral matrix still exercises dead generic lifecycle seams, not the registered production graph

**Severity: P0. Affected gates: D-GDA5-01, D-GDA5-05; AC-GDA5-01, -02, -06, -10.**

The target wires production logout and shutdown through the two-phase route:
`broker_session_logout` calls `begin_terminal_transition` and
`finish_terminal_transition` at `src-tauri/src/auth_session.rs:2382-2404`, and
the production shutdown wrapper does the same at `2413-2425`. Production Drive
disconnect uses `begin_drive_disconnect` / `finish_drive_disconnect` at
`1760-1771`.

However, the generic `SessionLifecycle::logout`, `shutdown`, and
`disconnect_drive` methods remain compiled in the non-test build at
`src-tauri/src/auth_session.rs:700-710` and `835-839`. `cargo check` and the
focused Rust test both report all three as `never used`. The behavioral tests
nevertheless call `broker.logout()` at `3221`, `3231`, `3432`, and `3658`,
`broker.shutdown()` at `3439` and `3447`, and `broker.disconnect_drive()` at
`3484`.

The whole behavioral harness is `SessionLifecycle<FakeKeyring, FakeClock,
FakeListener, FakeProvider>` (`2960-3135`), while the registered singleton is
`SessionLifecycle<NativeKeyring, NativeClock, NativeListener, NativeProvider>`
at `1688-1699`. Fix3's named login wrappers improve the login path, but neither
the registered production listener loop (`2096-2138`) nor the production
logout/shutdown/Drive wrapper graph is executed by the behavioral matrix.
The Node assertion is only a source-pattern check at
`tests/nativeSessionCustody.test.mjs:193-210`; it cannot close a compiler-proven
dead production seam.

This violates the approved prohibition on a generic test-only core/dead
lifecycle seam and fails the required actual registered adapter graph evidence.

### P0-IMP-02 — Drive source fences are present, but the mandatory production-route race evidence is absent

**Severity: P0. Affected gates: D-GDA5-02, D-GDA5-05; AC-GDA5-01, -03, -10.**

Source inspection confirms a real improvement: production disconnect releases
the lifecycle mutex before `wait_empty` (`auth_session.rs:1760-1771`), production
logout/shutdown obtain drains before waiting (`2382-2425`), and the resumable
loop has lifecycle checks immediately before and after each `.put()`
(`drive_oauth.rs:1242-1256`). This is necessary but not sufficient for the
required evidence gate.

The deterministic race test instead constructs a fake generic broker and calls
`begin_drive_disconnect` directly (`auth_session.rs:3492-3527`). Its worker
manually releases the drain before calling `finish_drive_operation`
(`3505-3508`); it does not exercise the real `DriveOperationGuard::drop`
ordering (`drive_oauth.rs:218-222`). It never invokes
`broker_drive_list_archives` (`935-972`), `broker_drive_upload_archive`
(`975-1024`), `broker_drive_restore` (`1367-1486`), or
`upload_resumable_file` (`1201-1284`). No test deterministically interleaves a
real provider-boundary/chunk send with disconnect, logout, or shutdown.

The supporting Node contract is also source-pattern evidence only at
`tests/nativeSessionCustody.test.mjs:212-225`. As a result, no test proves that
the registered guard owns the drain through actual provider work, rejects the
post-send stale result, and lets the production transition complete without a
deadlock. AC-GDA5-03 requires that evidence, not merely the source shape.

### P0-IMP-03 — Required startup recovery matrix is only generic fake-port evidence

**Severity: P0. Affected gates: D-GDA5-04, D-GDA5-05; AC-GDA5-05, -10.**

`startup_recover` delegates to the non-test lifecycle recovery method in the
registered native singleton at `auth_session.rs:1818-1823`; this source mapping
is an improvement. The five-ordering recovery test, however, calls
`broker.recover_startup()` on the same fake-port harness at
`3254-3305`. It never enters `startup_recover`, the registered Drive startup
route, or `NativeKeyring` (`1486-1502`). The fault state is injected by a map
backed `FakeKeyring` (`2960-3028`) and by direct slot mutation
(`3268-3285`).

The candidate permits injectable ports for deterministic behavior, but the
final Fix2 gate expressly requires production-entrypoint or demonstrated
production-equivalent evidence for Account plus registered Drive recovery. The
target provides only an asserted source mapping and fake-port execution; it
does not demonstrate the registered startup entrypoint/fault boundary. This is
insufficient to promote the complete failure-atomic matrix to a local hard-gate
PASS. A real OS-keyring proof remains external, but an in-process registered
entrypoint test with an injected native-equivalent keyring was still required
for this local gate.

## Retained locally passing controls

| Control | Disposition | Evidence boundary |
|---|---|---|
| B1-B5 recovery phrase ingress | PASS locally, bounded | `RecoveryPhrase` moves deserialized input into `Zeroizing<String>` at `drive_oauth.rs:236-245`; `broker_drive_restore` accepts it at `1367-1376`. Framework buffer zeroization remains unproved/external. |
| Source-level Drive fences | PASS as source inspection only | Pre/post `drive_check(ticket)` surrounds list, upload, download, delete, and each resumable chunk boundary. It does not cure P0-IMP-02's missing execution evidence. |
| Cleanup failure state | PASS locally, bounded | The generic matrix asserts `cleanup_failed` at `3436-3449` and post-marker cleanup at `3639-3652`. Production exit-path execution remains blocked by P0-IMP-01. |
| W1 authority/replay proof | PASS locally | The prescribed PostgreSQL authority test passes 8/8; this is not real Edge/RLS proof. |
| Typed IPC / Browser / Mobile scope | PASS locally/by scope | Named command contracts pass; no Browser or Mobile source changed in this target. |

## Command evidence

All commands below were run independently on `837b04476b720553997719b9be71da9470029d6e`. Passing commands are evidence only for the behavior they execute.

| Required command | Result | Wall duration and limitation |
|---|---|---|
| `node --test tests/nativeSessionCustody.test.mjs` | PASS — 12/12; Rust child 26/26 | 2.75 s. Includes source-pattern assertions and the fake-port matrix; does not execute registered production adapters. |
| `node --test tests/googleDriveContract.test.mjs` | PASS — 6/6 | 0.53 s. Contract/source coverage; no provider race execution. |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS — 8/8 | 20.57 s. PostgreSQL 17 migration/rollback proof only. |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS — 8/8 | 0.61 s. Auth-flow contract only. |
| `rustfmt --edition 2021 --check src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs` | PASS | 0.58 s; check mode only. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS with 18 warnings | 1.53 s. Crucially, warnings include new lifecycle methods `logout`, `shutdown`, and `disconnect_drive` as never used at `auth_session.rs:700-839`; this blocks AC-GDA5-02. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS — 26/26 | 1.84 s. Confirms the fake-port matrix and reproduces the same three dead-seam warnings. |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS — 406/406 | 25.30 s. Full suite retains the same warnings; it does not replace the missing production-route evidence. |
| `npm run build` | PASS — 1,764 modules | 8.28 s. Web build only. |
| `git diff --check 837b044^ 837b044` | PASS | 0.38 s. Whitespace integrity only. |

## D-GDA5 and AC-GDA5 disposition

| Gate | Independent disposition | Basis |
|---|---|---|
| D-GDA5-01 | FAIL | P0-IMP-01: compiler-proven dead non-test lifecycle seams are the paths behavioral tests use; actual registered adapter graph is not exercised. |
| D-GDA5-02 | FAIL | P0-IMP-02: source fences exist, but no deterministic registered provider/chunk race proves drain and stale-result behavior. |
| D-GDA5-03 | PASS locally, bounded | Typed `RecoveryPhrase` custody is retained; framework residual memory remains outside local proof. |
| D-GDA5-04 | FAIL | P0-IMP-03: failure matrix remains fake-port/source mapping rather than registered startup-entrypoint/equivalent evidence. |
| D-GDA5-05 | FAIL | Mandatory actual-entrypoint, race, and recovery evidence is not closed despite green command exits. |
| D-GDA5-06 | PASS, provenance only | Exact candidate commit/hash are retained; implementation acceptance is blocked. |
| AC-GDA5-01 | FAIL | No behavioral run covers the registered listener/callback and native adapter route end-to-end. |
| AC-GDA5-02 | FAIL | `cargo check` identifies three non-test lifecycle methods that are unused in production. |
| AC-GDA5-03 | FAIL | No deterministic registered provider/chunk interleaving validates disconnect, logout, and shutdown. |
| AC-GDA5-04 | PASS locally, bounded | Direct typed recovery ingress remains. |
| AC-GDA5-05 | FAIL | Five orderings execute only through `FakeKeyring` and direct map mutation, not the required startup route/equivalent proof. |
| AC-GDA5-06 | FAIL | Generic test coverage exists, but the required production exit-path proof is absent and the generic exit methods are dead in production. |
| AC-GDA5-07 | PASS locally | W1 executable authority test passes 8/8. |
| AC-GDA5-08 | PASS locally | Bounded contract tests retain a typed closed command surface. |
| AC-GDA5-09 | PASS by scope | Browser unchanged and Mobile deferred; focused contract passes. |
| AC-GDA5-10 | FAIL | Green commands do not satisfy the required actual-entrypoint/race/fault execution evidence. |
| AC-GDA5-11 | OPEN | Local checks do not prove OS keyring, Supabase/Edge/RLS, Google provider, device/UAT, signing, release, deployment, monitoring, or production state. |
| AC-GDA5-12 | FAIL | Fresh independent Fix3 implementation review returns FAIL/BLOCK. |

## Limitations and external gates

This review neither invoked a real Google OAuth/provider nor accessed an OS
keyring credential. It does not prove clean-VM behavior, Supabase/Edge/RLS,
device or provider UAT, signing, release publication, deployment, monitoring,
or production approval. Those gates remain open regardless of this BLOCK.

No user-owned dirty or untracked file was staged or altered. No source/test
change, push, PR, merge, provider action, deployment, release, or production
approval occurred in this review.

## Required disposition

Do not accept or promote `837b04476b720553997719b9be71da9470029d6e` as
D-GDA5 complete. The approved Fix3 cycle is exhausted. Any remediation must
start with a new amendment that explicitly authorizes the necessary test and
production-route changes, then obtain fresh candidate hash approval and an
independent Terra review.

## Version Diff

- Adds the final independent Fix3 implementation review against the exact
  approved candidate bytes and target commit.
- Records all prescribed local command results, including the compiler-proven
  dead lifecycle seams and their mismatch with the behavioral test route.
- Distinguishes source improvements and retained local controls from the
  mandatory production-route evidence that remains unproved.
- Marks the implementation-fix budget as exhausted; no additional code change
  is authorized under D-GDA5-01 through D-GDA5-06.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-25 | stable | FAIL/BLOCK: Fix3 passes local commands but leaves compiler-proven dead production lifecycle seams and missing registered race/recovery execution evidence; fix cycles exhausted. | recorded by this review commit | Terra 5.6 |
