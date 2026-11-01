---
version: "0.1.3b"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Issue 96 upstream packet-loss localization"
  language: "English"
---

# PID upstream boundary investigation

Continuation of the [root-attribution investigation](../verification/implementation-reports/2026-11-01-pid-root-attribution.md).
Both root choices failed. Do not make another product change from the assumption
that a writer can recover audio which the capture interface did not return.

## Questions and experiments

1. Search original received stereo, independently of the writer and segment
   threshold, for the source windows used to infer missing edges. Separate
   epochs: the fixture restarts the marker at zero on recreation, so a match
   in a later epoch is not delivery of the missing earlier tail. Report bounded
   search and timing uncertainty; do not promote correlation into physical proof.
2. Read Windows Audio event schemas using TDH without enabling a provider. Test
   native trace-session permissions with a unique session and zero providers.
   Numeric queue metadata could add a boundary unavailable through GetBuffer;
   permissions, event identity/filtering and actual runtime semantics must be
   established before tracing. Do not elevate or change machine policy.
3. Instrument an isolated owned renderer at successful buffer submission and
   Stop/release boundaries. Hold the *stopped* client for predeclared 0/10/50 ms
   before destruction, with no more writes. Compare the same PID reader and
   endpoint reference. Prediction: a lifetime-sensitive delayed delivery path
   may recover more of the tail as the hold increases. A change does not identify
   a unique Windows internal component, and does not provide FUNG with ownership
   of a foreign application's audio client.

## Constraints

- Preserve the ±48-frame edge gate, isolation criteria and all failure receipts.
- No synthetic replacement PCM, suppressed gaps, parent-PID substitution or
  implicit endpoint fallback. Submitted buffers are not physical playback proof.
- Use fresh diagnostic source/output roots and immutable hash-named artifacts.
- Bounded in-memory instrumentation; an overflow invalidates the observation.
- Native work is sequential, on an explicit muted, nondefault, Active endpoint
  with sampled foreign-session guards. No microphone or user application capture.
- Preserve existing source, runtime, profiles, routing, volumes and artifacts.
- Any promising result requires independent review and repetition before a
  justified production fix; only installed PID **and** endpoint acceptance can
  authorize merge/tag. Historical endpoint component passes do not close PID.

## Execution

- [x] Inspect current source, previous paired receipts and independent findings.
- [x] Finish raw packet searches with epoch attribution and limitations.
- [x] Query TDH schemas; native no-provider StartTrace returns access denied (5).
- [x] Prepare/review/build bounded render-submission and destruction-delay fixture.
- [x] Run matched/reversed-order 0/10/50 ms comparisons: Stop tail improves at 50 ms, but all six whole runs fail; retain source/packet boundary limitations.
- [ ] Implement a regression-backed product repair if a supported cause is found.
- [ ] Verify source and full installed PID/endpoint package workflow.
- [ ] Independent review, merge and tag only after both modes pass.

See [actual evidence and remaining PID failures](../verification/implementation-reports/2026-11-01-pid-upstream-boundary.md).

## Resume and packet cadence continuation

The six retained recordings are now independently checked at the raw onset and
packet boundaries. Eleven credible restarts align PID onset to source frame 480
and endpoint onset to frame 39; the twelfth retains its NOT_PROVEN alignment.
All twelve raw first-active packets begin with 64 nearly-zero frames followed by
416 frames matching source 480–895. This is a pipeline observation, not proof
that the converter is responsible. Read-entry timing follows 24 of 25 QPC leaps;
neither a timestamp-only repair nor filling gaps as silence is justified.

- [x] Independently verify raw stereo/mono custody, onset position and QPC/read cadence.
- [x] Query self-PID allocation without starting capture: requests 0/1/5 seconds
  all allocate 480 frames in the tested event-driven Float32 configuration.
- [x] Reconcile prior polling and Float32 trials; do not repeat defeated polling
  as a new idea or treat the old Float32 whole-run failure as a packet-level oracle.
- [x] Prepare and independently review a bounded standalone receiver with raw
  format self-report and bytes before ReleaseBuffer.
- [x] If admitted, compare PCM16 plus AUTO/SRC conversion versus Float32 without
  AUTO on the same frozen owned renderer and endpoint control, initially A/B.
  If apparatus succeeds, repeat in reverse order B/A; retain every failure.
  First attempt `pcm16-a` was REFUSED before launching audio processes: both HDMI
  endpoints reported NotPresent. After user confirmation and Active readback,
  `pcm16-b`, `float32-a`, `float32-b`, `pcm16-c` complete in A/B/B/A order. All
  four remain FAIL: Resume +441 frames, four QPC gaps each, and Stop deficits.
- [x] Compare raw onsets, first-packet shape, Stop edges and native QPC gaps.
  Both sample representation and conversion flags change together: attribute any
  effect to that combined pipeline, not uniquely to format or a resampler.

New artifacts belong under `E:\verification\fung-pid96-resume-localization-20261101`.
The original six-arm artifacts remain immutable. The receiver is diagnostic,
not a product capture replacement. Its successful process exit means collection
completed, not that continuity passed. Product changes and installed acceptance
still require a demonstrated repair under the original criteria.
