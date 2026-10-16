---
version: "1.0.0b"
created_at: "2026-10-16,Codex"
last_update: "2026-10-16,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "implementation-report"
  scope: "FUNG Desktop native screen UAT before release"
---

# FUNG Desktop native screen UAT

## Outcome

**The latest native recheck passed the corrected Desktop screen states, but
release UAT remains incomplete. Keep PR #80 in draft and do not tag a release
from this run.** Home, Live setup, History, Output, Appearance, Companion,
project/More disclosures, Pairing, and every Settings tab were opened in the
isolated native debug app. Native Acrylic, the disabled Pairing setup state,
and the opt-in audio defaults were verified after the fixes.

This is still a screen and setup pass, not end-to-end acceptance. Native
capture, populated recording review, output writes, connected account/device
flows, provider calls, the packaged installer and clean-install acceptance
were not exercised.

This is a UI screen pass with explicit integration gaps; it is not end-to-end
acceptance for every feature shown in Settings.

## Environment and data boundary

- Source branch: `codex/readiness-fixes-20261015` (PR #80), working tree based
  on `6abab2c` while the UAT fixes were being verified. The commit containing
  this recheck is recorded in Git history after closeout.
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
- No microphone capture, model inference, API-key entry, OAuth, Zoom connection,
  FUNGWIRE enablement, local API start or LAN share was triggered.
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
- GitHub Actions on source head `6abab2c`: [run 38906460788](https://github.com/Freshair129/FUNG/actions/runs/38906460788) — frontend PASS, Rust PASS. The new commit will receive its own CI run after push.
- Native debug build and isolated-profile launch passed. No installer was
  produced or installed during this UAT.

## Release gates still open

1. Set up the intended Supabase/auth environment and re-run device-list and
   pairing with a disposable paired-device fixture. Pairing is still blocked
   here because the environment has no Supabase config or signed-in account.
2. Run capture and populated Review with a disposable recording, including
   playback, pause/seek, transcript action and export, without using user data.
3. Qualify the output chooser and new-file path in a disposable directory.
4. Decide whether URL fetch, TTS, cloud, Zoom, account/backup and Local API/LAN
   surfaces are included in this release; if included, stage their dependencies
   in the actual packaging workflow and run provider-specific acceptance.
   URL-fetch consent remains off; no URL was fetched during this pass.
5. Build, install and click through the actual package, then repeat the clean
   install/upgrade and same-profile reopen gates on the exact release candidate.

Until these gates are dispositioned, this UAT does not authorize PR merge or a
release tag.
