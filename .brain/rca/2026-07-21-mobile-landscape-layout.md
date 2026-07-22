---
version: "0.1.1b"
created_at: "2026-07-21T05:20:00+07:00,ATHER"
last_update: "2026-07-21T05:35:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "mobile-ui"
  doc_type: "root-cause-analysis"
  scope: "FUNG Mobile Android landscape layout"
  language: "Thai"
---

# RCA — Landscape UI renders as a clipped portrait surface

## Classification

| Item | Value |
| --- | --- |
| Complexity | C-2 — Cross-module UI repair |
| Change risk | MEDIUM — shared mobile shell, navigation and dense editor surfaces |
| Device evidence | Samsung SM-A075F, Android 16, 1600×720 landscape |

## Symptom

When the physical Android device is rotated to landscape, FUNG renders a narrow portrait card in the middle of the display. The lower content is obscured by the floating bottom navigation, and the wide display is not used for the timeline/editor workflow.

## Evidence

- `output/uat-android-genesis/landscape-before.png` shows the Home screen limited to a portrait-width surface while the Android display is 1600×720.
- `output/uat-android-genesis/landscape-before.xml` reports `rotation="1"` and root bounds `[0,0][1600,720]`.
- `src/mobile/mobile.css` sets `.m-app { height: 100dvh; max-width: 520px; overflow: hidden; }` for every orientation.
- The stylesheet has a desktop-width media query but no `orientation: landscape` mobile rule.
- `src/main.tsx` selects `MobileApp` only when `window.matchMedia("(max-width: 760px)")` matches. On the 1600px-wide landscape device it instead renders `App.tsx`, the Desktop workspace.
- `docs/Mobile/PRODUCT_UX_SPEC.md` explicitly marks landscape editor layouts as out of scope for the first concept pass, so there is no implemented landscape contract.

## Root Cause

The mobile surface selection is width-only. At landscape phone dimensions, FUNG selects the Desktop workspace instead of `MobileApp`. If MobileApp is forced, its shared shell also applies a portrait maximum width and vertical composition. Both defects must be repaired: route selection first, then the landscape composition and navigation clearance.

## Why It Escaped Detection

Prior physical UAT covered portrait only; `docs/Mobile/ANDROID_PHYSICAL_UAT_2026-07-20.md` records landscape as not yet proven. The frontend CSS had no landscape viewport test or acceptance criterion.

## Proposed Prevention

- Add landscape phone visual checks at 1600×720 and a compact landscape size to the physical UAT matrix.
- Make the shared shell choose an explicit portrait or landscape composition rather than relying on a portrait max-width at every aspect ratio.
- Verify Home, capture, Timeline/Story/Processing and Devices at each orientation, including navigation clearance and horizontal overflow.

## Proposed Repair Boundary

The repair requires `src/main.tsx`, `src/mobile/mobile.css` and a landscape UAT record:

1. Select `MobileApp` for phone-like dimensions based on the smaller viewport dimension, including 1600×720 landscape; preserve an explicit `?surface=desktop` override.
2. At phone landscape heights, make the app use the full available width and remove the portrait-card framing.
3. Replace the bottom dock with a compact left-side rail so vertical content keeps a usable viewport.
4. Reserve rail clearance in `.m-screen`, detail layers and sheets; retain safe-area insets.
5. Recompose Home and capture as two-column layouts; make Timeline/Story/Processing use the wider track area without clipping.
6. Preserve the existing portrait and explicit desktop layouts unchanged.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-21 | candidate | Evidence-backed landscape UI RCA and bounded repair proposal | pending approval | ATHER |
| 0.1.1b | 2026-07-21 | need review | Added confirmed root-surface selection defect and expanded repair boundary | pending approval | ATHER |
| 0.1.1b | 2026-07-21 | beta | Approved correction implemented and verified on Android landscape | pending | ATHER |
