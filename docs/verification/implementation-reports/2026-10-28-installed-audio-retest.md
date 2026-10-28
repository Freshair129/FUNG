---
version: "0.1.0b"
created_at: "2026-10-28"
last_update: "2026-10-28"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Installed PID and dual-endpoint retest after HDMI availability returned"
  language: "Thai-first"
---

# Installed PID and endpoint retest — 2026-10-28

## Outcome and authority

ผู้ใช้เปิดจอทั้งสองและอนุญาตให้ทดสอบเสียงจริงต่อ โดยยังห้าม merge/tag
จนทั้งสองโหมดผ่านครบ รอบนี้ทดสอบ native diagnostic และแพ็กเกจที่ติดตั้งจริงแล้ว:

| Gate | Fresh result |
| --- | --- |
| PID decoded-native diagnostic | FAIL: active source bands already absent before clock/queue/writer |
| Installed PID Stop/recreate, two cycles | FAIL: 1,984 missing frames before Stop and 441 after Resume in each cycle |
| Installed endpoint PCM/separation | PASS component only: two runs, two sources and two transitions per source/run |
| Installed routed recording projection | Active during capture, inactive after completion; stopping transient NOT_OBSERVED |
| Routed playback | Correctly rejects `PLAYBACK_SOURCE_UNSUPPORTED`; feature acceptance remains open |
| Explicit export jobs | All three fail: PID timeline gaps; both endpoint recordings have no transcript |
| Orderly persistence | Project/three recording metadata retained; full 32-file media seal unchanged across a later close/reopen |
| Both full installed mode acceptances | NOT PASSED; no merge/tag/release |

This continues the [remediation plan](../../plans/2026-10-27-installed-audio-remediation.md)
and [source/package checkpoint](2026-10-27-installed-audio-remediation.md).
It does not overwrite the [previous installed failures](2026-10-27-installed-pid-endpoint-audio-uat.md)
or the earlier hardware-blocked n01 receipt. No product source changed in this retest.

## Exact package, environment and admission

- Source: `611aa0b628260e387a1d5f3a64617041dfb08b2d`, existing
  `fix/issue-96-app-loopback-transition` branch. Twelve selected build inputs
  rechecked against the recorded commit provenance; this is not an exhaustive
  resource manifest or a new build claim.
- Actual already-installed image:
  `G:\AI-Studio\verification\fung-installed-remediation-20261027\package\installed\fung.exe`.
  SHA-256 `017EC1C8C40C1DC6A742C453F092CF29ECB52EBEB1CD2CBBF5AF6E34283AAFE4`.
  The previous installation is reused after a fresh hash check; no reinstall is claimed.
- Isolated app ID `dev.fung.uat.remediation20261027`; normal `dev.fung.local`
  untouched. Hidden main window, legitimate Tauri IPC through an owned loopback
  WebView debugger, no desktop inputs/focus and no visible UI qualification.
- Fresh installed evidence:
  `G:\AI-Studio\verification\fung-installed-audio-uat-20261028\r01`.
- Existing isolated project `52a1f215-8c33-4993-86fe-9eba8bbefc34` retains its actual
  `storagePath` under the prior `fung-installed-remediation-20261027\installed-check\media`.
  Changing the future output root does not relocate that project; analysis and
  seals use the returned actual project path.
- Exact nondefault render endpoints: LM22
  `{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}` and Mi Monitor
  `{0.0.0.00000000}.{febdb10f-94d3-4af8-af69-b3ac1e887122}`.
  Both returned Active and muted. Fresh idle/session/default checks precede
  owned markers. No device/default/per-app-routing/volume setters, microphone,
  user-app capture, driver installation or target keepalive injection.
- Repeated read-only guards retained 58/18/17 observations for PID/ep01/ep02.
  Guards run during wait boundaries, not while an IPC subprocess blocks for up
  to 60 seconds. Neither sampled guards nor endpoint separation establish an
  OS security boundary or zero-transient-leak guarantee.

## Native boundary diagnostic n02

Root ran the frozen test executable SHA-256
`5E5C75E00A5DEE8F8B0E0E49D7B949C05B5075E5612BFE5C58F21CC0289E9B70`
with the exact ignored `native_application_loopback_payload_probe`, owned marker
renderer and simultaneous explicit LM22 endpoint reference. Evidence:
`G:\AI-Studio\verification\fung-installed-remediation-20261027\n02`.

The bounded collector is valid: 1,995 packets / 957,600 mono frames; native Stop,
capture and writer each return Ok. All 13 explicitly reported WAV chunks retain
every decoded-native mono byte, combined PCM SHA-256
`F071D8F246749FB6EFB55AFA06788D2D62559E16892D7482BE7C31A93AA77016`.
The subject exits 101 because four discontinuity assertions fail; renderer and
fault-free reference exit 0. No force termination or cleanup failure.

| Cycle | Native Stop minus reference | Native Resume minus reference |
| --- | ---: | ---: |
| 1 | -1,024 frames | +441 frames |
| 2 | -1,984 frames | +441 frames |

The unchanged tolerance is 48 frames. Missing bands have substantial,
source-correlated PCM in the independent endpoint reference. The loss is already
present in decoded/downmixed mono after GetBuffer/decode/ReleaseBuffer and before
ApplicationPacketClock/queue/writer. Original stereo buffer bytes were not retained:
Windows delivery versus decoding/downmix remains unresolved. This does not prove
a universal Windows/backend limitation, and it establishes no production repair.

Read-only offline analysis and independently checked receipts:
`n02\analysis\native-analysis.json` SHA-256
`BE460F156CE658E976957CA9A66C4A39B7F796CE98BC09B4B3EF599071A0C226`.

## Actual installed capture

| Run | Recording ID | Final source result |
| --- | --- | --- |
| pid01 | `352c0dd9-0801-4bac-b32c-0205e162329c` | Capture job failed with four lost intervals, zero stream faults; PCM edges fail both cycles |
| ep01 | `79568097-bf25-4313-a7bb-b27e7c6714da` | Two sources, 628 accepted packets/source, four durable chunks/source; 371,756 / 371,741 frames |
| ep02 | `99279285-d893-461a-a60f-abb86db300a6` | Two sources, 613 accepted packets/source, four durable chunks/source; 367,010 / 366,998 frames |

All three controllers finish without apparatus/cleanup failures; that is not an
audio PASS. PID record metadata still says `completed` while the capture job says
failed, with `capture_source_fault` warning retained. The legacy source label says
`microphone`, but requested microphone capture was false and the actual channel
is `system`; do not claim a microphone test from that label.

Both endpoint runs pass all eight combined source/transition PCM comparisons
and four leakage fits under the original 48-frame / 0.001 relative gain policy.
The prior 457-frame fading-tail NOT_PROVEN result remains historical evidence;
two fresh passes do not establish universal repeatability. Covered private
accepted-packet PCM agrees with covered WAV positions. Uncovered timeline slots
are not observed silence, and filename-derived placement is not authoritative
Genesis chunk/source/checkpoint custody. Full source-aware UI/export/recovery/
enrichment acceptance therefore remains NOT_PROVEN.

Real `desktop_recording_get` observes routed `captureState=active` during both
captures and `inactive` after completion. No installed stopping transient was
observed. CPU stopping regressions from the prior source checkpoint remain a
separate evidence category.

## Consumers, automatic processing and persistence

- Both completed routed recordings reject actual `desktop_playback_open` with
  `PLAYBACK_SOURCE_UNSUPPORTED`. This verifies the new admission fence in the
  installed package; it does not implement source-aware playback.
- Three actual `export.render` jobs fail. PID rejects its noncontiguous timeline
  (`2450..3480`, gaps must not be compressed); endpoint recordings reject missing
  transcripts. No transcript was inserted to manufacture a pass.
- The normal post-capture pipeline did automatically run for PID: a summary job
  reports completed and three Markdown/SRT/VTT files exist. Their existence is
  retained as output evidence, not speech accuracy, source completeness, successful
  explicit export-job acceptance or provider/backend qualification. This controlled
  input is non-speech marker audio. No manual model/quality optimization was run.
- First orderly close/reopen retains exact raw project and three recording
  projections. Initial ordinary-path media enumeration covers only 16 files;
  that retained seal is partial because long source paths were omitted.
- Corrected read-only extended-path enumeration covers all **32 files**:
  **21 WAVs**, **8 private spool files**, **3 generated outputs**. Its baseline
  is taken after the first reopen; all 32 sizes/hashes remain identical across
  the next close/reopen. Do not describe this later full seal as a full seal
  taken before the first close.
- Installed PIDs 12184, 2804 and 21916 close through native window IPC, exit 0,
  no force kill. All three close-command client receipts separately report
  `FAIL` / `CDP timeout: Runtime.evaluate`: the WebView disappears before the IPC
  reply is acknowledged. Native process exits and persistence are established
  by retained process handles/reopen receipts, not a passing transport reply.
  Recording/route ownership is inactive on reopen. This is orderly persistence,
  not crash-recovery or active-capture recovery acceptance.

## Reproduction and review

Root-only native command:

```powershell
python G:\AI-Studio\verification\fung-installed-remediation-20261027\run-native-payload-study.py n02 G:\AI-Studio\verification\fung-installed-remediation-20261027\native-payload-probe.exe 5E5C75E00A5DEE8F8B0E0E49D7B949C05B5075E5612BFE5C58F21CC0289E9B70
```

Installed controller under `fung-installed-audio-uat-20261028` ran `pid pid01`,
`endpoint ep01` and `endpoint ep02`, using fresh-only launch/IPC receipts.
`prepare-analysis.py` supplies explicit actual paths to the existing read-only
`analyze-installed-audio.py` (SHA-256
`E849A3BE18871AEA4EF5013CD174650B5D41039AC4A7E73004442B9CC5C7B883`).
Bundled NumPy Python runs all offline audio comparisons. Each derived JSON retains
input/analyzer hashes and its limited evidence scope.

Independent apparatus review: `reviews-controller\controller-review.json`,
SHA-256 `E22BC9788E3B64F86F4794A80A356E40FE4AE1C8E2EDD0419E591790D53B4831`.
Independent installed reanalysis reruns all three analyses and reproduces root
outputs; actual image, selected source inputs, cited audio inputs and guard samples
checked. `reviews-installed\independent-installed-review.md`, SHA-256
`89C843DFF97B9F318B23DEA59FCD9EE72F8E2CB94B4F0B321C55B5E1C41A0FEB`;
`receipt-audit.json`, SHA-256
`2C23E54FDE77CB63EDA65671645C58F49B1BB724CD7AFA0E0B230FEC46C63444`.
These reviewers performed no native/audio/UI actions. Later persistence/consumer
checks are separately recorded in their actual IPC/seal/close receipts.

Remaining work: establish original-buffer versus decoder delivery boundary before
another PID backend change; retain honest failed gaps; implement/qualify routed
consumer and authoritative custody/recovery flows. Both complete installed gates
remain open. No merge, tag or release operation was performed.
