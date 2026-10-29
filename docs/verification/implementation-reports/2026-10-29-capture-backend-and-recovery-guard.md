---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Supported backend research, direct acquisition comparison and legacy recovery source fence"
  language: "Thai-first"
---

# Capture backend investigation and recovery guard — 2026-10-29

## Current result

**ยังไม่มี PID repair ที่ยืนยันว่าผ่าน** การอ่าน GetBuffer ตรง ๆ ก็ยังขาด
active PCM ส่วน recovery พบและแก้บั๊กจริง: เส้นทาง legacy เคยนำ chunk เข้า routed
recording และตอบสำเร็จได้ โดยไม่ได้กู้หรือปิด source epochs ของมัน

| Work | Evidence result |
| --- | --- |
| Different supported direct-PID backend | No independently different replacement or exact-loss fix verified in the primary sources inspected |
| Fresh baseline size-hint gated native capture | FAIL: Stop/Resume -1,984/+441 frames in cycle1, -1,504/+441 in cycle2 |
| Fresh direct GetBuffer native capture | FAIL: -1,504/+441 frames in both cycles |
| Original stereo → mono → writer | Complete byte custody in both runs; no recovery of missing source bands |
| Legacy recovery source admission | Actual-entry RED5; fixed recovery module GREEN15; source review GO |
| New installed two-mode audio acceptance | NOT_RUN: no demonstrated PID fix to promote into a repair package |
| Merge/tag/release | Not authorized by the open acceptance gates; none performed |

Source baseline `f399d020ca875d18bd5d03a00b50288a14e4b33b`, existing isolated
`fix/issue-96-app-loopback-transition` worktree. Execution follows the
[bounded plan](../../plans/2026-10-29-direct-acquisition-and-recovery-guard.md).
The [previous raw-buffer investigation](2026-10-28-pid-original-buffer-diagnostic.md)
and [actual installed retest](2026-10-28-installed-audio-retest.md) remain distinct
historical evidence. The user confirms only this Windows machine is available;
cross-host qualification is unavailable, not implicitly passed.

## Backend research and acquisition hypothesis

Root/researchers inspect primary interface documentation and maintainer-owned code.
[OBS](https://github.com/obsproject/obs-studio/blob/master/plugins/win-wasapi/win-wasapi.cpp),
[bozbez](https://github.com/bozbez/win-capture-audio/blob/main/src/audio-capture-helper.cpp)
and [NAudio](https://github.com/naudio/NAudio/blob/main/src/NAudio.Wasapi/WasapiRecorderBuilder.cs)
activate the same Windows process-loopback service. Their existence is not
evidence of an independent PCM source or a fix for this loss. OBS's silent-stream
workaround is endpoint-only; NAudio rejects process capture combined with RAW/
IAudioClient3 low-latency options. These are inspected implementation constraints,
not a fresh capability query result for this host.

Inspected [AudioGraph device-input APIs](https://learn.microsoft.com/en-us/uwp/api/windows.media.audio.audiograph.createdeviceinputnodeasync),
[MediaFrameReader guidance](https://learn.microsoft.com/en-us/windows/apps/develop/camera/process-audio-frames-with-mediaframereader)
and [AppRecordingManager guidance](https://learn.microsoft.com/en-us/windows/uwp/gaming/capture-game-audio-video-screenshots-and-metadata)
do not establish an arbitrary third-party PID replacement. No corresponding
build-specific repair is verified. This is bounded research, not proof that no
possible solution exists or permission to update Windows/drivers, inject a target
or rename routed endpoint capture as direct PID.

The remaining local hypothesis is availability admission: all prior timer/MF
variants retain a GetNextPacketSize gate. Microsoft documents that the caller
[can query the next size](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getnextpacketsize)
and can [acquire directly, including an empty result](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer).
No false-zero hint had been measured. A synthetic source exposing data after a
zero hint models an availability race, not a demonstrated Windows contract bug.

The candidate changes only acquisition admission: cfg(test) `run_direct` calls
the existing read/accept path until an empty buffer, preserving event wait,
format, stream flags, clock, writer, cancellation, single Stop and error cleanup.
Empty acquisition does not release a nonexistent packet. Production's positive-
peek/empty anomaly remains an error. Distinct ignored native tests and scoped,
single-use caller-thread requests select `size_hint_gated` / `direct_get_buffer`.
There is no shipped switch; both raw/mono receipts identify strategy and all
three source hashes.

## Frozen source, tests and native apparatus

External evidence root:
`G:\AI-Studio\verification\fung-capture-backend-research-20261029`.
Host: Windows `26300.9711`, DisplayVersion `26H2`, read from registry;
BuildLabEx `26100.1.amd64fre.ge_release.240331-1435`. Do not infer an OS cause or
release channel from those values alone.

| Diagnostic build input | SHA-256 |
| --- | --- |
| `application_packet_driver.rs` | `CA7C313D6F0141562604111B271F0F80FCCB8989E2E27B78B6DA44896F8ABED6` |
| `live_application_audio.rs` | `DCF51F925A38E2BF229564B5D0FFED03CAC2058147045B9F50BF576E27B7136A` |
| `application_native_payload_diagnostic.rs` | `B12FE1045A4ABE90B505C339620F3A6ACB5B77DD3C9A0C3540FE6EE58565D00A` |

Meaningful RED3 detects dropped zero-hint payload/final packets and the stopped
empty-read behavior. Focused GREEN **47 passed / 4 native ignored**,25.04s;
Rust1.99.1 fmt/strict all-target Clippy pass (Clippy1m08s), no-run build exit0.
Frozen probe `direct-implementation\direct-acquisition-payload-probe.exe`,
39,551,488 bytes, SHA-256
`19BFC254F9C5C0A5DB9D89A355712FFD911693EF65C19446930410BF3FCC49F1`.
Frozen snapshots/receipt cover the three changed Rust files at that build time.
The recovery guard is added later; this probe does not include or test it.

The root controller changes only the explicit strategy/test-name allowlist and
scope fields from its reviewed predecessor. SHA-256
`CC1D041CD9C4969DB76B0735E57CAF43EFB88B46DFA1FEFFD557B57658308DBB`.
Source/apparatus review: `source-review\source-apparatus-review.json`, SHA-256
`656B65CE835CF0F7DF283098944BCD80AD857258764E5C435923F6EED22F12EF`.

Root runs fresh `baseline01` then `direct01` with that same executable and actual
simultaneous explicit-LM22 endpoint reference, two owned Stop/destroy/recreate
cycles and frozen marker/renderer/control hashes. Exact LM22 ID ends
`c7033d7f-b081-40c6-a5ff-1df83a87d352`; it is active, nondefault, idle before
admission and muted. Mi Monitor also passes read-only initial readiness but is
not captured in these PID comparisons. There are no setting/routing changes,
microphone/user-app capture, drivers, target keepalive, UI input or model use.
Guards are sampled observations, not an OS-enforced security isolation guarantee.

| Run | Player / subject / reference PIDs | Exit codes | Inventory / mute observations |
| --- | --- | --- | --- |
| baseline01 | 31200 / 39572 / 5740 | 0 / 101 / 0 | 31 / 32 |
| direct01 | 10152 / 30748 / 43616 | 0 / 101 / 0 | 28 / 29 |

Both subjects fail one final `faults.is_empty()` assertion reporting four lost
intervals. Native Stop, capture loop and writer outcomes separately return Ok;
references have no faults. Cleanup has no failures or forced termination.

```powershell
python <evidence-root>\run-acquisition-study.py baseline01 <frozen-probe.exe> 19BFC254F9C5C0A5DB9D89A355712FFD911693EF65C19446930410BF3FCC49F1 size_hint_gated
python <evidence-root>\run-acquisition-study.py direct01 <frozen-probe.exe> 19BFC254F9C5C0A5DB9D89A355712FFD911693EF65C19446930410BF3FCC49F1 direct_get_buffer
```

## Actual audio comparison and independent confirmation

| Run | Payload / empty receipts | Stereo bytes / mono frames | Writer WAVs |
| --- | --- | --- | --- |
| baseline01 | 2,002 / 0 | 3,843,840 / 960,960 | 12 |
| direct01 | 1,997 / 1,998 | 3,834,240 / 958,560 | 13 |

Collectors are complete. Every original packet independently downmixes to every
mono byte, and all explicitly reported writer chunks retain that mono. Scalar
`struct.iter_unpack` audit independently reproduces the results without reusing
the NumPy downmix. Empty direct receipts have QPC0 and are not used as clock
points or as sampled silence. Every embedded source hash is compared with the
three frozen source snapshots, separately from raw/mono equality.

Original L/R and independently downmixed mono fail both transition cycles with
the tabled -1,984/-1,504 Stop and +441 Resume differences. The unchanged criterion
is48frames. A single480frame phase difference is not a demonstrated improvement:
the baseline itself also produces -1,504. No further delivery/flag variant is
adopted from this failed experiment, and the candidate stays test-only.

Actual correlated endpoint PCM is the digital reference. Renderer stream clocks
jump during Stop and are not continuously linear physical/acoustic playback
oracles. Queued unplayed padding is excluded; do not dismiss the observed missing
bands as that padding or claim a physical-device acoustic measurement.

Independent outputs exactly equal root outputs:

- Baseline analysis SHA-256
  `FA67097BC5818EAAA3D6B32C8BEC081F50C93098311F8CF643D07B1CAD66D4FE`.
- Direct analysis SHA-256
  `3133403DD8CA290080D4A773D0776DD674C13525A4033FD8FA9515B4D811A6CC`.
- `native-comparison-review\comparison-audit.json`, SHA-256
  `DDBA0EC9854FB9A05F63CF09CE6196E3FEE43D60E4CADF22EB532D92B63732EB`.

Two initial independent offline helper attempts fail on path/variable assumptions;
those logs remain separate. Corrected v3 audits and fresh independent analyzer
outputs pass. They perform no native/device/process/audio/UI actions. Source
custody does not prove every possibly available native packet was retrieved,
Windows internals, or an authoritative exact Genesis writer timeline.

## Real recovery defect and bounded correction

Legacy `recovery::recover_recording` scans `canonical/chunks`, adopts inferred
legacy channels, calls finish and writes a recovery audit. Routed data instead
belongs to `sources/<UUID>/epoch-*` and authoritative source bindings. Previously
the legacy entry could answer Ok or adopt a stray legacy-looking file for such a
recording without recovering its source epochs. `recovery_recover` IPC calls that
entry directly before transcript gap-fill.

The first executable statement now invokes the existing fresh
`require_legacy_media_source`. No new schema, command or source mode. Source marker
and authoritative bindings are read through Genesis before paths or mutations.
Five actual-entry RED cases—pending, active, quarantined, completed and a legacy
label hiding source bindings—previously each adopt one synthetic9644byte bait
chunk. The new guard rejects them and preserves SDK frontier, complete rows in
seven tables and every project media byte. Healthy legacy orphan adoption and
idempotency remain covered. Entire recovery module GREEN **15 passed**,8.82s;
fmt/strict all-target Clippy pass (Clippy22s).

Frozen `recovery.rs` SHA-256
`64F039E6EDDFAC5E2FE55F321CCFE56984EEC16422D2F73EFF4EE2744FBA8451`.
Independent review GO for that bounded source fix:
`recovery-review\review-receipt.json`, SHA-256
`0410DA8C9DA68EFF21CA7AE199CA48E2AC6058D9A37EC66F31F4C89911C10197`.
The IPC error propagates before gap-fill/model execution. This is fresh admission,
not a lifetime lease, implemented source-aware recovery or a package-level
five-state matrix.

## Verification and remaining gates

The first four-thread full library run retains **901 passed /1failed /14ignored**:
`local_api::tests::a_stopped_listener_releases_its_port` fails its initial ephemeral
bind with Winsock10055 (insufficient buffer/queue resources). It is not the new
recovery or native audio test. An exact standalone rerun passes1test in3.47s;
no assertion, port range, process setting or test source is changed. The failed
full result is not erased or promoted to a full pass by that standalone result.

A separate fresh complete serial run on all four frozen Rust files passes
**902 passed /0failed /14ignored**,911.61s, exit0:
`cargo +1.99.1 test --lib -- --test-threads=1` in `src-tauri`.
Logs `full-library-serial.log` and `full-library-serial-exit.txt` are retained
alongside the failed four-thread run and exact standalone rerun. The ignored
native tests are not counted as passed audio acceptance.

Fresh `cargo +1.99.1 test --test meeting_knowledge` passes **17 tests**,2.58s,
exit0 (`integration.log`, `integration-exit.txt`). Final workspace format check
also exits0 (`final-format.log`, `final-format-exit.txt`). The previously frozen
strict all-target Clippy run covers these same four Rust hashes and exits0;
the source hashes remain unchanged through both full suites.

Local Markdown path checking across the six changed documents verifies285
existing targets with zero missing paths (`changed-doc-links.json`); this does
not verify URL availability or anchor semantics. `git diff --check` passes.

No new repair installer or installed two-mode PASS is claimed. Native candidate
failure prevents its production adoption and a package presented as an audio
repair. The actual installed source611aa0b evidence remains PID FAIL; endpoint
has bounded PCM/separation passes but source-aware playback/export/transcription,
custody/recovery, routing UI/MCP and two-app-plus-mic qualification still have
open work. See the [routing plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md).
The new recovery admission fix is source-tested; actual installed verification
of that change is separate and has not been run at this checkpoint.

No merge/tag/release. The user's requirement that both full modes pass is retained.
