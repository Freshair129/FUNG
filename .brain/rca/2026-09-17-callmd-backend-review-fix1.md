---
version: "0.1.0b"
created_at: "2026-09-17T05:32:22+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T05:32:22+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Bounded backend FIX1 within already approved P1-B safety and behavior"
  risk: "HIGH"
---

# Backend review FIX1

## Symptom and evidence

Frozen Turing candidate in output/callmd-worktrees/baseline-376ef30 failed
two independent Terra facets. Actual hashes/review actors are in the task DAG.
Recordings/Q&A reviewer Mencius01a0ac4c-e761-7713-a4de-a07b1c19a65e;
playback reviewer Laplace01a0ac4c-e81e-71b3-a462-76001b72b2cf.
Both requested gpt-5.6-terra/high, now closed. No native application was launched.

## Root causes / bounded corrections

1. recording_review.rs:533 adds every released/expired cursor to
   expired_cursors without a cap or expiry. Repeated limit1 pagination/release
   grows memory despite two active snapshots. Bound retention, preserve recent
   expiry/foreign-owner/replay behavior and explicit error states. Test repeated
   maximum pagination, release, expiry and retention eviction; no silent empty.
2. meeting_intel.rs:738 directly interpolates transcript text into instruction
   text. Citation membership alone cannot establish semantic grounding or prevent
   transcript instructions influencing an answer. Build an explicit untrusted-data
   boundary with safe structural serialization, keep source data as data, and add
   adversarial prompt-capture coverage on the actual production construction path.
   Preserve both-ID filtering, excluded graph/live tail, local provider selection,
   no-evidence/no-inference and existing source/prompt bounds. Never claim a prompt
   delimiter or valid citation guarantees truth or eliminates prompt injection.
   Do not remove transcript text through speculative keyword censorship or add
   another model/provider/dependency/semantic-grading system.
3. desktop_playback.rs:611,653 divides hound duration by channels a second time.
   Installed primary source hound3.5.1/src/read.rs:667-674 defines duration as
   num_samples/channels (per-channel frames). Stereo duration/timeline is wrong.
   Correct units and test mono/stereo, odd/even frame counts, rates8–96kHz,
   duration/seek/EOF, without relying on a physical device.
4. A source available during prepare can disappear before allowed reopen.
   fill_queue:1118 receives None and emits silence while metadata still claims
   available/no degradation. Return a truthful supported error or update missing
   range/degradation consistently; test delete-after-prepare deterministically.
5. expected_epoch validation:1425 and seek increment/send:1469 are not serialized.
   Concurrent same-epoch seeks can both pass and enqueue in reverse epoch order.
   Serialize validation, state transition and enqueue in the native owner/control
   path; cover concurrent seek/play/pause/close interleavings without sleeps-as-proof.
6. Preparation:1366 and worker readiness:1291 each wait a fresh10seconds;
   a call may wait almost20seconds. Timeout detaches owned work and releases
   admission while late work may still run. Use ONE deadline and an off-dispatch
   blocking-work boundary. Cancellation must prevent late activation/publication;
   keep explicit ownership of cleanup/join and retain admission until resources
   really quiesce. Do not solve a deadline by an unbounded UI/dispatch-thread join
   or by orphaning a thread and declaring it closed. Mock delayed stages to prove
   aggregate deadline, cancellation, late resource cleanup and no overlap.
7. Lifecycle warning needs bounded proof/correction: lib.rs:3555 handles only
   ExitRequested and releases capture admission without joining/signalling the
   actual live coordinator. Cover actual main-window destruction/player cleanup
   and true capture lifetime on process exit. Preserve existing window-close/
   native-capture policy; do not equate hiding a view with stopping capture.

## Withdrawn finding and uncertainty

The original claimed same-handle path-custody bypass was withdrawn after
controller evidence and reviewer reinspection. open_source calls
open_custodied_file, verifies metadata/final path on the actual File passed to
WavReader, and revalidates format/timeline; bounded reopen is allowed by spec.
Keep this invariant and add allowed replacement/reopen tests. Do not hold20k
handles or copy all audio to avoid a nonexistent bypass. Closed-handle tracking
is already capped16 and is not the cursor-memory finding.

The synchronous Tauri wrapper directly invokes the command handler; exact
Windows UI-thread affinity was not proven. Record dispatch blocking, not an
unsupported claim about which OS UI thread is blocked.

## Why author checks missed these cases

Focused suites passed4recording/7playback/15meeting-intel tests, but only two of
the15meeting-intel tests were newly added Q&A helpers. Coverage omitted adversarial
prompt capture, unbounded release cycles, stereo units, post-prepare deletion,
concurrent epoch order and deadline cancellation/lifetime. Build and counts alone
do not prove those invariants. Existing native behavioral22/22 also passed.

## Prevention / exact lease

Fresh Luna/max FIX1 may edit only the original five native paths and its original
backend report, with colocated behavioral tests. No renderer/shared command/DTO
change, Cargo/lock/schema/auth/CSP/config/Drive/other source changes. Read current
controller specs; preserve accepted test/baseline bytes. Use ABSOLUTE patch paths
inside baseline-376ef30, explicit command cwd, actual nonzero test counts.
Reproduce each issue, implement bounded corrections, rerun targeted/native tests,
check/fmt/clippy. Full453-suite result is historically446PASS/6FAIL/1ignored;
independent review reproduced all6 as missing isolated Whisper Python runtime.
Do not install or fake that runtime or relabel the full suite green.
Native device/provider/package/hosted-CI evidence remains NOT_RUN.
Release exact hashes for independent re-review. No code acceptance before that.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Evidence-backed bounded native correctness/safety FIX1; erroneous custody finding withdrawn | UNCOMMITTED;base376ef30 | Codex orchestrator |
