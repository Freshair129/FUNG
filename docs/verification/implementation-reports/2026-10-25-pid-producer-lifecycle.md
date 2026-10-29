---
version: "0.1.1b"
created_at: "2026-10-25"
last_update: "2026-10-26"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Original PID capture against two owned producer stacks on explicit HDMI"
  language: "Thai-first"
---

# PID capture — producer and session lifecycle checkpoint

Follow-up, 2026-10-26: the owned 44.1-kHz diagnostic was subsequently executed
with a corrected readiness controller; [p08 evidence](2026-10-26-source-custody-foundation.md)
records FAIL. The NOT_RUN section below preserves the earlier checkpoint, not the
current execution status. Production PID capture remains unchanged/failing.

## Disposition

**Original PID mode remains FAIL.** The user chose to retain PID capture and
require **both PID and endpoint modes to pass before release**. The
[endpoint feasibility PASS](2026-10-25-dedicated-endpoint-feasibility.md) and
[read-only admission foundation](2026-10-25-route-admission-foundation.md) do not
waive #96. No rebuilt installed acceptance, merge, tag or release is claimed.

Baseline source is `607079f` on `fix/issue-96-app-loopback-transition`. Native
PID reader `live_application_audio.rs` remains SHA-256
`FFE188C5CE2A7BCCDDACADCD5D9A415D6065A1864FA2082806B18109264C79D6`.
Temporary flag diagnostics were frozen externally and restored byte-for-byte.

## Scope and controlled changes

Windows 11 Pro build 26300. Evidence root:
`G:\AI-Studio\verification\fung-endpoint-routing-20261025`.
All new audio is from owned marker programs on LM22-B200S, exact endpoint
`{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}`.
Focusrite/Dota is excluded. No application routing/default, driver, target injection
or endpoint mute change is part of these producer studies.

The same 48-kHz nonperiodic marker is played through either an owned explicit
WinMM waveOut renderer or a native WASAPI renderer. WinMM maps IMMDevice ID to its
numeric device using the documented instance-ID messages and rechecks after open;
it never selects by friendly name or default. Both sources destroy/recreate their
stream twice in the same process, resetting source position to zero.

A separate silent keeper maintains the endpoint. Original selected-PID capture
runs for 12 seconds alongside an independently compiled I16 endpoint reader for
20 seconds. Marker interiors align the timelines; unequal initial/final tails are
censored. WinMM position after Reset is zero and is not the playback oracle.
The reference must be clean and self-log the intended opaque HDMI ID. The existing
48-frame/1-ms edge criterion is unchanged.

## Independent results

| Run | Isolating condition | Missing active frames before Stop 1 / Stop 2 | Missing restart frames 1 / 2 |
| --- | --- | ---: | ---: |
| p01-winmm-hdmi | WinMM on HDMI, original PID reader | 2,464 / 1,984 | 0 / 0 |
| p02-wasapi-hdmi | WASAPI on the same HDMI endpoint | 1,984 / 1,984 | 0 / 0 |
| p03-wasapi-retained | Hold target session COM references, WASAPI | 1,984 / 1,984 | 0 / 441 |
| p04-winmm-retained | Hold target session COM references, WinMM | 1,984 / 1,984 | 0 / 0 |
| p05-wasapi-reference-flags | AUTO retained, only SRC_DEFAULT_QUALITY omitted, WASAPI | 1,984 / 1,984 | 441 / 441 |
| p06-winmm-reference-flags | Same exact flags, WinMM | 1,984 / 1,984 | 0 / 0 |

Every application probe exits 101 with four source gaps; endpoint controls,
producers and keepers exit 0, with no endpoint-control faults. Retainers exit 0.
Missing pre-Stop bands correlate above 0.999999964 with source media and contain
substantial nonzero PCM, not idle padding. A 1,984-frame band is about 41.33 ms;
the 2,464-frame band is about 51.33 ms. The 441-frame restart bands also contain
active fading audio; they are not reclassified as silence.

Producer stacks also differ in representation/conversion: WinMM accepts mono
PCM16 while the WASAPI helper renders stereo Float32. The result shows failure
in both tested stacks, not causation by WinMM API alone. It extends the earlier
Focusrite/PlaySound context but does not establish a universal Windows limitation
or the precise internal native drop point.

## Session-reference hypothesis

The retainer opens no audio client and renders no sound. It holds exact target
IAudioSessionControl2 references, the endpoint, manager and enumerators on an MTA.
Readiness requires a single-process target session; process generation is pinned.
No callback blocks the audio service and no volume/ducking/routing setter is used.

p03/p04 hold one target instance before PID capture starts, through
active → inactive → active → inactive → active. Observers record 403/405 polls,
no errors and exit 0. Holding the object alive does not repair the observed loss;
it is not equivalent to keeping an active renderer inside the target PID.

## Exact converter-quality flag hypothesis

wasapi 0.26's automatic conversion helper adds SRC_DEFAULT_QUALITY. The Microsoft
sample uses AUTO without that bit. Earlier no-conversion tests removed two flags;
p05/p06 now test the isolated bit while retaining AUTO, the same 48-kHz PCM16
format, shared/event mode and zero period/duration. Native initialization self-logs
`rate=48000 flags=0x80060000`. Both cases still fail.

Primary configuration reference:
[Microsoft ApplicationLoopback source](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/ApplicationLoopback/cpp/LoopbackCapture.cpp).
The test is an owned Rust diagnostic, not execution of the previously blocked
Microsoft sample executable or a wrapper around it.

## Historical unexecuted 44.1-kHz checkpoint — 2026-10-25

An owned MF Capture-queue diagnostic matching the sample's requested PCM16 stereo
44,100-Hz format and `0x80060000` flags was built, but runtime is **NOT_RUN**.
The launch request returned “aborted before dispatch” without a stated reason;
the planned p07 directory was not created. This is not a native API failure,
an approval-policy diagnosis, or evidence that the format works. No alternate
execution path is used to bypass that result.

Artifact: `G:\AI-Studio\verification\fung-issue96-20261025\mf-reference44100\mf-reference44100.exe`.
SHA-256 `379BB9837C30F1E4778DCEDF0360C581E781525475AC699867B30161DEA9C070`.
`predeclared-pid-44100-analysis-policy.json` was frozen before any planned audio:
exact native frame/rate custody, two independent offline analysis views and
at most two interpolation frames **inside** the original 48-common-frame bound.
No cross-rate one-LSB claim or extrapolated/padded capture is allowed.

## Artifact custody

| Owned executable | SHA-256 |
| --- | --- |
| Native unchanged PID probe | `C890F844F51BDB314890103DE59429188C1A97C79A71103358C07C1EF7316B54` |
| Exact-reference-flags 48-kHz PID probe | `B1F62CA62B920F1D145C9F5DF2EC8C87F227DDDD81707FBE0D578C347424293D` |
| Explicit WinMM renderer | `08F5E65224F3BE6741C089EF6595279C5CE7F6C690AAB70BC087E5669D09323C` |
| Read-only session retainer | `64219976DD6342C3227CF7201C70BD1AA044996D726A7C3DF0B0129656ED3C88` |

`run-pid-producer.ps1` and per-run controller snapshots retain arguments, PIDs,
exit codes, target/reference identities, render/session events and PCM. Independent
`analyze-pid-producer.py` results are named `p01` through `p06` with
`-independent-pid-analysis.json`. Consolidated conclusions and hash receipts are
in `p01-p06-independent-conclusions.md` and the study-specific custody JSONs.
No failure receipt was overwritten with a passing verdict.

## Next gates

A subsequent actual 44.1-kHz execution/unchanged-bound analysis still fails, as
linked above. A demonstrated native PID correction is required before
claiming #96 repaired. Source-custody integration, rebuilt installed endpoint/PID
workflows, exact-head CI/review/merge and release remain separately open. Neither
a read-only API pass nor another endpoint-only PASS is release qualification.
