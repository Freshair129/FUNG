---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Actual read-only engine-period queries for two HDMI endpoints; no capture repair"
  language: "Thai-first"
---

# HDMI engine-period admission — 2026-10-29

## Result

**ยังไม่มี period ที่สั้นลงให้ทดลองบน HDMI ทั้งสองตัวในฟอร์แมตที่ตรวจ**
Device queries succeed; this is not a capture PASS. #96 remains FAIL and both
full actual installed audio gates remain open. No new installer, merge, tag or
release is produced by this admission.

| Explicit endpoint | Format | Default | Fundamental | Minimum | Maximum | Current |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| LM22-B200S, ID suffix `c7033d7f-b081-40c6-a5ff-1df83a87d352` | 48kHz stereo Float32 | 480 | 480 | 480 | 480 | 480 |
| Mi Monitor, ID suffix `febdb10f-94d3-4af8-af69-b3ac1e887122` | 48kHz stereo Float32 | 480 | 480 | 480 | 480 | 480 |

Units are audio frames:480frames/48000Hz =10ms. All shared-period API outcomes
succeed; no API error is recorded in either query. The tool normalizes successful
Result-returning calls to documented S_OK and retains actual failure HRESULTs;
it is not an exact raw successful-HRESULT trace. COM initialization HRESULT is
recorded directly. Current engine period is an instantaneous observation.

`GetDevicePeriod` separately reports default100000hns and minimum30000hns.
Its minimum is the [exclusive-mode scheduling period](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-getdeviceperiod),
not a permitted144frame shared period. Do not substitute that3ms figure for the
explicit IAudioClient3 shared-mode minimum of480frames.

This conclusion covers these exact endpoints, their current mix and API-default
client properties only. No RAW/alternate processing mode, sample-rate change,
other driver/device, OS-wide guarantee or host root cause is inferred. A smaller
period is not claimed by trying undocumented or out-of-range values.

## Hypothesis and authority

Baseline `fff68af01f426ed9c4f93c66190bad7afe6c0653`, existing isolated
`fix/issue-96-app-loopback-transition` checkout. The user explicitly authorizes
the two queries and a real paired audio study only if smaller periods are
supported, while retaining both installed gates before merge/tag.

The [bounded plan](../../plans/2026-10-29-pid-engine-period-admission.md) first
queries a **normal physical endpoint** IAudioClient3. A separate low-period
endpoint renderer would negotiate upstream engine scheduling; it is different
from the defeated process-loopback polling/MF/format variants. It is still only
a hypothesis, not an independently different process-capture backend.
[Microsoft Low Latency Audio](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/low-latency-audio)
describes driver-dependent small-buffer support. The documented
[GetSharedModeEnginePeriod](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient3-getsharedmodeengineperiod)
returns the permitted range; an initialization period must satisfy that range
and its fundamental multiple. Neither document guarantees lossless process
capture across render-client destruction.

The read-only range has minimum=default=maximum, so the smaller-period paired
stream experiment is **NOT_RUN: capability condition unmet**. This is not a failed
audio experiment or an installed test result. No alternate format or host setting
is changed to force admission.

## Frozen tool and exact execution

External evidence root:
`G:\AI-Studio\verification\fung-pid96-new-path-20261029`.

External helper `engine-period-query` uses Rust1.99.1, windows0.62.2 and the
retained Cargo lock. It allows only these two IDs and a fresh absolute JSON
output. COM activation and Get calls only; no Initialize, InitializeSharedAudioStream,
SetClientProperties, Start, audio capture/render or setting setter. Formats are
bounded to4096bytes and freed, and COM is released after all interfaces.

| Frozen input | SHA-256 |
| --- | --- |
| `src/main.rs` | `292CA116112636C84B493E1DC0A9BC5933F9BBFC990582FCFE1446F38377DB59` |
| `Cargo.toml` | `6B50E49DC64A47960002FEC388D0D906B75FBE164E0FE2A2B07DF2B1826BFEBC` |
| `Cargo.lock` | `5B07BDF3C770C954F626C9B638195599AD696467166E8B1AE332BDC0817D6767` |
| Frozen debug executable | `1752914EF2415FD9C9E0BA017A3D3D9D7FD21317835160D0E2F96C2581CD1212` |

Offline build2 exits0 (8.36s); workspace format and strict all-target Clippy exit0
(Clippy6.57s). Initial build101 from missing windows crate Activate feature gates
is retained in `build.log`; adding StructuredStorage/Variant features fixes
compilation only and is not an audio repair. `frozen-receipt.json` and immutable
source/binary copies retain this sequence.

Independent pre-run source/apparatus review gives conditional GO for the readonly
query only: `capability-review/receipt.json`, SHA-256
`009B366C93712BD90C1867F9A96A24796391768ED3FCA177C3C8B435D98B1303`.

Root executes `python run-period-query.py` from the FUNG worktree. The controller
rehashes each allowlisted helper before execution, creates fresh `query01`,
and reads inventory/defaults plus the exact-ID mute guard before/after each
query. Both endpoints are Active, nondefault, idle and muted at all admission
snapshots; no unrelated process is stopped or rerouted. Each of10 owned helper
calls exits0. The two capability JSONs retain the full mix-format blobs,
GetDevicePeriod, interface cast, shared ranges and current format/period.
`query01/summary.json` records both outcomes as NO_SMALLER_PERIOD_ADVERTISED.
No stream was started, so no PCM/audio or user speech was captured.

Independent offline verification accepts the bounded result, reconciling all10
call receipts and23 query JSONs, parsing both IEEE_FLOAT format GUIDs and checking
the frozen controller/helper provenance. See `period-query-review/README.md` and
`period-query-review/receipt.json`. No native actions are performed by the reviewer.

## Source and remaining acceptance

The product Rust, frontend, dependencies, schemas and normal/UAT profiles are
unchanged in this checkpoint. Hosted CI for exact product baseline `fff68af`
is independently queried and reports success (CI run40771786832, `baseline-ci.json`); later docs-head
CI is separate. Previous902library/17integration and installed evidence remain
at their recorded revisions; these docs changes do not claim a new full rerun.

The independent raw-boundary audit confirms the old missing bands contain
active PCM, not discarded unplayed padding or an oracle defect. Actual Stop
versus immediate destruction of a retained renderer client is still unmeasured;
changing the target's lifetime would be a diagnostic, not an arbitrary-app fix.
Holding session objects in p03/p04 was not holding the actual render client.
The audit is `boundary-review/review-receipt.json`, SHA-256
`569A29326AB306702769800A03EE717A584A89E788DEFFA5AE28BA93AE0994ED`.

Primary-source research also identifies Chrome tabCapture as an explicit browser
source using a distinct browser audio path; it cannot be renamed PID or close
arbitrary-app acceptance. No extension/browser capture/transport contract is
implemented. Endpoint source-aware playback still needs a reviewed interface
amendment: the current mic/system/file channel DTO cannot represent source UUIDs.
No unused resolver or denial-only change is counted as completed playback.

Both full installed gates stay open. The [prior actual installed report](2026-10-28-installed-audio-retest.md)
remains PID FAIL and endpoint PCM/separation component-only evidence. No package
is presented as repaired while no new PID repair is demonstrated.
