# Task 1 — RWANG Doc Preflight Audit Report

**Status:** DONE_WITH_CONCERNS  
**Project:** FUNG  
**Audited:** 2026-08-12 01:00 ICT  
**Scope:** `docs/`, `src/`, `src-tauri/src/`, `src-tauri/tests/`, and `tests/` (build outputs excluded)  
**Trust hierarchy:** Code > SDD/design > PRD/requirements

## Evidence Boundary

The repository is dirty.  Current external-retrieval source, tests, and several
documentation artifacts are uncommitted, so this report distinguishes **current
working-tree evidence** from **committed-history evidence**.  No application code
was changed.

- Markdown documents scanned: **62** (excluding `docs/.rwang-tasks/` worker files).
- Source/test files scanned: **64**.
- Focused verification: `npm run test:external-tools` — **5 passed, 0 failed**.
- Git evidence: committed Live Meeting files changed on **2026-08-10**; the latest
  committed Desktop status, architecture, and product documents changed on
  **2026-07-21**.  The newer status update is currently uncommitted.

## 10-Point Summary

| # | Check | Status | Evidence / finding |
|---|---|---|---|
| 1 | Document existence & structure | INFO | Scoped requirements, architecture, appendices, agent guide, and doc graph exist. Root `README.md`, `docs/CONTRIBUTING.md`, `docs/api/README.md`, and `.editorconfig` are absent. |
| 2 | Document control | WARNING | The existing classification still identifies 19 historical/support Markdown files with one or more control gaps. Current candidate requirements/design use front matter and changelogs. |
| 3 | Section completeness | PASS | Scoped requirements and technical design cover product, API, data, security, error, testing, observability, accessibility, and rollout. |
| 4 | Requirement-ID coverage | WARNING | 50 unique FR/NFR/BR/AC IDs occur in docs, but no source/test file contains `@req`, `@spec`, or a requirement-ID annotation. |
| 5 | Internal contradictions | CRITICAL | Candidate requirements say external retrieval is unimplemented, while current source registers its commands and renders its operator UI. Code wins. |
| 6 | Staleness using git dates | WARNING | Committed Desktop status/architecture/product docs (2026-07-21) predate committed Live Meeting source (2026-08-10). The locally edited `08-real-progress.md` reduces but does not remove this concern because it is uncommitted; the candidate requirements remain stale. |
| 7 | Cross-references | PASS | Four relative Markdown links were resolved; no broken relative target was found. Section-reference sampling found no confirmed broken scoped reference. |
| 8 | Doc-code symlink health | WARNING | Zero structured requirement annotations; traceability therefore rests on manually maintained `.doc-graph.json` and Appendix D rather than executable links. |
| 9 | Mermaid diagrams | PASS | 13 Mermaid blocks have balanced fences and a supported diagram declaration. A Mermaid renderer was not available/run, so parser rendering is not independently certified. |
| 10 | Glossary completeness | INFO | No central glossary was found for MCP grant, connector, egress, evidence span, or sanitized result. |

**Overall:** **CRITICAL / DONE_WITH_CONCERNS** — one source-versus-requirements contradiction must be resolved before treating the documentation set as current or using it as a release/implementation gate.

## Critical Finding

### CRIT-06 — Candidate requirements contradict the current external-retrieval implementation

**Evidence**

- `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:86-94,96,148-150`
  says FR-106--FR-114 and FR-116 are not implemented or have no runtime/UI.
- `src-tauri/src/lib.rs:1992-1999` registers `external_connectors_list`,
  registration/disconnect, suggest/execute/cancel/revoke, and run-list commands.
- `src/components/LiveMeetingPanel.tsx:17,338` imports and renders
  `ExternalMeetingToolsPanel`.
- `npm run test:external-tools` passes 5/5 for the default-off flag, argument
  minimization, UI states, Thai-safe errors, command surface, and embedding.

**Resolution:** Code is the present implementation truth.  This does **not** prove
real-connector, visual/keyboard, restart, or physical-device UAT.

**Action:** Truth-sync the candidate requirements implementation/traceability
columns after reviewing the uncommitted feature as a coherent change.  Do not
silently promote any unverified runtime claim.

## Warnings

### WARN-03 — Committed status and architecture are stale against git history

`docs/Desktop/08-real-progress.md`, `docs/Desktop/ARCHITECTURE.md`, and
`docs/Desktop/PRODUCT_SPEC.md` last changed in commit `dcdf93c` on 2026-07-21.
`src-tauri/src/live_meeting.rs`, `src-tauri/src/meeting_intel.rs`, and
`src-tauri/src/lib.rs` changed in committed Live Meeting work on 2026-08-10.
The `08-real-progress.md` working-tree update is evidence of an attempted
truth-sync, but it has no commit date yet.

**Action:** Review and commit the status update; update product/architecture where
the external command and persistence boundaries are intended to be authoritative.

### WARN-04 — Requirement coverage is not executable

The docs contain 50 unique `FR-*`, `NFR-*`, `BR-*`, and `AC-*` IDs; the source/test
scan found zero matching IDs and zero `@req`/`@spec` annotations.  The graph's
coverage percentages are manual claims, not annotation-derived coverage.

**Action:** Add narrow requirement/test annotations or generate the graph from
traceable evidence before using its coverage percentage as a gate.

### WARN-05 — Document-control metadata remains inconsistent

Nineteen historical/support documents remain incomplete under the existing
preflight classification.  This does not block the scoped candidate pack, but it
weakens whole-repository governance.

**Action:** Normalize authoritative documents first and label historical snapshots
clearly; retain provenance rather than bulk rewriting history.

### WARN-06 — Candidate pack is not a runtime-completion claim

The focused operator-surface test proves code-level contract coverage only.
Existing design/status evidence still leaves real connector, automated visual and
keyboard UAT, restart provenance, and physical-device capture isolation open.

**Action:** Keep those gates explicitly open in any update to requirements or
release status.

## Information Findings

- Recommended repository documents absent: `README.md`, `docs/CONTRIBUTING.md`,
  `docs/api/README.md`, and `.editorconfig`.
- Add a compact glossary for connector, MCP grant, egress, evidence span,
  provenance, and sanitized result.
- Mermaid structure is balanced, but add a renderer check in CI before diagram
  syntax is treated as release evidence.

## Artifact Changes Made

Only the allowed stale preflight artifacts were updated:

1. `docs/DOC_PREFLIGHT_2026-08-11.md` — version `0.3.0b`; added an independent
   2026-08-12 refresh that records working-tree/committed-history boundaries and
   CRIT-06.
2. `docs/.preflight-report.json` — version `1.1.0`; added machine-readable
   current-audit findings while retaining the earlier reflight as historical data.
3. `docs/.rwang-tasks/task-1-report.md` — this worker report.

No app code or requirements/design artifacts were changed.

## Recommended Next Steps

1. Resolve CRIT-06 by reviewing and truth-syncing the candidate requirements and
   traceability table against the current external-retrieval implementation.
2. Commit reviewed documentation/status changes so git-date staleness can be
   re-evaluated.
3. Add generated or annotated requirement-to-code/test traceability.
4. Run the still-open real-connector, visual/keyboard, restart, and device UAT
   before any release claim.

## Review Gate — T1

**Independent review date:** 2026-08-12 ICT  
**Review verdict:** **PASS — audit quality; project documentation gate remains BLOCKED.**

The worker report correctly identifies an unresolved source-versus-requirements
contradiction.  This PASS validates the audit and its evidence boundary; it is
not a release, implementation-completion, or requirements-approval result.

### Hard-gate checks

| Hard gate | Verdict | Independent evidence |
|---|---|---|
| 1. Completeness | PASS | The report covers all 10 requested preflight checks, classifies critical/warning/info findings, names its evidence boundary, and records artifact changes. Its 62-document/64-source-test count is scoped to the worker scan; `DOC_PREFLIGHT_2026-08-11.md:17-18` reports the broader 63-document/67-code-contract scan, so these figures must not be compared as the same metric. |
| 2. Requirement traceability | FAIL — blocking project finding | The report accurately states that executable annotations are absent: an independent scan of `src/`, `src-tauri/`, and `tests/` found no `@req`, `@spec`, `FR-*`, `NFR-*`, `BR-*`, or `AC-*` matches. `docs/.preflight-report.json:9-16` also records `requirement_ids_in_code: 0`. The manual graph's 92% coverage claim is therefore not an executable traceability gate. |
| 3. Internal consistency | FAIL — blocking project finding | `LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:86-96,148-150` still says the external-retrieval requirements have no implementation/UI. Current code contradicts that status: `src-tauri/src/lib.rs:1992-1999` registers the external commands and `src/components/LiveMeetingPanel.tsx:17,338-341` imports/renders `ExternalMeetingToolsPanel`. `docs/.doc-graph.json:10` reports zero contradiction edges despite this conflict. |

| Review dimension | Verdict | Review result |
|---|---|---|
| Completeness | PASS | All ten required checks and the output-contract elements are represented. |
| Requirement traceability | FAIL — blocking | The report's WARN is correct, but zero executable requirement links means the project cannot use the graph percentage as a hard requirement-to-code/test gate. |
| Internal consistency | FAIL — blocking | The report's CRIT-06 is correct. Candidate requirements, graph contradiction count, and implemented working-tree state must be truth-synced before the documentation set is treated as current. |
| Standards/control | WARN | The report correctly retains the 19 control-field gaps and notes that Mermaid validation was fence-only; candidate docs do have front matter/changelog controls. |
| Code alignment | PASS | Command registration and UI embedding are present, and `npm run test:external-tools` independently passed 5/5. This is code-level evidence only; it does not verify a real connector, restart persistence, keyboard/visual behavior, or device capture isolation. |
| Writing quality | PASS | Findings are structured, specific, evidence-bounded, and distinguish current working-tree evidence from committed history. |

**Required disposition:** Keep CRIT-06 and the traceability gap open.  Review and
truth-sync the candidate requirements and graph after the uncommitted feature is
reviewed as one coherent change; then run the still-open real-connector,
visual/keyboard, restart, and device UAT before any release claim.

**Files independently checked:**
`docs/.rwang-tasks/task-1-brief.md`; `docs/.rwang-tasks/task-1-report.md`;
`docs/DOC_PREFLIGHT_2026-08-11.md`; `docs/.preflight-report.json`;
`docs/.doc-graph.json`; `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md`;
`docs/specs/2026-08-11-live-meeting-external-retrieval-design.md`;
`docs/Desktop/08-real-progress.md`; `docs/Desktop/ARCHITECTURE.md`;
`docs/Mobile/IMPLEMENTATION_STATUS.md`;
`docs/plans/2026-08-09-fung-master-implementation-plan.md`;
`src-tauri/src/lib.rs`; `src-tauri/src/external_mcp_commands.rs`;
`src/components/LiveMeetingPanel.tsx`; `src/components/ExternalMeetingToolsPanel.tsx`;
`tests/externalMeetingTools.test.mjs`; and `package.json`.
