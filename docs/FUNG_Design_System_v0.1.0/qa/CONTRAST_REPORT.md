# FUNG — Solid-color contrast report

Version: 0.1.0 · Status: token-level check, not a whole-application accessibility certification.

Method: sRGB relative luminance; ratio = (lighter + 0.05) / (darker + 0.05).
Project targets: 4.5:1 for listed text pairs; 3:1 for essential outlines/focus against adjacent base surfaces.
Rounded values are displayed; unrounded values decide pass/fail.

Not covered: transparency, blur, photography, disabled controls, decorative borders, the logo/wordmark,
font rasterization, high-contrast modes, focus clipping, keyboard use, screen readers, or native overlays.
Do not use brand sage/metal or text-disabled as ordinary body text without another measured pair.

| Theme | Foreground token | Background token | Hex pair | Ratio | Target | Result |
| --- | --- | --- | --- | ---: | ---: | --- |
| light | `text-primary` | `bg-canvas` | `#171918` / `#FAF8F3` | 16.65:1 | 4.5:1 | PASS |
| light | `text-secondary` | `bg-canvas` | `#505A54` / `#FAF8F3` | 6.75:1 | 4.5:1 | PASS |
| light | `text-tertiary` | `bg-canvas` | `#59665F` / `#FAF8F3` | 5.67:1 | 4.5:1 | PASS |
| light | `link` | `bg-canvas` | `#4A5B8B` / `#FAF8F3` | 6.26:1 | 4.5:1 | PASS |
| light | `text-primary` | `bg-surface` | `#171918` / `#FFFEFA` | 17.51:1 | 4.5:1 | PASS |
| light | `text-secondary` | `bg-surface` | `#505A54` / `#FFFEFA` | 7.10:1 | 4.5:1 | PASS |
| light | `text-tertiary` | `bg-surface` | `#59665F` / `#FFFEFA` | 5.96:1 | 4.5:1 | PASS |
| light | `link` | `bg-surface` | `#4A5B8B` / `#FFFEFA` | 6.58:1 | 4.5:1 | PASS |
| light | `text-primary` | `bg-elevated` | `#171918` / `#FFFFFF` | 17.67:1 | 4.5:1 | PASS |
| light | `text-secondary` | `bg-elevated` | `#505A54` / `#FFFFFF` | 7.16:1 | 4.5:1 | PASS |
| light | `text-tertiary` | `bg-elevated` | `#59665F` / `#FFFFFF` | 6.02:1 | 4.5:1 | PASS |
| light | `link` | `bg-elevated` | `#4A5B8B` / `#FFFFFF` | 6.64:1 | 4.5:1 | PASS |
| light | `action-primary-fg` | `action-primary-bg` | `#FAF8F3` / `#4A5B8B` | 6.26:1 | 4.5:1 | PASS |
| light | `action-primary-fg` | `action-primary-hover` | `#FAF8F3` / `#3F4E77` | 7.71:1 | 4.5:1 | PASS |
| light | `action-primary-fg` | `action-primary-pressed` | `#FAF8F3` / `#354263` | 9.37:1 | 4.5:1 | PASS |
| light | `action-secondary-fg` | `action-secondary-bg` | `#171918` / `#ECEEE8` | 15.11:1 | 4.5:1 | PASS |
| light | `action-danger-fg` | `action-danger-bg` | `#FAF8F3` / `#B0553F` | 4.69:1 | 4.5:1 | PASS |
| light | `state-local-fg` | `state-local-bg` | `#3F6150` / `#E7EEE8` | 5.86:1 | 4.5:1 | PASS |
| light | `state-pending-fg` | `state-pending-bg` | `#765925` / `#F4ECDD` | 5.55:1 | 4.5:1 | PASS |
| light | `state-danger-fg` | `state-danger-bg` | `#9A4634` / `#F7E7E1` | 5.30:1 | 4.5:1 | PASS |
| light | `state-recording-fg` | `state-recording-bg` | `#9A4634` / `#F7E7E1` | 5.30:1 | 4.5:1 | PASS |
| light | `state-inactive-fg` | `state-inactive-bg` | `#59665F` / `#ECEEE8` | 5.15:1 | 4.5:1 | PASS |
| light | `state-inferred-fg` | `state-inferred-bg` | `#4A5B8B` / `#E9EDF7` | 5.67:1 | 4.5:1 | PASS |
| light | `focus` | `bg-canvas` | `#4A5B8B` / `#FAF8F3` | 6.26:1 | 3.0:1 | PASS |
| light | `border-strong` | `bg-canvas` | `#768177` / `#FAF8F3` | 3.82:1 | 3.0:1 | PASS |
| light | `focus` | `bg-surface` | `#4A5B8B` / `#FFFEFA` | 6.58:1 | 3.0:1 | PASS |
| light | `border-strong` | `bg-surface` | `#768177` / `#FFFEFA` | 4.02:1 | 3.0:1 | PASS |
| light | `focus` | `bg-elevated` | `#4A5B8B` / `#FFFFFF` | 6.64:1 | 3.0:1 | PASS |
| light | `border-strong` | `bg-elevated` | `#768177` / `#FFFFFF` | 4.06:1 | 3.0:1 | PASS |
| dark | `text-primary` | `bg-canvas` | `#FAF8F3` / `#171918` | 16.65:1 | 4.5:1 | PASS |
| dark | `text-secondary` | `bg-canvas` | `#CAD1C9` / `#171918` | 11.34:1 | 4.5:1 | PASS |
| dark | `text-tertiary` | `bg-canvas` | `#A3AFA4` / `#171918` | 7.76:1 | 4.5:1 | PASS |
| dark | `link` | `bg-canvas` | `#B9C8EE` / `#171918` | 10.57:1 | 4.5:1 | PASS |
| dark | `text-primary` | `bg-surface` | `#FAF8F3` / `#202521` | 14.68:1 | 4.5:1 | PASS |
| dark | `text-secondary` | `bg-surface` | `#CAD1C9` / `#202521` | 10.00:1 | 4.5:1 | PASS |
| dark | `text-tertiary` | `bg-surface` | `#A3AFA4` / `#202521` | 6.84:1 | 4.5:1 | PASS |
| dark | `link` | `bg-surface` | `#B9C8EE` / `#202521` | 9.32:1 | 4.5:1 | PASS |
| dark | `text-primary` | `bg-elevated` | `#FAF8F3` / `#292F2A` | 12.90:1 | 4.5:1 | PASS |
| dark | `text-secondary` | `bg-elevated` | `#CAD1C9` / `#292F2A` | 8.78:1 | 4.5:1 | PASS |
| dark | `text-tertiary` | `bg-elevated` | `#A3AFA4` / `#292F2A` | 6.01:1 | 4.5:1 | PASS |
| dark | `link` | `bg-elevated` | `#B9C8EE` / `#292F2A` | 8.19:1 | 4.5:1 | PASS |
| dark | `action-primary-fg` | `action-primary-bg` | `#171918` / `#B9C8EE` | 10.57:1 | 4.5:1 | PASS |
| dark | `action-primary-fg` | `action-primary-hover` | `#171918` / `#CDD8F2` | 12.37:1 | 4.5:1 | PASS |
| dark | `action-primary-fg` | `action-primary-pressed` | `#171918` / `#A8BAE2` | 9.07:1 | 4.5:1 | PASS |
| dark | `action-secondary-fg` | `action-secondary-bg` | `#FAF8F3` / `#303932` | 11.26:1 | 4.5:1 | PASS |
| dark | `action-danger-fg` | `action-danger-bg` | `#FAF8F3` / `#B0553F` | 4.69:1 | 4.5:1 | PASS |
| dark | `state-local-fg` | `state-local-bg` | `#ACC6B4` / `#23382C` | 6.87:1 | 4.5:1 | PASS |
| dark | `state-pending-fg` | `state-pending-bg` | `#E3C493` / `#3C3021` | 7.69:1 | 4.5:1 | PASS |
| dark | `state-danger-fg` | `state-danger-bg` | `#F1AB97` / `#3D2722` | 7.28:1 | 4.5:1 | PASS |
| dark | `state-recording-fg` | `state-recording-bg` | `#F1AB97` / `#3D2722` | 7.28:1 | 4.5:1 | PASS |
| dark | `state-inactive-fg` | `state-inactive-bg` | `#A3AFA4` / `#2A302C` | 5.92:1 | 4.5:1 | PASS |
| dark | `state-inferred-fg` | `state-inferred-bg` | `#B9C8EE` / `#293246` | 7.66:1 | 4.5:1 | PASS |
| dark | `focus` | `bg-canvas` | `#B9C8EE` / `#171918` | 10.57:1 | 3.0:1 | PASS |
| dark | `border-strong` | `bg-canvas` | `#788A7A` / `#171918` | 4.81:1 | 3.0:1 | PASS |
| dark | `focus` | `bg-surface` | `#B9C8EE` / `#202521` | 9.32:1 | 3.0:1 | PASS |
| dark | `border-strong` | `bg-surface` | `#788A7A` / `#202521` | 4.24:1 | 3.0:1 | PASS |
| dark | `focus` | `bg-elevated` | `#B9C8EE` / `#292F2A` | 8.19:1 | 3.0:1 | PASS |
| dark | `border-strong` | `bg-elevated` | `#788A7A` / `#292F2A` | 3.72:1 | 3.0:1 | PASS |

Total: 58 pairs; 58 pass; 0 fail.
