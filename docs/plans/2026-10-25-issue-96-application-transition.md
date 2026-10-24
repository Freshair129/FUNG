---
version: "0.1.1b"
created_at: "2026-10-25"
last_update: "2026-10-25"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "implementation-plan"
  scope: "Issue 96 selected-process renderer Stop/Resume"
---

# Issue 96 application transition repair

Goal: preserve selected-process audio across renderer Stop/Resume, then verify the
rebuilt installed package before authorizing release. This continues the approved
[meeting workflow](2026-10-22-desktop-meeting-workflow-mvp.md) and
[installed acceptance](../verification/implementation-reports/2026-10-23-installed-releasefix-uat.md).
Use systematic-debugging, TDD and independent review under master-plan §9.

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

## Investigation and implementation

- [x] Freeze merged baseline `090daac`; work in the existing linked worktree on
  `fix/issue-96-app-loopback-transition`. Prior source and installed evidence are
  compositional baseline evidence, not a fresh run of all tests.
- [x] Re-read native capture, renderer lifecycle and the reference comparator.
  MMCSS/shorter-drain hypotheses already failed; do not repeat them blindly.
- [x] Isolate the converter-quality hypothesis: wasapi 0.26 combines
  `AUTOCONVERTPCM` with `SRC_DEFAULT_QUALITY`; Microsoft's reference initialization
  omits the latter. Test a native-format/no-conversion control first. If unsupported,
  use identical initialization flags minus only `SRC_DEFAULT_QUALITY`. This is a
  hypothesis, not a documented Windows loss guarantee. No-conversion PCM16 and
  Float32 both failed; no converter change was adopted.
- [x] Preserve a failing native control and record each experimental binary/hash.
  Require simultaneous system positive control, nonperiodic marker correlation,
  multiple renderer transitions and selected-source isolation. Distinguish renderer
  destruction/restart from pause without weakening either tested claim.
- [ ] Once the cause is demonstrated, add a failing regression, apply the smallest
  production fix in `src-tauri/src/live_application_audio.rs`, remove diagnostic
  switches and rerun source tests, full library, Clippy and native custody checks.
- [ ] Independent source/acceptance review. Build and install an isolated candidate,
  exercise actual installed capture → Stop → transcript/output → export → reopen,
  including renderer transitions; compare persisted channels against controls.
- [ ] Update canonical progress/release evidence and changelog. Commit/push/open PR,
  merge after checks/review, then build the merged normal package and publish the
  versioned tag/release only when the installed acceptance gate passes.

## Review focus

- Converter flags and actual negotiated data representation must agree with decoding.
- Source isolation must exclude an unrelated simultaneous renderer.
- Idle periods and active media loss must remain distinguishable in durable evidence.
- Stop/error paths must retain every accepted packet and nonempty final tail.
- Test-only controls must not enter a shipped runtime configuration.

Primary references: [Microsoft loopback sample](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/ApplicationLoopback/cpp/LoopbackCapture.cpp),
[stream flags](https://learn.microsoft.com/en-us/windows/win32/coreaudio/audclnt-streamflags-xxx-constants).

## Current checkpoint

The [six-run lifecycle investigation](../verification/implementation-reports/2026-10-25-issue-96-render-lifecycle.md)
shows exact edges only when the owned target PID retains an independent silent
stream; included-child silence does not repair arbitrary-target capture. No
supported production fix has been established. Temporary diagnostic source is
removed, installed retest remains NOT_RUN, and issue #96/release acceptance stays
FAIL. The unchecked implementation/review/package/release tasks remain open.

## User-authorized alternative capture investigation — 2026-10-25

The user requested alternative capture approaches, then actual installed acceptance
before release. Compare timer-driven process loopback (without EVENTCALLBACK) with
the existing event-driven route, keeping identical PID isolation, PCM and ledger
criteria. This tests whether the virtual event-buffer lifecycle causes loss; it
does not assume polling has a larger native buffer. Temporary cfg(test) controls
must be removed if unsuccessful. Research a supported different backend in parallel.
No all-system fallback, target injection or driver installation is part of this scope.

- [x] Freeze and run timer-driven and MF callback native candidates with simultaneous controls.
- [x] Record actual buffer size, transition edges, faults and exact binary hashes.
- [ ] Adopt only a demonstrated repair; otherwise retain FAIL and investigate the next supported alternative.

The [alternative delivery report](../verification/implementation-reports/2026-10-25-issue-96-alternative-capture.md)
records v23–v28. Timer delivery and true MF Capture callbacks still fail; the
same-PID altered positive control passes. v25 has a failed system control and is
excluded from causal comparisons. Source is restored, installed retest NOT_RUN,
release FAIL. Dedicated-endpoint routing is a design candidate requiring explicit
source-mode/isolation requirements; it has not been installed as a fallback.
