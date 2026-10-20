---
version: "1.0.15"
created_at: "2026-10-19,Codex"
last_update: "2026-10-20,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "FUNG packaged Desktop capture, review, recovery, and persistence UAT"
  language: "Thai-first"
---

# FUNG Desktop core UAT follow-up

## Outcome

**Overall Desktop/core acceptance remains NOT PASSED.** Earlier isolated UAT
verified interrupted-recording recovery and close/reopen persistence. A later
mic-only run now confirms nonzero ONIKUMA M530 signal was saved and its chunk
ledger and database integrity passed, but the 21.93-second sample produced
inaccurate Thai transcript segments. That run also exposed elapsed time
continuing to advance while stop finalization was pending. A source correction
and focused tests/build pass; packaged native retest is pending. History UI
reopen for this sample, Thai transcription/summary quality, diarization, signed
packaging, provider/account integrations and smaller-window/display-scaling
qualification remain open. The UAT installer is unsigned; no tag or release
was created.

## Follow-up 2026-10-20 — merged core acceptance

The UAT source was PR #80 head `56c53544718989fd5429dd5071479262fab7c2ee`.
Remote `main` is merge commit `b59bc3212442d9773ccdd4b6417ede3e3d74a613`;
both commits resolve to tree `a35239d094ad37c4f485ec7f0d32e666b60bcbf0`.
Post-merge GitHub Actions run `39290542777` completed successfully on that
merge commit, with both frontend and Rust jobs passing.

| Follow-up scenario | Result | Evidence and limit |
| --- | --- | --- |
| Packaged profile correction and restart | PASS | On the isolated app id `dev.fung.uat.core20261019`, the corrected transcript survived a normal close and relaunch from the same profile. History still showed the same project and four recordings. The ordinary `dev.fung.local` profile was not opened. |
| Packaged profile database integrity | PASS | After the second normal close, read-only SQLite `PRAGMA integrity_check` returned `ok` for the isolated profile projection, and the packaged `fung.exe` process was no longer running. |
| Packaged imported-file playback | PASS | The selected six-second synthetic WAV moved from `0:00` to `ended` at `0:06 / 0:06` in the packaged player. This confirms player progress and end handling, not independent acoustic-output measurement. |
| Packaged export artifacts | PASS — artifacts | After the export action initially returned `queued`, the isolated output contained newly written `audio-a273843e.wav`, `transcript-a273843e.srt`, and `transcript-a273843e.vtt` at 20 Oct 2026 00:19. The WAV was 301,392 bytes and its SHA-256 matched the imported fixture (`D52863A874B942E4B3EE453720B7771FC18CAA7F5721B83159CA16EF284719BF`). SRT SHA-256: `1ED74E028298F6C969AFDF6CA59A010244D482E7BE05680981158D68A5B5A44E`; VTT SHA-256: `B51FD4E313BE2E637AAE396D517C8BCBE09DD238E8BD4F9DE710F7A9D791459B`. The packaged panel did not expose a terminal job status; the production acceptance below verifies terminal status and export ledger directly. |
| Production loopback CPU transcription | PASS | `local_api::tests::upload_transcribes_with_the_real_runtime`: 1 passed, 0 failed; faster-whisper took 25.2 seconds and matched 12/12 configured English keywords. It used the generated 48 kHz fixture, SHA-256 `493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F`, Python 3.11.9, faster-whisper 1.2.1, CPU/int8, and pinned `large-v3-turbo` revision `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf`. `Cargo.lock` pins `genesis-block-native` 0.2.9 at `2cf485573d48d0e5e2ae76db647ec4c4dee543c3`. The production loopback HTTP import returned matching audio bytes; the test edited a segment, exported SRT/VTT/WAV, observed a completed job, then reopened the same disposable Genesis root and read back the job, recording, correction, export ledger, and files. It does not establish Thai recognition quality. |
| Thai speech and summary quality | FAIL — qualitative gate | In the packaged synthetic Thai import, the transcript required a manual correction; the reviewed microphone-session summary displayed malformed/raw content. This is a limited observed sample, not a quantified language benchmark, and does not meet a Thai-quality release claim. |
| Installer upgrade and signed release | NOT PASSED | The existing NSIS attempt did not update the isolated install. The recorded installer remains unsigned (`NotSigned`). No release tag was created for the PR #80 merge; remote tag `v0.1.1` points to the older `de7fd81d8995a47c22a6c865035b7e2384f69d9c`. |

The production run receipt and captured raw log are stored outside the source
repository at `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\production-loopback-acceptance.json` and
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\production-loopback-acceptance.log`.
Read-only post-run provenance and output hashes are in
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\production-loopback-acceptance-readback.json`.
The run used `CARGO_TARGET_DIR=G:\AI-Studio\development\FUNG\src-tauri\target`,
`--offline --locked`, `CARGO_BUILD_JOBS=1`, and did not touch Ollama or run GPU
inference. Its disposable acceptance store is
`C:\Users\suanr\AppData\Local\Temp\fung-acceptance-20261020-pr80core`.

After orderly shutdown, read-only queries of that same store returned
`integrity_check=ok`, the completed `transcript.transcribe` job and its
persisted import path, the `web-import` recording with a 14,483 ms duration,
two timed transcript segments (`0–14,000` ms), and the audio chunk's persisted
SHA-256 matching the fixture. The exported WAV hash also matches the fixture;
SRT and VTT hashes are recorded in the readback JSON. This provides persisted
source provenance and artifact hashes. It is an orderly shutdown/reopen test,
not crash-recovery evidence.

## Follow-up 2026-10-20 — live microphone and desktop page UAT

This pass used the exact PR #80 source commit
`56c53544718989fd5429dd5071479262fab7c2ee` and an isolated packaged build at
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-current\fung.exe`
(SHA-256 `70D6D2C28AC591E165AA435166D8849EE94B16194546B0AF68500AF4BD213666`).
It used app id `dev.fung.uat.core20261019`, project
`e93def68-ae36-4c01-8292-77688eeb9f18`, and recording
`81627c5b-d249-4e0f-9c2c-0e6e3878eb22`. The regular `dev.fung.local` profile was
not opened or modified. The machine evidence receipt is stored outside this
repository at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\live-microphone-desktop-uat.json`.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Real microphone capture and persistence | PASS — audio persisted | With participant-notification consent enabled, the user spoke the requested Thai test phrase. The completed recording is 33,290 ms and contains five contiguous mono 48 kHz PCM16 WAV chunks covering 0–33,290 ms. One chunk contains a strong signal, one a low-level signal, and three are silent. The capture job completed and two transcript segments persisted. This proves capture/persistence, not intelligible transcription. |
| Stop confirmation in Live | FAIL — UI state | After stop, the UI stayed on “กำลังยืนยันการหยุดบันทึก” / “กำลังหยุดบันทึก” while waiting for the Desktop inactive state, although the capture job and files had completed. Returning to Live cleared the stale pending panel and showed stopped state. This is an observed UI confirmation defect; the recording itself was not lost. |
| Thai microphone transcription | FAIL — quality | The two persisted segments did not accurately recognize the short spoken Thai phrase. The test was a short manual sample, not a language benchmark. Raw speech and transcript text are intentionally omitted from this repository. |
| Summary and diarization | FAIL / unavailable | The summary job completed, but its three-item output included a structured-parse warning/raw irrelevant content and did not provide a usable summary. Diarization failed because `pyannote.audio` is not installed in the packaged runtime. |
| Orderly close, reopen, and database integrity | PASS | The isolated app was closed and relaunched under the same app id/profile. Read-only SQLite `PRAGMA integrity_check` returned `ok`; the same recording remained completed with five chunks, two transcript segments, and completed capture/summary jobs. The normal FUNG profile was untouched. This was an orderly restart, not crash recovery. |
| Reopened microphone playback | PASS — UI progress | The persisted 33-second recording loaded in the Playback tab after restart and advanced to `0:02 / 0:33` before being paused. This proves load and timeline progress, not independent acoustic-output measurement or full-duration listening. |
| Home page visual fit | PASS — tested viewport | The home dashboard, shell, and navigation dock fit in the captured desktop viewport without outer-page scrolling or visible clipping. The empty next-meeting state was shown because there was no calendar data. |
| History/Review visual fit | PARTIAL — layout issue | Recording history and Review/player content each showed an internal vertical scrollbar at the tested viewport. The dock remained visible, but the player card's lower help content required scrolling. Full no-scroll layout acceptance therefore did not pass. |
| Desktop route/page coverage | PASS — navigation only where stated | Home, Live, History/Review/transcript/summary, Playback, Files, AI Companion, Settings, Storage, Appearance, Pair device, and More-menu routes were opened/read. AI Companion did not start recording on open. Settings showed account setup pending, no TTS provider, and no cloud credentials; local API was stopped, pairing required sign-in, and backup was unavailable while signed out. These gated screens were recorded as configuration states, not provider/integration acceptance. No credentials, OAuth, cloud, LAN, local API, import, backup, or appearance settings were changed. |

The chunk-level byte counts, time ranges, signal levels, SHA-256 hashes, job
states, restart check, and playback observation are in the external JSON
receipt. Audio and transcript bodies were not copied into the repository.

## Follow-up 2026-10-20 — refreshed packaged build UI retest

The refreshed executable was built from PR #80 source commit
`56c53544718989fd5429dd5071479262fab7c2ee` plus the current local working-tree
changes. GitHub currently reports PR #80 as already merged into `main` at
`b59bc3212442d9773ccdd4b6417ede3e3d74a613`; `origin/main` matches that commit.
The local UAT fixes remain uncommitted and unpushed. No release was created
for this run: GitHub lists v0.1.0 as the latest published release, while the
remote v0.1.1 tag still points to the older `de7fd81d8995a47c22a6c865035b7e2384f69d9c`.
It ran as app id `dev.fung.uat.core20261019` from the isolated install
path `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-current\fung.exe`
(SHA-256 `FA9466A6B8FB61F3121C0DDE37C7AB2F1072461D1EB22C65AFB0FED01B99766A`).
The normal `dev.fung.local` profile was not opened or modified.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| History layout at 1280×800 | PARTIAL — tested viewport | Six rows fit without an inner list scrollbar; with seven rows, the unbounded history list scrolls internally. The long project id remains on one line. No outer page overflow was visible. Other sizes/display scaling are not qualified. |
| Selected Review and player layout | PASS — tested viewport | The selected 0:22 recording details, playback controls, seek bar, and help text fit without a player/detail scrollbar. The long Summary/Ask view has an internal content scrollbar. |
| Playback on refreshed executable | PASS — UI progress/load | The 0:22 item advanced to `0:04 / 0:22` and paused at `0:11 / 0:22`. The new 0:54 item loaded paused at `0:00 / 0:54`; it was not played. This is UI evidence, not independent acoustic-output measurement or full-duration listening. |
| History refresh after leaving and returning | PASS — same-session UI | Immediately after stopping the new capture, returning from Live showed the 0:54 recording at the top without pressing Refresh. The same recording also appeared after an orderly close/reopen of the isolated profile. |
| Real microphone capture and storage | PASS — persistence; transcript quality FAIL | The refreshed build recorded 54,660 ms. Seven contiguous mono 48 kHz PCM16 chunks exist; all file sizes and SHA-256 hashes match the DB. Six chunks had non-zero signal. The capture job completed; raw audio and transcript text were not copied to the repository. |
| Stop completion and status copy | PARTIAL — long finalization | The recording completed without navigating away. The audio endpoint was 54.660 s after start; the capture job completed about 50.082 s later. UI timer froze at 0:54 and the strip stayed in generic stopping copy; the `transcribing` phase label did not appear. No permanent hang was observed. |
| Thai transcript, summary, and diarization | FAIL / unavailable | Two transcript segments appeared but were materially inaccurate; no text is retained here. Summary generation completed but output was structurally malformed and irrelevant. Diarization failed in the packaged runtime. |
| Orderly close/reopen and stored data | PASS | After closing the refreshed app, read-only SQLite `PRAGMA integrity_check` returned `ok`; recording `3e056555-1e6f-4534-9083-b84976976b3e` remained completed with seven chunks. Relaunching the same app id/profile and opening History showed the 0:54 record at top. This is orderly shutdown evidence, not crash recovery. |
| Build and focused regression checks | PASS | `npm run build` and `tauri build --no-bundle` completed. `test:callmd-history` 13/13, `test:callmd-live` 14/14, and `test:callmd-shell` 17/17 passed. |


The new 54.660-second microphone recording was created on the refreshed
executable after the user checked the participant-notification acknowledgment.
The UI showed two transcript segments, but audio and transcript bodies were not
copied into the repository. Chunk metadata, signal levels, job state, and hashes
are in the external machine receipt at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\refreshed-build-ui-retest.json`.
FUNG was closed normally, its isolated profile passed SQLite integrity check,
then the same app id/profile reopened and History displayed the latest item.

## Environment and data boundary

- Repository branch: `codex/readiness-fixes-20261015`; source `HEAD` before
  the local UAT fix: `cb602095e6012dea53710beec0747843bae170d8`. The packaged
  executable includes the working-tree legacy-transcript fix. The local test
  changes also correct stale shell assertions: Output is reached through the
  utility action, Review yields while Meeting Intelligence is open, and egress
  checks read production functions that follow embedded test modules.
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

At the 19 Oct profile snapshot, the database had four completed and four
failed jobs. Failures included one earlier interrupted capture, two
diarization jobs due to missing runtime components, and the tone-only summary
with no transcript. The latest controlled capture completed. No transcript or
audio body from the live mic/system capture is included in this evidence
summary.

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
- `npm run test:egress`: **8 passed, 0 failed** after fixing its source reader
  to inspect production code that follows embedded `cfg(test)` modules.
- `npm run build` (`tsc` and Vite): passed; 1,820 modules transformed.
- Historical GitHub Actions run `39287230186` for head `6eb6de2` failed only
  at `test:egress`; the helper had truncated `lib.rs` at its first embedded
  test module. The helper was corrected, and post-merge CI run `39290542777`
  passed on `b59bc3212442d9773ccdd4b6417ede3e3d74a613`, with frontend and Rust
  jobs successful.
- SQLite read-only inspection: `integrity_check=ok`; project and recording
  counts matched before/after app restarts. No transcript text was read from
  the live mic/system capture for this check.

## Remaining release gates before the latest live/recovery follow-up

1. The visible timer now passes the packaged live UAT. Stop finalization varied
   across runs: 10.91 seconds after the audio endpoint for the 15.83-second
   timer sample, about 14 seconds for the prior 37.32-second sample, and about
   50.08 seconds for the 54.66-second sample. Decide whether this latency and
   its intermediate status copy meet the desktop acceptance bar.
2. Improve and qualify Thai transcription and summary output with representative
   spoken samples. The refreshed-build sample failed qualitatively; it is not a
   quantified language benchmark.
3. The separate isolated NSIS 0.1.0-to-0.1.1 upgrade test passed and preserved
   all 484 profile files. The interrupted-recording recovery test below also
   passed once for already committed chunks; interruption during an individual
   file write or power loss remains untested. A duplicate recovery-scan audit
   event remains to be explained.
4. Signed packaging and provider/account flows remain separate release gates.
5. Install and validate diarization only if speaker separation is in scope.
6. Qualify additional window sizes/display scaling and decide whether the
   Summary/Ask panel's inner scroll meets the visual acceptance bar.

At the time of this earlier snapshot, overall desktop/core acceptance was
incomplete. The later follow-up below records successful capture/persistence
and one committed-chunk crash-recovery run. Thai transcription/summary quality,
variable stop-finalization latency, signing, provider/account integration and
remaining viewport gates still prevent acceptance. No release was created by
this UAT; PR #80 was already merged at
`b59bc3212442d9773ccdd4b6417ede3e3d74a613`; local UAT changes remain
uncommitted and unpushed.

## Follow-up 2026-10-20 — imported Review playback completion retest

A focused native UI retest used the isolated packaged app id
`dev.fung.uat.core20261019`, project
`e93def68-ae36-4c01-8292-77688eeb9f18`, and the selected six-second synthetic
import `A273843E-D8E4-4676-A2A7-D04B4AD3D8D6`. The app remained open in Review.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Imported recording playback timeline | PASS — UI progress and end state | The player started at `paused`, `0:00 / 0:06`. After selecting Play, a fresh native window snapshot showed `playing`, `0:02 / 0:06`; after natural completion it showed `ended`, `0:06 / 0:06`. This confirms playback state transitions and timeline completion, not independent acoustic-output measurement. |
| Executable identity | PASS — exact local binary match | `installed-current\fung.exe` is version `0.1.1`, SHA-256 `70A8B4081F6111F3887B7DEE99A61E03CB4EC201DD9E2A54F23795CD747C5E0B`; it matches `src-tauri\target\release\fung.exe`. This focused retest therefore applies to that executable, which differs from the earlier `70D6D2C2...` live-microphone retest binary. It does not qualify NSIS install or upgrade behavior. |
| PR and published release state | NOT RELEASED | GitHub reports PR #80 merged at `b59bc3212442d9773ccdd4b6417ede3e3d74a613`. The published release list still shows v0.1.0 as latest. Remote tag v0.1.1 still resolves to the older `de7fd81d8995a47c22a6c865035b7e2384f69d9c`; no release for PR #80 was created. |

The machine receipt is outside the repository at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\review-import-playback-retest.json`.
Overall Desktop/core acceptance remains **NOT PASSED**: Thai transcription and
summary quality, long stop-finalization behavior, installer upgrade, crash
recovery, signing, provider/account integrations, and remaining layout
qualification remain open. No release is authorized by this evidence.

## Follow-up 2026-10-20 — Summary and Ask layout retest

The native UAT build was rebuilt from PR #80 source commit
`56c53544718989fd5429dd5071479262fab7c2ee` plus the current local working-tree
changes. The Ask panel now uses compact context copy and a two-line initial
question field so its controls fit the available Review panel height. The build
command was `npm run tauri -- build --no-bundle --config
'{"identifier":"dev.fung.uat.core20261019"}' --ci`; `npm run build`, the
focused Review suite (15/15), and `git diff --check` passed.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Isolated packaged app and profile | PASS | Version `0.1.1` opened as `dev.fung.uat.core20261019` from `C:\\Users\\suanr\\AppData\\Local\\Temp\\fung-uat-pr80-core-20261019\\installed-current\\fung.exe`. The exact source and installed executable SHA-256 is `742E1D43BC82F4619740453C57B4A8B760379C13AD45E0F8BFCFC15DF43F005D`. The regular `dev.fung.local` profile was not opened. |
| Archive Review Summary panel at 1280×800 | PASS — tested viewport | The Summary sub-tab displayed its empty state within the panel; no panel scrollbar or clipped content was visible. |
| Archive Review Ask panel at 1280×800 | PASS — tested viewport | The selected recording context, privacy note, question label and field, `0/4,000` counter, and disabled submit button all fit in the visible card without an inner scrollbar. The question was not submitted. |
| Inner-tab keyboard navigation | PASS | Left and Right changed between Ask and Summary; Home selected Summary and End selected Ask. Follow-up accessibility snapshots showed one visible inner tab panel at a time. |
| Persistence and broader layout qualification | NOT RUN / OPEN | Relaunch returned to Home and History retained the existing project and synthetic imports. The selected recording had to be selected again. No test was run at other window sizes or display scaling. |
| Overall Desktop/core acceptance | NOT PASSED | Earlier Thai transcript/summary quality, stop-finalization latency, installer-upgrade, crash-recovery, signing, and provider/account gates remain open. This layout check does not qualify those behaviors or a release. |

The exact after-change native screenshot is retained outside the repository at
`G:\\AI-Studio\\verification\\2026-10-20-fung-pr80-core-uat\\summary-ask-layout-compact-1280x800.jpg`;
the before-change screenshot is
`G:\\AI-Studio\\verification\\2026-10-20-fung-pr80-core-uat\\summary-ask-layout-1280x800.jpg`.
The previous executable was preserved at
`C:\\Users\\suanr\\AppData\\Local\\Temp\\fung-uat-pr80-core-20261019\\fung-pre-ask-compact-uat-20261020.exe`
(SHA-256 `932B813E7CBEA08CA397DAC6C8C58781BC95DA8F354DF91948B513D367D467A6`).
The machine receipt is
`G:\\AI-Studio\\verification\\2026-10-20-fung-pr80-core-uat\\review-summary-ask-layout-uat.json`.
Source changes and evidence remain local and uncommitted; no release was
created.

## Follow-up 2026-10-20 — installed package desktop page sweep

The page sweep used the isolated NSIS-upgraded `0.1.1` package at
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-nsis-20261020\fung.exe`
(SHA-256
`FD7891FDE9E0F75ADC0396FC1E8807A6999D8F9804B13047B356F73B9C29B43C`), app id
`dev.fung.uat.core20261019`, and a 1280×800 window. The normal
`dev.fung.local` profile was not opened. Screenshots and the machine receipt
are stored under
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat`.

| Surface | Result | Evidence and limit |
| --- | --- | --- |
| Home | PASS — tested viewport | The dashboard, next-meeting empty state, local project summary and empty AI task pool fit at 1280×800. No outer-page scrolling or clipping was visible. Calendar data and extracted tasks were absent, so populated states were not qualified. |
| Live capture gate | WAITING FOR USER | `Microphone (ONIKUMA M530)` was selected and system loopback was off. The per-session participant-notification acknowledgment remained unchecked, the Start button stayed disabled, and no recording was started. |
| Live transcript and data tabs | PASS — empty-state navigation | Transcript showed zero confirmed items. The data/question tab showed no active recording (`RECORDING_NOT_FOUND`) and disabled its actions. Both tabs fit in the tested viewport; no prompt was submitted. |
| History and Review | PARTIAL — internal scrolling visible | The selected synthetic import reopened with its one-segment transcript. The left history list and Files/Tools details panel each showed an internal vertical scrollbar while the outer shell and dock remained visible. The Files/Tools view correctly labels exports as project-scoped and does not imply every file belongs to the selected recording. |
| Assistant and project menu | PASS — navigation only | The Companion panel opened without starting capture. The project popover showed the existing `Session 1` project and selected-recording context. No project was changed. |
| Settings | PARTIAL — integrations unconfigured | The right-side settings panel and its categories opened. Supabase auth was not configured; no TTS provider or cloud API key was present; link import remained unapproved; Zoom was disconnected; the local API was stopped and LAN sharing remained off. The account/backup view required sign-in and had no backup destination. No credentials or settings were changed. |
| Storage location | PASS — isolated destination | The page showed the isolated output path `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\output` as ready to write. No path was changed. |
| Appearance | PASS — read-only | The page showed Light theme, Glass material and Full transparency. Values were left unchanged. The Home hero remains blue/purple while Live, Review and Settings use pale neutral surfaces; this sweep records that visual difference but does not claim a design-system rework. |
| Device pairing | BLOCKED — sign-in required | The pairing dialog stated that sign-in is required. FUNGWIRE remained disabled with zero connected devices and zero jobs; no network service was enabled. |
| File import and export routes | PARTIAL — route only | Import opened the Windows audio/video picker at `Documents\\fung`, then was canceled without selecting a file. Export routed to Review and displayed its selection instructions; no additional export was created. Existing imported-file playback was qualified in the prior retest above. |

The external receipt `nsis-upgrade-uat.json` now records these page results.
The captured Home, History/Files and Live screenshots are
`installed-nsis-page-sweep-home-1280x800-1.jpg`,
`installed-nsis-page-sweep-history-files-1280x800-1.jpg` and
`installed-nsis-page-sweep-live-1280x800-1.jpg` respectively. No audio was
recorded in this sweep. Overall Desktop/core acceptance remains **NOT PASSED**:
Thai transcription/summary quality, stop-finalization latency, crash recovery,
signed packaging, provider/account integration, populated page states and
smaller-window/non-default-scaling qualification remain open. Crash recovery
still requires the user to check the per-session acknowledgment in the
isolated Live screen.

## Release and signing preflight recheck — 2026-10-20

The GitHub API currently reports PR #80 closed and merged at
`b59bc3212442d9773ccdd4b6417ede3e3d74a613`; `main` resolves to the same commit.
The latest published release is `v0.1.0`. The existing remote `v0.1.1` tag
resolves to the older commit `de7fd81d8995a47c22a6c865035b7e2384f69d9c`, not
the PR #80 merge. The tested local `0.1.1` NSIS package remains `NotSigned`.
Read-only checks of the CurrentUser and LocalMachine personal certificate
stores found no certificate with a private key and Code Signing EKU; `signtool`
is not available on PATH. No new tag or release was created.

The Windows system-menu `Size` action was disabled, and a drag from the
bottom-right window border did not change the 1280×800 viewport. Smaller-window
and alternate-scaling layout qualification therefore remains open.

## Responsive evidence boundary — 2026-10-20

The current layout authority is
`docs/design/2026-10-16-desktop-navigation-and-flows.md`, whose Responsive and
accessibility contract requires the shell and dock to stay within the viewport,
allows scrolling inside bounded readers, and calls for requalification at
narrow and short viewports. It does not prescribe exact numeric viewport sizes.
The explicit 1280×800, 1024×768, 768×1024 and 390×844 matrix in the
2026-09-19 shell-refresh document is historical because that document's shell
layout was superseded by the current navigation and flows contract.

The packaged native window is configured at 1280×800 with a 1200×780 minimum
and `resizable: false` in `src-tauri/tauri.conf.json`. The installed-package
evidence in this report therefore qualifies only the tested native sizes; the
page sweep did not run a separate browser viewport matrix. Narrow/short browser
layout and non-default display scaling remain unqualified, and browser
rendering cannot substitute for native capture or persistence evidence.

## Follow-up 2026-10-20 — isolated NSIS upgrade and Review retest

The NSIS acceptance gate was re-run with two locally built installers that use
the same isolated app id, `dev.fung.uat.core20261019`, and the same install
directory. The baseline `0.1.0` installer was built and installed first; the
candidate `0.1.1` installer then upgraded that installation with `/S`. Both
installers were unsigned. The candidate package was built from commit
`56c53544718989fd5429dd5071479262fab7c2ee` plus local working-tree changes,
including the compact Summary/Ask layout.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Baseline NSIS install | PASS | `FUNG_0.1.0_x64-setup.exe` SHA-256 `EEBBAD8D9C11EFD53C30D92279A361038B4FEA475B15CA420930A7936F71D220`; silent install exit code `0`; registry recorded version `0.1.0` at the isolated target. |
| Candidate NSIS upgrade | PASS | `FUNG_0.1.1_x64-setup.exe` SHA-256 `6FC3AB94AAC46BB4A92A406A07B5EDFD84DF244F744C049077A0D3B29BE65BCF`; silent upgrade exit code `0`; target executable and uninstall registry both report `0.1.1`. The installer Authenticode status is `NotSigned`. |
| UAT profile preservation | PASS | Before upgrade, copied `%APPDATA%\dev.fung.uat.core20261019` and `%LOCALAPPDATA%\dev.fung.uat.core20261019` to an external verification folder and hashed every file. All 484 files (118,141,930 bytes) matched their pre-upgrade hashes immediately after install; no file was missing or changed. The regular `dev.fung.local` profile was not opened. |
| Installed executable identity | PASS with packaging marker | Installed `fung.exe` is `0.1.1`, 33,431,040 bytes, SHA-256 `FD7891FDE9E0F75ADC0396FC1E8807A6999D8F9804B13047B356F73B9C29B43C`. It differs from the build output in exactly three bytes at offset 27,675,608: the embedded bundle marker changes from `BUNDLE_TYPE_VAR_UNK` to `BUNDLE_TYPE_VAR_NSS` for NSIS. The installed binary has the expected version and marker; this is not a byte-for-byte source executable match. |
| Existing Review data after upgrade | PASS | The isolated app reopened project `e93def68-ae36-4c01-8292-77688eeb9f18` and its existing synthetic import `A273843E-D8E4-4676-A2A7-D04B4AD3D8D6`. Review loaded its one-segment Thai transcript. Playback moved from `paused`, `0:00 / 0:06`, through `playing`, `0:04 / 0:06`, to `ended`, `0:06 / 0:06`. This confirms UI playback state and timeline, not independent acoustic-output measurement. |
| Summary/Ask layout in installed package | PASS — 1280×800 and maximized 1920×1032 | Both sub-tabs fit within the visible Review card without an inner scrollbar or clipped controls. Left/Right and Home/End switched between the two sub-tabs. The question was not entered or submitted. |
| Window size and display scaling | PARTIAL | The Windows system-menu `Size` command was disabled in the restored window; maximization and restoration worked. Smaller custom window sizes and non-default display scaling could not be qualified. |
| Crash-recovery UAT | WAITING FOR USER | Live showed the per-session participant-notification acknowledgment unchecked. No recording or forced termination was started; the isolated test needs the user to check that acknowledgment in the app. |
| Overall Desktop/core acceptance | NOT PASSED | Thai transcription/summary quality, stop-finalization latency, crash recovery, signed packaging, provider/account integrations, and smaller-window/non-default-scaling qualification remain open. This local unsigned package test does not authorize a release. |

The pre-upgrade profile copy and SHA manifest are retained outside the
repository at `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\profile-backup-pre-nsis-upgrade`
and `profile-pre-nsis-upgrade-manifest.json`. The package receipt is
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\nsis-upgrade-uat.json`.
Source changes and evidence remain local and uncommitted; no release was
created.

## Pre-live-UAT automated regression and responsive evidence snapshot — 2026-10-20

The current local working tree passed the focused Desktop regression suites:

| Command | Result |
| --- | --- |
| `npm run test:callmd-shell` | PASS — 17/17 |
| `npm run test:callmd-live` | PASS — 15/15 |
| `npm run test:callmd-history` | PASS — 15/15 |
| `npm run test:callmd-integration` | PASS — 12/12 |
| `npm run test:recovery` | PASS — 7/7 |
| `npm run test:callmd-contracts` | PASS — 13/13 |
| `npm run test:live-capture-routing` | PASS — 4/4 |
| `npm run test:recording-output` | PASS — 3/3 |
| `npm run test:design-system` | PASS — 5/5, including stale-artifact detection |
| `npm run design-system:check` | PASS — 202 tokens, 58 contrast pairs, 25/25 stylesheets |
| `npm run build` | PASS — 1,820 modules transformed |
| `git diff --check` | PASS |

These are source and build checks on a working tree with local changes; they do
not qualify the currently installed NSIS executable or close live audio UAT
gates. The design-system publishing test intentionally logs a stale generated
HTML failure while verifying that the checker detects stale output; the direct
`design-system:check` command passed after the artifacts were rendered.

A supplemental browser viewport attempt requested 1280×800, 1024×768,
768×1024 and 390×844. The 390×844 capture reported an actual CSS viewport of
500×749, so the screenshot matrix is invalid as viewport evidence and is not
counted as either a pass or a failure. The responsive matrix remains **NOT
RUN / UNQUALIFIED**. The current native window is fixed at 1280×800 with a
1200×780 minimum and cannot be resized through the tested Windows controls.

The isolated installed NSIS app remains at Live with its per-session
participant-notification acknowledgment unchecked and Start disabled. No
recording was started in this recheck. Crash-recovery UAT remains waiting for
the user to make that acknowledgment in the app; the earlier mic-only sample
was near-silent and does not qualify speech or crash recovery. Overall
Desktop/core acceptance remains **NOT PASSED**, with no tag or release created.
## Pre-live-UAT native package and release-state snapshot — 2026-10-20

A fresh read-only Computer Use snapshot was taken from the running isolated
NSIS app (`dev.fung.uat.core20261019`). The target window is backed by
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-nsis-20261020\fung.exe`, version `0.1.1`, SHA-256
`FD7891FDE9E0F75ADC0396FC1E8807A6999D8F9804B13047B356F73B9C29B43C`.
`Get-AuthenticodeSignature` reports `NotSigned`. Live shows ONIKUMA M530 as
the microphone, system loopback unchecked, participant-notification
acknowledgment unchecked, and Start disabled. No recording was started.

At this snapshot, live UAT remained **WAITING FOR USER** because the per-session
participant-notification acknowledgment was unchecked. The later native UAT
below records the subsequent user confirmation and test results.

Remote GitHub API reads confirm PR #80 is already merged at
`b59bc3212442d9773ccdd4b6417ede3e3d74a613`, matching `origin/main`. The latest
published release remains `v0.1.0`; existing tag `v0.1.1` still dereferences
to the older `de7fd81d8995a47c22a6c865035b7e2384f69d9c`. A fresh local
certificate-store scan found no code-signing certificate with a private key,
and `signtool.exe` is unavailable. No new tag or release was created.

## Follow-up 2026-10-20 — spoken microphone and crash recovery

The user requested a real microphone test and then an interrupted-recording
recovery test on the isolated installed package. The app id was
`dev.fung.uat.core20261019`; the executable was
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-nsis-20261020\fung.exe`
(version `0.1.1`, SHA-256
`FD7891FDE9E0F75ADC0396FC1E8807A6999D8F9804B13047B356F73B9C29B43C`,
Authenticode `NotSigned`). The ordinary `dev.fung.local` profile was not
opened. The machine receipt is outside this repository at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\live-mic-crash-recovery-uat.json`.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Real microphone capture and persistence | PASS — capture/persistence | With the user-confirmed participant notice, ONIKUMA M530 selected and system loopback off, recording `f8663395-7e09-40b4-a3ee-b5510ad14a44` completed at 41,940 ms. Six contiguous mono 48 kHz PCM16 WAV chunks covered 0–41,940 ms; chunk sizes and SHA-256 values matched the database ledger. SQLite `PRAGMA integrity_check` returned `ok`. This proves real microphone bytes were captured and persisted, not intelligible recognition. |
| Spoken Thai recognition | FAIL — quality | The user confirmed speaking the requested Thai test phrase during capture. Three transcript segments persisted, but recognition was materially inaccurate. Raw speech and transcript text are intentionally omitted. |
| Interrupted microphone recording detection and recovery | PASS — one isolated run | Recording `6fc0995b-bfe5-499f-bec3-74e6867504b0` was deliberately interrupted after five committed 8-second chunks (40,000 ms total). After restarting the same isolated app/profile, the project drawer showed one interrupted recording and five ledger chunks. The user recovery action completed; History then showed a completed 0:40 microphone item. |
| Recovered data and database integrity | PASS | Read-only SQLite `PRAGMA integrity_check` returned `ok`; all five existing WAV chunks remained present and matched ledger byte counts and SHA-256. Recovery reported zero missing files, orphan files, unreadable files, or newly adopted chunks/bytes. No nonterminal jobs remained after startup recovery. |
| Pre-test profile backup | PASS — hashes verified | The isolated app was cleanly closed before backup to checkpoint the database. The backup and `backup-manifest.json` are under `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\profile-backup-pre-crash-20261020-085116`. The manifest verifies 16 Roaming files (8,521,462 bytes), 480 Local files (109,670,689 bytes), and 45 output files (26,859,631 bytes). |
| Recovery audit event | SOURCE FIX PASS; rebuilt-package runtime retest open | Packaged `0.1.1` emitted two `recovery.interrupted_recording_found` events about 0.28 seconds apart. Source review found startup `scan()` and the UI refresh command were both writing the event. The working tree now uses side-effect-free `inspect()` for UI refresh and an idempotent startup audit that recognizes legacy random-ID events. Rust recovery tests pass 9/9. A new NSIS candidate was rebuilt from the fix but was not installed or runtime-tested; the two historical audit rows were left unchanged. |
| Codex in-app browser route smoke | PASS — browser navigation; native actions unavailable | With no native window control, Codex IAB navigated Home, Live (all three tabs), History, project drawer, More menu, Settings and all seven Settings tabs. History/Live report `NATIVE_UNAVAILABLE`; the project drawer has no browser-backed project list. The Settings account/backup tab displays `Cannot read properties of undefined (reading 'invoke')` in this browser preview; backup correctly says it requires Desktop. No credentials or cloud settings were entered. At the observed 1005×868 viewport, Live Insights has vertical content scrolling while the shell dock remains visible. Browser navigation does not access the installed profile, persisted recordings, or native microphone and is not native UAT. No browser microphone permission was requested. |

## Supplemental diagnostics — 2026-10-20

The packaged microphone chunks 2 and 3 were re-transcribed offline with the
packaged `large-v3-turbo` runtime on CPU/int8 using two threads. Automatic
language detection and forced Thai both detected Thai and produced identical
results per chunk: 84 characters at 0.089 character similarity for chunk 2,
and 47 characters at 0.162 similarity for chunk 3, measured against the
requested test phrase. Transcript text is omitted. This small diagnostic shows
that automatic language detection alone does not explain the poor result; it is
not a Thai accuracy benchmark.

The recovery audit repair is based on working tree
`56c53544718989fd5429dd5071479262fab7c2ee` plus local changes.
`cargo fmt --all -- --check` passed and
`cargo test --locked recovery::tests -- --nocapture` passed 9/9 tests,
including read-only repeated UI inspection, idempotent startup scans, and
deduplication against a legacy random-ID audit. The updated tree has since
been rebuilt into an NSIS installer, but that installer has not been installed
or runtime-retested. No native window was controlled after the user requested
browser-only testing.

The microphone capture finalized in about 10 seconds after stop in this run;
earlier packaged runs took longer, so stop-finalization timing remains
unqualified. Crash recovery passed only for the tested already-committed chunks;
this run did not test power loss during an individual file write. Overall
acceptance remains **NOT PASSED** because Thai transcription/summary quality,
stop-finalization consistency, packaged diarization, packaged verification of
the recovery-audit repair, signed release artifacts, provider/account
integrations and smaller-window/display-scaling qualification remain open. No
commit, push, tag or release was created for this UAT.

## NSIS rebuild after recovery-audit fix — 2026-10-20

`npm run tauri -- build --bundles nsis` completed successfully and produced
`src-tauri/target/release/bundle/nsis/FUNG_0.1.1_x64-setup.exe` from the local
working tree, including the recovery-audit fix. The installer is 1,538,685,741
bytes with SHA-256
`46E8A77DC01EF6BCAB7E0B8144BD217C4DBE9CBC0541BFE3D54FA20006E71E60`;
Authenticode reports `NotSigned`. The prior installer was preserved at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\FUNG_0.1.1_x64-setup.pre-audit-fix.exe`
with SHA-256
`6FC3AB94AAC46BB4A92A406A07B5EDFD84DF244F744C049077A0D3B29BE65BCF`.

This is a successful package build only. The new installer was not launched or
installed, and the installed-package recovery-audit behavior remains unverified
because native-window control was stopped in favor of Codex IAB testing. The
package remains unsigned; no commit, push, tag, or release was created.

## Same-capture Thai ASR follow-up — 2026-10-20

The existing 41.94-second ONIKUMA microphone capture was reprocessed offline
with the packaged `faster-whisper` 1.2.1 `large-v3-turbo` model on CPU/int8,
using two threads, forced Thai and the production VAD setting. No raw transcript
text was saved or reported. Chunk 2 had RMS `-25.5 dBFS` and peak `-4.9 dBFS`;
chunk 3 had RMS `-38.0 dBFS` and peak `-16.1 dBFS`. This confirms an audible
level in the main speech chunk, while most other chunks were near silence.

Transcribing all six chunks independently and joining the text scored `0.065`
similarity to the requested phrase. Concatenating all six decoded chunks and
transcribing the 41.94-second waveform as one input scored `0.110` (125 output
characters); this small difference does not show that chunk boundaries are the
main cause. A generic FUNG vocabulary hint raised the same full-capture score
from `0.110` to `0.132`, still far below a usable result. These are single-sample
diagnostics using normalized `SequenceMatcher` character similarity against
Thai-only and FUNG-included phrase variants, not an accuracy benchmark.

A VAD sweep on chunks 2 and 3 at thresholds `0.50`, `0.65` and `0.75` changed
their similarity only from `0.093` to `0.094` and `0.171` to `0.174`,
respectively. All emitted segments had `no_speech_prob=0.0`. This does not
support VAD thresholding as the remedy for this sample.

The installed package and shared Whisper model cache expose only
`large-v3-turbo`; no alternate checkpoint was present in the inspected local
locations, and none was downloaded. The Thai recognition gate therefore remains
**FAIL / OPEN**. The report and external receipt contain metrics only, not the
spoken transcript.


## Qwen3-ASR Thai candidate pilot — 2026-10-20

The isolated pinned Qwen3-ASR and Thai CTC models were staged and both
checkpoint hashes verified. The candidate pipeline completed on original
microphone chunks 2–3 with 29 aligned segments and 0.121 similarity. On all
six chunks, ASR-only scored 0.097; the full worker failed closed because the
CTC vocabulary cannot represent U+0E46. Thai quality and alignment remain
FAIL / OPEN; Detailed routing remains disabled. A subsequent full-waveform
comparison reached 0.125 raw, 0.132 with `afftdn`, and 0.130 with
`speechnorm`; these are still low single-sample scores. This does not qualify
Thai recognition or Desktop acceptance. See the
[dated candidate pilot report](2026-10-20-qwen-thai-asr-candidate-uat.md)
and machine receipt at
G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\qwen-thai-asr-candidate-uat.json.

## Package identity and live release-state recheck — 2026-10-20

The current NSIS candidate remains at
`src-tauri/target/release/bundle/nsis/FUNG_0.1.1_x64-setup.exe` with the
previously reported SHA-256
`46E8A77DC01EF6BCAB7E0B8144BD217C4DBE9CBC0541BFE3D54FA20006E71E60` and
Authenticode status `NotSigned`. The current release executable in the same
target tree is configured with the default app id `dev.fung.local`; that
user-data directory exists. The isolated NSIS test installation still has
app id `dev.fung.uat.core20261019` and executable hash
`FD7891FDE9E0F75ADC0396FC1E8807A6999D8F9804B13047B356F73B9C29B43C`, so it
does not contain the recovery-audit fix build.

To prepare a profile-safe retest, the same working tree was rebuilt with the
existing UAT identifier config using
`npm run tauri -- build --bundles nsis --config
C:/Users/suanr/AppData/Local/Temp/fung-uat-pr80-core-20261019/tauri-uat.config.json
--ci`. The isolated UAT installer is
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\FUNG_0.1.1_x64-setup.uat-id-20261020.exe`
(1,538,764,629 bytes, SHA-256
`B1C5C7AB90BBAE0D2534EBFD14687C9DC8BC8C42E9A1BDF899C8691F5ED2E45A`,
Authenticode `NotSigned`). Its companion executable SHA-256 is
`BE85453882B1082CAE544E7070D81DA3E759D1D78D5281CDC35674C6402A8830`, and a
binary identity scan confirmed `dev.fung.uat.core20261019` is embedded.

The UAT package has not been installed or launched. The default release
installer and executable were restored after the build and their original
hashes reverified (`46E8A77DC01EF6BCAB7E0B8144BD217C4DBE9CBC0541BFE3D54FA20006E71E60`
and `A83EC7D36ABC6E824523CD5F210C5EAC244CE9F71427765E8D09E09536D40519`). Recovery-audit
behavior in the rebuilt package remains **NOT RUNTIME-VERIFIED**; source tests
and a successful build do not close that gate. The remaining test requires
installing and observing startup/recovery in the isolated native app. Native
window testing remains paused in favor of Codex IAB at the user's request.

Post-build checks on the same working tree passed: `npm run test:recovery`
(7/7), `npm run test:release` (11/11), and
`cargo test --locked recovery::tests -- --nocapture` (9/9). Tauri's build also
ran the frontend build successfully (1,820 modules transformed) and compiled
the optimized Windows release executable. These checks validate source and
package construction, not installed-package recovery behavior.

Live GitHub reads on this date confirm PR #80 is already merged at
`b59bc3212442d9773ccdd4b6417ede3e3d74a613`; the latest published release is
still `v0.1.0`. No new merge, tag, or release was performed. Overall
Desktop/core acceptance remains **NOT PASSED**.

## Installed UAT-ID NSIS package first-launch retest — 2026-10-20

After the user confirmed the desktop was free, the isolated installer
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\FUNG_0.1.1_x64-setup.uat-id-20261020.exe`
was installed over the test-only directory
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-nsis-20261020`.
The installer SHA-256 was
`B1C5C7AB90BBAE0D2534EBFD14687C9DC8BC8C42E9A1BDF899C8691F5ED2E45A` and its
Authenticode status is `NotSigned`. The application was then launched with
`APPDATA` and `LOCALAPPDATA` redirected to the separate test root
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\package-recovery-retest-uat-20261020`.
The ordinary `dev.fung.local` profile was not opened.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Isolated package startup | PASS — opened | FUNG opened from the isolated install directory and Home showed the copied UAT project. The native window remained responsive while Home, History, and Live were inspected. |
| Existing history and review | PASS — native playback UI state | History loaded 11 completed recordings (9 microphone and 2 imported files). Selecting the six-second import opened Review; its native player moved from `paused` at `0:00 / 0:06` to `playing`, then `ended` at `0:06 / 0:06`. The player was closed afterward. This verifies the packaged playback state and timeline; acoustic output was not independently captured. |
| Review transcript, summary, and Ask | PASS — read-only packaged views | The selected six-second import showed one transcript segment at `0:00` with no speaker label; no edit was made. Summary showed no linked summary. Ask showed an empty `0 / 4,000` field and stated it uses only the selected transcript and does not call another provider automatically. No question was entered or submitted. |
| Live workspace tabs and devices | PASS — setup only | Capture, Transcript, and Insights tabs switched in the packaged app. The ONIKUMA M530 microphone was available and system loopback was off. Transcript showed zero items; Insights correctly had no active recording and reported `RECORDING_NOT_FOUND`. |
| Participant notice and live capture | NOT RUN — user action pending | The participant-notice acknowledgment was unchecked and Start was disabled. The user must check it in the app before capture can begin. No microphone capture was started in this retest. |
| Window size and scaling | NOT QUALIFIED — fixed native size | `src-tauri/tauri.conf.json` sets `1280 × 800`, minimum `1200 × 780`, and `resizable: false`; the disabled native Size action and ineffective border drag match that configuration. The captured window was `1280 × 800`. The current navigation contract still requires the shell to fit its viewport and requalification at narrow/short viewports; this packaged run did not change the viewport or Windows display scaling. |
| Pre-capture safety backup | PASS — isolated profile/output copied | Before any new recording, the idle UAT package profile and output were copied to `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\package-retest-pre-crash-backup-20261020-052458`. All 75 copied files across roaming/local/output were SHA-256 verified; the local directory was empty. |
| Database and preserved audit history | PASS — startup/read-only readback | Read-only SQLite `PRAGMA integrity_check` returned `ok`. The copied package profile still had 11 completed recordings (9 microphone, 2 imports), 49 audio chunks, two historical `recovery.interrupted_recording_found` rows, and one `recovery.recording_recovered` row. The original isolated source profile reported the same integrity and recovery-event counts; no recording or audit event was added while the Live consent gate remained closed. |
| Installed executable identity | PASS — expected NSIS marker transformation | The installed `fung.exe` was version `0.1.1`, size 33,430,528 bytes, SHA-256 `9D13145513BB904A0AD22508D2EC246B82126DE913DA56812CA8230837E9362A`, and `NotSigned`. It contains the expected `dev.fung.uat.core20261019` app id and `BUNDLE_TYPE_VAR_NSS` marker. In-memory only, changing the three marker bytes at offset 27,674,520 from `NSS` back to `UNK` produced SHA-256 `BE85453882B1082CAE544E7070D81DA3E759D1D78D5281CDC35674C6402A8830`, exactly matching the recorded companion build hash. No executable on disk was modified. |

The actual interrupted-recording recovery action has not been repeated in this
installed-package session. A follow-up in-memory comparison replaced only the
three NSIS-mutated marker bytes at offset `27,674,520`; the derived pre-bundle
SHA-256 exactly matched the recorded companion build hash. This reconciles the
installed executable with the UAT build without changing either file. The
remaining blocker is runtime recovery verification: consent remained
unchecked, so no new recording or interrupted-recovery action was performed.
Overall Desktop/core acceptance remains **NOT PASSED**. No commit, push, tag,
or release was created.

## Installed-package microphone and interrupted-recording recovery retest — 2026-10-20

This later run supersedes the first-launch note above that consent remained
unchecked and no recording was performed; that note remains accurate for its
earlier run. The same unsigned UAT-ID NSIS `0.1.1` package was tested from
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-nsis-20261020\fung.exe`
(34,430,528 bytes; SHA-256
`9D13145513BB904A0AD22508D2EC246B82126DE913DA56812CA8230837E9362A`). Its
application id remained `dev.fung.uat.core20261019`; the normal
`dev.fung.local` profile was not opened. At verification time the FUNG checkout
was on `codex/readiness-fixes-20261015`, HEAD
`56c53544718989fd5429dd5071479262fab7c2ee`, with unrelated local changes. No
claim is made that the installed binary represents a clean commit build.

The actual isolated data roots used by this run were the app-id-scoped
`%APPDATA%\dev.fung.uat.core20261019`, `%LOCALAPPDATA%\dev.fung.uat.core20261019`,
and `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\output`.
A pre-crash copy was made after a normal stop and before the interrupted test;
a second copy was made after recovery inspection and a normal close. Every
copied file hash matched its source:

- Pre-crash: `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\package-recovery-retest-uat-20261020\pre-crash-live-profile-20261020-125927\backup-manifest.json` — 585 files, 177,804,408 bytes.
- Post-crash/reopen: `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\package-recovery-retest-uat-20261020\post-crash-reopened-profile-20261020-130547\backup-manifest.json` — 596 files, 186,465,302 bytes.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Mic-only capture after manual consent | FAIL — usable speech was not captured | ONIKUMA M530 selected; system loopback unchecked; participant notice manually acknowledged. Recording `3e41cbc2-c1ee-4339-b788-e6611d3dcc89` stopped at 73,934 ms with 10 WAV chunks. Eight chunks contained all-zero PCM; the other two had peaks of only 7 and 1 on signed PCM16. No transcript was produced and local summary creation failed. This does not establish that the user did not speak; it establishes that the saved signal was effectively silent. |
| Interrupted-session setup | PASS — durable files created | Recording `ac04a18a-c88c-48d5-a0b1-e97fa2820c24` was left active for 11 complete 8-second WAV files, then only the uniquely identified isolated PID 31152 at the exact UAT executable path was force-stopped. Before termination the pre-crash snapshot had been SHA-256 verified. |
| Database/files immediately after crash | PASS for integrity; PARTIAL for ledger | Read-only SQLite `PRAGMA integrity_check` returned `ok`. Recording status remained `recording`, duration 64,000 ms, with eight ledger rows. Each of the eight indexed file paths, sizes, and SHA-256 hashes matched. Eleven WAVs existed on disk, leaving `mic-00009.wav`–`mic-00011.wav` outside the ledger. There were no missing files among the eight registered chunks. |
| Startup and recovery audit | FAIL — interrupted session not recovered | After relaunch, Home showed no recovery prompt. History listed the item as `recording`; Review simultaneously showed the badge `บันทึกเสร็จแล้ว`, an inconsistent state. Startup wrote one `recovery.interrupted_recording_found` event (`knownChunks=8`, `missingFiles=0`, `orphanFiles=3`, `status=recording`) but no `recovery.recording_recovered` event. The recording stayed `recording`, and the three orphan WAVs were not adopted. A second orderly close/reopen left the finding count at one, so this event was idempotent across that restart. |
| Review/playback/transcript | PARTIAL | The interrupted item opened in Review and the PCM16 player advanced to `playing` at 0:04 / 1:04; playback was stopped without listening for or recording acoustic output. The Transcript tab said there was no readable transcript. The recording metadata still said `recording`; opening Review did not repair it. |
| Acceptance | NOT PASSED | This package run does not qualify real speech capture or interrupted-recording recovery. Database integrity and the eight committed chunk hashes passed, but status reconciliation and orphan adoption failed. The package remains unsigned; this UAT created no new commit, push, tag or release. PR #80 remains merged at the previously recorded commit. |

Read-only inspection after the second restart still returned SQLite integrity
`ok`, 15 recording rows and 86 audio-chunk rows. The current recording had
exactly one `live_meeting.started` event and one
`recovery.interrupted_recording_found` event; it had zero
`recovery.recording_recovered` events. WAV amplitude statistics were measured
without playing or transcribing the raw recording. Overall Desktop/core
acceptance remains **NOT PASSED**, and release remains gated.

## Recovery visibility and installed-package retest — 2026-10-20

The recovery UI/idempotency fix was built from HEAD
`56c53544718989fd5429dd5071479262fab7c2ee` plus the current working-tree
changes. The isolated UAT-ID NSIS installer was installed from the newly built
package at
`C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261019\installed-package-uat-20261020`.
Installer exit code was 0. The installed package contains 8,108 files totaling
2,133,498,579 bytes. Its installer is 1,538,970,629 bytes with SHA-256
`9D94BDC08968D19CFE5C22E2946C48197B2FB8933AC77A529D1F45E023401FE6`; the
installed `fung.exe` is version `0.1.1`, 33,435,648 bytes, SHA-256
`F0B3393967A25CD1EAF3083FCFF9866EA1289D1594A61E48B649C723F0FB0E2F`. Both
files are `NotSigned`. The package used app id
`dev.fung.uat.core20261019` and the isolated UAT profile/output described
above. The normal `dev.fung.local` profile was not opened.

| Scenario | Result | Evidence and limit |
| --- | --- | --- |
| Recovery notice on installed build | PASS | After the user manually acknowledged participant notice, the isolated app displayed the recovery action directly in the main shell. Selecting it adopted three orphan WAVs (2,304,132 bytes), reported 88 seconds, and did not report unreadable files. |
| Recovered recording and idempotent audit | PASS | Read-only projection query shows recording `ac04a18a-c88c-48d5-a0b1-e97fa2820c24` as `completed`, `duration_ms=88000`, 11 contiguous eight-second chunk rows, and exactly one `recovery.recording_recovered` event with `adoptedChunks=3`, `adoptedBytes=2304132`, `unreadableFiles=0`. All 11 chunks have `transcribed_at`; no transcript segments were produced from this near-silent audio. |
| SQLite and close/reopen persistence | PASS | After recovery, the app was closed using its own Close action. The freshly installed package was launched again with the same UAT app id/profile. Home showed no interrupted-recording prompt; History displayed the same item as `1:28 · completed` at 20 Oct 2026 13:00. Read-only SQLite `PRAGMA integrity_check` returned `ok` after relaunch. |
| ONIKUMA M530 mic signal | FAIL — source signal near-silent | The user confirmed speaking while the ONIKUMA M530 mic was selected and system loopback was off. Across the 11 chunks of the interrupted recording, seven had all-zero PCM; the maximum peak was 21 and maximum RMS was 0.5 on signed PCM16. There were no transcript segments. An earlier 73,934 ms mic-only sample had eight zero chunks and only two tiny peaks (7 and 1). This means the saved mic signal is not usable; it does not prove the user did not speak. Mic capture remains an open acceptance gate. |
| Source checks and release-artifact restoration | PASS | `npm run test:callmd-shell` 18/18, `cargo test --lib recovery::tests -- --nocapture` 10/10, `npm run build`, `cargo fmt --all -- --check`, and `git diff --check` passed. The generated UAT installer and exe were retained under the external verification `artifacts` directory. Shared release outputs were restored from the verified pre-build copies; their hashes match the original installer `46E8A77DC01EF6BCAB7E0B8144BD217C4DBE9CBC0541BFE3D54FA20006E71E60` and executable `A83EC7D36ABC6E824523CD5F210C5EAC244CE9F71427765E8D09E09536D40519`. |

The machine-readable receipt is stored outside the repository at
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\installed-package-recovery-retest-20261020.json`.
Raw audio and transcript text were not copied into the repository. The
recovery UI and persistence gates now pass on the isolated installed build;
mic signal, Thai quality, signing, integrations, and viewport qualification
remain open. Overall Desktop/core acceptance remains **NOT PASSED**. No commit,
push, tag, or release was created.

## Participant-notice default and fresh isolated package — 2026-10-20

At the user's request, the Live page initializes its participant-notice
acknowledgment as checked each time a new Live session is mounted. The control
remains visible and toggleable. The separate system-loopback choice continues
to default off, and Start remains gated by capture capability as well as the
acknowledgment state. `tests/callmdLiveWorkspace.test.mjs` verifies these source
conditions; the focused suite passed **16/16**.

The updated source was built and installed with a separate app identity. The
regular `dev.fung.local` profile was not opened:

| Item | Evidence |
| --- | --- |
| UAT app id | `dev.fung.uat.autonotice20261020` |
| Installer | `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\FUNG_0.1.1_x64-setup.uat-autonotice-20261020.exe`; SHA-256 `A7977A15C6E34F811C1D97885B9F57E8075DF6196C5767F45E226060DA01BFDB`; unsigned |
| Installed executable | `C:\Users\suanr\AppData\Local\Temp\fung-uat-pr80-core-20261020-autonotice\installed\fung.exe`; SHA-256 `B1E5CD90A72B4AE752616BD382CF673EF5A6FD5796F264C5FDE44723B21DAE81`; unsigned |
| Live UI | Participant notice checked on first Live view; loopback unchecked; Start enabled |
| Microphone endpoint | System default is ONIKUMA M530; explicit dropdown selection was not needed for this run |
| Fresh mic sample | NOT RUN — the app is ready, but user readiness to speak has not been received; no new audio was recorded |
| Profile boundary | Isolated UAT app id and temp output only; normal profile not opened |

The existing recovery/History close-reopen result remains valid for its earlier
isolated build. This new package run verifies the participant-notice default
in the installed UI, not microphone signal quality. Prior ONIKUMA captures
remain near-silent; Thai recognition, mic quality, signing, integrations and
viewport qualification remain open. Overall Desktop/core acceptance remains
**NOT PASSED**. No commit, push, tag, or release was created.

Machine-readable package receipt: `G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\installed-package-notice-default-uat-20261020.json`.
It contains package and test evidence only; no raw audio or transcript.

## User-confirmed microphone capture and stopping-timer finding — 2026-10-20

The user later confirmed readiness and spoke during the installed isolated
package run. The capture used ONIKUMA M530 with system loopback off. The
completed database row is `b9b4d9ff-c5cf-4a3e-a390-bce80b0e9d19`, project
`9f9b875d-8dd0-46ea-a050-13209df6a48e`, source `microphone`, duration
**21,930 ms**. This was longer than the requested short sample; the persisted
duration is the measured value.

| Chunk | Interval | Bytes | SHA-256 | Peak / RMS | Nonzero samples |
| --- | --- | ---: | --- | ---: | ---: |
| 1 | 0–8,000 ms | 768,044 | `742bc71d43bcb6e1f381a1f25a6ad8b4579b447e490c95839585ec667c4d64fd` | 8,424 / 452.81 | 38.61% |
| 2 | 8,000–16,000 ms | 768,044 | `722a40ff514e13790c758e7dccafd20cde164894820c90f38d018d8127ac04b3` | 14,666 / 895.98 | 61.95% |
| 3 | 16,000–21,930 ms | 569,324 | `55834be933aab6c6245a51d89bcda9c243774cc69f9bbfb135a1e61659bbe723` | 2,516 / 139.86 | 46.71% |

All three WAV chunks contain nonzero PCM, each file hash matches its database
ledger value, and read-only SQLite `PRAGMA integrity_check` returned `ok`.
This passes nonzero microphone signal capture for this sample; it does not
establish intelligibility. Four transcript segments appeared, but Thai wording
and tonal contour were inaccurate, so Thai ASR remains **NOT QUALIFIED**. Raw
transcript text is not included in this report or the receipt.

During stop finalization, the rendered timer continued from `0:26` to `0:44`
and `1:00` while the UI still said it was stopping. The persisted audio length
remained 21.93 seconds. The user-facing History view was not verified for this
capture: the UI requested project selection and the screenshot did not prove
the new item reopened. Therefore only the completed database row, WAV ledger,
and database integrity are confirmed for this sample.

The source correction in `src/components/LiveMeetingPanel.tsx` freezes the
first elapsed value when stopping begins, including a stopping snapshot with
no elapsed value. `node --test tests/callmdLiveWorkspace.test.mjs` passed
**16/16**, and `npm run build` passed. A native package with the correction is
now built and hash-verified but has not yet been installed or run. It uses
isolated app id `dev.fung.uat.timerfix20261020`; its unsigned installer SHA-256
is `6B97092EF1BFE1889F1B7D5352EDC569788461CA79A1FE82FFA1658AD8DEB3C0`.
Installation and launch await action-time user approval. Core acceptance and
release remain **NOT PASSED**. The test was isolated to
`dev.fung.uat.autonotice20261020`; the regular `dev.fung.local` profile was not
opened. No commit, push, tag, or release was created.
