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

### Offline GetBuffer-to-ReleaseBuffer timing audit — 2026-10-31

An offline audit paired each successful 480-frame GetBuffer with its immediately
following ReleaseBuffer in the retained `calltracebaseline03` and
`calltraceheld03` receipts. It measures from GetBuffer **entry** to
ReleaseBuffer **completion**: an upper bound on the interval for which the
buffer was held, including the calls, decode, instrumentation and any
descheduling. It is not a measurement of packet availability in Windows.

| Run | Paired packets | Maximum upper bound | p99 upper bound | Above 10 ms |
| --- | ---: | ---: | ---: | ---: |
| immediate destroy | 1,997 | 0.8266 ms | 0.0986 ms | 0 |
| hold stopped client | 1,997 | 1.2305 ms | 0.0816 ms | 0 |

The 10 ms comparison is the **earlier observed** HDMI engine period, not a new
hardware reading. The result weakens a long-held-buffer hypothesis for these
two runs only; it does not establish a root cause or prove that missing PCM was
silent. The unchanged audio verdict remains FAIL for both runs.

Reproducible offline source and receipts are in
`G:\AI-Studio\verification\fung-pid96-buffer-hold-audit-20261031`.
`audit.py` SHA-256 `46A1613B78008B656560B3F849260B7ECA09536D69D91C0C0416EE1E7D9E3A7B`;
`receipt-v2.json` and independent replay `receipt-replay.json` both SHA-256
`A5268DF314D269B564DA355318276E6AF9A7B6B47C9436E649BA0BDCA9AAE5C0`.
The receipt records both original trace hashes, the diagnostic and capture source
hashes from those traces, packet-level QPC jump observations and limitations.
For a fresh output, run `python audit.py <new-receipt-path>` from that directory;
the script refuses to overwrite existing output. No live audio or ETW was used.

### ETW metadata feasibility — 2026-10-31

Read-only local provider metadata for `Microsoft-Windows-Audio`
(`ae4bd3be-f36f-45b6-8d21-bdd6fb832853`) describes event 115, version 0,
task 134, level 5, keyword `0x0200000000000000` as a cross-process packet
addition; event 159, task 166, has the same level and keyword and names a history
packet task. Events 24 and 25 describe stream start and stop, with an App ID and
PID in the message. Glitch event 34 describes a cross-process output queue full
and packet drop. These are metadata names and message formats, not observations
of events firing in the tested process-loopback path. The inspected metadata
does not establish payload schema, association with the owned recording, or
whether any packet event distinguishes availability from missing audio.

Microsoft documents an [ETW PID scope filter](https://learn.microsoft.com/en-us/windows/desktop/api/evntprov/ns-evntprov-event_filter_descriptor)
for currently running processes, limited to eight PIDs; with this filter the
provider is enabled in those user-mode processes and not in a kernel-mode
registration. Filtering only the owned probe/player may therefore omit events
emitted by shared audio services. Adding a shared service PID may collect events
for other applications; downstream filtering is not process-private
collection. The built-in local `Audio.Light.File` profile is broader still: it
lists system context switches, file and disk I/O, process/thread events and
many providers. No trace was started, and permission or privacy of a future
trace is not presumed. A future experiment must establish provider scope,
payload and timestamp correspondence, bound collection and event loss, and
compare trace-on with trace-off under the existing guarded audio criteria.

The original L/R packets downmix exactly to retained mono, and every writer PCM
byte matches that mono in both runs. Independent offline review recomputes edge
arithmetic from the existing fitted segments (without refitting), and verifies
the producer hold/release sequence and guards. This points to a relevant
renderer-lifetime boundary in the owned fixture; it does not establish Windows
internals or a supported general arbitrary-PID repair.

## Reproduction and scope

### Packet-boundary follow-up and source regression — 2026-11-01

A later capability-only probe tried a separate approach: request
`IAudioClient2` on a process-loopback client for the probe's own PID, before
attempting `AUDCLNT_STREAMOPTIONS_RAW`. Activation succeeded, but QueryInterface
returned `0x80004002` (`E_NOINTERFACE`), so SetClientProperties was never called.
The probe did not Initialize/Start a stream or read audio, and exited 1 normally.
This rules out that interface path in this probe, not all targets or Windows
builds. Source, binary hash and HRESULT receipt are retained under
`E:\verification\fung-pid96-raw-capability-20261101`. No PID repair is established.

The bounded offline follow-up at
`E:\verification\fung-pid96-packet-edge-audit-20261101` inspects retained original
stereo packets, marker fits and reference windows from `calltracebaseline03` and
`calltraceheld03`. Its five unit/negative tests pass. Repeated analysis produces
the same receipt hash. In the immediate-destroy run, sampled marker windows that
match the endpoint reference do not match the nearby original PID payload at
cosine 0.95 within the searched ±4,800-frame window. This weakens a threshold-only
explanation for those samples; it does not prove every missing frame is absent,
identify an OS loss mechanism, or establish installed acceptance. Some held-run
edge correlations are below the credible threshold and must not establish exact
edge preservation. Reference WAV hashes are post-hoc, not pre-pinned provenance.
The original ±48-frame acceptance threshold is unchanged.

Strict offline Clippy on the unchanged source passed with Rust 1.99.1. A fresh
complete library run reported **1,084 passed, one failed, 15 ignored** in 253.05 s.
The failing test was
`desktop_playback::tests::one_open_deadline_expires_delayed_readiness_without_a_second_window`:
its preparation thread did not complete inside the test's 100 ms deadline. An
isolated rerun passed 1/1 in 0.12 s. This is an unresolved scheduling-sensitive
test failure, not a passing full-suite result. No production timeout was changed.
All 176 inputs in the existing installed candidate's `source-hashes.json` still
match the checkout; this does not imply that the manifest covers every possible
build input.

Source inspection also confirms a concrete endpoint workflow gap, beyond missing
UAT evidence: `audio_export` and legacy `recover_recording` explicitly call
`require_legacy_media_source`, which rejects routed endpoint recordings. These
consumers require source-aware implementation; the guard must not be removed to
make legacy filename-based processing accept routed media. The separate installed
source-playback command smoke does not implement export or recovery.

The installed candidate was then exercised through its own main WebView command
bridge, with an isolated-profile backup and sampled exact process/debugger
generation guards. `playback-uat-20261101` under the existing UAT evidence root
records a two-source catalog and successful A/B/A paused open → adopt → heartbeat
→ status → close cycles. The harness never issues play/seek, but opening a paused
source creates a real CPAL output stream; output silence was not independently
measured. This is **PASS_PAUSED_COMMANDS_ONLY**, not rendered-audio or full endpoint
acceptance. The command driver exited zero. `CloseMainWindow` did not terminate
the app within ten seconds, so the launcher terminated its exact retained process
and confirmed exit −1 (`forcedAfterSmoke: true`). This is not a clean-shutdown
pass. PID and full endpoint acceptance remain open, so merge/tag/release remain
blocked.

### Cold-first diagnostic — 2026-11-01

Installed UAT retry2 on 2026-11-01 used isolated app ID
`dev.fung.uat.issue9620261101` and installed executable SHA-256
`D828281CDFF447A0BA1D8690970D2D1020FBC7771694DE454C7AA1F20ED439C9`.
PID execution and owned cleanup completed, but independent PCM review found
Stop edges −1,984/−1,949 frames and Resume +441/+441, outside ±48: **FAIL**.
The endpoint run passed two-source PCM and leakage checks, but authoritative
ledger and source-aware UI/export/recovery remain unproven; it is not full
installed acceptance. Both controller guards are sampled, not continuous.
PID chunk filename derivation does not prove an authoritative ledger or selected
PID isolation. Evidence is under
`G:\AI-Studio\verification\fung-issue96-uat-20261101\audio-uat-retry2`.
The owned UAT app exited 0; close IPC response timed out after window exit, so
no close-response success is asserted. Neither release gate has passed.

On 2026-11-01 a read-only Genesis SDK query on a copied UAT database matched
selected SQLite projection rows (2 sources, 10 bindings, 10 audio chunks, one
checkpoint). Source and copy were checked before and after; this was not an
atomic snapshot against every possible writer. Independent review found that
the SDK query reads the same projection database, so this is interface parity,
not independent journal verification or installed playback/export/recovery
acceptance. Evidence is under
`G:\AI-Studio\verification\fung-issue96-uat-20261101\genesis-readonly-audit`.

An isolated owned-player experiment activated the PID probe before the player
created its first render stream. `cold-first-04` still failed: the subject
reported six timeline gaps and exited 101. An isolated adapter for the cold
event protocol measured −1,504 frames at Stop and +441 at Resume in both
cycles against the unchanged ±48-frame threshold. Reference and player exited
zero, and raw stereo, mono, and writer bytes reconciled. Evidence and adapter:
`G:\AI-Studio\verification\fung-pid96-cold-first-20261031`.

Independent read-only review confirmed 14 recorded input hashes (which do not
cover reference WAVs, reference report/stderr, or device guard receipts), three
credible marker epochs, and both transition windows, but reference per-packet
QPC continuity and physical tail were not established. The adapter did not
independently re-read the no-session inventory receipts or prove activation
chronology; device guards were sampled. This is bounded diagnostic failure,
not a cause or installed-package acceptance. The original analyzer protocol
failure and earlier regression failure are retained alongside the new analysis.

An independent offline audit of `dualbaseline01` found four overlapping QPC
gap intervals between two PID readers. Review recomputed the interval values
and verified 11 input hashes. Marker fits were not credible and per-gap
reference marker coverage was not established, so shared missing PCM and the
feasibility of filling one reader from the other are **NOT_PROVEN**. QPC
overlap alone does not identify a cause. Receipt and scripts are in
`G:\AI-Studio\verification\fung-pid96-dual-consumer-20261030`.

### Bounded offline marker spot-check — 2026-10-31

A later CPU-light audit checked retained stereo against the frozen marker at the
previously fitted sample positions. For each of the held and direct-GetBuffer
runs it checked nine 256-frame windows per channel; the lowest Pearson
correlation was 0.999999938 for held and 0.999999933 for direct. It verified the
original-stereo hash against its receipt and recorded hashes of the endpoint WAVs.
The audit receipt at
`G:\AI-Studio\verification\fung-pid96-offline-audit-20261031\audit.json`
contains the selected positions, hashes and inherited fault and edge reports.

This confirms only the sampled, retained marker windows. It does not independently
refit the edges or correlate the endpoint WAVs, determine the content of missing
packets, or prove gaps were silent. The previously fitted direct run remains
−1,504/+441 frames in both cycles with four gaps and exit101; the held run remains
approximately 0/0 edges with three gaps and exit101. The 48-frame edge threshold
and nonzero-exit/fault requirements are unchanged: both runs remain FAIL.

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
