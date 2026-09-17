---
version: "0.1.0b"
created_at: "2026-09-17T13:15:28+07:00,Codex,UNCOMMITTED"
last_update: "2026-09-17T13:20:29+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "release-verification"
  doc_type: "verification-report"
  scope: "PR59 CI workflow repair; frozen local and prospective-merge evidence only"
  complexity: "C-2"
  risk: "MEDIUM: CI coverage and integration semantics; no product behavior change"
  hosted_gate: "NOT_RUN"
  workflow_path: ".github/workflows/ci.yml"
  report_path: "docs/plans/2026-09-17-callmd-pr59-ci-repair-verification.md"
---

# PR59 CI repair verification

## Scope and disposition

This report covers only the two worker-owned paths:

1. .github/workflows/ci.yml — implementation source, frozen at the approved
   postimage SHA-256 below.
2. This report — local inventory and prospective-merge evidence.

No workflow edit is authorized after the frozen checkpoint unless an
independent review requests a fresh approved fixer change. No package,
script, test, source, assertion, security, native runtime, provider, Drive,
deployment, commit, push, merge, or hosted-run action was performed by this
worker.

The hosted verification gate is explicitly NOT_RUN. Local evidence is not
hosted CI proof.

## Pinned provenance

| Item | Value |
| --- | --- |
| Worker branch | codex/callmd-ui-dag |
| Feature head | 053d2c5024033d4eed0ec6bd057ce45b0ef112d1 |
| Current main | 05ed107a2233e8785b95d2ba7dc282c47aee35a7 |
| origin/main observed | 05ed107a2233e8785b95d2ba7dc282c47aee35a7 |
| Common merge base | c378af9fac3c00db063948f49f9ee857ebad9126 |
| Previous hosted synthetic merge object | 23c5edba4baac89c76bc622e5f14cbe4a3b11c44 |
| Workflow preimage SHA-256 | 3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a |
| Workflow postimage SHA-256 | 52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e |
| Common-base workflow blob | 2a0dbdac49ed7d58026c5ea19aa3bb2b420e7ee1 |
| Main workflow blob | d428e20557b4ec65a2ecec0bffccce85608e65af |
| Feature-head workflow preimage blob | 6144fbec57836418434e6c8f9f91be563d445370 |
| Frozen postimage workflow blob | f9dee9eb19ed72f2dc7b3eb470e11dcc244a63d6 |
| Verification clock | 2026-09-17T13:15:28+07:00 |

The previous synthetic merge object is recorded from the approved RCA and
hosted evidence. It was not present as a local Git object in this isolated
clone; the prospective check below therefore uses the pinned base/main
objects plus the frozen working-tree postimage.

## Root cause and bounded repair

Main deleted the workflow tail that the feature head retained unchanged from
the common base. A textual three-way merge therefore loses
npm run test:native-session-custody even though the package script and
feature source remain present. The inventory assertion correctly catches that
merged-result omission.

The frozen repair relocates the Windows Rust-job Node 22 setup and custody
invocation immediately after cargo fmt, with an idempotent empty-resource
preflight before the custody suite. The original resource preparation remains
before strict Clippy and cargo test. This keeps the custody suite on the
Windows Rust job, preserves its resource prerequisite, and leaves all
CallMD/other suites, strict Clippy, and cargo tests intact.

## Exact workflow diff

~~~diff
diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml
index 6144fbe..f9dee9e 100644
--- a/.github/workflows/ci.yml
+++ b/.github/workflows/ci.yml
@@ -59,36 +59,39 @@ jobs:
       - run: npm run test:audio-viz
       - run: npm run test:transcribe-concat

   rust:
     runs-on: windows-latest
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
         with:
           components: rustfmt, clippy
       - uses: Swatinem/rust-cache@v2
         with:
           workspaces: src-tauri
           # Pulling a warm cache is useful for PR verification, but uploading
           # the Windows target directory can keep the required check open long
           # after all verification steps have passed.
           save-if: ${{ github.event_name != 'pull_request' }}
       # Formatting first: it needs no build, so a formatting-only failure
       # reports in seconds instead of after a full compile.
       - run: cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
+      # The native custody matrix runs on the Windows Rust job after formatting
+      # and needs the same empty bundle-resource roots before its cargo-backed checks.
+      - run: New-Item -ItemType Directory -Force .venv-whisper, runtime
+      - uses: actions/setup-node@v4
+        with:
+          node-version: 22
+      # This suite invokes the Rust behavioral matrix. Keep it on the Rust
+      # runner so it uses the platform/toolchain that owns that matrix rather
+      # than requiring GTK/GLib development packages on the frontend runner.
+      - run: npm run test:native-session-custody
       # tauri-build copies bundle.resources at build-script time (unconditionally,
       # regardless of the custom-protocol feature). .venv-whisper/ and runtime/ are
       # gitignored, so create them empty — empty dirs are skipped by ResourcePaths.
       - run: New-Item -ItemType Directory -Force .venv-whisper, runtime
       # `-D warnings` covers dead code too, so an unused item has to be either
       # removed or annotated with why it is kept. Every current exemption
       # carries that reason inline.
       - run: cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings
       - run: cargo test --manifest-path src-tauri/Cargo.toml
-      - uses: actions/setup-node@v4
-        with:
-          node-version: 22
-      # This suite invokes the Rust behavioral matrix. Keep it on the Rust
-      # runner so it uses the platform/toolchain that owns that matrix rather
-      # than requiring GTK/GLib development packages on the frontend runner.
-      - run: npm run test:native-session-custody
~~~

## Local verification

All commands below were run from
C:\Users\pc\.codex\worktrees\9000\fung. No dependency installation was run.

| Command | Result |
| --- | --- |
| Get-FileHash .github/workflows/ci.yml -Algorithm SHA256 | Frozen postimage matched 52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e |
| git diff --check | PASS |
| npm run test:ci-coverage | PASS — 4 passed, 0 failed, 0 skipped; exit 0 |

The local inventory covered the four assertions for package-script coverage,
test-file coverage, workflow command resolution, and exact command inventory.

## Prospective merge reproduction

The check was read-only with respect to the repository. It created three
temporary text inputs and one output under a unique %TEMP% directory, ran
git merge-file, captured the output hashes, and removed only that exact
worker-created temporary directory. No Git index, branch, commit, merge,
remote, or shared .tmp path was changed.

### Reproduction inputs

- base.yml: .github/workflows/ci.yml from
  c378af9fac3c00db063948f49f9ee857ebad9126
- main.yml: .github/workflows/ci.yml from
  05ed107a2233e8785b95d2ba7dc282c47aee35a7
- head-frozen.yml: the working-tree workflow whose SHA-256 is the frozen
  postimage above

### Reproduction commands

~~~powershell
$base = git show c378af9fac3c00db063948f49f9ee857ebad9126:.github/workflows/ci.yml
$main = git show 05ed107a2233e8785b95d2ba7dc282c47aee35a7:.github/workflows/ci.yml
Copy-Item C:\Users\pc\.codex\worktrees\9000\fung\.github\workflows\ci.yml head-frozen.yml

git merge-file --stdout --diff3 main.yml base.yml head-frozen.yml > prospective-merged.yml
git hash-object prospective-merged.yml
Get-FileHash prospective-merged.yml -Algorithm SHA256
~~~

The worker used main.yml as the current side, base.yml as the common
ancestor, and head-frozen.yml as the other side. The prospective merged
result was not written into the repository.

### Prospective result

| Check | Result |
| --- | --- |
| git merge-file exit | 0 |
| prospective_merge_exit | 0 |
| Conflict markers | None |
| Effective Windows custody invocation | True |
| Rust-job ordering | resource @921 -> Node 22 @1040 -> custody @1300 -> strict Clippy @1861 -> cargo test @1959 |
| Head/prospective unique npm run command set | Equal, 28 commands each |
| Prospective merged workflow Git blob | f92c0fd55c247611943a2236bdee3acaf8cb9c1e |
| Prospective merged workflow SHA-256 | c2880d42f126509c47e0fd34687a26019f181cd903e4ae3f7df402af9962ffd4 |

The prospective workflow retained these unique npm run commands:

~~~text
build
test:audio-viz
test:auth
test:backup-flow
test:callmd-contracts
test:callmd-history
test:callmd-integration
test:callmd-live
test:callmd-shell
test:ci-coverage
test:design-system
test:desktop-bootstrap
test:device-authority
test:device-reconcile
test:diarization
test:egress
test:external-tools
test:job-actions
test:local-api-client
test:mobile
test:native-session-custody
test:recovery
test:release
test:summary-scoping
test:traceability
test:transcribe-concat
test:w1-authority-schema
test:web-recordings
~~~

## Hosted gate and excluded evidence

Hosted verification is NOT_RUN in this worker report. There is no claim here
that GitHub Actions executed the repaired custody suite, that frontend/Rust
hosted jobs passed, or that the PR merged. Main/Terra must arrange fresh
hosted verification against the reviewed frozen source.

The parallel hosted-playback investigation is outside this lease and produced
no source change. Main-owned governance/RCA overlays and any shared .tmp
artifacts are outside this report's worker-owned path inventory and were not
edited or cleaned.

## Version diff

New -> 0.1.0b: added frozen workflow provenance, exact diff, local inventory
evidence, reproducible prospective-merge method/object hashes, and explicit
hosted NOT_RUN status.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-17 | beta | Frozen PR59 CI workflow verified locally and against an isolated prospective merge; hosted verification remains NOT_RUN. | UNCOMMITTED; workflow postimage 52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e | Codex |
