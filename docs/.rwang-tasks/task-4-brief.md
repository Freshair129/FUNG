# Task 4 Brief — Hard-Finding Documentation Reconciliation

## Role

You are a fresh documentation fix worker for FUNG. Apply only the hard findings from `task-3-report.md` that can be fixed with evidence already present in the workspace.

## Required fixes

1. **H1** — Truth-sync `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` implementation/status and mapping columns with the current eight-command/operator-UI evidence. Preserve the distinction between code-level implementation and open real-connector, visual/keyboard, restart, artifact-secret-scan, device, and full-regression gates.
2. **H2** — Recompute `docs/.doc-graph.json` coverage metadata from its current edges. It must not claim more than the derived 22/26 code mappings and 16/26 test mappings. Preserve the one open contradiction edge and all manual edges.
3. **H4** — Clarify the external-retrieval design security wording so registration may receive one transient credential only for keyring storage, no execution command accepts a raw credential, and credential resolution/use during stdio execution is explicitly open or out of scope unless current code proves it.
4. **H5** — Update `docs/implementation-plan.md` to point at the current `DOC_PREFLIGHT_2026-08-11.md` v0.3.0b critical gate, not the historical zero-critical audit.

## H3 boundary

Do not invent requirement annotations. If adding a narrowly scoped `@req`/`@tested` annotation is fully supported by current requirement IDs and tests, record it precisely; otherwise leave H3 open and state why in the report. Do not bulk-annotate unrelated code.

## Inputs

- `D:\FUNG\docs\.rwang-tasks\task-3-report.md`
- `D:\FUNG\docs\Desktop\LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md`
- `D:\FUNG\docs\.doc-graph.json`
- `D:\FUNG\docs\appendices\D-traceability.md`
- `D:\FUNG\docs\superpowers\specs\2026-08-11-live-meeting-external-retrieval-design.md`
- `D:\FUNG\docs\implementation-plan.md`
- Current Sprint 4 source/tests named in `task-3-brief.md`

## Constraints

- No application behavior changes.
- No soft-warning cleanup in this task.
- No claims of real-connector/UAT/full-regression completion.
- Keep document metadata/version diff/changelog truthful and preserve existing user work.

## Output contract

1. Write `D:\FUNG\docs\.rwang-tasks\task-4-report.md` with each H1-H5 disposition, exact changed paths, and any intentionally open gate.
2. Update only the documents listed above (plus the report) when evidence requires it.
3. Return DONE, DONE_WITH_CONCERNS, or BLOCKED.
