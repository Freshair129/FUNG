---
id: RCA-PR33-CI-GATES
status: remediation
date: 2026-08-27
scope: PR-33 frontend and Rust CI gates
---

# RCA — PR #33 CI gate failures

## Symptom

PR #33 is structurally mergeable but its required `frontend` and `rust` jobs
fail. The frontend job fails after a successful production build. The Rust job
fails before tests because Clippy warnings are promoted to errors.

## Evidence

- Frontend run `32995637908`, job `98263970246`:
  - `test:google-drive` exists in `package.json` but is not invoked by
    `.github/workflows/ci.yml`.
  - `nativeSessionCustody.test.mjs` and `w1AuthoritySchema.test.mjs` exist in
    `tests/` but no npm script invokes them.
- Rust run `32995637908`, job `98263970558`:
  - `cargo clippy --lib --all-targets -- -D warnings` reports 27 errors.
  - Diagnostics cover unused legacy functions, three type-complexity results,
    `manual_contains`, three too-many-arguments results, an identical branch,
    and two needless returns.
- The working tree before remediation is clean and PR #33 currently changes
  only the documentation truth-sync set.

## Root cause

The CI coverage contract was updated to detect unwired suites, but the
workflow and npm scripts were not updated for the two newer suites and the
Google Drive contract suite. Separately, the Rust toolchain used by CI emits
newly enforced Clippy diagnostics for legacy deregistered commands, test-only
helpers compiled in the library target, and a small number of pre-existing
signature/return-shape patterns.

## Why the issue escaped detection

The local verification run covered focused suites and `cargo test`, but did not
reproduce the complete CI frontend sequence and did not run Clippy with
`-D warnings` on the same Windows toolchain version as the hosted runner.

## Remediation

1. Add npm scripts for both orphan test files and invoke all three missing
   suites from the frontend workflow.
2. Remove deregistered legacy command wrappers and the unused legacy enrollment
   proof implementation; mark test-only filesystem helpers with test/non-
   desktop cfg boundaries.
3. Replace simple Clippy findings with equivalent expressions, add aliases for
   repeated lifecycle result types, and preserve intentionally multi-argument
   transaction/test helpers with narrow, reasoned lint annotations where a
   refactor would obscure the security boundary.
4. Re-run the complete frontend sequence and Rust fmt, Clippy, and test gates.

## Prevention

- Keep `test:ci-coverage` first in the frontend workflow and require it to pass
  before suite execution.
- Treat `cargo clippy --all-targets -- -D warnings` as a required local check
  whenever Rust code or the Rust toolchain changes.
- Keep legacy command removal and test-only compatibility helpers explicit in
  source cfg boundaries so deregistration cannot silently leave dead APIs.

## Acceptance criteria

- `npm run test:ci-coverage` passes and reports no unwired scripts or orphan
  test files.
- Every frontend command listed in the workflow passes, including Google Drive,
  Native Session Custody, and W1 Authority Schema.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` passes.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings`
  passes without broad warning suppression.
- `cargo test --manifest-path src-tauri/Cargo.toml` passes.
