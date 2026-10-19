---
version: "1.0.1c"
created_at: "2026-10-16,Codex"
last_update: "2026-10-17,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "implementation-report"
  scope: "FUNG Desktop native screen UAT before release"
---

# FUNG Desktop native screen UAT

## Outcome

**The initial native screen pass, synthetic Review-profile click-through, and
2026-10-17 isolated packaged upgrade/reopen passed, but release UAT remains
incomplete. Keep PR #80 in Draft and do not tag a release from this run.** The
original screen pass covered Home, Live setup, History, Output, Appearance,
Companion, project/More disclosures, Pairing, and every Settings tab. The
isolated debug-app pass completed a synthetic import, populated Review
playback, pause/seek, transcript display, export, and output-folder selection
followed by a new export write. The packaged upgrade pass covered Home and
selected account/backup settings in an empty test profile.

This is not end-to-end acceptance for every feature. Native microphone capture,
connected account/device flows, provider calls, signed-release packaging,
upgrade with a data-bearing profile, and the remaining reliability and
speech-quality gates were not exercised. The packaged UI pass did not sign in,
create a recovery key, or write backup/cloud data.

This is a UI screen pass with explicit integration gaps; it is not end-to-end
acceptance for every feature shown in Settings.

## Environment and data boundary

- Source branch: `codex/readiness-fixes-20261015` (PR #80), latest tested code
  head `3ac92d83aa3ef4dd48306f7ebe7e013c8b2cca02`.
- Built and launched the Tauri **debug** application from the current source.
  A temporary config used identifier `dev.fung.uat.local`, so GenesisBlockDB
  was opened under `C:\Users\suanr\AppData\Roaming\dev.fung.uat.local\genesisdb`.
- The isolated UAT database had no projects or recordings. The default output
  path displayed by the UI was `C:\Users\suanr\Documents\fung`; no folder
  selection, import, export, capture, backup or restore was performed.
- `scripts/stage_media_fetch_runtime.ps1 -WithJsRuntime` staged the hash-pinned
  URL-fetch packages into the ignored local `.venv-whisper` resource folder;
  the embedded Python import probe and Deno wheel completed successfully. No
  media URL was entered or fetched; the app's consent state remained withheld.
- No microphone capture, provider API request, API-key entry, OAuth, Zoom
  connection, FUNGWIRE enablement, local API start or LAN share was triggered.
  The earlier follow-up ran the local CPU Whisper worker against synthetic
  audio. That worker-only step did not start inference through the app UI;
  the later app-level synthetic import and transcription are recorded below.
- In the initial screen pass, the cloud credentials, backup destination/key,
  account login, and restore actions were not activated. The current native
  click-through used the separate `dev.fung.uat.review20261017b` profile; the
  normal `dev.fung.local` profile was not opened or modified in that pass.

## Screen results

| Desktop screen | Result | Evidence and boundary |
| --- | --- | --- |
| Home | PASS — UI | Dashboard, empty project/activity counts, shortcuts and persistent bottom dock rendered. “Start meeting” navigated to Live setup only. |
| Live | PASS — setup UI; capture NOT_RUN | Device/language controls rendered. Participant-notice consent and system-audio capture both start unchecked; Start remains disabled until the notice is acknowledged. No capture was started. |
| Review / History | PASS — populated synthetic review | Native app imported one synthetic WAV, completed transcription, displayed one Thai segment, and passed playback, pause, seek, and export. No microphone capture, summary, or question action was run. |
| Output | PASS — isolated chooser and write | Native folder picker selected the same isolated `%TEMP%` output directory; the setting persisted and a subsequent export job wrote WAV/SRT/VTT there. No user Documents path was used. |
| Appearance | PASS — native Glass toggle | Windows Acrylic blurred the transparent outer margin. Switching to solid cleared the native Acrylic effect; returning to Glass/full applied it again. The setting was restored to Glass/full. |
| Projects disclosure | PASS — empty state | Empty local-project state and “selection does not start recording/playback” disclosure rendered. |
| More disclosure | PASS — menu | Import, Pair, Appearance and Account actions rendered; Export was correctly disabled without a project. Import was not invoked. |
| Companion | PASS — dialog | In-app panel opened and Escape closed it. No inference or capture started. |
| Pairing | PARTIAL — service configuration verified; app flow NOT_RUN | The initial screen pass showed missing `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY`. The current `.env` now has both; read-only Auth health and settings requests returned HTTP 200. No app sign-in, device-list acceptance, paired-device fixture, or pairing mutation was run. FUNGWIRE stayed off. |
| Settings dialog | PASS — shell | Dialog opened; all seven tabs rendered. First opening lazy-loaded tab content took several seconds and then completed. |
| Settings: External Connections | PARTIAL — configuration | Shows setup pending and no auth endpoint configured. No account portal was opened. |
| Settings: Sign In and Backup | PARTIAL — configuration | The initial screen pass showed missing Supabase configuration and backup destination. Current read-only Auth checks accept the configured anon key; no account login, recovery key, backup, or restore was run. |
| Settings: TTS Providers | PARTIAL — provider | Empty state rendered; no TTS provider is configured. |
| Settings: Cloud Providers | PASS — form only | Provider forms and local/cloud policy controls rendered. Credential fields were blank; no credentials or provider request were sent. |
| Settings: Fetch from URL | PASS — consent boundary; fetch NOT_RUN | Shows that internet media fetching is not authorized. Pinned `yt-dlp` and Deno were staged locally and passed the embedded Python probe, but no URL was entered and consent was not enabled. |
| Settings: Zoom import | PARTIAL — connection | Shows Zoom disconnected. No OAuth or import was started. |
| Settings: Runtime | PASS — status only | Local API showed Stopped. Start API, web link and LAN share were not activated. |

### Visual and interaction notes

- Native window size was 1280×800. Home, Live, History, Output, Appearance,
  Companion, disclosures, Pairing and Settings were inspected. Live and
  Settings use vertical scrolling regions.
- Native Acrylic now blurs the content behind the full-glass window margin;
  the prior sharp text bleed was not visible. The solid material clears Acrylic,
  and returning to Glass/full reapplies it.
- The system-audio and participant-notice options both start unchecked, and
  Start remains disabled until the notice is selected.
- Visual inspection and the native accessibility tree were used at the fixed
  desktop size. This is not a WCAG certification or a resizable-window test.

## Verification receipts

- Focused Desktop/recording CPU suite, including Pairing config and error states:

  ```powershell
  node --test --experimental-strip-types tests/callmdDesktopShell.test.mjs tests/callmdDesktopIntegration.test.mjs tests/callmdLiveWorkspace.test.mjs tests/callmdRecordingReview.test.mjs tests/recordingOutput.test.mjs tests/meetingIntelligenceUi.test.mjs tests/captureOrchestration.test.mjs tests/desktopBootstrap.test.mjs tests/egressRegister.test.mjs tests/deviceAuthority.test.mjs tests/pairingErrors.test.mjs
  ```

  **98 passed, 0 failed, 0 skipped.** These checks do not substitute for the
  native flows listed as NOT_RUN above.
- TypeScript check and Vite production build completed with exit code 0 using
  the checked-in Node entrypoints (`node node_modules/typescript/bin/tsc` and
  `node node_modules/vite/bin/vite.js build --logLevel warn`).
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  `cargo check --manifest-path src-tauri/Cargo.toml`, and `git diff --check`
  completed successfully.
- URL-fetch staging completed with `media-fetch-ready 2026.07.04`; the deno
  wheel and both wheel-provided license files were present. This proves local
  runtime installation only, not an installed bundle or a real remote fetch.
- GitHub Actions on source head `3ac92d83aa3ef4dd48306f7ebe7e013c8b2cca02`: [run 38917418422](https://github.com/Freshair129/FUNG/actions/runs/38917418422) — frontend PASS, Rust PASS.
- Native debug build and isolated-profile launch passed in the initial screen
  run; package evidence from the follow-up is recorded below.

## Follow-up evidence (2026-10-17)

### Synthetic import and transcription boundary

- Created an offline synthetic Thai speech fixture with the installed Windows
  `Microsoft Pattara` voice. The 6.833-second, 301,392-byte WAV lives under
  `%TEMP%\fung-uat-followup-20261017a`; it contains no user audio or personal
  data.
- Ran the checked-in worker against the bundled `large-v3-turbo` model on CPU:

  ```powershell
  .venv-whisper\Scripts\python.exe scripts/transcribe.py `
    "$env:TEMP\fung-uat-followup-20261017a\fung-synthetic-thai-uat.wav" `
    --model "$PWD\.venv-whisper\models\large-v3-turbo" --profile cpu --language th
  ```

  The worker returned `language=th`, `languageProbability=1`, `durationMs=6833`,
  and one populated segment: “นี่คือไฟล์ทดสอบของฟัง Desktop เสียงพูด นี้ สร้างขึ้นอัตโนมัติ ไม่มีข้อมูลส่วนตัว”. This proves local worker output only; the file was not imported through the FUNG UI and this is not Thai quality acceptance.
- Launched the Tauri debug build with identifier
  `dev.fung.uat.followup20261017a`. Its data directory was
  `%APPDATA%\dev.fung.uat.followup20261017a`, and the configured output path
  was inside the same temporary UAT root. The normal `dev.fung.local` profile
  was not used.
- At this point in the earlier follow-up, `cua.getState()` continued to return
  `apps: []`, so the file picker, import action, populated Review controls,
  and output chooser were not run in that pass. The subsequent native
  click-through is recorded below.

### NSIS build and isolated package install

- Built the NSIS bundle from this source with the test identifier above and
  `--no-sign`; this is an unsigned local UAT artifact, not a signed release.
- Installer: `src-tauri/target/release/bundle/nsis/FUNG_0.1.1_x64-setup.exe`,
  1,541,777,847 bytes (1.44 GiB), SHA-256
  `4947315103D8C48EC0234ED5E78A751B8CFCB35953840E4CE12C3E92B5AECFD4`.
- Installed to `%TEMP%\fung-uat-followup-20261017a\installed`. The installed
  FUNG executable (version `0.1.1`), Python runtime, transcription scripts,
  knowledge-parser runtime, and 1,617,884,929-byte `large-v3-turbo` model were
  present. The installed model SHA-256
  `E76620F83D5F5B69EFD3D87E3DC180C1BD21DF9FBEBACFD4335E5E1EFCC018DA` matched
  the source model.
- The packaged executable opened with window title `FUNG` and reused the same
  empty UAT profile initialized by the debug launch (13 profile files, about
  2.29 MB). This initial install proved empty-profile startup at the process
  level. The later packaged UI and upgrade pass is recorded below. The test app
  remains isolated from the normal `dev.fung.local` profile.

### Focused regression checks

- Four focused Node suites covering Desktop integration, recording Review,
  output routing, and native-session custody completed with 38 unique tests
  passing. The first combined attempt hit a file-lock collision because the
  debug FUNG app was still running; after closing it, the native-session suite
  passed 12/12.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib import -- --nocapture`
  passed 13/13, and the `recording_output` filter passed 4/4.
- Exact-head CI run 38917418422 passed both frontend and Rust jobs. No source
  files changed during this follow-up; the follow-up changes are documentation
  only. Exact-head CI for the current documentation commits is tracked in
  PR #80.

### Native Review-profile click-through (2026-10-17)

- Built and opened the FUNG debug app from this branch using the isolated app
  identifier `dev.fung.uat.review20261017b` at tested app-source head
  `b66d2447d0afb197bc5ea1b33e0edf227527ad73`. Its profile directory was
  `%APPDATA%\dev.fung.uat.review20261017b`; the output directory was
  `%TEMP%\fung-uat-review20261017b\output`. The path was shown as “ready to
  write” in the native Output page and persisted in
  `%APPDATA%\dev.fung.uat.review20261017b\recording-output.json`. The native
  folder picker selected this same path. Only this test profile was changed.
- Imported the offline synthetic Thai WAV
  `%TEMP%\fung-uat-followup-20261017a\fung-synthetic-thai-uat.wav` through the
  app. The imported file was 301,392 bytes with SHA-256
  `D52863A874B942E4B3EE453720B7771FC18CAA7F5721B83159CA16EF284719BF`.
  FUNG created project `c70988c5-2670-47d5-af5e-f2373d522cd2` and recording
  `8490cd8a-ae36-44d5-a375-9d44c36f3a47`; the transcription job completed with
  one populated Thai segment. The fixture contains generated speech only, not
  user audio or personal data.
- In populated Review, opened the native player, played, paused, sought to
  approximately 4 seconds of the 6-second recording, and closed the player.
  The transcript remained visible and matched the same project/recording pair.
- Triggered export from Review both before and after selecting the isolated
  output path. The post-selection `export.render` job
  `68428132-73c1-4c31-a878-3f469c64a2a4` completed at 100%. WAV, SRT, and VTT
  artifacts appeared in Review and were written under the isolated project
  export directory. The exported WAV is 301,392 bytes and has the same SHA-256
  as the imported WAV. SRT/VTT contain the transcript cue at 00:00–00:06.180.
- This closes the synthetic app-level import/Review/playback/export and
  output-folder-selection/write gate for the debug app. It does not prove
  microphone capture, Thai speech quality, packaged UI behavior, or any
  account/provider integration. A prior UAT left synthetic test data in the
  primary `dev.fung.local` profile; this pass did not open, modify, or delete
  that profile or its data.

### Supabase configuration check (2026-10-17)

- Confirmed `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` are populated in
  the local `.env`; values were not printed or added to this report.
- Read-only requests to the configured Auth health endpoint and Auth settings
  endpoint returned HTTP 200. The settings request included the anon key; no
  response body was retained.
- This verifies endpoint reachability and that the API accepts the configured
  key for a public settings request. It does not verify a signed-in session,
  device list, pairing, or any write operation; those remain release gates.

### Packaged UI and isolated upgrade UAT (2026-10-17)

- Rebuilt the NSIS installer from source head
  `49025946d69d24fdc22c944eb0be4c4cd206a91e` using the temporary app identifier
  `dev.fung.uat.followup20261017a` and `--no-sign`. The local installer is
  1,538,564,985 bytes (1.43 GiB), SHA-256
  `940DA0435C06A1EAF7C50C9BE07D8FF6CC3D531465587D77A75AC6630985EBA0`; Windows
  reports its Authenticode status as `NotSigned`. This is a local UAT package,
  not a release artifact.
- Before reinstalling, copied the existing test profile from
  `%APPDATA%\dev.fung.uat.followup20261017a` to
  `%TEMP%\fung-uat-followup-20261017a\profile-before-upgrade-20261017-234102`.
  The copy contains 15 files (4,861,096 bytes); SHA-256 checks matched all
  source files before installation.
- Ran the installer maintenance option `Add/Reinstall components` into the
  exact existing directory `%TEMP%\fung-uat-followup-20261017a\installed`.
  Setup completed successfully. The optional desktop shortcut was unchecked;
  the installed app was launched from Setup.
- The packaged `0.1.1` app loaded Home, then Settings → Sign In and Backup.
  That page rendered Google sign-in and the local Development/Test backup
  controls. No sign-in, recovery-key creation, backup, cloud write, or provider
  call was performed. Settings → External Connections remained `Setup pending`
  because `VITE_FUNG_WEB_APP_URL` is not configured. The local `.env` Supabase
  URL and anon key are present in the built bundle; their values are not
  recorded here.
- Closed the packaged app normally, then reopened `fung.exe` from the same
  temporary install and profile. Home loaded again and showed zero projects.
  After the first open/close, the profile still had 15 files and the same
  4,861,096-byte total, with no added or missing paths. Hashes changed for
  `genesisdb/projection.sqlite`, `genesisdb/projection.sqlite-shm`, and
  `genesisdb/state.json` during normal startup/shutdown. The verified
  pre-upgrade copy remains available. This profile was empty; the run does not
  establish upgrade safety for a profile containing real projects or
  recordings. The normal `dev.fung.local` profile was not used.

### Data-bearing packaged-profile upgrade UAT (2026-10-18)

- Imported the offline synthetic Thai WAV through packaged FUNG under the
  isolated app identifier `dev.fung.uat.followup20261017a`. It created project
  `74ef455d-4ee5-4f38-95a5-0a009504600a`, recording
  `b42bf53a-0be3-4369-bcec-c41538fef964`, and transcription job
  `40d2de80-3942-4a6e-b242-87bf57ab878a`. The job completed at 100% and wrote
  one Thai transcript segment. The synthetic WAV is 301,392 bytes with SHA-256
  `D52863A874B942E4B3EE453720B7771FC18CAA7F5721B83159CA16EF284719BF`.
- Closed FUNG normally before the upgrade. Backed up the isolated profile
  (15 files, 5,341,500 bytes) and the project output WAV to
  `%TEMP%\fung-uat-followup-20261017a\pre-populated-upgrade-backup-20261017-1953UTC`;
  all 16 copied files matched their source SHA-256 values.
- Re-ran the previously recorded unsigned NSIS package
  (`940DA0435C06A1EAF7C50C9BE07D8FF6CC3D531465587D77A75AC6630985EBA0`)
  against the same isolated install directory with `/S` and the explicit
  `/D=<install-path>` target. The installer exited with code 0. This was the
  same local UAT package, not a signed release artifact.
- Reopened the installed app with the same app identifier and profile. Home
  showed one project (`Session 1`); Review showed the same recording as
  `completed` at 0:06 and displayed its Thai transcript. SQLite
  `integrity_check` returned `ok`; the database still contained one project,
  one completed recording, one completed transcription job, and one transcript
  segment. The 14 profile files other than `projection.sqlite-shm` matched the
  pre-upgrade backup exactly; the shared-memory file hash differed. The output
  WAV remained present and matched the backed-up SHA-256.
- The Review audio-player button was disabled, so playback was not verified.
  The separate Local Meeting Intelligence panel showed
  `The local meeting-intelligence action could not be completed` and no
  transcript revision. These observations do not change the confirmed
  project/recording/transcript persistence result. The normal `dev.fung.local`
  profile was not opened or modified.

## Release gates still open

1. Run Supabase device-list and pairing acceptance with a signed-in test
   account and disposable paired-device fixture. `.env` configuration and
   read-only Auth requests now pass, but no account or pairing flow was used.
2. Decide whether URL fetch, TTS, cloud, Zoom, account/backup and Local API/LAN
   surfaces are included in this release; if included, stage their dependencies
   in the actual packaging workflow and run provider-specific acceptance.
   URL-fetch consent remains off; no URL was fetched during this pass.
3. Validate a signed release package and upgrade a disposable profile that
   contains projects, recordings, transcripts, and provider settings. The
   unsigned isolated package now passed the synthetic data-bearing
   project/recording/transcript persistence check, but it is not a signed
   release artifact and the profile had no provider settings.
4. Verify concurrent enabled Local API writes, active capture/job shutdown,
   forced-crash recovery, Thai speech quality, microphone/device behavior, and
   packaged audio playback. The Review player was disabled during this UAT.

Until these gates are dispositioned, this UAT does not authorize PR merge or a
release tag.
