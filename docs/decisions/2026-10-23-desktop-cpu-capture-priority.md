---
version: "0.1.0b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio-capture"
  doc_type: "decision"
  scope: "Windows CPU recording and local transcription timing"
  language: "Thai-first"
---

# CPU capture takes priority over decoding

The user approved testing and releasing the capture remediation described in
[the evidence report](../verification/implementation-reports/2026-10-23-capture-backpressure-remediation.md).
Synchronous CPU decoding blocked bounded capture queues and caused source gaps
in the installed candidate. Losing source audio prevents reliable later recovery.

For the CPU Desktop release, capture writes durable audio first and decoding
runs after Stop. This amends DLR-02 and the real-meeting gate in the
[initial public-release specification](../specs/2026-08-14-desktop-live-meeting-public-release.md):
the required populated transcript may be produced after Stop. A transcript event
while recording is not a CPU release requirement. Saved audio, transcript,
normal close/reopen, and installed runtime independence remain required.

The UI reports the deferred operation. GPU live decoding keeps its existing
behavior; GPU runtime redistribution and arbitrary slow-GPU backpressure safety
are not accepted by this decision. An asynchronous CPU decoder may be designed
later without changing the requirement to preserve captured source audio.
