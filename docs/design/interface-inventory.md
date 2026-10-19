---
version: "0.5.1b"
created_at: "2026-08-22T20:06:53+07:00,ATHER"
last_update: "2026-10-19,Codex"
status: "beta"
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

1. Implementation truth: [`docs/Desktop/08-real-progress.md`](../Desktop/08-real-progress.md) และ [`docs/Mobile/IMPLEMENTATION_STATUS.md`](../Mobile/IMPLEMENTATION_STATUS.md)
2. Current Desktop layout/navigation/flows: [Studio flow contract](2026-10-16-desktop-navigation-and-flows.md); material rules: [Liquid Glass addendum](../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md). September sidebar placement is historical.
3. Shared design-system tokens and generated board: [`fung.tokens.json`](../FUNG_Design_System_v0.1.0/tokens/fung.tokens.json), [`design-system/index.html`](../design-system/index.html)
4. Desktop component and content IA: [`04-components.md`](../Desktop/04-components.md) and [`07-meeting-mode.md`](../Desktop/07-meeting-mode.md); [`05-sitemap-ia.md`](../Desktop/05-sitemap-ia.md) is the superseded P1–P4 draft. Current navigation is owned by the Studio flow contract above.
5. Mobile product/UI contract: [`PRODUCT_UX_SPEC.md`](../Mobile/PRODUCT_UX_SPEC.md)
6. Shared visual rules: [`DESIGN.md`](../../DESIGN.md), [`docs/Desktop/DESIGN_SYSTEM.md`](../Desktop/DESIGN_SYSTEM.md), [`docs/Mobile/DESIGN_SYSTEM.md`](../Mobile/DESIGN_SYSTEM.md)
7. Source implementation: `src/App.tsx`, `src/components/desktop/`, `src/mobile/`, `src/web/`

## Inventory Summary

| Layer | IDs | Canonical source | Current conclusion |
| --- | --- | --- | --- |
| Desktop shell and Meeting Mode | D01-D14 | `src/components/desktop/DesktopShell.tsx`, [Studio flows](2026-10-16-desktop-navigation-and-flows.md), `07-meeting-mode.md` | Primary dock is Home/Live/Archive; Storage location and Appearance are secondary More destinations; current browser/CPU checks are scoped, native/provider/device acceptance remains separate |
| Mobile product surface | M1-M8 | `docs/Mobile/PRODUCT_UX_SPEC.md` | Screen inventory exists; several lifecycle/device/AI gates remain open |
| Shared design system | S01-S06 | `DESIGN.md`, desktop/mobile design-system sources | Tokens, components, states and generated references exist in separate sources |
| Meeting-artifacts plugin | P01 | `.agents/plugins/fung-meeting-artifacts/` | No standalone UI; interaction is owned by Claude Code, Codex or Antigravity |
| Browser bootstrap surfaces | W01-W04 | `src/main.tsx`, `src/lib/bootstrap.ts`, `src/web/` | Landing, Dashboard, callback and forced preview routes have different configuration/auth/native boundaries |

## Desktop Inventory

| ID | Surface | Primary job | Implementation evidence | Status / boundary |
| --- | --- | --- | --- | --- |
| D01 | Studio dashboard and glass shell | Own hero, next-meeting card, local-space/task-pool cards, shortcuts, frame and bottom dock | `src/components/desktop/StudioHome.tsx`, `src/components/desktop/DesktopStudio.css`, `src/components/desktop/DesktopShell.tsx` | `IMPLEMENTED`; native frame acceptance remains separate; project/capture facts preserve unavailable states |
| D02 | Active-surface navigation | Primary dock Home/Live/Archive; More → Import/Export/Storage location/Appearance/Pairing/Account; project, Companion, Settings and window controls stay in utility areas | `src/components/desktop/DesktopShell.tsx`, `src/components/desktop/contracts.ts` | `IMPLEMENTED`; five controlled content surfaces, three primary task destinations, capture guard, no current hover sidebar; anchors are not persistent deep links |
| D03 | Appearance surface | Keep theme, material and transparency controls on a dedicated support page reached from More, outside the primary task navigation | `src/components/desktop/DesktopShell.tsx`, `src/App.tsx` | `IMPLEMENTED`; WebView heading and three controls verified |
| D04 | Live Meeting / Capture | Record, import, inspect capture state and open meeting workbench | `src/components/LiveMeetingPanel.tsx`, `src/lib/jobActions.ts` | `PARTIAL`; live Whisper runtime and UAT must be proven per [`08-real-progress.md`](../Desktop/08-real-progress.md) |
| D05 | Review / transcript | Select actual project/recording, read transcript, explicitly play, ask and queue scoped jobs | `src/components/desktop/RecordingReview.tsx`, `src/App.tsx` | `PARTIAL`; current CPU scope/race checks pass; native audio/provider/device acceptance is separate from source completeness |
| D06 | Summary / Runtime / Export | Show recap, provider state, privacy, provenance and export | `src/App.tsx`, `src/components/TtsProviderPanel.tsx`, `src/components/CloudProvidersPanel.tsx` | `PARTIAL`; export queue and some provider/runtime gates remain open |
| D07 | External retrieval approval | Preview, approve, cancel, revoke and inspect sanitized connector results | `src/components/ExternalMeetingToolsPanel.tsx` | `IMPLEMENTED` behind default-off flags; real connector and visual UAT remain open |
| D08 | Audio/media ingestion | Import local media, URL media and Zoom-linked sources | `src/components/MediaFetchPanel.tsx`, `src/components/ZoomPanel.tsx` | `PARTIAL`; real-site and source-custody/checksum gaps remain |
| D09 | Account / connection / pairing | Sign in, manage account, pair devices and inspect cloud providers | `src/components/AccountLoginPanel.tsx`, `src/components/ExternalAccountPanel.tsx`, `src/components/DevicePairingPanel.tsx`, `src/web/AccountSettings.tsx` | `PARTIAL`; external identity/device gates are not equivalent to local UI existence |
| D10 | Backup / recovery | Export, restore and explain data-safety state | `src/components/BackupPanel.tsx`, `src/components/RecoveryNotice.tsx` | `IMPLEMENTED` with clean-install restore and production transport gates open |
| D11 | Loading / failure / degraded states | Keep data safety and recovery action visible | `src/web/LoadingScreen.tsx`, `src/components/RecoveryNotice.tsx`, component state rules | `IMPLEMENTED`; coverage should be expanded as new surfaces are added |
| D12 | In-app Companion | Open Live/Settings and disclose selected context | `src/components/desktop/CompanionPanel.tsx` | `IMPLEMENTED` as navigation/context panel; no arbitrary chat, autonomous agent or external floating window is implied |
| D13 | Storage location | Read/set/reset root for new files; lock while capture is active; not a recording library | `src/components/desktop/RecordingOutputPanel.tsx` | `PARTIAL`; secondary More destination; browser unavailable state and source/CPU custody checks; native folder/permission acceptance remains separate |
| D14 | Settings dialog | Account/backup/providers/import/runtime sections | `src/components/SettingsPanel.tsx` | `IMPLEMENTED` dialog with focus loop/Escape/restore; each child provider/device/runtime has separate gates |

### Desktop Surface Crosswalk

| Product domain | Active surface | Primary UI surface | Main states |
| --- | --- | --- | --- |
| Home | `home` | Start/import surface and project context | empty, project selected, live entry available |
| Capture and Library | `live` | Live Meeting / capture workspace | idle, recording, paused, saving, interrupted, recovered |
| Transcript and Speakers | `review` | Review workspace, transcript/evidence rows | pending, processing, low-confidence, speaker editing, ready |
| Appearance | `appearance` | Dedicated theme/material/transparency page | light, dark, glass, solid, full transparency, reduced transparency |
| Storage location | `output` | New-file destination preference | loading, writable, denied/unavailable, busy, capture-locked |
| Runtime and Governance | Dock / More / overlays | Runtime, export, pairing, settings and Companion surfaces | local-ready, missing model, offline, approval-required, failed |

The five controlled content surfaces are not five peer navigation items. The
persistent task navigation is Home, Live and Archive; Storage location and
Appearance remain secondary destinations under More. This distinction is the
current sitemap contract in [Studio flows](2026-10-16-desktop-navigation-and-flows.md).

### Desktop Page-Level Wireframes

| Page | Wireframe | Covers |
| --- | --- | --- |
| P1 | [`p1-capture.svg`](../Desktop/wireframes/p1-capture.svg) | Capture setup, live capture, import, capture safety and D/E/F/G signals |
| P2 | [`p2-transcript-review.svg`](../Desktop/wireframes/p2-transcript-review.svg) | Transcript rows, speaker review, evidence pins and uncertainty |
| P3 | [`p3-summary-intent.svg`](../Desktop/wireframes/p3-summary-intent.svg) | Recap, per-person intent, next steps, citations and confirmation |
| P4 | [`p4-runtime-export.svg`](../Desktop/wireframes/p4-runtime-export.svg) | BYOM/runtime, local policy, export bundle, provenance and egress |

The original set is indexed at [Desktop wireframes](../Desktop/wireframes/README.md).
These P1-P4 SVGs and the September three-board CallMD package preserve historical
content/layout references. Current screen anatomy, wireflows and 18 Desktop
journeys are in [Studio flows](2026-10-16-desktop-navigation-and-flows.md);
their [audit](../verification/implementation-reports/2026-10-16-wireframe-wireflow-userflow-audit.md)
links actual current screenshots. Static design artifacts are not runtime evidence.

## Mobile Screen Inventory

The product UX specification defines M1-M8. Several screens currently share `src/mobile/MobileApp.tsx`; the ID is a product surface ID, not necessarily a one-file component ID.

| ID | Screen | Primary job | Implementation evidence | Status / boundary |
| --- | --- | --- | --- | --- |
| M1 | Voice Home | Start a command, recording, note or recent work | `src/mobile/MobileApp.tsx` (`HomeScreen`) | `IMPLEMENTED`; on-device STT asset remains a gate |
| M2 | Live Capture | Record safely and expose durable chunk state | `src/mobile/MobileApp.tsx` (`CaptureScreen`), `src/mobile/captureOrchestration.ts` | `PARTIAL`; screen-off, kill/restart and extended lifecycle evidence remain open |
| M3 | Notes Library | Find notes and projects | `src/mobile/MobileApp.tsx` (`NotesScreen`) | `IMPLEMENTED` at code/test level; physical/reopen evidence remains open |
| M4 | Note Detail | Read/edit a selected note and follow related timeline | `NotesScreen` detail/editor in `src/mobile/MobileApp.tsx`; `src/mobile/TimelineScreen.tsx` | `PARTIAL`; shared nested state rather than an independent route; physical/source playback proof remains separate |
| M5 | Graph Explorer | Explore note, speaker, topic, decision and evidence relations | `src/mobile/MobileApp.tsx` (`GraphScreen`), `src/mobile/bridge.ts` | `IMPLEMENTED` at code/test level; provider/device proof remains open |
| M6 | Devices and Runtime | Pair Desktop and choose execution location | `src/mobile/MobileApp.tsx` (`DevicesScreen`), `src/mobile/bridge.ts` | `PARTIAL`; transport/delegation implementation and historical LAN evidence exist; current APK/device requalification remains open per Mobile status |
| M7 | Job Detail | Track queued, running, paused, failed and completed work | `src/mobile/MobileApp.tsx`, `src/mobile/CreativeStudio.tsx` | `PARTIAL`; executor and lifecycle evidence remain open |
| M8 | MCP and Privacy | Enable bounded tools and inspect access | `src/mobile/MobileApp.tsx`, `src/mobile/bridge.ts` | `PARTIAL`; contract/build evidence exists, device-to-client interoperability remains open |

## Shared Design-System Inventory

| ID | Shared surface/rule | Source | Status |
| --- | --- | --- | --- |
| S01 | Color, typography, spacing, radius, material and elevation tokens | [`fung.tokens.json`](../FUNG_Design_System_v0.1.0/tokens/fung.tokens.json) | `IMPLEMENTED` as token source; CSS integration is verified; full component coverage is partial |
| S02 | App shell, panel zones and ownership rules | [Studio flows](2026-10-16-desktop-navigation-and-flows.md), [`DESIGN_SYSTEM.md`](../FUNG_Design_System_v0.1.0/DESIGN_SYSTEM.md) | `IMPLEMENTED` as design contract; screen implementation status is tracked per surface above |
| S03 | Buttons, segmented controls, signal cards and transcript/evidence rows | [`DESIGN_SYSTEM.md`](../FUNG_Design_System_v0.1.0/DESIGN_SYSTEM.md) | `SPECIFIED` as shared guidance; implementation coverage is tracked per surface above |
| S04 | Loading, empty, error, offline and model-missing states | [`04-components.md`](../Desktop/04-components.md) | `IMPLEMENTED` as rule; each new component must map its state coverage |
| S05 | Thai-first Mobile behavior and selected brand assets | [`PRODUCT_UX_SPEC.md`](../Mobile/PRODUCT_UX_SPEC.md), [`Mobile/design-system/assets/`](../Mobile/design-system/assets) | `IMPLEMENTED` as Mobile-specific contract and selected beta assets |
| S06 | Shared interactive design-system reference | [`docs/design-system/index.html`](../design-system/index.html) | `GENERATED` from the DTCG token source; do not edit HTML manually |

## Browser Route Inventory

| ID | Entry | Owner | Boundary |
| --- | --- | --- | --- |
| W01 | `/` and unmatched browser paths | `src/web/LandingPage.tsx` | Landing; not native application evidence |
| W02 | `/app` | `src/web/Dashboard.tsx`, `src/web/AuthGuard.tsx` | Requires configured Supabase/session; no real sign-in performed in current flow audit |
| W03 | `/auth/callback` | `src/web/AuthCallback.tsx` | Configuration and OAuth callback boundary; real provider ceremony remains separate |
| W04 | `/app?surface=desktop` or `mobile` | `src/main.tsx`, `src/lib/bootstrap.ts` | Desktop browser preview has no native bridge; Mobile browser route requires configured Supabase |

Mobile source uses bottom tabs Home/Files/Voice/Notes/Devices, with nested Graph
and Timeline. M1-M8 are product domains, not eight standalone router paths.
Source-only observations and device/runtime gates are in the current flow contract.

## Plugin Interaction Boundary

| ID | Surface | Owner | Evidence | Status |
| --- | --- | --- | --- | --- |
| P01 | Meeting-artifacts interactive workflow | Claude Code, Codex or Antigravity host UI | `.agents/plugins/fung-meeting-artifacts/skills/`, `bin/`, `server/` | `HOST-OWNED`; the plugin has no standalone FUNG UI |

The plugin's transcript, model, format and destination questions are rendered by the host agent. Google Docs, Google Sheets and LINE delivery are external integrations and are not counted as FUNG-owned UI surfaces.

## Known Inventory Gaps

1. D01-D14 / M1-M8 / W01-W04 map product domains and entry owners; not every nested state has an independent stable ID.
2. Desktop shell surfaces use `home|live|review|output|appearance`, while historical domain content and Mobile use P1-P4/M1-M8 IDs; the crosswalk is documented but is not a generated machine-readable graph.
3. Implementation status and release/UAT status are intentionally different; a row marked `IMPLEMENTED` can still have runtime, device, provider or release gates open.
4. Screenshot, keyboard, physical-device and clean-install evidence are not attached to every row; the new wireframes describe structure and state coverage, not execution proof.
5. Mobile Graph search remains unimplemented; the enabled icon with no handler was removed in this repair. Current native/device/provider gates are retained in platform status, not closed by browser navigation.

## Update Rules

- Add or update an inventory row whenever a user-facing surface, state or platform boundary changes.
- Include the implementation file and the evidence/status source in the same change.
- Keep generated CSS, contrast report and board derived from the DTCG source; do not hand-edit generated artifacts.
- Do not mark a surface release-ready from source presence alone; update the corresponding implementation-status and release-gate documents.

## Version Diff

| Version | Change |
| --- | --- |
| 0.5.1b | Clarified that `05-sitemap-ia.md` is only a superseded P1–P4 draft and that the Studio flow contract owns current navigation. |
| 0.5.0b | Reframed Desktop navigation around three task destinations and clarified Storage location as a secondary destination rather than a file library. |
| 0.4.0b | Reconciled current Studio dock/five surfaces, overlays, platform entry routes, legacy wireframes and flow evidence boundaries. |
| 0.3.0b | Reconciled the September Liquid Glass shell, active surfaces, Appearance page, hover sidebar and profile header. |
| 0.2.0b | Added complete Desktop P1-P4 page-level wireframes and linked them to the inventory. |
| 0.1.0b | Added the first cross-platform UI interface inventory and separated UI implementation status from runtime/UAT/release readiness. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.5.1b | 2026-10-19 | beta | Corrected the IA index so the superseded P1–P4 draft cannot be mistaken for current shell navigation. | working-tree | Codex |
| 0.5.0b | 2026-10-19 | beta | Reconciled the task-first Desktop sitemap, utility destinations and Home inventory. | working-tree | Codex |
| 0.4.0b | 2026-10-16 | beta | Updated D01-D14, Mobile handoffs and W01-W04; linked current wireframes/flows/audit and corrected legacy layout/status pointers. | working-tree | Codex |
| 0.3.0b | 2026-09-20 | beta | Synced the cross-platform interface inventory with the current DesktopShell and Liquid Glass active-surface contract. | 285566b9515c5fc83b4fde64ce8e57389f7a565a | RWANG |
| 0.2.0b | 2026-08-22 | draft | Added and indexed the complete Desktop P1-P4 structural wireframe set. | N/A — uncommitted | ATHER |
| 0.1.0b | 2026-08-22 | draft | Added Desktop, Mobile, Shared Design System and Plugin Boundary inventory. | N/A — uncommitted | ATHER |
