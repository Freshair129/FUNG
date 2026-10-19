# Task 8 Brief — Annotation-Derived Graph and Preflight Reflight

## Role

You are a fresh traceability audit/preflight worker for FUNG after the approved H3 GREEN slice.

## Task

This archived task rechecked preflight and traceability after the H3 annotation work. At the time, it refreshed a generated relationship snapshot and Appendix D from structured source/test annotations. The generated snapshot is retired; current traceability sources are Appendix D and the annotation contract.

## Evidence rules

- The annotations establish source/test intent and canonical IDs; they do not close runtime, device, visual/keyboard, real-connector, or UAT gates.
- Do not count annotations for requirements not present in the contract test.
- Keep manual coverage and annotation coverage distinguishable in stats/traceability.
- Recompute every changed node hash and `last_verified`; no stale hash may remain.

## Inputs

- `D:\FUNG\docs\.rwang-tasks\task-6-report.md`
- `D:\FUNG\docs\.rwang-tasks\task-7-report.md`
- `D:\FUNG\tests\traceabilityAnnotations.test.mjs`
- Historical relationship metrics recorded in this task's report
- `D:\FUNG\docs\.preflight-report.json`
- `D:\FUNG\docs\DOC_PREFLIGHT_2026-08-11.md`
- `D:\FUNG\docs\appendices\D-traceability.md`
- Current annotated source/test files

## Constraints

- No application behavior changes.
- Do not remove manual edges or rewrite historical audit findings.
- Keep graph stats derived from actual edge sets and distinguish `annotation` from `manual`, `code-inspection`, and `test-run` sources.
- Preserve the explicit full-regression and runtime/UAT boundaries.

## Output contract

1. Update the current preflight, graph, and Appendix D artifacts as evidence requires.
2. Write `D:\FUNG\docs\.rwang-tasks\task-8-report.md` with before/after node/edge/hash counts, annotation-derived coverage, commands/results, and remaining gates.
3. Return DONE_WITH_CONCERNS or BLOCKED; do not claim release completion.
