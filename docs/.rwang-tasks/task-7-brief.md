# Task 7 Brief — Scoped Source/Test Annotations (GREEN)

## Role

You are a fresh implementation worker for FUNG's approved H3 traceability slice.

## Task

Make the RED contract from Task 6 pass by adding structured comments to exactly the eight scoped files listed in `tests/traceabilityAnnotations.test.mjs`. Use the repository's canonical form:

```text
// @req FR-106, FR-107 — short source/test intent
// @tested tests/externalMeetingTools.test.mjs
```

For Rust use `//!` module comments where that is the existing style. Keep each file's IDs exactly equal to the `requiredAnnotations` contract. The annotations describe source/test intent only; they do not certify runtime, device, visual/keyboard, real-connector, or UAT completion.

## Constraints

- Do not change application behavior, exports, APIs, or test assertions.
- Do not update `.doc-graph.json` or Appendix D in this task; T8 will do the graph reflight.
- Do not annotate unsupported requirements just to improve percentages.
- Preserve existing comments and formatting; add only the smallest module/file-level annotation block.

## Output contract

1. Add annotations to the six implementation and two test files in the RED contract.
2. Run `npm run test:traceability` and record GREEN evidence.
3. Write `docs/.rwang-tasks/task-7-report.md` with changed paths, annotation counts, and the explicit non-UAT boundary.
4. Return GREEN or BLOCKED.
