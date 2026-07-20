---
version: "0.1.0b"
created_at: "2026-07-06T02:30:00+07:00,ATHER"
last_update: "2026-07-06T02:30:00+07:00,ATHER"
status: "beta"
attributes:
  domain: "frontend-layout"
  doc_type: "rca"
  scope: "FUNG"
---

# RCA - Transparent HUD Layout

## Symptom

- Outer window shows a rectangular background instead of transparent area.
- Sidebar and topbar feel misaligned with the subtract notches.
- Sectors look uneven and visually submerged.
- Clicking topbar empty space does not drag the Tauri window.

## Evidence

- `src-tauri/tauri.conf.json` used `decorations: false` but did not set `transparent`.
- `index.html` and `.app-shell` painted full rectangular backgrounds.
- `.fab-topbar` had `data-tauri-drag-region`, but CSS did not set `-webkit-app-region: drag`.
- FAB positions matched the old wireframe loosely, while visible notch/FAB borders had different visual weights.
- Surface alpha and shadow values were too soft against the warm background, reducing contrast.

## Root Cause

The app mixed a shaped HUD visual model with a normal rectangular webview background. The panel was clipped, but the window/webview behind it remained opaque. Drag behavior was marked in markup but not enabled at the CSS app-region layer. FABs and panel sectors also used independent visual treatments, so their edges appeared offset even when their coordinates were close.

## Why It Escaped Detection

Previous validation used browser screenshots, where true OS-level transparent window behavior cannot be proven. Browser rendering also hides Tauri drag-region behavior.

## Proposed Prevention

- Treat transparent HUD as a Tauri-window contract, not just CSS.
- Keep root HTML/body/app shell transparent.
- Use `-webkit-app-region: drag` for intended drag surfaces and `no-drag` for controls.
- Validate both browser geometry and Tauri runtime for transparent/drag behavior.

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-06 | beta | Initial RCA for transparent HUD and drag fixes. | N/A | ATHER |
