# Task 3 Brief — Cross-Document Consistency Audit

## Role

You are an independent final documentation consistency auditor for FUNG.

## Task

Read the current design, requirements, implementation plan, progress ledger, preflight report, graph, traceability matrix, and the Sprint 4 source/test contracts. Audit terminology, requirement coverage, data-flow/API agreement, diagram/component agreement, version/date alignment, glossary signals, and cross-reference validity. Apply Code > SDD/design > PRD/requirements when comparing claims.

## Inputs

- `D:\FUNG\docs\.rwang-progress.md`
- All files under `D:\FUNG\docs\`
- `D:\FUNG\src\components\ExternalMeetingToolsPanel.tsx`
- `D:\FUNG\src\lib\externalMeetingTools.ts`
- `D:\FUNG\src\tauri.ts`
- `D:\FUNG\src-tauri\src\external_mcp.rs`
- `D:\FUNG\src-tauri\src\external_mcp_transport.rs`
- `D:\FUNG\src-tauri\src\external_mcp_commands.rs`
- `D:\FUNG\tests\externalMeetingTools.test.mjs`

## Constraints

- Do not change application code.
- Do not rewrite docs merely for style.
- Report hard-gate failures separately from soft warnings; include exact evidence and recommended owner/path.

## Output contract

Write `D:\FUNG\docs\.rwang-tasks\task-3-report.md` with PASS/WARN/FAIL for each audit item, unresolved contradictions, and a final recommendation. Return status DONE, DONE_WITH_CONCERNS, or BLOCKED.
