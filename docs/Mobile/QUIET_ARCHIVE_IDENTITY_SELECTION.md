---
version: "0.1.1b"
created_at: "2026-07-21T07:05:00+07:00,ATHER"
last_update: "2026-07-21T08:05:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "mobile-brand-identity"
  doc_type: "selected-identity-spec"
  scope: "FUNG Quiet Archive logo and CI migration"
  language: "Thai"
---

# FUNG Quiet Archive — Selected Identity Specification

## Decision record

| Decision | Selected value |
| --- | --- |
| Logo direction | Concept C — Quiet Archive |
| Visual metaphor | A single soft folded archive ribbon around an open centre: durable capture, preserved note, and an available place to return to. |
| Core palette | Deep Blue & Grey `#28374C`, Cold Grey `#D5DCE2`, Total White `#FFFFFF`, Salmon Orange `#FE6A3C` |
| Palette reference | `docs/Mobile/references/quiet-archive-palette.png` — SHA-256 `273D3A461BD633B1F99664178F1A016ACF7CBDAB42B01FAFB54137199663975B` |
| Product claim boundary | Local-first voice capture, notes and evidence; no cloning, cloud-first, avatar or generation claim. |

## Logo master direction

The production mark must be a flat, original vector silhouette. It retains only the Quiet Archive geometry: one continuous rounded ribbon with an open internal archive space and one diagonal fold cue. Porcelain texture, depth, paper grain and cast shadow belong to presentation mockups only; they are not part of the logo master.

Required variants:

1. Deep Blue mark on Total White.
2. Total White mark on Deep Blue.
3. Single-colour Deep Blue small-size mark.
4. Adaptive Android icon: Deep Blue field, Total White mark, optional Salmon Orange notification dot only when a real signal state needs it.

The final mark must not be confused with a letter, generic chat bubble, cloud, microphone or any supplied reference logo.

## CI token migration

This palette replaces the previous indigo/sage/metal visual accent set for the selected Mobile identity. It does not override product semantics; state labels, icons and text remain mandatory.

| Role | Token | Value | Rule |
| --- | --- | --- | --- |
| Dark canvas / primary ink | `archive.ink` | `#28374C` | Default dark app/brand canvas and primary mark colour. |
| Cool neutral surface | `archive.cloud` | `#D5DCE2` | Raised surface, divider field and inactive material. Never use as small text on white. |
| Reading / reversed mark | `archive.white` | `#FFFFFF` | Reading surface, reversed logo and highest-contrast content. |
| Signal / primary action | `archive.salmon` | `#FE6A3C` | Primary action, active capture and selected focus. Must not fill the entire canvas. |
| Destructive derivative | `archive.salmon-danger` | derived darker salmon | Used only after confirmation; must pass contrast testing and retain a destructive icon/label. |
| Inferred relation | `archive.inferred` | Deep Blue outline + `Inferred` label | No new gold/green semantic colour; provenance remains textual and patterned. |
| Confirmed/local | `archive.local` | Cold Grey field + Deep Blue verified icon/label | No implication that processing is cloud-based. |

## Accessibility and semantic safeguards

1. Salmon is never the only indicator for recording, destructive action, selection or inference.
2. Recording must combine a Salmon state with timer, text and an active capture icon.
3. Inferred graph relations use a dashed Deep Blue line and `Inferred` label; confirmed relations use a solid line and verified icon.
4. The final Salmon action token and its darker destructive derivative must be contrast-tested against Deep Blue, White and Cold Grey before release.
5. Text on Cold Grey uses Deep Blue; text on Deep Blue uses White.

## Application direction

- **Brand presentation:** Deep Blue canvas, a tactile Quiet Archive mockup surface, Salmon sparingly as an action/signal detail.
- **Voice/capture:** Deep Blue work canvas; Salmon is the active record/action point; timer and durable-write truth remain visible.
- **Notes/graph:** White reading canvas, Cold Grey dividers/sheets, Deep Blue content and relation structure; Salmon indicates focus, not evidence truth.
- **Landscape Mobile:** retain the responsive Mobile rail and full-bleed surface; this identity must not introduce Desktop framing.

## Implemented identity boundary

1. Editable SVG masters now live in `docs/Mobile/design-system/assets/` as the Deep Blue, Total White and Android icon variants.
2. `DESIGN_SYSTEM.md` is the selected-beta source of truth for the master assets and palette mapping.
3. The interactive design-system and brand pages are regenerated from that Markdown SOT before each export.
4. Mobile CSS consumes the palette only through the selected CI tokens; runtime, GenesisBlockDB, recording behaviour, model packaging and MCP permission logic remain unchanged.

## Acceptance criteria

1. The final mark is an original, flat vector with valid small-size variants.
2. The four supplied core values are traceable in the SOT and generated brand pages.
3. All primary and destructive states remain distinguishable without colour alone.
4. Generated HTML and long-image export use the same SOT version.
5. The app retains local-first, provenance and evidence semantics under the new CI.

## Version Diff

| Version | Change |
| --- | --- |
| `0.1.0b` | Records the selected Quiet Archive direction and proposed CI mapping from the supplied palette. |
| `0.1.1b` | Promotes the approved identity to beta and records the SVG/SOT/CSS implementation boundary. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| `0.1.0b` | 2026-07-21 | candidate | Selected Quiet Archive and documented palette-driven CI migration. | pending approval | ATHER |
| `0.1.1b` | 2026-07-21 | beta | Added vector master variants and wired approved palette tokens into the SOT and Mobile surface. | pending verification | ATHER |
