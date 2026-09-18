---
version: "0.1.0b"
created_at: "2026-09-17T16:29:45+07:00,Terra independent review"
last_update: "2026-09-17T16:29:45+07:00,Terra independent review"
status: "under review"
superseded_by: null
attributes:
  domain: "desktop-playback-custody"
  doc_type: "independent-source-review"
  scope: "Approved Windows test-only playback diagnostics"
  complexity: "C-2"
  risk: "HIGH: security-sensitive custody investigation; diagnostic-only test change"
---

# Independent Terra review — playback diagnostic-only patch

## Verdict

**PASS — source review gate.** The frozen postimage is a Windows-only,
failure-only diagnostic addition inside the existing `#[cfg(test)] mod tests`.
It does not modify the protected production prefix, runtime custody comparator,
fixture construction, test inputs, asserted error code, assertion outcome,
workflow, dependencies, or playback behavior.

This is a source-review result, not hosted evidence. The local diagnostic
branch was **NOT_EXERCISED** because the focused tests passed. Hosted CI remains
**NOT_RUN** and is still required to collect the four expected failure
diagnostics. Root cause remains **UNKNOWN**; no fixture correction, custody
comparator change, deployment, or acceptance waiver is authorized by this
review.

## Authority, inputs, and identity checks

- Reviewed against base `76b14c5e4a3bd8ba2e8602de908abf2351f35f97`; main was
  supplied as `05ed107a2233e8785b95d2ba7dc282c47aee35a7`.
- Read approved RCA
  `.brain/rca/2026-09-17-callmd-hosted-playback-path.md` v`0.1.3b` and the
  approved orchestration plan before source review.
- Source preimage required by the approved record:
  `b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae`.
- Frozen reviewed source postimage:
  `22caf03015b390bca32dc25076256f3c17edeacc45a5a8b9a8091d3007e47990`.
- Exact test-module boundary: byte offset `65854`; protected UTF-8 prefix
  SHA-256:
  `e2c09351b7a9d0c1718eaa0cb5ab6c1bdf99f6afb027b35941db6b267b55621d`.
  Both match the approved baseline values.
- Darwin verification report was present and read before this verdict:
  SHA-256 `85efebaefebcd3a227540983e58c331c74e2eef283afea33f9947a40239b392d`.
- Review clock: `2026-09-17T16:29:45+07:00`.

## Diff and safety assessment

`git diff --check` against the base was clean. The only tracked diff from the
base is `src-tauri/src/desktop_playback.rs`, with all additions/call-site
changes after the protected `#[cfg(test)]` boundary. No staged changes were
present during review.

The patch adds `#[cfg(windows)]` test helpers and calls them only when one of
the four existing test expectations is false:

- `unsupported_wav_is_rejected_before_worker_creation`
- `valid_pcm16_source_is_validated_from_the_custodied_file`
- `stereo_wav_duration_is_per_channel_at_supported_rates`
- `eof_closes_source_and_clears_source_identity`

The assertion-bearing results remain unchanged: the unsupported fixture still
requires `PLAYBACK_FORMAT_UNSUPPORTED`; the PCM/stereo calls still require a
source; and EOF still requires successful queue fill followed by the existing
state assertions. The patch only binds each pre-existing call result before
the same unwrap/assertion, so it neither skips nor weakens a failure.

On Windows, the helper repeats read-only custody observations for the trusted
`temp_wave` fixture: raw fixture root, resolved candidate, handle-final path,
canonical root, stage/status facts, byte-size/file facts, and raw/canonical
plus case-insensitive containment booleans. Candidate/final/canonical paths
are emitted only when spelling or canonical containment establishes that they
are inside the test fixture; otherwise the output is redacted. The raw root is
the test-owned `TempDir` root supplied directly by the four fixture calls.
There is no environment dump, credential/provider/keyring access, audio/file
content output, external file traversal, persistent write, or production log.

These facts are sufficient to distinguish the prior gate alternatives without
changing the gate: reject/resolve/open/metadata/final-handle/containment versus
post-custody failure, then compare raw, canonical, and case-insensitive path
relationships. The extra observation cannot cause a test to pass; it runs
only after the actual result is known to violate its original expectation.

## Independent proportional verification

Executed in `C:\Users\pc\.codex\worktrees\9000\fung`:

```text
cargo fmt --check --manifest-path src-tauri/Cargo.toml
exit 0

cargo test --offline --manifest-path src-tauri/Cargo.toml desktop_playback::tests --lib -- --nocapture
exit 0
20 passed; 0 failed; 0 ignored; 452 filtered out

git diff --check 76b14c5e4a3bd8ba2e8602de908abf2351f35f97 -- src-tauri/src/desktop_playback.rs
exit 0
```

The focused Windows build emitted no warning from `desktop_playback.rs`.
Only pre-existing warnings remained: unused `domain` in
`src/auth_session.rs:3384` and dead-code `acquire_job` in `src/backup.rs:110`.
Cargo also printed its pre-existing tooling warning that it could not
canonicalize `C:\Users\pc`. No full Rust suite or unrelated Clippy baseline was
required for this diagnostic-only review.

## Publication status and blockers

There is **no Terra source-review blocker** to the authorized, explicit-path
commit/push. Publication must still be performed by the main orchestrator with
an explicit index that includes only the intended diagnostic source and review
artifacts; this reviewer made no Git write, commit, push, or hosted dispatch.

Hosted diagnostic evidence is the remaining gate: after publication, verify
the exact published SHA and hosted synthetic-merge SHA, then retain the
expected four-test failure if it recurs and collect its failure-only lines.
Hosted success/failure must not be inferred from this local 20/20 result. Any
behavioral fix remains separately approval-gated.

## Version diff / CHANGELOG

New -> `0.1.0b`: independent Terra review records frozen source identity,
test-only scope/safety assessment, proportional local verification, and the
remaining publication/hosted-evidence boundary.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | under review | PASS source-review gate; hosted diagnostics remain not run | UNCOMMITTED; base `76b14c5` | Terra independent review |
