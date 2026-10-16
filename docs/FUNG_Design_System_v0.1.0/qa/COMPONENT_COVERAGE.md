# FUNG — Component token coverage

Generated from CSS under `src/` and the canonical generated token CSS. This is a source inventory, not a visual acceptance report.

## Findings

- Stylesheets scanned: **24**.
- Stylesheets using at least one canonical `--fung-*` variable: **7/24**. A locally invented `--fung-*` alias does not count as a canonical token reference.
- Stylesheets with no canonical token reference: **17**.
- Simple single-color CSS declarations written as a literal: **617**. This includes visual states and surface-specific colors and is a triage count, not a claim that every occurrence is a defect.
- CSS color syntax occurrences: **1181** across **221** unique hex values. This count includes fallbacks, translucent materials, charts, illustrations and third-party marks.
- The board's rendered component specimens demonstrate token intent; they do not prove production component adoption.

## Canonical token references

| Stylesheet | Referenced tokens | Literal declarations |
| --- | --- | ---: |
| `src/components/desktop/CompanionPanel.css` | `--fung-glass-blur-sheet`, `--fung-glass-control`, `--fung-glass-edge`, `--fung-glass-highlight`, `--fung-glass-muted`, `--fung-glass-reader`, `--fung-glass-saturation`, `--fung-glass-sheet`, `--fung-glass-text` | 5 |
| `src/components/desktop/DesktopShell.css` | `--fung-action-danger-bg`, `--fung-action-primary-bg`, `--fung-action-primary-fg`, `--fung-bg-canvas`, `--fung-bg-hover`, `--fung-bg-inset`, `--fung-bg-surface`, `--fung-border-strong`, `--fung-border-subtle`, `--fung-brand-sage`, `--fung-brand-slate`, `--fung-focus`, `--fung-font-ui`, `--fung-logo-mark`, `--fung-radius-sm`, `--fung-shadow-elevated`, `--fung-state-danger-bg`, `--fung-state-danger-fg`, `--fung-state-inferred-bg`, `--fung-state-local-fg`, `--fung-text-primary`, `--fung-text-secondary`, `--fung-text-tertiary` | 3 |
| `src/components/desktop/LiquidGlass.css` | `--fung-glass-blur`, `--fung-glass-blur-sheet`, `--fung-glass-canvas`, `--fung-glass-chrome`, `--fung-glass-control`, `--fung-glass-edge`, `--fung-glass-highlight`, `--fung-glass-muted`, `--fung-glass-radius-card`, `--fung-glass-radius-control`, `--fung-glass-reader`, `--fung-glass-saturation`, `--fung-glass-sheet`, `--fung-glass-solid`, `--fung-glass-text` | 5 |
| `src/landing/landing.css` | `--fung-action-primary-bg`, `--fung-action-primary-fg`, `--fung-action-primary-hover`, `--fung-bg-canvas`, `--fung-brand-metal`, `--fung-brand-sage`, `--fung-brand-slate`, `--fung-focus`, `--fung-focus-offset`, `--fung-focus-width`, `--fung-font-display`, `--fung-font-ui`, `--fung-text-primary`, `--fung-text-secondary` | 38 |
| `src/mobile/mobile.css` | `--fung-action-danger-bg`, `--fung-action-primary-bg`, `--fung-action-primary-fg`, `--fung-action-primary-pressed`, `--fung-bg-backdrop`, `--fung-bg-canvas`, `--fung-bg-elevated`, `--fung-bg-inset`, `--fung-bg-surface`, `--fung-border-subtle`, `--fung-brand-metal`, `--fung-brand-sage`, `--fung-brand-slate`, `--fung-control-height`, `--fung-focus`, `--fung-focus-offset`, `--fung-focus-width`, `--fung-font-latin`, `--fung-font-mono`, `--fung-font-size-body-mobile`, `--fung-font-size-caption`, `--fung-font-size-label`, `--fung-font-size-timer-mobile`, `--fung-font-size-title`, `--fung-font-ui`, `--fung-font-weight-medium`, `--fung-font-weight-semibold`, `--fung-layout-mobile-landscape-rail`, `--fung-line-height-body-mobile`, `--fung-line-height-label`, `--fung-line-height-timer-mobile`, `--fung-line-height-title`, `--fung-logo-wordmark`, `--fung-radius-md`, `--fung-radius-pill`, `--fung-radius-sm`, `--fung-shadow-elevated`, `--fung-shadow-inset`, `--fung-shadow-overlay`, `--fung-shadow-surface`, `--fung-space-12`, `--fung-space-4`, `--fung-state-danger-bg`, `--fung-state-danger-fg`, `--fung-state-local-bg`, `--fung-state-local-fg`, `--fung-state-pending-bg`, `--fung-state-pending-fg`, `--fung-state-recording-bg`, `--fung-state-recording-fg`, `--fung-text-primary`, `--fung-text-secondary`, `--fung-text-tertiary`, `--fung-touch-target`, `--fung-wordmark-tracking` | 13 |
| `src/styles.css` | `--fung-action-primary-bg`, `--fung-bg-canvas`, `--fung-focus`, `--fung-focus-offset`, `--fung-focus-width`, `--fung-font-ui`, `--fung-text-inverse`, `--fung-text-primary` | 0 |
| `src/web/Dashboard.css` | `--fung-action-primary-bg`, `--fung-action-primary-fg`, `--fung-action-primary-hover`, `--fung-bg-canvas`, `--fung-bg-elevated`, `--fung-bg-surface`, `--fung-border-subtle`, `--fung-brand-sage`, `--fung-brand-slate`, `--fung-focus`, `--fung-focus-offset`, `--fung-focus-width`, `--fung-font-ui`, `--fung-radius-md`, `--fung-shadow-elevated`, `--fung-shadow-surface`, `--fung-space-16`, `--fung-text-primary`, `--fung-text-secondary` | 92 |

## Stylesheets without canonical token references

| Stylesheet | Literal declarations |
| --- | ---: |
| `src/components/AccountLoginPanel.css` | 34 |
| `src/components/BackupPanel.css` | 1 |
| `src/components/CloudProvidersPanel.css` | 49 |
| `src/components/DevicePairingPanel.css` | 56 |
| `src/components/ExternalAccountPanel.css` | 26 |
| `src/components/LiveMeetingPanel.css` | 32 |
| `src/components/MediaFetchPanel.css` | 25 |
| `src/components/MeetingIntelligencePanel.css` | 19 |
| `src/components/RecoveryNotice.css` | 1 |
| `src/components/SettingsPanel.css` | 8 |
| `src/components/TtsProviderPanel.css` | 58 |
| `src/components/ZoomPanel.css` | 39 |
| `src/components/desktop/LiveWorkspace.css` | 46 |
| `src/components/desktop/RecordingOutputPanel.css` | 2 |
| `src/components/desktop/RecordingReview.css` | 2 |
| `src/web/AccountSettings.css` | 50 |
| `src/web/LoadingScreen.css` | 13 |

## Acceptance boundary

New semantic colors should use a canonical token or include a documented, reviewed exception. Existing uncovered styles need an owner and a migration decision before being represented as complete. Source inspection cannot establish rendered contrast, alpha-over-image contrast, clipping, text scaling, native focus, screen-reader announcements or reduced-motion behavior. See the package verification report for the separate runtime evidence and limits.
