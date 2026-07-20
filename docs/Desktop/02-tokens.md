---
version: "0.1.0b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-05T13:15:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-tokens"
  scope: "FUNG"
---

# 02 - Tokens

## Design Direction

The visual system is Apple Japan, Quiet Luxury, Minimal Cozy, and Skeuomorphic. The UI should read as a calm desktop instrument made from molded porcelain, soft metal, inset glass, and pressed controls.

## Color Tokens

| Token | Suggested Value | Use |
| --- | --- | --- |
| `--bg-porcelain` | `#efe9dd` | App background base |
| `--surface-porcelain` | `#f8f3ea` | Main panel and cards |
| `--surface-inset` | `#e8dfd2` | Inset controls, meters, trays |
| `--ink` | `#171716` | Primary text |
| `--ink-muted` | `#5f5b52` | Secondary text |
| `--sage` | `#6f8c75` | Local/private/ready state |
| `--indigo` | `#28324d` | Focus, selected transcript, deep controls |
| `--warm-metal` | `#b79b68` | Premium accent, rim highlights |
| `--signal-red` | `#b84a3d` | Error/destructive only |
| `--line-soft` | `rgba(54, 48, 39, 0.18)` | Hairline borders |

## Material Tokens

| Token | Use |
| --- | --- |
| `--shadow-raised` | Raised porcelain panel or card |
| `--shadow-pressed` | Pressed button, selected signal card |
| `--shadow-inset` | Inner tray, meter, waveform well |
| `--rim-highlight` | Thin top-left bevel highlight |
| `--rim-lowlight` | Thin bottom-right bevel lowlight |

Material rule: shadows must explain physical depth. Do not use decorative glow blobs or gradients that make the layout feel weightless.

## Radius Tokens

| Token | Value | Use |
| --- | ---: | --- |
| `--radius-panel` | `28px` | Outer subtract panel |
| `--radius-notch` | `20px` | Boolean subtract notches |
| `--radius-fab` | `14px` to `16px` | Floating controls in approved notches |
| `--radius-card` | `8px` max | Repeated cards |
| `--radius-control` | `7px` | Buttons, toggles, chips |

Cards stay at `8px` radius or less unless the layout spec defines a larger structural panel.

## Typography Tokens

| Token | Use |
| --- | --- |
| `--font-ui` | UI labels, controls, tables |
| `--font-mono` | Timers, counters, timestamps, technical IDs |
| `--text-hero` | Reserved for true hero contexts; normally unused in the app shell |
| `--text-panel-title` | Compact panel titles |
| `--text-body` | Long transcript and notes |
| `--text-caption` | Metadata, timestamps, confidence |

Rules:

- Letter spacing is `0`.
- Do not scale font size with viewport width.
- Text must not overlap or escape fixed controls.
- Transcript text prioritizes reading comfort over ornament.

## Spacing Tokens

| Token | Value | Use |
| --- | ---: | --- |
| `--space-1` | `4px` | Dense internal gaps |
| `--space-2` | `8px` | Controls and chips |
| `--space-3` | `12px` | Signal card grid gaps |
| `--space-4` | `16px` | Standard zone padding |
| `--space-5` | `24px` | Section spacing |

## Motion Tokens

| Token | Value | Use |
| --- | --- | --- |
| `--motion-fast` | `120ms ease-out` | Button press and hover |
| `--motion-standard` | `180ms ease` | Card selection and panel state |
| `--motion-slow` | `260ms ease` | View transition |

Motion should be quiet and functional. Avoid attention-seeking animation during recording or review.

## State Tokens

| State | Visual Treatment |
| --- | --- |
| Default | Raised surface, subtle rim, no glow |
| Hover | Slight highlight, no layout shift |
| Active | Pressed/inset surface, stronger contrast |
| Recording | Signal red used sparingly with timer/waveform emphasis |
| Processing | Warm metal or indigo progress state |
| Ready | Sage state with calm confirmation |
| Error | Signal red plus readable message |

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Added tactile skeuomorphic token set. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-05 | beta | Added design tokens. | N/A | ATHER |
