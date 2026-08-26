# Task 12 Brief — FUNG MVP Critical Path Amendment

## Role

You are a technical documentation writer for FUNG. Use model `gpt-5.6-luna`.

## Task

Audit current repository truth and draft one reviewable amendment that reprioritizes
work around the FUNG MVP critical path. The user-defined product outcome is:

`local audio input/capture -> transcription -> transcript review -> Minute of Note -> local export`

The amendment must distinguish implemented, locally verified, runtime/UAT-open,
externally gated, and deferred work. It must identify the smallest next implementation
slice that materially improves the MVP and provide Goal, Acceptance Criteria, Success
Criteria, Exit Criteria, PIC, approver, risk, dependencies, exact write scope, tests,
and out-of-scope boundaries.

## Scope boundaries

- Documentation only. Do not modify source code, configuration, environment files,
  credentials, tests, generated artifacts, Git state, or external systems.
- Google Drive OAuth, Hyper-V/ISO, clean-install restore, Android/device proof,
  cloud/provider release claims, speaker recognition, and production deployment are
  deferred unless repository evidence proves one is an unavoidable dependency of the
  local MVP flow.
- Preserve all existing dirty/untracked files and do not stage, commit, push, open a
  PR, merge, deploy, or clean up.
- Do not claim a runtime pass from unit/static evidence.

## Input context

- `AGENTS.md`
- `docs/plans/2026-08-09-fung-master-implementation-plan.md`
- `docs/Desktop/ARCHITECTURE.md`
- `docs/Mobile/IMPLEMENTATION_STATUS.md`
- `docs/Desktop/08-real-progress.md`
- `docs/.rwang-progress.md`
- `docs/.rwang-tasks/task-11-report.md`
- Relevant current source/tests for import/capture, transcription, transcript view,
  summary/Minute of Note, and local export. Trust hierarchy: code > SDD/design > PRD.
- `C:/Users/freshair/.codex/plugins/cache/personal/rwang-plugin/1.0.2/references/templates.json`

## Requirements and constraints

- Thai-facing prose; retain technical identifiers in English.
- Use fresh IDs `D-MVP-01` onward, without reusing existing project IDs.
- Prefer one focused implementation slice; list later MVP slices separately and in
  dependency order.
- Explicitly assess whether login, Google Drive, mobile pairing, cloud credentials,
  diarization/voice recognition, and external connectors block this MVP definition.
- Surface contradictions and unknowns; do not silently choose unsupported facts.
- Candidate document must include required YAML frontmatter and CHANGELOG/footer per
  `AGENTS.md`.

## Output contract

1. Create `docs/specs/2026-08-26-fung-mvp-critical-path-amendment.md` as a candidate
   specification only.
2. Create `docs/.rwang-tasks/task-12-report.md` using the Writer Report contract.
3. Do not edit any other file.

## Success condition

The candidate is precise enough for Terra to review and for Boss to approve an exact
implementation slice later, while making no code or external-state change now.
