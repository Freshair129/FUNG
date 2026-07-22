---
version: "0.1.1b"
created_at: "2026-07-21T05:45:00+07:00,ATHER"
last_update: "2026-07-21T06:42:00+07:00,ATHER"
status: "superseded"
superseded_by: "docs/Mobile/DESIGN_SYSTEM.md v0.1.0b"
attributes:
  domain: "mobile-design-system"
  doc_type: "design-token-proposal"
  scope: "FUNG Mobile visual refresh inspired by Clony reference"
  language: "Thai"
---

# FUNG Mobile — Voice App Visual Token Proposal

## 1. Authority and Reference Boundary

| Item | Decision |
| --- | --- |
| Complexity | C-2 — cross-surface visual-system refinement |
| Risk | MEDIUM — shared token, navigation and component appearance changes |
| Reference | UI8 Clony AI Voice & Face App UI Kit, supplied by product owner |
| Reuse boundary | interaction grammar and composition only; no copied assets, copy, logo, product claims, colours or face/voice cloning workflows |
| FUNG authority | `PRODUCT_UX_SPEC.md`, `CONCEPT_REVIEW.md`, `DESIGN_SYSTEM.md` remain authoritative for functionality, provenance and CI |

## 2. Reference Patterns to Adapt

The visual review found a voice-first mobile grammar built from:

1. an immersive dark canvas with one clear primary voice/identity object;
2. a strong hero action rather than a dense dashboard;
3. compact capsule controls and rounded sheets with clear touch targets;
4. a small persistent navigation dock; and
5. simple workflow cards that give one next action at a time.

FUNG adopts these as layout and hierarchy patterns. It does **not** adopt the reference neon-green palette, marketing copy, avatars, clone-anyone framing, subscriptions or social-video workflows.

## 3. FUNG CI Token Set

### Core colour tokens

| Token | Value | Use |
| --- | --- | --- |
| `canvas.dark` | `#171918` | immersive dark voice/capture canvas |
| `canvas.light` | `#FAF8F3` | porcelain reading canvas |
| `surface.dark` | `#202321` | raised dark surface and dock |
| `surface.porcelain` | `#FDFBF7` | cards, sheets and controls in light mode |
| `ink.on-dark` | `#F2EFE8` | primary text on dark canvas |
| `ink.primary` | `#242423` | primary text on porcelain |
| `graphite.muted` | `#A9AAA5` / `#77746F` | secondary text by theme |
| `indigo.focus` | `#7888B7` / `#4A5B8B` | selected state, primary intent and voice focus |
| `sage.local` | `#8FA88F` / `#6F897E` | local-safe, confirmed and connected state |
| `metal.inferred` | `#C39A4A` / `#A47E43` | inference awaiting review |
| `signal.record` | `#D83A32` | active recording, stop and destructive warning only |
| `line.quiet` | `rgba(242,239,232,.11)` / `rgba(75,70,61,.14)` | low-contrast boundaries by theme |

Dark/light pairs are existing FUNG CI values; green is deliberately excluded.

### Material and elevation tokens

| Token | Value | Use |
| --- | --- | --- |
| `radius.control` | `14px` | compact buttons, chips and inputs |
| `radius.surface` | `20px` | workflow cards and sheets |
| `radius.dock` | `28px` | mobile dock/landscape rail |
| `radius.voice` | `999px` | voice capture object only |
| `elevation.rest` | soft 1px line + `0 12px 34px rgba(0,0,0,.16)` dark / warm shallow shadow light | card separation |
| `elevation.pressed` | inset shadow only | selected/active controls; never moves layout |
| `spacing` | `4, 8, 12, 16, 20, 24, 32, 40, 48` | all surface rhythm |

## 4. Component Mapping

| Reference grammar | FUNG implementation |
| --- | --- |
| central voice/identity hero | FUNG’s primary voice command/capture control; shows local state, never a cloned identity |
| dark full-bleed canvas | dark FUNG theme for voice and capture; porcelain mode remains available for reading-heavy views |
| large rounded action | indigo primary action with Thai label and a ≥44dp target |
| compact chips | runtime location, model availability, local-safe and evidence states — never fake capability |
| bottom dock | FUNG’s five destinations; landscape adapts this to the responsive rail already defined in `LANDSCAPE_UI_REPAIR_SPEC.md` |
| sheet/workflow card | one next user action plus source/evidence/provenance where a result is AI-derived |

## 5. Surface Direction

- **Voice Home:** dark immersive canvas, one indigo/sage focal control, two practical next actions, concise recent-work list.
- **Capture:** dark high-focus surface; recording red is reserved for live/stop state; safety and durable-chunk state remain visible.
- **Notes/Graph/Devices:** porcelain reading surfaces with stronger card hierarchy, capsule filters and calm empty states.
- **Timeline/Story/Processing:** retain tactile DAW functionality, but use a dark work canvas with porcelain inspector sheets where it improves sustained reading.
- **MCP/AI:** always show local/Desktop/runtime truth and consent. Reference-style polish cannot hide unavailable models or permissions.

## 6. Acceptance Criteria

1. No reference brand, asset, copy, neon green or cloning claim appears in FUNG.
2. All mobile surfaces use the FUNG semantic colour tokens above in light and dark themes.
3. Primary voice/capture action remains visually dominant without obscuring standalone-core work.
4. Touch targets remain ≥44dp and Thai labels remain readable at phone portrait and landscape sizes.
5. The responsive landscape rail and full-bleed canvas remain intact.
6. AI/MCP/provenance states remain truthful and distinguish local, Desktop and unavailable states.

## 7. Implementation Boundary

After approval, implementation may modify `src/mobile/mobile.css` and the existing Mobile components only for token application, component hierarchy and responsive presentation. It must not change data schemas, GenesisDB, model runtime, MCP permissions or recording behaviour.

## Supersession

The approved values and reference boundary in this proposal have been consolidated into `docs/Mobile/DESIGN_SYSTEM.md`. That document is now the only editable Mobile token source. This file remains as decision provenance and must not be edited as a competing token set.

## Version Diff

### `0.0.0` → `0.1.0b`

- Proposes a reference-informed FUNG Mobile visual token system using only FUNG CI colours and local-first product semantics.
- `0.1.1b`: Superseded by the approved Markdown SOT.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.1b | 2026-07-21 | superseded | Consolidated approved tokens and reference boundaries into `DESIGN_SYSTEM.md` SOT. | pending | ATHER |
| 0.1.0b | 2026-07-21 | candidate | Clony-inspired composition and FUNG CI token proposal | pending approval | ATHER |
