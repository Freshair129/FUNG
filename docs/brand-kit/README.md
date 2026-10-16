# FUNG Brand Kit

Concept: **Quiet Archive** — a private, local-first voice capture and notes app.

The shared token authority is [fung.tokens.json](../FUNG_Design_System_v0.1.0/tokens/fung.tokens.json). It owns the approved beta palette, typography, spacing, radius, semantic themes and material values. This directory contains the logo and a generated CSS compatibility export.

## Contents

| File | Role |
| --- | --- |
| logo/fung-mark.svg | Scalable mark using currentColor. |
| logo/Quiet Archive.png | Original concept sheet; reference only. |
| tokens.css | Generated brand CSS aliases. Do not edit directly; run node scripts/design_system.mjs render. |

## Logo

The mark has no background of its own and inherits currentColor. Keep clear space of at least 25% of its width, use a minimum size of 24 px in UI and 16 px as a favicon, and do not stretch or box it. The OS app icon is the one use that places it on a tile.

The selected-beta SVG masters are in [Mobile design-system assets](../Mobile/design-system/assets/). See the [shared board](../design-system/index.html) for the generated brand and token references.

## Voice and state

Quiet, private and honest. Names should remain readable in Thai and English. Local, inferred, recording and error states need a text label or shape as well as color. The generated [contrast report](../FUNG_Design_System_v0.1.0/qa/CONTRAST_REPORT.md) covers declared solid-color pairs only; it is not whole-application accessibility certification.
