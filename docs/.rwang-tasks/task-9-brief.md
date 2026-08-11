# Task 9 Brief — Preflight Staleness-Wording Fix

## Role

You are a fresh documentation fix worker for FUNG.

## Task

Correct only the current staleness finding in `docs/.preflight-report.json` and the current post-fix table in `docs/DOC_PREFLIGHT_2026-08-11.md`. The graph reflight now proves all 28 hash-bearing mapped nodes match; the remaining warning is only that untracked scoped documents cannot be compared to committed git dates. Replace the stale wording that says three hashes still require refresh with this accurate provenance limitation. Preserve all historical audit sections unchanged.

## Constraints

- Do not change source, tests, graph, Appendix D, requirements, or implementation plan.
- Keep the warning (do not downgrade to PASS) because git-date provenance is unresolved for untracked scoped artifacts.
- Bump document versions/last_update/version diff/changelog truthfully.
- Append `## Fix Cycle 1` to `docs/.rwang-tasks/task-8-report.md` with the exact changed paths and no runtime/UAT claim.

## Output

Write `docs/.rwang-tasks/task-9-report.md` with the before/after wording, command validation, and remaining gates. Return DONE_WITH_CONCERNS or BLOCKED.
