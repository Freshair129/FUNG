---
version: "1.0.0b"
created_at: "2026-10-15T08:48:00+07:00,Codex"
last_update: "2026-10-15T08:48:00+07:00,Codex"
status: "candidate"
attributes:
  domain: "desktop-ui"
  doc_type: "implementation-evidence"
  scope: "FUNG reference dashboard layout"
---

# Reference dashboard layout

Issue opened before implementation: [#82](https://github.com/Freshair129/FUNG/issues/82).
Candidate: `codex/readiness-fixes-20261015`, [PR #80](https://github.com/Freshair129/FUNG/pull/80).
Starting source: `3bf6de52bda636394e75de330d78ddc300c6c112`.

## Change

The supplied reference becomes a large left hero, two upper-right cards, a wide
activity card and a black bottom dock using existing FUNG actions. Project
context and secondary actions are keyboard-operable disclosures.

`StudioHome.tsx` reads existing project/live-status props. Activity counts project
creation by local calendar day over 7 or 30 days. Unknown, loading, error and
empty states are explicit; no synthetic statistics enter production. Companion
opens the existing panel without starting microphone capture or pretending to
answer. The waveform is decorative.

`DesktopStudio.css` scopes layout to `data-layout="studio"`. Main owns page
scrolling; document stays inside the viewport. Below 800px cards stack; below
720px dock uses two rows. Existing live/review owners, capture strip, stop guard
and handlers remain mounted when Home is shown. Returning from a guard whose
disclosure closed restores focus to its summary. No new backend route or native
window-close policy was added.

## Assets

Cloud-generated for this adaptation:

- `public/images/fung-listening-hero.png`: 1122 × 1402, blue/lavender listener portrait.
- `public/images/fung-assistant-waveform.png`: 1672 × 941, transparent waveform.

Both are decorative. Existing FUNG branding/Lucide icons remain. No local
Ollama/GPU model was started, stopped or unloaded.

## Verification

Evidence: `G:\AI-Studio\verification\2026-10-15-fung-dashboard`.

- `npm run test:callmd-shell`: 15/15 passed (`shell-tests-final.log`), including
  dashboard real/unknown data and preserved navigation/capture contracts.
- `npm run test:callmd-integration`: 6/6 passed (`integration-tests-final.log`).
- `npm run build`: passed; final native build rebuilds frontend again.
- [Browser design QA](../../../design-qa.md): passed. Compared reference and
  render together at 958 × 663; also checked 1280 × 800, 800 × 600 and 390 × 844.
  No document/horizontal overflow after root containment correction.
- Home/live/review/output, Companion, project menu, Settings and Appearance
  exercised. Capture guard and populated activity used an ignored isolated
  fixture with synthetic props; no audio capture occurred.
- Native debug builds passed; isolated preview startup is recorded below.

Historical HMR warnings and browser/native boundaries are in design QA. Native
audio, model accuracy, installer and release qualification are not established
by this layout patch. This does not promote programme gates.

## Native preview outcome

Both builds used `tauri build --debug --no-bundle`, omitted bundled resources
and rebuilt the embedded frontend. No installer was replaced.

The normal `dev.fung.local` launch failed in setup with
`REL_SCHEMA_VERSION_CONFLICT: schema version must advance exactly once`, matching
the still-open [issue #41](https://github.com/Freshair129/FUNG/issues/41).
Logs: `native-app.stderr.log` and `native-app.stdout.log`. No database was deleted
or reset. The previous preview binary was preserved as `previous-layout.exe`.
This layout change does not repair the existing-profile startup issue.

An isolated preview with identifier `dev.fung.studio-preview-20261015` built and
opened successfully: process 24464, title `FUNG`, nonzero window handle and
`Responding=true`. Executable: `.codex/studio-preview/fung.exe`; SHA-256
`aca51ec03237d2f7b25f3673c0dc1352ffc24310b338b1117ea4e98545c07b28`.
Its new data root is `%APPDATA%\dev.fung.studio-preview-20261015`; existing
projects were not copied into it. Logs and receipt: `native-isolated-build.log`,
`native-isolated-start.json`, `native-ui-build-receipt.json` in the evidence folder.
The native window was left open for user review. Native visual/feature acceptance
and this profile's second boot remain unverified.

## Changelog

| Version | Date | Change |
|---|---|---|
| 1.0.0b | 2026-10-15 | Reference composition, real activity states, responsive dock and browser QA; existing capture owners and navigation guards retained. |
