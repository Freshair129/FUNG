# Task 5 Brief — Post-Fix Preflight and Graph Reflight

## Role

You are a fresh post-fix audit worker for FUNG.

## Task

Re-run the user-requested RWANG documentation gates after T4: first the full 10-point `doc-preflight`, then the incremental `doc-graph` validation. Use current working-tree source/docs, the T4 report and review, and git dates. The graph must be checked after preflight artifacts are refreshed.

## Inputs

- `D:\FUNG\docs\.rwang-tasks\task-4-report.md`
- `D:\FUNG\docs\Desktop\LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md`
- `D:\FUNG\docs\.preflight-report.json`
- `D:\FUNG\docs\DOC_PREFLIGHT_2026-08-11.md`
- `D:\FUNG\docs\.doc-graph.json`
- `D:\FUNG\docs\appendices\D-traceability.md`
- Current source/tests under `src/`, `src-tauri/`, and `tests/`

## Constraints

- No application code changes.
- Preserve explicit H3 zero-annotation status unless exact evidence changes it.
- Do not close real-connector, visual/keyboard, restart, artifact-secret-scan, device, packaging, or full-regression gates.
- Do not hide historical contradiction evidence; remove/resolve a contradiction edge only when current documents and source agree.

## Output contract

Write `D:\FUNG\docs\.rwang-tasks\task-5-report.md` with:

1. Preflight 10-point status and changed artifact paths.
2. Graph node/edge/hash/stale/contradiction counts and coverage derived from current edges.
3. Explicit remaining warnings/hard gates.
4. Commands and results used.

Update the current human/machine preflight and graph/traceability artifacts only when current evidence requires it, preserving metadata/version/changelog. Return DONE, DONE_WITH_CONCERNS, or BLOCKED.
