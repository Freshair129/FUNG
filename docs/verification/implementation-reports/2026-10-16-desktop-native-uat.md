---
version: "1.0.1b"
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

**The latest native screen pass, synthetic Thai transcription, NSIS build,
isolated install, and packaged-app launch passed, but release UAT remains
incomplete. Keep PR #80 in draft and do not tag a release from this run.** The
original screen pass covered Home, Live setup, History, Output, Appearance,
Companion, project/More disclosures, Pairing, and every Settings tab. The
follow-up proved local transcription and package startup in an isolated test
profile; it could not perform native file-picker or Review clicks because the
computer-use window inventory returned no app windows.

This is still not end-to-end acceptance. Native capture, importing the
synthetic audio through the app, populated Review playback and actions, output
chooser writes, connected account/device flows, provider calls, and upgrade
acceptance were not exercised. The packaged installer was built and installed
in a clean temporary directory; the packaged UI was launched but could not be
clicked through in this session.

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
  The follow-up ran only the local CPU Whisper worker against synthetic audio;
  no inference was started through the app UI.
- The output chooser, cloud credentials, backup destination/key, account login,
  and restore actions were not activated. The UAT application profile remained
  separate from the normal `dev.fung.local` profile.

## Screen results

| Desktop screen | Result | Evidence and boundary |
| --- | --- | --- |
| Home | PASS — UI | Dashboard, empty project/activity counts, shortcuts and persistent bottom dock rendered. “Start meeting” navigated to Live setup only. |
| Live | PASS — setup UI; capture NOT_RUN | Device/language controls rendered. Participant-notice consent and system-audio capture both start unchecked; Start remains disabled until the notice is acknowledged. No capture was started. |
| Review / History | PASS — empty state; populated review NOT_RUN | “Select a project first” state rendered. No recording, playback, question, summary or export job was created. |
| Output | PASS — display; path mutation NOT_RUN | Native UI showed `C:\Users\suanr\Documents\fung` and “ready to write”. Choose Folder and Reset were not activated. |
| Appearance | PASS — native Glass toggle | Windows Acrylic blurred the transparent outer margin. Switching to solid cleared the native Acrylic effect; returning to Glass/full applied it again. The setting was restored to Glass/full. |
| Projects disclosure | PASS — empty state | Empty local-project state and “selection does not start recording/playback” disclosure rendered. |
| More disclosure | PASS — menu | Import, Pair, Appearance and Account actions rendered; Export was correctly disabled without a project. Import was not invoked. |
| Companion | PASS — dialog | In-app panel opened and Escape closed it. No inference or capture started. |
| Pairing | PARTIAL — configuration | Dialog names the missing `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY`; Create Pairing is disabled and FUNGWIRE stays off. No account, paired-device fixture, or network pairing was available. |
| Settings dialog | PASS — shell | Dialog opened; all seven tabs rendered. First opening lazy-loaded tab content took several seconds and then completed. |
| Settings: External Connections | PARTIAL — configuration | Shows setup pending and no auth endpoint configured. No account portal was opened. |
| Settings: Sign In and Backup | PARTIAL — configuration | Missing Supabase configuration and backup destination are explicit. No recovery key was generated, folder selected, backup or restore run. |
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
- After the process opened a `FUNG` window, `cua.getState()` continued to
  return `apps: []`. The installed FUNG process had a live window handle, but
  this session had no CUA window entry to bind. Therefore the file picker,
  import action, populated Review controls, and output chooser could not be
  clicked or observed; those remain NOT_RUN.

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
  `E76620F83D5F5B69EFD3D87E3DC180C1BD21DF9FBACFD4335E5E1EFCC018DA` matched
  the source model.
- The packaged executable opened with window title `FUNG` and reused the same
  empty UAT profile initialized by the debug launch (13 profile files, about
  2.29 MB). This proves clean install and empty-profile startup/reopen at the
  process level. Packaged UI click-through and an upgrade install were not
  verified. The installed test app remains open in the isolated profile.

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
  only. The new documentation commit will receive its own CI run after push.

## Release gates still open

1. Set up the intended Supabase/auth environment and re-run device-list and
   pairing with a disposable paired-device fixture. Pairing is still blocked
   here because the environment has no Supabase config or signed-in account.
2. Import the synthetic fixture through the native app and run populated
   Review: playback, pause/seek, transcript action and export. The CPU worker
   transcript above does not close this app-level gate.
3. Select the temporary output directory through the native chooser and
   verify a new file write. The test profile was preconfigured directly, so
   the chooser remains untested.
4. Decide whether URL fetch, TTS, cloud, Zoom, account/backup and Local API/LAN
   surfaces are included in this release; if included, stage their dependencies
   in the actual packaging workflow and run provider-specific acceptance.
   URL-fetch consent remains off; no URL was fetched during this pass.
5. Click through the installed package and run an upgrade over the temporary
   install. A clean temporary install and empty-profile reopen passed at the
   process level; interactive packaged UI and upgrade acceptance remain open.

Until these gates are dispositioned, this UAT does not authorize PR merge or a
release tag.
