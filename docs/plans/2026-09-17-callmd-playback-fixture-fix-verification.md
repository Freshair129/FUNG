---
version: "0.1.0b"
created_at: "2026-09-17T17:30:47+07:00,Codex"
last_update: "2026-09-17T17:30:47+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-playback-custody"
  doc_type: "verification-report"
  scope: "Approved four-fixture canonical-root correction"
  complexity: "C-2"
  risk: "HIGH: custody-related fixtures; production unchanged"
---

# Four-fixture playback correction verification

## Scope and frozen source boundary

The approved RCA v0.2.1b and fixture-fix orchestration authorize only four
test-fixture call-site corrections. The source lease is frozen after the
focused test pass. No production comparator, runtime path, workflow,
dependency, assertion, timeout, or unrelated fixture was changed.

Branch: `codex/callmd-ui-dag`  
HEAD: `343f6ea30a1404a4d756cc4a02f0a5458e11dca2`  
Main pin: `05ed107a2233e8785b95d2ba7dc282c47aee35a7`

The four affected tests are:

- `unsupported_wav_is_rejected_before_worker_creation`
- `valid_pcm16_source_is_validated_from_the_custodied_file`
- `stereo_wav_duration_is_per_channel_at_supported_rates`
- `eof_closes_source_and_clears_source_identity`

Each now canonicalizes its own `TempDir` root with fail-closed
`std::fs::canonicalize(...).expect(...)` before the custody operation. The
canonical root is used consistently for `validate_wave`, `open_source`,
`PreparedPlayback.project_root`, and failure-only diagnostics. Raw descriptor
paths, fixture audio, expected error codes, and all assertions are unchanged.
The separate missing-source fixture remains unchanged.

## Root-cause alignment

CI143 showed the raw temporary root using the hosted short spelling
`C:\Users\RUNNER~1\...`, while the file handle resolved to
`\\?\C:\Users\runneradmin\...`. Raw containment was false and canonical-root
containment was true. The correction restores these four fixtures to the
existing production-shaped canonical-root contract; it does not weaken the
custody comparator or turn canonicalization failure into a skip.

## Local verification

| Check | Result |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS, exit 0; existing tooling warning: could not canonicalize `C:\Users\pc` |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml desktop_playback::tests --lib -- --nocapture` | PASS — 20 passed, 0 failed, 0 ignored, 452 filtered out |
| `git diff --check` | PASS, exit 0 |
| Protected 65,854-byte prefix | PASS — SHA-256 `e2c09351b7a9d0c1718eaa0cb5ab6c1bdf99f6afb027b35941db6b267b55621d` |

The focused test run retained only the known unrelated warnings in
`src/auth_session.rs:3384` and `src/backup.rs:110`; no warning was introduced
in `desktop_playback.rs`. Failure-only diagnostics were not exercised locally
because all four expectations passed.

## Hashes and evidence boundary

Source preimage SHA-256: `22caf03015b390bca32dc25076256f3c17edeacc45a5a8b9a8091d3007e47990`  
Source post-patch SHA-256: `efed4fbe4b6746a09e8f88b0c7ec9fed81a8da35684dd8cfd7a29061b327dbaf`

Hosted re-run after this correction: **NOT_RUN**. Full Rust, local strict
Clippy, frontend/build, commit, push, merge, deployment, native launch,
packaged runtime, device, provider, and production evidence: **NOT_RUN** in
this lease. Local focused PASS does not waive hosted or runtime gates.

## Version diff / CHANGELOG

New -> `0.1.0b`: recorded the approved four-fixture canonical-root
correction, source freeze, local focused verification, hashes, and explicit
unrun hosted/runtime gates.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Four-fixture-only correction verified locally; hosted and runtime gates remain NOT_RUN. | UNCOMMITTED; base `343f6ea` | Codex |
