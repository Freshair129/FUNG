---
version: "0.1.0b"
created_at: "2026-09-17T05:30:37+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T05:30:37+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "UI_HISTORY output location correction; preserve all source bytes"
  risk: "MEDIUM"
---

# History worker worktree binding

## Symptom / evidence

Godel01a0ac3b-5f11-7081-ad50-89e125547b8a reported ten dependency files,
RecordingReview.tsx/.css and a passing build in the assigned history worktree.
Controller repeatedly observed that worktree clean, with no source artifacts.
At05:28 controller located both sources under controller root src/components/desktop:
TSX72fb91665b87350ef407226fb14d678f08423031fe52ec375937615fd71f5448;
CSS19c92aed68dc112feb142ccf346278684c1ac5f9d95a5e13ad6c85d279caee73.

Worker paused and confirmed Get-Location/git top-level and build cwd were
C:/Users/pc/.codex/worktrees/9000/fung, not output/callmd-worktrees/history-376ef30.
Its build is not evidence for the assigned isolated candidate. No application
mount, native launch or userdata action occurred. Existing root App is unchanged.

## Root cause

The worker did not bind writes/build commands to the explicit assigned worktree.
Tool default cwd remained controller root. An exec working-directory argument
does not change apply_patch's default context; absolute patch paths are required.
The worker read-back confirms location misbinding, not a permission blocker.

## Why it escaped detection

The checkpoint inferred destination from its assignment rather than resolving
the actual artifacts and build cwd. Content hashes alone do not prove location.
Controller's in-flight inventory exposed the mismatch before integration.

## Bounded correction and prevention

The same still-live Luna session may perform a one-time corrective transfer.
First materialize all ten already accepted dependency bytes at the assigned
absolute history worktree; freeze them. Transfer the two source bytes there,
verify both raw hashes BEFORE removing any misplaced copy. The retained history
copies are recoverable source, not accepted implementation.

Then restore ONLY these known worker-introduced controller-root paths:

- src/tauri.ts: only if it still matches accepted shared hash816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe,
  restore pre-task HEAD376ef30 content via apply_patch; no git checkout/reset.
- Remove only the matching newly introduced copies of
  src/components/desktop/contracts.ts,
  src/components/desktop/RecordingReview.tsx and RecordingReview.css,
  tests/callmdDesktopContracts.test.mjs,
  docs/verification/implementation-reports/2026-09-17-callmd-contract-tests.md,
  docs/verification/implementation-reports/2026-09-17-callmd-shared-contract.md.

All accepted dependency hashes are in the UI dispatch inventory. If ANY hash
differs from the worker's known copy, stop rather than overwrite concurrent work.
Do not touch the five pre-existing accepted root baseline files, other root docs,
App.tsx, user changes, other worktrees, or broad directories. No recursive delete.
Every edit uses an absolute apply_patch path; every test supplies explicit cwd.
Rerun build and task checks in the assigned worktree; record the misplaced build
as historical wrong-location evidence, not candidate acceptance.

This is recovery within the approved implementation task, not a broader feature
or general cleanup lease. Controller does not edit or transfer source code.

## Controller resolution check — 2026-09-17 05:43 ICT

Assigned History TSX/CSS hashes exactly match the preserved original values above.
Root git status for all seven corrective targets is empty, and git diff HEAD
for src/src-tauri is empty. Root's five accepted baseline artifacts still match
5/5 hashes; History's ten dependency artifacts match10/10. Only misplaced
copies were removed after retaining verified copies in the assigned worktree.
The one-time root corrective lease is now released. Implementation/tests continue
only in History's original assigned worktree; candidate build/test evidence must
be rerun there. No source implementation edit or transfer by controller.

## Version Diff / CHANGELOG record

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Confirm wrong cwd and bound recoverable Luna correction | UNCOMMITTED;base376ef30 | Codex orchestrator |
