---
version: "0.1.1"
created_at: "2026-09-17T23:41:54+07:00,Codex orchestrator"
last_update: "2026-09-18T00:47:00+07:00,Codex orchestrator"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "root-cause-analysis"
  scope: "Native window controls and Tauri drag-region boundaries"
  risk: "MEDIUM: visible desktop interaction and window lifecycle"
---

# RCA: Desktop shell has no usable close control and content drags the window

## Symptom

The packaged Desktop UI does not present an obvious close control. Pointer
movement over the content or Home surface can move the entire frameless window
instead of scrolling or interacting with the intended content.

## Evidence

- `src-tauri/tauri.conf.json:39` sets `decorations` to `false`, so Windows does
  not provide the normal title-bar close/minimize buttons.
- `src/components/desktop/DesktopShell.tsx:734-759` renders the visible shell
  header with theme, settings, and pairing actions, but no close or minimize
  action.
- `src/App.tsx:1934-1963` renders the only close/minimize controls inside the
  legacy HUD power dock. That dock is below the new shell's visible header and
  is not the primary window-control surface.
- `src/App.tsx:1574` marks the whole `.panel-glass` content surface as
  `data-tauri-drag-region`.
- `src/components/HomeScreen.tsx:20` marks the whole Home surface as a drag
  region. Its scrollable meeting list at `HomeScreen.css` has no explicit
  `no-drag` boundary.
- `src/styles.css:1230-1243` relies on descendants opting out with
  `-webkit-app-region: no-drag`; only selected sections and controls do so.
- The existing Desktop component contract tests cover state/navigation and
  native command shapes, but do not assert that the visible shell exposes
  window controls or that scroll containers are outside drag regions.

## Root Cause

Two layout generations were composed without a single window-control and
interaction boundary:

1. The new `DesktopShell` owns the visible application chrome but does not own
   window lifecycle controls. The close/minimize implementation remained in
   the legacy HUD, which is not reliably visible as the shell's primary
   control surface.
2. Frameless-window dragging was applied at container level to the legacy
   panel and Home surface. The opt-out rule is selective, so content and
   scrollable regions inherit the Tauri drag affordance. Native WebView input
   therefore treats a user gesture as window movement instead of content
   scrolling/interacting.

## Why the issue escaped detection

CI and browser-preview checks can verify React structure, build output, and
browser navigation, but they cannot prove Windows frameless-window hit
testing. The current contracts also encode the old power-dock location rather
than requiring controls in the visible `DesktopShell` header. The packaged
native artifact was built before a native interaction pass exercised these
two boundaries.

## Bounded fix (implemented after approval)

1. Add explicit minimize and close buttons to the visible `DesktopShell`
   header, wired to the existing typed `minimizeWindow` and `closeWindow`
   bridge functions. Keep an accessible label, visible focus state, and a
   destructive visual treatment for close.
2. Remove drag-region attributes from content containers (`panel-glass` and
   `HomeScreen`). Keep dragging only on a dedicated empty chrome/header area;
   explicitly mark scroll containers, content zones, and interactive wrappers
   as `no-drag`.
3. Add focused UI contract/render assertions for visible close/minimize
   controls and drag-free scroll/content surfaces. Preserve existing native
   permission contracts and the current close/minimize bridge behavior.
4. Rebuild and run the relevant UI tests, then perform the available native
   smoke checks. Full native hit-test UAT remains pending when the environment
   does not expose a native-app inspection surface.

## Implementation evidence

- `node --test tests/callmdDesktopShell.test.mjs tests/callmdDesktopIntegration.test.mjs` passed 15/15.
- `npm run build` passed with TypeScript and Vite production output.
- `npm run tauri -- build --bundles nsis` passed and produced the runtime-inclusive Windows installer.
- Browser preview rendered Home, Live, and History with visible `ย่อหน้าต่าง` and `ปิดหน้าต่าง` controls.
- The rebuilt native `fung.exe` started and remained responsive. Native pixel-level drag/close interaction was not independently asserted because this environment exposes no native-app UI surface.
- MSI packaging remains unavailable in this run because WiX `light.exe` failed; NSIS is the delivered installer format.

## Out of scope

No redesign of the content IA, no new window manager, no data migration, no
change to the native command allowlist, and no release publication/signing
change are proposed by this RCA.

## Version diff / CHANGELOG

`0.1.0b` -> `0.1.1`: records the approved bounded correction, verification
evidence, and the remaining native UAT limitation.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| `0.1.0b` | `2026-09-17` | candidate | Documented missing visible window controls and overbroad native drag regions. | Uncommitted | Codex orchestrator |
| `0.1.1` | `2026-09-18` | beta | Implemented visible native controls and narrowed drag boundaries; verified tests, build, NSIS package, browser preview, and native process smoke start. | Uncommitted | Codex orchestrator |
