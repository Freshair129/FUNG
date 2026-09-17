---
version: "0.1.3b"
created_at: "2026-09-17T13:09:50+07:00,Codex,053d2c5024033d4eed0ec6bd057ce45b0ef112d1"
last_update: "2026-09-17T16:17:11+07:00,Codex orchestrator"
status: "under review"
superseded_by: null
attributes:
  domain: "desktop-playback-custody"
  doc_type: "root-cause-analysis"
  scope: "Hosted Windows desktop_playback failures in PR59 synthetic merge"
  complexity: "C-2"
  risk: "HIGH: security-sensitive path custody; test-only diagnostics approved, no behavior fix authorized"
---

# Candidate RCA — hosted Windows playback custody failures

The initial investigation below was read-only. Boss's latest `approve`
authorizes only the diagnostic-only scope recorded below, not a playback fix.
Root cause remains UNKNOWN until observed evidence establishes it. The current
execution record is `docs/plans/2026-09-17-callmd-playback-diagnostic-orchestration.md`.

## Pinned scope and evidence boundary

- Local head: `053d2c5024033d4eed0ec6bd057ce45b0ef112d1`, branch
  `codex/callmd-ui-dag`.
- Hosted synthetic merge: `23c5edba4baac89c76bc622e5f14cbe4a3b11c44`.
- Hosted run/job: [run 35186717039, Rust job 105090249134](https://github.com/Freshair129/FUNG/actions/runs/35186717039/job/105090249134).
- `src-tauri/src/desktop_playback.rs` preimage SHA-256:
  `b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae`.
- The approved CI RCA `.brain/rca/2026-09-17-callmd-pr59-ci-merge-inventory.md`
  is a separate workflow-only authorization. This report does not extend it.

Evidence classes are kept separate: local head/test evidence is not hosted
synthetic-merge evidence, and neither is native packaged/device/production
runtime evidence.

## Symptom

On the hosted Windows job for synthetic merge `23c5edb`, formatting and strict
Clippy passed, but the Rust suite ended at `467 passed, 4 failed, 1 ignored`.
The four failures were:

- `eof_closes_source_and_clears_source_identity`
- `stereo_wav_duration_is_per_channel_at_supported_rates`
- `unsupported_wav_is_rejected_before_worker_creation`
- `valid_pcm16_source_is_validated_from_the_custodied_file`

All four reported `PLAYBACK_PATH_DENIED`. The unsupported-format test therefore
did not reach WAV validation and reported `PLAYBACK_PATH_DENIED` instead of its
expected `PLAYBACK_FORMAT_UNSUPPORTED`.

This is a hosted CI failure only. It is not evidence of a successful native
runtime, packaged Desktop, device, or production playback run.

## Evidence

### Hosted synthetic merge

The hosted job metadata confirms head `053d2c5`, the `windows-latest` label,
successful fmt/Clippy steps, and a failed `cargo test` step. The supplied job
failure evidence records the four test names and error-code behavior above.

The hosted output does not provide the exact raw `TempDir` root, ledger path,
`GetFinalPathNameByHandleW` result, or canonical project-root spelling. No
hosted runner path or path representation is inferred here.

### Local head

The bounded local command was:

```text
cargo test --offline --manifest-path src-tauri/Cargo.toml desktop_playback::tests --lib -- --nocapture
```

Result on the local Windows host: `20 passed; 0 failed; 0 ignored` in the
`desktop_playback::tests` module, including all four hosted-failing tests.
This result is local-head evidence only.

### Source path-custody flow

The relevant source at the pinned preimage shows:

- `normalized_path_inside` strips only a leading `\\?\` and then uses
  component-based `Path::starts_with` (`desktop_playback.rs:731-746`). It does
  not canonicalize both operands, resolve short names, or explicitly normalize
  Windows case.
- Windows `final_path_from_handle` calls
  `GetFinalPathNameByHandleW(..., FILE_NAME_NORMALIZED)` and rejects UNC/device
  forms before returning the handle path (`:750-786`).
- `open_custodied_file` resolves/open-checks the candidate, checks file type and
  byte size, then compares the final handle path against the supplied root; a
  mismatch returns `PLAYBACK_PATH_DENIED` before `hound` sees the file
  (`:812-835`).
- Production `project_custody_root` canonicalizes the `projects` root and the
  project root before `prepare_playback` passes that root into `validate_wave`
  (`:925-952`, `:1000-1088`).
- The four focused fixtures use `temp_wave`, whose `directory.path()` is passed
  directly as the root while the file path is passed as `path.display()`
  (`:2083-2200`). This is a different root-construction path from production.

### Narrow path-representation diagnostic

A temporary PowerShell probe created one 4-byte file, opened it, and queried
the same Windows handle API used by production. Its result was:

```text
raw root:
C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\fixture-root-with-long-name

handle path:
\\?\C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\fixture-root-with-long-name\mic-00001.wav

after the production prefix strip:
C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\fixture-root-with-long-name\mic-00001.wav

raw-root prefix check: True
GetShortPathNameW(root): no alias returned
```

Therefore the proposed short-8.3 mismatch did **not reproduce on this local
fixture**; an alternate alias was unavailable. The following is an inference
from the comparator source, not an observed probe result: if the root supplied
to `normalized_path_inside` used a different spelling (for example a short
alias or case variant) from the normalized handle path, this comparator could
return false even when both names identify the same Windows directory. That
inference alone does not prove that the hosted runner used such a spelling.

## Root Cause

**UNKNOWN — not proven by the available hosted evidence.**

The strongest current hypothesis is a Windows path-representation mismatch
between the test's raw `TempDir` root and the handle's normalized final path.
Short 8.3 spelling is one possible form of that mismatch; case spelling or
another Windows path normalization difference are also possible. The local
probe did not reproduce the hypothesis on the current fixture because no
alternate alias was available, while the hosted logs supplied for this run
omit the two path values needed to confirm or reject it for `windows-latest`.

The failure is therefore not attributed to the WAV parser, channel-duration
math, EOF cleanup, or a production storage escape. `PLAYBACK_PATH_DENIED` is an
earlier custody gate that masks those later assertions.

## Fixture/production impact boundary

- **Fixture boundary:** `temp_wave` passes a raw temporary-directory spelling as
  the root. A hosted-only spelling mismatch could make the four tests fail even
  if the file is physically under that directory. This remains a possible
  test-fixture precondition defect, not a confirmed production defect.
- **Production boundary:** production canonicalizes the project root before
  playback, which is stronger than the fixture setup. However, production still
  uses the same final-handle versus `Path::starts_with` comparison, so a real
  environment-specific spelling mismatch could deny production too. No native,
  packaged, device, or production runtime evidence was collected or claimed.
- **Security boundary:** no custody check was weakened, no assertion was
  skipped, and no path was treated as trusted based on the hypothesis.

## Why the issue escaped detection

1. The local Windows path spelling did not expose an 8.3 alias and all 20
   focused playback tests passed on the local head.
2. The hosted test ran in a different Windows environment and the fixtures did
   not record the raw root and handle-final path needed to diagnose a
   representation mismatch.
3. The custody check runs before WAV-format validation, so one path failure
   presents as four apparently different playback failures and changes the
   expected error for the unsupported-WAV case.
4. The workflow/merge investigation and this playback investigation are
   separate gates; the CI workflow repair does not establish a playback RCA.

## Proposed prevention / minimal future approval scope

No fix is authorized by this report. If a future approval is requested, keep it
bounded to the following evidence-first scope:

1. Add one Windows-only diagnostic/regression fixture that records (or safely
   hashes) the raw root spelling and final handle spelling, and exercises an
   actual same-directory alternate spelling when the host provides one. Keep
   the output limited to the focused playback test; do not log credentials or
   unrelated runner data.
2. If the hosted evidence proves a fixture-only mismatch, adjust only the
   fixture root to the production-shaped canonical root and retain the existing
   `PLAYBACK_PATH_DENIED` assertions. Re-run the four focused tests and the
   local custody inventory.
3. If a production-shaped fixture reproduces the mismatch, propose a separate
   security-reviewed comparator change that canonicalizes/normalizes both
   sides while preserving component-boundary, symlink/escape, UNC/device, and
   fail-closed checks. Add a regression for the proven representation before
   changing behavior.
4. Re-run the focused local module and a fresh hosted Windows job. Do not use a
   green local run to waive a hosted failure, and do not classify a hosted path
   until the logs contain the relevant path evidence.

This proposed scope does not authorize changes to the workflow, WAV parser,
path-custody policy, security assertions, package/runtime, deployment, or
production state.

## Diagnostic temporary-path inventory and cleanup

The following task-owned paths were created under the explicitly verified
task-specific directory and were removed after the probe. The shared `.tmp`
parent was not recursively inspected or deleted.

- `C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\path-probe.ps1`
  — generated probe; SHA-256 before cleanup
  `a518c1e22d630bf1afea88701eeae073a09b3d2654453d0ec07b3c2f1a58c227`.
- `C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\fixture-root-with-long-name\mic-00001.wav`
  — generated 4-byte sentinel fixture; removed.
- `C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917\fixture-root-with-long-name`
  — generated fixture directory; removed.
- `C:\Users\pc\.codex\worktrees\9000\fung\.tmp\callmd-playback-20260917`
  — task directory; removed after verifying it contained only the paths above.

Post-cleanup verification: the owned task directory is absent; the source
preimage SHA remains unchanged; no source/test files were modified.

## Status

`DIAGNOSTIC_APPROVED / ROOT CAUSE UNKNOWN / NO BEHAVIOR FIX AUTHORIZED`.

## Fresh post-repair hosted evidence and next approval boundary

The workflow-only repair was independently reviewed, committed and pushed as
`76b14c5e4a3bd8ba2e8602de908abf2351f35f97`. Remote branch equality was verified.
Main remains `05ed107a2233e8785b95d2ba7dc282c47aee35a7`; PR59 is not merged.
Hosted CI142 checked out synthetic merge
`94d2c037f728ca30af5b10fa7272a986699af147`.

Fresh [run35190221858](https://github.com/Freshair129/FUNG/actions/runs/35190221858)
completed with failure:

- Frontend job105100934385: PASS, build1812 modules, inventory4/4, all50 CallMD
  tests and remaining frontend suites passed.
- Windows Rust job105100934188: formatting PASS, native-session-custody11/11
  PASS, strict Clippy PASS.
- Full Cargo suite:467 PASS,4 FAIL,1 ignored. The same four playback tests above
  again reached `PLAYBACK_PATH_DENIED`; unsupported-WAV still expected
  `PLAYBACK_FORMAT_UNSUPPORTED`. This confirms recurrence, not its root cause.
- The actual hosted merged workflow Git blob is
  `f9dee9eb19ed72f2dc7b3eb470e11dcc244a63d6`, equal to Terra's byte-verified
  prospective result. The missing-invocation defect is repaired; full CI and
  deployment gates remain open.

### Approved next step — diagnostic-only

Boss approved this exact scope after the v0.1.2b proposal. This is not permission
to implement the conditional fixes discussed earlier:

1. A fresh Luna/max receives `src-tauri/src/desktop_playback.rs` **test-only**
   diagnostic scope inside `#[cfg(test)]`, plus a dedicated report. Main remains
   orchestrator and does not author implementation/test code.
2. Add bounded failure diagnostics for the four affected fixture paths: raw
   fixture root, resolved candidate, final handle path and canonical root, or
   safe representations sufficient to compare them. Emit only test-owned
   paths/identity facts on failure; no credentials, audio contents, unrelated
   runner data, production logging or general environment dumps.
3. Preserve every assertion, error code, test invocation and production custody
   check. Do not canonicalize a fixture to hide the failure, alter the comparator,
   skip tests, change timeouts, or claim a fix before the evidence identifies the
   actual failing precondition.
4. Independent fresh Terra review verifies the test-only diff and no runtime
   change. After approval/review, explicitly scoped commit/push may collect a
   fresh hosted Windows failure with diagnostics.
5. Update the RCA from those observations. Any subsequent fixture correction or
   production/security behavior change requires a separate evidence-backed
   proposal and Boss approval; it is not automatically authorized here.

Complexity remains C-2; risk remains HIGH because the investigation touches a
security-sensitive custody boundary even though the proposed change is test-only.
Deployment target remains unanswered (installed Desktop, web Vercel, or both).
No deployment, installation, native launch, credentials/user-data access, PR
merge, production comparator change or acceptance waiver has occurred.

This playback RCA was excluded from the earlier five-file workflow-repair
commit. It may accompany the newly approved diagnostic-only publication after
independent review; it does not imply a source behavior fix or deployment.

## Version diff / CHANGELOG

New -> `0.1.0b`: bounded read-only hosted/local playback investigation; local
short-8.3 hypothesis did not reproduce on the available fixture; hosted path
identity remains unavailable; minimal future approval scope recorded; no
implementation change.
`0.1.0b` -> `0.1.1b`: corrected the report timestamp and marked the
alternate-spelling statement as source-based inference rather than probe
observation; root cause remains unknown and no implementation change was made.
`0.1.1b` -> `0.1.2b`: add exact pushed SHA and fresh hosted recurrence; propose
test-only diagnostics as a separate approval scope, not a playback fix.
`0.1.2b` -> `0.1.3b`: record Boss approval of test-only diagnostics and scoped
commit/push after independent review. No behavior fix or deployment authorized.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.3b | 2026-09-17 | under review | Diagnostic-only execution approved; RCA remains unknown | UNCOMMITTED; base76b14c5 | Codex orchestrator |
| 0.1.2b | 2026-09-17 | candidate | CI wiring repaired and verified; playback failure recurs; diagnostic-only proposal awaits approval | UNCOMMITTED; inspected 76b14c5 | Codex orchestrator |
| 0.1.1b | 2026-09-17 | candidate | Corrected evidence wording and timestamp; hosted path identity remains unknown; no code/test fix authorized | UNCOMMITTED; inspected 053d2c5 | Codex |
| 0.1.0b | 2026-09-17 | candidate | Hosted playback custody RCA remains unknown after bounded local reproduction; no code/test fix authorized | UNCOMMITTED; inspected 053d2c5 | Codex |
