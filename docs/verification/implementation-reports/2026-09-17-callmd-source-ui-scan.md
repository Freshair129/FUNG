---
version: 0.1.0b
status: candidate
generated_at: 2026-09-17T01:38:13+07:00
created_at: "2026-09-17T01:38:13+07:00,Luna max,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-17T01:49:00+07:00,Codex metadata normalization"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "verification-report"
  scope: "Call.md source UI evidence; no implementation"
base_sha: c378af9fac3c00db063948f49f9ee857ebad9126
branch: codex/callmd-ui-dag
source_snapshot: F:/call.md-main
---

# Call.md source/UI scan

## Scope and evidence boundary

This is a bounded, read-only source/UI scan for the FUNG Call.md adaptation. It covers the Call.md renderer entry points, selected stores/hooks, Electron preload/IPC and tRPC edges, selected main-process services, package metadata, README feature claims, and `F:/call.md-main/screenshot.png` (inspected at 3590x1950). It does not deep-scan FUNG backend/Genesis, perform an exhaustive upstream audit, or implement any feature.

The source is a local snapshot without a `.git` directory; `F:/call.md-main` therefore has no verified source commit. `package.json` identifies `call-md` 1.0.4, author VideoDB, and declares MIT (`F:/call.md-main/package.json:2-11`). That is package metadata only, not a legal conclusion or a complete dependency/license audit; the lockfile and legal notices were not audited.

Required FUNG planning/architecture context was read before this scan: `docs/plans/2026-08-09-fung-master-implementation-plan.md`, `docs/Desktop/ARCHITECTURE.md`, `docs/Mobile/IMPLEMENTATION_STATUS.md`, `docs/Desktop/08-real-progress.md`, `docs/design/FRONTEND_REDESIGN_BRIEF.md`, and `docs/plans/2026-08-23-fung-luna-terra-multiagent-workflow.md`. The result below preserves their local-first Tauri/React/Rust/Genesis, Thai-first, existing-brand, auth/egress/approval-boundary constraints.

## Executive findings

1. The current main renderer is `main.tsx -> TrpcProvider -> App`; `App` mounts `NewSidebar`, auth/onboarding, Home/History/Settings, and a recording branch. The live recording branch is `RecordingHeader + LiveAssistPanel + MetricsBar + optional MeetingAgendaPanel + TranscriptionPanel` (`F:/call.md-main/src/renderer/main.tsx:35-63`; `F:/call.md-main/src/renderer/App.tsx:1-40,453-478,710-808`). `TopStatusBar` and `CallSummaryView` are mounted only in the completed-call summary branch (`App.tsx:393-423`).

2. The screenshot is not evidence of the current renderer tree. It shows a central Live Transcription feed, Bookmark control, Enabled toggle, Avoid Saying cards, sentiment cards, and a right Conversation Metrics card. Current source has a static `MetricsPanel` with similar labels but it is not imported by `App`; current live layout uses `MetricsBar` and the summary-only `TopStatusBar`. There is no renderer bookmark action. Preload exposes bookmark IPC, but no current main-renderer mount was found (`F:/call.md-main/src/renderer/components/copilot/MetricsPanel.tsx:156-266`; `F:/call.md-main/src/renderer/components/recording/MetricsBar.tsx:11-47`; `F:/call.md-main/src/preload/index.ts:111-145,415-434`).

3. Call.md's useful visual patterns are presentational only. Its recording/session behavior is coupled to VideoDB session tokens, API keys, cloud capture/transcription/visual indexing, and an OpenAI-compatible VideoDB proxy. These runtime paths must not be transplanted into FUNG's local-first surface (`F:/call.md-main/src/renderer/hooks/useSession.ts:27-31,56-165`; `F:/call.md-main/src/main/services/videodb.service.ts:17-58,151-179`; `F:/call.md-main/src/main/services/llm.service.ts:1-5,92-108,185-210`).

4. Phase 1 should resurface only capabilities already accepted and evidenced in FUNG, using Call.md as a visual/presentational reference. Bookmarks, new agenda generation, new metrics semantics, sentiment/Avoid Saying, and new assists remain P2/gated until their FUNG contracts, local persistence, Thai behavior, and approval/egress boundaries are reviewed.

## Mounted versus static-only renderer findings

### Verified mounted paths

| Surface | Evidence | Status and adaptation note |
|---|---|---|
| Main root and local tRPC provider | `renderer/main.tsx:9-63` | Mounted. The provider targets the local loopback tRPC endpoint; it is not itself proof of offline/local inference. |
| Navigation shell | `App.tsx:4,835`; `components/layout/NewSidebar.tsx:140-202` | Mounted. `Sidebar.tsx` is not the active shell. |
| Home, history, settings | `App.tsx:781-808`; `HomeView.tsx:18-19,633-640`; `HistoryView.tsx:1-5,86-94`; `SettingsView.tsx:391-432` | Mounted. These are composition references, not FUNG backend contracts. |
| Meeting setup | `App.tsx:770-779`; `MeetingSetupFlow.tsx:5-7,200-235` | Mounted flow imports `InfoStep`, `QuestionsStep`, and `ChecklistStep`. |
| Live recording composition | `App.tsx:341-380,453-478` | Mounted when session is recording. `mcpFindings` is explicitly an empty TODO placeholder at `App.tsx:377-378`. |
| Transcript presentation | `components/transcription/TranscriptionPanel.tsx:33-76,150-229` | Mounted. It merges mic/system-audio items and renders pending/final states; FUNG must provide its own provenance/speaker contract. |
| Live assists | `components/recording/LiveAssistPanel.tsx:201-227,338-440`; `hooks/useLiveAssist.ts:20-73` | Mounted in the live branch. The UI pattern is reusable; the service is cloud-LLM coupled. |
| Metrics/agenda presentation | `MetricsBar.tsx:11-47`; `MeetingAgendaPanel.tsx:76-149` | Mounted as a compact metrics bar and only when a checklist exists. The full metrics card is not the mounted live surface. |
| Completed-call summary | `App.tsx:393-423`; `TopStatusBar.tsx:19-34,84-178` | Mounted only after call completion. Do not infer screenshot-time live placement from this branch. |
| Floating widget | `main/windows/widget.window.ts:60-129`; `renderer/widget/index.tsx:6-13`; `renderer/widget/App.tsx:24-116`; `main/ipc/capture.ts:623-629` | Separately mounted by Electron after recording starts. It is not part of the main `App` tree. |

### Static-only, legacy, or unreachable from the current main `App` import graph

The following are static reachability findings, not deletion recommendations:

- `components/layout/Sidebar.tsx:8-73` and `components/layout/TitleBar.tsx:13-36` have definitions but no current root import. `App` imports `NewSidebar`; Electron creates a hidden-inset titlebar window (`main/index.ts:131-159`).
- `components/copilot/MetricsPanel.tsx:156-266` is exported from `components/copilot/index.ts:7` but is not imported by `App`; its labels closely resemble the screenshot's right metrics card. It is a candidate reference, not verified mounted UI.
- `components/mcp/MCPResultCard.tsx:66-233`, `MCPResultsOverlay.tsx:24-105`, and `MCPStatusIndicator.tsx:58-146` are exported, but no main `App` path imports the overlay/status components. The result card is only used by its own overlay. `SettingsView.tsx:20-23,402-412` mounts `MCPServersPanel`, which is a separate settings surface.
- `components/meeting-setup/MeetingInfoPanel.tsx:8-109` and `SourcesStep.tsx:7-31` are exported by the meeting-setup index, but `MeetingSetupFlow.tsx` imports only the three step components listed above. `StreamToggles.tsx` is only reached by those static `SourcesStep`/`SessionControls` paths; `SessionControls.tsx:9-100` itself is not in the current recording composition.
- `components/settings/CalendarPanel.tsx:67-474`, `components/history/RecordingDetailsModal.tsx:77-301`, and `components/auth/AuthModal.tsx:20-111` have definitions but no current route/import in the inspected main renderer path. Calendar is handled through other mounted banner/setup/home paths.
- `SentimentIcons.tsx:2-138` supplies icon types/helpers, and `NudgeToast` recognizes a sentiment nudge category, but no dedicated mounted screenshot-like sentiment card was found. The screenshot's sentiment treatment is therefore not a verified current surface.

## Screenshot versus current source

The inspected screenshot is a visual reference only. It contains an Electron-style Meeting Copilot window with a left rail, top REC/status strip, central live transcript, Bookmark button, Enabled toggle, Avoid Saying cards, Customer/Overall Sentiment, and a right Conversation Metrics panel. The current source differs in at least these verified ways:

- The current live composition is a two-column `LiveAssistPanel` plus right `MetricsBar`/agenda/transcript (`App.tsx:453-478`), not the screenshot's central transcript plus screenshot-like right rail.
- `MetricsPanel` contains `Conversation Metrics`, WPM, questions, and health markup (`MetricsPanel.tsx:161-266`) but is static-only from the current `App` import graph.
- Renderer search found no Bookmark button/action. Bookmark calls exist only in the preload API (`preload/index.ts:143-144,431-433`), so the screenshot cannot be used as evidence that bookmarks are currently mounted or end-to-end.
- The current source has no mounted Avoid Saying/sentiment card matching the screenshot. A sentiment type exists in a transcript/nudge model, but the screenshot composition is not verified.
- The screenshot's visible values and labels are not current-runtime evidence; no live Electron launch or DOM capture was run.

## Stores, hooks, and dependency edges

### Renderer state and event flow

- `config.store.ts:4-61` persists `accessToken`, `userName`, `apiKey`, and onboarding state in a Zustand persist store. This is a direct boundary conflict with FUNG's keyring-only BYOM/cloud-key rules; do not copy this storage model.
- `session.store.ts:3-62,68-83,166-171` carries VideoDB session identifiers/tokens and expiry state. `useSession.ts:56-165` obtains a token, creates a VideoDB capture session through tRPC, then starts Electron capture with `sessionToken`, `accessToken`, and `apiUrl`.
- `useGlobalRecorderEvents.ts:9-14,37-154` is mounted once at `App` level. Final transcript events update the transcription store and are forwarded through Electron IPC to Copilot and Live Assist (`:60-89`); visual-index events update local store, Live Assist, and a persistence IPC call (`:93-129`).
- `copilot.store.ts:21-64,69-163`, `live-assist.store.ts:11-66`, `mcp.store.ts:24-104`, `meeting-setup.store.ts:1-93`, `transcription.store.ts:1-77`, and `visual-index.store.ts:1-54` are separate state domains. No FUNG store contract should be inferred from their names or shapes.

### Electron/tRPC/cloud edge map

```text
React App
  -> local tRPC client: http://localhost:<port>/api/trpc + x-access-token
  -> protected tRPC context resolves local user from access token
  -> token/capture/transcription/visual-index procedures
  -> VideoDB connect({ apiKey, baseUrl }) and VideoDB capture/session APIs

React hooks
  -> contextBridge electronAPI
  -> main IPC capture / copilot / live-assist / MCP / calendar handlers
  -> VideoDB recorder, VideoDB-proxy LLM, MCP servers, or Google OAuth as selected
```

Evidence: `renderer/api/trpc.ts:7-41`; `main/server/trpc/context.ts:8-29`; `main/server/trpc/trpc.ts:9-25`; `main/server/trpc/router.ts:12-22`; `main/server/trpc/procedures/capture.ts:1-23`; `transcription.ts:33-50,219-263`; `visual-index.ts:75-111,190-196,245-251`; `preload/index.ts:322-350,635-649`.

The Copilot/assist path is also cloud-dependent: `useCopilot.ts:37-56,61-104` initializes with the renderer-held API key; `main/ipc/copilot.ts:76-78` receives it; `sales-copilot.service.ts:87-132` initializes `LLMService`; `live-assist.service.ts:231-266` calls LLM JSON completion every 20 seconds; `llm.service.ts:92-108,185-210` defaults the OpenAI client to the VideoDB API base. This is incompatible with an unapproved FUNG cloud path.

MCP and calendar are separate egress risks: `useMCP.ts:53-87,92-285,290-392` drives IPC server/tool operations, while `google-calendar.service.ts:34-41` uses a bearer token for remote calendar API access. Treat both as opt-in, approval-gated integrations in FUNG.

## Thai and metric risks

The Call.md metric implementation counts words by splitting on ASCII whitespace (`conversation-metrics.service.ts:65-80,166-210`). Thai text commonly has no inter-word spaces, so this can undercount and distort WPM. Question counting is also a literal `?` test (`conversation-metrics.service.ts:213-217`). The displayed WPM/health values must not be ported as Thai-correct semantics without a reviewed tokenizer/locale contract. English prompt limits also appear in `meeting-setup.prompts.ts:21,48`; they are not evidence of Thai-safe limits.

FUNG UI copy must remain Thai-first. English Call.md labels and the screenshot's English product language are not copy authority. Any missing or unsupported FUNG metric should remain an explicit `UNKNOWN`/not-available state until its source and calculation are approved; do not fabricate parity values from the screenshot.

## Feature priority map for FUNG adaptation

| Feature/surface | Phase 1 recommendation | P2/gated condition |
|---|---|---|
| Live transcript/timeline | Resurface only an already accepted FUNG local meeting/transcript capability. Reuse the `TranscriptionPanel` message/pending/scroll pattern as presentation inspiration. | New speaker/provenance semantics or cloud transcription requires a reviewed FUNG contract. |
| Recording status and controls | Resurface existing FUNG local capture state using the `RecordingHeader`/status composition pattern. | New widget/window behavior is gated; Call.md widget is Electron-specific. |
| Agenda/checklist | Resurface only if FUNG already has the corresponding meeting-prep/checklist capability; use `MeetingAgendaPanel` as a collapsible presentation pattern. | New agenda generation, persistence, or completion semantics require a feature contract. |
| Metrics/health/talk ratio | Display only existing FUNG metrics with FUNG-owned definitions; `MetricsBar`/`MetricsPanel` are visual references. | New WPM, health scoring, trends, or Thai question counting require locale-safe definitions and tests. |
| Live assists | Resurface only an already approved FUNG local assist path; use `InsightSection`/card grouping as presentation inspiration. | New model inference, external provider, MCP-triggered assist, or egress requires approval and local/offline behavior. |
| Bookmarks | Not a verified mounted Call.md UI capability; preload IPC exists but no renderer action was found. | P2. Define local persistence, session/event identity, note/category semantics, replay/export behavior, and approval before code. |
| Avoid Saying / sentiment | Screenshot-only design signal; no matching mounted current card was verified. | P2. Requires an explicit FUNG signal contract, Thai copy/semantics, provenance, and no invented analysis. |
| MCP/external retrieval | Do not add to Phase 1. | P2 or later, default-off and bounded by FUNG egress/approval rules. |

## Suggested DAG and exact write partitions

This worker owns only the report path below. The following are integration boundaries for the parent orchestrator; they are not implementation changes.

```text
D0 Evidence report (this file, complete)
  -> D1 FUNG contract/source-of-truth review
  -> D2 Phase-1 presentation resurface: existing FUNG local capabilities only
  -> D3 Phase-1 visual/accessibility/Thai verification
  -> D4 P2 bookmark contract + persistence (gated)
  -> D5 P2 agenda/metrics contracts (gated, can run disjointly)
  -> D6 P2 assists/sentiment/egress contract (gated)
  -> D7 parent integration and final review
```

Suggested disjoint write partitions:

1. `P1-DESKTOP-UI`: the FUNG desktop entry surface named by the design brief, `src/App.tsx`, plus only its existing desktop presentational subtree. Own markup/styles/data adapters; do not edit `src-tauri/**`, Genesis, auth, egress, or shared persistence contracts.
2. `P1-MOBILE-UI`: `src/mobile/MobileApp.tsx` plus the existing mobile presentational subtree, only if the parent includes mobile in the approved slice. Do not edit native capture or bridge code.
3. `P1-WEB-UI`: `src/web/Dashboard.tsx` is out of this Phase 1 slice unless the parent explicitly activates it; keep it untouched otherwise.
4. `P2-BOOKMARKS`: one owner for bookmark UI plus its reviewed FUNG contract/persistence boundary; no edits to shared stores or Genesis by a UI-only worker.
5. `P2-AGENDA-METRICS`: one owner for agenda/metric presentational surfaces and contract tests; split agenda and metrics files internally if parallelized, with one owner for shared types.
6. `P2-ASSISTS-SENTIMENT`: one owner for assist/sentiment UI and egress/approval states; no Call.md VideoDB/OpenAI/MCP runtime imports.
7. `BRIDGE-BACKEND-GENESIS`: reserved for the FUNG backend/bridge worker. This scan makes no claim about its implementation status.

The `src/App.tsx`, `src/mobile/MobileApp.tsx`, and `src/web/Dashboard.tsx` anchors come from `docs/design/FRONTEND_REDESIGN_BRIEF.md`; the exact current component subpaths and ownership remain `UNKNOWN` because this bounded scan did not deep-scan FUNG.

## Verification and acceptance criteria

### Completed for this scan

- Required FUNG entry documents and relevant redesign/workflow documents read before analysis: `PASS`.
- Call.md renderer, selected stores/hooks, preload/IPC, tRPC and cloud-coupling scan completed within scope: `PASS`.
- `F:/call.md-main/screenshot.png` inspected with image viewer: `PASS`.
- Branch/base identity checked: branch `codex/callmd-ui-dag`, base `c378af9fac3c00db063948f49f9ee857ebad9126`: `PASS`.
- New-DAG artifact presence was observed in the worktree (`docs/plans/2026-09-17-callmd-ui-luna-max-dag-workflow.md` and `docs/plans/2026-09-17-callmd-ui-task-dag.json`); semantic DAG validation is `NOT_RUN`.
- No implementation code, credentials, `.env`, Google OAuth JSON, app data, commit, or push was changed/read by this worker: `PASS` within the stated boundary.

### Required before parent integration is accepted

- Confirm every Phase 1 UI maps to an already accepted FUNG capability and existing local Genesis/bridge contract; no Call.md cloud runtime is copied.
- Confirm renderer/bridge dependency direction: UI -> approved Tauri/React boundary -> FUNG local API/Genesis; no direct VideoDB/OpenAI/MCP/Google network path.
- Verify Thai-first strings, keyboard semantics, screen-reader names, and explicit loading/empty/error/unknown states.
- Define and test Thai-safe transcript/metric semantics before displaying WPM, word count, question count, health, or sentiment.
- Gate bookmarks, new agenda generation, new metrics semantics, assists, sentiment, and external retrieval behind approved contracts and local persistence/egress review.
- Verify screenshot parity only as a design target; do not treat it as runtime or feature evidence.

### Remaining checks deliberately not run

- Call.md `npm` build, typecheck, lint, unit tests, packaged Electron launch, and live DOM/screenshot verification: `NOT_RUN`.
- FUNG frontend/backend/Genesis implementation inspection, integration tests, device/package gates, and production readiness: `NOT_RUN` / owned by other workers.
- Semantic validation of the new DAG JSON/Markdown, worker disjointness, and parent merge integration: `NOT_RUN`.
- Dependency lockfile, transitive license/provenance, and legal review: `NOT_RUN`.
- Current source commit or upstream history verification: `UNKNOWN` because the source snapshot has no `.git`.

## Unknowns and non-claims

- Screenshot provenance, capture date, and source revision are `UNKNOWN`.
- The scan does not establish that any Call.md cloud, VideoDB, OpenAI, MCP, Google, bookmark, sentiment, or metric path is suitable for FUNG.
- The scan does not establish FUNG backend capability, FUNG current component paths beyond the design-brief anchors, or production readiness.
- No successful build/test/launch claim is made.
- The declared MIT package field is not a definitive legal approval for reuse; attribution and dependency review remain required.

## Version diff and changelog

### Version diff

- `0.1.0b`: initial candidate bounded source/UI scan; adds mounted/static reachability, screenshot-vs-source distinction, Electron/tRPC/cloud edges, Thai metric risks, feature priority map, suggested DAG/write partitions, and verification status.

### Changelog

| Version | Date | Status | Summary | Base SHA | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17T01:38:13+07:00 | candidate | Initial evidence-backed Call.md source/UI scan for FUNG adaptation | c378af9fac3c00db063948f49f9ee857ebad9126 | Luna max worker |
