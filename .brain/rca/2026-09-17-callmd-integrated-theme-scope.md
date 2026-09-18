---
version: "0.1.0b"
created_at: "2026-09-17T09:35:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T09:35:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved integrated dark/light/system theme ancestry only"
  risk: "MEDIUM"
---

# Integrated theme ancestry mismatch

## Symptom

On the frozen IntegrationFIX1 App at1280x800, Home/Live/History navigation and
hidden layout passed. Switching History from light to dark changed Shell but
left RecordingReview visibly light. Main stopped the theme checkpoint, restored
light, closed its tab/server and reset viewport; no native or account action.

## Evidence

- Actual CUA screenshots/DOM at127.0.0.1:15473/app?surface=desktop: Shell class
  desktop-shell--dark, data-theme=dark, background rgb(23,25,24); RecordingReview
  background rgb(250,248,243), no closest .theme-dark ancestor.
- App hash3efb66d9e71b6158eec9e5c0753d7dbcd6c132ac9a186bc40d24c79a9d5fc804:
  lines1509-1532 place Live/Review in callmd-surface-stack, outside the sibling
  legacy-only app-shell theme-* wrapper.
- RecordingReview.css42 and LiveWorkspace.css505 dark rules require .theme-dark.
  DesktopShell.tsx728 supplies desktop-shell--dark/light/system instead; its CSS
  handles system preference independently through prefers-color-scheme.
- Live shares the same unmatched selector ancestry (source inference, not a second
  dark Live browser observation). No recording/history-data defect is asserted.

## Root Cause

Integration moved accepted panels outside their required theme ancestor while
the new Shell uses a different theme class contract. Its own palette changes,
but the accepted panels' dark selectors never match.

## Why it escaped detection

Isolated component styling/SSR and build tests did not exercise the actual
integrated ancestor chain under theme changes. Main browser theme switching did.

## Proposed prevention within approved scope

Fresh Luna IntegrationFIX2 owns only App.tsx, styles.css if necessary, existing
integration test and its own integration report. Restore one coherent theme
boundary for both new panels and retained legacy workspace. Explicit light/dark
and system preference (including subsequent preference changes) must agree with
Shell, without mutating unrelated DOM or adding any live/session listener/store.
No new dependency, native command, leaf palette rewrite, provider or data action.
Add focused integration theme ancestry/system regression evidence; independently
review and recheck actual light/dark UI before acceptance. Main edits docs only.
Reviewer coherence confirmation precedes the exact implementation packet.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Integrated ancestor mismatch and bounded approved theme correction | UNCOMMITTED; base376ef30 | Codex orchestrator |
