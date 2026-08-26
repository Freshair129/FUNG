---
version: "1.0.0b"
created_at: "2026-08-24T19:30:33+07:00,Terra 5.6,36fa29412fc46a764e1bccae94e44bf0d4d7a6e5"
last_update: "2026-08-24T19:30:33+07:00,Terra 5.6"
status: "need review"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-code-review"
  scope: "Desktop/Tauri Native Session Broker fix3; browser unchanged; Mobile deferred; review only"
  risk: "HIGH"
  reviewed_initial_commit: "cd6ceef87d4b0754b17ab04b2c463a942ba978c5"
  reviewed_initial_fail_report: "7a51ee16be8419c0e757299370ada4a5688aafea"
  reviewed_fix2_commit: "abb7a329258e4a2b94c0bc4406e9a2904e7da591"
  reviewed_fix2_fail_report: "6b815433b67ab771ee1dbbe96a2a012b0a629ae9"
  reviewed_fix3_commit: "36fa29412fc46a764e1bccae94e44bf0d4d7a6e5"
  fix_cycle: "3 of 3 (maximum)"
  verdict: "FAIL"
  integration_recommendation: "BLOCK; escalation required"
---

# Native Session Broker Fix3 — Terra 5.6 Final Independent Re-review

## Verdict

**FAIL — BLOCK INTEGRATION. The third and maximum fix cycle is exhausted;
escalation is required.** The final fix preserves the executable PostgreSQL
50-client proof, typed Desktop IPC, and redacted shutdown error propagation.
It does not close the P0 production-lifecycle evidence gate: the fourteen
passing behavioral cases exercise a generic model whose lifecycle methods are
not called by the corresponding production Tauri paths. The production paths
also retain ordinary secret-bearing `String` custody through `url::Url`, PKCE
verifier creation, and the native restore input, and Drive deletion/readback
can treat an unreadable keyring as absent.

No code, tests, configuration, external service, or pre-existing dirty path
was changed by this review. The only intended write is this report.

## Provenance and immutable approval check

| Item | Independent evidence |
|---|---|
| Approved candidate | `docs/specs/2026-08-24-native-session-broker-amendment.md` v0.2.0b at `7d48aa01c243ce5f32af1005b95b71082c5a5984`. |
| Exact-hash approval | Approval record v0.1.0b binds D-GDA4-01..05 to `41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4`. |
| Hash recheck | `Get-FileHash -Algorithm SHA256` returned that exact 64-character value; the candidate blob matched `7d48aa01` with blob `f68bdbbb75e3d4917b3461d869dde9559d03c1f5`. |
| Initial implementation and FAIL | `cd6ceef87d4b0754b17ab04b2c463a942ba978c5`; Terra FAIL report `7a51ee16be8419c0e757299370ada4a5688aafea`. |
| Fix2 and FAIL | `abb7a329258e4a2b94c0bc4406e9a2904e7da591`; Terra FAIL report `6b815433b67ab771ee1dbbe96a2a012b0a629ae9`. |
| Final candidate under review | `36fa29412fc46a764e1bccae94e44bf0d4d7a6e5` — `fix(auth): wire native broker production seams`. |
| Luna input | `2026-08-24-native-session-broker-luna-implementation.md` v0.1.4b was read and independently checked, not inherited. |

The reviewed broker sources, consumers, and focused tests match fix3 `HEAD`.
Unrelated pre-existing modified/untracked files were present and left
unstaged; no assertion below treats them as part of the reviewed commit.

## Independent command evidence

| Command | Result | Independent result and limitation |
|---|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS | 8/8. The PostgreSQL 17 Docker test executed, not skipped. |
| `node --test tests/nativeSessionCustody.test.mjs` | PASS, insufficient | 9/9, including its Rust child process reporting 14/14; the selected Rust path is not the full production lifecycle path (P0-NSB-02). |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS | 8/8. |
| `node --test tests/googleDriveContract.test.mjs` | PASS | 5/5. |
| `npm run build` | PASS | TypeScript and Vite completed; 1,764 modules transformed. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS with WARN | Exit 0 and 28 warnings. The non-test build reports `LifecycleCore` lifecycle methods, seam methods, and core fields as dead code, corroborating the call-graph finding. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` | PASS, insufficient | 14/14 named tests pass, but 13 use `FakeKeyring`, `FakeClock`, `FakeListener`, `FakeRequestTarget`, and `FakeProvider` under `#[cfg(test)]`. |
| `cargo test --manifest-path src-tauri/Cargo.toml -j 1` | PASS with WARN | 395 library tests passed, 0 failed. It does not repair the hard-gate evidence gap. |
| `git diff --check 6b815433b67ab771ee1dbbe96a2a012b0a629ae9 36fa29412fc46a764e1bccae94e44bf0d4d7a6e5` | PASS | No whitespace errors in fix3. |

### P0-NSB-01 — 50-client replay proof

**PASS locally.** `tests/w1AuthoritySchema.test.mjs:470-493` launches 50
simultaneous PostgreSQL clients with one captured timestamp, requires exactly
one exit-0 winner and 49 `proof_replayed` outcomes, and rejects any other
result. Lines 495-511 require final mutation counts
`1|1|0|0|0|0|0` across enrollment request, proof reservation, device,
connection, grant, authorization reservation, and decision state. The
independent executable run passed all eight Node tests.

## Original finding dispositions

| Finding | Final disposition | Exact evidence |
|---|---|---|
| P0-NSB-01 — 50-client proof | **PASS locally** | Executed PostgreSQL proof above; source assertions at `tests/w1AuthoritySchema.test.mjs:470-511`. |
| P0-NSB-02 — production lifecycle evidence | **FAIL — P0 hard gate remains** | The fourteen cases target `LifecycleCore` plus test fakes under `auth_session.rs:620-707`; the actual Tauri login, refresh, cancel, timeout, logout, and generation paths bypass those methods. |
| P1-NSB-03 — end-to-end zeroizing custody | **FAIL — P1** | OAuth parsing/building retains ordinary `url::Url` serialization strings containing state/code, PKCE verifier creation begins as ordinary `String`, and native Drive restore retains its recovery phrase as `String` until late in the path. |
| P1-NSB-04 — generic IPC / Drive bypass | **PASS locally/static; P2 retained** | Dedicated per-operation broker functions are used. `GoogleDrivePanel` still supplies its old compatibility argument, but `googleDriveFlow.ts:28-62` ignores it and never forwards it to Tauri. |
| NSB-R2-01 — shutdown cleanup failure | **PASS locally/static** | Production shutdown preserves only `cleanup_failed` and the Tauri hook handles only its stable code; see the dedicated assessment below. |
| New: Drive staged/delete readback | **FAIL — P1** | `drive_oauth.rs:406-407` and `427-430` treat a failed readback as if absence had been verified, defeating the required fail-closed deletion/readback contract. |
| New: Luna v0.1.4b accuracy | **FAIL — provenance/report accuracy** | Its metadata calls fix3 the candidate and leaves candidate hash `pending commit`, contrary to the approved immutable candidate/hash; it also says `lib.rs` was unchanged although fix3 modified it. |

## P0-NSB-02 — generic core is not the production lifecycle path

The generic core is compiled outside `#[cfg(test)]`, but that fact alone does
not make its lifecycle behaviors production behaviors.

| Required production behavior | Actual production call | What the passing test invokes |
|---|---|---|
| Login begin/listener/deadline | `broker_session_login_begin` at `auth_session.rs:465-480` constructs `PendingLogin`, opens the real listener, and starts `finish_login`; it never calls `LifecycleCore::begin`. | `native_behavioral_success_*`, timeout, cancel, and malformed-callback tests call `LifecycleCore::begin/complete` with `FakeListener`, `FakeClock`, and fake callback data at lines 683-699. |
| Callback exchange and generation | `finish_login` at lines 454-461 calls `parse_callback` and the real `exchange_code`, then invokes only `LifecycleCore::accept_material` at line 457. | `LifecycleCore::complete` uses `FakeProvider::exchange`; `NativeProvider` itself merely returns `auth_exchange_failed` / `auth_refresh_unavailable` at lines 257-260. |
| Startup, rotation, and single flight | `ensure_access_token` at lines 434-448 owns a separate `SessionMemory.refresh_flight`, calls `refresh_from_keyring`, and publishes material directly. It never calls `LifecycleCore::startup` or `refresh_single_flight`. | `native_behavioral_startup_*`, rotation, and single-flight call those generic methods with `FakeKeyring` / `FakeProvider` at lines 685-689. |
| Logout | `broker_session_logout` at lines 497-503 directly mutates `SessionMemory` and calls `clear_refresh_with`; it never calls `LifecycleCore::logout`. | `native_behavioral_logout` calls only `LifecycleCore::logout` at line 701. |
| Shutdown | `shutdown` at lines 505-519 uses a new core only for `production_shutdown`; it does not make the prior login/refresh/lifecycle methods shared. | `native_behavioral_shutdown` and cleanup failure call the generic shutdown with `FakeKeyring` at lines 703-705. |

`LifecycleCore::new` appears in production only at
`auth_session.rs:457` and `514`; its test factory is at line 678. The
non-test `cargo check` additionally reports `begin`, `complete`, `startup`,
`rotate_refresh`, `refresh_single_flight`, `protected`, `logout`, the clock,
request-target, and provider fields/methods as dead code. This is direct
compiler evidence that the injected seams do not exercise the claimed
production lifecycle.

There is also an unproved and source-visible generation race. In
`refresh_from_keyring`, the current generation is checked at lines 416-417,
then the refresh key is committed at line 418; `publish_material` checks the
generation only afterward at lines 422-425. Logout or shutdown can invalidate
the generation and delete keyring entries in that interval, after which the
in-flight refresh can write a new credential before its stale result is
rejected. `finish_login` has the same shape at line 457: it awaits real
exchange and commits through `accept_material` before `publish_material`
performs its stale-generation check. The fake stale-generation test changes
the generic core generation before `complete`; it does not exercise either
production interleaving.

This fails candidate §2.2's linearization and stale-completion requirements,
AC-GDA4-03, and the mandatory AC-GDA4-08 production behavioral evidence.

## P1-NSB-03 — secret-custody trace

The final code improves several direct readbacks, but it does not satisfy the
required "immediately into zeroizing custody" boundary.

| Path | Independent trace | Result |
|---|---|---|
| Auth callback target | `auth_session.rs:329-340` creates a `Zeroizing<String>` callback target. `parse_callback` then calls `Url::parse(raw)` at line 344, where `raw` carries the callback query. | **FAIL.** `url` 2.5.8 owns `serialization: String` (`url` crate source `src/lib.rs:228,238`) and its parser allocates it from the input (`:306-314`), so code/state are copied into a non-zeroizing `String`. |
| Auth PKCE verifier/state URL | `callback_pair` creates ordinary `let verifier = ...` before wrapping it at `auth_session.rs:326`; login then appends state and challenge into mutable `url::Url` at lines 466-477. | **FAIL.** The verifier is not immediately moved into zeroizing custody, and the ordinary `Url` serialization carries OAuth state. |
| Drive authorization URL | `drive_oauth.rs:445-450` creates ordinary verifier text before wrapping it. `build_authorization_url` appends state/challenge to `Url` and then wraps a second `url.to_string()` result at lines 459-476. | **FAIL.** The first `Url` serialization remains an ordinary secret-bearing allocation; wrapping its later string copy does not zeroize it. |
| Drive callback target/query | `callback_from_request` joins the callback target into `Url` at lines 479-519, then immediately wraps query values at lines 509-510. | **FAIL.** Immediate wrapping of the extracted `Cow` values is good, but `base.join(target)` has already created an ordinary `Url` serialization containing state/code. |
| Auth/Drive provider JSON fields | `AuthTokenResponse` uses custom zeroizing deserializers at `auth_session.rs:307-321`; Drive `TokenResponse` does the same at `drive_oauth.rs:207-222`. | **PASS only for the final deserialized field allocations.** This does not cure the earlier URL/verifier copies or prove all provider-library buffers. |
| Drive refresh payload/readback | `save_refresh_token` wraps JSON payload, staged readback, and active readback immediately in `Zeroizing` at `drive_oauth.rs:393-405`; `load_refresh_token` wraps the payload at lines 411-420. | **PASS for those explicit String moves.** |
| Drive staged/delete readback | At lines 406-407, staged deletion errors are discarded and only a successful `get_password` is treated as failure. At lines 427-430, deletion similarly succeeds when `get_password` returns an error instead of proving absence. | **FAIL.** A keyring read error cannot prove delete/readback absence; this is not fail closed. |
| Drive restore phrase | `broker_drive_restore` accepts `recovery_phrase: String` at line 1325, inspects it through line 1342, performs authorization/intent/network work, and wraps it only at line 1407. | **FAIL.** This is a long-lived ordinary native secret string rather than immediate zeroizing custody. |

The above sources violate AC-GDA4-02. The delete/readback behavior and the
uncovered login/refresh race also prevent AC-GDA4-03 from passing.

## NSB-R2-01 — shutdown result propagation

**PASS locally/static.** The final source correctly improves the narrow
shutdown-error handling path:

- `clear_refresh_with` maps active/staged delete or absence-verification
  failures to the stable `cleanup_failed` code at
  `auth_session.rs:235-240`.
- `LifecycleCore::shutdown` stores `CleanupFailed` and returns that error at
  lines 151-159; `production_shutdown` returns it unchanged at lines 171-174.
- Production `auth_session::shutdown` preserves the `Result` and records
  `CleanupFailed` at lines 505-519.
- The Tauri exit hook handles only `error_code` through `eprintln!` at
  `lib.rs:3122-3125`; it does not discard the keyring cleanup error or surface
  provider/keyring detail.

This source-level PASS does not repair P0-NSB-02: the focused test injects the
generic keyring, not the top-level Tauri session state/race boundary.

## Typed IPC, Drive order, commands, and platform boundary

| Area | Result | Evidence |
|---|---|---|
| Typed IPC | PASS locally/static | `desktopSessionBroker.ts:31-84` exposes dedicated operation functions; no `Record<string, unknown>` forwarding or `InvokeFn` Drive bypass remains. |
| Retained compatibility argument | P2 WARN only | `GoogleDrivePanel.tsx:49,59,106,123,134,157,162,182` supplies `invoke`; `googleDriveFlow.ts:28-62` names it `_legacyPanelArgument` and never forwards it. The local restore picker remains a separately permitted local native command. |
| Drive deny-before-secret/provider order | PASS locally/static, external proof open | Connection begin authorizes before listener/provider work (`drive_oauth.rs:562-566`); complete authorizes before exchange (`:701-718`); status/disconnect authorize before keyring (`:822-843`); list/upload/restore authorize before keyring/provider/archive work (`:919-975`, `:1311-1350`). Separate `BackupRead`, `BackupWrite`, and `BackupRestore` operations are used. |
| Desktop command registration | PASS locally/static | Broker auth commands are registered at `lib.rs:3010-3022`, Drive commands at `:3101-3109`; old secret-bearing auth/Drive command names are absent from `generate_handler!`. |
| Legacy code signal | P2 WARN | Unregistered `paired_device_upsert/list/revoke` helpers remain at `lib.rs:539-551` and produce dead-code warnings; they are not registered aliases. `open_external_account_portal` and `account_portal_open` both remain registered at `:3008-3009`, but both are no-input trusted-URL operations. |
| Browser unchanged / Mobile deferred | PASS locally/static | The full candidate-to-fix3 diff contains no `src/web/*`, `src/mobile/*`, `src/main.tsx`, or `src/App.tsx` change. Browser and Mobile adapters remain in their separate graphs; no Mobile readiness claim is accepted. |

## Luna v0.1.4b report accuracy

The writer report cannot be accepted as accurate closure evidence:

- Lines 12-14 label fix3 as both `implementation_commit` and
  `candidate_commit` and set `candidate_sha256: "pending commit"`. The
  immutable approved candidate is instead `7d48aa01...` with the verified
  hash `41B91DCC...2AA3D4` above.
- Lines 21-26 claim the fourteen cases exercise the production lifecycle,
  contradicted by the production call graph and non-test compiler warnings.
- Lines 51 and 57-59 contradict each other: the report lists `lib.rs` as
  changed, then says it was inspected but unchanged; fix3's diff also shows
  it modified.
- Lines 54-55 list the implementation report twice.

These defects do not invalidate the already verified Boss approval record,
but they do prevent the Luna report from serving as accurate final-fix
provenance or closure evidence.

## AC-GDA4-01..09 final re-evaluation

| AC | Result | Independent assessment |
|---|---|---|
| AC-GDA4-01 | WARN | No direct Desktop token event/storage DTO was found and public outputs are redacted. The recovery phrase is allowed as write-only input, but its delayed native zeroizing prevents an unqualified custody pass. |
| AC-GDA4-02 | **FAIL** | Ordinary `url::Url` state/code copies, ordinary verifier creation, and delayed native recovery-phrase custody violate end-to-end zeroizing requirements. |
| AC-GDA4-03 | **FAIL** | Production startup/refresh/generation paths do not use the tested core; stale completion can commit after the pre-commit generation check. Drive delete/readback is not fail closed on read error. |
| AC-GDA4-04 | PASS locally/static | Closed typed Desktop adapter, no generic broker surface, and legacy secret-bearing registered aliases are absent; browser/Mobile remain separated. |
| AC-GDA4-05 | PASS locally/static; P2 WARN | Account, pairing, and Drive consumers route through typed functions; Drive source order is deny-before-keyring/provider. The ignored panel compatibility argument is not forwarded. |
| AC-GDA4-06 | PASS locally/static | Browser sources are unchanged and Mobile remains deferred in its existing adapter/import graph. |
| AC-GDA4-07 | **PASS locally** | Independently executed 50-client PostgreSQL 17 proof: 1 winner, 49 replays, zero loser mutation. |
| AC-GDA4-08 | **FAIL — hard gate** | Fourteen test names pass, but their lifecycle/clock/listener/provider/keyring scenarios are not the Tauri production paths or injected production seams. |
| AC-GDA4-09 | **FAIL** | AC-GDA4-02/-03/-08 fail, the independent review is FAIL, and the updated Luna provenance/closure report is inaccurate. Build and static checks cannot substitute for the hard gates. |

## Integration recommendation and external gates

**BLOCK. Do not integrate, merge, push, deploy, release, promote, or claim
completion.** Because this is the third and maximum authorized fix cycle,
escalation to the approval owner is required before any further implementation
or test change.

The following external gates remain open independently of the local FAIL:

1. Real Supabase/Edge/RLS authorization, durable reservation, grant/revocation,
   and denial-before-keyring/provider evidence in the target environment.
2. Real Google installed-app OAuth, refresh/revocation, appDataFolder upload,
   digest-bound restore, and cancellation evidence.
3. Clean Windows VM OS-keyring startup, rotation, logout, shutdown, and stale
   completion proof after a newly approved remediation plan.
4. Supported-device UAT, signing, release artifact/publication, merge,
   deployment, promotion, and explicit production go/no-go approval.

## Version Diff

- `new -> 1.0.0b`: final independent fix3 review. It records the live
  50-client PASS, the still-disconnected lifecycle seam evidence, ordinary
  secret-custody copies, Drive deletion/readback failure, shutdown PASS,
  typed IPC/platform results, Luna report accuracy issues, all AC dispositions,
  and the mandatory escalation after the three-cycle limit.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0b | 2026-08-24 | need review | Final fix3 independent re-review: FAIL/BLOCK. P0 production-lifecycle and P1 custody/readback gates remain; maximum fix cycle exhausted and escalation is required. | pending | Terra 5.6 |
