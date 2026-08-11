---
version: "0.1.4b"
created_at: "2026-08-11T10:37:54+07:00,Agent: ATHER"
last_update: "2026-08-12T03:39:30+07:00,Agent: ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  scope: "FUNG Desktop Live Meeting and controlled external retrieval"
  doc_type: "product-requirements"
---

# Live Meeting and Controlled External Retrieval Requirements

## Executive Summary

FUNG Desktop already routes real-time microphone/system capture, live transcript, current-topic extraction, local Knowledge Base questions, and post-meeting summaries. This document defines the missing product contract for a safe meeting assistant that may suggest and, only after an explicit preview and approval, execute a read-only external MCP lookup such as finding a document or reading a customer status from CRM.

The first delivery remains local-first. Capture and durable transcript work must continue when the local LLM, connector, network, MCP server, or external provider fails.

## Scope and Boundaries

### In Scope

- One supported entry point from P1 Meeting Mode into the real Live Meeting panel.
- Microphone plus optional system-audio capture with source-channel labels.
- Best-effort live topic, open-point, and action-item state.
- Manual local Knowledge Base question answering with cited evidence.
- Evidence-linked external lookup suggestions.
- Per-call preview and approval for read-only MCP tools.
- Generic MCP connector catalogue supporting document lookup and CRM status lookup capabilities.
- Sanitized result panel with origin, time, policy, query scope, and evidence context.
- Local audit/provenance without secret or raw full-transcript leakage.
- Existing post-meeting overview, key points, action items, and Markdown export.

### Out of Scope

- Autonomous execution triggered directly from transcript keywords.
- CRM create/update/delete, email sending, calendar changes, webhooks, or any external write.
- Sending a whole transcript or raw audio to an external connector.
- Vendor-specific HubSpot, Salesforce, Notion, Google Drive, or SharePoint integrations in the first slice.
- Screen capture, calendar auto-record, coaching scores, personality labels, or biometric identity.
- Replacing Tauri/Rust, GenesisBlockDB, the local control plane, or the BYOM provider boundary.

## Stakeholders and Personas

| Persona | Need | Protection |
|---|---|---|
| Meeting operator | Obtain relevant evidence without stopping the meeting | Clear preview, fast denial, capture never blocked |
| Project owner | Control which data and tools a meeting may use | Project/meeting-scoped, expiring, revocable grant |
| Participant/data subject | Avoid undisclosed recording or data disclosure | Consent state, minimization, visible external-call state |
| Administrator | Register approved connectors and inspect failures | Allowlist, keyring credentials, diagnostics, audit |
| Auditor | Reconstruct why a result appeared | Tool, connector, input refs, policy decision, output hash, timestamps |

## Use Cases

### UC-01 — Continue a local live meeting

The operator starts P1 Meeting Mode, optionally enables system audio, sees source-aware transcript segments, and receives topic updates. Local capture remains durable if intelligence services fail.

### UC-02 — Find a document mentioned in the meeting

FUNG detects or the user enters a question such as “เอกสารตัวนี้อยู่ไหน”. FUNG shows a suggestion containing the selected evidence span, connector, tool, fields leaving the device, and a read-only approval button. After approval, the result panel shows sanitized document metadata and its origin.

### UC-03 — Read customer status from CRM

The operator asks for a customer status. FUNG previews the customer identifier and the minimum CRM fields requested. After approval, a read-only tool returns status and provenance. FUNG does not update the CRM.

### UC-04 — Deny or revoke external access

The operator denies a suggestion or revokes the meeting grant. No request leaves the machine, the denial is audited, and local capture/transcript/summary continue.

### UC-05 — Finish the meeting offline

When the network or connector is unavailable, FUNG explains the failure without hiding local evidence. Stopping the meeting still runs the local summary/export path when its local model is available.

## Functional Requirements

| ID | Requirement | Priority | Current evidence/status |
|---|---|---|---|
| FR-101 | P1 `Start recording` and the fixed microphone control shall open the same real `LiveMeetingPanel`. | Must | Both entry points route to `LiveMeetingPanel`; `npm run test:desktop-bootstrap` passes 5/5, including the microphone-rail regression. |
| FR-102 | Live Meeting shall capture microphone and optional system audio as source channels, not verified identities. | Must | `live_meeting.rs` implements the source-channel boundary and its annotation now covers source intent; two-channel/real-device UAT remains required. |
| FR-103 | The UI shall render bounded live transcript segments with timestamp, channel/speaker label, and degradation state. | Must | Implemented and capped at 200 UI segments; the Live Meeting source annotation is green, while visual/device UAT remains required. |
| FR-104 | Topic intelligence shall expose topic, open points, action candidates, model origin, and unavailable/degraded state without blocking capture. | Must | Topic path is implemented and annotated; explicit unavailable-state and local-model/real-device verification remain incomplete. |
| FR-105 | Local questions shall search stored transcript and knowledge graph, return cited sources, and state when the search ceiling may truncate results. | Must | Implemented in `meeting_ask` and annotated; cited-result integration and restart review evidence remain incomplete. |
| FR-106 | FUNG may create an external-tool suggestion from a manual question or evidence span, but shall never execute from transcript text alone. | Must | Code-level default-off suggestion/preview surface implemented with focused fixture and frontend evidence; visual/keyboard UAT remains open. |
| FR-107 | Before an external call, FUNG shall show connector, tool, capability, selected evidence/input, fields leaving the machine, expected result type, expiry, and allow/deny controls. | Must | Code-level default-off preview, canonical hash, minimization, allow/deny, and execution recheck implemented; visual/keyboard and real-connector UAT remain open. |
| FR-108 | Every credentialed read/network egress shall require explicit per-call approval in the first release. | Must | Code-level default-deny one-time approval and revoke policy implemented with focused tests; real-connector and device UAT remain open. |
| FR-109 | Connector registration shall use an allowlisted generic MCP transport and expose only approved read-only capabilities. | Must | Allowlisted local stdio registration/execution and read-only capability filtering implemented behind default-off flags; Streamable HTTP and real-connector UAT remain open. |
| FR-110 | The Live Meeting result surface shall sanitize untrusted output and display connector/tool origin, request time, completion time, policy decision, evidence refs, and failure state. | Must | Sanitized, inert result/provenance surface and bounded backend result path implemented behind default-off flags; visual/keyboard and real-connector UAT remain open. |
| FR-111 | Every suggestion, approval, denial, revocation, execution, result, timeout, and failure shall append a local audit event without credentials or full raw transcript. | Must | Code-level external tool run/result/audit lifecycle implemented with focused integration evidence; artifact-wide secret scan remains open. |
| FR-112 | A project/meeting grant shall be capability-specific, expiring, revocable, and default-deny. | Must | Code-level capability-specific expiry/revoke/default-deny grant path implemented with focused policy/integration evidence; real-connector and device UAT remain open. |
| FR-113 | Credential material shall remain in OS keyring; GenesisBlockDB stores only credential references and non-secret connector metadata. | Must | Code-level transient registration-to-keyring and non-secret reference lifecycle implemented with focused tests; artifact-wide secret scan remains open. |
| FR-114 | External connector failure shall not pause, stop, or corrupt capture, transcript persistence, local search, or post-meeting jobs. | Must | Code-level bounded child-process failure isolation and active-recording-row integration evidence exists; restart, real-device capture-isolation, and real-connector gates remain open. |
| FR-115 | Stopping a meeting shall retain/generate overview, key points, action items, evidence refs, provider provenance, and reviewed Markdown export. | Must | Summary/export path is implemented and annotated; the relaunch smoke retained base transcript rows, but this run had no summary rows, so post-meeting review after restart remains open. |
| FR-116 | Settings shall expose connector health, granted read capabilities, credential-reference state, last diagnostic, and revoke/disconnect. | Should | Code-level default-off settings lifecycle exposes list/register/disconnect/revoke, granted capabilities, and credential-reference state; detailed health diagnostics and real UAT remain open. |

## Non-Functional Requirements

| ID | Requirement | Verification target |
|---|---|---|
| NFR-101 | Local-first availability | Capture and transcript durability pass with network disabled and connector process killed. |
| NFR-102 | Data minimization | External payload contains only approved fields and selected refs; no audio/full transcript in negative tests. |
| NFR-103 | Security | Secrets never appear in Genesis rows, logs, events, serialized UI state, crash reports, or test snapshots. |
| NFR-104 | Responsiveness | Suggest/preview work never runs on the UI or capture thread; UI feedback begins within 250 ms. |
| NFR-105 | Bounded execution | Default read timeout is 15 seconds, output is capped at 256 KiB, and cancellation is supported. |
| NFR-106 | Accessibility | All approval/result states are keyboard-operable, visibly focused, text-labelled, and not color-only; Thai retains readable line height. |
| NFR-107 | Auditability | Every completed or denied call resolves to one immutable audit chain and stable output hash. |
| NFR-108 | Untrusted rendering | Markdown/links are sanitized; scripts, HTML execution, file URLs, and automatic navigation are rejected. |
| NFR-109 | Compatibility | Initial Desktop target is Windows/Tauri v2; local capture remains functional without any connector installed. |
| NFR-110 | Testability | Policy, minimization, denial, revoke, timeout, sanitization, and capture-isolation paths have automated tests plus one real-connector UAT. |

## Business Rules

| ID | Rule |
|---|---|
| BR-101 | External access is default-deny and opt-in per meeting/project. |
| BR-102 | Transcript keywords may suggest but cannot authorize or execute a tool. |
| BR-103 | First release exposes read-only tools; every write capability is rejected even if advertised by the server. |
| BR-104 | A grant cannot outlive the meeting unless the user explicitly creates a separate project grant in Settings. |
| BR-105 | Denial, cancellation, timeout, or connector failure cannot degrade durable local capture. |
| BR-106 | External results are evidence with provenance, not verified truth; the UI must state source and retrieval time. |
| BR-107 | Credentials are never stored in project export, Genesis payloads, logs, or the MCP result artifact. |
| BR-108 | Full transcript/audio export to an external service requires a future separately approved egress design. |

## Acceptance Criteria

| ID | Acceptance criterion |
|---|---|
| AC-101 | Both supported recording entry points open the same Live Meeting panel and no mock recording state is shown as real. |
| AC-102 | A 30-minute two-channel fixture produces durable chunks and live segments while a killed topic model leaves capture running. |
| AC-103 | “เอกสารตัวนี้อยู่ไหน” can return a cited local result without network access. |
| AC-104 | A document MCP lookup shows a preview and sends zero bytes before approval. |
| AC-105 | A CRM status lookup sends only the approved customer key/fields and renders a sanitized, sourced result. |
| AC-106 | Deny/revoke/expired grant paths issue no external call and append an audit event. |
| AC-107 | Any advertised write tool is filtered and cannot be approved or executed. |
| AC-108 | Killing or timing out the MCP server leaves capture, transcript, stop, summary retry, and export controls operational. |
| AC-109 | Secret-leak scans find no connector secret in GenesisBlockDB, logs, UI snapshots, exports, or error strings. |
| AC-110 | Result HTML/script/file URLs are neutralized and cannot execute or navigate automatically. |
| AC-111 | Post-meeting overview, key points, actions, evidence refs, model provenance, and Markdown artifact remain reviewable after restart. |
| AC-112 | Keyboard-only UAT can preview, approve, deny, revoke, inspect sources, and dismiss results at 1200 × 780. |

## Requirement Traceability Baseline

| Requirement group | Design owner | Existing implementation | Planned implementation/tests |
|---|---|---|---|
| FR-101–FR-105 | `07-meeting-mode.md`, current Live Meeting code | `App.tsx`, `LiveMeetingPanel.tsx`, `live_meeting.rs`, `meeting_intel.rs`; FR-102–FR-105 carry implementation-file annotations, while FR-101 is anchored by the desktop-bootstrap test annotation. | Entry repair, degradation UI, and focused runtime tests exist; two-channel, local-model, visual, device, and cited-result UAT remain open. |
| FR-106–FR-114 | External retrieval security design | Current working tree: default-off eight-command Rust boundary, allowlisted stdio adapter, policy/grant/keyring/audit/sanitizer path, Genesis entities, and embedded `ExternalMeetingToolsPanel`; the annotation contract covers all 26 FR/NFR IDs across implementation and test intent. | Focused fixture/unit/integration/frontend evidence exists; artifact-secret-scan, visual/keyboard, restart, real-device capture-isolation, and real-connector gates remain open. |
| FR-115 | Architecture + current meeting intelligence | `meeting_intel.rs` plus `LiveMeetingPanel.tsx` | Base rows survive relaunch; summary/export review after restart remains open because the UAT dataset has no persisted summary row. |
| FR-116 | External retrieval security design | Current working tree: default-off operator list/register/disconnect/revoke lifecycle with non-secret connector summaries and credential-reference state. | Detailed health diagnostics, visual/keyboard, real-connector, and device UAT remain open. |

## Version Diff

| Version | Change |
|---|---|
| 0.1.3b | Expanded executable source/test-intent annotations to all 26 FR/NFR IDs, recorded the 195/195 Rust regression, and added bounded restart/UAT evidence without closing blocked visual, device, or real-connector gates. |
| 0.1.2b | Corrected FR-101 to record that both recording entry points route to `LiveMeetingPanel`, backed by the passing desktop-bootstrap microphone-rail regression; all other UAT and open-gate boundaries are unchanged. |
| 0.1.1b | Truth-synced FR-106--FR-114 and FR-116 plus their mapping rows to the current default-off code-level operator/backend evidence, while retaining every real-connector, visual/keyboard, restart, artifact-secret-scan, device, and full-regression gate. |
| 0.1.0b | Established traceable product, security, accessibility, and verification requirements for controlled read-only meeting retrieval. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.4b | 2026-08-12 | candidate | Clarified that FR-101 is test-anchored while the combined implementation/test annotation contract covers all 26 IDs. | pending | ATHER |
| 0.1.3b | 2026-08-12 | candidate | Expanded all-26 annotation intent, recorded full Rust regression and bounded restart evidence; retained blocked visual/device/real-connector gates. | pending | ATHER |
| 0.1.2b | 2026-08-12 | candidate | Corrected FR-101 entry-point routing evidence; retained all UAT and release gates. | pending | ATHER |
| 0.1.1b | 2026-08-12 | candidate | Truth-synced current implementation evidence without closing UAT or release gates. | pending | ATHER |
| 0.1.0b | 2026-08-11 | candidate | Added FR/NFR/BR/AC baseline for Live Meeting and controlled external retrieval. | pending | ATHER |
