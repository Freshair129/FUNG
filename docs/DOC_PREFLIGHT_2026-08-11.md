---
version: "0.3.5b"
created_at: "2026-08-11T10:37:54+07:00,Agent: ATHER"
last_update: "2026-08-12T03:39:30+07:00,Agent: ATHER"
status: "draft"
superseded_by: null
attributes:
  domain: "documentation-governance"
  scope: "Live Meeting, Knowledge Search, External MCP, CRM, Post-meeting Intelligence"
  doc_type: "preflight-report"
---

# RWANG Doc Pre-flight Report — FUNG Meeting Intelligence

**Project:** FUNG  
**Date:** 2026-08-11  
**Documents scanned:** 63 Markdown files  
**Code/contract files scanned:** 67 TypeScript, Rust, JavaScript, Python, and YAML files  
**Trust hierarchy:** Code > SDD > PRD

## Scope

This preflight evaluates the workflow discussed for FUNG Desktop:

1. Real-time meeting capture and speaker-aware transcript.
2. Current-topic understanding.
3. Immediate knowledge retrieval during a meeting.
4. Controlled external MCP tools, including document and CRM lookup.
5. Evidence-backed post-meeting summary and action items.

It does not certify release readiness for unrelated Mobile, cloud-storage, authentication, or distribution work.

## Runtime/UAT Reflight — 2026-08-12 03:16 ICT

This current 10-point reflight was run after the expanded all-26 annotation
contract, full Rust regression repair, and bounded desktop relaunch smoke. It
scanned the 62-document controlled scope (excluding task reports), current
source/test annotations, local Markdown links, Mermaid fences, current graph
hashes, focused regressions, and environment-bounded UAT evidence. The trust
hierarchy remains **Code > SDD > PRD**.

| Check | Current status | Evidence |
|---|---|---|
| Document existence & structure | INFO | All scoped required artifacts exist. Recommended root `README.md`, `docs/CONTRIBUTING.md`, `docs/api/README.md`, and `.editorconfig` remain absent. |
| Document control | WARNING | 15 of 62 controlled Markdown documents still lack one or more version, date, status, or history controls. Current scoped governance artifacts are controlled. |
| Section completeness | PASS | The scoped requirements/design cover product scope, API/transport, data, security, errors, testing, observability, accessibility, and rollout boundaries. |
| Requirement coverage | WARNING | The approved contract maps **57 `@req` occurrences across nine implementation files (25/26 unique IDs) plus 11 across two test files; together they cover 26/26 scoped FR/NFR IDs**. Retained manual, code-inspection, and test-run graph edges remain separately counted; annotations establish intent only. |
| Internal contradictions | PASS | The requirements, current source, and focused tests agree; the prior source-versus-requirements edge is retained as resolved historical evidence. |
| Staleness | WARNING | All 28 of 28 mapped graph-node SHA-256 prefixes match current content and `stats.stale_edges` is 0. The remaining warning is provenance only: git cannot compare commit dates for untracked scoped documents, which remain current-working-tree evidence. |
| Cross-references | PASS | All local relative Markdown links resolved. |
| Doc-code symlinks | WARNING | `npm run test:traceability` passes 1/1 for the exact eleven-file contract. All 26 IDs are annotated for source/test intent; this does not establish runtime, device, visual/keyboard, real-connector, or UAT completion. |
| Diagrams | INFO | 13 Mermaid-containing controlled documents have balanced fences; renderer parsing is not installed/run. |
| Glossary | INFO | No central controlled-retrieval glossary exists. |

**Current overall: WARNING — 0 CRITICAL, 4 WARNING, 3 INFO, 3 PASS.** H3 is
GREEN for all 26 source/test-intent IDs. Full Rust regression is now 195/195,
and the desktop process relaunch smoke retained base Genesis rows. Summary/
export review after restart, real-connector, visual/keyboard, artifact-secret-
scan, real-device capture-isolation, and packaging gates remain open.

Focused commands run in this reflight:

- `npm run test:traceability` — 1 passed, 0 failed.
- `npm run test:desktop-bootstrap` — 5 passed, 0 failed.
- `npm run test:external-tools` — 5 passed, 0 failed.
- `cargo test --lib` — 195 passed, 0 failed.
- Relaunch smoke — `fung.exe` title/handle observed before and after close;
  Genesis counts remained `projects=1`, `recordings=1`,
  `transcript_segments=13`, `audit_events=1`.

## Post-Fix Preflight and Graph Reflight — 2026-08-12 01:34 ICT

This is the current 10-point result after Task 4 reconciliation. It evaluates
the current working tree; current source is the controlling evidence where it
differs from older historical audit text. The retained initial/independent
findings below are history, not current CRITICAL findings.

| Check | Current status | Evidence |
|---|---|---|
| Document existence & structure | INFO | All scoped required artifacts exist. Recommended root `README.md`, `docs/CONTRIBUTING.md`, `docs/api/README.md`, and `.editorconfig` remain absent. |
| Document control | WARNING | 15 of 62 Markdown documents lack one or more version, date, status, or history controls; the current scoped requirements, design, preflight, graph, and traceability artifacts have control metadata. |
| Section completeness | PASS | The scoped requirements/design cover product scope, API/transport, data, security, error handling, test strategy, observability, accessibility, and rollout boundaries. |
| Requirement coverage | WARNING | 46 FR/NFR/BR/AC IDs are defined in current docs, but source/tests have zero textual requirement-ID or structured annotations. The graph retains manual links for 22/26 FR/NFR requirements to code and 16/26 to tests. |
| Internal contradictions | PASS | T4's H1 and FR-101 corrections align the requirements with the routed source and both focused regressions; H4's credential wording matches the DTO/execution boundary; the former graph contradiction is preserved as resolved historical evidence. |
| Staleness | WARNING | Incremental SHA-256 validation found three stale graph document hashes (requirements, external-retrieval design, implementation plan); this reflight refreshes them. Git cannot date-compare those untracked scoped artifacts, so they are treated as current-working-tree evidence. |
| Cross-references | PASS | All 7 relative local Markdown links resolved; no broken target was found. |
| Doc-code symlinks | WARNING | No `@req`, `@spec`, `@designs`, or `@tested` annotations exist under `src/`, `src-tauri/`, or `tests/`. H3 remains an explicit open manual-traceability gate. |
| Diagrams | INFO | 13 Mermaid-containing documents have balanced fences; renderer parsing is not installed/run. |
| Glossary | INFO | No central glossary defines the controlled-retrieval terms. |

**Current overall: WARNING — 0 CRITICAL, 4 WARNING, 3 INFO, 3 PASS.** Focused
regressions pass, but this is not release evidence. Real-connector,
visual/keyboard, restart/provenance, artifact-secret-scan, real-device
capture-isolation, packaging, and full-regression gates remain open.

## Independent Audit Refresh — 2026-08-12

This refresh scans the current working tree as well as committed history.  The working tree contains uncommitted external-retrieval implementation files, so their state is **implemented in the current tree, not commit-verified**.  Git dates show the committed Live Meeting sources changed on 2026-08-10, after the last committed Desktop status/architecture documents (2026-07-21).

| Check | Current status | Evidence |
|---|---|---|
| Document existence | INFO | Required scoped requirements, architecture, appendix, agent guide, and doc graph exist; root `README.md`, `docs/CONTRIBUTING.md`, `docs/api/README.md`, and `.editorconfig` are absent. |
| Document control | WARNING | The 19 historical/support-document control gaps recorded below remain; candidate requirements and design have front matter and changelogs. |
| Section completeness | PASS | The scoped requirements/design cover product, API, data, security, error, test, observability, accessibility, and rollout concerns. |
| Requirement coverage | WARNING | 50 unique FR/NFR/BR/AC IDs exist in docs; source/test files contain zero textual `@req`, `@spec`, or requirement-ID annotations. |
| Contradictions | CRITICAL | `LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:86-94,96,148-150` says FR-106--FR-114/116 are unimplemented, but the current tree registers eight commands and embeds `ExternalMeetingToolsPanel`. Code wins; the candidate requirements must be truth-synced. |
| Staleness | WARNING | Committed Desktop architecture/product/status docs last changed 2026-07-21; Live Meeting code changed 2026-08-10. `08-real-progress.md` is locally updated but uncommitted; the requirements document remains inconsistent with current source. |
| Cross-references | PASS | Four relative Markdown links were resolved; no broken target was found. Section-reference sampling found no confirmed broken scoped reference. |
| Doc-code symlinks | WARNING | No structured annotations are present, so requirement-to-code/test traceability depends on the manually maintained graph. |
| Diagrams | PASS | Thirteen Mermaid blocks have balanced fences and expected diagram declarations; a renderer was not run. |
| Glossary | INFO | No central glossary was found for MCP grant, connector, egress, evidence span, or sanitized result. |

**Refresh overall: CRITICAL — one current source-versus-requirements contradiction.** The external-tool frontend test passes 5/5, but that does not substitute for the missing documentation correction, real-connector UAT, or the remaining visual/restart/device gates.

## Reflight After Documentation Remediation — 2026-08-11 10:51 ICT

| Check | Current status | Evidence |
|---|---|---|
| Document existence | PASS | Requirements, security design, traceability appendix, and `docs/.doc-graph.json` now exist. |
| Document control | WARNING | 19 historical/support Markdown files still lack one or more control fields; new authoritative artifacts are complete. |
| Section completeness | PASS | Scoped requirements and design cover product, architecture, API, data, security, errors, tests, observability, accessibility, and rollout. |
| Requirement coverage | WARNING | 84 unique requirement IDs exist; External MCP requirements are correctly planned and code annotations remain absent. |
| Contradictions/staleness | PASS | Real-progress is truth-synced; the old scorecard is marked superseded with a current-state correction. |
| Cross-references | PASS | No broken relative Markdown target found. |
| Doc graph | PASS | Scoped graph contains 43 nodes and 76 edges, including plan-to-requirement links; traceability appendix generated. |
| Diagrams | INFO | 13 documents contain balanced Mermaid fences; parser rendering remains a future CI improvement. |

**Reflight overall: WARNING — 0 CRITICAL. Implementation planning may proceed; code remains gated on candidate-document approval.**

## Initial Summary Dashboard — Before Remediation

| Check | Status | Findings |
|---|---|---|
| Document Existence & Structure | CRITICAL | `docs/.doc-graph.json` and a scoped approved MCP/CRM security design are missing. |
| Document Control | WARNING | 19/63 Markdown files lack one or more of version, date, status, or history metadata. |
| Section Completeness | CRITICAL | External MCP/CRM consent, connector lifecycle, audit, redaction, and error contracts are not specified. |
| Requirement Coverage | CRITICAL | No stable FR/NFR IDs cover this workflow; code contains zero requirement annotations. |
| Internal Contradictions | CRITICAL | Progress and Call.md scorecard claims conflict with current Live Meeting code. |
| Staleness | CRITICAL | Two ground-truth/status documents still describe capture, intelligence, summary, and export as absent. |
| Cross-References | PASS | Four relative Markdown links were checked; no missing target was found. |
| Doc-Code Symlinks | WARNING | No `@req`, `@spec`, `FR-*`, `NFR-*`, or scoped `REQ-*` references exist in code for this workflow. |
| Diagrams | INFO | Ten documents contain Mermaid blocks and all fences are balanced; renderer syntax was not independently executed. |
| Glossary | INFO | No central glossary defines MCP grant, egress, evidence span, connector, tool result, or live speaker channel. |

**Initial overall:** **CRITICAL.** The findings below are retained as audit history; CRIT-01 through CRIT-05 were remediated by the documents listed in the reflight section.

## Critical Findings

### CRIT-01 — Dependency graph required by implementation planning is absent

**Evidence**

- `docs/.doc-graph.json` does not exist.
- The active master plan declares its own program phases but does not contain a dependency node for controlled Live Meeting MCP/CRM retrieval.

**Impact**

The RWANG implementation planner cannot topologically sort this feature against local API, GenesisBlockDB, model-provider, secret-storage, egress, UI, and audit dependencies.

**Action**

Create the document graph after the scoped requirements and security design are approved. Add nodes for product requirements, architecture, local MCP contract, Meeting Mode UI, connector policy, implementation plan, and verification artifacts.

### CRIT-02 — No traceable requirements exist for the requested workflow

**Evidence**

- Documentation contains 38 unique `REQ-*`/`AC-*` IDs, but they belong to existing program phases or the GPU runtime.
- No stable requirement IDs define live topic tracking, local knowledge retrieval, external MCP suggestion/preview/approval, CRM/document lookup, or meeting result rendering.
- Code contains zero matching requirement annotations.

**Impact**

The implementation planner cannot satisfy its rule that every task and sprint map back to approved requirements. Acceptance and negative tests would otherwise be invented during implementation.

**Action**

Approve a scoped requirement set before planning. At minimum it must define capture-channel semantics, topic cadence/degradation, local retrieval evidence, external-tool grants, read/write confirmation policy, redaction, audit, result rendering, post-meeting evidence, accessibility, latency, and offline behavior.

### CRIT-03 — Status documents contradict current code

**Evidence**

- `docs/Desktop/08-real-progress.md:48-58` lists long microphone recording, real summary, export, and the full MCP runtime as not implemented.
- `docs/Desktop/CALLMD_FUNG_SCORECARD_TH.md:35,106-111,139-141` says Desktop cannot record, has no summary/export, and has no live intelligence.
- Current code implements the Live Meeting panel in `src/components/LiveMeetingPanel.tsx`, capture/transcription in `src-tauri/src/live_meeting.rs`, and topic, local-KB Q&A, summary, action extraction, and Markdown export in `src-tauri/src/meeting_intel.rs`.

**Resolution rule**

Code wins for routed/implemented behavior. It does not prove current end-to-end runtime verification or external integration readiness.

**Action**

Update both status documents to separate `implemented`, `routed`, `runtime-verified`, and `not implemented`. Preserve their previous claims in changelog/history rather than silently deleting them.

### CRIT-04 — External MCP/CRM security and consent design is not approved

**Evidence**

- `docs/Desktop/CALLMD_ARCHITECTURE_COMPARISON.md:162-183` selects suggest → preview → approved execution and requires scoped grants, minimization, confirmation, audit, untrusted-output rendering, and resilience.
- Its acceptance item for external MCP/webhook security remains unchecked, and OQ-02/OQ-05 remain open.
- `contracts/local-mcp-v1.yaml` defines local FUNG tools only. It does not define external connector registration, credential ownership, per-meeting grants, CRM/document scopes, egress preview, redaction, revocation, or result provenance.
- No Desktop external MCP runtime or Live Meeting tool-result panel exists in code.

**Impact**

Implementing CRM/document lookup now would require making product, privacy, credential, and side-effect decisions without approval.

**Action**

Create and approve a dedicated external MCP/connector security design before implementation planning. Default behavior must remain suggest-only; credentialed reads, network egress, and all writes require an explicit policy decision and visible approval state.

## Warnings

### WARN-01 — Local MCP contract is design-only and incomplete for meeting retrieval

`contracts/local-mcp-v1.yaml` has version and update date but no author, lifecycle status, changelog, error model, authentication rotation, audit event, pagination, capability grant, or external connector definitions. Align the contract with the single local control plane before adding external tools.

### WARN-02 — Architecture lacks feature-level operational sections

`docs/Desktop/ARCHITECTURE.md` covers the stack, components, data, security defaults, and phases, but the requested workflow lacks a concrete API contract, connector state machine, timeout/cancellation behavior, observability, testing strategy, and deployment/secret-rotation procedure.

### WARN-03 — Live Meeting entry points disagree

The P1 `Start recording` action opens `LiveMeetingPanel`, but the fixed sidebar microphone button in `src/App.tsx` still toggles local React state without opening the real panel. Documentation does not identify the supported entry point or mark the sidebar action as incomplete.

## Information Findings

### INFO-01 — Document control is inconsistent

Nineteen of 63 Markdown files omit at least one control field. Root `PRODUCT.md`, `DESIGN.md`, and the main Desktop architecture have usable control blocks, while the master implementation plan intentionally uses a different structure. Standardize control metadata for authoritative documents first; do not spend scope on historical or generated artifacts until authority is clear.

### INFO-02 — Diagram and glossary verification are shallow

Mermaid fences are balanced but were not rendered through a Mermaid parser in this audit. Add a small glossary and CI renderer check when the new connector diagrams are introduced.

## Verified Current Capability Boundary

| Capability | UI | Backend | Current boundary |
|---|---|---|---|
| Microphone + system audio capture | Present | Present | Two source channels (`เรา`/`อีกฝ่าย`), not arbitrary live multi-speaker diarization. |
| Live transcript | Present | Present | Requires the local transcription runtime; degradation keeps capture running. |
| Current topic/open points/actions | Present | Present | Best-effort local LLM tick; unavailable model yields no topic card. |
| Local knowledge question | Present | Present | Manual question; searches stored transcript and knowledge graph with cited sources. |
| External document/CRM lookup | Absent | Absent | Requires approved connector/MCP security design. |
| MCP result/approval surface | Design only | Absent | No Desktop external MCP execution path. |
| Post-meeting summary/actions/export | Present | Present | Requires configured local LLM; current-session runtime verification was not rerun by this preflight. |

## Implementation-plan Gate

The `rwang-plugin:implementation-plan` documentation prerequisite is now **met with warnings**:

- [x] PRD/SDD-like documents exist.
- [x] Doc preflight completed.
- [x] No CRITICAL findings remain after remediation.
- [x] `docs/.doc-graph.json` exists.
- [x] Scoped requirements have stable IDs and acceptance criteria.
- [x] External MCP/CRM security and consent design exists as a candidate.

Therefore a candidate sprint/team/timeline plan may be generated. Before code starts, the user must approve the requirements, security design, traceability matrix, and implementation plan together.

1. Generate `docs/implementation-plan.md` from the remediated graph.
2. Review the three remaining design choices: initial MCP transport, external-link confirmation, and real UAT server.
3. Obtain explicit approval of the full candidate document pack.
4. Start code only through the approved implementation-plan gates.

## Version Diff

- `0.3.2b -> 0.3.3b`: corrected the current staleness wording after the graph
  reflight confirmed all 28/28 mapped hashes are current. The warning now
  records only unavailable git-date comparison for untracked scoped documents;
  historical audit sections are unchanged.
- `0.3.1b -> 0.3.2b`: refreshed the 10-point preflight after the approved H3
  slice. The exact eight-file contract is GREEN with 47 annotation occurrences
  over 17/26 scoped FR/NFR IDs; all runtime and UAT gates remain open.
- `0.3.0b -> 0.3.1b`: post-fix 10-point reflight resolves the current
  requirements-versus-source contradiction, refreshes stale graph hashes, and
  retains the former contradiction as resolved history. H3 and all external/UAT
  gates remain open.
- `0.2.0b -> 0.3.0b`: independent 2026-08-12 refresh; records the current-tree external-retrieval implementation, absence of source annotations, committed-date staleness, and the unresolved candidate-requirements contradiction.
- `0.1.0b -> 0.2.0b`: recorded the post-remediation reflight with zero CRITICAL findings and opened the candidate planning gate.
- `new -> 0.1.0b`: added the first code-versus-doc preflight for Live Meeting intelligence and controlled external retrieval.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.3.5b | 2026-08-12 | draft | Corrected implementation-versus-test annotation coverage wording; the combined source/test contract remains 26/26. | pending | ATHER |
| 0.3.3b | 2026-08-12 | draft | Corrected current staleness wording: 28/28 mapped hashes are current; untracked scoped documents lack git-date provenance. | pending | ATHER |
| 0.3.2b | 2026-08-12 | draft | Scoped annotation reflight: exact H3 contract GREEN; runtime/UAT gates remain open. | pending | ATHER |
| 0.3.1b | 2026-08-12 | draft | Post-fix 10-point preflight; no current CRITICAL findings, H3 and all release/UAT gates retained. | pending | ATHER |
| 0.3.0b | 2026-08-12 | draft | Independent refresh found current source-versus-requirements drift; no release/planning closure. | pending | ATHER |
| 0.2.0b | 2026-08-11 | draft | Recorded requirements/design/graph/status remediation and planning readiness with warnings. | pending | ATHER |
| 0.1.0b | 2026-08-11 | draft | Recorded preflight findings, verified capability boundary, and implementation-plan gate. | pending | ATHER |
