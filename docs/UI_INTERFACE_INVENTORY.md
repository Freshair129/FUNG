---
version: "0.2.0b"
created_at: "2026-08-22T20:06:53+07:00,ATHER"
last_update: "2026-08-22T20:06:53+07:00,ATHER"
status: "draft"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "interface-inventory"
  scope: "FUNG Desktop, Mobile, Shared Design System, Plugin Boundary"
---

# FUNG UI Interface Inventory

## Purpose

เอกสารกลางสำหรับตอบว่า FUNG มี UI surface อะไรบ้าง อยู่ที่ไหน มีสถานะใด และมีหลักฐานจาก source code หรือเอกสารใดรองรับ โดยแยกให้ชัดระหว่าง:

- `IMPLEMENTED` — มี UI ใน source code แล้ว
- `PARTIAL` — มี UI หรือ contract แล้ว แต่ยังมี runtime, UAT หรือ release gate เปิดอยู่
- `SPECIFIED` — มีใน design/UX specification แต่ยังไม่มีหลักฐาน implementation ครบ
- `GENERATED` — สร้างจาก source-of-truth อื่น ห้ามแก้โดยตรง
- `HOST-OWNED` — UI เป็นของ host platform ไม่ใช่ FUNG plugin

สถานะในเอกสารนี้ไม่ใช่คำประกาศว่า production-ready; ต้องอ่านคู่กับ implementation status และ release gates ของแต่ละ platform

## Source-of-Truth Order

1. Implementation truth: [`docs/Desktop/08-real-progress.md`](Desktop/08-real-progress.md) และ [`docs/Mobile/IMPLEMENTATION_STATUS.md`](Mobile/IMPLEMENTATION_STATUS.md)
2. Desktop component and IA contracts: [`04-components.md`](Desktop/04-components.md), [`05-sitemap-ia.md`](Desktop/05-sitemap-ia.md), [`07-meeting-mode.md`](Desktop/07-meeting-mode.md)
3. Mobile product/UI contract: [`PRODUCT_UX_SPEC.md`](Mobile/PRODUCT_UX_SPEC.md)
4. Shared visual rules: [`DESIGN.md`](../DESIGN.md), [`docs/Desktop/DESIGN_SYSTEM.md`](Desktop/DESIGN_SYSTEM.md), [`docs/Mobile/DESIGN_SYSTEM.md`](Mobile/DESIGN_SYSTEM.md)
5. Source implementation: `src/App.tsx`, `src/components/`, `src/mobile/`, `src/web/`

## Inventory Summary

| Layer | IDs | Canonical source | Current conclusion |
| --- | --- | --- | --- |
| Desktop shell and Meeting Mode | D01-D11 | `docs/Desktop/04-components.md`, `05-sitemap-ia.md`, `07-meeting-mode.md` | UI foundation and routed Live Meeting exist; runtime/UAT gates remain separate |
| Mobile product surface | M1-M8 | `docs/Mobile/PRODUCT_UX_SPEC.md` | Screen inventory exists; several lifecycle/device/AI gates remain open |
| Shared design system | S01-S06 | `DESIGN.md`, desktop/mobile design-system sources | Tokens, components, states and generated references exist in separate sources |
| Meeting-artifacts plugin | P01 | `.agents/plugins/fung-meeting-artifacts/` | No standalone UI; interaction is owned by Claude Code, Codex or Antigravity |

## Desktop Inventory

| ID | Surface | Primary job | Implementation evidence | Status / boundary |
| --- | --- | --- | --- | --- |
| D01 | App shell / fixed HUD | Own the 1280 × 720 working canvas and material layout | `src/App.tsx`, `src/styles.css` | `IMPLEMENTED`; visual and keyboard UAT remain environment-gated |
| D02 | P rail, topbar and center deck | Switch P1-P4 domains without route-heavy navigation | `src/App.tsx`; [`05-sitemap-ia.md`](Desktop/05-sitemap-ia.md) | `IMPLEMENTED` |
| D03 | P1 Capture / Live Meeting | Record, import, inspect capture state and open meeting workbench | `src/components/LiveMeetingPanel.tsx`, `src/lib/jobActions.ts` | `PARTIAL`; live Whisper runtime and UAT must be proven per [`08-real-progress.md`](Desktop/08-real-progress.md) |
| D04 | P2 Transcript Review | Review transcript, timestamps, speakers and evidence | `src/components/LiveMeetingPanel.tsx`, `src/App.tsx` | `PARTIAL`; transcript surface exists, but long-read/runtime completeness gates remain |
| D05 | P3 Summary / Intent | Show recap, decisions, actions and evidence-backed intelligence | `src/lib/meetingSummaries.ts`, `src/App.tsx` | `PARTIAL`; summary surface exists, intent-specific evidence review is incomplete |
| D06 | P4 Runtime / Export / Governance | Inspect provider state, privacy, provenance and export | `src/App.tsx`, `src/components/TtsProviderPanel.tsx`, `src/components/CloudProvidersPanel.tsx` | `PARTIAL`; export queue and some provider/runtime gates remain open |
| D07 | External retrieval approval | Preview, approve, cancel, revoke and inspect sanitized connector results | `src/components/ExternalMeetingToolsPanel.tsx` | `IMPLEMENTED` behind default-off flags; real connector and visual UAT remain open |
| D08 | Audio/media ingestion | Import local media, URL media and Zoom-linked sources | `src/components/MediaFetchPanel.tsx`, `src/components/ZoomPanel.tsx` | `PARTIAL`; real-site and source-custody/checksum gaps remain |
| D09 | Account / connection / pairing | Sign in, manage account, pair devices and inspect cloud providers | `src/components/AccountLoginPanel.tsx`, `src/components/ExternalAccountPanel.tsx`, `src/components/DevicePairingPanel.tsx`, `src/web/AccountSettings.tsx` | `PARTIAL`; external identity/device gates are not equivalent to local UI existence |
| D10 | Backup / recovery | Export, restore and explain data-safety state | `src/components/BackupPanel.tsx`, `src/components/RecoveryNotice.tsx` | `IMPLEMENTED` with clean-install restore and production transport gates open |
| D11 | Loading / failure / degraded states | Keep data safety and recovery action visible | `src/web/LoadingScreen.tsx`, `src/components/RecoveryNotice.tsx`, component state rules | `IMPLEMENTED`; coverage should be expanded as new surfaces are added |

### Desktop Domain Crosswalk

| Product domain | Page | Primary UI surface | Main states |
| --- | --- | --- | --- |
| Capture and Library | P1 | Capture console, Live Meeting | idle, recording, paused, saving, interrupted, recovered |
| Transcript and Speakers | P2 | Review workspace, transcript/evidence rows | pending, processing, low-confidence, speaker editing, ready |
| Summary and Intent | P3 | Intelligence deck, recap/action surfaces | unavailable, generating, inferred, cited, ready |
| Runtime and Governance | P4 | Runtime control, export and policy surfaces | local-ready, missing model, offline, approval-required, failed |

### Desktop Page-Level Wireframes

| Page | Wireframe | Covers |
| --- | --- | --- |
| P1 | [`p1-capture.svg`](Desktop/wireframes/p1-capture.svg) | Capture setup, live capture, import, capture safety and D/E/F/G signals |
| P2 | [`p2-transcript-review.svg`](Desktop/wireframes/p2-transcript-review.svg) | Transcript rows, speaker review, evidence pins and uncertainty |
| P3 | [`p3-summary-intent.svg`](Desktop/wireframes/p3-summary-intent.svg) | Recap, per-person intent, next steps, citations and confirmation |
| P4 | [`p4-runtime-export.svg`](Desktop/wireframes/p4-runtime-export.svg) | BYOM/runtime, local policy, export bundle, provenance and egress |

The complete set is indexed at [`docs/Desktop/wireframes/README.md`](Desktop/wireframes/README.md). These are structural wireframes, not final visual mocks or release evidence.

## Mobile Screen Inventory

The product UX specification defines M1-M8. Several screens currently share `src/mobile/MobileApp.tsx`; the ID is a product surface ID, not necessarily a one-file component ID.

| ID | Screen | Primary job | Implementation evidence | Status / boundary |
| --- | --- | --- | --- | --- |
| M1 | Voice Home | Start a command, recording, note or recent work | `src/mobile/MobileApp.tsx` (`HomeScreen`) | `IMPLEMENTED`; on-device STT asset remains a gate |
| M2 | Live Capture | Record safely and expose durable chunk state | `src/mobile/MobileApp.tsx` (`CaptureScreen`), `src/mobile/captureOrchestration.ts` | `PARTIAL`; screen-off, kill/restart and extended lifecycle evidence remain open |
| M3 | Notes Library | Find notes and projects | `src/mobile/MobileApp.tsx` (`NotesScreen`) | `IMPLEMENTED` at code/test level; physical/reopen evidence remains open |
| M4 | Note Detail | Read evidence, edit text, play source and follow relations | Shared mobile surface; `src/mobile/MobileApp.tsx`, `src/mobile/TimelineScreen.tsx` | `PARTIAL`; dedicated screen-to-source mapping needs tightening |
| M5 | Graph Explorer | Explore note, speaker, topic, decision and evidence relations | `src/mobile/MobileApp.tsx` (`GraphScreen`), `src/mobile/bridge.ts` | `IMPLEMENTED` at code/test level; provider/device proof remains open |
| M6 | Devices and Runtime | Pair Desktop and choose execution location | `src/mobile/MobileApp.tsx` (`DevicesScreen`), `src/mobile/bridge.ts` | `PARTIAL`; secure mutual-auth transport and resumable delegation remain open |
| M7 | Job Detail | Track queued, running, paused, failed and completed work | `src/mobile/MobileApp.tsx`, `src/mobile/CreativeStudio.tsx` | `PARTIAL`; executor and lifecycle evidence remain open |
| M8 | MCP and Privacy | Enable bounded tools and inspect access | `src/mobile/MobileApp.tsx`, `src/mobile/bridge.ts` | `PARTIAL`; contract/build evidence exists, device-to-client interoperability remains open |

## Shared Design-System Inventory

| ID | Shared surface/rule | Source | Status |
| --- | --- | --- | --- |
| S01 | Color, typography, spacing, radius and elevation tokens | [`DESIGN.md`](../DESIGN.md), [`docs/Mobile/DESIGN_SYSTEM.md`](Mobile/DESIGN_SYSTEM.md) | `IMPLEMENTED` as documented source; versions remain beta/draft where marked |
| S02 | App shell, panel zones and ownership rules | [`04-components.md`](Desktop/04-components.md) | `IMPLEMENTED` as component contract |
| S03 | Buttons, segmented controls, signal cards and transcript/evidence rows | [`DESIGN.md`](../DESIGN.md) | `IMPLEMENTED` as design rule; implementation coverage is tracked per surface above |
| S04 | Loading, empty, error, offline and model-missing states | [`04-components.md`](Desktop/04-components.md) | `IMPLEMENTED` as rule; each new component must map its state coverage |
| S05 | Thai-first mobile visual system and brand assets | [`docs/Mobile/DESIGN_SYSTEM.md`](Mobile/DESIGN_SYSTEM.md) | `IMPLEMENTED` source-of-truth; generated HTML must not be edited manually |
| S06 | Interactive design-system reference | [`docs/Mobile/design-system/index.html`](Mobile/design-system/index.html) | `GENERATED` from `docs/Mobile/DESIGN_SYSTEM.md` |

## Plugin Boundary

| ID | Surface | Owner | Evidence | Status |
| --- | --- | --- | --- | --- |
| P01 | Meeting-artifacts interactive workflow | Claude Code, Codex or Antigravity host UI | `.agents/plugins/fung-meeting-artifacts/skills/`, `bin/`, `server/` | `HOST-OWNED`; the plugin has no standalone FUNG UI |

The plugin's transcript, model, format and destination questions are rendered by the host agent. Google Docs, Google Sheets and LINE delivery are external integrations and are not counted as FUNG-owned UI surfaces.

## Known Inventory Gaps

1. There is no single stable cross-platform ID map from `D01-D11` / `M1-M8` to every React component and Tauri surface.
2. Desktop uses P1-P4 domain IDs while Mobile uses M1-M8 screen IDs; the relationship is documented conceptually but not yet represented as a machine-readable graph.
3. Implementation status and release/UAT status are intentionally different; a row marked `IMPLEMENTED` can still have runtime, device, provider or release gates open.
4. Screenshot, keyboard, physical-device and clean-install evidence are not attached to every row; the new wireframes describe structure and state coverage, not execution proof.

## Update Rules

- Add or update an inventory row whenever a user-facing surface, state or platform boundary changes.
- Include the implementation file and the evidence/status source in the same change.
- Keep generated design-system HTML derived from its Markdown source; do not hand-edit generated artifacts.
- Do not mark a surface release-ready from source presence alone; update the corresponding implementation-status and release-gate documents.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Added complete Desktop P1-P4 page-level wireframes and linked them to the inventory. |
| 0.1.0b | Added the first cross-platform UI interface inventory and separated UI implementation status from runtime/UAT/release readiness. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.2.0b | 2026-08-22 | draft | Added and indexed the complete Desktop P1-P4 structural wireframe set. | N/A — uncommitted | ATHER |
| 0.1.0b | 2026-08-22 | draft | Added Desktop, Mobile, Shared Design System and Plugin Boundary inventory. | N/A — uncommitted | ATHER |
