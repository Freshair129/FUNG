---
version: "0.2.3b"
created_at: "2026-09-29T21:39:00+07:00,RWANG"
last_update: "2026-09-29T23:25:00+07:00,RWANG"
status: "local runtime qualification passed; hosted CI rerun pending; Thai accuracy not qualified"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "implementation-plan"
  scope: "Isolated local runtime for Detailed Thai transcription"
  risk: "HIGH"
  complexity: "C-3"
---

# Thai candidate runtime qualification

## Goal

Make the approved `ละเอียด` path invoke `thai-large-candidate` through its own
isolated Python runtime, pinned local model and manifest. Keep the existing
`turbo` and `medium` runtime unchanged. The candidate model and dependencies
remain opt-in local artifacts and are not added to the Tauri application
bundle.

## Approved scope and boundaries

- Follow [Whisper Model Profiles](../specs/2026-09-21-whisper-model-profiles.md)
  v0.4.2b and the candidate-runtime RCA.
- Stage a dedicated Python 3.11.9 embedded runtime, candidate-only CPU
  dependencies, the pinned Thai checkpoint and a hash-bearing manifest under
  `.venv-whisper-transformers-candidate/`.
- Resolve that interpreter for candidate readiness and detailed batch jobs.
  Keep live-meeting transcript workers on the operational interpreter because
  Detailed is a post-meeting draft and live output writes to the committed
  transcript stream. Continue to use `.venv-whisper` for general transcription,
  medium, diarization, decode jobs and other workers.
- Load the model offline with bounded host-memory loading. The app must never
  download dependencies or model files, mutate the operational runtime, or
  fall back to another model.
- Do not change schema, committed-transcript authority, external meeting
  providers, installer resources, or release claims.

## Work items

1. [x] Add a pinned candidate dependency lock that combines the already qualified
   Transformers/PyTorch versions with the `faster-whisper` audio decoder
   dependencies required by the candidate worker.
2. [x] Extend candidate staging to create a sibling embedded Python runtime and
   install dependencies only into that candidate root. Stage the model and
   runtime transactionally, preserve any existing `.venv-whisper` content,
   verify the checkpoint size/SHA-256, and record interpreter/dependency/model
   provenance in the manifest.
3. [x] Add a candidate-interpreter resolver derived from the FUNG resource root.
   Use it for candidate readiness and Detailed batch jobs; retain the
   operational interpreter for all existing workers, including live meeting
   transcript workers.
4. [x] Make model loading explicitly use low-CPU-memory loading supported by the
   pinned `accelerate` dependency. Keep local-only/offline loading and existing
   `WhisperOutput` output unchanged.
5. [x] Add focused tests for candidate interpreter selection, readiness import
   checks, missing-runtime fail-closed behavior, worker executable choice,
   offline model path and low-memory loader arguments. Extend staging and
   release-contract tests to prove the candidate runtime remains excluded from
   the application bundle.
6. [x] Run the consolidated source verification campaign after implementation
   and staging: Python 2/2, release/resource Node 8/8, frontend production
   build, Rust 594 passed/0 failed/2 ignored, dependency consistency, Rust
   formatting and `git diff --check`.
7. [x] Run the ignored Rust smoke test through the actual Detailed readiness
   and batch-worker path, then compare the same approved 30-second F:\meeting
   WAV with operational Turbo. No reference transcript is available, so report
   elapsed time and qualitative output only; do not claim WER/CER or improved
   Thai accuracy.

## Acceptance and exit criteria

- The candidate stage is isolated from and leaves the operational
  `.venv-whisper` tree unchanged.
- The candidate manifest records the approved repository/revision/license,
  checkpoint size and SHA-256, and installed runtime dependency versions.
- Readiness succeeds only when the candidate Python, required imports, pinned
  manifest and local model files all pass; it reports
  `accuracyQualified=false`.
- Detailed batch jobs launch the candidate interpreter. General and medium
  jobs, including live-meeting transcript workers, continue launching the
  existing interpreter.
- The staged dependencies import at their pinned versions. Detailed readiness
  passes with `accuracyQualified=false`; the Rust worker loads the local model
  offline and returns valid `WhisperOutput`. The candidate remains absent from
  Tauri release resources. A 30-second same-audio Turbo comparison is complete
  with elapsed-time and qualitative observations only.
- Focused Python/Node/Rust tests, frontend build, Rust formatting and diff
  checks pass. Installer, clean-install, provider, Thai WER/CER, device and
  release gates remain open.

## Verification sequence

Write/update tests with the implementation, but defer test execution until all
work items and local staging are complete. Staging, the consolidated source
suite and the ignored Rust candidate smoke are complete. The Rust suite
required a transient `TAURI_CONFIG` override setting bundle resources to an
empty list because `.knowledge-parser-runtime` is absent from this isolated
worktree; no tracked Tauri config was changed. The first hosted frontend run
found a stale egress assertion after the worker-helper refactor; the assertion
now checks the shared helper and both offline flags and passes 8/8 locally.
Hosted CI rerun is pending. The one-clip comparison has no reference transcript
and does not qualify Thai accuracy.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.2b → 0.2.3b | 2026-09-29 | stale egress assertion fixed; hosted CI pending | Updated the offline source contract to check the shared worker helper and both offline flags. | working-tree | RWANG |
| 0.2.1b → 0.2.2b | 2026-09-29 | local runtime qualification passed; Thai accuracy not qualified | Rust readiness and candidate worker passed on the staged model; one same-audio Turbo comparison and consolidated source checks passed. | working-tree | RWANG |
| 0.2.0b → 0.2.1b | 2026-09-29 | source validation complete; model load pending | Candidate runtime and model staged; pinned imports and source suites pass; the actual worker load and same-audio comparison await sufficient RAM. | working-tree | RWANG |
| 0.1.0b → 0.2.0b | 2026-09-29 | implementation complete; verification pending | Added the separate embedded runtime and hashed CPU lock, candidate interpreter routing, manifest checks and low-memory offline loading; the requested consolidated qualification remains open. | working-tree | RWANG |
| 0.1.0b | 2026-09-29 | approved | Task plan for isolated local candidate runtime and production-worker qualification; app-bundle distribution excluded. | base de696eaa; working-tree | RWANG |
