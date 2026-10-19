---
version: "1.0.0"
created_at: "2026-10-19,Codex"
last_update: "2026-10-19,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "FUNG packaged core Review, playback, meeting intelligence, and persistence UAT"
  language: "Thai-first"
---

# FUNG Desktop core UAT follow-up

## Outcome

**Packaged core UAT is partial; do not merge PR #80 or tag a release from this
result.** The isolated data-bearing profile survived normal shutdown, package
install, and reopen. The packaged app displayed the synthetic transcript and
played the imported file through its full 0:06 duration. System-channel
playback reported missing audio, and Local Meeting Intelligence failed to
load a transcript revision for a recording that has transcript segments.
Speaker diarization also remains unavailable because `pyannote.audio` is not
installed in the local runtime.

## Environment and data boundary

- Repository branch: `codex/readiness-fixes-20261015`; starting repository
  `HEAD`: `787e736e4313fd7eb0da768a7fc3ace3381f769f`. The packaged UI includes
  the working-tree change that makes Local Meeting Intelligence an explicit
  Review action and hides Review while that panel is open. The matching test
  assertion was updated after the first integration run exposed its old
  unconditional-visibility expectation.
- Built a local NSIS bundle with a temporary Tauri identifier,
  `dev.fung.uat.core20261019`, and installed it to
  `%TEMP%\fung-uat-pr80-core-20261019\installed-current`. The app data stayed
  under `%APPDATA%\dev.fung.uat.core20261019`; the regular `dev.fung.local`
  profile was not opened or modified.
- UAT output was isolated at
  `%TEMP%\fung-uat-pr80-core-20261019\output`. The import fixture was an
  offline generated Thai WAV (301,392 bytes; SHA-256
  `D52863A874B942E4B3EE453720B7771FC18CAA7F5721B83159CA16EF284719BF`). The
  separate live mic/system capture is not copied into this report; no audio
  or transcript text from that recording is retained here.
- Installer: `src-tauri/target/release/bundle/nsis/FUNG_0.1.1_x64-setup.exe`,
  1,538,836,389 bytes, SHA-256
  `090BCEFA5F71184D1E1BFC0DAF2AAFFE477D1C0D5046BC67A8DC8DBCFFA8250B`.
  Authenticode status is `NotSigned`; this is a local UAT package, not a
  release artifact.

## Core results

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Packaged startup and isolated profile | PASS | Installed app opened under `tauri.localhost`; Home showed the existing UAT project. No normal FUNG profile was used. |
| Data-bearing persistence | PASS | After graceful close/reopen, History showed the same project and three completed recordings. SQLite `integrity_check` returned `ok`; counts were 1 project, 3 recordings, 5 jobs, 6 transcript segments, and 3 summaries. |
| Synthetic import Review | PASS | The imported `0:06` recording reopened in Review after package restart. The transcript tab displayed one synthetic segment; the text is intentionally not reproduced here. |
| Imported-file player | PASS — UI progress | Packaged player opened paused at `0:00 / 0:06`, advanced, and reached `0:06 / 0:06` with `ended` state. This confirms the app player path and timeline, not independent acoustic-output measurement. |
| System-channel player | PARTIAL | Selected `ระบบ`, opened a `0:39` player, advanced to `0:07`, and paused at `0:14`. FUNG displayed its missing/unreadable audio warning throughout. Full system-track playback is not accepted. |
| Local Meeting Intelligence navigation | PASS — UI only | The explicit action opened MI in place of Review, and its close control returned to Review. |
| Local Meeting Intelligence transcript read | FAIL | MI displayed `The local meeting-intelligence action could not be completed` and `ยังไม่มี transcript revision สำหรับ recording นี้`, despite six persisted transcript segments and zero rows in `app_fung_mobile__transcript_revisions`. MI readiness also showed unavailable agent/provider features. |
| Speaker diarization | NOT_QUALIFIED | `speakers.diarize` failed because the local runtime lacks `pyannote.audio`; transcription and summary jobs completed. |
| Graceful close and profile integrity | PASS | The packaged process exited after normal close. The profile database passed `integrity_check`; project/recording/transcript/summary counts remained unchanged. Output files matched the pre-close manifest. SQLite/state hashes changed during normal checkpointing, and temporary WAL/SHM files disappeared after shutdown. |

Job totals after reopen were four completed and one failed. The completed jobs
were capture, summary, and two transcription jobs. The failed job was speaker
diarization; no transcript or audio body is included in the evidence summary.

## Verification receipts

- Native packaged UI was opened, used, closed normally, and launched again
  from the same temporary install/profile. The reopened app returned to Home;
  History and the synthetic Review transcript were then verified.
- `npm run test:callmd-history`: **11 passed, 0 failed**.
- `npm run test:callmd-integration`: first run exposed one stale assertion for
  unconditional Review visibility. The assertion now checks the explicit MI
  visibility condition; rerun: **12 passed, 0 failed**.
- `npm run test:recording-output`: **3 passed, 0 failed**.
- `npm run test:native-session-custody`: **12 passed, 0 failed**, including
  the registered Rust behavioral matrix.
- `npm run test:meeting-intelligence-contract`: **16 passed, 0 failed**.
- Focused suites total: **54 passed, 0 failed** after the assertion update.
- `npm run build` (`tsc` and Vite): passed; 1,820 modules transformed.
- SQLite read-only inspection: `integrity_check=ok`; project and recording
  counts matched before/after app restarts. No transcript text was read from
  the live mic/system capture for this check.

## Remaining core and release gates

1. Diagnose why the packaged system recording reports missing audio for part
   of its duration and repeat the playback acceptance with complete coverage.
2. Reconcile the transcript segment pipeline with the revision-based MI read
   contract, then verify transcript loading through the packaged UI.
3. Install and validate the diarization dependency if speaker separation is
   part of the release scope; rerun its job through the app.
4. Keep Thai recognition quality, signed-release packaging, provider/account
   flows, crash recovery, and any untested integrations as separate gates.

These results finish this UAT pass with a clear partial outcome. They do not
meet the core acceptance gate for merge or release; no merge or release tag
was performed.
