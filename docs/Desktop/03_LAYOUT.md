---
version: "0.2.3b"
created_at: "2026-07-05T00:00:00+07:00,ATHER"
last_update: "2026-07-09T15:10:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "layout-spec"
  scope: "FUNG"
---

# 03 - Layout

Reference assets:

- `assets/wireframe-annotated.svg`
- `assets/subtract-shape.svg`

## Layout Concept - Skeuomorphic Subtract HUD

Command Deck is a single tactile porcelain panel with two intentional subtract notches:

- Top-right notch: Topbar FAB.
- Bottom-left notch: Sidebar FAB and Close FAB.

Signals D/E/F/G are not FABs. Per the 2026-07-05 RCA, Signal cards must be owned by the panel layout system as an in-panel sector. They sit in the fixed Signals sector at `x=756, y=442, w=512, h=254`. This prevents the "floating sector" failure mode where a fixed-size absolute FAB visually detaches from the rest of the deck.

The visual material is skeuomorphic: molded porcelain, pressed controls, bevel highlights, inset meters, and soft mechanical shadows. The outer Tauri window stays transparent outside the rim, but the main clipped panel must be opaque enough that background windows and text do not show through the working surface.

## Canvas and Scaling

| Item | Value |
| --- | --- |
| Design canvas | `1280 x 720` |
| Origin | top-left `(0,0)` |
| Stage scale | `scale(s)` |
| Scale formula | `s = min((vw - 24) / 1304, (vh - 24) / 744, 1.4)` |
| Transform origin | center |
| Minimum window | `1200 x 780` |

All zone coordinates use the `1280 x 720` design canvas. Implementation places zones with these coordinates, then scales the whole stage. The `24px` viewport safe margin keeps shadows, anti-aliased edges, and floating controls from clipping against the transparent window edge.

## Subtract Shape Path

Panel implementation:

- Use a `<div>` with `clip-path: url(#panelClip)` so the panel material follows the true shape.
- Draw the rim as an overlay `<path stroke>`.
- `clipPath` and `stroke` must use the same path.
- If the shape changes, update this path and regenerate `assets/subtract-shape.svg`.

```svg
M 40,12 H 800 A 20 20 0 0 1 820,32 V 54 A 20 20 0 0 0 840,74
H 1248 A 20 20 0 0 1 1268,94 V 688 A 20 20 0 0 1 1248,708
H 112 A 20 20 0 0 1 92,688 V 350 A 20 20 0 0 0 72,330
H 32 A 20 20 0 0 1 12,310 V 40 A 28 28 0 0 1 40,12 Z
```

| Notch | Opens for | Approximate coordinate |
| --- | --- | --- |
| Top-right | Topbar FAB | from `x≈820`, down `42px`, then connect to `x=840` |
| Bottom-left | Sidebar FAB and Close FAB | inset to `x≈92`, from `y≈330` downward |

No bottom-right notch is allowed while Signals are panel-owned. Reintroducing it risks recreating the RCA root cause.

## Zone Dimensions

### Inside Panel

| Zone | x | y | w | h | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| Anchor rail P1-P5 | 16 | 22 | 70 | 290 | Left column for agent anchors |
| Score header | 104 | 18 | 690 | 50 | GSI badge and scoreboard |
| Stats bar | 104 | 76 | 700 | 42 | 4 metric cells |
| Battle zone | 104 | 128 | 700 | 290 | Single-column focus workbench |
| Focus workbench | 104 | 128 | 700 | 290 | Domain focus, current item, and quick actions |
| Focus tile grid | 120 | 190 | 668 | 212 | 3 frequent-use tiles, gap `12px` |
| Agent card B | 848 | 92 | 404 | 326 | Under topbar notch |
| Sector C log | 104 | 430 | 624 | 266 | Activity/Events grid |
| Signal sector D/E/F/G | 756 | 442 | 512 | 254 | Panel-owned sector, not FAB |

### Floating FABs

| FAB | x | y | w | h | Radius |
| --- | ---: | ---: | ---: | ---: | ---: |
| Topbar A | 836 | 12 | 432 | 50 | 14 |
| Sidebar I | 14 | 342 | 64 | 306 | 16 |
| Close X | 14 | 656 | 64 | 44 | 14 |

Implementation note:

- The table above is in `1280 x 720` canvas coordinates.
- In-panel zones are children of the clipped panel, so they use these values directly.
- Floating FABs are siblings of the panel inside the `1304 x 744` stage. Their CSS positions must add the panel offset: `stageX = canvasX + 12`, `stageY = canvasY + 12`.
- Current stage-space FAB positions are Topbar `(848, 24)`, Sidebar `(26, 354)`, and Close `(26, 668)`.

## Grid Systems

| Area | Grid |
| --- | --- |
| Stats bar | 4 equal cells, `gap: 8px` |
| Battle | CSS grid `1fr`, `gap: 10px` |
| Focus workbench | Header plus tile body, `grid-template-rows: auto 1fr` |
| Focus tile grid | CSS grid `1fr 1fr 1fr`, `gap: 12px` |
| Sector C | CSS grid `1fr 1fr` for Activity and Events |
| Signal sector | CSS grid `1fr 1fr` x 2 rows, `gap: 12px` |

## RCA Layout Rule

When a sector looks "floating", check structural ownership before cosmetic changes:

1. Verify whether the element is inside the intended grid/zone system.
2. Compare `getBoundingClientRect()` with sibling sectors.
3. Look for dead reserved slots or old grid classes.
4. Fix layout ownership before shadows, blur, opacity, or notch shape.

Signals must remain an in-panel sector unless a future layout spec replaces the whole deck system.

## Responsive and Window Presets

Window is not freely resizable. Tauri uses `decorations: false` and `resizable: false`. Users choose a preset in Settings.

| Preset | Size | Use case |
| --- | --- | --- |
| Compact | `1200 x 780` | Small monitor or alongside game |
| Standard | `1280 x 800` | Default |
| Wide | `1440 x 900` | Large monitor |
| XL | `1600 x 1000` | Partial ultrawide |
| Max | `1920 x 1080` | Full-HD deck |

Every preset scales the original `1280 x 720` stage. Zone proportions remain stable and content must fit without scrollbar.

## Acceptance Criteria

- `assets/subtract-shape.svg` uses the path in this document.
- `assets/wireframe-annotated.svg` shows the simplified single-column battle zone with 3 frequent-use tiles.
- `assets/wireframe-annotated.svg` shows Signal sector as an in-panel zone, not a FAB.
- Implementation uses a clipped div for the panel shape.
- Stage scaling uses `s = min((vw - 24) / 1304, (vh - 24) / 744, 1.4)`.
- Signals D/E/F/G are structurally inside the panel layout system.
- Visual material reads as skeuomorphic tactile porcelain with pressed controls.
- Battle zone content is simplified to one focus workbench plus 3 high-frequency actions.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.3b | Synced layout spec to current implementation: 4-cell stats bar, single-column focus workbench, and 3-tile battle content. |
| 0.2.2b | Main panel opacity clarified and scale formula adds a 24px viewport safe margin. |
| 0.2.1b | Clarified floating FAB coordinate conversion from canvas-space to stage-space. |
| 0.2.0b | RCA fix: Signals moved from floating FAB/notch model to panel-owned sector; bottom-right notch removed; visual direction changed to skeuomorphic tactile HUD. |
| 0.1.0b | Initial Subtract HUD layout spec. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.3b | 2026-07-09 | beta | Synced zone and grid definitions to the simplified implemented content layout. | N/A | ATHER |
| 0.2.2b | 2026-07-06 | beta | Added main-panel readability and safe-fit scale rule. | N/A | ATHER |
| 0.2.1b | 2026-07-06 | beta | Clarified FAB stage coordinate offset to keep controls aligned with subtract notches. | N/A | ATHER |
| 0.2.0b | 2026-07-05 | beta | RCA layout correction and skeuomorphic direction. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Initial Subtract HUD layout spec. | N/A | ATHER |
