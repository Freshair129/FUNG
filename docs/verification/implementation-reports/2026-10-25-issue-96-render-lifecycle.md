---
version: "0.1.0b"
created_at: "2026-10-25"
last_update: "2026-10-25"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Issue 96 renderer-lifecycle discrimination, native probes only"
  language: "Thai-first"
---

# Issue 96 — selected-process render lifecycle investigation

## Disposition

**Issue #96 remains FAIL; no production fix or release acceptance is claimed.**
PR #91 was merged as `090daac` on 2026-10-25 (Asia/Bangkok) following an explicit
user instruction after disclosure of this defect. The subsequent request is to
fix #96, verify an installed package, then release only if it passes.

Six additional controlled native runs narrow the failure to the target PID's
render-stream lifecycle on this machine. No supported non-invasive correction
has been demonstrated within the current process-loopback API. This is not proof
that every Windows build, renderer or device has the same behavior. The precise
internal Windows/WinMM drop point remains unobserved.

The native source is restored byte-for-byte to the merged baseline; the temporary
format switches are absent from the proposed source tree. There is no new
production candidate to install. Rebuilt installed-package Stop/Resume acceptance
is **NOT_RUN** for this investigation; the failing gate remains open.

## Scope and evidence

- Repository: `G:\AI-Studio\development\FUNG`, baseline `090daac`.
- Host: Windows 11 Pro `10.0.26300` (build 26300); native positive control reports
  `Speakers (Focusrite USB Audio)`. This is one OS/device combination.
- Existing linked worktree; branch `fix/issue-96-app-loopback-transition`.
- External evidence: `G:\AI-Studio\verification\fung-issue96-20261025`.
- The selected owned player uses SoundPlayer/WinMM Stop followed by a fresh
  PlayLooping from the beginning. This is renderer destruction/restart, not
  continuation from a paused frame position.
- A separate silent renderer keeps the output endpoint running. An independent
  full-system capture records the positive control while the selected-process
  probe records the target tree. Speakers stay muted; no human speech is used.
- Both sides are correlated against the same nonperiodic 48-kHz marker. The
  historical comparator's edge inference is independently checked directly
  against final/initial 480-frame windows and the entire system-only active band.
  Chunk length/ledger differences are bounded at the existing 48-frame (1-ms)
  rounding ceiling. That ceiling was not relaxed.

## Experiments

| Run | Isolating change | Missing active frames before Stop / after restart | Native result |
| --- | --- | ---: | --- |
| v17-native-format | PCM16 with auto conversion disabled; initialization succeeds | 1,984 / 0 | FAIL, two source gaps |
| v18-selected-graph-keepalive | Independent silent waveOut stream in the **same target PID**; original capture mode | 0 / 0 | PASS, no source gaps |
| v19-float-native | Float32 virtual capture with auto conversion disabled; same reader clock/queue | 1,504 / 0 | FAIL, two source gaps |
| v20-selected-tree-keepalive | Silent renderer in an included **child PID**, original target stream still closes | 1,504 / 461 | FAIL, two source gaps |
| v21-selected-graph-repeat | Repeat same-PID silent-stream control | 0 / 0 | PASS, no source gaps |
| v22-baseline-repeat | Original capture mode, no same-PID silent stream | 1,984 / 0 | FAIL, two source gaps |

Every corresponding full-system control completes without source faults. The
direct pre-Stop system-only bands correlate above **0.99999999** with the original
marker and have RMS approximately 3,443–3,481 PCM16 units. They contain active
audio. The 461-frame restart band in v20 contains 443 samples above magnitude 20;
it is not an idle interval. Same-PID controls have no such missing band.

The native test executable returns 101 for the four failing application runs
because its unchanged `faults.is_empty()` assertion fails. The controller script
itself exits after collecting receipts; that script exit is not a passing audio
result. The two same-PID controls return 0. Their altered fixture is diagnostic
evidence, not acceptance for capturing an arbitrary external application.

## Interpretation and limits

Disabling the converter and changing the virtual capture representation did not
repair the failure. Retaining a stream in the target process itself repairs the
controlled transition twice; retaining one in an included child does not. Together
with the previously rejected scheduling/drain experiments, this supports a
target-process render-lifecycle boundary rather than filesystem backpressure.

The public activation contract selects a target PID and include/exclude-tree
mode. It does not expose a target-process render-graph keepalive or recovery of
media that the capture API did not deliver. Resetting our capture client flushes
pending data and is not a demonstrated recovery path. No unproven workaround is
installed: no target-process code injection, virtual driver, broader system
capture, manufactured samples or dropped acceptance checks.

An independent reviewer verified historical active edges and reviewed the new
causal controls. They found no defensible supported production repair within
the stated boundary. Further work needs either a demonstrated correction at
the native API handoff or an explicitly designed alternative capture backend.
The original arbitrary-renderer Stop/Resume gate remains unchanged.

## Reproducibility and retained receipts

The evidence root contains `run-controlled-probes.ps1`,
`play-controlled-fixture.ps1`, `OwnedSilentRenderer.cs`, the copied reference WAV,
the frozen native probe executables and their identity JSONs. Same-PID silence
uses a separate waveOut handle in the owned fixture; it never changes another
application. Included-child silence uses a normal child player. Owned helper
processes are stopped after each run.

Frozen native-format executable SHA-256:
`42462F45590E067B9DF453B4253C4DA177A27A7CBC71F8846D7A75D375B106B8`.
Frozen Float32 diagnostic executable SHA-256:
`0849AE2F844DACAA4EFF95AC2CF9F81C69D9AE4ED2509289BAA889D999CBE175`.
Restored production `live_application_audio.rs` SHA-256:
`FFE188C5CE2A7BCCDDACADCD5D9A415D6065A1864FA2082806B18109264C79D6`.

For each named run retain `*-identities.json`, `*-app/probe-report.json`,
`*-system/probe-report.json`, stdout/stderr, WAV chunks and
`*-transition-comparison.json`. The independent `direct-edge-check.py` produces
`v17-direct-edge.json` through `v22-direct-edge.json`. Historical v13/v15/v16
direct checks and `comparator-audit.md` are also retained.

The first experimental build compiled the test-only native-format switch in
1m 57s; the Float32 diagnostic build took 34.91s. These are build/probe facts,
not full-suite, installed-app, transcription or performance acceptance.

After restoring source, the focused native capture regression group rebuilt in
57.87s and passed **15 tests, zero failures, two ignored**; 685 unrelated tests
were filtered out. The ignored device probes were exercised separately above,
with their failures retained. This unit pass does not override those failures.
`restored-native-unit-tests.log` records the run. An independent document/evidence
review accepted the bounded claims, and `final-investigation-verification.json`
checks all six reported outcomes plus 258 relative file/directory links across
the ten changed documents (URL-decoded; anchors/network not checked).

## Related authorities

- [Repair plan](../../plans/2026-10-25-issue-96-application-transition.md)
- [Previous installed-package acceptance](2026-10-23-installed-releasefix-uat.md)
- [Issue #96](https://github.com/Freshair129/FUNG/issues/96)
- [Microsoft process-loopback parameters](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ns-audioclientactivationparams-audioclient_process_loopback_params)
- [Microsoft loopback sample](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/ApplicationLoopback/cpp/LoopbackCapture.cpp)
- [OBS native capture implementation](https://github.com/obsproject/obs-studio/blob/master/plugins/win-wasapi/win-wasapi.cpp)

## Changelog

| Version | Date | Change |
| --- | --- | --- |
| 0.1.0b | 2026-10-25 | Recorded converter/format failures, repeated same-PID positive controls, included-child failure and unchanged release gate. |
