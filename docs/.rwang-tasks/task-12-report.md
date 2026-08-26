---
version: "0.1.0b"
created_at: "2026-08-26T00:00:00+07:00,Agent: Luna"
last_update: "2026-08-26T00:00:00+07:00,Agent: Luna"
status: "done_with_concerns"
superseded_by: null
attributes:
  domain: "rwang-task-report"
  doc_type: "writer-report"
  scope: "Task 12 FUNG MVP critical-path amendment"
  language: "Thai"
---

# Task 12 — Writer Report

## Status

**DONE_WITH_CONCERNS.** สร้างเอกสาร candidate amendment แบบ documentation-only สำเร็จ โดยใช้ repository evidence ที่อ่านแล้วเท่านั้น ไม่แก้ code, tests, config, env, Git state, ledger หรือ external systems และไม่รัน test/search เพิ่มหลังคำสั่งหยุดสำรวจ

## Output files

- `docs/specs/2026-08-26-fung-mvp-critical-path-amendment.md`
- `docs/.rwang-tasks/task-12-report.md`

## IDs

- Primary slice: `D-MVP-01` — Close the local import handoff
- Later dependency-ordered slices: `D-MVP-02`, `D-MVP-03`, `D-MVP-04`, `D-MVP-05`
- Task: `T12`

## What the amendment records

- Local Desktop critical path: local audio input/capture → transcription → transcript review → Minute of Note → local export
- Status separation: implemented, documented local/static evidence, runtime/UAT-open, externally gated, and deferred
- Smallest proposed implementation slice: repair imported recording activation/chunk truth and connect recording-scoped review to existing summary/export paths
- Goal, Acceptance Criteria, Success Criteria, Exit Criteria, PIC, approver, risk, dependencies, exact proposed write scope, tests, blocker assessment, and out-of-scope boundaries

## Cross-references

- Parent roadmap: `docs/plans/2026-08-09-fung-master-implementation-plan.md`
- Architecture: `docs/Desktop/ARCHITECTURE.md`
- Desktop truth: `docs/Desktop/08-real-progress.md`
- Mobile truth: `docs/Mobile/IMPLEMENTATION_STATUS.md`
- Controller ledger: `docs/.rwang-progress.md` — not edited
- Prior UAT report: `docs/.rwang-tasks/task-11-report.md`
- Relevant source/contracts: `src-tauri/src/live_meeting.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/meeting_intel.rs`, `src-tauri/src/transcript_export.rs`, `src/App.tsx`, `src/components/LiveMeetingPanel.tsx`, `src/tauri.ts`
- Relevant tests/contracts: `tests/captureOrchestration.test.mjs`, `tests/transcribeConcatOnly.test.py`, `tests/summaryScoping.test.mjs`, `tests/jobActions.test.mjs`, `tests/desktopBootstrap.test.mjs`, `tests/recoveryFlow.test.mjs`, plus embedded Rust tests in the named source modules

## Concerns

- Current source creates imported recording/chunk/transcript state, but the inspected `run_import_pipeline` does not visibly update `projects.active_recording_id`; the shell uses that pointer for queued summary/export targets. This is recorded as a proposed implementation target, not as a runtime reproduction.
- The same source initializes an imported chunk with `end_ms: 0`; the success path visibly updates recording duration but does not visibly finalize the chunk duration/transcribed marker. This requires a focused regression test before implementation is approved.
- Transcript review is currently display-oriented with confidence/timestamps and speaker rename; full editing/evidence marking is still out of scope for `D-MVP-01`.
- Existing documented local tests/static checks do not prove Live Meeting real capture, packaged click-through, local provider availability, physical-device behavior, or post-restart summary/export. No such proof is claimed.

## Missing context

- Terra review result and Boss approval for the exact `D-MVP-01` write scope are not yet available.
- No current runtime click-through evidence was collected in this interrupted turn.
- The exact project convention for the product label “Minute of Note” versus existing `summary.generate`/`whole_story` naming is not documented in the inspected inputs; the amendment maps the product term to the existing summary contract without renaming identifiers.
- GenesisBlockDB cursor/offset support is not present in the inspected FUNG boundary; long-session acceptance remains dependent on upstream pagination work.

## Verification boundary

This report is a writer handoff, not implementation or release approval. The candidate is ready for Terra review; no code may be written until the candidate and exact implementation IDs are approved.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Initial Task 12 Writer Report for the documentation-only MVP critical-path amendment. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-26 | done_with_concerns | Candidate amendment and evidence-bounded writer handoff created; implementation and runtime claims remain gated. | not created | Luna |

## Terra Review Gate — Task 13

**Reviewed candidate:** `D:\FUNG\docs\specs\2026-08-26-fung-mvp-critical-path-amendment.md`
**SHA-256:** `A80051FBB12052EA69D22062BD08B4372A91C22CE2ED6AD18B5126C568BAC789` — matches the required exact bytes.
**Review basis:** Task 12 brief, RWANG ledger, Task 11 report, named parent/current-truth documents, and current source/tests. Trust order applied: code > SDD/design > PRD. No candidate, source, test, configuration, environment, Git, ledger, or external state was changed.

### Findings

1. **P1 — D-MVP-01 and D-MVP-02 overlap on the same review capability.** Candidate `:52` and AC `:59` require `D-MVP-01` to make transcript review recording-scoped for the active recording. The dependency table at `:105` then assigns “Recording-scoped transcript review” to `D-MVP-02` again. This conflicts with the stated smallest focused slice at `:48` and leaves no unambiguous implementation boundary. Current code confirms that the gap is real: `src-tauri/src/lib.rs:1583-1711` exposes/project-reads transcript segments by `project_id`, and `src/tauri.ts:390-392` has no recording parameter. Code is the source of truth; the candidate must assign that bridge change to exactly one D-MVP slice.

2. **P2 — “deterministic” summary retry is stronger than the implemented contract.** Candidate test scope `:98` says “Summary retry remains deterministic.” The current guarantee is idempotent row identity: `src-tauri/src/meeting_intel.rs:645-655` derives IDs and `:728-766` upserts them. The same function invokes the local LLM three times at `:694-726`, so generated content is not established as deterministic. The test/AC should require idempotent replacement and recording-scoped provenance, not byte-identical model output.

3. **P2 — Candidate metadata does not fully meet the stated AGENTS frontmatter convention.** Candidate `:3` provides the agent in `created_at` but omits the required commit-hash component described by AGENTS.md; `:4` likewise only contains the agent. Version, status, attributes, Version Diff, and CHANGELOG are present at `:2-11` and `:140-150`.

### Rubric

| Check | Result | Evidence |
|---|---|---|
| 1. Completeness | **PASS** | `D-MVP-01` contains Goal (`:46`), AC (`:56-63`), SC (`:65`), Exit (`:67-72`), PIC/approver/risk/dependencies (`:74-80`), exact scope (`:82-90`), tests (`:92-99`), and out-of-scope boundaries (`:122-127`). Candidate placeholder scan found no TODO/TBD/FIXME/TBA. |
| 2. Requirement Traceability | **PASS** | `D-MVP-01` through `D-MVP-05` are unique in the repository search and are cross-referenced consistently by the Task 12 report/ledger. Candidate links the governing plan, Desktop/Mobile truth, Task 11 UAT evidence, and relevant source surfaces at `:129-138`. |
| 3. Internal Consistency | **FAIL** | P1 duplicates recording-scoped transcript review between `D-MVP-01` (`:52`, `:59`) and `D-MVP-02` (`:105`), violating the focused-slice and later-slice boundary. |
| 4. Standards Compliance | **WARN** | Required frontmatter, candidate status, Version Diff, and CHANGELOG are present; created/updated metadata omit the AGENTS-required commit-hash component (P2). C-2 documentation content is otherwise reviewable; no C-3 architecture change is proposed. |
| 5. Code Alignment | **WARN** | The active-recording/chunk gap is supported by current code: import creates the recording/chunk with `end_ms: 0` (`src-tauri/src/lib.rs:2061-2074`) and the success mutation only writes recording/segments (`:2094-2115`), while the shell queues summary/export from `activeRecordingId` (`src/App.tsx:1072-1117`). Runtime/UAT remains correctly open. P2 narrows the unsupported deterministic-retry wording. |
| 6. Writing Quality | **PASS** | Thai-facing prose is concise, preserves technical IDs, separates implemented/local/static/runtime-external/deferred boundaries, and does not promote static evidence to runtime proof. |

### Verdict

**FAIL** — hard gate 3 (Internal Consistency) is not satisfied. No implementation is approved.

### Required fix scope

Amend the candidate only before re-review:

- Assign recording-scoped transcript retrieval/UI bridge to exactly one slice. The smallest correction is to retain it in `D-MVP-01` and revise `D-MVP-02` to minimal correction/audit affordance only, with no duplicated recording-scope work.
- Replace “deterministic” summary-retry language with idempotent replacement/provenance language matching the current derived-ID/upsert contract.
- Bring `created_at`/`last_update` metadata into the AGENTS.md convention, including the applicable commit-hash state, and apply the corresponding candidate patch version bump.

Do not change source, tests, configuration, environment, Git state, ledger, or external systems as part of this documentation fix.

## Terra Re-review Gate — Task 13 Fix Cycle 1

**Reviewed candidate:** `D:\FUNG\docs\specs\2026-08-26-fung-mvp-critical-path-amendment.md`
**SHA-256:** `3F8C7121BD1399696360670A86B5E4E7CC167482E4E0D0D8F0A011DA8B0A14A9` — matches the required exact bytes.
**Review basis:** Task 12 brief and original Terra findings, candidate, AGENTS.md, and only the current source/tests needed to validate closure. Trust order applied: code > SDD/design > PRD. No candidate, ledger, code, tests, config, env, Git state, or external system was changed.

### Finding closure matrix

| Original finding | Closure | Independent evidence |
|---|---|---|
| P1 — recording-scoped transcript retrieval/UI bridge was assigned to both D-MVP-01 and D-MVP-02 | **CLOSED** | Candidate `:52` assigns the bridge to `D-MVP-01`; `D-MVP-02` at `:105` explicitly says the bridge is delivered by D-MVP-01 and excluded from D-MVP-02. Current `list_transcript_segments` still accepts only `project_id` and returns the project view (`src-tauri/src/lib.rs:1584-1715`; `src/tauri.ts:390-392`), so D-MVP-01 remains the single, necessary implementation slice. |
| P2 — summary retry incorrectly required deterministic LLM output | **CLOSED** | Candidate `:98` requires idempotent replacement with recording-scoped provenance and explicitly does not require byte-identical LLM output. Current code derives summary identity from `(project, recording, kind)` and upserts model-run/summary rows (`src-tauri/src/meeting_intel.rs:638-655`, `:745-766`); retry coverage preserves that identity (`:1287-1299`). |
| P2 — frontmatter omitted the commit-hash component and candidate patch version | **CLOSED** | Candidate frontmatter is `0.1.1b` and both timestamps contain `Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67` (`:2-4`), which matches current `HEAD`. Version Diff and CHANGELOG record the patch at `:140-152`, consistent with AGENTS.md. |

### Regression check

**PASS.** The fix does not reintroduce duplicate slice ownership, does not claim deterministic LLM content, and does not expand implementation, migration, runtime/UAT, release, or external-system scope. The candidate remains documentation-only and continues to keep runtime/provider/restart evidence explicitly open.

### Rubric

| Check | Result | Evidence |
|---|---|---|
| 1. Completeness | **PASS** | `D-MVP-01` retains Goal, acceptance/success/exit criteria, PIC, approver, risk, dependencies, exact write scope, tests, and bounded out-of-scope sections (`:44-99`, `:122-127`). |
| 2. Requirement Traceability | **PASS** | Unique `D-MVP-01` through `D-MVP-05` form a dependency-ordered slice set (`:103-108`) and link the Task 12 flow, parent roadmap, Desktop/Mobile truth, Task 11 evidence, source, and test seams (`:129-138`). |
| 3. Internal Consistency | **PASS** | Recording-scoped retrieval/UI ownership is unambiguous: D-MVP-01 owns it; D-MVP-02 expressly excludes it (`:52`, `:105`). |
| 4. Standards Compliance | **PASS** | YAML frontmatter supplies version, dated creator/updater identity with commit, candidate status, attributes, and `superseded_by`; Version Diff and CHANGELOG are present and record `0.1.1b` (`:1-11`, `:140-152`). |
| 5. Code Alignment | **PASS** | The amendment correctly describes a proposed repair, not existing completion: imported transcription still does not set `active_recording_id` or finalize its chunk on success (`src-tauri/src/lib.rs:2028-2115`), while queued jobs target that active recording (`src/App.tsx:1072-1117`; `tests/jobActions.test.mjs:132-140`). Its retry wording matches current derived-ID/upsert and recording-provenance behavior; `.srt`/`.vtt` retry is likewise recording-idempotent (`src-tauri/src/job_engine.rs:123-127`). |
| 6. Writing Quality | **PASS** | Thai-facing prose remains specific, preserves technical identifiers, separates proposed implementation from current evidence, and does not promote static/source evidence into runtime or release proof. |

### Verdict

**ALL PASS** — all hard gates (1-3) pass. The candidate is suitable for Boss approval of the exact documentation-defined implementation scope; this is not approval of implementation, runtime/UAT, release, or external-state completion.
