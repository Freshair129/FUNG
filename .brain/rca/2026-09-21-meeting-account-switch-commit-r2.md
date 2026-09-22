---
version: "0.1.2b"
created_at: "2026-09-21T18:15:33+07:00,Banach,01a0c378-b728-7722-a412-f2155fb45b73,gpt-5.6-luna,max"
last_update: "2026-09-21T18:46:49+07:00,Banach,01a0c378-b728-7722-a412-f2155fb45b73,gpt-5.6-luna,max"
status: "need review"
superseded_by: null
attributes:
  domain: "meeting-intelligence/security"
  doc_type: "rca"
  scope: "N3 remediation R2 account-switch to protected-commit boundary"
  execution_state: "CONFIRMED_BLOCKER_USER_SCOPE_REQUIRED"
  verification_status: "FOCUSED_REPRODUCTION_PASS"
  change_risk: "C3/HIGH"
  author_id: "01a0c378-b728-7722-a412-f2155fb45b73"
  nickname: "Banach"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  write_lease: ".brain/rca/2026-09-21-meeting-account-switch-commit-r2.md only"
---

# RCA — Account switch after final lifecycle check and before Genesis commit

## Status and boundary

`CONFIRMED_BLOCKER_USER_SCOPE_REQUIRED`. This record was created only after the
focused injected-broker reproduction passed. It records a remaining HIGH
native-account-lifecycle boundary in the approved R2 implementation. USER/Boss
approval is required for any new broker critical-section or commit-fence API;
the parent may review and recommend but cannot grant that scope. This record
does not add an auth policy, broker commit-fence implementation, provider path,
UI path, or release approval. Previous RCA and implementation reports remain
frozen.

The evidence is fixture-only: the existing `RegisteredBrokerEntrypoints` path
with its fake keyring/provider ports was used. No provider request, real
keyring, user database, renderer, network, or native app was used.

## Symptom

The native broker can switch from account-a to account-b after the protected
operation has completed its final lifecycle witness and
`AccountOperationGuard::check`, but before
`GenesisTransaction::commit_transaction`. The current short account guard does
not prevent this login/account-switch transition. The protected meeting commit
can therefore succeed under the previously checked lifecycle context while the
broker is already authenticated as another account.

The reproduction uses an account-free local-owner vault with no
`bound_account_ref`. It demonstrates a native broker witness A->B race; it does
not switch the vault binding, grant account-bound vault permission, or prove a
vault-owner transfer.

## Evidence

The focused command was run in the R2 worktree with the sole approved target,
`--offline --locked`, `CARGO_INCREMENTAL=0`, profile debug disabled, and
`TAURI_CONFIG={"bundle":{"resources":[]}}`:

```text
cargo test --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\Cargo.toml --offline --locked --lib r2_registered_broker_account_switch_after_final_check_is_unclosed
exit 0
```

The test injects the existing registered broker as the lifecycle witness
source, authenticates native account-a, acquires the real short
`AccountOperationGuard`, and switches to account-b from the existing
`post_fence_hook` after the final check. The observed result was:

| Observation | Result |
|---|---|
| pre-hook native broker user | `account-a` |
| pre-hook account generation | unchanged at `1` |
| post-hook native broker user | `account-b` |
| post-hook lifecycle state | `authenticated` |
| account generation across switch | unchanged at `1` |
| protected Genesis commit | non-idempotent success |
| provider/network/real keyring | not used |

The reproduction is distinct from the passing vault operation-fence test and
the passing registered-broker logout-drain test. Those transitions are now
ordered through their respective controls; this login/account-switch path is
not.

## Root cause

1. `SessionLifecycle::begin_login` changes the state to `LoginPending` but does
   not quiesce or drain already admitted protected operations.
2. `SessionLifecycle::complete_login` accepts replacement identity material
   without advancing the account generation for an ordinary account switch.
3. `AccountOperationGuard::check` validates the existing operation ticket,
   account epoch, and generation. Because the login switch preserves those
   values, the guard remains valid even after the broker witness changes.
4. The R2 protected path performs the final native witness and account-guard
   checks before the injected post-check hook, then commits. A second witness
   read alone would not prove or close the final-check-to-commit race.

## Why the issue escaped detection

The prior acceptance coverage exercised same-account logout/relogin
invalidation, terminal logout drain, and vault lock/revoke serialization. The
logout path advances the lifecycle generation and clears pending operations;
the login/account-switch path does neither. Earlier tests also used a fixture
witness rather than the registered broker's own live witness during this
specific post-final-check interleaving, so they could not expose the identity
replacement while the operation was in flight.

## Prevention and smallest scope request

The smallest proposed USER/Boss-scope change is an existing-broker
critical-section/commit-fence API around the already approved protected
operation, not a general authentication-policy refactor. The actual R2
protected-operation order is: retain the short `AccountOperationGuard` for the
operation, acquire the VAULT operation read fence for canonical
`Storage::path`, perform the native lifecycle/vault/storage checks under that
VAULT fence, and commit to Genesis storage while the account guard remains
live. The unimplemented broker fence would be nested inside that existing
VAULT/storage fence; it would not change native identity directory custody or
storage-root resolution:

- add one internal `RegisteredBrokerPort`/registered-broker API that acquires a
  non-reentrant `AccountCommitFence` from the checked witness and holds the
  existing broker lifecycle critical section through the protected Genesis
  commit;
- use this lock order only: retain the admitted short
  `AccountOperationGuard`, hold the existing VAULT/storage operation fence,
  acquire the broker commit fence once, and then—under that acquired broker
  critical section—atomically re-read the expected lifecycle witness and
  revalidate the expected operation ticket before calling the existing
  `GenesisTransaction::commit_transaction`. A precheck before broker-fence
  acquisition is optional for early rejection only and is never sufficient for
  the authoritative check-to-commit boundary;
- while the broker fence is held, no nested broker lifecycle entry, login,
  logout, refresh, callback, provider, keyring, UI, or other broker call is
  allowed; the fence is non-reentrant;
- release the broker fence immediately after the Genesis commit returns
  (success or error), before the owning `AccountOperationGuard` is dropped;
  then allow the existing VAULT/storage fence and short account-drain guard to
  unwind. The broker fence is commit-only and is never retained by the
  long-lived unlock session;
- add a real registered-broker regression that attempts native account A->B
  after the final check and asserts the switch/commit cannot cross the fence
  and no partial durable effect is created.

This request is an explicit USER/Boss approval boundary for the existing
broker's lifecycle critical section and its focused tests. Parent review/G1 may
recommend or reject the request but cannot authorize it. It is not
authorization in this R2 lane to invent or implement a new broker commit-fence
API. Until separately approved, the account-switch boundary remains a blocker
for independent HIGH acceptance even though the unaffected R2 checks may
continue.

## Stop conditions and evidence status

Stop this lane if closure requires changes outside the approved auth-session
and existing protected-commit paths, a new product policy, UI/provider wiring,
or a permanent fail-closed state that removes the usable offline local-owner
path. The current reproduction is `FOCUSED_REPRODUCTION_PASS`; closure is
`NOT_RUN` pending explicit USER/Boss approval. Parent review/G1 cannot grant
that approval.

## Version diff

- New documentary RCA only; no prior frozen RCA or report was modified.
- `0.1.2b`: clarified the actual VAULT/account/storage order and required
  atomic lifecycle-witness plus operation-ticket revalidation under the
  acquired broker fence; pre-fence checks are only optional early rejection.
  The fence remains unimplemented and USER/Boss approval-only.
- `0.1.0b -> 0.1.1b`: corrected the authority boundary from parent scope to
  explicit USER/Boss scope, narrowed the evidence to native broker account A->B
  on an account-free vault, and added lock-order/non-reentrant/commit-only
  boundaries to the unimplemented proposal.
- No source, manifest, lockfile, engine, UI, provider, or release file was
  changed by this RCA.
- The R2 implementation test evidence and the earlier frozen reports remain
  separate records and retain their original hashes.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.2b | 2026-09-21 | need review | Clarified atomic-under-broker-fence witness and ticket revalidation, actual VAULT/account/storage order, non-reentrancy, and release order; no fence authority implemented. | uncommitted; base b336f33ec400a38f003a0665c121069a87a543ac | Banach / 01a0c378-b728-7722-a412-f2155fb45b73 |
| 0.1.1b | 2026-09-21 | need review | Corrected the proposed broker critical-section scope to require USER/Boss approval and documented lock-order, non-reentrancy, and commit-only boundaries without implementing it. | uncommitted; base b336f33ec400a38f003a0665c121069a87a543ac | Banach / 01a0c378-b728-7722-a412-f2155fb45b73 |
| 0.1.0b | 2026-09-21 | need review | Recorded confirmed account-switch-to-commit race and smallest parent-scope request; no authority implementation | uncommitted; base b336f33ec400a38f003a0665c121069a87a543ac | Banach / 01a0c378-b728-7722-a412-f2155fb45b73 |
