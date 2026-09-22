---
version: "0.1.0b"
created_at: "2026-09-21T11:24:11+07:00,Bohr,01a0c224-76c8-7900-8fe6-ba516aec21f6,gpt-5.6-luna,max"
last_update: "2026-09-21T11:36:39+07:00,Bohr,01a0c224-76c8-7900-8fe6-ba516aec21f6,gpt-5.6-luna,max"
status: "need review"
superseded_by: null
attributes:
  domain: "meeting-intelligence/security"
  doc_type: "rca"
  scope: "N3 remediation R2 preparation and lifecycle scope boundary"
  execution_state: "PREPARED_NEEDS_PARENT_REVIEW"
  verification_status: "NOT_RUN"
  change_risk: "C3/HIGH"
  author_id: "01a0c224-76c8-7900-8fe6-ba516aec21f6"
  nickname: "Bohr"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  write_lease: ".brain/rca/2026-09-21-meeting-unlock-replay-r2.md only"
---

# RCA — R2 explicit unlock lifecycle and replay identity boundary

## Status and preparation boundary

`PREPARED_NEEDS_PARENT_REVIEW`. The new R2 worktree and branch were created
from the verified root HEAD, but the R1 overlay was not copied because the
authorized source partition is insufficient for a correct production lifecycle
witness. No source, test, manifest, Cargo, frozen-snapshot, or engine file was
edited. No test, build, Cargo command, child worker, commit, push, PR, merge,
deployment, provider, user database, app, or OS-keyring action was run.

The existing approved repair scope remains valid: SI/D8 custody repair,
atomic replay preservation, and the local Genesis table-cap experiment. This
RCA requests only the smallest additional native lifecycle API needed to make
that scope enforceable; it does not request a product-policy change.

R2 is **bare base-only/unprepared**: there is no implementation handoff, no
test handoff, and no PASS claim. The requested `auth_session.rs` extension is
not approved and requires a USER decision.

## Symptom

The frozen R1 review remains `N4/G1 FAIL/BLOCKED` for two HIGH findings:

1. F001: an account-free local vault can be accepted from a persisted active
   row and native identity without an explicit, still-current local-owner
   unlock session. A stored active row, OS key availability, or an injected
   `TrustedIdentityContext` is not unlock proof.
2. F002: a matching replay made with a newly constructed attempt can return the
   new attempt transaction ID while the durable event retains the original
   transaction ID. Replay must return the durable original ID.

## Evidence

- The independently verified failed report is
  `docs/verification/implementation-reports/2026-09-21-meeting-intelligence-g1-contract-r1.md`,
  SHA256
  `19E41145C5869C67B314E4398D0831D7679B46E7F6B44DFB3B766FB20F70E9E6`.
- Parent read-only inspection found `OperationDrain` admission is owned by
  `AccountOperationGuard` and released only on `Drop` in
  `src-tauri/src/auth_session.rs:143-199`; retaining that guard for a
  long-lived unlock session could block logout/shutdown drain.
- The same inspection found that comparing `native_user_id` alone cannot
  distinguish logout followed by a new login as the same account.
- `production_lifecycle()` is private at `auth_session.rs:1558`; the broker
  has `generation()` but public `SessionStatus`/`native_user_id` do not expose
  the epoch/generation at `auth_session.rs:1582-1587,1936-1940`.
- R2 preparation has created, but not seeded, the new isolated checkout:
  `C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r2-20260921`,
  branch `codex/meeting-intelligence-repair-r2-20260921`, HEAD
  `b336f33ec400a38f003a0665c121069a87a543ac`.
- The original root overlay inventory and frozen R1 inputs were read. Overlay
  transfer and source/destination hash comparison are `NOT_RUN` by this
  checkpoint.
- Current root source hashes remain unchanged:

  | Path | SHA256 |
  |---|---|
  | `src-tauri/src/genesis_adapter.rs` | `2F2C3599931E4D68E3F6B050528491D7E99A957906D51A51A2E70E55DC7A15A9` |
  | `src-tauri/src/lib.rs` | `9948C2160DA422A406C5CF3F5B719E18F6C8F105ABC857E06407DFD3F3F7316F` |
  | `src-tauri/src/auth_session.rs` | `95F4114647932A6B072C7BBA1C3D1E06B42EA525FA939A14820FC5AF33A95EC7` |
  | `src-tauri/src/device_identity.rs` | `C5041F4689126F71055FDBD6C9A40189397F54C591B6BCA0A21C88FF8B94B93F` |
  | `src-tauri/Cargo.toml` | `54BEB684AA73B89F030B6627E9A98F9EC63A02EEC5CB8C1039394D70BE2B4AAD` |
  | `src-tauri/Cargo.lock` | `E756A52BB041C5F43D4674E46FD0781CBB66F2649B3CD0385C15F02FEE8B9D07` |
  | `src/tauri.ts` | `578FF537CC7F67432B93F8CB4F60475C1DA1B4B023059C51E0447578272C3782` |

Documentary lease interval: `2026-09-21T11:24:11+07:00` through
`2026-09-21T11:36:39+07:00`; source/test/Cargo leases were never acquired.

## Root Cause

The proposed R1 repair partition can validate injected identity and private
ciphertext, but it cannot observe the production auth-session epoch that
invalidates a long-lived unlock after logout, restart, lock, revoke, or an
account transition. A cached account ID is replayable across a same-account
relogin. Holding `AccountOperationGuard` across the unlock lifetime is not a
valid substitute because the guard owns operation-drain admission. A shadow
broker would create a second authority and diverge from the existing native
lifecycle.

The minimal missing boundary is therefore a read-only lifecycle witness from
the existing `auth_session` broker. F002 is independent: the adapter's replay
response must use the transaction ID read from the durable matching event,
not the current caller attempt.

## Why the issue escaped detection

R1 exercised an injected trusted context and account-bound checks, but did not
exercise a real production lifecycle witness through logout and same-account
relogin. The replay test reused the original attempt, so it did not create the
alternate-attempt condition. The schema/resource-limit failure had previously
blocked the original runtime path, and native UI/OS-keyring tests were
explicitly outside scope.

## Proposed Prevention and smallest scope extension

Request USER approval to add exactly one future source path/API. Parent review or
lease issuance cannot substitute for that USER approval:

`src-tauri/src/auth_session.rs` — a `pub(crate)` read-only lifecycle witness
hook, conceptually `read_lifecycle_witness()`. It must be a single coherent
read under the existing broker, returning the opaque native account identity,
the account lifecycle generation/epoch, and the account lifecycle state. It
must expose no secret, token, key byte, keyring handle, ciphertext, or guard
internals. The hook must not acquire or retain `OperationDrain`, and the
witness must come from the existing broker rather than a shadow broker or
cached authority.

### ACCOUNT witness versus VAULT authority

The ACCOUNT witness handles logout, relogin, account switch, and other native
account lifecycle transitions. The broker must capture and revalidate the
whole tuple `(native_user_id, epoch/generation, lifecycle_state)` atomically;
separate user-ID and epoch reads are not sufficient. A generation change on
logout followed by login as the same account must make the prior witness
stale. Restart must not depend on a numeric epoch surviving: the new process
starts with no persisted account witness authority and must observe a fresh
broker state.

The VAULT authority is independent. A bounded in-memory unlock session must
require an explicit native local-owner unlock and must separately enforce
process lifetime, vault lock/revoke state, binding/owner, current review
revision, and capture/frontier guards. ACCOUNT epoch equality neither grants
vault permission nor invalidates a vault lock, revoke, review revision, or
frontier guard. Restart starts locked with no persisted unlock authority.

The normal short `AccountOperationGuard`/check-plus-drain remains held through
each protected commit to close recheck-to-commit races. Only the long-lived
unlock session must not retain that guard; no active operation may be held for
the session lifetime.

The repair code would capture the coherent ACCOUNT witness at explicit
local-owner unlock and re-read/revalidate it before each protected mutation,
while independently revalidating the VAULT session and its guards:

No `lib.rs`, `device_identity.rs`, Cargo manifest/lock, UI, provider, or new
policy path is requested. If the existing module visibility makes the
`pub(crate)` hook impossible without another file, stop and request that exact
additional path for USER approval before editing it; do not broaden the lease
implicitly.

```text
existing auth_session broker
  └─ atomic ACCOUNT witness { opaque native_user_id, epoch/generation, state }
       ├─ explicit local-owner unlock captures witness in bounded session state
       └─ each protected mutation atomically re-reads and compares the tuple
            ├─ unchanged account witness
            │    └─ independently check VAULT session/lifetime/lock/revoke/
            │       review-revision/frontier guards, then short guard+drain
            └─ stale account witness or failed vault guard -> fail closed

restart -> locked process; no persisted unlock authority; epoch need not persist

matching durable event -> return stored original transaction_id
```

The existing exact later repair lease remains:
`src-tauri/src/genesis_adapter.rs`,
`src-tauri/src/meeting_intelligence_schema.rs`,
`contracts/meeting-intelligence-v1.yaml`,
`tests/meetingIntelligenceContract.test.mjs`, and the later remediation
report, plus the requested `auth_session.rs` hook only after explicit USER
approval and a fresh exact lease. No implementation or test handoff is granted
by this preparation record.

The prior R1 crypto evidence remains bounded to its injected fixture lane; it
does not prove native lifecycle enforcement or VAULT authority. The R1 gate
therefore remains `N4/G1 FAIL/BLOCKED`.

## Executable acceptance plan (not run)

After scope approval and a fresh exact lease, the worker must add injected
lifecycle/key backends and prove:

1. Account-free local mode works without signup/cloud auth only after explicit
   native local-owner unlock; an active row, OS key availability, or injected
   test context alone fails closed.
2. An injected lifecycle backend exercises logout followed by login as the same
   account and proves the old atomic ACCOUNT `(identity, epoch, state)` witness
   is stale; account switch and restart also fail closed without relying on a
   persisted numeric epoch.
3. An injected VAULT backend locks or revokes between read and commit and proves
   the independent vault session, review-revision, and frontier guards reject
   the mutation even when the ACCOUNT witness is unchanged.
4. An injected logout/drain control proves a long-lived unlock session retains
   no `OperationDrain`; a short per-operation `AccountOperationGuard`/check+
   drain still spans each protected commit and closes the race.
5. Concurrent lifecycle transition coverage proves capture and revalidation
   are coherent broker reads, never mixed separate identity/epoch snapshots.
6. A matching alternate-attempt replay returns the original durable
   `transaction_id`; changed payloads still conflict, and the five writes,
   capture-before-validation frontier, CAS, post-commit evidence, reopen, and
   rollback/recovery contracts remain intact.
7. Existing SI/D8 encrypted envelope, zeroization, AAD/scope/hash, profile and
   stored identity-link model provenance checks remain fail-closed.

Native OS keyring/UI, app/lib integration, provider, real meeting, production,
and release evidence remain `NOT_RUN` in this local contract lane.

## Version diff

| Version | Difference |
|---|---|
| new -> 0.1.0b | Recorded R2 F001/F002 RCA, the separate ACCOUNT/VAULT control boundary, the smallest USER-approval-only auth-session witness request, and preparation-only evidence. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | need review | R2 preparation stopped at the auth-session lifecycle witness boundary; no source or test repair performed. | working-tree; base b336f33 | Bohr / 01a0c224-76c8-7900-8fe6-ba516aec21f6 |
