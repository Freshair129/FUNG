---
version: "0.1.0b"
created_at: "2026-08-12T01:14:20+07:00,Agent: ATHER,ec4ca97"
last_update: "2026-08-12T01:14:20+07:00,Agent: ATHER"
status: "need review"
superseded_by: null
attributes:
  domain: "documentation-governance"
  scope: "FUNG cross-document consistency audit"
  doc_type: "audit-report"
---

# Task 3 — Cross-Document Consistency Audit

**Status:** DONE_WITH_CONCERNS  
**Project:** FUNG  
**Audited:** 2026-08-12 01:14 ICT  
**Change risk:** LOW — report-only; no application code or governed product document changed  
**Trust hierarchy:** Code > SDD/design > PRD/requirements

## Evidence Boundary

This audit read all 67 Markdown and 2 JSON artifacts under `docs/`, the T1/T2
reports, and the Sprint 4 frontend/Rust source and test contracts listed in the
task brief. The repository has 33 dirty/untracked paths; the external-retrieval
implementation, tests, graph, requirements, design, plan, and audit artifacts
are current-working-tree evidence, not commit-verified release evidence.

Focused verification rerun during T3:

- `npm run test:external-tools`: **5 passed, 0 failed**.
- Direct graph-edge recount: **22/26** requirements have a code edge and
  **16/26** have a test edge.
- Markdown links: **4 checked, 0 broken**. Backticked path references were
  audited separately because they are not Markdown links.

## Audit Dashboard

| Audit item | Status | Gate | Result |
|---|---|---|---|
| Terminology consistency | WARN | Soft | Feature terminology is mostly stable, but “Sprint 4” is overloaded between the master FUNGWIRE schedule and the external-retrieval plan; no central glossary disambiguates connector, MCP grant, egress, evidence span, or sanitized result. |
| Requirement coverage | FAIL | Hard | Requirement IDs are complete in the scoped PRD/traceability set, but the PRD implementation column contradicts code and the graph coverage header overstates its own edges. Traceability remains manual and non-executable. |
| Data flow / API agreement | FAIL | Hard | Eight command names, DTO casing, persistence entities, protocol, and flags agree. The security design nevertheless says no command accepts a raw credential while the registration command explicitly accepts one; its target flow also shows credential resolution that is not wired into execution. |
| Diagram / component agreement | WARN | Soft | The feature design diagram maps to the React → Tauri → policy/transport → Genesis implementation. The parent Desktop architecture diagram predates and omits the external MCP client, keyring, approval/egress, and result-sanitization boundary. |
| Version / date / lifecycle alignment | FAIL | Hard | The feature implementation plan points to an obsolete zero-critical preflight; the master plan remains “draft — pending Boss approval” while recording completed phases and cites an older Desktop progress version. |
| Glossary signals | WARN | Soft | Acronyms and security nouns are used consistently enough for specialists, but the repository has no authoritative glossary and “MCP” denotes both inbound local-server automation and an outbound external client. |
| Cross-reference validity | WARN | Soft | All actual Markdown links resolve. Several authoritative backticked paths do not resolve, including six Desktop parent/peer references and the named canonical GenesisBlockDB boundary spec. |

**Overall documentation gate:** **FAIL** — the audit completed, but the current
documentation pack must not be treated as internally current or as a release
gate until the hard findings below are resolved.

## Hard-Gate Findings

### FAIL-H1 — Requirements implementation status contradicts current code

**Evidence**

- `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:86-94,96`
  marks FR-106–FR-114 and FR-116 absent or without runtime/UI; its mapping at
  `:148-150` likewise names no implementation.
- `src-tauri/src/lib.rs:1992-1999` registers all eight approved commands.
- `src/components/LiveMeetingPanel.tsx:17,338-341` imports and renders
  `ExternalMeetingToolsPanel`.
- `docs/Desktop/08-real-progress.md:42-43,59` and
  `docs/appendices/D-traceability.md:30-40` describe the current default-off
  backend/operator implementation and retain its UAT limits.
- `docs/.doc-graph.json:246` correctly records this as an open contradiction.

**Disposition:** Hard documentation-consistency failure. Code wins for the
implemented command/UI surface. Code does **not** prove real-connector,
visual/keyboard, restart, artifact-secret-scan, or physical-device UAT.

**Recommended owner/path:** Product/requirements owner updates
`docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` implementation and
mapping columns, preserving every open S5/UAT gate.

### FAIL-H2 — Machine-readable graph coverage overstates its own edges

**Evidence**

- `docs/.doc-graph.json:12-13` claims code coverage **24/26** and test coverage
  **19/26**.
- A direct recount of graph edges yields code **22/26** and tests **16/26**.
  Missing code edges: NFR-101, NFR-104, NFR-109, NFR-110. Missing test edges:
  FR-102, FR-103, FR-104, FR-105, FR-115, FR-116, NFR-101, NFR-106, NFR-109,
  NFR-110.
- `docs/.rwang-tasks/task-2-report.md:34-40,72` reports the correct 22/26 and
  16/26 counts and states that all mappings are manual.

**Disposition:** Hard traceability failure because the machine-readable summary
can mislead a release gate even though the underlying edges and T2 report retain
the correct evidence boundary.

**Recommended owner/path:** Documentation-graph owner recomputes
`docs/.doc-graph.json` `stats.coverage` from edge types and adds a validation that
fails when stored totals differ from derived totals.

### FAIL-H3 — Requirement-to-code/test traceability is non-executable

**Evidence**

- `docs/.preflight-report.json:9-16` records 50 unique documentation IDs and
  zero requirement IDs in code.
- T1 and T2 independently found no `@req`, `@spec`, `@designs`, `@tested`, or
  requirement-ID annotations in scoped source/tests
  (`task-1-report.md:30,79,149`; `task-2-report.md:40,72`).
- The focused frontend test passes 5/5, but
  `tests/externalMeetingTools.test.mjs:81-113` is source/contract coverage and
  contains no requirement linkage.

**Disposition:** Hard traceability failure. Manual graph edges are useful review
evidence but cannot be used as an executable requirement or release gate.

**Recommended owner/path:** QA/docs owner adds narrowly scoped generated or
annotated requirement links in the touched source/tests, then regenerates the
graph and Appendix D. Do not bulk-annotate unrelated code.

### FAIL-H4 — Security design credential contract is internally contradictory

**Evidence**

- `docs/superpowers/specs/2026-08-11-live-meeting-external-retrieval-design.md:169`
  says “No command accepts a raw credential” and immediately says registration
  accepts a transient secret.
- The actual API accepts that transient value:
  `src/lib/externalMeetingTools.ts:19-25` and
  `src-tauri/src/external_mcp_commands.rs:123-131,350-356`.
- The target sequence at design `:87` resolves a credential by reference, but
  current execution builds the stdio config at
  `external_mcp_commands.rs:1343-1359,1431-1447` without calling the production
  `resolve_connector_credential` function. Current docs correctly leave
  real-connector UAT open (`08-real-progress.md:17,59`).

**Disposition:** Hard security-document/API failure, not evidence of secret
persistence. Code wins: registration accepts a transient raw credential across
the Tauri command boundary and stores only its keyring reference; current stdio
execution does not demonstrate credential consumption.

**Recommended owner/path:** Security/design owner clarifies the command-boundary
sentence in the external-retrieval design and explicitly labels credential
resolution/use as either an unimplemented real-connector task or a non-goal for
the stdio connector. Add a credentialed fixture only if that behavior is in
scope.

### FAIL-H5 — The implementation plan cites a superseded preflight gate

**Evidence**

- `docs/implementation-plan.md:25` says preflight v0.2.0b is WARN with
  0 CRITICAL.
- Current `docs/DOC_PREFLIGHT_2026-08-11.md:2-5,43,50` is v0.3.0b and CRITICAL
  because FAIL-H1 remains open.
- The older zero-critical statement remains only as audit history at preflight
  `:65`; it is not the current gate.

**Disposition:** Hard governance/version failure because the plan header presents
an obsolete approval premise as current.

**Recommended owner/path:** Implementation-plan owner updates the preflight field
to v0.3.0b/CRITICAL and blocks further completion/flag promotion until the open
contradiction is dispositioned.

## Soft Warnings

### WARN-S1 — Parent architecture is stale relative to the feature design

`docs/Desktop/ARCHITECTURE.md:2-5,43-65` is version 1.0.0b, last updated
2026-07-20. It describes the local UI/API/MCP/Genesis/job topology but not the
outbound external MCP client, OS keyring, approval/egress policy, or sanitizer.
The feature-specific component and sequence diagrams at
`live-meeting-external-retrieval-design.md:34-65,70-95` do agree with the current
React/Tauri/Rust/Genesis structure. Update the parent architecture only with the
new boundary, not the Sprint-level UI detail.

### WARN-S2 — “Sprint 4” is overloaded

The master plan assigns master Sprint S4 to the FUNGWIRE tunnel
(`2026-08-09-fung-master-implementation-plan.md:261,267,331`), while the scoped
implementation plan uses Sprint 4 for external-retrieval UI
(`docs/implementation-plan.md:225-239`). Desktop status uses the unqualified term
at `08-real-progress.md:17,91,97`. Qualify it as “External Retrieval Sprint 4”
or use a feature-specific ID so roadmap and feature evidence cannot be confused.

### WARN-S3 — Master/mobile status control is stale

- The master plan header remains `draft — pending Boss approval`
  (`master-implementation-plan.md:9-14`) while its tracker records Phases 0–2
  complete (`:452-454`). Its source table still cites Desktop progress v0.1.3b;
  the current file is v0.2.4b (`08-real-progress.md:2,132`).
- Mobile status still says authenticated Desktop delegation is not proven
  (`docs/Mobile/IMPLEMENTATION_STATUS.md:148,158`), while the master tracker
  records Boss-confirmed pairing/FUNGWIRE acceptance (`master plan:453-454`).

These are snapshot/lifecycle warnings rather than proof that the code is wrong.
The program owner should either refresh the status documents or label their
sections as dated historical snapshots.

### WARN-S4 — No authoritative glossary and inbound/outbound MCP ambiguity

No central glossary defines connector, MCP grant, egress, evidence span,
sanitized result, provenance, or the distinction between the local inbound MCP
server and the external outbound MCP client. The absence is already recorded at
`DOC_PREFLIGHT_2026-08-11.md:48,170-172`. Add a compact glossary to the scoped
requirements/design pack; do not rewrite historical docs for style.

### WARN-S5 — Backticked cross-references include unresolved paths

Actual Markdown links pass, but code-style references are not all valid:

- `docs/Desktop/IMPLEMENTATION_SURFACES.md:23-26,30-31` points to six `docs/*.md`
  files that actually live under `docs/Desktop/`.
- `docs/Mobile/FULL_FEATURE_WIRING_SPEC.md:26` names
  `docs/SPEC--GENESISDB-UNIFIED-OPERATIONAL-BOUNDARY-V1.md`, which is absent.
- `docs/Mobile/TECHNICAL_DESIGN.md:512` names a planned
  `PHASE_0_EVIDENCE.md`, which is absent; retain it as an open deliverable or
  replace it with the actual evidence artifact.

Owners should correct authoritative references surgically and preserve any
intentional historical mention of deleted files.

## Confirmed Agreements / Pass Evidence

1. **Command surface — PASS.** The eight design commands at
   `live-meeting-external-retrieval-design.md:160-167` match frontend invokes at
   `src/tauri.ts:439-499`, Rust command definitions at
   `external_mcp_commands.rs:1363-1489`, and registration at
   `src-tauri/src/lib.rs:1992-1999`.
2. **Transport/protocol/flags — PASS.** Design stdio-only MCP `2025-11-25` and
   default-off flags (`design:104,171,287`) agree with
   `external_mcp_transport.rs:1,13-16` and the frontend/backend flag gates.
3. **Persistence entities — PASS.** The four Genesis entities plus
   `external_connections`/`audit_events` at design `:143-154` match the table
   names used by `external_mcp.rs`, `external_mcp_commands.rs`, and the Genesis
   adapter. No feature code opens a separate SQLite handle.
4. **Frontend DTO/command casing — PASS.** TypeScript camelCase inputs at
   `src/tauri.ts:458-499` match Rust `serde(rename_all = "camelCase")` DTOs at
   `external_mcp_commands.rs:73-145`.
5. **Feature component diagram — PASS.** Suggestion, catalogue, preview,
   policy, minimizer, transport, sanitizer, audit, and result-panel nodes in the
   design are represented by the reviewed React and Rust modules. Real HTTP,
   detailed health, restart, visual/keyboard, device, and real-connector gates
   remain correctly open.
6. **Markdown links — PASS.** Four relative Markdown targets resolve; no broken
   Markdown link was found in the scanned documents.

## Recommended Final Action

**Return `DONE_WITH_CONCERNS`.** T3 is complete, but the parent final gate should
remain blocked for documentation approval/release use. Resolve in this order:

1. Truth-sync the candidate requirements without closing unverified UAT.
2. Correct graph coverage metadata and establish executable traceability.
3. Clarify the credential command/runtime contract in the security design.
4. Update the implementation-plan preflight pointer and parent architecture.
5. Normalize roadmap/status lifecycle labels, glossary terms, and unresolved
   authoritative backticked paths.

After those changes, rerun T1/T2/T3 checks and the focused tests. Release remains
separately gated by the existing real-connector, visual/keyboard, restart,
artifact-secret-scan, physical-device, packaging, and full-regression evidence.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Initial independent T3 cross-document consistency audit; recorded five hard failures, five soft warnings, six confirmed agreement areas, and a final DONE_WITH_CONCERNS disposition. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-12 | need review | Initial T3 cross-document consistency audit | pending | ATHER |
