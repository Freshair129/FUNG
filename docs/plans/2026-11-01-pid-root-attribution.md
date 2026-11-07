---
version: "0.1.0b"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Owned process-tree root attribution diagnostic for issue 96"
  language: "English"
---

# PID root attribution diagnostic

## Architecture checkpoint

Several buffer, reader, format, scheduling and stream-lifetime variants have not
repaired PID capture. Original stereo payload already lacks sampled marker regions
present in the endpoint reference. Writer reconciliation cannot recover samples
not returned by the capture boundary. No supported way to retain a foreign audio
client was established. These are investigation limits, not proof of an OS defect.

Before another product change, compare one untested attribution boundary on this
machine. Microsoft documents that
[include-tree mode](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ne-audioclientactivationparams-process_loopback_mode)
includes the selected process and its children. A non-rendering owned supervisor
P will have exactly one owned renderer child C. Readers/controllers remain outside
that tree. Compare otherwise matched capture targeting C versus P.

Hypothesis: if process-loopback history lifetime depends on the selected root's
own rendering session, the silent-root arm may differ at C's stream destruction
and recreation. Similar losses in both arms weaken that explanation. Neither
outcome identifies undocumented internals.

## Constraints and method

- Diagnostic only: never substitute a user's selected PID with its parent,
  capture excluded/all-process audio, or reroute a user application. Widening the
  selected tree could capture unintended siblings and is not an approved repair.
- Preserve matching math, ±48-frame bounds, failure status and leakage policy.
  No inserted audio, reconstructed samples or suppressed gaps.
- Fresh evidence/build roots and hash-named helpers; never overwrite old artifacts.
- Pin producer, reader, marker, analyzer and reference. Check process birth/parent
  identity and bound owned cleanup. No microphone recording.
- Require explicit Active, muted, nondefault HDMI with no foreign active renderer.
  Guards are sampled, not physical isolation guarantees.
- Sequential matched arms avoid concurrent-reader interference. A promising
  difference requires reversed-order repetition. Preserve every refused attempt.
- Missing hierarchy/guard/reference/alignment evidence means NOT_PROVEN.
  Diagnostic results do not qualify the installed package.

## Work

- [x] Inspect prior failures and product capture boundaries.
- [x] Independently review the hypothesis and prior non-equivalent tests.
- [x] Prepare isolated supervisor/controller and review/tests before native use.
- [x] Run direct-child and silent-parent arms; both fail unchanged PCM gates. See [actual observations](../verification/implementation-reports/2026-11-01-pid-root-attribution.md); broader independent audit limits are recorded there.
- [ ] If supported, write a failing regression and implement one justified fix.
- [ ] Verify source, build/install a candidate and pass both full installed modes.
- [ ] Only after both installed gates pass, review/merge/tag.

See [endpoint export/recovery and remaining PID evidence](../verification/implementation-reports/2026-11-01-endpoint-export-recovery.md).
