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

**The current Desktop pages render in the native debug shell, but release UAT
is incomplete. Keep PR #80 in draft and do not tag a release from this run.**
Core page navigation and empty states were exercised on the current PR head.
Native capture, populated recording review, output writes, configured provider
flows, the packaged installer and clean-install acceptance were not exercised.

This is a UI screen pass with explicit integration gaps; it is not end-to-end
acceptance for every feature shown in Settings.

## Environment and data boundary

- Repository head: `61940cddc732cb27d6f64f8e65758adcc8a6db7c` on
  `codex/readiness-fixes-20261015` (PR #80).
- Built and launched the Tauri **debug** application from the current source.
  A temporary config used identifier `dev.fung.uat.local`, so GenesisBlockDB
  was opened under `C:\Users\suanr\AppData\Roaming\dev.fung.uat.local\genesisdb`.
- The isolated UAT database had no projects or recordings. The default output
  path displayed by the UI was `C:\Users\suanr\Documents\fung`; no folder
  selection, import, export, capture, backup or restore was performed.
- No microphone capture, model inference, API-key entry, OAuth, Zoom connection,
  FUNGWIRE enablement, local API start or LAN share was triggered.
- Closing the UAT app persisted a snapshot and completed the Genesis shutdown
  checkpoint at frontier 16. The app and both test Vite servers were closed.

## Screen results

| Desktop screen | Result | Evidence and boundary |
| --- | --- | --- |
| Home | PASS — UI | Dashboard, empty project/activity counts, shortcuts and persistent bottom dock rendered. “Start meeting” navigated to Live setup only. |
| Live | PASS — setup UI; capture NOT_RUN | Device/language controls rendered. Consent was unchecked and Start remained disabled. The system-audio toggle appeared checked by default; no capture was started. |
| Review / History | PASS — empty state; populated review NOT_RUN | “Select a project first” state rendered. No recording, playback, question, summary or export job was created. |
| Output | PASS — display; path mutation NOT_RUN | Native UI showed the default Documents path and “ready to write”. Choose Folder and Reset were not activated. |
| Appearance | PASS — UI | Theme, material and transparency selectors rendered. Dark theme changed successfully for this UAT session; persistence across restart and other selector values were not tested. |
| Projects disclosure | PASS — empty state | Empty local-project state and “selection does not start recording/playback” disclosure rendered. |
| More disclosure | PASS — menu | Import, Pair, Appearance and Account actions rendered; Export was correctly disabled without a project. Import was not invoked. |
| Companion | PASS — dialog | In-app panel opened and Escape closed it. No inference or capture started. |
| Pairing | PARTIAL — UI; device read blocked | Dialog opened and showed the device-list read error, FUNGWIRE disabled, and zero connected devices/jobs. No pairing code or network action was attempted. |
| Settings dialog | PASS — shell | Dialog opened and Escape closed it. |
| Settings: External Connections | BLOCKED — configuration | Account page rendered with setup pending; Supabase/auth endpoint is not configured in this environment. |
| Settings: Sign In and Backup | BLOCKED — configuration | Supabase setup and backup destination were not ready. No key was generated and no backup was run. |
| Settings: TTS Providers | BLOCKED — provider | Empty state rendered; no TTS provider is configured. |
| Settings: Cloud Providers | PASS — form only | Provider forms and local/cloud policy controls rendered. No credentials were present or saved; no provider request was made. |
| Settings: Fetch from URL | BLOCKED — dependency | Page reported `yt-dlp` is not installed/staged. No URL was entered or fetched. |
| Settings: Zoom import | BLOCKED — connection | Page showed Zoom disconnected. No OAuth or import was started. |
| Settings: Runtime | PARTIAL — status UI | Local API showed Stopped. Start API, web link and LAN share were not activated. |

### Visual and interaction notes

- Native window size was 1280×800. The five primary surfaces, bottom dock,
  project disclosure and dialogs fit in the observed window; Live and Settings
  used their own vertical scrolling regions.
- In full-glass mode, sharp text from the window behind FUNG was visible through
  the native window's transparent outer margin and narrow card gutters. Dialog
  backdrops blurred correctly. Review this edge/gutter appearance before visual
  sign-off; no CSS change was made in this UAT task.
- The checked system-audio option is visible before participant-notice consent,
  while the Start button remains disabled. Confirm this default is intended for
  release.
- Visual inspection and the native accessibility tree were used at the fixed
  desktop size. This is not a WCAG certification or a resizable-window test.

## Verification receipts

- Focused Desktop/recording CPU suite:

  ```powershell
  node --test --experimental-strip-types tests/callmdDesktopShell.test.mjs tests/callmdDesktopIntegration.test.mjs tests/callmdLiveWorkspace.test.mjs tests/callmdRecordingReview.test.mjs tests/recordingOutput.test.mjs tests/meetingIntelligenceUi.test.mjs tests/captureOrchestration.test.mjs tests/desktopBootstrap.test.mjs tests/egressRegister.test.mjs
  ```

  **89 passed, 0 failed, 0 skipped.** These checks do not substitute for the
  native flows listed as NOT_RUN above.
- TypeScript check and Vite production build completed with exit code 0 using
  the checked-in Node entrypoints (`node node_modules/typescript/bin/tsc` and
  `node node_modules/vite/bin/vite.js build --logLevel warn`).
- GitHub Actions on this head: [run 38890387476](https://github.com/Freshair129/FUNG/actions/runs/38890387476) — frontend PASS, Rust PASS.
- Native debug build, first launch and orderly shutdown passed. No installer
  was produced or installed during this UAT.

## Release gates still open

1. Resolve or explicitly accept the native glass edge/gutter bleed and the
   system-audio default before visual/privacy sign-off.
2. Re-run the device-list and pairing flow with the intended local service and
   a disposable paired-device fixture.
3. Run capture and populated Review with a disposable recording, including
   playback, pause/seek, transcript action and export, without using user data.
4. Qualify the output chooser and new-file path in a disposable directory.
5. Decide whether URL fetch, TTS, cloud, Zoom, account/backup and Local API/LAN
   surfaces are included in this release; if included, stage their dependencies
   and run their provider-specific acceptance. Otherwise keep the unavailable
   states explicit in release notes.
6. Build, install and click through the actual package, then repeat the clean
   install/upgrade and same-profile reopen gates on the exact release candidate.

Until these gates are dispositioned, this UAT does not authorize PR merge or a
release tag.
