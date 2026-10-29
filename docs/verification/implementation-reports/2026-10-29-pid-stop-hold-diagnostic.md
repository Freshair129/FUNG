---
version: "0.1.1b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Owned stopped-renderer retention versus immediate destruction"
  language: "Thai-first"
---

# PID Stop versus destruction diagnostic

Both actual native experiments completed; **neither qualifies as a full PID
capture pass**. Holding the stopped owned renderer changed the measured marker
edges, but the PID subject still reported three timeline gaps and exited101.
The experiment manipulates an owned test player and does not give FUNG control
over a third-party application's AudioClient lifetime.

| Owned renderer behavior | Stop edge vs reference, cycle1 / cycle2 | Resume edge, cycle1 / cycle2 | PID process and overall |
| --- | --- | --- | --- |
| Stop and destroy immediately | −1,984 / −1,984 frames | +441 / +441 frames | exit101; FAIL |
| Stop and retain actual client during pause; release before recreate | 0 / 0 frames | approximately0 / 0 frames | exit101; FAIL (three reported gaps) |

All edge fits are credible under the existing 48-frame policy. These are matched
native component results with an independent endpoint reference, not installed
package acceptance. The retained-client case has `application_loopback_discontinuity`
reports at 3590–3599, 3609–3615 and 5875–5899 ms. Do not call the clean edge
measurements a whole-run PASS or assume every gap represents active missing PCM.
The immediately destroyed case reports four discontinuities. Both player and
reference exit0 and cleanup has no forced process termination.

An offline localization places the three held-case QPC jumps next to payloads
whose two-channel peak is at most1 LSB, outside fitted active marker segments,
around release and recreation rather than the earlier Stop. Packet flags and
device indices do not establish the missing intervals' contents. These observed
neighbors are not evidence that all missing PCM was silent, so the gaps and
overall FAIL remain. `stop-hold-fault-audit/receipt.json` SHA-256
`2044C037FB374E26B21D8543C57DEE0BE2E407A9D15720847A9790E1844D3A9A`.

### Further lifetime and timestamp investigation

At the three held-case anomalies, consecutive native QPC intervals are
19.9876/15.7956/33.3844 ms; the recorded read-entry elapsed intervals are
19.2156/16.0306/33.1742 ms. Longer QPC intervals coincide with longer observed
read-entry intervals in these packets. Read-entry is a scheduling observation,
not a measurement of when the native packet became available or the contents of
unobserved audio: this does not distinguish delayed retrieval, missing packets,
or timestamp behavior. No gap was suppressed or filled based on this comparison.

The inspected Microsoft [render-stream lifecycle documentation](https://learn.microsoft.com/en-us/windows/win32/coreaudio/rendering-a-stream)
describes lifetime through references to the stream's client and its services.
[IAudioSessionControl2](https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessioncontrol2)
provides session information but does not document retrieving another process's
existing IAudioClient from its PID. No supported general-purpose mechanism to
retain that foreign client was established in the interfaces inspected. This
is a bounded research result, not proof no such mechanism exists elsewhere.
The producer retaining its own client is cooperative and does not resolve
arbitrary-PID capture. No product repair or new installed test follows here.

The original L/R packets downmix exactly to retained mono, and every writer PCM
byte matches that mono in both runs. Independent offline review recomputes edge
arithmetic from the existing fitted segments (without refitting), and verifies
the producer hold/release sequence and guards. This points to a relevant
renderer-lifetime boundary in the owned fixture; it does not establish Windows
internals or a supported general arbitrary-PID repair.

## Reproduction and scope

Product baseline `8efe02f14fe911aa846773583be218c16abbfd86`; no production Rust
change in this investigation. [Plan](../../plans/2026-10-29-pid-stop-versus-destruction.md).
Evidence root `G:\AI-Studio\verification\fung-pid96-new-path-20261029`:
`stopbaseline`, `stopheld`, `stop-hold-fixture`, `stop-hold-analysis`, and
`stop-hold-native-review`. Each run's `analysis.json`, actual original/mono PCM,
writer chunks, event receipts, control PCM and guard snapshots are retained.

One frozen renderer executable (SHA-256
`246152F661FAC50863B4506CB9053557FA6B2B70ABDDE96240016CC6F0294B52`)
used an explicit boolean flag; only the owned renderer hold behavior changed.
No stream is fed or restarted while held; it is released before fresh renderer
creation. PID capture and independent endpoint control are unchanged, with the
same nonperiodic marker and two Stop/recreate cycles. Explicit LM22 is verified
active, nondefault, idle and muted before and during each run.

Before native execution, fixture tests pass5 and offline build, formatting and
strict Clippy pass. Analyzer tests pass18, including failures on malformed hold
protocol and a nonzero subject exit despite clean edges. An initial fixture Clippy
failure on copied capture-only code is retained; the external test helper uses a
scoped documented lint allowance. Source/apparatus review permits only the owned
native study. No new product installation or package qualification occurred.
Independent actual-result audit receipt `stop-hold-native-review/audit.json` has
SHA-256 `5A3CC290C04B4065A89EDD11F65F1BFB382E11F166D852D67421067CD8CD3FDD`.

Prior full installed PID failure and endpoint component-only evidence remain as
documented in the [installed retest](2026-10-28-installed-audio-retest.md).
Source-aware endpoint consumers and full PID/endpoint installed acceptance stay
open. No merge/tag/release is justified.
