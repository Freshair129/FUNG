---
version: "0.1.0b"
created_at: "2026-09-20T23:20:00+07:00,Agent: Codex,Commit: d6543c3"
last_update: "2026-09-20T23:20:00+07:00,Agent: Codex"
status: "candidate"
superseded_by: null
attributes:
  domain: "release-verification"
  doc_type: "root-cause-analysis"
  document_kind: "root-cause-analysis"
  scope: "PR #60 Output destination CI failures"
  language: "Thai"
  complexity: "C-2"
  risk: "LOW: CI wiring and unused native helper only"
---

# RCA — Output destination CI failures

## Symptom

PR #60 failed both hosted CI jobs after the Output destination commit. The
frontend job failed its suite inventory gate, and the Rust job failed strict
Clippy.

## Evidence

1. Frontend run `35521764937`, job `106106955317`, reported:
   `test:live-capture-routing, test:recording-output defined in package.json
   but never invoked by .github/workflows/ci.yml`.
2. Rust run `35521764937`, job `106106955178`, reported:
   `method known_roots is never used` at
   `src-tauri/src/recording_output.rs:114` with `-D warnings`.
3. The local implementation tests and build passed before hosted CI, but the
   local checks did not include the exact hosted inventory and strict Clippy
   invocation together.

## Root Cause

The Output destination change added two npm test scripts without adding their
commands to the CI workflow. The same change also left a production-target
helper method that had no caller; hosted Clippy correctly promoted that dead
code warning to an error.

## Why the issue escaped detection

Local verification ran the new tests directly and ran Rust tests, but did not
run `npm run test:ci-coverage` against the workflow inventory or
`cargo clippy --lib --all-targets -- -D warnings` after the final native module
shape was assembled.

## Proposed prevention

1. Add every new `test:*` script to the workflow in the same change.
2. Run `npm run test:ci-coverage` before committing CI-affecting changes.
3. Run strict all-target Clippy after adding or removing native helpers.
4. Keep uncalled production helpers out of the target unless a documented
   consumer is added.

## Bounded repair

- Add `test:live-capture-routing` and `test:recording-output` to the frontend
  CI job.
- Remove the unused `RecordingOutputManager::known_roots` method.
- Preserve the playback allow-list helper and all product behavior.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-20 | candidate | Recorded and repaired PR #60 frontend inventory and strict Clippy failures. | d6543c3 | Codex |
