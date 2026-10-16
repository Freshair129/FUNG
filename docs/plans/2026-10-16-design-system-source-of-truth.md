# Design system source of truth and board

## Goal

Consolidate FUNG's brand, semantic, typography, spacing, layout, motion and theme tokens into one portable source, then derive runtime CSS and the interactive design board from it. Reconcile Mobile, Desktop, Brand Kit and redesign documentation so each points to the same authority.

## Approved constraints

- Preserve the approved Quiet Archive beta identity and the existing shared token values unless a validation defect requires a documented correction.
- Local-first, evidence and inferred-state semantics remain explicit; color alone must not communicate status.
- Mobile touch targets remain at least 44 dp. Desktop shell material and navigation follow the current Liquid Glass shell decision.
- Token integration status, component implementation status, accessibility evidence and full-screen design delivery remain separate claims.
- Generated CSS, board HTML and compatibility references are outputs; do not edit them as independent token sources.

## Tasks

- [x] Record the current state and source conflicts.
- [x] Migrate the canonical token file to the Design Tokens Format Module 2025.10 structure and a namespaced FUNG extension for metadata, contrast pairs and board content.
- [x] Validate supported token types, color encoding, aliases and generated CSS/contrast outputs.
- [x] Generate the cross-surface HTML board and brand presentation from the canonical token file.
- [x] Keep former Mobile board URLs as compatibility redirects and make Mobile/Desktop/Brand Kit specs point to the shared source.
- [x] Reconcile the docs index, interface inventory, redesign brief, design-system README and package check against actual delivery evidence.
- [x] Gate generated token, contrast, board and compatibility outputs with `design-system:check` in CI.
- [x] Add theme-aware component specimens for primary/secondary actions and the required semantic states.
- [x] Generate a source-derived component token coverage inventory and include it in `design-system:check`.
- [x] Record bounded Desktop Home accessibility-tree and sequential keyboard evidence with its limits.
- [x] Capture current production-preview Home, skip-link focus and skip-link activation screenshots; record the observed scroll regions and runtime boundary in the accessibility review.
- [x] Run focused design-system tests, generated-file checks, documentation link/index checks and the application build.
- [x] Review the complete diff and local checks.
- [x] Migrate production stylesheet flat semantic colors to canonical tokens; remove duplicate fallback values for every canonical `--fung-*` token in application stylesheets; the generated inventory now finds canonical references in all 24/24 application stylesheets and zero active flat single-color literal declarations. Embedded gradients, shadows, translucent materials and illustration colors remain inventoried for contextual visual review.
- [ ] Complete visual, assistive-technology, zoom, forced-color, reduced-motion and native accessibility acceptance.
- [ ] Deliver the native Figma/Penpot library and complete screen/wireframe exports when that integration is available.
- [ ] Publish a PR; hosted CI evidence requires a push.

## Acceptance criteria

1. `fung.tokens.json` is valid JSON using DTCG token/group conventions: every token has an explicit or inherited standard `$type` and `$value`; project metadata lives in `$extensions.com.fung.design-system`.
2. CSS keeps the existing `--fung-*` public variable names and scoped theme behavior.
3. All listed contrast checks pass from source values; stale generated CSS, report or board content is detectable without rewriting files.
4. The interactive board presents shared foundations and cross-surface guidance, has working theme/copy/print controls, and declares its source and verification boundary.
5. Existing Mobile design-system URLs continue to resolve to the new shared board.
6. No documentation claims a second editable token source or claims that native Figma screens, whole-app accessibility or component implementation are complete without evidence.
7. The board renders token-driven component specimens and makes clear that they are examples, while generated QA reports identify production component coverage and accessibility evidence separately.

## Status

The initial source-of-truth and token-board slice is committed on `codex/design-system-standard-20261016` at `11f2332`; follow-up commits add theme-aware specimens, generated coverage, stylesheet migration and scoped Home accessibility evidence. The current checkout has three local commits ahead of `origin/main`. All 24/24 application CSS files reference canonical tokens, canonical token fallback values are not duplicated in application CSS, and the source inventory finds zero active flat single-color literal declarations. A current production-preview audit now records three 1280×720 Home screenshots, visible focus, keyboard reachability and successful skip-link activation; it also records two independent scroll regions and the fact that native project state is unavailable in the preview. These observations do not prove complete component mapping or rendered theme parity. Full route/theme/assistive-technology, zoom, forced-color, reduced-motion, mobile and native accessibility acceptance remains open, as do native Figma/Penpot source and complete screen exports. Hosted Actions evidence requires a push.
