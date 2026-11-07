---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Installed PID and dedicated-endpoint audio UAT; release blocked"
  language: "Thai-first"
---

# Installed PID / endpoint audio UAT — 2026-10-27

**ผล: ยังไม่ผ่านทั้งสองโหมด ห้าม merge/tag/release.** ติดตั้ง candidate ใหม่จริง
และทดสอบ capture ผ่าน IPC ของแพ็กเกจที่ติดตั้ง ไม่ได้ใช้ unit probe แทนตัวแอป.
PID ยังสูญเสีย active PCM ที่ Stop/Resume; endpoint สร้างไฟล์แยก source ได้
แต่ยังพิสูจน์ขอบเสียงและ workflow ทั้งหมดไม่ได้. ผลนี้ไม่ปิด issue #96.

Authority: [original #96 contract](../../plans/2026-10-25-issue-96-application-transition.md),
[lifecycle plan and readiness follow-up](../../plans/2026-10-27-pid-packet-lifecycle-plan.md),
[earlier offline/build snapshot](2026-10-27-pid-packet-lifecycle-candidate.md).
Source revision: `f06c9f1fb87f58efadab35b789e9aabc7ec89675`;
branch `fix/issue-96-app-loopback-transition`, Draft PR #98. No runtime source
changed during this installed campaign. Earlier build-only restrictions describe
the earlier stage; the user subsequently explicitly authorized installation and
real audio testing, retaining the two-mode merge/release gate.

## Environment and package identity

Windows host, local WASAPI, two active idle nondefault HDMI endpoints:
LM22 `{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}` and
Mi Monitor `{0.0.0.00000000}.{febdb10f-94d3-4af8-af69-b3ac1e887122}`,
48 kHz. Both were already muted; read-only volume checks retained. No volume,
default output, application routing or microphone setting was changed. Owned
marker processes rendered only to these explicit endpoints; no user app or mic
was recorded. Actual desktop visual interaction was not tested in this campaign.

| Artifact | SHA-256 |
| --- | --- |
| NSIS installer, 2,116,581,508 bytes | `D1A236756D1A5348C8E10E7FB973DEC99825FC101F17E66D58241962A0A32806` |
| Built executable before official NSIS marker mutation | `0F5EE99FB4F03133581675D43C087EE530AB98B2620E06D5D770638400FF256D` |
| Actual installed executable, 34,938,368 bytes | `A70257961855EA3759AC5BF1927F216EC08D47B65BF4E587DC1ECA1B34C18015` |

Package root: `G:\AI-Studio\verification\fung-pid-packet-lifecycle-20261027\package`.
Installed image: `installed\fung.exe`; isolated app ID
`dev.fung.uat.pidguard20261027`. NSIS silent install exited **0** and installed
hash matched the independently predicted marker-patched image. Normal
`dev.fung.local` and package C were not used. Health confirmed the isolated
GenesisBlockDB profile; no direct SQLite access or public recording API was used.

## Actual execution and retained invalid attempts

Evidence root: `G:\AI-Studio\verification\fung-installed-audio-uat-20261027`.
Project `86bd9555-48cf-4fbb-acae-d5d3caf0d54b`, created by the installed app.
The installed WebView's loopback CDP port 9339 was verified as a descendant of
the exact pinned installed process. IPC uses actual Tauri start/status/stop and
metadata commands. Renderers use owned process handles, explicit endpoint IDs
and two Stop/destroy/recreate cycles; graceful cleanup receipts are retained.

| Run / recording | Observed result | Acceptance meaning |
| --- | --- | --- |
| ep01 / `82c44cf9-afbb-4ed2-9bf1-b7b2d9f3bd26` | Native completed; only 3,008 ms visible data | Invalid schedule: public `recording` precedes the future common origin; first transition was too early. Not an audio PASS or product failure. |
| pid01 / `3331d326-efc6-4bbc-88e5-9c60d350b4a0` | Actual capture job failed with 4 lost chunks/media intervals, 0 stream faults | PID edge FAIL; see below. |
| ep02 / `72ead8ba-6348-44a7-bf51-bdfbbf11ae20` | Controller failed to read an existing 266-character WAV path; native cleanup completed | Apparatus reader failure. Original receipt retained; no product criterion relaxed. |
| ep03 / `2cd77277-bc12-4b5c-8e69-a123be73e5c7` | Both sources finalized, 625 accepted packets each, 4 WAV chunks each; 361,923 / 361,949 durable frames; no first/cleanup failure | Capture-to-file execution demonstrated. Endpoint aggregate acceptance NOT_PROVEN. |

The corrected ep03 controller waits for **96,000 actual visible WAV frames per
source**, rather than trusting phase readiness, before the first transition.
The Windows long-path correction only normalizes reads of owned artifacts with
the extended path prefix. It does not change backend paths, product code,
timing tolerance or generated audio. All prior controller failures remain.

## PID: original transition failure reproduced

An independent 20-second endpoint reference captured the same owned renderer
alongside the installed PID recording. It exited 0, reported no faults, and
qualified under the unchanged policy. Inner comparison windows correlate at
approximately 0.9999999; first/final outer recording edges are censored.

| Transition cycle | Missing active frames before Stop | Missing active frames after Resume | Result at 48-frame bound |
| --- | --- | --- | --- |
| 1 | 1,984 (~41.33 ms) | 441 (~9.19 ms) | FAIL |
| 2 | 1,984 (~41.33 ms) | 441 (~9.19 ms) | FAIL |

The actual job error is `4 audio chunk(s) or media interval(s) were lost; 0 audio
stream fault(s)`. Recording projection retains `captureWarning: capture_source_fault`.
Its row status `completed` does not override the failed capture job or PCM result.
The legacy projection labels `source: microphone` although this request explicitly
disabled microphone and its actual channel is `system`; that label is not mic
capture evidence. Persisted target-generation/audit and authoritative chunk ledger
bindings are not exposed by this metadata projection and remain unproven.

## Endpoint: separate source files, incomplete edge proof

| Source | Cycle 1 edge | Cycle 2 edge | Cross-source leakage component |
| --- | --- | --- | --- |
| LM22 / marker A | PASS | NOT_PROVEN: fading tail extends 457 frames beyond the renderer clock/padding bound | PASS, derived foreign/own gain about -112.36 dB |
| Mi Monitor / marker B | PASS | PASS | PASS, derived foreign/own gain about -137.53 dB |

This is not a proven missing-audio failure for endpoint A: the extra fading tail
prevents bounding the played endpoint to 48 frames with the available oracle.
The analysis does not trim it away or promote it to PASS. Leakage uses aligned
own/foreign/DC regression and the unchanged 0.001 (-60 dB) policy, including its
confidence bound. It applies to sampled aligned windows on these owned sources,
not universal per-program isolation or a security boundary.

Private accepted packets match their corresponding covered WAV positions;
uncovered intervals and extra WAV tails are outside that equality claim. Filename
ordering does not prove authoritative `capture_sources`, `audio_chunks`,
`capture_source_chunks`, checkpoint or audit bindings. No independent endpoint
reference capture was supplied in ep03. Source-aware UI/export/recovery/enrichment
are explicitly still open in the product's final routed status. Public routed
recordings have `channels: []`; source custody alone is not a complete review flow.

## Export and orderly reopen

Actual `create_job(export.render)` probes were performed on pid01 and ep03:
jobs `7ae7c23b-9a40-49dc-bbb1-e71f6ea09e43` and
`b479cc98-9908-457b-b693-8983b48458ea`. Both failed because no transcript exists
for subtitle export. These non-speech markers are not suitable transcription
fixtures; no transcript was injected. This does not prove audio export, subtitle
success, enrichment, or source-aware consumer acceptance.

After all capture jobs were inactive, native window-close IPC closed installed
PID 2756 with **exit 0**, without force. Its original 30-minute monitor had already
timed out and honestly records that apparatus timeout; a fresh exact-generation
process handle observed the subsequent orderly exit. Reopen PID 28820 verified
the same installed hash/profile, the exact same project and all **4 recording
metadata rows**, then closed **exit 0**, without force. This proves orderly
persistence, not crash recovery or playback UI acceptance.

The ordinary-path before/after media seals enumerate only 17 files because long
paths were omitted by that enumerator. They are retained as partial evidence.
Extended-path enumeration finds **33** owned media files; all **13** pid01/ep03 WAV
chunk hashes saved before close match after reopen. No deletion or repair was
performed to make this comparison pass. Owned renderers/reference exited normally;
the test app and debug listener were closed after qualification.

## Reproduction, independent review and evidence integrity

These are records of already executed commands, not instructions to rerun audio
against a busy device. Run directories and output files must always be fresh.

```powershell
pwsh -NoProfile -File '<package-root>\install-new-uat.ps1'
pwsh -NoProfile -File '<package-root>\launch-installed-audio-uat.ps1'
python '<evidence-root>\run-installed-capture.py' endpoint ep01
python '<evidence-root>\run-installed-capture.py' pid pid01
python '<evidence-root>\run-installed-capture.py' endpoint ep02
python '<evidence-root>\run-installed-capture.py' endpoint ep03
python '<evidence-root>\prepare-analysis.py' pid01
python '<evidence-root>\prepare-analysis.py' ep03
```

The analyzer runs with bundled NumPy Python and explicit `--manifest` / `--out`
paths, documented in external `analysis\README.md`. Final analyzer SHA-256:
`E849A3BE18871AEA4EF5013CD174650B5D41039AC4A7E73004442B9CC5C7B883`;
ep03 controller SHA-256:
`1F9D040ECDB0DD1CD49E6B6DD25BF28923EBB62BB15FCA98D8FFECD6EC8A6573`.
Independent read-only review reproduced every PID/ep03 result field and matched
all 34 referenced receipt entries (32 unique input paths). It confirmed the reader-only change reconstructs
the previously reviewed analyzer when removed. Component conclusions remain
bounded by the missing ledger, oracle and consumer evidence above.

Review: external `review\pid01-ep03-independent-review-20261027T081944Z.md`,
SHA-256 `8630022609471C7D79C711FC67CEF06A091BC1A03BF2941BE61B17290507CD9B`;
its JSON reproduction/hash evidence has SHA-256
`A3511E63ECAC460C877FB3C1D846E6C6CC3C79BB28D541DF8ABA28ABAB50185E`.

External `installed-uat-evidence-manifest.json` freezes 259 evidence-file hashes,
33 owned media files and the 13 post-reopen chunk comparisons. Manifest SHA-256:
`016E74ED4AD3C3235AC7488DAEE3E4EDF7CF7809B3567F29B96BCF4DC9C59F5C`.
Original run JSON, controller failures, native render reports and derived results
remain in place; no synthetic native probe report was fabricated.

## Remaining gates

- Repair and rerun original PID active Stop/Resume loss under the same policy.
- Resolve/bound endpoint A's fading-tail oracle; retain both source isolation checks.
- Verify authoritative source/chunk/epoch/checkpoint binding and closure, plus
  actual source-aware playback/export/recovery/enrichment through installed consumers.
- Run remaining UI and any human speech/microphone acceptance separately; this
  campaign did not establish those by owned markers or API success.
- Both installed modes must pass before any merge/tag/release. No such action
  was performed by this campaign.

## Changelog

| Version | Date | Change |
| --- | --- | --- |
| 0.1.0b | 2026-10-27 | Recorded installed PID FAIL, endpoint NOT_PROVEN, retained apparatus failures, actual export rejection and orderly reopen persistence. |
