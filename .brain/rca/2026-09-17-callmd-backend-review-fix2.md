---
version: "0.1.0b"
created_at: "2026-09-17T06:30:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:30:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved native dispatch and disposal corrections"
  risk: "HIGH"
---

# Native review FIX2

## Symptom and evidence

Independent static reviewer Terra Halley01a0ac89-7ddd-7a41-979a-7dee03a61761
returned FAIL against desktop_playback.rs76bcb044ec12d29176f5f8ef5d7932f45067ea8fd8337e944aea74c68c2e29f6
and lib.rs7f15d1eb8022702929badbd3e106b4f1e7e09ae02e0995c676ed650838718d2f.
The six-source/report frozen hashes matched. Reviewer did not execute Cargo.

1. desktop_playback_open:1538 remains synchronous; recv_until_deadline at1580,
   1640 blocks the Tauri command for up to10s. Helper-only channel timing tests
   do not verify actual production async dispatch. No Windows UI-affinity claim.
2. shutdown:299 clears opening but does not cancel/own the pending preparation.
   Destroyed -> lib:3574 shutdown_native_state can be followed by the still-open
   request installing an active session at1692-1706 after window destruction.
3. close:1751-1755 and shutdown:305-310 synchronously join a potentially blocked
   worker from command/window/exit dispatch. Timeout-open reaper tests do not
   cover active-player close/shutdown cleanup.

## Root cause

FacetA reviewer Terra Pasteur01a0ac89-7d38-7252-a22a-d68d36cc5ffa also returned
CHANGES_REQUESTED: meeting_intel.rs:780 test calls build_recording_prompt only
with a benign question. It does not capture the outbound body on the actual
meeting_ask_recording -> call_llm path or verify returned source isolation for
an adversarial question. The structured serialization fix itself is valid;
the claimed production-path regression evidence is incomplete.

FIX1 introduced one deadline and an open-timeout reaper, but left the command
entry synchronous and did not make the manager own cancellation/disposal state
for all in-progress opens. Successful and shutdown paths still depend on joins
on dispatch threads. Cleanup lifetime and admission are not one coherent owner.

## Why it escaped detection

The reported14playback tests cover helper channels, frames, queue epochs and
timeout reaping. They do not invoke the actual production open-dispatch/manager
destruction race or close/shutdown while an owned worker is delayed. Focused
tests/build do not prove responsive native dispatch. Review caught this before
App integration or any user-data/native-device run.

## Bounded FIX2 / prevention

After all read-only reviewers release the frozen tree, fresh Luna/max may edit
only the original five Rust paths/report. Keep command names/DTO/interface and
all approved custody/resource bounds. Use existing async runtime facilities,
no dependencies. Actual open and close work must not synchronously wait/join on
command dispatch; window/exit handlers must not perform unbounded worker joins.
The manager must own cancellation and a disposal/generation boundary, invalidate
all pending opens on shutdown, and check that same boundary atomically before
installation. Late readiness after disposal can never install/activate audio.

Cleanup/reaping must have explicit ownership and retain native admission until
preparation/stream workers actually quiesce. A timeout is not successful close;
never acknowledge closed or release admission merely because waiting stopped.
Do not pretend blocked OS IO can be killed. Preserve the existing close-request
policy and prevent detached late activation. Keep the single total10s open
deadline, stereo frame fix, disclosed late-source failures and serialized epochs.
Add deterministic production-dispatch/manager-path tests using delayed owned
workers for timeout, destroy-during-open, late readiness, close/shutdown and
admission-retained-until-quiescent; helpers alone are insufficient. No device or
provider actions. Rerun focused/full tests and distinguish untouched strictCI
auth/backup and absentWhisper failures. Report0.1.2b; independent review required.
No shared/UI/Cargo/lock/auth/backup/config/capability edits or new authority.

Add a test-only local provider capture seam on the actual production recording
Q&A path, not a test-only reimplementation or build-prompt-only call. Use temp
fixtures containing same-project/different-recording, foreign-project, graph
and live-tail bait plus an adversarial question/transcript. Capture the exact
outbound payload and validate result sources/citations; no provider/network call.
Preserve configured local-only routing and no-evidence/no-call behavior. Do not
claim citations or JSON serialization guarantee model truth/injection immunity.
FacetA independently reproduced6review/16intel/14playback/16live/22native PASS,
full457PASS/6missingWhisperFAIL/1ignored464 and the same untouched strictCI errors.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record production dispatch and disposal gaps remaining after FIX1 | UNCOMMITTED;base376ef30 | Codex orchestrator |
