---
version: "0.1.0b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "beta"
superseded_by: null
attributes:
  domain: "product-mcp"
  doc_type: "verification"
  scope: "MCP-00 baseline inventory for FUNG Desktop, Mobile, developer tooling, and local APIs"
  language: "Thai-first"
---

# FUNG Product MCP baseline inventory

## Outcome

**MCP-00 source inventory is accepted after independent review.** The
Desktop product MCP tool set in `contracts/local-mcp-v1.yaml` is a contract,
not a registered Desktop MCP server. The repo contains three distinct,
implemented surfaces: a fixed developer workflow MCP, an opt-in Mobile MCP
gateway, and an outbound Desktop external-MCP client. The authenticated local
HTTP API is a fourth surface; it is not an MCP server.

This report records the source state at the revision below. It does not claim a
fresh packaged-app or external-client interoperability run.

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-20 |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Source HEAD | `1ccf1075120e294c15bae57183adf8af392ed0dd` |
| Scope | Read-only source and documentation inventory |
| Fresh tests or app interactions | None in this audit |

## Runtime surfaces

| Surface | Current behavior | Evidence and boundary |
| --- | --- | --- |
| Developer workflow MCP | Three fixed tools: `fung.workflow.describe`, `fung.workflow.run_acceptance`, `fung.workflow.list_runs`. The runner accepts no arbitrary path, shell command, model, or profile. | `scripts/fung-workflow-mcp.mjs:44-82,247-400`; contract `contracts/workflow-acceptance-mcp-v1.yaml`. A passing synthetic import/edit/export/reopen run is documented in [workflow acceptance](2026-10-20-workflow-mcp-acceptance.md), on its recorded earlier source revision; it is not a fresh run at this report's HEAD. |
| Desktop product MCP server | No Desktop `tools/list`/`tools/call` registry for `local-mcp-v1` was found. The Desktop Tauri command bridge is not itself an MCP server. | `src-tauri/src/lib.rs:7161-7320` registers Tauri commands; no Desktop product MCP transport/dispatcher is registered there. `docs/Desktop/08-real-progress.md:1555-1561` also records the full product MCP server as not implemented. |
| Mobile MCP gateway | Optional HTTP/JSON-RPC gateway exposes four bounded tools: `fung.projects.list`, `fung.notes.search`, `fung.capture.status`, and default-deny `fung.voice.speak`. It binds loopback unless LAN exposure is explicitly enabled and creates a per-session Bearer token. | `src-tauri/src/mobile.rs:2735-2848,2866-2958`; `contracts/mobile-mcp-v1.yaml`. The dated [Mobile status](../../Mobile/IMPLEMENTATION_STATUS.md) records code/test-build evidence and says device-to-client interoperability remains open; this audit did not rerun it. |
| Desktop external-MCP client | FUNG can connect to configured external MCP servers and preview/execute approved meeting tools. This is an outbound client, not a server exposing FUNG tools. It is disabled by default behind `FUNG_EXTERNAL_MEETING_TOOLS=1`. | `src-tauri/src/external_mcp.rs`, `external_mcp_commands.rs:1365-1492`, `external_mcp_transport.rs:423-433`; registered Tauri commands at `src-tauri/src/lib.rs:7266-7273`; feature gate at `src-tauri/src/lib.rs:1727-1730`. |
| Desktop local HTTP API | An opt-in same-machine API with per-launch Bearer token supports health, recording list/audio/transcript reads, job-by-ID read, and file import. LAN sharing is a separate explicit toggle. It is not an MCP server. | `src-tauri/src/local_api.rs:581-716`; start/toggle commands at `src-tauri/src/lib.rs:6518-6532` and registered in the Tauri handler. |

## Contract tool inventory

All 17 rows below are **`CONTRACT_ONLY` as a Desktop product MCP tool**: the name
and intended HTTP mapping appear in `contracts/local-mcp-v1.yaml`, but no
Desktop MCP tool registration was found. “Related implementation” names an
existing UI, Tauri, Mobile, or HTTP path; it does not make the MCP tool
callable.

| Declared tool | Related implementation found | Product MCP status |
| --- | --- | --- |
| `fung.health` | Local API `GET /health` (`local_api.rs:632-644`) exists, but returns `app`, `version`, `databasePath`, `storageAuthority`, and `stableFrontier`, not the contract's `status` and `app_version`. | `CONTRACT_ONLY` |
| `fung.project.list` | Native `list_projects` reads Genesis (`lib.rs:1798-1813`); no Desktop `GET /projects`. Mobile has a separate `fung.projects.list` (`mobile.rs:2840-2844,2876-2895`). | `CONTRACT_ONLY` |
| `fung.project.create` | Native `create_project` commits project, graph-node and audit rows to Genesis (`lib.rs:1764-1794`); no Desktop `POST /projects`. | `CONTRACT_ONLY` |
| `fung.recording.start` | Desktop native capture command `live_meeting_start` (`live_meeting.rs:4179`); local HTTP only imports files | `CONTRACT_ONLY` |
| `fung.recording.control` | Native stop/status and recovery commands exist (`lib.rs:7218-7219,7252-7254`); no matching HTTP pause/resume/stop/recover route. | `CONTRACT_ONLY` |
| `fung.job.list` | Native `list_jobs` reads Genesis and returns at most 30 recent rows (`lib.rs:2202-2238`); no HTTP `GET /jobs` route. | `CONTRACT_ONLY` |
| `fung.job.get` | HTTP `GET /jobs/{id}` returns `{job}` without contract `events` (`local_api.rs:658-673`); native `job_by_id` reads Genesis (`lib.rs:2171-2198`). | `CONTRACT_ONLY` |
| `fung.job.control` | Native `cancel_job` exists (`lib.rs:2167-2169`); generic product API POST currently accepts only `/recordings/import`. The separate meeting-task-manager integration prefix has its own POST routes; neither surface registers this MCP retry/cancel tool. | `CONTRACT_ONLY` |
| `fung.transcript.get` | Native `list_transcript_segments` is project+recording scoped (`lib.rs:2731-2738`); HTTP path is recording-scoped (`local_api.rs:675-681,857-895`). | `CONTRACT_ONLY` |
| `fung.layer.list` | No route or tool found; Genesis schema documents that `audio_layers` has no table and `source_layer_id` is only a plain reference (`genesis_adapter.rs:808-813`). | `CONTRACT_ONLY` |
| `fung.transcript.patch_segment` | Native correction command `correct_transcript_segment` writes current text, refinement history and audit through Genesis (`lib.rs:4822-4859`); no HTTP PATCH/MCP tool. | `CONTRACT_ONLY` |
| `fung.transcript.rename_speaker` | Native Mobile speaker correction writes a Genesis speaker row/revision (`mobile.rs:1580-1619`); it is not a Desktop MCP tool or HTTP route. | `CONTRACT_ONLY` |
| `fung.model.list` | Native `list_model_providers` reads Genesis (`lib.rs:2266-2306`); no `GET /models` or MCP tool. | `CONTRACT_ONLY` |
| `fung.model.test` | No matching MCP/local HTTP route found | `CONTRACT_ONLY` |
| `fung.summary.list` | Desktop `meeting_summaries` Tauri command reads Genesis (`meeting_intel.rs:1564`, registered in `lib.rs:7260`); no HTTP summaries route. | `CONTRACT_ONLY` |
| `fung.intent.list` | No `intent_inferences` Genesis runtime operation or matching API route was found; the current intent capability is missing/unproven. | `CONTRACT_ONLY` |
| `fung.export.create` | Native job processing creates subtitle/audio artifacts (`job_engine.rs:1064-1067`) and lists artifacts (`transcript_export.rs:349`); no HTTP `POST /exports` or MCP tool. | `CONTRACT_ONLY` |

The machine-facing data API currently exposes `GET /health`, `GET /recordings`,
`GET /jobs/{id}`, `GET /recordings/{id}/transcript`,
`GET /recordings/{id}/audio`, and `POST /recordings/import` (plus the separate
meeting-task-manager integration prefix). The generic product API accepts no `POST /recordings`
microphone start, generic job list/control, transcript patch, model, summary,
intent, or export-create route. Routes other than the landing page and health
check require the per-launch token. LAN binding is opt-in.

## Data and authorization boundary

- The native product uses the GenesisBlockDB adapter as its persistence owner;
  the local API and Mobile MCP receive the app's Genesis storage handle or call
  the Genesis adapter. The proposed Desktop product tools must reuse these
  paths and must not open or mutate SQLite directly.
- The developer acceptance MCP uses a pinned synthetic fixture and an isolated
  verification root. Its workflow evidence proves file import, a completed
  transcription job, edit, export and orderly store reopen for that run. It
  does not prove microphone control through MCP.
- Capture, local edits, export, external disclosure, destructive operations,
  and administrative actions need distinct capability scopes. The current
  Desktop product MCP contract does not yet encode a complete capability
  model, revision preconditions, idempotency, or long-job cancellation shape.
- External MCP execution is per-call approved in its existing surface; this
  does not authorize a new product MCP tool to perform external writes.

## Page and task ownership

The current Desktop sitemap has three primary task destinations—Home, Live,
and Archive—plus utility/support destinations through More and Settings. MCP
tools should follow those user tasks and the underlying object boundaries,
not be generated as one tool for each visual panel. Initial ownership is:

| Surface | MCP task domains |
| --- | --- |
| Home | capabilities, local workspace/meeting overview, reviewed task proposals |
| Live | source discovery, capture lifecycle, active jobs and live state |
| Archive | project/recording reads, transcript and speaker review, search/citations, summaries, exports |
| Settings / More | engines, privacy, storage diagnostics, connector grants, audit and administration |

This mapping follows [the current Desktop navigation contract](../../design/2026-10-16-desktop-navigation-and-flows.md). It is an ownership map, not evidence that those MCP tools are already implemented.

## Independent review and next step

The independent read-only review confirmed all 17 contract rows and the
surface distinctions, and requested the two precision fixes now applied above.
This remains a source inventory; there was no fresh runtime test, packaged UAT,
or MCP client interoperability interaction in this audit. The workflow acceptance
report and Mobile status file are historical evidence with their own revision
and scope. MCP-00 is accepted. MCP-01 must freeze a versioned Desktop transport, local client authentication model,
stable object identities, scoped authorization, idempotency, revisions,
errors, audit fields, and long-job lifecycle before implementation starts.

See the [Product MCP roadmap](../../plans/2026-10-20-product-mcp-feature-roadmap.md)
and its [execution DAG](../../plans/2026-10-20-product-mcp-execution-dag.json).
