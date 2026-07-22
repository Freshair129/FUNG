---
version: "0.2.0b"
created_at: "2026-07-21T06:35:00+07:00,ATHER"
last_update: "2026-07-21T07:22:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "mobile-design-system"
  doc_type: "design-system"
  scope: "FUNG Mobile and brand presentation"
  language: "Thai"
---

# FUNG Mobile Design System

## Authority

This is the single editable source of truth for FUNG Mobile visual tokens and the generated brand/system reference. Product behaviour remains governed by `PRODUCT_UX_SPEC.md`; this document must not contradict local-first, evidence, runtime-location or MCP-consent truth.

## Brand status

Quiet Archive is the selected beta identity. The production mark is the flat vector master at `docs/Mobile/design-system/assets/quiet-archive-mark.svg`; texture and depth remain presentation-only treatments.

## Visual character

- Apple Japan hierarchy: quiet, exact and readable.
- Tactile porcelain for reading surfaces; dark ink canvas for voice/capture work.
- Compact capsule controls and an owned command dock, never a detached FAB.
- Thai-first type and ≥44 dp touch targets.
- Indigo means focused action; sage means local/confirmed; warm metal means inference pending review; red means recording or destructive action.

## Reference boundary

The attached references under `references/clony-ai-voice-face-ui-kit/` inform composition only: dark surface hierarchy, waveform playback, compact touch controls and sequential brand presentation. They do not grant use of their logo, lime/neon palette, avatars, voice-cloning claims, subscription language or generation workflow.

## Generated artifacts

Run `npm run design-system:render` after changing this document. The renderer generates:

- `docs/Mobile/design-system/index.html` — interactive tokens/components reference.
- `docs/Mobile/design-system/brand/index.html` — vertical brand/CI case study.

Run `npm run design-system:check` in validation. Do not edit generated HTML by hand.

## Structured token payload

<!-- fung-mobile-design-system:data:start -->
```json
{
  "version": "0.2.0b",
  "sourceUpdatedAt": "2026-07-21T07:22:00+07:00",
  "brand": {
    "name": "FUNG",
    "tagline": "Local-first voice intelligence",
    "markStatus": "selected-beta",
    "markStatusLabel": "Quiet Archive selected · beta identity",
    "markAsset": "assets/quiet-archive-mark.svg",
    "markInkAsset": "assets/quiet-archive-mark-ink.svg",
    "markWhiteAsset": "assets/quiet-archive-mark-white.svg",
    "appIconAsset": "assets/quiet-archive-app-icon.svg"
  },
  "themes": {
    "dark": {
      "canvas": "#28374C",
      "surface": "#35465E",
      "surfaceRaised": "#40536D",
      "ink": "#FFFFFF",
      "muted": "#D5DCE2",
      "line": "rgba(255,255,255,.16)"
    },
    "light": {
      "canvas": "#FFFFFF",
      "surface": "#FFFFFF",
      "surfaceRaised": "#D5DCE2",
      "ink": "#28374C",
      "muted": "#53637A",
      "line": "rgba(40,55,76,.16)"
    }
  },
  "semantic": {
    "focus": "#FE6A3C",
    "focusOnDark": "#FE6A3C",
    "local": "#28374C",
    "localOnDark": "#D5DCE2",
    "inferred": "#28374C",
    "inferredOnDark": "#D5DCE2",
    "record": "#FE6A3C",
    "danger": "#C94E2B"
  },
  "type": {
    "uiFamily": "Noto Sans Thai, Leelawadee UI, system-ui, sans-serif",
    "display": { "size": "32px", "lineHeight": "1.12", "weight": "650" },
    "title": { "size": "22px", "lineHeight": "1.25", "weight": "650" },
    "body": { "size": "16px", "lineHeight": "1.55", "weight": "400" },
    "label": { "size": "14px", "lineHeight": "1.3", "weight": "600" },
    "caption": { "size": "12px", "lineHeight": "1.4", "weight": "500" }
  },
  "spacing": [4, 8, 12, 16, 20, 24, 32, 40, 48],
  "radii": { "control": "14px", "surface": "20px", "dock": "28px", "voice": "999px" },
  "elevation": {
    "rest": "0 12px 34px rgba(0,0,0,.16)",
    "pressed": "inset 0 2px 7px rgba(0,0,0,.18)"
  },
  "components": [
    { "name": "Voice control", "state": "local-ready", "note": "Owned by the command dock; touch target at least 44 dp." },
    { "name": "Runtime badge", "state": "on-device", "note": "States where the action runs; never implies unavailable capability." },
    { "name": "Evidence link", "state": "inferred", "note": "Dashed Deep Blue relation remains a proposal until user confirmation." },
    { "name": "Capture control", "state": "recording", "note": "Red is reserved for live recording, stop or destructive action." }
  ],
  "brandNarrative": [
    "Brand idea",
    "Quiet Archive mark",
    "Construction and clear space",
    "Semantic colour system",
    "Thai-first type and icons",
    "Tactile material and motion",
    "Product application",
    "Reference boundary"
  ]
}
```
<!-- fung-mobile-design-system:data:end -->

## Rules that tokens cannot override

1. Mobile standalone functions remain usable when Desktop, LAN, models or MCP are unavailable.
2. AI-derived content remains visibly inferred until the user confirms it.
3. Voice, recording, storage and durable-write state retain visible non-colour indicators.
4. Landscape remains responsive Mobile UI, never a squeezed Desktop UI.

## Version Diff

| Version | Change |
| --- | --- |
| `0.1.0b` | Established the editable Mobile design-system SOT, structured payload and generated-artifact contract. |
| `0.2.0b` | Selected Quiet Archive, added flat vector masters and migrated the SOT to Deep Blue, Cold Grey, White and Salmon. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| `0.2.0b` | 2026-07-21 | beta | Selected Quiet Archive and migrated official Mobile/brand tokens to the supplied palette. | pending | ATHER |
| `0.1.0b` | 2026-07-21 | beta | Initial Mobile SOT used to generate interactive system and brand reference pages. | pending | ATHER |
