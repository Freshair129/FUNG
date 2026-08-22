# Task 1 Brief — Documentation Preflight Audit

## Role

You are an independent documentation-health auditor for FUNG.

## Task

Run the complete 10-point `rwang-plugin:doc-preflight` audit against the current repository. Scan both `docs/` and source/test code. Use the trust hierarchy Code > SDD/design > PRD/requirements. Check document existence/control/sections, requirement coverage, contradictions, staleness using git dates, cross-references, annotations, Mermaid diagrams, and glossary signals.

## Inputs

- `D:\FUNG\docs\.doc-graph.json`
- `D:\FUNG\docs\.preflight-report.json`
- `D:\FUNG\docs\DOC_PREFLIGHT_2026-08-11.md`
- `D:\FUNG\docs\Desktop\08-real-progress.md`
- `D:\FUNG\docs\implementation-plan.md`
- `D:\FUNG\docs\specs\2026-08-11-live-meeting-external-retrieval-design.md`
- Current source and tests under `src/`, `src-tauri/`, and `tests/`

## Constraints

- Do not change application code.
- Do not auto-fix contradictions; name the evidence and recommendation.
- Keep findings specific with file paths/line anchors where practical.
- Bilingual report: Thai narrative is fine; IDs and technical terms stay English.

## Output contract

1. Write a structured worker report to `D:\FUNG\docs\.rwang-tasks\task-1-report.md` using the 10-point summary plus critical/warning/info findings.
2. If the existing machine-readable or human preflight artifacts are stale or factually wrong, update only those artifacts with evidence and record every changed path in the report. Otherwise leave them untouched.
3. Return the report path and a concise status: DONE, DONE_WITH_CONCERNS, or BLOCKED.
