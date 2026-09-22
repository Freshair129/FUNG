---
version: "0.1.1b"
created_at: "2026-09-22T00:00:00+07:00,RWANG / Luna max,head-0f08ab6bff8acc96b64663c35d94f11914031af8"
last_update: "2026-09-22T08:59:43.918+07:00,RWANG / Luna max"
status: "candidate"
superseded_by: null
attributes:
  doc_type: "rca"
  domain: "desktop-ui"
  scope: "PR #61 baseline desktop tile-action wiring"
  risk: "MEDIUM"
  workflow_level: "C-2"
  agent: "RWANG / Luna max"
  parent_role: "orchestrator/reviewer only"
  base_sha: "b336f33ec400a38f003a0665c121069a87a543ac"
  pr_sha: "0f08ab6bff8acc96b64663c35d94f11914031af8"
  pr_url: "https://github.com/Freshair129/FUNG/pull/61"
  ci_run: "35676414302"
  evidence_status: "baseline failure reproduced; bounded implementation and focused local validation passed; CI/merge pending"
  write_scope: "src/App.tsx and this RCA only"
---

# RCA — PR #61 desktop tile actions

## Symptom

CI run `35676414302` reports the frontend failure at
`tests/jobActions.test.mjs:115`. The focused local reproduction is:

```text
17 tests
16 pass
1 fail
the desktop shell disables tile buttons instead of filing dead rows
```

The failing contract cannot find the primary and secondary tile-button
`disabled` bindings or an `action-notice` area in `src/App.tsx`.

## Evidence

- `git diff --no-ext-diff --quiet b336f33 -- src/App.tsx tests/jobActions.test.mjs`
  succeeds, so both the application file and the test are unchanged from the
  base commit. The failure is baseline relative to `b336f33` and PR #61
  exposed it by reaching this suite.
- `src/App.tsx` already owns `currentPage`/`currentTile`,
  `primaryActionLabel`, `tileActionEnabled`, `tileActionTitle`,
  `performTileAction`, and `actionNotice`.
- `DesktopShell` accepts `mainContent` and mounts it under the desktop surface;
  the current `mainContent` contains only `LiveMeetingPanel` and
  `RecordingReview`. No tile-action controls are reachable from that surface.
- `performTileAction` routes anchor, API, capture, and job actions, while
  `handleCreateJob` refuses unavailable or missing-context jobs through
  `setActionNotice`. The missing piece is rendering and binding those existing
  controls, not changing the job vocabulary or navigation guard.

## Root Cause

The desktop shell migration left the tile model and its action helpers in
`App.tsx` without attaching them to the `DesktopShell` content tree. The
computed state therefore has no DOM controls: action availability is not
reflected in `disabled`, refusal reasons have no visible notice region, and
`performTileAction` is unreachable from the tile UI. The test correctly catches
this as a real wiring defect rather than a workflow problem.

## Why it escaped detection

The unused state/helpers still type-check and do not prevent the Vite build,
and the existing local evidence focused on backend/job contracts and shell
bootstrap behavior. The base branch was already red for this source contract;
PR #61 did not modify the affected application or test files, so the defect
was only surfaced when the PR reached the job-actions suite in CI.

## Proposed prevention

- Keep the source contract that requires both tile-button disabled bindings and
  a visible refusal notice.
- Treat computed UI state as incomplete until it is mounted through the actual
  `DesktopShell` `mainContent` path and covered by the focused suite.
- Include the focused job-actions test and `npm run build` in the implementation
  handoff whenever the desktop shell or tile model changes.

## Bounded fix and verification

Render the current tile summary and two buttons in `src/App.tsx`, bind both
buttons to `performTileAction`, use `tileActionEnabled`/`tileActionTitle` for
availability and explanation, and render `actionNotice` in the same surface.
The existing capture/navigation guards remain owned by `DesktopShell`; disabled
controls cannot invoke their handlers. No tests, workflows, package files,
Rust, or unrelated dirty paths are in scope.

Required checks after the edit:

1. `npm run test:job-actions`
2. `npm run build`
3. `git diff --check` scoped to the allowed files
4. Verify the scoped diff contains only `src/App.tsx` and this RCA

No commit, push, merge, deployment, or production claim is part of this RCA.

## Result

- `npm run test:job-actions`: **17/17 passed**.
- `npm run build`: **passed** (`tsc` and Vite production build).
- `git diff --check -- src/App.tsx`: **passed**; the new RCA has no trailing
  whitespace.
- The worker-scoped status contains exactly `src/App.tsx` and this RCA. Existing
  unrelated dirty/deleted/untracked paths were preserved.
- The fix mounts the existing tile state in `DesktopShell` content, disables
  both action buttons from the existing helper, shows the existing refusal
  notice or the current blocked reason, and rechecks availability in the
  handler before dispatching.

## Version diff

- `0.1.0b` → `0.1.1b`: recorded the bounded implementation and focused local
  validation result; CI, merge, deployment, and production evidence remain
  open.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-22 | candidate | Implemented the bounded desktop tile-action wiring and recorded focused local validation; CI, merge, deployment, and production evidence remain open. | working tree; base b336f33 | RWANG / Luna max |
| 0.1.0b | 2026-09-22 | candidate | Recorded the baseline desktop tile-action wiring defect exposed by PR #61 and the bounded C-2 fix/verification scope. | head 0f08ab6; base b336f33 | RWANG / Luna max |
