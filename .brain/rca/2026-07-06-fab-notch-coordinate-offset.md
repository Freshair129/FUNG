---
version: "0.1.0b"
created_at: "2026-07-06T05:05:00+07:00,Agent: ATHER"
last_update: "2026-07-06T05:05:00+07:00,Agent: ATHER"
status: "beta"
attributes:
  domain: "layout"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: FABs do not align with subtract notches

## Symptom

Topbar, sidebar, and close FABs appear visually offset from the subtract-shape notches.

## Evidence

- `docs/03_LAYOUT.md` defines floating FAB coordinates on the `1280 x 720` design canvas.
- `.panel-glass` is placed at `left: 12px; top: 12px` inside the larger `1304 x 744` stage.
- In-panel zones are children of `.panel-glass`, so their canvas coordinates resolve correctly.
- FABs are siblings of `.panel-glass`, so their absolute coordinates resolve against `.stage`, not the panel canvas.
- Current FAB CSS used canvas coordinates directly, missing the panel offset.

## Root Cause

The implementation mixed two coordinate spaces:

- panel children: canvas coordinates relative to `.panel-glass`
- floating FABs: stage coordinates relative to `.stage`

Floating FABs therefore need `+12px` on both x and y compared with the documented canvas coordinates. Without that offset, they drift up and left from the matching notch geometry.

## Why The Issue Escaped Detection

Previous checks focused on build success and transparent-window behavior. They did not compare FAB bounding boxes against the subtract path in the rendered stage coordinate system.

## Fix

Apply the stage offset to all floating FABs:

- Topbar: `(836, 12)` canvas -> `(848, 24)` stage
- Sidebar: `(14, 342)` canvas -> `(26, 354)` stage
- Close: `(14, 656)` canvas -> `(26, 668)` stage

## Proposed Prevention

- Document that floating FAB CSS positions are stage-space coordinates derived from canvas coordinates plus the panel offset.
- Validate rendered FAB `getBoundingClientRect()` values against expected stage-space positions after layout edits.

## Version Diff

| Version | Change |
|---------|--------|
| 0.1.0b | Initial RCA for FAB notch offset caused by mixed coordinate spaces. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-06 | beta | Added RCA for FAB notch alignment mismatch. | N/A | ATHER |
