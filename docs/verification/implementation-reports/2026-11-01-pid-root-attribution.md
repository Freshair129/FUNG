---
version: "0.1.0b"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Owned parent versus direct-child process-loopback diagnostic"
  language: "English"
---

# PID root attribution investigation

This follows the [architecture checkpoint and experiment plan](../../plans/2026-11-01-pid-root-attribution.md).
No product fix or new installed acceptance is established. The checkout remains
on `fix/issue-96-app-loopback-transition`, HEAD
`4950696c607c5b77eaba3840b2413ab4702beee2`, with prior uncommitted work preserved.
All 258 post-format candidate inputs still matched before this investigation.
The host reports Windows 11 Pro, build 26300. The evidence root is
`E:\verification\fung-pid96-root-attribution-20261101`.

## Hypothesis and apparatus

Compare capture rooted at an owned renderer C against capture rooted at its
non-rendering supervisor P. Same renderer/marker/reference/reader and unchanged
±48-frame transition bound. No user process, microphone, global/exclusion capture,
default endpoint change, or fabricated PCM. Actual process birth/image/tree,
explicit muted nondefault HDMI identity and active-session guards are sampled.

Installed and normal native-test entry points both reject a silent PID through
active-session admission. Those guards were not changed. The separate frozen
`cold-first-probe.exe` (SHA-256 `2135F6DC3B148B7084815530FB95F4A3A42448B97BB57C6C958372956E49DF78`)
already has a test-only owned-PID admission seam in an isolated historical clone.
Both comparison arms use that 20-second reader; it is not the latest installed
application and cannot qualify it. Capture decoding and threshold math are not
changed. A 45-second variant is avoided because the 30-second marker would repeat.

The new supervisor creates one suspended renderer in a kill-on-close Job Object,
publishes its child identity, and waits for a controller GO marker. Independent
reviews caught receipt-read races, incomplete pre-GO cleanup, capture-stop ordering
and malformed generated paths before native capture. Failure cleanup signals both
capture handles before bounded waits; it never waits for the renderer first.
Earlier generated drafts are retained and are not approved run artifacts.

## Preserved refused attempts

- `direct-child01` (controller v7) refused before GO/capture because the supervisor
  had an additional child. Cleanup exceeded its bound and forced the owned
  supervisor. This is a failed apparatus run, not a PID audio result.
- `prego-diagnostic.json` identifies the extra child as `conhost.exe` while the
  renderer remains suspended. No GO marker, capture or audio was started.
- A separately staged GUI-subsystem supervisor removes that console child in the
  next pre-GO observation (`prego-gui-diagnostic.json`); the guard is unchanged.
- `direct-child02` (v9) admits the supervisor but later refuses a grandchild after
  renderer startup, before either capture starts. Supervisor exits 0 and cleanup
  reports no failure. This is also not a PID audio verdict.
- `renderer-console-preguard.json` subsequently identifies the renderer's own
  console host. The separate GUI renderer observation has no such grandchild.
  These bounded observations use an oversized WAV that the source rejects before
  opening an endpoint; exit 1 alone does not prove the rejection reason.

The original renderer remains unchanged. A new diagnostic copy changes only its
PE subsystem byte (console 3 to GUI 2), leaving its entry point unchanged; byte
comparison is in `renderer-gui-patch-receipt.json`. This removes an apparatus
console dependency without changing renderer code, but byte comparison alone
does not establish runtime audio parity or a valid signature. Each actual run
must still satisfy identity, protocol, reference and PCM checks.

## Actual paired native observations

Controller v10 SHA-256
`C2CA6653CDB038700EF168D7C6D30CCCA95B8D2D8C26EA3756CF6A23072F3F85`
uses the same GUI renderer (`156CEB4F...`) and GUI supervisor (`73C8B3AE...`)
in both arms. The renderer code is unchanged; this does not qualify that fixture's
signature or full equivalence to the earlier console build. Both arms complete
two Stop/destroy/recreate cycles within the readers' recorded coverage.

| Run / target | Stop difference, cycles 1 / 2 | Resume difference, cycles 1 / 2 | Result |
| --- | --- | --- | --- |
| `direct-child03` / rendering child | −1,984 / −1,984 frames | +441 / +441 frames | FAIL |
| `parent-tree01` / silent supervisor | −1,984 / −1,504 frames | +441 / +441 frames | FAIL |

The ±48-frame gate is unchanged. Both PID readers report four discontinuities
and exit 101. Both independent endpoint references report no faults and exit 0;
both supervisors propagate child exit 0, with no forced termination or cleanup
failure. The original nonzero statuses and raw receipts are retained. Do not
confuse successful orchestration/analysis execution with successful audio capture.

Independent no-FFT review (`root-attribution-custody-audit.json`) verifies 1,996
and 1,998 received stereo packets respectively. Their downmix equals recorded
mono byte-for-byte, and all 12/13 writer WAV payloads concatenate to those mono
bytes (958,080 / 959,040 frames). This excludes a discrepancy after the received
packet tap in these runs; it cannot establish completeness of upstream delivery.
Each arm has 33 passing volume receipts and 32 matching inventory snapshots.

The frozen comparator emits a misleading `controller.failure: null` summary by
reading a different field from the actual controller's `firstFailure`. Both raw
controllers explicitly report `subject returned nonzero 101`, and the analyses
retain FAIL and four subject faults. The independent audit records the actual
failure; original analysis files are not rewritten or treated as clean runs.

The offline adapter pins the original comparator, marker and policy. It adapts
only expected root/renderer identity and an explicitly derived in-memory protocol
argument view; raw receipts are unchanged. It rejects cleanup/identity/protocol
failures and preserves nonzero-subject FAIL. Parent birth FILETIME is not persisted
in the controller receipt; ownership during execution uses its retained process
handle. Child exit is derived from the supervisor's propagated exit. Process and
endpoint guards are sampled, not continuous isolation proof.

This pair weakens the hypothesis that selecting a non-rendering parent avoids
the loss. It does not establish a Windows internal cause or justify substituting
a parent PID in the product. There is no promising improvement to promote or
claim as a repair. A reversed-order replication has not been run. The original
console build's signature/waveform equivalence and arbitrary application behavior
are not inferred from these two controlled runs.

## Remaining gates

No production capture changes or new package build/install were made in this
investigation: there is no validated fix to package. The previous installed
PID failure and bounded endpoint/export/recovery observations remain in the
[candidate report](2026-11-01-endpoint-export-recovery.md). No merge or tag until
both modes pass the required installed-package acceptance.
