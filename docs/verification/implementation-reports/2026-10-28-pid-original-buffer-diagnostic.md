---
version: "0.1.0b"
created_at: "2026-10-28"
last_update: "2026-10-29"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Original WASAPI PCM16 stereo before decode, native owned fixture only"
  language: "Thai-first"
---

# PID original-buffer diagnostic — 2026-10-28

## Result

เสียงที่ขาดอยู่ใน original stereo ที่ `GetBuffer` ส่งกลับแล้ว ทั้งช่องซ้ายและขวา
ก่อน decoder/downmix และก่อน writer การคำนวณ mono แยกต่างหากตรงกับทุก packet
และ writer เก็บ mono ที่ได้รับครบทุก sample แต่ **PID #96 ยัง FAIL**
ผลนี้ระบุขอบเขตของความสูญเสียในรอบทดสอบ ไม่ได้พิสูจน์สาเหตุภายใน Windows
หรือว่าดึงทุก packet ที่อาจมีอยู่สำเร็จแล้ว ไม่ใช่ production repair หรือ installed PASS

| Stream compared with fault-free endpoint control | Cycle 1 Stop / Resume | Cycle 2 Stop / Resume |
| --- | ---: | ---: |
| Original left | -1,984 / +441 frames | -1,984 / +441 frames |
| Original right | -1,984 / +441 frames | -1,984 / +441 frames |
| Independently downmixed original stereo | -1,984 / +441 frames | -1,984 / +441 frames |

Every fit is credible and fails the unchanged 48-frame criterion. Original left
and right are not byte-identical; no identical-stereo assumption is used to reach
this result. The reference is the actual concurrently captured endpoint waveform,
not expected file length, renderer-submitted duration or fabricated silence.

This executes the [bounded diagnostic plan](../../plans/2026-10-28-pid-original-buffer-diagnostic.md)
after the [installed retest](2026-10-28-installed-audio-retest.md) located loss before
clock/queue/writer but had not retained original stereo bytes. Both complete
installed PID and endpoint acceptances remain open; no merge/tag/release.

## Source and acquisition contract

Baseline `66a09d9405ed3aabb961651ff2a3b44a8f78cc1e` only commits the preceding
installed-test documents; its production runtime is still source611aa0b.
This diagnostic changes two files:

| Frozen source | SHA-256 |
| --- | --- |
| `src-tauri/src/application_native_payload_diagnostic.rs` | `07C9CC459C99F5E365B39915334A6545A95996D5FAA6C7FC5495E4E061F088C6` |
| `src-tauri/src/live_application_audio.rs` | `6A0029036F7DF2E6D9DA723C1AEA4400605905C81E9C66E1B8602743A4AB0032` |

All additions to the actual acquisition path are cfg(test), and the diagnostic
module is cfg(all(windows,test)). Production decoder/backend/flags/format/wait/
clock/queue/writer behavior remains unchanged. The same explicit ignored payload
probe arms a scoped caller-thread-local single-use request; no shipped switch.

After successful `GetBuffer`, original PCM16 stereo is copied to preallocated
memory before decoding and before `ReleaseBuffer`. At most 1,152,000 frames /
4,608,000 raw bytes / 8,192 packet receipts. SILENT and empty buffers retain flags,
frame count and identity with zero raw payload bytes, never invented raw zeros.
Invalid null/oversize pointers are not dereferenced. Overflow invalidates evidence
completeness without changing reader/writer outcomes. All serialization, hashes
and fresh-only file writes occur after native stop and writer join.

Microsoft documents the [GetBuffer acquisition/frame contract](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer)
and [buffer validity until ReleaseBuffer, including avoiding excessive release delays](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-releasebuffer).
These contracts motivate bounded memory-only copying; they do not explain or
guarantee a repair of this host's observed transition loss. Reading the Microsoft
sample's documentation did not execute the previously blocked sample.

`observedElapsedNs` on original receipts samples reader entry before GetBuffer;
mono observer time is sampled later. Neither is misrepresented as the native
capture timestamp. Packet association uses counts/order/frames/flags/deviceIndex/
native QPC, with separate successful stop/capture/writer requirements.

## Frozen executable and real native n01

Evidence root: `G:\AI-Studio\verification\fung-pid-original-buffer-20261028`.
Windows 11 Pro `10.0.26300`, Rust 1.99.1. Actual ignored-test executable:
`implementer\original-native-payload-probe.exe`, 39,449,088 bytes, SHA-256
`C8C14FF80AA1F7EAC8E2EABB635D4855AA08B2B573C8D9821689AA655C6772FC`.
The executable and embedded source hashes match frozen/current source snapshots.

Root reuses the byte-identical previously reviewed native controller, SHA-256
`D0C9E887011FEA0128ACEBFD45119C2128A9B85110AFCAD2488AD01F2FACE085`, in
fresh `n01` under this new evidence root. This is distinct from old hardware-blocked
`fung-installed-remediation-20261027\n01`; no evidence is overwritten.

Root checks the exact idle/nondefault/muted LM22 endpoint
`{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}`, frozen native fixture
hashes and owned process identities. Mi Monitor also passes initial read-only
readiness but is not captured in this single-endpoint PID study. During capture,
33 inventory and 34 mute observations retain admissible exact LM22 state.
These are sampled observations, not a security lock or zero-transient guarantee.
No routing/default/volume setters, microphone/user-app capture, target injection,
driver installation, UI control or manual model/GPU invocation.

- Owned WASAPI renderer PID400: two Stop/destroy/recreate cycles, exit0.
- New subject PID15788: exits101 because one final `faults.is_empty()` assertion
  reports four genuine discontinuity intervals. Native stop, capture-loop and
  writer results separately return Ok.
- Independent explicit LM22 endpoint reference PID21476: exit0, no faults.
- No force termination or cleanup failures. All owned native processes terminate.

Command, with a fresh-only run directory:

```powershell
python G:\AI-Studio\verification\fung-pid-original-buffer-20261028\run-native-payload-study.py n01 G:\AI-Studio\verification\fung-pid-original-buffer-20261028\implementer\original-native-payload-probe.exe C8C14FF80AA1F7EAC8E2EABB635D4855AA08B2B573C8D9821689AA655C6772FC
```

## Byte reconciliation and independent review

Actual raw evidence: 3,838,080 bytes / 959,520 stereo frames / 1,999 packets.
Every packet is a payload packet; this run has zero SILENT packets. Both raw and
mono collectors are complete. Original packet metadata/offsets, byte lengths and
SHA-256 match; all source/binary identities and native lifecycle results qualify.

The offline analyzer independently truncates negative odd stereo sums toward zero,
matching Rust's signed division. Every resulting mono packet byte matches the
recorded mono. An independent reviewer additionally uses scalar
`struct.iter_unpack`, without reusing the NumPy downmix algorithm, and reproduces
all mono bytes. All **12 explicitly reported writer WAVs** retain the full mono
sample sequence. This is byte custody, not proof of exact sub-millisecond writer
timeline placement or authoritative Genesis source ledger closure.

`n01\original-analysis.json` SHA-256
`486721B0F077AF15062DE134EFE35411AA76B0CFC99E73281D490DEF66BBDAFB`;
the independently rerun analyzer output is exactly equal. Its authority gates
require matching target PID/endpoint, fault-free/nonoverlapped control, two actual
Stop/destroy/recreate events and valid producer outcome. Subject failure is
retained; requiring a subject PASS would conceal the defect being investigated.

Root's analyzer-assurance tests retain six semantic RED cases that previously
accepted wrong PID/endpoint, excessive control overlap, producer error or invalid
transition protocol/count. Corrected four-test GREEN passes without changing the
48-frame policy. Earlier three-test checker runs remain historical snapshots.

Independent source/analyzer review at frozen hashes: no MUST findings;
`reviews-source\source-review-v2.json` SHA-256
`599C782790A0125C4EA52A280E8B16DA67E8980856E7A56DF7A966F66F891E71`.
Earlier source-review files are retained preliminary checkpoints, superseded by
v2 after final focused build/binary freeze. The reviewer performs no native actions.

Independent actual-data review: `reviews-native\independent-native-review.md`,
SHA-256 `EAF970CB78593D5D0BC212D40384201766CE9A831092C8D736AD210FCA917F25`;
`receipt-audit.json`, SHA-256
`CE2D6846126A431B656AC6A843FBE19E2D8B7C0950193EC3CD4C8735C6F870E5`.
The scalar audit verifies all packets/source snapshots/fixture hashes and sampled
guards independently. This review has no native/process/audio/UI side effects.

## Source verification and non-test build

- Meaningful RED: two tests compile and fail because original payload/silent
  evidence is absent. Initial GREEN2 and final focused **40 passed / 3 native
  probes ignored** retained; final focused runtime 39.40 seconds.
- An expanded COM fixture initially fails compilation because the implementation
  macro resolves mismatched windows_core versions. This is retained apparatus
  failure, not semantic RED or a product runtime defect. A local borrowed COM
  vtable replaces the fixture without Cargo/dependency changes. It exercises
  the real reader's empty/SILENT/invalid/acquisition/release-error branches without
  any Windows device activation.
- Rust1.99.1 full library: **890 passed / 0 failed / 13 ignored**, 397.21 seconds,
  command `cargo +1.99.1 test --lib -- --test-threads=2`. The ignored native result
  above is separately FAIL; the full CPU suite cannot override it.
- Rust1.99.1 integration `cargo +1.99.1 test --test meeting_knowledge`:
  **17 passed / 0 failed**, 1.94 seconds of test execution after compilation.
  Its child-process filtered test output is part of these 17, not an extra test.
- Rust1.99.1 formatting and strict all-target Clippy pass; Clippy27.85 seconds.
- Non-test build: `cargo +1.99.1 build --release --bin fung --jobs 2`, exit0,
  6m09s. Frozen `production-no-diagnostic.exe`, 30,630,400 bytes, SHA-256
  `D064756303B9D8930476C9CC2AA06CEA7CA9BB511E84612DB185F4FF9785CCC5`.
  Six raw/mono diagnostic schema/path/overrun markers are absent in ASCII and
  UTF-16LE (`production-image-check.json`). This is a bounded marker check plus
  cfg source review, not exhaustive binary equivalence to the UAT package.
  The built executable is not launched or installed; no new installer is claimed.

Logs, exit files, frozen snapshots, actual-data analysis and marker checks are
retained under the external evidence root. Completion dates for source/doc checks
may extend into 2026-10-29; original native capture evidence remains dated2026-10-28.

## Limits and next boundary

This evidence excludes sample mutation/loss in decoder/downmix and writer for
this run. It locates missing active source bands in bytes actually delivered by
GetBuffer. It does not prove every possibly available native packet was retrieved,
identify a particular Windows internal defect, generalize to all Windows builds/
devices/apps, or eliminate measurement perturbation from the added memory copy.

No further timer/flag/format guess or inferred zero fill is adopted. A production
backend change needs a supported architectural proposal and fresh acceptance;
same-PID silent keepalive, implicit endpoint fallback or tolerance expansion cannot
close the existing contract. Previously installed source611aa0b remains the actual
package authority: PID FAIL; endpoint PCM components PASS but full source-aware
consumers/custody/recovery remain unqualified. This diagnostic neither rebuilds
nor reinstalls that UAT package and cannot upgrade those installed verdicts.
