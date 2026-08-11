# Task 2 Brief — Document Graph and Traceability Audit

## Role

You are an independent doc-graph/traceability auditor for FUNG.

## Task

Validate and incrementally update `docs/.doc-graph.json` using the `rwang-plugin:doc-graph` process. Scan all docs and relevant source/test files, refresh content hashes, preserve manual edges, check up to 3-hop drift propagation, and verify requirement-to-code/test traceability. Generate/update `docs/appendices/D-traceability.md` only when current evidence requires it.

## Inputs

- `D:\FUNG\docs\.doc-graph.json`
- `D:\FUNG\docs\appendices\D-traceability.md`
- `D:\FUNG\docs\.rwang-tasks\task-1-report.md`
- Current docs under `docs/`
- Current source/tests under `src/`, `src-tauri/`, and `tests/`

## Constraints

- Do not change application code.
- Preserve manually-authored graph edges.
- Do not invent requirement IDs or coverage; distinguish implemented, tested, and externally unverified.
- Keep graph changes incremental and auditable.

## Output contract

1. Write a structured worker report to `D:\FUNG\docs\.rwang-tasks\task-2-report.md` with node/edge/hash counts, stale/contradiction findings, coverage, and changed paths.
2. Update the graph/traceability artifacts only when evidence requires it.
3. Return the report path and a concise status: DONE, DONE_WITH_CONCERNS, or BLOCKED.
