---
version: "1.0.1"
created_at: "2026-10-20,Codex"
last_update: "2026-10-20,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "FUNG packaged live timer fix UAT"
  language: "Thai-first"
---

# FUNG Desktop live timer fix UAT

## Outcome

**Active-recording timer: PASS; stopping-state timer: FAIL on the later mic
sample; native retest of the source correction is pending. Overall
Desktop/core acceptance: NOT PASSED.** The earlier isolated package displayed
elapsed time advancing during an active microphone recording and retained its
15.83-second item through an orderly close and relaunch. A later spoken mic
sample in the participant-notice package exposed that the displayed timer kept
advancing while stop finalization was pending, beyond the saved 21.93-second
audio duration. A source correction freezes the timer on entry to stopping;
focused tests and the web build pass. A separate native package is built and
hash-verified but has not yet been installed or launched. It uses app id
`dev.fung.uat.timerfix20261020`; its unsigned installer SHA-256 is
`6B97092EF1BFE1889F1B7D5352EDC569788461CA79A1FE82FFA1658AD8DEB3C0`.
The user subsequently approved installation and launch. The candidate had not
been installed or runtime-tested as of this update, so no native retest result
is claimed.

The timer sample was near-silent and produced no transcript segments. It does
not qualify microphone intelligibility, Thai transcription, or summary quality.
Previous spoken-sample failures and independent package/release gates remain
open; see [the broader Desktop UAT follow-up](2026-10-19-desktop-core-uat-followup.md).

## Subsequent microphone capture and stop-state retest — 2026-10-20

This later test used the separately installed app id
`dev.fung.uat.autonotice20261020`; the normal `dev.fung.local` profile was not
opened. The selected endpoint was ONIKUMA M530 and system loopback was off.
The user confirmed speaking. Recording
`b9b4d9ff-c5cf-4a3e-a390-bce80b0e9d19` completed in the database at 21,930 ms.

| Check | Result | Evidence and limit |
| --- | --- | --- |
| Microphone signal | PASS for nonzero capture | Three contiguous chunks span 0–8,000, 8,000–16,000, and 16,000–21,930 ms. All contain nonzero PCM; combined RMS is 610.69. This verifies signal was saved, not intelligibility. |
| Chunk ledger and database | PASS | Chunk SHA-256 values and sizes match the database ledger; read-only SQLite `PRAGMA integrity_check` returned `ok`. |
| Stop timer | FAIL on this installed build | While the app still displayed “กำลังหยุดบันทึก”, the elapsed display advanced from `0:26` through `0:44` to `1:00`, although the saved audio duration was 21.93 seconds. This is a UI/finalization-state defect, not extra recorded audio. |
| Thai transcription | NOT QUALIFIED / FAILED | Four transcript segments appeared, but the Thai words and tonal contour were inaccurate. Raw transcript text is intentionally omitted. |
| History UI persistence | NOT VERIFIED | The database row is completed, but this run did not establish that the item could be reopened from History; the UI requested a project selection. |

The stop-state correction is present in `src/components/LiveMeetingPanel.tsx`
through `resolveFrozenElapsedMs`; the focused suite passed **16/16** and
`npm run build` passed. A native package containing that correction has not yet
been installed or run. The user must approve that app action before the
computer-use retest. Machine-readable details are in the external receipt
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\installed-package-notice-default-uat-20261020.json`.

## Build and isolated environment

- Repository: `G:\AI-Studio\development\FUNG`
- Branch: `codex/readiness-fixes-20261015`
- Source `HEAD`: `56c53544718989fd5429dd5071479262fab7c2ee`, plus the uncommitted
  working-tree changes present when the test build was created.
- Build command: `npm run tauri -- build --no-bundle --config '{"identifier":"dev.fung.uat.core20261019"}' --ci`
- App id: `dev.fung.uat.core20261019`
- UAT executable:
  `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-current\fung.exe`
- SHA-256: `6252B02B10B3FC5E2AA71BD590282CC3BA22C4A72876F7F75CB4822A5E2E0ACA`
- Isolated profile:
  `C:\Users\suanr\AppData\Roaming\dev.fung.uat.core20261019`
- Isolated recording output:
  `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\output`
- The normal `dev.fung.local` profile was not opened or modified.

The user checked the participant-notification acknowledgment in FUNG. The
capture source was `Microphone (ONIKUMA M530)`; system loopback was unchecked.
The language selector was left at automatic detection. Consent was not changed
by automation.

## Packaged UI retest

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Timer at start | PASS | The active Live page showed `0:00` immediately after the capture started. |
| Timer while recording | PASS | After a 4.2-second wait, the captured app screenshot showed `0:08`; the accessibility text from that same snapshot was stale at `0:00`. A later fresh accessibility snapshot showed `0:15` while the session was still active. The rendered UI evidence shows that elapsed time advances. |
| Stop and capture completion | PASS — capture; latency recorded | The UI entered its stopping state and returned to “หยุดบันทึกแล้ว” within the next 10-second observation. The capture job finished at `2026-10-19T19:20:13.336638500+00:00`. The recording began at `19:19:46.596765700+00:00` and lasted 15,830 ms, so job completion was about 10,910 ms after the audio endpoint. |
| Recording persistence | PASS — near-silent sample | Recording `98ca4fa2-1615-4121-846c-bcc621e10221` completed with two contiguous mono, 48 kHz PCM16 WAV chunks spanning `[0, 8000]` and `[8000, 15830]` ms. Both files existed, had the byte sizes recorded in the database, and matched their stored SHA-256 checksums. Signal was effectively silent; this run does not verify speech capture quality. |
| Transcript, summary, diarization | NOT QUALIFIED | No transcript segments were created. Summary and diarization jobs were marked failed for this near-silent sample. This is not a language-quality sample. |
| History refresh | PASS | The new `0:15` microphone item appeared at the top of History immediately after stop. |
| Orderly close/reopen | PASS | FUNG was closed through its app control and reopened with the same app id/profile. History displayed the same latest `0:15` recording. Read-only `PRAGMA integrity_check` returned `ok` after close. |
| Profile boundary | PASS | All actions used the isolated `dev.fung.uat.core20261019` profile. The ordinary FUNG profile remained untouched. |

The machine receipt is outside the repository at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\refreshed-build-ui-retest.json`
under `timer_fix_retest`. No raw microphone audio or transcript body was added
to the repository.

## Regression checks

| Check | Result |
| --- | --- |
| `npm run build` | PASS — TypeScript and Vite; 1,820 modules transformed |
| `npm run test:callmd-live` | PASS — 15 passed, 0 failed |
| `npm run test:callmd-shell` | PASS — 17 passed, 0 failed |
| `npm run test:callmd-history` | PASS — 13 passed, 0 failed |
| `npm run test:callmd-contracts` | PASS — 13 passed, 0 failed |
| `npm run tauri -- build --no-bundle --config '{"identifier":"dev.fung.uat.core20261019"}' --ci` | PASS — optimized Windows executable built |
| `git diff --check` | PASS before this report was added; rerun after documentation updates |

## Remaining acceptance gates

- Live timer rendering is verified on the packaged build. Stop finalization
  still varies between samples: about 10.91 seconds after the endpoint in this
  15.83-second test, about 14 seconds in the earlier 37.32-second test, and
  about 50.08 seconds in the earlier 54.66-second test. The latency and
  intermediate status copy still need an acceptance decision.
- Spoken Thai transcription and summary quality remain failed in earlier
  samples; this quiet timer run does not change that result.
- Installer upgrade, signed packaging, crash recovery, provider/account
  integrations, optional diarization, and additional viewport qualification
  were not established by this run.
- PR #80 is already merged. This UAT did not create a commit, push, tag, or
  release. Local working-tree changes remain uncommitted and unpushed.
