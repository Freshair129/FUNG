---
version: "0.1.0b"
created_at: "2026-09-17T17:36:21+07:00,Codex Terra independent reviewer,343f6ea30a1404a4d756cc4a02f0a5458e11dca2"
last_update: "2026-09-17T17:36:21+07:00,Codex Terra independent reviewer"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-playback-custody"
  doc_type: "independent-review"
  scope: "Approved four-fixture canonical-root correction"
  base_commit: "343f6ea30a1404a4d756cc4a02f0a5458e11dca2"
  main_pin: "05ed107a2233e8785b95d2ba7dc282c47aee35a7"
  complexity: "C-2"
  risk: "HIGH: custody-related fixtures; production unchanged"
  review_decision: "PASS_WITH_HOSTED_GATE_OPEN"
---

# Independent review — four-fixture playback correction

## Decision

**Source review: PASS.** The reviewed worktree remains at base
`343f6ea30a1404a4d756cc4a02f0a5458e11dca2`; the approved source change is
bounded to the four named fixture call sites in
`src-tauri/src/desktop_playback.rs`. The correction constructs a canonical
fixture root with fail-closed `std::fs::canonicalize(...).expect(...)` before
the custody operation and sends that same supplied root to failure-only
diagnostics.

**Closure: WARN — hosted evidence is still required.** A local pass does not
close CI, installed Desktop, native-package, device, provider, or deployment
acceptance. Before fixture closure, hosted Windows CI must show that all four
previously failing cases pass, alongside the required custody, full Cargo,
strict Clippy, and frontend checks.

## Reviewed provenance and integrity pins

| Item | Independent result |
| --- | --- |
| Base / current HEAD | `343f6ea30a1404a4d756cc4a02f0a5458e11dca2` / same |
| Main pin | `05ed107a2233e8785b95d2ba7dc282c47aee35a7` (not merged) |
| Source preimage SHA-256 | `22caf03015b390bca32dc25076256f3c17edeacc45a5a8b9a8091d3007e47990` — PASS |
| Source postimage SHA-256 | `efed4fbe4b6746a09e8f88b0c7ec9fed81a8da35684dd8cfd7a29061b327dbaf` — PASS |
| Protected prefix | first 65,854 bytes before `#[cfg(test)] mod tests {`; SHA-256 `e2c09351b7a9d0c1718eaa0cb5ab6c1bdf99f6afb027b35941db6b267b55621d` — PASS |
| Worker verification report | SHA-256 `32b206ba61faf087687632450569c9aaac6c8f50e47b3d6245d0bc296fcb1384` — PASS |

The source diff contains exactly four added fixture-root canonicalizations at
lines 2286, 2321, 2365, and 2783. Their changed function contexts are exactly:

- `unsupported_wav_is_rejected_before_worker_creation`
- `valid_pcm16_source_is_validated_from_the_custodied_file`
- `stereo_wav_duration_is_per_channel_at_supported_rates`
- `eof_closes_source_and_clears_source_identity`

Raw descriptor paths, WAV fixture bytes, expected errors, assertions, error
codes, negative custody tests, other fixtures, production prefix, and custody
comparator are unchanged. No workflow, dependency, timeout, skip, production
logging, refactor, source behavior, Git, provider, deployment, native-app, or
credential action was performed by this review.

## Independent commands and results

| Command | Result |
| --- | --- |
| `git diff --check 343f6ea30a1404a4d756cc4a02f0a5458e11dca2 --` | PASS |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS, exit 0; warning: `could not canonicalize path C:\\Users\\pc` |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml desktop_playback::tests --lib -- --nocapture` | PASS: 20 passed, 0 failed, 0 ignored, 452 filtered out |

The focused run emitted two pre-existing, unrelated compiler warnings only:
unused `domain` in `src/auth_session.rs:3384` and unused `acquire_job` in
`src/backup.rs:110`. No warning named `desktop_playback.rs` was emitted.

## Boundary remaining after this review

Hosted re-run, full Cargo, custody, strict Clippy, frontend/build, commit,
push, merge, deployment, native launch, packaged runtime, device, provider,
and production evidence remain **NOT_RUN** by this review. Existing approved
implementation may proceed to its explicit reviewed commit/push path, but this
review grants no source-fix authority beyond the four fixtures and makes no
native-package acceptance claim.

## Version diff / CHANGELOG

New -> `0.1.0b`: independent bounded review records matched source/report
hashes, protected-prefix integrity, exact four-fixture scope, scoped formatter
and 20-test evidence, known warnings, and the hosted-only closure gate.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Fixture-only source review passed; hosted closure remains open. | UNCOMMITTED; base `343f6ea` | Codex Terra independent reviewer |
