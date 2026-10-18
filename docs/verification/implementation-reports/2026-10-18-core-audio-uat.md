---
version: "1.0.0"
created_at: "2026-10-18,Codex"
last_update: "2026-10-18,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "FUNG local microphone, system loopback, STT, TTS, stop and playback UAT"
---

# FUNG core local audio UAT

## Outcome

**The local capture, stop/finalize, history playback, English TTS, and app-level
STT paths were exercised in the FUNG UI against an isolated profile. The audio
pipeline works, but Thai STT accuracy is partial and Thai TTS is not qualified.**
Do not treat this report as release acceptance for Thai speech quality,
speaker diarization, or connected providers.

The user's spoken mic test produced a non-silent microphone WAV, one Thai
transcript segment, a completed capture job, and a completed history entry.
The recognized wording differed from the requested phrase, so this proves
capture and transcription execution, not verbatim Thai recognition. A separate
live system-loopback session also produced transcript text. FUNG's configured
Windows SAPI English test provider generated and played a valid five-second WAV.

## Environment and data boundary

- Source branch: `codex/readiness-fixes-20261015`, starting source head
  `1fba180af0a05e1f2b4742a092641dfc442490c7`.
- Built/launched the Tauri debug app with isolated identifier
  `dev.fung.uat.audio20261018`; GenesisBlockDB profile:
  `%APPDATA%\dev.fung.uat.audio20261018\genesisdb`.
- Isolated project: `f95a027a-d440-4756-91ef-52bc060b454b`. Captured audio is
  scoped to that test project under `%USERPROFILE%\Documents\fung\projects`.
  No normal FUNG profile, project, or recording was opened or modified.
- The STT loopback fixture was generated offline for this test; no cloud
  provider, API key, account, external connection, or network upload was used.

## Core feature results

| Feature | Result | Evidence and limit |
|---|---|---|
| Microphone capture | PASS — signal captured | Live UI selected the default `Microphone (ONIKUMA M530)` with system loopback off and participant notice checked. Recording `d505553c-8b8e-4ef8-adc3-57b3577f7049` completed at 49,950 ms. It contains seven 48 kHz mono PCM WAV chunks; the spoken section in `mic-00003.wav` has peak 5,692 and RMS 513.4, unlike the silent chunks before/after speech. |
| Stop and finalize | PASS — isolated mic session | FUNG returned to “หยุดบันทึกแล้ว”; the recording row changed to `completed`, all seven chunks received transcription completion timestamps, and `recording.capture` reached `completed` at 100%. Finalization waited for local chunk transcription before releasing the session. |
| Thai STT from microphone | PARTIAL — execution works, wording not exact | The live UI and history each displayed one Thai segment from the user's spoken test. The recognizer substituted words relative to the prompted phrase; this is not a Thai accuracy pass. The user's voice content is intentionally not copied into this report. |
| System-loopback capture and STT | PASS — app-level local run | Recording `25ab41da-60ac-47e7-a45d-a62e4deabc5c` completed at 30,510 ms with separate mic and system tracks. The system track carried clear non-silent signal and the UI showed local transcript segments from the offline playback fixture. |
| History and playback | PASS — current mic recording | Refreshed History and found the new `0:49 · completed · mic` row. Opened its review, saw the transcript, selected the microphone channel, and played it: the native player changed to `playing` and the position advanced from the start to 0:08. |
| Local TTS | PASS — English provider test | Through Settings → TTS Providers, the FUNG test button reported `ok` in 1,238 ms and played output. The app-created WAV is 220,506 bytes, mono PCM16 at 22,050 Hz, 4.999 seconds, with peak 21,312 and 58,661 non-zero samples. TTS result `7f9678a9-0d26-4740-a3a9-adbea91658b3`. |
| Thai TTS | NOT_QUALIFIED | The tested Windows SAPI voice was English. Pattara was not available in the child voice process, so no Thai pronunciation claim is made. |
| Speaker diarization | NOT_QUALIFIED | The optional post-capture job reports `dependencies_missing`: `pyannote.audio` is not installed in this local runtime. This does not block capture, transcript, or playback, but speaker separation has not passed. |
| Interrupted-capture recovery | NOT_RUN | This pass tested an orderly stop and finalize. It did not crash/kill the app during a live recording or test recovery of an interrupted session. |

## Fixes in this change

- Resolve the bundled Whisper model path for the live child worker before
  launching it on Windows, matching the normalized path already used by the
  worker runtime.
- Give the UI stop poll up to 120 seconds for local finalization, including
  channel drain, WAV sealing, and catch-up transcription. The clean mic-only
  UAT completed inside that limit.
- Let a TTS provider test use editable text so the local provider can be tested
  with a known phrase.
- Validate generated TTS WAV format, duration, and non-silent samples before
  reporting success. The validator rejects empty or silent WAV output rather
  than accepting a header-only file.

## Verification receipts

- Native FUNG UI was used for mic-only capture, stop, history refresh, transcript
  review, and playback. No source edits or Vite hot reload occurred during the
  clean mic capture.
- Read-only GenesisBlockDB inspection confirmed capture status, seven chunk
  records, chunk transcription timestamps, one transcript segment, and the
  completed `recording.capture` job.
- Read-only WAV inspection confirmed actual signal in the user's spoken test
  chunk and non-empty TTS samples. The TTS path is under `%TEMP%\fung-tts`.
- The same isolated profile had previously been closed and reopened for
  persistence verification; this latest spoken recording was additionally
  found in History after refresh and opened for playback in the UI.
- `node --test tests/callmdLiveWorkspace.test.mjs`: **13 passed**.
- `npm run build`: passed.
- `cargo fmt --check`: passed.
- `cargo test --lib tts_executor::tests`: **6 passed**.
- `git diff --check`: passed after the code and report edits.

## Acceptance boundary

This closes a real local core-audio smoke pass for recording, session stop,
local STT execution, History review/playback, and English TTS. It does not
establish verbatim Thai STT quality, Thai TTS, diarization, every microphone
hardware configuration, interrupted-capture recovery, packaged-release
behavior, or connected provider acceptance. PR #80 remains Draft; no merge or
release tag is authorized by this report.
