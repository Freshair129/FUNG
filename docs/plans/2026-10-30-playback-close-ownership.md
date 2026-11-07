---
version: "0.1.0b"
created_at: "2026-10-30"
last_update: "2026-10-31"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-playback"
  doc_type: "plan"
  scope: "Retained playback cleanup ownership"
  language: "Thai-first"
---

# Playback close ownership implementation plan

> For agentic workers: use `superpowers:subagent-driven-development` or
> `superpowers:executing-plans`; follow TDD and independent review per master §9.

**Goal:** retain a bounded, retryable owner for a playback worker and admission
guard across close, failed scheduling, dropped futures, shutdown and panic.

**Architecture:** Atomically move the single active session to a manager-owned
closing slot before any handoff; the manager retains an Arc-backed session
capsule while eager spawn occurs outside locks. A started thread can claim the
capsule before spawn returns; if spawn fails no thread has run and the same
session remains recoverable. Cleanup signals cancellation before fencing output,
joins outside all manager locks and marks closed only after successful join.
Dropping a caller's future never drops the cleanup owner. One closing slot is a
bounded admission resource: opening another session is denied while it exists.

**Tech stack:** Rust 1.99.1, Tauri, CPAL; CPU-only tests and mocked workers.

**Spec:** [Native playback contract](../specs/2026-09-17-callmd-desktop-contracts.md)
and [routed quarantine candidate](../specs/2026-10-29-routed-playback-quarantine-contract.md).

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
surface-param routes ungated. Preserve local-main-window ownership and capture
exclusion; no PCM or paths through renderer. No audio hardware, GPU, default device,
routing or volume changes for these tests. No manufactured audio or relaxed gap
rules. Keep player to 20,000 descriptors/8 MiB metadata, two handles, 64 KiB reads,
1 MiB queue and 10 s open deadline. No merge/tag until both installed PID and
endpoint audio gates pass. Existing profile and unrelated edits remain untouched.

## Review focus

- Duplicate close while pending must report pending/error, never a false receipt.
- Spawn failure must retain the exact worker join handle and guard, with retry.
- Drop of an unpolled close future must not orphan legacy or routed worker.
- Panic, teardown or blocked I/O must not release the admission guard or claim closed.
- Quarantine and shutdown races must preserve lock order and a single cleanup owner.
- Poison, stale generations and different owners must fail closed without losing a worker.
- Late install/source-frontier failures and open-reaper spawn failures must retain
  detached worker ownership; do not overwrite an occupied closing slot.

## Task 1 — retain owner and recover spawn failures

**Files:** modify/test `src-tauri/src/desktop_playback.rs`; add a narrowly scoped
non-panicking guard-release API and poison fixture in
`src-tauri/src/recording_review.rs`, preserving existing callers.

- [ ] Write and run failing CPU tests for eager close followed by an unpolled future
  drop, injected spawn failure retaining the same session/JoinHandle, retry,
  duplicate close and concurrent new open. Cover legacy and routed workers.
- [ ] Move Active → Closing atomically; make the closing slot own a single bounded
  shared handoff capsule keyed by owner, handle and generation; represent pending
  spawn, running and failed explicitly. Never wait, spawn, quiesce or join while
  holding the manager mutex. A worker takes the capsule atomically and releases
  its mutex before quiesce or join. Spawn failure keeps the exact session for retry.
- [ ] Start cleanup eagerly before returning the close future; completion and
  idempotent receipt depend on joined worker, never on consumer of the future.
- [ ] Establish cancellation for legacy and routed callbacks before scheduling;
  actual in-flight callbacks are fenced/joined before acknowledging success.
- [ ] Run the focused regression suite. An executor/thread startup or hard-blocked
  I/O remains an error or pending owner; never report it as closed.
- [ ] Independent task review: race, ownership transfer and lock ordering.

## Task 2 — shutdown and terminal failures

- [ ] RED tests for shutdown during queued/active close, worker panic, capture
  admission and failed cleanup, plus retry without losing owner or guard. Test
  stale completion, owner mismatch, poison and shutdown at handoff barriers.
- [ ] RED tests for late open/source-frontier failure with shutdown and cleanup
  spawn failure, and opening-reaper spawn failure or worker/prepare panic. A
  cleanup path that holds the routed output gate must not quiesce synchronously.
- [ ] Keep the closing slot and guard fail-closed on unproven worker cleanup;
  explicitly handle late completion, stale handles and repeated close. A panic
  consumes the join handle and is terminal, not a retry of that same worker.
  Preserve ownership through shutdown or explicitly report the process-exit boundary.
- [ ] Run targeted legacy/routed suites and formatting; record actual results.
- [ ] Independent branch review. These tests do not constitute installed audio
  or hardware acceptance; source-aware public API and buffering remain separate.

## Candidate process-lifetime owner — design review required

The normal-close prototype fails a deterministic case where shutdown happens
while cleanup spawn is pending and that spawn then fails: once AppState and the
caller drop the manager, its retryable capsule and JoinHandle are lost. A local
self-cycle or guard flag is not an acceptable owner. The opening reaper has the
same ownership gap if its thread fails to start. The currently failing CPU test
`owned_close_shutdown_racing_failed_spawn_retains_manager_owner` records this gap.

Candidate: start a single process-lifetime supervisor before playback admission,
with a fixed maximum number of reserved owner slots and an explicit startup/
capacity error. Reserve an owner slot before admitting an opening operation;
the supervisor holds the manager plus the opening/closing ownership until a
verified completion. Handoff of a worker or preparation JoinHandle to the slot
must be atomic; failed delivery returns the exact ownership to a persistent slot
for bounded retry, never dropping it in a rejected closure. Closing and reaper
tasks are submitted eagerly independent of any consumer future. The queue is
bounded and nonblocking; saturation retains the pre-reserved owner and reports
pending/error. Join, cleanup, callback fences and I/O occur outside supervisor,
manager and SDK locks. A panic, poisoned state or hard-blocked I/O keeps that
slot and its capture admission guard fail-closed; no close acknowledgement is
issued. Supervisor failure stops new admission while existing owners remain
reachable; process exit is an explicit boundary, not a successful join.

Design and tests must additionally specify (a) startup and thread-spawn failure
before reservation, (b) wake/retry for a parked owner after transient saturation,
(c) single-worker hard-blocking I/O without starving unrelated owners, (d) owner
identity/generation and duplicate close, (e) AppState shutdown and dropped client
futures, (f) normal close, detached late-open and open-reaper paths, and (g)
bounded pool exhaustion with no session displaced or access widened.

Independent design review selects **one slot per process**: the current player
and capture admission already allow only one playback owner, so a stalled join
deliberately blocks new playback. Do not claim unrelated playback can proceed.
Start the supervisor successfully before admitting an open and reserve its sole
slot before starting preparation or a playback worker. The process-owned
supervisor has a strong manager/guard/owner reference only while a slot is
occupied; it is not held by that manager, and success clears the slot. Describe
process exit as an unverified boundary. Startup or capacity failure denies
admission before creating resources. A full/disconnected bounded wake channel
must leave the capsule in the process-owned slot; retry uses a bounded mechanism.
Unexpected supervisor exit denies admission and keeps outstanding slot/guard
reachable. Never claim recovery of a JoinHandle already consumed by a panic;
terminal uncertainty remains retained without acknowledgement. Tests exercise
isolated supervisors, injected startup/send failure, held prepare/reaper, delayed
worker, client and AppState drop, owner/epoch races, panic/poison, success and
slot release. Independent implementation review and exact-revision tests remain
open; this proposal is not evidence of acceptance.

### Candidate supervisor state machine — review before wiring

The one process slot is the authoritative owner for a single generation. Slot
state, payload and response-delivery state must be updated atomically under its
mutex; manager lock precedes the slot mutex when both are required. Neither
mutex is held across spawn, blocking wait, audio fences, database work or join.
Any operation that cannot preserve the stated owner must fail closed.

| State | Retained ownership | Allowed transition |
| --- | --- | --- |
| `Reserved` | manager, guard, generation; no worker has started | `Queued` after atomic task handoff; rollback only before handoff |
| `Queued` | exact pending open task and cancellation/delivery record | `Executing` on a supervised claim; spawn/queue failure retains task and schedules bounded retry; cancellation goes to `Reaping` |
| `Executing` | slot retains guard and all published prepare/worker handles; the running executor has a supervised claim | publish each newly spawned handle into the slot before waiting or returning; cancellation/timeout/failure moves to `Reaping`, never synchronous waiting on the supervisor's own queue |
| `InstalledAwaitingDelivery` | installed manager session, guard and response record | `Active` only when future atomically accepts delivery; drop/cancel or closed response triggers `Reaping` of that exact session |
| `Active` | installed manager session and guard | close/shutdown atomically transfers exact session to `Reaping` |
| `Reaping` | all outstanding prepare/worker joins and guard, including late installs | `Quiescent` only after all owned threads, streams and fences finish; spawn/queue failure keeps exact handles for retry |
| `Quiescent` | verified completion receipt | release guard and matching slot exactly once; then close receipt can be returned |
| `Terminal` | manager, guard and whatever owner payload remains | deny new playback and no close receipt; expose failure, never pretend a consumed/panicked join is retryable |

A response uses a shared `Pending` / `Delivered` / `Cancelled` state with one
linearization point. A future that is dropped before acceptance sets `Cancelled`;
the owner must inspect that state after installation and schedule cleanup. An
already delivered response is not revoked by a later future drop. Tests must
force cancellation before prepare, during ready wait, after install before
delivery and simultaneous delivery/drop. A successful send into an intermediate
channel is **not** proof the caller accepted the response.

Independent review clarification: `Delivered` means the native future returned
Ready, not that Tauri serialized the result or the renderer consumed it. Stronger
renderer adoption requires owner-bound acknowledgement/timeout or separately
verified window/selection cleanup; it is not implied by native delivery. A later
window shutdown still closes an already delivered active session.

The transition from a successful `spawn` to handle registration has a distinct
panic/poison boundary: reserve the handoff capsule first and transfer the exact
JoinHandle into it without blocking. On failed spawn the capsule retains the
original work; on uncertainty after a spawned handle is consumed or dropped,
mark terminal and retain the independent guard—never claim handle recovery.
For queued cancellation before any worker exists, rollback can reach Quiescent
only after checking generation and the absence of all owners. Prove no lost wake
on full/disconnected notification, and detect supervisor death before admitting
a new generation.

The supervisor starts before admitting resources and its one slot is reserved
before prepare/worker creation. An unavailable or full supervisor denies new
admission without displacing an existing owner. Its wake queue is bounded and
nonblocking: rejected notifications cannot own or drop jobs; work remains in
the slot. A worker blocked in OS I/O intentionally keeps the only slot busy.
Executor termination and poison are terminal fail-closed conditions; process
exit is not an acknowledgement. No unbounded retry or helper-thread creation.

## Current CPU checkpoint — 2026-10-31

The desktop open command now uses a process-rooted registry and bounded cleanup
supervisor in the working tree. The root spans admission, preparation, active
playback and cleanup. An unstarted scheduler envelope rolls back only after
checking the exact generation and capture token. Late sessions enter a reserved
fallback before the source-frontier check. These changes remain under independent
review and are not installed-package or audio acceptance evidence.

- `process-command-admission-tests.log`: 49 focused CPU tests passed at that
  checkpoint, including cross-manager exclusion and handoff shutdown/poison.
  `process-shutdown-delivery-tests.log` subsequently recorded 50 passed.
- `process-auto-retry-owner-regression.log`: 51 focused CPU tests passed before
  the latest lint-only edits. `process-queued-stale-retry-tests.log` recorded
  10 close tests passed, including shutdown-first, failure-first, queue-full,
  disconnected, and an old queued retry after a new admission. The worker's
  initial stale check is barrier-tested; a post-precheck race is supported by
  the atomic manager/registry reservation check and separate stale tests.
- A shutdown racing cleanup-thread spawn failure schedules one bounded retry.
  A full/disconnected queue retains the exact owner and guard as pending; it is
  **not** proof of eventual retry. A supervisor blocked in OS I/O may hold the
  only slot indefinitely. Retry-spawn failure remains fail-closed.
- `production-auto-retry-cargo-check.log` compiled non-test code with 23 warnings
  at that checkpoint. Strict clippy previously failed on unused private routed
  playback code and actionable lints. The CI clippy recheck found 17 dead-code
  errors for the still-private routed playback path, plus two test-build issues;
  those are not a passing CI result. Later lint and identity-check edits are not
  covered by the already-running Rust test binary and require fresh verification.
- Eager native-future drop is tested before and after install; a shutdown before
  native Ready is rejected. Tauri IPC delivery and renderer adoption are **not**
  proven by native Ready. Default ignored hardware tests, real installed PID
  and dedicated-endpoint capture acceptance, and release remain outstanding.

### Deadline cleanup follow-up — in progress

- Two RED tests have been added to `src-tauri/src/desktop_playback.rs` but are
  **unrun**: retain a same-generation close when a capacity-one queue has an
  Open claim parked in the overflow slot, and prove Drop returns while both the
  overflow-slot and registry locks are held.
- Focused Cargo session `47650` was interrupted with Ctrl-C for the BTS S9 GPU
  benchmark hold. It exited 1 with no test output; this is not a test result.
- The last Clippy run failed on two `too_many_arguments` lints in scheduler
  helpers and one redundant closure in `desktop_playback.rs`; none are fixed.
- After the hold ends (04:30 Bangkok or explicit release), run the RED tests,
  implement a root-retained close request with nonblocking Drop notification,
  address those Clippy findings, then rerun focused cleanup/source-open tests and
  Clippy on the exact final tree. No builds/tests, hardware or audio work during
  the hold.

## Queued executor timeout: open

The current `open_playback_owned` deadline starts after the blocking executor
begins. If that scheduler accepts but never starts the job, the caller can wait
past the CallMD timeout even though the process root retains ownership. A
candidate correction captures one absolute deadline before enqueue, bounds the
native response with a runtime timer, and cancels the exact generation when it
expires. The queued envelope, guard and process slot remain owned until the
scheduler either starts or discards the job; timeout itself is not a cleanup
acknowledgement. A delayed start must check the same deadline and cancellation
before taking the task or spawning preparation; when expired it retains its claim
until exact unstarted rollback completes. The same remaining budget must reach the
open executor rather than starting another deadline. The timer requires a
working, time-enabled runtime and an explicit timer dependency. Verify scheduler rejection, accepted-but-stalled,
start-versus-timeout, discard and poison races, as well as timer/runtime shutdown,
without inventing an early guard release. This is a plan, not yet implemented or
verified, and must not be claimed as the current behavior.

## Renderer adoption: initial proposal (superseded below)

A lost IPC response after native Ready can leave an installed paused session
whose handle the renderer never received. A possible follow-up is an explicit
owner-bound adoption acknowledgement for the exact handle and generation,
combined with a bounded lease and cleanup when adoption never arrives. A
stale or wrong-owner acknowledgement must not adopt another session; expiry,
acknowledgement and shutdown need one atomic ordering and output suppression
before cleanup. The frontend must complete this handshake before exposing the
handle as usable. The proposal was not the accepted implementation boundary
at the time it was written. The bounded playback-only contract recorded below
supersedes it; renderer acceptance must still not be inferred from Rust future
completion or native `Ready` alone.

### Verification update — 2026-10-31

The deadline and retained-close implementation has advanced since the earlier
checkpoint above. On the subsequent source snapshot the corrected
`process_owner_` suite passed 57 tests, the source-open factory passed 2 tests,
and the routed source catalog passed 5 tests. Their logs are under
`E:\FUNG-codex-test-target-20261031` as `process-owner-all-final.log`,
`source-open-final.log`, and `catalog-final.log`. Strict all-target Clippy
passed again after the final test assertion change; its log is
`clippy-all-targets-final.log` in the same directory. These tests do not establish IPC delivery,
renderer adoption, cross-process quarantine or installed audio acceptance.

Independent call-graph review identified a remaining reload/IPC-loss risk:
after native Ready, the renderer can lose its handle while the native session
still owns its guard. App exit cleanup does not prove cleanup on webview reload.
The adoption/reset lifecycle needs an owner- and generation-fenced contract,
including pending-open races and failed or blocked close, before it can be
claimed resolved. This observation has not yet been reproduced in an installed
app. PID and dedicated-endpoint installed acceptance remain release gates.

### Renderer-loss containment contract — in progress, bounded

2026-11-01 checkpoint: strict offline all-target Clippy passed after adding the
missing lease revocation helper, visibility transition fencing and correcting a
needless boolean expression. At that earlier checkpoint playback lease
regression tests were pending; Clippy does not establish runtime behavior or
installed-package acceptance.

Later the same day, after independent review and additional callback and
close-binding fixes, strict all-target Clippy passed on `desktop_playback.rs`
SHA-256 `DB83AB3FA7E238AA4E1A4C3029869470AC1E8772BD6949DADCBE100300CF6131`.
The process-owner suite passed 57/57, playback adoption/lease 21/21, and the
retryable/closed binding test 1/1. The frontend contract suites passed 28/28.
These are CPU/IPC-seam checks; installed playback, PID and endpoint gates remain
open. The earlier six process-owner failures and one pre-ACK failure were fixed
and rerun; they are not silently counted as passes.

Scope is playback-only. It does not change PID admission, native capture
ownership, release gates, recording custody, or the existing one-session
process owner.

- Every successful native open returns an adoption offer bound to the exact
  playback owner, process generation, opaque handle, and native-generated
  nonce. The session enters `AwaitingAdoption` before the response can be handed
  off. A 10-second monotonic acknowledgement deadline begins when the response
  is ready.
- The renderer validates the full open DTO, then acknowledges that exact
  offer before exposing the handle or sending play/control commands. Native
  ACK and timeout serialize in one atomic per-session lease word; stale or
  duplicate ACKs cannot adopt a newer generation.
- Output stays suppressed until ACK. ACK expiry, heartbeat expiry, and
  lock-free suppression/cancel latch and a coalesced process-root close
  request. The independent 10 ms lease watchdog submits close work outside the
  cleanup supervisor. The session slot and capture guard remain owned until
  the native worker is joined and exact guard release is proven. A blocked
  worker keeps admission fail-closed; expiry does not prove cleanup completed.
- Adopted handles renew a monotonic heartbeat lease every 2 seconds, with a
  30-second deadline. Hide-to-tray extends the live lease to a bounded
  5-minute grace; tray restore gives 30 seconds for heartbeat recovery. A
  visibility transition cannot revive a lease whose atomic deadline already
  expired. Host suspend is not detected separately; whether the platform
  monotonic clock advances through suspend remains unproven.
- Legacy and routed-source opens share the same adoption/ACK/heartbeat path.
  Control and status require the exact live owner generation and adoption
  token. Close requires that exact handle and lease; repeated close for the
  exact previously closed handle is idempotent.

Regression tests cover exact ACK/expiry deadline behavior, wrong/stale
handle-generation-token combinations, duplicate ACK, heartbeat renewal and
terminal expiry, hidden-to-tray and restore grace, and output suppression while
close work is parked on the manager lock. These are CPU state-machine tests;
full process-owner and installed reload/crash evidence is still pending.

Residual gate: Tauri's public `PageLoadPayload` does not expose a WebView2
`NavigationId`, and Tauri's termination hook is unsupported on Windows. This
change does not revoke on navigation start and does **not** bind the random
per-open token to one exact renderer document. A delayed old document that
still holds its open response can acknowledge and heartbeat that session until
the lease expires. The heartbeat bounds an orphan when that renderer stops
running, but does not establish exact crash detection or a maximum cleanup
completion time when workers block. Host suspend behavior is also unproven.
Installed reload/crash acceptance remains required. No document-bound,
crash-safe, or complete renderer-loss fix claim is permitted from CPU tests.
