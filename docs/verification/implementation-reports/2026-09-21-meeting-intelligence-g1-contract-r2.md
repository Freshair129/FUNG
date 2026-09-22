---
version: "0.1.1b"
created_at: "2026-09-21T19:13:22+07:00,Harvey independent N4 reviewer,b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-21T19:21:50+07:00,Harvey independent N4 reviewer"
status: "need review"
superseded_by: null
attributes:
  doc_type: "implementation-report"
  domain: "meeting-intelligence"
  scope: "N4 G1 independent bounded verification of frozen N3 R2"
  risk: "HIGH"
  actual_identity: "Harvey / 01a0c3d4-0996-7ef3-a3c7-16a5e49a914b"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
---

# N4/G1 independent verification — frozen N3 R2 repair

## Verdict

**FAIL — BLOCKED / NOT_ACCEPTED.**

The frozen R2 repair closes the R1 explicit-unlock and replay gaps within the
bounded native contract tests, but it leaves a confirmed HIGH account-switch
commit race. The passing diagnostic is the proof of the open race; it is not a
security acceptance result. N5/N6/N7/N9/N13/N15 remain locked. N3
DONE/REVIEW_READY is not acceptance.

This review is independent of Banach
01a0c378-b728-7722-a412-f2155fb45b73 and Schrodinger
01a0c378-b816-7383-8394-a1f4d34ec457. No source, test, contract,
dependency, workflow, package, manifest, lock, runtime, or generated-artifact
repair was made. The only permitted write is this new root report.

## Assumptions and boundary

1. N3 is the native contract/schema/API foundation. UI and library integration
   are not expected until N13; unused new API alone is not a failure.
2. The diagnostic uses the registered broker façade with injected/fake
   keyring/provider ports and an account-free local-owner vault. It proves the
   ordering defect, not a bound-vault ownership transfer, real-provider
   behavior, or a production exploit.
3. Real OS keyring, user DB, UI, provider/model/runtime, device, CI, portable
   package, and production evidence are outside this bounded review and remain
   NOT_RUN.

## Frozen custody

The root and R2 worktree both resolve to
b336f33ec400a38f003a0665c121069a87a543ac. The root is intentionally dirty;
existing changes were preserved. The R2 branch is
codex/meeting-intelligence-repair-r2-20260921. The required new report path
was absent before writing.

The immutable author transfer ledger was independently used by hash, without
copying its 49-entry manifest:

- execution report SHA256:
  2C3CE96847316E6D55D4B1EBC15ED1A2F297C1E078FDF344DAC1435CAB08C026
- execution JSON SHA256:
  99B857171024BDAE8C923B57D5CA15E020B6D353184ED3BCE1006C181EA485A6
- frozen workflow SHA256:
  CE83DE3D4073EE01365D370B2B516816E7710FC5D529AB534D0F1E7C70D6CE6F
- frozen DAG SHA256:
  2E98F55D4D71AEB308EA0EF9906DC04DB580F5CEAEBE10901301BE06C98A1DFB
- conditional account-switch RCA SHA256:
  975CF8487045F3E3775071B1C9FDAEA8242540C1CD42D400B35CC38FCB0ACA76

The independent pre-write hash ledger below matched the frozen R2/root
records. The post-write release check must remain byte-identical for every
listed source/dependency/protected path.

| Frozen R2 path | Pre-write SHA256 | Post-write SHA256 |
|---|---|---|
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\auth_session.rs | 345FE8F4E0EC38B97CF2B6EDCAAB421D6AAD08263F5A316B9DE9A56D37ADDBF7 | 345FE8F4E0EC38B97CF2B6EDCAAB421D6AAD08263F5A316B9DE9A56D37ADDBF7 |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\genesis_adapter.rs | D756634950BE300698115F2AD55842CBB14A1CD9839954FBCB9A00CA75461756 | D756634950BE300698115F2AD55842CBB14A1CD9839954FBCB9A00CA75461756 |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\meeting_intelligence_schema.rs | 80B396198647E745F26B63ED283513D2579DA199A49137C47B38B95B6B34BBAD | 80B396198647E745F26B63ED283513D2579DA199A49137C47B38B95B6B34BBAD |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\contracts\meeting-intelligence-v1.yaml | 4EA0B4EB0A9CCACBCDF3410BFED0407AE024FABF73133A998E457FD245FBBC9D | 4EA0B4EB0A9CCACBCDF3410BFED0407AE024FABF73133A998E457FD245FBBC9D |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\tests\meetingIntelligenceContract.test.mjs | E3ED22375683DC1BF858CA65F68B1F2EF791FCA498A0689481EBCA68F936870D | E3ED22375683DC1BF858CA65F68B1F2EF791FCA498A0689481EBCA68F936870D |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\docs\verification\implementation-reports\2026-09-21-meeting-identity-custody-r2.md | 720992E5D65849FF4C60FB744E2C55C55850DF1FABED8090A58A025659171BAF | 720992E5D65849FF4C60FB744E2C55C55850DF1FABED8090A58A025659171BAF |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\Cargo.toml | ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E | ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E |
| C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\Cargo.lock | 2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E | 2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E |

| Root protected path | Pre-write SHA256 | Post-write SHA256 |
|---|---|---|
| C:\Users\pc\workspace\fung\src-tauri\src\lib.rs | 9948C2160DA422A406C5CF3F5B719E18F6C8F105ABC857E06407DFD3F3F7316F | 9948C2160DA422A406C5CF3F5B719E18F6C8F105ABC857E06407DFD3F3F7316F |
| C:\Users\pc\workspace\fung\src-tauri\src\device_identity.rs | C5041F4689126F71055FDBD6C9A40189397F54C591B6BCA0A21C88FF8B94B93F | C5041F4689126F71055FDBD6C9A40189397F54C591B6BCA0A21C88FF8B94B93F |
| C:\Users\pc\workspace\fung\src-tauri\Cargo.toml | 54BEB684AA73B89F030B6627E9A98F9EC63A02EEC5CB8C1039394D70BE2B4AAD | 54BEB684AA73B89F030B6627E9A98F9EC63A02EEC5CB8C1039394D70BE2B4AAD |
| C:\Users\pc\workspace\fung\src-tauri\Cargo.lock | E756A52BB041C5F43D4674E46FD0781CBB66F2649B3CD0385C15F02FEE8B9D07 | E756A52BB041C5F43D4674E46FD0781CBB66F2649B3CD0385C15F02FEE8B9D07 |
| C:\Users\pc\workspace\fung\src\tauri.ts | 578FF537CC7F67432B93F8CB4F60475C1DA1B4B023059C51E0447578272C3782 | 578FF537CC7F67432B93F8CB4F60475C1DA1B4B023059C51E0447578272C3782 |

## Required diagnostic — run first

Process-only settings were:

CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0,
CARGO_INCREMENTAL=0, exact
CARGO_TARGET_DIR=C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target,
and TAURI_CONFIG={"bundle":{"resources":[]}}. Cargo used --offline --locked.

Exact command:

~~~text
cargo test --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\Cargo.toml --offline --locked --lib r2_registered_broker_account_switch_after_final_check_is_unclosed
~~~

Started 2026-09-21T19:03:21.2076148+07:00; finished
2026-09-21T19:03:37.1473329+07:00; exit 0; result 1 passed, 0
failed, 498 filtered. This is intentionally classified as one diagnostic
pass, not acceptance.

## Independent bounded command evidence

All Cargo commands used the process-only settings above, the frozen R2
manifest, the single FUNG-family target, --offline --locked, and were run
serially. No engine-family Cargo command was run.

| Started | Finished | Exact filter/command tail | Exit | Result |
|---|---|---|---:|---|
| 2026-09-21T19:08:21.5909043+07:00 | 2026-09-21T19:08:23.8329076+07:00 | cargo check --manifest-path ...\src-tauri\Cargo.toml --offline --locked | 0 | check passed |
| 2026-09-21T19:08:33.8054806+07:00 | 2026-09-21T19:08:34.3939851+07:00 | cargo test --manifest-path ...\src-tauri\Cargo.toml --offline --locked --lib --no-run | 0 | test binary built |
| 2026-09-21T19:08:44.4561410+07:00 | 2026-09-21T19:08:49.7300297+07:00 | cargo test ... --offline --locked --lib r2_ | 0 | 5 passed, 494 filtered; 4 ordinary controls plus the diagnostic |
| 2026-09-21T19:08:59.8625313+07:00 | 2026-09-21T19:09:11.0903740+07:00 | cargo test ... --offline --locked --lib r1_ | 0 | 3 passed, 496 filtered |
| 2026-09-21T19:09:22.4583942+07:00 | 2026-09-21T19:09:25.7308305+07:00 | cargo test ... --offline --locked --lib n3_ | 0 | 8 passed, 491 filtered |
| 2026-09-21T19:09:34.2082897+07:00 | 2026-09-21T19:09:34.8278907+07:00 | cargo test ... --offline --locked --lib oversized_identity_envelope_fails_closed | 0 | 1 passed, 498 filtered |
| 2026-09-21T19:09:43.2851416+07:00 | 2026-09-21T19:09:43.9184604+07:00 | cargo test ... --offline --locked --lib auth_session | 0 | 26 passed, 473 filtered |
| 2026-09-21T19:09:50.6359884+07:00 | 2026-09-21T19:09:50.7449185+07:00 | node --test tests/meetingIntelligenceContract.test.mjs | 0 | 3 passed, 0 failed |

The ordinary Rust result is 42 passed (4 + 3 + 8 + 1 + 26) plus
1 diagnostic. Non-fatal warnings included the known path canonicalization
warning and dead-code warnings for the N3 candidate API; those warnings are
not relabeled as security failures because N3 integration is deferred.

## Findings

### F001-R2 — HIGH — account-switch commit fence is unclosed

The failure is independently confirmed by
C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\genesis_adapter.rs:5064-5101.
The test uses the registered broker test harness, an injected lifecycle/key
backend, and an account-free local-owner vault. It:

1. captures account A's LifecycleWitness and takes a real short
   AccountOperationGuard;
2. reaches the commit path's final checks;
3. synchronously invokes post_fence_hook, which switches the broker to
   account B on the same thread;
4. commits successfully with unchanged generation/ticket; and
5. asserts A before, B after, equal account_generation, authenticated state,
   and a non-idempotent commit.

The source ordering explains the result. begin_login retains the existing
generation and does not quiesce admitted operations
(C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\auth_session.rs:303-326);
ordinary complete_login accepts the replacement without advancing that
generation (:427-460). The registered façade's with method locks only for
the individual callback (:673-695), and
check_account_operation/finish_account_operation reacquire that mutex
separately (:896-902). The account guard therefore protects logout/drain
admission but does not reserve the broker against an account replacement.

In the commit path, the VAULT read fence and lifecycle revalidation occur at
C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\genesis_adapter.rs:3497-3507;
the account ticket is checked afterward at :3509-3515; the hook then runs at
:3517-3518; and Genesis commits immediately afterward. There is no
existing-broker fence held across witness/ticket validation and commit. The
account A-to-B switch changes the LifecycleWitness tuple because native_user_id
changes. However, the previously issued ticket and generation remain valid
because ordinary account replacement does not advance generation, and no
atomic witness revalidation/hold spans the commit. This is a real HIGH
integrity and authorization boundary failure even though the fixture does not
prove bound-vault transfer or production exploitability.

The coherent tuple itself is present:
LifecycleWitness contains native_user_id, account_generation, and state
(auth_session.rs:70-75), and unlock/revalidation compares the full captured
tuple (genesis_adapter.rs:2651-2699,2760-2813). The missing property is
atomic validation under the acquired existing-broker lock.

### F001 closure conditions — proposal review only

The conditional RCA at
C:\Users\pc\workspace\fung\.brain\rca\2026-09-21-meeting-account-switch-commit-r2.md:110-143
is directionally correct but requires these explicit acceptance conditions.
No condition below authorizes implementation:

- Use an existing-broker AccountCommitFence or equivalent that atomically
  acquires the required admission/lock order, then validates the expected
  LifecycleWitness and LifecycleTicket under that acquired broker lock, and
  holds it through the Genesis commit.
- Do not execute a synchronous broker-switch/login callback, provider,
  keyring, UI, network, or other re-entry under the fence. The current
  post_fence_hook pattern is a sequencing witness only; it cannot be wrapped
  unchanged by a non-reentrant fence because it re-enters the same broker on
  the commit thread.
- Replace that callback in the future acceptance test with an independently
  contending login/account-switch thread and deterministic barriers. The
  switch must not complete while the broker fence is held. For the
  commit-fence-first schedule, the commit thread acquires the fence, the
  contender attempts the switch, witness and ticket are validated under the
  fence, the commit completes, and the broker fence is released before the
  AccountOperationGuard is dropped; the contender may complete immediately
  after fence release and need not wait for guard drop, because the current
  guard drains logout. For the contender-first schedule, the switch
  linearizes before fence acquisition and the commit must reject the stale
  witness. Assert commit-versus-switch linearization and the expected
  reject/commit result for both schedules without introducing a new
  login/drain policy.
- Keep VAULT/storage/broker lock order explicit; avoid re-entry and callbacks
  while the fence is held; release the broker fence immediately after commit
  and before the guard's drain-release/drop path. Do not replace the short
  per-operation guard with a long-lived OperationDrain.

Until those conditions are implemented and independently demonstrated through
the real registered entrypoint, F001 remains unresolved and N4 cannot be
accepted.

### F002 — bounded PASS: durable alternate-attempt replay

query_existing_event reads the stored transaction_id from
transcript_event_log (genesis_adapter.rs:2199-2216). Before any mutation,
the commit path returns that stored ID for the same payload and rejects a
changed payload (:3283-3304); the initial frontier is captured before
validation (:1939-1960), and the five relational writes remain inside one
Genesis transaction. Independent n3_ passed:

- n3_event_identity_replay_is_idempotent_and_changed_payload_conflicts
  (genesis_adapter.rs:6621-6674): same-attempt and alternate-attempt replay
  return the durable original transaction ID; changed payload conflicts; one
  revision remains.
- n3_reopens_and_replays_durable_transaction_identity
  (genesis_adapter.rs:7080-7119): the same behavior survives reopen.

This is bounded local evidence, not production or cross-process evidence.

### SI identity-custody invariants — bounded PASS, production evidence deferred

The frozen R2 path retains:

- XChaCha20Poly1305 with a random 24-byte nonce, authenticated serialized AAD,
  zeroizing key/plaintext handling, and bounded envelope validation
  (C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\meeting_intelligence_schema.rs:665-813);
- AAD scoped to account, meeting scope, vault, link entity, review revision,
  and the stored identity-link model provenance
  (C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r2-20260921\src-tauri\src\genesis_adapter.rs:3060-3110), not an unrelated ASR run;
- the people_metadata: key namespace and opaque durable attribution
  (meeting_intelligence_schema.rs:286-316, genesis_adapter.rs:2952-2983);
- active-vault, owner, current-profile, current-review, lock/revoke, and
  semantic relationship checks (genesis_adapter.rs:3007-3130);
- redacted Debug, zeroization, no caller-authorized identity context, and
  add-only v10-to-v11 migration coverage in the passing R1/N3 suites.

The explicit native unlock path uses the device-derived owner principal and
canonical data-root binding (genesis_adapter.rs:2625-2649,2702-2743),
separates VAULT authority/data-root/native-owner/process lifetime, allows the
account-free local-owner path only after explicit unlock, and has no
long-lived OperationDrain in NativeOwnerUnlockSession. The short account
guard remains in the capture/commit path. Restart/lock/revoke and concurrency
controls passed their bounded injected tests. Real OS keyring/device identity
is NOT_RUN; that is an evidence boundary, not a fabricated pass.

## Unrun and historical evidence boundaries

- Independent engine rerun: NOT_RUN by instruction. Schrodinger's historical
  17/17 is author evidence only.
- cargo fmt --check: historical author result exit 1 from broad pre-existing
  R1 drift; no rewrite was made and it is not relabeled as a product failure.
- Full FUNG suite: not rerun. Historical result remains FAILED exit 101,
  491 total: 484 pass, 6 fail, 1 ignored; six FUNGWIRE cases lacked
  .venv-whisper/Scripts/python.exe, so there is no valid baseline comparison.
- Native UI, real keyring, CI, provider/model, device, portable package, and
  release evidence: NOT_RUN.
- No old binary was substituted for the Cargo rerun. No model/runtime was
  installed and no network activation occurred.

## Artifact custody, leases, and rollback

The one shared target remained the FUNG build family:

C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target

The independently observed normal artifact is
C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target\debug\deps\libgenesis_block_native.rlib,
61,534,810 bytes,
SHA256 ACD1F097759485AA858F28CA898D36DF4DEB0C5D9ACB4306594DA7AEABE94636.
The recoverable quarantines were not moved, deleted, cleaned, or substituted:

- ...\libgenesis_block_native.rlib.quarantine-banach-r2-20260921-20260921-180040767.bak,
  61,533,404 bytes, SHA256
  87C1744DEA7259630BB0EAE64D3BD2259D4D03233E4CB34E95B28802907B0EAA;
- ...\libgenesis_block_native.rlib.quarantine-schrodinger-r2-20260921-172944.bak,
  61,534,810 bytes, SHA256
  ACD1F097759485AA858F28CA898D36DF4DEB0C5D9ACB4306594DA7AEABE94636.

The post-report zero-process check completed with
CARGO_RUSTC_PROCESS_COUNT=0. CARGO_SLOT_RELEASED. No commit, push, PR, merge,
deploy, or external state mutation was performed.

## Version diff and changelog

Version 0.1.1b is a clarification patch to the draft root-level independent
N4/G1 verification report: it corrects delegated identity, distinguishes the
changed LifecycleWitness from the still-valid ticket/generation, and tightens
the non-reentrant contention acceptance conditions. There is no source, test,
contract, dependency, workflow, manifest, lock, package, or
generated-artifact repair in this review.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | need review | Initial independent bounded N4/G1 verification draft; HIGH account-switch race remains BLOCKED/NOT_ACCEPTED | b336f33ec400a38f003a0665c121069a87a543ac | Harvey |
| 0.1.1b | 2026-09-21 | need review | Clarified delegated identity, changed witness versus valid ticket/generation, and non-reentrant contention conditions | b336f33ec400a38f003a0665c121069a87a543ac | Harvey |

**Final status: N4/G1 FAIL — BLOCKED / NOT_ACCEPTED.**
