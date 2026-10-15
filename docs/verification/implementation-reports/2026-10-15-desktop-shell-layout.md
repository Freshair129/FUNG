---
version: "1.0.1b"
created_at: "2026-10-15T08:08:05+07:00,Codex"
last_update: "2026-10-15T08:15:00+07:00,Codex"
status: "candidate"
attributes:
  domain: "desktop-ui"
  doc_type: "implementation-evidence"
  scope: "FUNG desktop shell viewport and scroll ownership"
---

# Desktop shell layout correction

Issue: [#81](https://github.com/Freshair129/FUNG/issues/81)

Readiness branch: [PR #80](https://github.com/Freshair129/FUNG/pull/80)

## Report and root cause

The user reported shell overflow, a scrolling sidebar and main content cut into
multiple scroll regions. The browser desktop surface reproduced this at
1280×720 before the correction:

- Header top was y=58 and body top y=116. The glass selector applying
  `position: relative` to every direct shell child overrode the absolute skip
  link, placing it in the first grid row. It also overrode fixed modal wrappers.
- The shell's unnamed three-row grid depended on whether capture status was
  present. Main repeated that assumption despite conditional notices/home actions.
- Sidebar client height was 604px with 743px of content. Glass padding overrode
  rail geometry; expanded project/status cards permanently occupied extra space.
- The review surface had 408px of client height and 719px of content while the
  review page also owned viewport-capped overflow. The embedded live panel kept
  modal scrolling, and its implicit grid minimum overflowed a narrow viewport.

## Correction

1. Name header/capture/body grid areas so an absent capture strip leaves a zero
   row. Keep the skip link absolute, overlays fixed and the companion wrapper
   out of grid flow. Clip decorative ambient paint without making the shell a
   programmatically scrollable viewport.
2. Limit glass stacking to the three structural regions. Theme rules no longer
   override sidebar/nav/main layout spacing.
3. Let main own page scrolling with a flexible column layout. Hosted review/live
   pages and their surface wrapper no longer create a second page scroller.
   Long transcript and history-list widgets retain their intentional scrolling.
4. Compact rail actions on short desktop windows. Put project context behind a
   native, initially closed `details`/`summary`, showing the selected project
   name when available. The whole rail can scroll when those details are opened
   or the window is too short; there is no tiny independently scrolling context.
5. Keep all four primary tabs in one row below the narrow-screen titlebar, with
   utility actions on the next row. Use zero-minimum grid columns so embedded
   live content can shrink within the main width.

Thai labels, existing handlers, recording guards and backend contracts are
preserved. This is a bounded presentation correction, not a programme phase.

## Verification

- `npm run test:callmd-shell`: 14/14 passed, including three new CSS/markup
  contract regressions for named regions, overlay stacking and page scrolling.
  These source assertions complement the rendered browser inspection below.
- `npm run build`: TypeScript and Vite production build passed.
- Rendered desktop surface checked at 1280×720, 1024×768, 800×600, 720×600 and
  390×844. Header starts at y=0; body and main end at the viewport bottom.
- At 1280×720, expanded rail with project details closed has client/scroll
  heights 662/662px. Keyboard Enter opens the project details and makes the
  full rail scrollable; closing them returns to 662/662px.
- Main owns the review page scroll. Home, live, output and appearance were
  visited; the narrow live overflow was corrected from 398/375px scroll/client
  width to 375/375px with no children beyond the main right edge.
- Glass and solid materials both keep header top at 0 and body bottom at 720
  for a 1280×720 viewport.
- An isolated fixture using the real shell component checked active capture
  and a modal at 1280×720 and 390×844. Capture starts immediately after the 58px
  header, body ends at the viewport bottom, and the modal wrapper stays fixed
  at y=0 with viewport height. The fixture does not start recording or models.
- Opening the real browser Companion leaves body height unchanged; its wrapper
  is `display: contents` and its layer remains fixed.

Local evidence is in `G:\AI-Studio\verification\2026-10-15-fung-layout`:
`browser-layout-receipt.json`, `after-desktop.jpg`, `after-narrow.jpg`,
`shell-tests-final.log` and `frontend-build-final.log`. The receipt retains the
earlier narrow live failure alongside its corrected result.

### User-requested native preview

After publishing the frontend correction at `46f6c78`, the user requested opening
FUNG. `tauri build --debug --no-bundle` completed successfully with the current
embedded frontend, normal `dev.fung.local` identity and a temporary build overlay
omitting bundled runtime resources. The executable was copied to
`G:\AI-Studio\development\FUNG\.codex\desktop-preview\fung.exe` and launched.
Process 29076 reported the window title `FUNG`, a nonzero main window handle and
`Responding: True`. This proves build/launch, not native visual or feature acceptance.

Receipts: `native-ui-build-receipt.json`, `native-app-launch-receipt.json` and
`native-ui-build.log` in the evidence directory above. The prior native7 executable
was preserved as `previous-native7.exe` with its recorded SHA-256 before reusing
the build cache. No model inference was requested. The preview uses normal local
app data; opening the app initialized its store. No installer was built or replaced.

## Limits

The rendered checks used Chrome and the production desktop frontend with native
commands unavailable. Capture/modal cases were component fixtures. No audio,
Ollama/GPU inference or installed-native-app replacement was performed during
the layout checks. The later user-requested native launch is scoped above;
native WebView visual/feature acceptance and installer acceptance remain separate. Long pages
and explicitly opened project details still scroll when their contents exceed
the viewport; scrolling is not hidden to make the check appear successful.

## Changelog

| Version | Date | Summary |
| --- | --- | --- |
| 1.0.1b | 2026-10-15 | Record native preview build and user-requested launch separately from browser layout acceptance. |
| 1.0.0b | 2026-10-15 | Record issue #81, viewport/scroll root causes, scoped correction and browser/build evidence. |
