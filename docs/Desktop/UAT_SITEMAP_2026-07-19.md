---
version: "0.1.0b"
created_at: "2026-07-19T00:00:00+07:00,ATHER"
last_update: "2026-07-19T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "uat-report"
  scope: "FUNG sitemap and Meeting Mode"
---

# Sitemap and Meeting Mode UAT

## Scope and Environment

| Item | Value |
| --- | --- |
| Build under test | `npm run build` output served by `npm run preview` |
| URL | `http://127.0.0.1:4173/` |
| Browser | Playwright CLI, Chromium |
| Viewport | 1280 x 720 and 1200 x 780 |
| Reference | `docs/05-sitemap-ia.md`, `docs/07-meeting-mode.md` |

The in-app Browser could not render localhost despite the dev server returning resources. UAT therefore used the production `dist` preview through Playwright. This is a browser-tooling limitation, not a product-runtime finding.

## Result

**UI sitemap UAT: PASS.**

**End-to-end product-workflow UAT: NOT READY.** The current beta intentionally has incomplete recording, export, and several runtime-backed workflows; those are tracked in `docs/08-real-progress.md` and are not represented as passed simply because the HUD controls render.

## Executed Cases

| ID | Scenario | Expected result | Result |
| --- | --- | --- | --- |
| UAT-SM-01 | Load the preview build | FUNG review workspace renders, no framework overlay | Pass |
| UAT-SM-02 | Open P1 | Capture and Library, exactly three P1 focus tiles, Capture console, and Capture events appear | Pass |
| UAT-SM-03 | Select P1 Live capture tile | Active tile, Agent card, Sector C, and focus signal change together to Live capture | Pass |
| UAT-SM-04 | Open P2 | Transcript and Speakers, exactly three review tiles, Review workspace, and Review events appear | Pass |
| UAT-SM-05 | Use topbar Summary segment | P3 renders Meeting recap, People and intent, and Action register | Pass |
| UAT-SM-06 | Use topbar Runtime segment | P4 renders Export bundle, Privacy and policy, and Runtime and archive | Pass |
| UAT-SM-07 | Toggle recording control | New recording control changes to active state | Pass |
| UAT-SM-08 | Toggle light/dark theme | Global theme state changes without console error | Pass |
| UAT-SM-09 | Render at minimum desktop viewport | HUD remains visible at 1200 x 780 without a framework overlay or console warnings/errors | Pass |
| UAT-SM-10 | Select New project in browser preview | User receives a visible new-project confirmation and the project is retained | Fail — browser-preview fallback does not persist/list projects, so no visible confirmation appears |
| UAT-SM-11 | Execute real recording/import/export lifecycle | Runtime writes durable data and produces output artifacts | Not ready — outside the currently implemented runtime baseline |

## Findings

### UAT-01 — Browser-preview New action has no user-visible completion state

**Severity:** Medium for browser/demo UAT; not yet a packaged-Tauri blocker.

In browser preview, the `New` action creates a fallback project object, but refresh reads an empty fallback project list. The user therefore receives no visible project name or success state. This is reproducible at P4 or any page via the topbar `New` control.

**Acceptance needed for closure:** persist preview projects in browser storage or show a transient confirmation with the created session name; separately repeat in packaged Tauri with SQLite enabled.

### UAT-02 — Runtime lifecycle UAT remains gated by known implementation scope

The sitemap correctly presents Record, Import, Export, Runtime, and Archive as command surfaces, but the current build does not yet implement the real microphone capture, durable export pipeline, or complete runtime flow. These controls must remain labelled as preview/demo state until the related milestones in `08-real-progress.md` are complete.

## Evidence

- P1 Live capture selection updated the active focus, Agent card, Sector C events, and focus signal in one UI state transition.
- P3 and P4 were reached through the topbar segments, proving the topbar mirror and P rail share the same page-state model.
- The P4 screen at 1200 x 780 rendered the three prescribed tiles and fixed sectors without console warnings or errors.

## Exit Criteria

| Criterion | Status |
| --- | --- |
| P1–P4 sitemap surfaces render with their prescribed three tiles | Pass |
| Active tile synchronizes its dependent fixed zones | Pass |
| Topbar mirrors P1–P4 page selection | Pass |
| Desktop minimum viewport visual smoke | Pass |
| Browser-preview New project confirmation | Open |
| Packaged runtime recording/import/export UAT | Blocked by implementation scope |

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial sitemap and Meeting Mode UAT result with explicit UI/runtime evidence boundary. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-19 | beta | Added sitemap UAT execution record and findings. | N/A | ATHER |
