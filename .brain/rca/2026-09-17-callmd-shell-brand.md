---
version: "0.1.0b"
created_at: "2026-09-17T05:45:06+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T05:45:06+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Bounded Shell brand fidelity correction"
  risk: "LOW"
---

# Shell brand fidelity

## Symptom / evidence

Independent Terra Kierkegaard01a0ac61-2516-79a0-859c-d46c827e20d7 returned
WARN requiring correction before UI acceptance. DesktopShell.tsx:716 imports/
renders legacy FungLogo, a filled gradient squircle; approved docs/design/
2026-09-17-callmd-desktop-ui.md and brand-kit require the unboxed currentColor
mark in docs/brand-kit/logo/fung-mark.svg. System-dark also retains sage wordmark.
Frozen Shell TSX1cdfb9edc35826073f161b1eeb74e22599215823b9c7ed425ba9904ea70a5782.

## Root cause

The existing legacy component was reused as if it were the authoritative
approved brand asset. Theme-dependent markup also interprets system as light
instead of allowing effective dark colors to follow the existing CSS theme.

## Why it escaped detection

Shell4/4 tests cover guard/state/dialog/SSR behavior, not the exact brand mark
or effective system-dark colors. Independent review caught it before mounting.
Build/bootstrap10/10/egress8/8 pass do not verify visual brand fidelity.

## Bounded prevention / FIX1

Fresh Luna/max may edit only the four original Shell lease files in absolute
shared-376ef30 paths. Replace only this legacy logo usage with the authoritative
FUNG mark (exact inlined SVG path or compatible existing asset import, no new
asset/plugin/dependency). Keep unboxed currentColor styling: ink on light and
porcelain on dark; system mode inherits the CSS effective theme. Preserve
specified40px mark/clearspace and meaningful accessible semantics.
Do not alter the shared contract, legacy FungLogo component or unrelated UI.
Add focused actual-render/asset-identity and theme-rule tests alongside existing
production tests; do not claim SSR proves browser rendering. Rerun Shell tests,
build, bootstrap/egress. Update report0.1.0b to0.1.1b with hashes and real cwd;
independent bounded re-review and integrated browser checks remain mandatory.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record reviewed brand source/theme mismatch and exact correction | UNCOMMITTED;base376ef30 | Codex orchestrator |
