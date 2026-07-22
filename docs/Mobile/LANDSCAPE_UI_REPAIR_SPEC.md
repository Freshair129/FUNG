---
version: "0.1.1b"
created_at: "2026-07-21T05:20:00+07:00,ATHER"
last_update: "2026-07-21T05:35:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "mobile-ui"
  doc_type: "repair-spec"
  scope: "FUNG Mobile Android phone landscape"
  language: "Thai"
---

# FUNG Mobile — Landscape UI Repair Specification

## Authority and Scope Change

This candidate repair extends the first-pass portrait boundary in `PRODUCT_UX_SPEC.md` only for Android phone landscape. It does not introduce a tablet product layout, desktop mode, new feature route, or new data/runtime behavior.

The selection rule must treat a landscape phone as Mobile. The Desktop workspace remains available only through the explicit `?surface=desktop` override (or a non-phone viewport), and Mobile remains explicitly forceable with `?surface=mobile`.

## Goals

- Use the full landscape phone viewport without a centered portrait card.
- Keep every primary action reachable, with no content hidden behind navigation.
- Give Timeline, Story and Processing the extra horizontal space they need.
- Preserve the approved portrait composition and the existing light/dark visual language.

## Layout Contract

| Surface | Portrait | Landscape phone (`orientation: landscape`, `max-height: 760px`) |
| --- | --- | --- |
| App shell | centered, max 520px wide | full viewport width and height; no card radius/margin |
| Root surface | Mobile when viewport width ≤760px | Mobile when the shorter viewport dimension ≤760px; Desktop only when explicitly requested or outside phone dimensions |
| Navigation | bottom dock | 72px left rail, vertically centered, safe-area aware |
| Content | bottom dock clearance | left-rail clearance; normal vertical scroll remains available |
| Home | vertical voice orbit | header/status plus smaller voice orbit and quick actions in two columns |
| Capture | vertical stage | stage and durable-state card share horizontal space; controls remain ≥44px |
| Timeline/Story/Processing | portrait controls and lanes | horizontal track receives remaining width; labels retain readable minimum widths |
| Sheet/detail | bottom sheet/full overlay | bounded centered overlay with no rail overlap |

## Acceptance Criteria

1. On 1600×720 landscape, Home uses the available width and does not show a portrait-width max-width card.
2. Navigation does not cover any interactive or scrollable content.
3. All rail buttons meet a 44px minimum touch target and Thai labels remain readable.
4. Timeline, Story and Processing tracks are wider than their portrait rendering and have no horizontal clipping outside their intended track viewport.
5. Rotating back to portrait restores the existing layout with no visual or interaction regression.
6. Light and dark themes both pass the same layout checks.
7. A 1600×720 Android landscape phone renders `MobileApp`; it never auto-switches to `App.tsx` Desktop workspace.

## Verification Plan

- Physical Android: Samsung SM-A075F, portrait and 1600×720 landscape.
- Rendered checks: home, capture, timeline, story/processing and devices in light and dark themes.
- Evidence: screenshot, UI hierarchy bounds, no console/framework error, one navigation interaction in each orientation.

## Out of Scope

- Tablet-specific information architecture.
- New orientation-dependent data models or navigation destinations.
- Any recording, GenesisDB, AI runtime or MCP behavioral change.

## Version Diff

### `0.0.0` → `0.1.0b`

- Introduces the candidate Android phone landscape repair contract following physical-device evidence.

### `0.1.0b` → `0.1.1b`

- Corrected the root-surface selection rule after physical UAT proved a landscape phone was rendering the Desktop workspace.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-21 | candidate | Candidate landscape layout contract and verification plan | pending approval | ATHER |
| 0.1.1b | 2026-07-21 | need review | Adds Mobile surface-selection contract for phone landscape | pending approval | ATHER |
| 0.1.1b | 2026-07-21 | beta | Corrected responsive Mobile landscape implementation verified on device | pending | ATHER |
