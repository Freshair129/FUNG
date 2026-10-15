# FUNG dashboard layout QA

final result: passed

Date: 2026-10-15. Scope: reference-led desktop layout adaptation, issue #82.
This is browser layout/interaction acceptance, not native audio, model, installer,
or release acceptance.

## Reference and comparison

- User reference: `codex-clipboard-58758952-5c53-4e0d-9e8b-d77fb8490757.png`
  (958 × 663).
- Render: `http://127.0.0.1:1421/app?surface=desktop`, same 958 × 663 viewport.
- Both reference and rendered screenshot were inspected together in one image
  comparison input. Evidence: `home-reference-size.png` under
  `G:\AI-Studio\verification\2026-10-15-fung-dashboard`.
- Retained composition: approximately 55% photographic hero, two upper-right
  cards, one wide lower-right card, rounded black frame and bottom navigation.
  Fitness content is intentionally adapted to the existing FUNG product.

## Surface audit

| Surface | Result |
|---|---|
| Typography | Single-line Thai hero at reference viewport; existing DM Sans/IBM Plex Sans Thai retained. Distinct heading, metadata and dock scales; no clipped headline. |
| Spacing and layout | 5px card gutters, rounded cards, four hero shortcuts and fixed bottom dock. Main owns page scrolling; no sidebar or document scrollbar. |
| Color | Blue/lavender hero, white cards and black frame follow the reference hierarchy. Dark appearance preserves readable card backgrounds and text. |
| Imagery | Generated raster listener portrait and transparent waveform; existing FUNG mark and Lucide icons. Waveform is decorative, not an audio-level claim. |
| Copy and data | Existing FUNG actions in Thai. Project totals and dated activity use actual props. Missing native data stays unknown/empty; no invented progress or assistant response. |

## Iterations

1. Initial render exposed a two-line headline and oversized account control.
   Corrected headline, profile selector and dock label specificity.
2. Collapsing outer margins shifted the frame upward by 8/16px after navigation.
   Root formatting context and viewport containment fixed this. Re-measured all
   four sizes after the fix.
3. Guard focus from closed utility menus now returns to their visible summary;
   the synthetic capture flow remained on Home after Stay.

No unresolved P0, P1 or P2 findings in this bounded browser layout review.

## Final geometry

| Viewport | Document size | Frame top | Main client / scroll height |
|---|---|---|---|
| 1280 × 800 | 1280 × 800 | 16 | 699 / 699 |
| 958 × 663 | 958 × 663 | 16 | 562 / 562 |
| 800 × 600 | 800 × 600 | 16 | 499 / 1110 |
| 390 × 844 | 390 × 844 | 8 | 717 / 1110 |

Narrow views stack cards and scroll main; the dock stays visible. A capture strip
uses its own row, so short windows may also scroll main. Evidence in the directory
above: `final-geometry.json`, `home-desktop.png`, `home-mobile-final.png`,
`capture-fixture.png`, `guard-focus.json`.

## Interaction and accessibility

- Home/live/review/output navigation; CTA opens Live without starting audio.
- Companion opens/closes via existing panel and restores trigger focus.
- Settings opens/closes; project disclosure opens/closes, Escape returns focus.
- Utility menu opens Appearance; dark/light checked and restored light.
- 7/30-day selection updates period. Isolated synthetic project fixture shows
  seven meters with 12 projects; fixture is excluded from production.
- Capture fixture opens existing guard; Stay preserves Home. Utility-menu guard
  returns focus to visible summary after Stay.
- Focus treatment and semantic links, buttons, selects and meters retained.

Fresh navigation produced no new console errors. Four historical Vite HMR
`createRoot` warnings occurred during development and did not repeat during the
fresh-page check. Native-only data is unavailable in browser; these checks do
not prove native recording or model execution.
