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
- [x] Run focused design-system tests, generated-file checks, documentation link/index checks and the application build.
- [x] Review the complete diff and local checks.
- [ ] Publish a PR; native Figma delivery and complete screen exports remain open until that integration is available.

## Acceptance criteria

1. `fung.tokens.json` is valid JSON using DTCG token/group conventions: every token has an explicit or inherited standard `$type` and `$value`; project metadata lives in `$extensions.com.fung.design-system`.
2. CSS keeps the existing `--fung-*` public variable names and scoped theme behavior.
3. All listed contrast checks pass from source values; stale generated CSS, report or board content is detectable without rewriting files.
4. The interactive board presents shared foundations and cross-surface guidance, has working theme/copy/print controls, and declares its source and verification boundary.
5. Existing Mobile design-system URLs continue to resolve to the new shared board.
6. No documentation claims a second editable token source or claims that native Figma screens, whole-app accessibility or component implementation are complete without evidence.

## Status

Local delivery is complete on `codex/design-system-standard-20261016`: the canonical JSON follows the DTCG 2025.10 typed token/group conventions with FUNG data in a namespaced extension; CSS, Brand Kit export, contrast report and the shared HTML/brand boards are generated from that source. Five focused design-system tests, the generated-file check, TypeScript and Vite production build passed. CI is configured to fail when generated outputs drift; hosted Actions evidence and PR publication await a push. Component coverage remains partial; Figma/Penpot source and complete screen exports remain outside this local handoff.
