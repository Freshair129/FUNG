---
version: "0.1.0b"
created_at: "2026-10-16,Codex"
last_update: "2026-10-16,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "design-governance"
  doc_type: "index"
  scope: "Current design ownership and historical references"
---

# FUNG Design Index

This index assigns design ownership. Status and runtime acceptance remain in
[Desktop progress](../Desktop/08-real-progress.md), [Mobile status](../Mobile/IMPLEMENTATION_STATUS.md)
and the linked verification reports.

| Topic | Canonical source | Boundary |
| --- | --- | --- |
| Current Desktop layout, wireframes, navigation and journeys | [Studio flow contract](2026-10-16-desktop-navigation-and-flows.md) | Home/Live/Review/Output/Appearance, bottom dock and overlays |
| Shared visual rules and semantic tokens | [Quiet Archive Design System](../FUNG_Design_System_v0.1.0/README.md) | JSON token source and generated CSS stay in their existing package path |
| Desktop glass, motion and fallbacks | [Liquid Glass addendum](../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md) | Material rules; Studio frame section is scoped evidence, not native acceptance |
| Brand asset geometry | [Brand kit](../brand-kit/README.md) | Logo assets/identity; do not duplicate semantic token authority |
| Capture input routing | [Device routing](2026-09-20-live-capture-device-routing.md) | Microphone/system-source selection |
| Output destination | [Recording output](2026-09-20-recording-output-destination.md) | New-file root selection and custody of existing roots |
| Recording review/playback/Q&A scope | [P1-B UX](2026-09-17-callmd-desktop-ui.md) and [contracts](../specs/2026-09-17-callmd-desktop-contracts.md) | Retain feature scope; current shell placement follows Studio flow contract |
| Mobile UX | [Mobile product UX](../Mobile/PRODUCT_UX_SPEC.md) | Product IDs are distinct from component/route IDs |
| Cross-platform interfaces | [Interface inventory](../UI_INTERFACE_INVENTORY.md) | Surface-to-source mapping and explicit evidence limits |
| Historical Desktop chrome | [September refresh](2026-09-19-liquid-glass-desktop-shell-refresh.md), [P1–P4](../Desktop/wireframes/README.md), [CallMD boards](callmd-desktop/wireframes.svg) | Historical sidebar/HUD/three-board presentation; do not use as current shell |
| Design brief | [Frontend brief](FRONTEND_REDESIGN_BRIEF.md) | Wider requirements; not proof every feature is delivered |

Supersession is per topic. Newer evidence does not automatically override an
older security, data, feature or material contract. Preserve runtime imports from
`docs/FUNG_Design_System_v0.1.0/tokens/` when reorganizing navigation.

## CHANGELOG

| Version | Date | Summary |
| --- | --- | --- |
| 0.1.0b | 2026-10-16 | Added current/legacy design ownership and flow entry points. |
