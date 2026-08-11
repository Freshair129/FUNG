---
version: "0.1.1b"
created_at: "2026-08-11T09:38:00+07:00,Agent: ATHER"
last_update: "2026-08-11T10:13:01+07:00,Agent: ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Desktop left action rail overlaps the utility dock

## Symptom

The lower buttons in the left Desktop action rail paint outside their porcelain panel and overlap the Power, cloud/account and profile controls below it.

## Evidence

- The screenshot at the 1280×800 Desktop layout shows buttons continuing below the visible `.fab-sidebar` surface.
- `src/App.tsx` currently renders 10 `.sidebar-action` buttons in `.fab-sidebar`.
- `src/styles.css` fixes `.fab-sidebar` to `height: 306px` and `grid-template-rows: repeat(6, 1fr)` with an 8 px gap.
- `.sidebar-action` is fixed at 40 px high, so 10 actions plus gaps and padding cannot fit before `.power-dock` at `top: 668px`.
- The sidebar did not contain vertical overflow, so the extra rows remained visible outside the panel.

## Root Cause

The sidebar layout contract still describes the original six-action rail. Four later account/provider actions were appended without updating the fixed six-row geometry or adding overflow containment.

## Why The Issue Escaped Detection

Build and component tests do not evaluate fixed-position geometry. No regression asserted that a growing action list remains contained inside the rail at the minimum Desktop viewport.

## Proposed Prevention

- Keep the fixed left-rail geometry, but make its action rows a contained vertical scroll region.
- Hide the narrow Chromium scrollbar so the existing 48 px buttons retain their width; mouse wheel and keyboard focus still scroll the rail.
- Add a source-level layout regression requiring vertical containment and rejecting the obsolete six-row template.

## Validation Boundary

The fix is complete after the layout regression, frontend build and operator-visible 1280×800 check pass without overlap.

## Resolution

- Replaced the obsolete six-row template with fixed 40 px auto rows.
- Contained additional actions with vertical scrolling inside `.fab-sidebar`.
- Hid the narrow scrollbar without removing wheel or keyboard scrolling.

## Verification

- `npm run test:desktop-bootstrap` — 4/4 passed, including the rail-containment regression.
- `npm run build` — passed; Vite transformed 1,748 modules.
- `npm run test:auth` — 5/5 passed.
- `npm run test:mobile` — 4/4 passed.
- `git diff --check` — passed.
- Operator-visible verification remains required at the target viewport.

## Version Diff

`0.1.0b -> 0.1.1b`: recorded the implemented containment rules and automated verification evidence.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.1b | 2026-08-11 | beta | Added the implemented containment rules and verification evidence. | pending | ATHER |
| 0.1.0b | 2026-08-11 | beta | Recorded the stale six-row sidebar geometry and containment fix. | pending | ATHER |
