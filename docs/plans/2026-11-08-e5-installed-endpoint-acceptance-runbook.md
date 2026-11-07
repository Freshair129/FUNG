---
version: "0.1.0b"
created_at: "2026-11-08"
last_update: "2026-11-08"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-audio-capture"
  doc_type: "runbook"
  scope: "E5 installed acceptance of endpoint-routed capture for v0.1.2 (backlog B-08, B-09)"
  language: "English"
  execution_status: "NOT_RUN"
  authority: "Owner runs or approves the session; lead records evidence"
---

# E5 — Installed endpoint acceptance runbook

This runbook closes E5 of the
[endpoint-first release plan](2026-11-07-endpoint-first-release-plan.md). It
also covers Task 4 of the
[dedicated-endpoint routing plan](2026-10-25-dedicated-endpoint-routing-plan.md)
for endpoint mode. PID mode is experimental under the
[2026-11-07 decision](../decisions/2026-11-07-endpoint-first-release-pid-experimental.md)
and is not a gate here.

**Owner:** project owner (hardware, routing in Windows, speech).
**Recorder:** lead (evidence files, report). **Status:** NOT RUN.

## 1. Candidate identity

| Item | Value |
| --- | --- |
| Source | `main` `e7608d0214e8bd73f0332d5f87ad257dc03171f4` (E2, E3, E4, E6 merged; CI green) |
| Installer | `FUNG Capture UAT E5 20261108_0.1.2_x64-setup.exe`, 2,117,926,012 bytes |
| Installer SHA-256 | `0B7302C2B36E4B8AB52D873F77BB00E1CF81C725F15E6480F98E27628CC8EF81` |
| Signature | Authenticode `NotSigned` (UAT; NSIS compression `none`) |
| Identity | product `FUNG Capture UAT E5 20261108`, identifier `dev.fung.uat.e5endpoint20261108` (separate profile) |
| Installed to | `G:\AI-Studio\verification\fung-e5-endpoint-uat-20261108\installed` (silent install exit 0) |
| Installed `fung.exe` SHA-256 | `888A846C0D7B405CE928D733C19DE8ED0154F76B17617D01FA44CF00382D1446` |
| Bundled worker probe | `.venv-whisper` Python 3.11.9 imports `faster_whisper` |

The evidence root is `G:\AI-Studio\verification\fung-e5-endpoint-uat-20261108`
(`installer-identity.json`, `installer-build.log`). It is a UAT package. The
release distributable is rebuilt from merged `main` with normal compression
(B-13).

## 2. Prerequisites

- At least **two output endpoints that are not the Windows default**. For
  example: a USB headset plus a monitor's HDMI/DP audio, or two USB adapters.
  FUNG rejects the default endpoint (`DefaultEndpoint`). Add no virtual driver
  for this session; a driver is out of scope.
- One microphone capture endpoint.
- Two different applications that play audio, for example a meeting app and a
  browser or media player. Use known test audio, or two people speaking.
- Normal FUNG profile untouched. Run only the UAT executable above.
- Windows → Settings → System → Sound → Volume mixer: set app A's output to
  endpoint 1 and app B's output to endpoint 2. FUNG never changes this routing.

## 3. Steps and expected results

Record a screenshot or JSON status for each step in the evidence root.

| # | Step | Expected |
| --- | --- | --- |
| 1 | Launch the installed `fung.exe`. Open Live, then the tab `อุปกรณ์เฉพาะต่อแอป`. | Inventory lists the apps and both endpoints. The PID tab shows `เลือกรายแอป (ทดลอง)` with its warning. |
| 2 | Add route 1 (app A → endpoint 1) and route 2 (app B → endpoint 2). Select the microphone. Validate. | Validation passes. Mic option default was `ไม่บันทึกไมโครโฟน` before selection. |
| 3 | Negative: choose the same endpoint for both routes, then the default endpoint. | Rejected with `DuplicateEndpoint`, then `DefaultEndpoint`. Nothing records. |
| 4 | Start. Play audio in A and B, and speak into the mic, for ≥ 2 minutes, with periods where only one source is active. | All three sources reach `พร้อมบันทึก`. Durable duration grows per source. |
| 5 | Stop. | Phase `completed`, not `partial`. A `transcript.retry` job is queued automatically. |
| 6 | Wait for the job. Open the recording in Review. | Transcript with three speakers: app A label, app B label, and `เรา` (mic). Each speaker's text matches what was audible only on that source (no cross-source bleed). |
| 7 | Export the recording. Close and reopen the app; reopen the recording. | Export has per-source tracks. Reopen shows the same sources, labels and transcript. |
| 8 | Contamination: during a new short recording, route a third app to endpoint 1 and play it. | Source 1 fails or is quarantined. The recording becomes `partial`. No transcription job is queued, and a manual retry fails with `routed_transcription_blocked`. No false PASS. |
| 9 | Device removal: during a short recording, unplug endpoint 2 (or the mic). | That source fails; recording `partial`; app stays responsive; no other device is substituted. |
| 10 | Mic-unavailable: select a mic, unplug it, then Validate or Start without refreshing. | Rejected with `MicrophoneUnavailable` (Thai message); the old choice shows `ไมโครโฟนเดิม — ไม่พร้อมใช้งาน` after refresh. |
| 11 | #94 retest (B-09): system-loopback delayed playback per [installed UAT](../verification/implementation-reports/2026-10-23-installed-releasefix-uat.md). | Recorded separately as #94 PASS or FAIL. |

Human-speech acceptance (step 6, mic) is a separate gate from synthetic
playback. State which kind each step used.

## 4. Pass rule

E5 passes when steps 1–10 match the expected results with evidence. A FAIL at
any step stops release. The lead files an issue and the fix goes through a
normal PR with a rebuilt candidate. Step 11 is reported against #94 and does
not by itself decide E5.

## 5. Recovery and rollback

- Uninstall with `installed\uninstall.exe` or delete the UAT folder. The UAT
  identity keeps its own app data, so the normal profile is not affected.
- Restore Volume mixer routing to Default after the session.
- Keep every failed recording and its evidence. Do not delete rows to retry.

## CHANGELOG

| Version | Date | Summary | Agent |
| --- | --- | --- | --- |
| 0.1.0b | 2026-11-08 | First E5 runbook with candidate identity from `e7608d0`. | Claude (lead) |
