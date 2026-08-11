# Task 8 Brief — Annotation-Derived Graph and Preflight Reflight

## Role

You are a fresh doc-graph/preflight worker for FUNG after the approved H3 GREEN slice.

## Task

Run the current preflight scan, then incrementally update `docs/.doc-graph.json` and `docs/appendices/D-traceability.md` from the structured annotations in the six implementation and two test files. Preserve all manual edges and the resolved historical contradiction. Add a node for `src/tauri.ts` and `tests/traceabilityAnnotations.test.mjs` only if the graph needs them to represent the new evidence. Store annotation metadata on existing nodes and add `implements`/`verifies` edges with `source: "annotation"` only where the comments and focused test support them.

## Evidence rules

- The annotations establish source/test intent and canonical IDs; they do not close runtime, device, visual/keyboard, real-connector, or UAT gates.
- Do not count annotations for requirements not present in the contract test.
- Keep manual coverage and annotation coverage distinguishable in stats/traceability.
- Recompute every changed node hash and `last_verified`; no stale hash may remain.

## Inputs

- `D:\FUNG\docs\.rwang-tasks\task-6-report.md`
- `D:\FUNG\docs\.rwang-tasks\task-7-report.md`
- `D:\FUNG\tests\traceabilityAnnotations.test.mjs`
- `D:\FUNG\docs\.doc-graph.json`
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
