---
version: "0.1.1b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-09T15:12:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "component-spec"
  scope: "FUNG"
---

# 04 - Components

## Component Ownership

Components must live in the zone that owns their layout. Do not solve structural placement bugs with shadows, blur, or opacity.

## App Shell

| Component | Responsibility |
| --- | --- |
| Stage | Owns `1280 x 720` canvas and viewport scale. |
| Ambient background | Provides quiet material context without competing with content. |
| Panel clip | Owns subtract HUD shape and material. |
| Rim path | Draws the exact same shape as the clip path. |

## Panel Zones

| Component | Zone | Required States |
| --- | --- | --- |
| Anchor rail | P1-P5 | default, active, processing |
| Score header | Header | local-ready, recording, processing, issue |
| Stats bar | Metrics | normal, warning, unavailable |
| Battle zone | Recording/analysis summary | idle, recording, reviewing |
| Agent card B | Model/runtime panel | ready, missing model, running, failed |
| Sector C | Activity and Events | empty, populated, warning |
| Signal sector | D/E/F/G signal controls | default, active, disabled, error |

## Floating Controls

Floating controls are allowed only in notches defined by `03_LAYOUT.md`.

| Component | Rule |
| --- | --- |
| Topbar FAB | May include drag region on empty space. Buttons and badges must be no-drag. |
| Sidebar FAB | Owns primary navigation icons. |
| Close FAB | Owns close/minimize/exit action depending on desktop policy. |

## Signal Cards

Signal cards are panel-owned controls inside the Signal sector.

| Card | Meaning |
| --- | --- |
| D | Capture/recording readiness |
| E | Transcript and speaker processing |
| F | Summary and intent analysis |
| G | Export and artifact queue |

Rules:

- Use `2 x 2` grid with `12px` gap.
- Card selection may use pressed material.
- Card position must not depend on viewport directly.
- Do not reintroduce `.fab-signals`.

## Control Patterns

| Need | Component Pattern |
| --- | --- |
| Binary setting | Toggle |
| Mode selection | Segmented control |
| Numeric value | Slider, stepper, or input |
| Export format | Menu or segmented control |
| Tool action | Icon button with tooltip |
| Destructive action | Explicit confirmation |

## Data States

| State | UI Behavior |
| --- | --- |
| Loading | Reserve stable dimensions; show quiet progress. |
| Empty | Show useful action, not marketing copy. |
| Error | Explain what failed and whether local data is safe. |
| Offline | Treat local mode as normal, not degraded. |
| Model missing | Show capability gap and setup action. |

## Accessibility

- All interactive controls must have accessible labels.
- Keyboard focus must be visible.
- Transcript and controls must be readable in long sessions.
- Audio-only feedback must have visual equivalents.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.1b | Corrected the canonical layout spec reference to `03_LAYOUT.md`. |
| 0.1.0b | Added component ownership and state rules. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.1b | 2026-07-09 | beta | Updated component spec references to the current layout source-of-truth file. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added component spec. | N/A | ATHER |
