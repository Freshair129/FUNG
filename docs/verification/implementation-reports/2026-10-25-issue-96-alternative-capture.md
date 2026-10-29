---
version: "0.1.0b"
created_at: "2026-10-25"
last_update: "2026-10-25"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Issue 96 timer and Media Foundation callback diagnostics"
  language: "Thai-first"
---

# Issue 96 — alternative capture delivery

## Disposition

**ยังแก้ #96 ไม่สำเร็จ; release gate remains FAIL.** The user authorized trying
alternative capture approaches and required a passing installed package before
tagging. This follow-up tests timer delivery and an actual Media Foundation
Capture-workqueue callback. Neither preserves all active transition samples.
No unsuccessful diagnostic is adopted into production. No new installer was
built or installed; rebuilt installed acceptance is **NOT_RUN**.

Baseline is `7acc3b4` on `fix/issue-96-app-loopback-transition`, with native
production source identical to merged `090daac`. This continues the
[lifecycle investigation](2026-10-25-issue-96-render-lifecycle.md) and
[repair plan](../../plans/2026-10-25-issue-96-application-transition.md).

## Environment and unchanged gate

Windows 11 Pro build 26300; positive control device is Speakers (Focusrite USB
Audio). Evidence root: `G:\AI-Studio\verification\fung-issue96-20261025`.
Only owned synthetic players were captured; normal FUNG data was untouched.
All four active speaker endpoints remained muted; microphone settings unchanged.

The selected SoundPlayer stops/destroys its renderer, then starts again from
frame zero. It is not a position-preserving pause. Compare persisted selected-PID
PCM against the simultaneous system control and the nonperiodic 48-kHz marker.
The existing 48-frame/1-ms ledger-rounding ceiling is unchanged. Final recording
tails differ because probes intentionally run 12/20 seconds; compare transition
edges, not those final tails.

## Results

| Run | Change | Missing active frames before Stop / after restart | Disposition |
| --- | --- | ---: | --- |
| v23-timer-driven | PollingShared, requested buffer 0, reader polls every 1 ms, no EVENTCALLBACK | 1,504 / 461 | FAIL; application exit 101, two gaps; system exit 0/no gaps |
| v24-timer-buffer | PollingShared requests 1-second buffer | 1,504 / 0 | FAIL; application exit 101, three gaps; system exit 0/no gaps |
| v25-timer-selfreport | Same requested 1-second mode, with explicit mode log | Not qualified | INVALID CONTROL: both routes have additional multi-second discontinuities; excluded from causal edge comparison |
| v26-mf-callback | Read/release packets inside actual MF Capture queue callback | 1,984 / 461 | FAIL; application exit 2, two gaps; system exit 0/no gaps |
| v27-mf-positive | Same MF binary, independent silent waveOut stream retained inside the target PID | 0 / 0 | Positive control PASS, both exits 0/no gaps; altered fixture is not arbitrary-app acceptance |
| v28-mf-repeat | Repeat original target Stop/restart with MF callback | 1,024 / 461 | FAIL; application exit 2, two gaps; system exit 0/no gaps |

All polling runs report an actual native buffer of **480 frames**, including the
one-second request. v25 explicitly prints `PollingShared { autoconvert: true,
buffer_duration_hns: 10000000 }`. The old buffer log literally says
`requested_hns=0`, so it must not be used to infer the actual requested duration.
v23/v24 inherit the explicit parent `FUNG_APPLICATION_PROBE_POLLING=1` setting;
`v23-v24-invocation-ledger.json` is a retrospective ledger from the tool calls,
not a child-process self-report. Preserve that provenance limitation.

Direct PCM checks find pre-Stop missing-band correlations above 0.99999999 to the
source marker in v23, v24, v26 and v28. The MF missing bands have RMS about
3,482–3,549 PCM16 units. Their 461-frame restart deficits contain 442 samples
above magnitude 20. These are active audio, not timestamp rounding or idle padding.

## What differs in the MF diagnostic

This is an externally built, owned Rust executable, not the previously blocked
Microsoft sample executable. `MFStartup` and `MFLockSharedWorkQueue("Capture",0)`
create the queue; `IMFAsyncCallback::GetParameters` selects it and
`MFPutWaitingWorkItem` waits on the native capture event. `Invoke` directly drains
GetNextPacketSize/GetBuffer/ReleaseBuffer, then rearms. It does not merely wake
the old Rust reader thread.

A mutex serializes callbacks and shutdown; stop closes rearming, cancels the
pending wait, synchronizes with draining, stops capture and drains the tail before
closing the bounded 256-packet writer. The safe decoder and QPC clock were copied
from the frozen native source. There is no source injection, new driver or broader
system capture. The standalone writer is diagnostic code, not the production
recording/export pipeline and not an installed acceptance substitute.

v26 records 1,196 callbacks/rearms on thread 13036, main thread 38828. v28 records
1,198 on thread 42484, main thread 44052. Both report no native errors or rejected
queue packets. The positive control also uses the callback route and preserves
both edges. This supports successful exercise of the alternative delivery path;
it does not identify the precise internal Windows drop point.

Official API references: [event work item](https://learn.microsoft.com/en-us/windows/win32/api/mfapi/nf-mfapi-mfputwaitingworkitem),
[shared Capture queue](https://learn.microsoft.com/en-us/windows/win32/api/mfapi/nf-mfapi-mflocksharedworkqueue).

## Reproduction and artifact custody

From the evidence root, run `run-controlled-probes.ps1 -Name <fresh-name>
-WavePath <absolute continuity-marker-30s.wav> -AppExecutable <frozen-exe>`.
Polling additionally sets `FUNG_APPLICATION_PROBE_POLLING=1` in the controller.
For the altered positive control only, add `-KeepAliveInProcess`.
The controller collects per-process exits; its own exit 0 does not mean capture passed.

Retain each run's identities, app/system reports, WAVs, ready/control files,
stdout/stderr, transition comparison and direct-edge JSON. v25 retains raw failure
receipts without a qualifying comparison. `compare-transition.py <name>` and
`direct-edge-check.py --base <root> --name <name> --reference <marker> --out <fresh-json>`
provide separate correlation/edge checks. Use the repository's staged CPU Python.

| Frozen artifact | SHA-256 |
| --- | --- |
| `polling-probe.exe` | `ED502B346E67EB3F8DEA858BC3D20BA6360EA66583CA5B92C66A9A9352B5B039` |
| `polling-buffer-probe.exe` | `B479AE44405AF52E82AFFECAA4D5E59A7E1DBD700641FEE4C654771190402E11` |
| `polling-selfreport-probe.exe` | `0A2BD97BBBC2855E116CF7C8AA368E4FFE0B02C0B64885547734728668442CD3` |
| `mf-callback-probe.exe` | `0E8645007AACB39ADF28E01A69BCCC17527525A6D2BDD5F266411017013F5A8A` |

Frozen polling sources/build logs accompany these executables. MF source,
Cargo.lock, source hashes, README and build identity are in `mf-callback-probe/`.
Production `src-tauri/src/live_application_audio.rs` was restored byte-for-byte:
SHA-256 `FFE188C5CE2A7BCCDDACADCD5D9A415D6065A1864FA2082806B18109264C79D6`.

After restoration, the focused capture regression command passed **15 tests,
zero failures, two ignored, 685 filtered**, tests 0.03 seconds. Receipt:
`alternative-restored-tests.log`. Ignored native probes are separate executions
above; a unit pass does not supersede their failures.

Independent review accepted the diagnostic report with no actionable findings;
fresh v26/v27/v28 PCM checks reproduce the table. External audit:
`alternative-delivery-independent-review.md`. The final local path check verifies
166 relative file/directory links across the five changed documents, none missing;
anchors and remote links are not covered. `git diff --check` passes.

## Architecture boundary and next gate

Both alternatives still use Windows virtual **process loopback** underneath.
Changing delivery and buffer request did not provide an independent PCM source.
Media Foundation's generic audio capture is also based on WASAPI devices;
[Microsoft device-capture documentation](https://learn.microsoft.com/en-us/windows/win32/medfound/audio-video-capture-in-media-foundation)
does not establish an independent arbitrary-PID tap.

A genuinely different candidate is explicit routing of a chosen application's
output to a dedicated physical/virtual playback endpoint, then endpoint loopback.
That is a proposed architecture, not an implemented fix: it needs device/routing
setup, exclusion of unrelated apps, device-change recovery and its own native and
installed acceptance. It must be visible as a different source mode; it cannot be
a silent fallback that captures the user's entire system. No virtual driver or
audio-routing change was installed in this investigation.

Do not continue blind scheduling variants or relabel same-PID keepalive as a
generic application solution. A supported correction of the existing PID route,
or an explicitly designed alternative backend with proven isolation and transition
preservation, remains required before production integration, package UAT and tag.
