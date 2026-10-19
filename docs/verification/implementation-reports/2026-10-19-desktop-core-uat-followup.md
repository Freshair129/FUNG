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

**Core packaged UAT passes for the tested capture, playback, Review, legacy
transcript, and normal restart flows.** The controlled system-loopback signal
was captured in contiguous chunks, the system player advanced and exposed
only the expected tail gap after the signal ended, and the same isolated
profile reopened with its project, recordings, and output files intact. The
packaged Meeting Intelligence panel now displays the legacy transcript as
read-only. Release acceptance remains open for signed packaging, Thai speech
quality, diarization, crash recovery, and provider/account integrations. The
NSIS installer itself was not used to update the UAT install; the newly built
release executable was placed in the existing isolated install directory.

## Environment and data boundary

- Repository branch: `codex/readiness-fixes-20261015`; source `HEAD` before
  the local UAT fix: `cb602095e6012dea53710beec0747843bae170d8`. The packaged
  executable includes the working-tree legacy-transcript fix. The local test
  changes also correct stale shell assertions: Output is reached through the
  utility action, and Review yields while Meeting Intelligence is open.
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
  1,538,829,406 bytes, SHA-256
  `D5DEC738DC41CF280E3DFC558923580BBD6284675F75EBA26C3D110E80DF2D15`.
  Authenticode status is `NotSigned`. The UAT executable is 33,427,456 bytes,
  SHA-256 `A8E6F9D5F645CAC27053BC438EB8170186045B6EC2A16DC72E715D5FEEAF4D2F`;
  it matches the release executable built from the working tree. The silent
  NSIS attempt did not update the existing directory, so only that executable
  was replaced in `installed-current`; previously unpacked resources remain
  from the isolated UAT install. This is executable UAT, not installer-upgrade
  acceptance.

## Core results

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Packaged startup and isolated profile | PASS | The newly built release executable opened under `tauri.localhost`; Home showed the existing UAT project. No normal FUNG profile was used. The app binary was refreshed in the isolated install directory; this did not exercise NSIS upgrade behavior. |
| Controlled system-loopback capture | PASS | A generated 440 Hz test tone played for 24 seconds. The latest 35.94-second recording has eight chunks total; three system chunks cover `[0, 8000]`, `[8000, 16000]`, and `[16000, 24000]` ms without gaps. No mic audio or live transcript content was inspected. |
| System-channel player | PASS — UI progress | The packaged player advanced past `0:05` and was paused at `0:11 / 0:35`. Its sole reported gap was `0:24–0:35`, after the 24-second test signal ended. This verifies app playback progress and timeline reporting, not independent acoustic-output measurement. |
| Synthetic import Review | PASS | The imported `0:06` recording opened in Review; its transcript tab displayed the existing synthetic segment. Transcript text is intentionally not reproduced here. |
| Imported-file player | PASS — prior smoke | The packaged player previously advanced to `0:06 / 0:06` and reported `ended`. This follow-up focused on the new system-track capture/player path. Neither smoke independently measured acoustic output. |
| Local Meeting Intelligence legacy transcript | PASS — UI | The newly built app displayed the same legacy snapshot at `cursor 0 · revision 0`, labeled `rev 0 · อ่านอย่างเดียว`. No correction control was offered in that panel; the native correction request also fails closed for this state. |
| Summary for the tone-only capture | NOT_QUALIFIED | The recording contains no spoken transcript; its summary job correctly reported that the session has no transcript. This does not assess speech transcription or summary quality. |
| Speaker diarization | NOT_QUALIFIED | Packaged runtime lacks `pyannote.audio`; diarization remains outside this core UAT. |
| Graceful close and profile integrity | PASS | The app process exited after normal close and reopened from the same install/profile. History showed the same project and all four recordings, including the latest 0:35 session. After shutdown, SQLite `integrity_check` returned `ok`; counts were 1 project, 4 recordings, 8 jobs, 6 transcript segments, 3 summaries, and 16 audio chunks. All 17 UAT output files matched their pre-close manifest. |

The profile has four completed and four failed jobs across all UAT attempts.
Failures include one earlier interrupted capture, two diarization jobs due to
missing runtime components, and the tone-only summary with no transcript. The
latest controlled capture completed. No transcript or audio body is included
in this evidence summary.

## Verification receipts

- Native packaged UI was opened, used, closed normally, and launched again
  from the same temporary install/profile. The reopened app returned to Home;
  History showed all four recordings, including the latest loopback session.
- The latest profile database passed SQLite `integrity_check` after shutdown;
  the output directory contained 17 files whose names, sizes, and SHA-256
  hashes matched the before-close manifest.
- `npm run test:callmd-history`: **11 passed, 0 failed**.
- `npm run test:callmd-integration`: **12 passed, 0 failed**.
- `npm run test:recording-output`: **3 passed, 0 failed**.
- `npm run test:native-session-custody`: **12 passed, 0 failed**, including
  the registered Rust behavioral matrix.
- `npm run test:meeting-intelligence-contract`: **17 passed, 0 failed**.
- `npm run test:job-actions`: **17 passed, 0 failed** after updating its
  stale Output-navigation and Review-visibility assertions.
- `npm run build` (`tsc` and Vite): passed; 1,820 modules transformed.
- SQLite read-only inspection: `integrity_check=ok`; project and recording
  counts matched before/after app restarts. No transcript text was read from
  the live mic/system capture for this check.

## Remaining release gates

1. Re-run installer upgrade acceptance with the actual NSIS package; the
   silent update attempt did not replace the isolated app executable.
2. Validate Thai speech transcription and summaries with a speech fixture;
   the loopback fixture in this pass was a tone without speech.
3. Install and validate the diarization runtime if speaker separation is in
   release scope.
4. Keep signed-release packaging, crash recovery, and provider/account flows
   as separate release gates.

Core capture/playback/review/restart acceptance is complete for this pass.
The release gates above remain open; no release tag was created.
