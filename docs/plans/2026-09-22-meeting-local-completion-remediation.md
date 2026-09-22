---
version: "0.4.0b"
created_at: "2026-09-22T04:55:31.6822893+07:00, Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373, b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-22T06:11:42.3707662+07:00, Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373"
status: "candidate"
superseded_by: null
attributes:
  doc_type: "implementation-plan"
  domain: "meeting-intelligence"
  scope: "local fake-transcription completion and separately reviewed runner/CI portability candidate"
  risk: "C3/HIGH"
  actual_identity: "Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  base_sha: "b336f33ec400a38f003a0665c121069a87a543ac"
  approval_boundary: "User-approved only for the bounded persistent test runner, Rust CI test step, and this evidence update; provider, source, downstream, and release work remain unapproved"
---

# Local completion remediation plan — fake-transcription runtime

## 1. Decision and boundary

This is a candidate plan for the verified local test-environment issue.
The immediate no-source process-local PATH remedy is within the current
Fix it all dispatch. It is the only remediation supported by the confirmed
RCA and it does not change production behavior.

The user explicitly approved the bounded persistent runner and Rust CI test
step after the completed local runtime G1. This implementation is limited to
those test-only paths and this evidence update. Portable integration beyond
that slice, source seams, provider work, downstream feature work, and release
work remain unapproved under AGENTS R5. The approved Whisper model
specification remains authoritative: turbo default, medium for lower-resource
hardware, and large-v3 qualification-only.

Primary evidence: .brain/rca/2026-09-22-meeting-full-suite-runtime.md.
Current bounded acceptance:
docs/verification/implementation-reports/2026-09-22-meeting-intelligence-g1-contract-r3.md.
Downstream gates and portability/provider facts: refer to the candidate gate
inventory at
docs/verification/implementation-reports/2026-09-22-meeting-remaining-gates-audit.md,
now frozen v0.1.6b, SHA256
677E47117C6B0F8575CA6365BF267EA3F2090C87619A8ECE085C5052DE8814B1,
REPORT_LEASE_RELEASED, Socrates CLOSED. It is inventory, not an acceptance
gate, and is not duplicated here.

## 2. Current verified state

| Concern | Current status | Boundary |
|---|---|---|
| Six fake/local historical tests with ordinary PATH | VERIFIED — FAILED | Exact selections, exit 101, 503 filtered; see RCA chronology |
| Same six with ROOT .venv-whisper\Scripts prepended only in-process | VERIFIED — PASS | Exact selections, exit 0, 503 filtered |
| Complete frozen R3 default Rust LIB suite | VERIFIED — PASS | 503 passed, 0 failed, 1 ignored, exit 0; no --ignored |
| Existing interpreter | VERIFIED | Python 3.11.9, SHA256 5F7B89A612C9B8AF1D6456CDFCD1DBE5CA630849E79AEBCED9BEE9A6694952EC |
| Production bundled Python/model readiness | NOT_RUN | Preserved; not inferred from fake tests |
| Real provider/Meet/account/gateway/external send | NOT_RUN | No authorization and out of scope |
| CI/portable integration acceptance | NOT_RUN — CANDIDATE | Requires explicit review and an available CI-owned interpreter |
| Local N5–N7 | NOT_RUN here; not portability-blocked | Separate feature work; no acceptance inferred |
| N8–N15 and Terra G2 | NOT_RUN — OUT OF SCOPE | Follow the frozen candidate gate inventory and hard sequence |

The frozen R3 worktree was already dirty. The source/hash ledger and zero
Cargo/rustc process release are recorded in the RCA. A fresh verifier may
test the already-frozen original R3 source and original G1/author reports now;
it reviews these new documents only after their final hashes freeze.

### Snapshot custody clarification — future integration only

Socrates's eight-row R3 table is a **review-set anchor list only**. It is not
the complete 51-file overlay and is not a substitute for the approved
snapshot. This plan does not reconstruct, copy, or broaden that inventory.

Before any future fresh integration, the integrator must materialize and
verify the complete approved path/hash snapshot: the 49-path inventory in
the frozen R2 execution JSON field `snapshot_transfer.transfer_inventory`,
plus `src-tauri/src/auth_session.rs` and the frozen-R2 identity-custody
report. It must hash the actual current frozen-R3 bytes with the four
approved R3 deltas, never old pre-edit R2 hashes, and produce a complete
manifest before copying or downstream partitioning. HEAD-only or
eight-files-only reconstruction is invalid.

Authority: G1 report lines 71–79 at
`docs/verification/implementation-reports/2026-09-22-meeting-intelligence-g1-contract-r3.md:71-79`;
author/execution evidence is the frozen R3 MD/JSON pair at
`docs/verification/implementation-reports/2026-09-22-meeting-remediation-r3-execution.md`
and
`docs/verification/implementation-reports/2026-09-22-meeting-remediation-r3-execution.json`,
with the R2 inventory and identity report at
`docs/verification/implementation-reports/2026-09-21-meeting-remediation-r2-execution.json`
and
`docs/verification/implementation-reports/2026-09-21-meeting-identity-custody-r2.md`.
This is a future integration gate, not current local regression work.

## 3. Immediate no-source local/G1 recipe

This is the actionable remedy now. It is process-local and disposable; it
does not write PATH or configuration and it does not create .venv-whisper
artifacts.

~~~powershell
$env:CARGO_TARGET_DIR='C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:TAURI_CONFIG='{"bundle":{"resources":[]}}'
$env:PATH='C:\Users\pc\workspace\fung\.venv-whisper\Scripts;' + $env:PATH

# Read-only preflight for the already-existing interpreter.
& where.exe python
& python --version

# One exact historical filter; repeat for the six names in the RCA.
& cargo test --quiet --manifest-path 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml' --offline --locked --lib '<exact-qualified-test>' -- --exact --nocapture --color never

# Complete default LIB regression after all six exact filters are green.
& cargo test --quiet --manifest-path 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml' --offline --locked --lib -- --color never
~~~

Required evidence for each replay:

1. Exact filter, source checkout, manifest, target, and ICT start/finish.
2. Actual selected/passed/failed/ignored/filtered counts and Cargo exit.
3. where python, python --version, and interpreter SHA256.
4. Relevant frozen source, fixture, manifest, and lock hashes.
5. No --ignored, no provider/accuracy claim, and a final zero-process check.

If the interpreter is absent, stop as NOT_RUN. Do not copy an interpreter,
create a shim or symlink, alter global/user PATH, install/download packages,
weaken assertions, unpin models, or auto-bundle large-v3.

## 4. Ordered execution gates

### Gate A — completed RCA evidence

The RCA has confirmed the resolver chain, six red/green exact filters, full
LIB 503/0/1 result, source hashes, and zero-process Cargo release.

### Gate B — independent G1 replay

G1 may test the already-frozen original R3 source plus the original G1/author
reports while these documents are being finalized. Do not retroactively block
those tests on this document freeze. G1 reviews these two new documents only
after their final hashes are frozen. The red/green result remains local and
must not become provider, accuracy, production, or integration evidence.

Minimum independent acceptance is:

- One representative original-PATH negative control, the same test with the
  corrected process-local PATH, and the complete default LIB suite.
- Explicit membership evidence that all six historical names are included in
  the complete suite. Six separate corrected filters are optional, not
  required when complete-suite membership covers all six.
- Default LIB: 503 passed, 0 failed, 1 ignored; preserve source/interpreter
  hashes and confirm zero cargo/rustc processes.

### Gate C — approved bounded runner/CI implementation

The exact R3/target command in section 3 remains the completed local replay
recipe. The user approved the smallest bounded implementation on 2026-09-22:

Leased paths: scripts/meeting-intelligence-test-runtime.ps1 and, only for the
test step, .github/workflows/ci.yml. The script accepts explicit manifest,
target, and interpreter paths; preflight them; set process-local variables;
and restore those variables on exit. It must not hardcode the current R3
absolute Temp checkout or target, copy/symlink/shim, alter global PATH,
install/download, or affect production packaging.

CI uses setup-python 3.12 on the Windows Rust job and does not use the current
Temp paths as CI inputs. No source changes are indicated.

Any source seam needs separate path/contract approval, must be test-only, and
must be impossible for production builds to honor. No current source change
is indicated.

### Gate C implementation record — 2026-09-22

- `scripts/meeting-intelligence-test-runtime.ps1` is new. It requires
  `-ManifestPath`, `-TargetDir`, and `-PythonPath`; validates the manifest,
  target directory, Python file, and Cargo executable; creates only a missing
  target directory; prepends only the Python parent directory to the
  process-local PATH; sets the five approved process-local values; restores
  all six touched process variables in `finally`; and returns Cargo's exit
  code. It invokes only:
  `cargo test --quiet --manifest-path <ManifestPath> --offline --locked --lib
  -- --color never`.
- `.github/workflows/ci.yml` changes only the Windows Rust job's final library
  test step: `actions/setup-python@v5` with the existing pinned `3.12` line,
  `python-path` output capture with a checked `python` fallback, and one
  PowerShell invocation of the runner using `${{ github.workspace }}` and
  `${{ runner.temp }}`. The existing `.venv-whisper` and `runtime` directory
  setup needed by Tauri build scripts remains unchanged.
- This plan file records the approval, implementation, and evidence boundary.

Local implementation verification (not CI) ran once, serially, on the frozen
R3 manifest with the existing ROOT interpreter:

~~~powershell
& .\scripts\meeting-intelligence-test-runtime.ps1 `
  -ManifestPath 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml' `
  -TargetDir 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target' `
  -PythonPath 'C:\Users\pc\workspace\fung\.venv-whisper\Scripts\python.exe'
~~~

ICT `06:10:08.7257557`–`06:10:30.8814771`; Python `3.11.9`, SHA256
`5F7B89A612C9B8AF1D6456CDFCD1DBE5CA630849E79AEBCED9BEE9A6694952EC`; runner
exit `0`; Cargo reported `504 selected, 503 passed, 0 failed, 1 ignored, 0
measured, 0 filtered`; no `--ignored` flag was used. The wrapper confirmed
all process-local environment values were restored and no Cargo/rustc process
remained. This is local fake/regression evidence only; it is not provider,
model accuracy, CI, packaged, integration, or production evidence.

The root source/manifest/lock files were not edited by this implementation.
The R3 and frozen-engine checkouts remain pre-dirty with their prior status;
the run used only the approved target directory and made no source, fixture,
model, report, RCA, audit, install, download, or cleanup change. CI execution
is **NOT_RUN**. Independent Luna verification and the Terra final gate remain
open.

### Gate D — integration and downstream work

This plan does not authorize package.json integration, real Meet/provider
work, account/gateway setup, external sends, or downstream feature scope.
Local N5–N7 are not PATH-blocked but remain unrun; N13 → N14 → N15 remains
hard-sequenced and Terra integrated G2 is not eligible. The frozen Socrates
report is candidate inventory, not acceptance evidence.

## 5. Risk, rollback, and exit criteria

- Local PATH remedy: low mutation risk, but bounded to fake/regression
  behavior. Persistent runner/CI remains C3/HIGH, test-only, and candidate
  until CI and independent review complete.
- Rollback is limited to reverting the three leased paths before merge; the
  runner restores process-local settings and creates no Python/runtime
  artifact. No global setting or production behavior changed.
- Exit for this worker: approved implementation is present, parser/local
  verification is recorded, exact evidence labels are retained, CI is
  explicitly NOT_RUN, and no unapproved source/provider/release work started.

## Version diff

- 0.3.1b -> 0.4.0b: recorded explicit approval and implemented the bounded
  parameterized test runner plus the Windows Rust CI test step; recorded
  PowerShell parse/local `503 passed / 0 failed / 1 ignored` evidence while
  preserving the no-source/provider/production boundary and CI `NOT_RUN`.
- 0.3.1b: corrected independent-G1 timing and minimum acceptance, separated
  local replay from a parameterized portable runner, and retained the frozen
  audit inventory/custody and no-source PATH boundary.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.4.0b | 2026-09-22 | candidate | Implemented the explicitly approved process-local test runner and Windows Rust CI invocation; local parser/full-LIB evidence passed, CI and production remain NOT_RUN. | working-tree; base b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.1.0b | 2026-09-22 | candidate | Initial actionable local completion remediation plan with deferred persistent integration scope | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.2.0b | 2026-09-22 | candidate | Clarified review-set anchors versus complete 51-file snapshot custody; retained deferred persistent integration scope | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.3.0b | 2026-09-22 | candidate | Recorded frozen candidate gate inventory and separated unrun N5–N7 from portability; retained snapshot custody and deferred integration scope | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.3.1b | 2026-09-22 | candidate | Corrected G1 timing/acceptance and separated local replay from portable runner parameters | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
