---
version: "0.1.0b"
created_at: "2026-09-17T01:41:09+07:00,Codex,gpt-5.6-luna,max,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-17T01:41:09+07:00,Codex"
status: "candidate"
superseded_by: null
base_sha: "c378af9fac3c00db063948f49f9ee857ebad9126"
branch: "codex/callmd-ui-dag"
attributes:
  domain: "FUNG desktop Call.md-inspired UI adaptation"
  doc_type: "implementation-report"
  scope: "bounded desktop dependency and contract scan; no implementation"
  evidence_state: "static repository evidence only"
---

# FUNG Call.md desktop contract scan

## Result

This is a documentation-only candidate report at base
c378af9fac3c00db063948f49f9ee857ebad9126. No product implementation,
credentials, app data, source-reference checkout, commit, push, or deployment
was performed.

The P1 UI-resurface subset is feasible against existing FUNG contracts:
current live capture, live transcript events, session controls, recording-scoped
transcript loading, recording-scoped summaries, existing job actions, and
export actions can be rearranged without inventing a backend.

The full P1 wording is not backend-free. Multi-recording history selection and
desktop playback have no complete desktop Tauri contract, and Q&A is currently
project-scoped rather than recording-scoped. Those capabilities are
WAITING_DEPENDENCY unless P1 is explicitly reduced to the current recording
and an honest playback-unavailable state.

P2 bookmarks, agenda state, derived metrics, and proactive assistance are
gated. The current code has no durable bookmark/agenda/metric tables or
commands. They require a reviewed persistence and DTO contract before any
implementation dispatch.

## Scope and evidence boundary

Read in the requested order: AGENTS.md; the master implementation plan;
Desktop architecture; Mobile implementation status; Desktop real progress;
the frontend redesign brief; and the Luna–Terra workflow. The current
Call.md source was not inspected in depth. This report records repository
facts observed at the pinned base, not runtime or production claims.

Primary governance evidence:

- docs/Desktop/ARCHITECTURE.md:21-90,113-149 — desktop Tauri/React/local
  API/Genesis boundary, job states, entities, and local-first constraints.
- docs/design/FRONTEND_REDESIGN_BRIEF.md:77-111,165-208,224-280 — preserved
  feature inventory, unavailable marker/compare/export actions, existing
  command/event DTOs, local API routes, egress and Thai-first constraints.
- docs/Desktop/08-real-progress.md:619-696 — implemented versus partial
  desktop inventory; no packaged/device/release claim.
- docs/plans/2026-08-23-fung-luna-terra-multiagent-workflow.md:87-114,116-136
  — at most three disjoint workers, serialized shared contracts, and review
  gates.
- docs/plans/2026-09-17-callmd-ui-luna-max-dag-workflow.md:76-80,163-180 —
  current candidate split: P1 resurface, P2 gated annotations/metrics, and
  single owners for shared files. This was read as coordination context only.

## Verified dependency graph

The desktop path is:

App.tsx → src/tauri.ts invoke/listener bridge → Tauri commands/events →
Rust live/job/intelligence services → GenesisBlockDB through
genesis_adapter.

The local API is a separate loopback/LAN surface. It is not a substitute for
an unreviewed desktop WebView fetch path.

### Frontend and bridge

- src/App.tsx:627-639 refreshes only health, projects, jobs, and providers.
  src/App.tsx:648-687 derives the visible library from projects and each
  project's activeRecordingId; it does not enumerate recordings.
- src/App.tsx:696-720 loads a transcript using the exact
  (projectId, recordingId) pair and guards stale completions.
- src/App.tsx:789-869 intentionally derives activity/events only from real
  transcript rows, providers, or jobs; it does not fabricate a history feed.
- src/App.tsx:1094-1159,1178-1222 routes actions through jobActions,
  requires a selected project and active recording, and reports unavailable
  actions instead of creating inert jobs.
- src/App.tsx:1232-1255 mounts RecoveryNotice, SettingsPanel, and
  LiveMeetingPanel at lazy/UI boundaries. src/App.tsx:1579-1634 keeps Search
  unhandled and passes export/recording controls through the rail.
- The shared command path is src/tauri.ts, not src/lib. Its actual
  wrappers/types are at src/tauri.ts:40-76,152-221,324-338,394-475,
  585-705,708-768.
- src/components/LiveMeetingPanel.tsx:38-141 owns in-memory live segments,
  status/topic/summary listeners, recording IDs, and stale-event protection.
  :153-220 starts/stops/asks and reloads summaries. :222-444 renders the
  live overlay and summary/external-tool surfaces; no bookmark, agenda,
  recording-history, or desktop playback command is present.
- src/components/SettingsPanel.tsx:1-42,78-156,158-281 owns the lazy
  settings boundary and local API connect/LAN controls. It has no meeting
  history, bookmark, agenda, metric, or source-playback surface.

### Existing command/data contracts

| Capability | Verified current contract | Boundary |
|---|---|---|
| Project/job shell | listProjects, listJobs, listModelProviders; App polls these | No recording list; jobs are capped to the newest 30 in src-tauri/src/lib.rs:1096-1128. |
| Live lifecycle | liveMeetingStart returns projectId, recordingId, jobId, devices, warning; stop/status use the same recording identity | src-tauri/src/live_meeting.rs:1534-1553,1607-1843; active session control is in memory. |
| Live events | live-status, live-segment, live-topic, live-summary; all carry recordingId | src-tauri/src/live_meeting.rs:89-110; topic events are event-only. |
| Transcript | listTranscriptSegments(projectId, recordingId); correction also requires both IDs | src/tauri.ts:450-475; current App path is recording-scoped. |
| Summary | meetingSummaries(projectId, recordingId) and generateMeetingSummary(projectId, recordingId) | Actual read attribution is through model_runs.recording_id in src-tauri/src/meeting_intel.rs:1019-1086; preserve excluded/unknown counts. |
| Q&A | meetingAsk(question, projectId?); sources may contain recordingId | src-tauri/src/meeting_intel.rs:224-280 filters transcript rows by project only. Active-recording isolation is not a current contract. |
| Jobs/actions | Five runnable kinds in src/lib/jobActions.ts:16-118: summary, transcript retry, graph build, diarize, export | capture.marker, summary.compare, export.queue, and related actions are explicit unavailable states. |
| Export | createJob("export.render", projectId, recordingId) plus listExportArtifacts(projectId) | src/tauri.ts:324-338; artifact listing is project-scoped and does not expose a recording selector to the desktop UI. |
| External tools | Suggest/execute/cancel/revoke/history are recording-scoped | src/tauri.ts:708-768; preserve approval and evidence refs. |

### Identity, recovery, and event/session behavior

- Durable meeting identity is recording_id, normally paired with project_id.
  Capture rows, audio chunks, transcript segments, model runs, summaries, and
  external tool runs use that scope.
  src-tauri/src/live_meeting.rs:713-766,1607-1708;
  src-tauri/src/meeting_intel.rs:481-540,1019-1086.
- The live process additionally holds project_id, recording_id, job_id, and
  an in-memory Instant in LiveSessionControl.
  src-tauri/src/live_meeting.rs:63-87,1780-1789. There is no separate
  durable session/epoch/event sequence contract. After restart the in-memory
  status is inactive; recovery works from durable recording/job state.
- Live topic openPoints and actionItems are intentionally ephemeral and never
  stored as facts: src-tauri/src/meeting_intel.rs:1-10,34-126.
- Startup recovery scans and reports interrupted recordings; explicit recovery
  adopts orphaned chunks and fills transcript gaps, while stale capture jobs
  are failed: src-tauri/src/live_meeting.rs:1555-1605;
  src-tauri/src/lib.rs:1949-2001,3363-3424;
  src/lib/recoveryFlow.ts:11-59. No bookmark/agenda reconciliation exists.
- The frontend correctly uses the event's recording ID when reloading
  summaries and clears per-session counts on start:
  tests/summaryScoping.test.mjs:18-86. Preserve this isolation behavior.

### History and playback: available versus missing

- The local API does provide a real recording list, transcript, and audio
  playback surface: GET /recordings, GET /recordings/{id}/transcript, and
  GET /recordings/{id}/audio?channel=mic|system|file, with bearer/query
  token authorization and byte ranges. Evidence:
  src-tauri/src/local_api.rs:542-551,555-684,807-845,899-974,1014-1086.
- That surface is designed for the tokenized phone/web page and is exposed to
  Settings as a connect URL/LAN opt-in. There is no corresponding desktop
  src/tauri.ts recording-list or playback wrapper. The desktop rail keeps
  Play disabled with an explicit unavailable label:
  src/components/InstrumentRail.tsx:28-43,97-105.
- There is no verified desktop meeting-history command, event-history command,
  or playback adapter in the current bridge. list_jobs returns job rows only;
  the local API /jobs/{id} route returns a job object, while the historical
  contract's events field is not wired by that route:
  src-tauri/src/lib.rs:1066-1128;
  src-tauri/src/local_api.rs:622-637;
  contracts/local-api-v1.yaml:84-104.
- Therefore, a History lane may render a truthful unavailable/current-recording
  state using existing data, but a functional multi-recording review and
  desktop playback lane must wait for a reviewed backend/bridge contract. Do
  not make the desktop UI call the loopback API directly without deciding its
  token, CSP, egress, range, and failure semantics.

## Actual missing contracts

### P1 blockers

1. Recording enumeration. Add or explicitly decline a desktop command that
   returns recording metadata under (projectId, recordingId) scope, stable
   ordering, status, duration, source, language, and incomplete/recovery
   state. Existing App data exposes only project activeRecordingId.
2. Playback adapter. Decide whether desktop playback uses a new native Tauri
   command or the existing tokenized local API. The contract must cover
   channel selection, range/stream behavior, missing chunks, authorization,
   and truthful degraded playback. Until then, the current Play-disabled
   state is the only verified desktop behavior.
3. Q&A scope. If P1 means “ask about this meeting”, extend the command
   contract to accept recordingId and filter transcript/graph sources by it.
   If project-wide Q&A is intended, label that scope in the UI rather than
   implying current-session answers.
4. Reopen/session semantics. Decide whether reopening means selecting a
   durable recording or restoring a distinct session lifecycle. Current code
   supports the former through Genesis rows only after a recording-list
   surface; it does not persist a session epoch or live event log.

### P2 bookmark/agenda/metrics candidate contract

This is a proposed contract for review, not an implementation instruction.

- Common key: project_id, recording_id, id, created_at, updated_at; reject
  cross-project/cross-recording references.
- Bookmark row: at_ms required, reviewed kind, user text/label, optional
  transcript_segment_id, actor, and idempotency key. A bookmark is not an
  AI fact and must be rendered as user-created unless evidence says otherwise.
- Agenda row: stable position, title, reviewed state enum, optional start/end
  offsets, and optional transcript evidence references. If metrics require
  time-in-state or transition history, add an append-only agenda event/audit
  contract; a mutable row alone cannot prove that history.
- Derived metrics should initially be read-time counts from durable rows only:
  bookmark count and agenda total/open/completed/unlinked counts. Do not
  invent speaking-time, sentiment, intent, or completion metrics from channel
  labels or ephemeral topic text.
- Candidate commands, names subject to product review:
  meeting_annotations_list(projectId, recordingId),
  meeting_bookmark_create(input),
  meeting_bookmark_delete(projectId, recordingId, bookmarkId),
  meeting_agenda_upsert(input), and
  meeting_agenda_set_state(projectId, recordingId, itemId, state).
  Mutation results should return the durable row plus a revision/timestamp.
- Candidate event: one reviewed annotation-change event carrying projectId,
  recordingId, entity type/id, and revision. Do not overload live-topic; that
  event is ephemeral and model-derived.
- Every mutation and its audit row must commit through Genesis in one
  transaction. Current schema is v10 and has no annotation tables:
  src-tauri/src/genesis_adapter.rs:543-585,626-675,781-826,938-1009.
  schemas/sqlite-wal-v1.sql:1-8 identifies the old SQLite file as migration
  input, not the new authority. A future schema migration and contract must
  own the Genesis path; do not put this feature in the legacy SQL file.

## Ordered candidate DAG and disjoint write lanes

The following is a candidate implementation graph. It is gated by feature
documentation, wireframes, independent review, and Boss approval; it is not an
authorization to edit code.

| Order | Node | Dependencies | Exclusive write partition |
|---|---|---|---|
| 0 | SCAN_REPORT | pinned base only | This report path only. |
| 1 | P1_CONTRACT_AND_UX | SCAN_REPORT plus source/workflow reports | New contract/wireframe docs; no code. Freeze whether history, playback, and Q&A are full or explicitly unavailable. |
| 2 | APPROVAL | P1_CONTRACT_AND_UX plus independent document review | Boss approval of exact scope and contract revision. |
| 3a | UI_SHELL | APPROVAL; existing bridge only | src/components/desktop/DesktopShell.tsx NEW, .css NEW, lane tests/report. No App.tsx, src/tauri.ts, Rust, or SettingsPanel. |
| 3b | UI_LIVE | APPROVAL; existing live contracts | src/components/desktop/LiveWorkspace.tsx NEW, .css NEW, src/components/LiveMeetingPanel.tsx, lane tests/report. Single owner of LiveMeetingPanel; no backend or App.tsx. |
| 3c | BACKEND_RECORDING_REVIEW | APPROVAL and accepted history/playback contract | src-tauri/src/recording_review.rs NEW plus explicitly acquired command-registration path in src-tauri/src/lib.rs. src-tauri/src/local_api.rs is owned here only if the reviewed choice extends/reuses loopback playback. No frontend files. |
| 4 | SHARED_BRIDGE | BACKEND_RECORDING_REVIEW if new commands are needed | Sole owner of src/tauri.ts, src/components/desktop/contracts.ts NEW, shared DTO tests/report. No UI layout or schema edits. |
| 5 | UI_HISTORY | SHARED_BRIDGE and backend acceptance | src/components/desktop/RecordingReview.tsx NEW, .css NEW, lane tests/report. No invented API; no App.tsx or shared bridge edits. |
| 6 | INTEGRATE | UI_SHELL, UI_LIVE, UI_HISTORY and their reviews | Sole owner of src/App.tsx, root styles, route mounting, and any explicit SettingsPanel/InstrumentRail wiring. |
| 7 | P2_ANNOTATION_CONTRACT | separate P2 approval | contracts/meeting-annotations-v1.yaml NEW and docs/design/meeting-annotations.md NEW. No implementation. |
| 8 | P2_ANNOTATION_BACKEND | P2 contract review | src-tauri/src/meeting_annotations.rs NEW, reviewed Genesis migration in src-tauri/src/genesis_adapter.rs, command registration in src-tauri/src/lib.rs, backend tests. Single persistence owner. |
| 9 | P2_ANNOTATION_UI | P2 backend and bridge review | src/components/desktop/MeetingAnnotations.tsx NEW, .css NEW, UI tests. src/tauri.ts remains owned by the shared-contract lane. |
| 10 | VERIFY_AND_REVIEW | integrated candidate | Read-only verification reports; no orchestrator fixes. Native/runtime/provider gates remain separate. |

Shared-file ownership is serialized: App.tsx belongs only to INTEGRATE;
src/tauri.ts to SHARED_BRIDGE; LiveMeetingPanel.tsx to UI_LIVE;
SettingsPanel.tsx to a separately approved settings/runtime lane;
src-tauri/src/lib.rs to the backend/integration owner; genesis_adapter.rs
to the persistence owner; live_meeting.rs to lifecycle/recovery; and
meeting_intel.rs to Q&A/summary/topic behavior. No parallel lane should
touch src/lib/jobActions.ts, src/lib/meetingSummaries.ts, or
src-tauri/src/job_engine.rs without an explicit contract-specific lease.

## Preserved feature inventory

Keep Tauri v2/React/Rust/Genesis and the Quiet Archive brand; local-only
storage and model provenance; microphone/system capture semantics; transcript
correction and speaker labels; recovery notice and explicit recovery;
summary/export job behavior and recording scoping; import/file/URL/Zoom paths;
pairing/FUNGWIRE; local API token/CORS/LAN opt-in; external account and
approval/evidence boundaries; CSP and no-direct-egress rules; lazy settings,
mobile/web routing, Thai-first copy, light/dark, focus, and truthful
loading/empty/error states.

Do not revive canceled Google Drive work, add cloud STT/VideoDB/screen
recording/calendar/OAuth automation, claim mic/system channels are verified
people, or fabricate metrics from missing persistence.

## Acceptance and verification

### Required before any code dispatch

- Reviewed P1 contract/wireframes explicitly choose current-only resurface,
  full history/playback, or separate gated nodes.
- Every new command has exact args, return DTO, project/recording scope,
  error/unavailable semantics, and one owner.
- P2 annotation schema/metrics remain outside P1 unless separately approved.
- No shared-file write partition intersects a running lane; base SHA and
  accepted contract revision are pinned.

### Candidate implementation acceptance

- Existing commands and event names remain compatible; transcript and summary
  scope never crosses recordings.
- History reads real Genesis data; playback uses the approved local/native
  path and reports missing chunks/errors without empty-success behavior.
- Restart/recovery states are visible and never represented as a live session
  without a live status response.
- Thai-first light/dark/loading/empty/error/focus states and existing recovery,
  settings, pairing, import, export, and mobile/web routes remain reachable.
- Bookmark/agenda metrics are derived only from persisted reviewed rows and
  evidence; no inferred facts are presented as verified facts.

### Verification status for this scan

- PASS — pinned branch/base SHA and requested read-only scope were used.
- PASS — current command path confirmed as src/tauri.ts; no src/lib/tauri
  path was assumed.
- PASS — evidence-backed blocker identified for full P1 history/playback and
  recording-isolated Q&A.
- PASS — new DAG plan/task manifest were read and their P1/P2 gate split and
  shared-file ownership were cross-checked against this scan.
- NOT_RUN — machine JSON parse, unique-ID/edge validation, acyclicity, and
  path-intersection validation for the new task manifest.
- NOT_RUN — frontend build, Rust format/clippy/test, CI, browser, native
  Tauri cold boot, device, hosted-CI, provider, production, or playback run.
- NOT_RUN — independent Terra review and Boss approval.

## Unknowns and blockers

- Product decision for the exact P1 meaning of history/review, recording
  ordering, and reopen behavior is still required.
- Desktop reuse of the tokenized local API versus a native playback bridge is
  unresolved; no assumption was made.
- Whether Q&A should remain project-wide or become recording-scoped is
  unresolved.
- Bookmark/agenda taxonomy, transition history, and metric definitions are
  unresolved; no implementation should infer them from UI labels.
- No runtime/provider/device evidence was collected in this bounded scan.

## Version Diff

- new → 0.1.0b: added the evidence-backed desktop dependency map, exact
  existing command/data boundaries, full-P1 blockers, P2 persistence
  prerequisites, ordered candidate DAG, exclusive write partitions, preserved
  feature inventory, and explicit verification status.
- Product implementation: unchanged.
- Other files: unchanged by this worker.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Bounded FUNG desktop contract and DAG scan; no implementation edits | UNCOMMITTED; base c378af9 | Codex / gpt-5.6-luna max |
