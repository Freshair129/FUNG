# FUNG Brand Kit

Concept: **Quiet Archive** — a private, local-first voice capture and notes app.
This folder is the source of truth for the identity: logo, colour, type, spacing, and voice.

## Contents

| File | What it is |
| --- | --- |
| `logo/fung-mark.svg` | The mark, scalable, `fill: currentColor` (tint per surface). Primary logo asset. |
| `logo/Quiet Archive.png` | Original concept sheet (paper-fold render + explorations). Reference only. |
| `tokens.css` | Brand tokens (colour, type, spacing, radius) as CSS custom properties. |

## Logo

A single porcelain sheet rolled into an aperture — reads as an **O** (open, listening) and as an archive (kept, folded, private).

- **No background of its own.** The mark is one shape that takes `currentColor`: **ink `#171918`** on light surfaces, **porcelain `#FAF8F3`** on dark. Never lock it inside a filled box.
- **Clearspace** ≥ 25% of the mark's width on every side.
- **Minimum size** 24px in UI, 16px as a favicon.
- **Misuse:** don't box it, don't recolour it with a brand hue (ink or porcelain only), don't stretch or distort it.
- The one place a container is correct is the **OS app icon**, where the mark sits on an ink tile.

Use the SVG anywhere it can scale; it inherits text colour, so `color: var(--fung-porcelain)` on a dark parent tints it.

## Colour

| Token | Hex | Role |
| --- | --- | --- |
| `--fung-porcelain` | `#FAF8F3` | Primary ground & material |
| `--fung-ink` | `#171918` | Text, dark surfaces, app-icon tile |
| `--fung-sage` | `#6F897E` | Brand accent, wordmark, positive / confirmed |
| `--fung-slate` | `#4A5B8B` | Interactive: primary action, links, focus |
| `--fung-metal` | `#9A8260` | Caution / pending (semantic extension) |
| `--fung-clay` | `#B0553F` | Critical / error only (semantic extension) |

Porcelain and ink do most of the work. Sage and slate are used sparingly — one accent per view.

## Typography

- **Fraunces** — display & concept (serif).
- **IBM Plex Sans Thai** — Thai interface. Leads in-product.
- **DM Sans** — Latin interface & the wordmark.
- **IBM Plex Mono** — data, labels, timers, tokens.

**Wordmark:** `FUNG`, DM Sans Medium, uppercase, letter-spacing `0.34em`; sage on light, porcelain on dark.

## Space, radius, elevation

- Spacing on a **4px base** (4 · 8 · 12 · 16 · 24 · 32).
- Radius: 8 / 12 / 16 / pill.
- Two elevation languages: **soft shadow** for mobile & web, **beveled porcelain** for the desktop material.

## Voice & tone

Calm, private, honest. Speaks Thai and English evenly; names things the way a person would; never fabricates — an empty screen says "ยังไม่มี…" and teaches the next action rather than showing an invented number.

---

Living references (wireframes, mockups, full guidelines) are maintained as design artifacts outside the repo.
