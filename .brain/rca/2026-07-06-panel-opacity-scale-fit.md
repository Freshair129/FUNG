---
version: "0.1.0b"
created_at: "2026-07-06T05:25:00+07:00,Agent: ATHER"
last_update: "2026-07-06T05:25:00+07:00,Agent: ATHER"
status: "beta"
attributes:
  domain: "layout"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Main layer is too transparent and HUD edges clip at runtime

## Symptom

The main HUD layer allows background text to show through, making content hard to read. Some floating or edge-adjacent controls can look clipped or overfit against the window edge.

## Evidence

- User screenshots show text from another app visible through the central panel material.
- `.panel-glass` used semi-transparent gradients with alpha around `0.76-0.86`.
- Cards and FABs also used partially transparent material.
- `useStageScale()` fit the stage exactly to `window.innerWidth / 1304` and `window.innerHeight / 744`, leaving no visual safe margin for shadows, antialiasing, and floating controls near the edge.

## Root Cause

Two independent layout/material assumptions conflicted:

1. The outer Tauri window should be transparent, but the main clipped panel should not be visually transparent enough to leak background text.
2. Stage scaling used a hard edge fit, so visual effects and edge FABs had no guard space.

## Why The Issue Escaped Detection

Earlier fixes focused on making the outside of the HUD transparent and aligning FAB coordinates. They did not separately define opacity rules for the main panel layer, and browser/build checks do not show real desktop background bleed-through.

## Fix

- Keep `body` and `.app-shell` transparent.
- Make `.panel-glass`, zones, cards, FABs, and key controls materially opaque enough to read over any desktop background.
- Add a `24px` viewport safe margin to the stage scale calculation so the deck fits with breathing room instead of touching the window edge.

## Proposed Prevention

- Treat transparency as two separate contracts: outside-rim transparency and inside-panel readability.
- Validate edge controls with desktop screenshots after scale changes.
- Keep a scale safe margin in the layout spec.

## Version Diff

| Version | Change |
|---------|--------|
| 0.1.0b | Initial RCA for readable main panel opacity and safe-fit scaling. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-06 | beta | Added RCA for panel opacity and scale safe margin issue. | N/A | ATHER |
