---
version: "0.2.1b"
created_at: "2026-09-22T04:55:31.6822893+07:00, Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373, b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-22T05:09:59.4445454+07:00, Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373"
status: "candidate"
superseded_by: null
attributes:
  doc_type: "rca"
  domain: "meeting-intelligence"
  scope: "R3 fake/local transcription runtime resolution and bounded full-LIB regression"
  risk: "C3/HIGH"
  actual_identity: "Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  base_sha: "b336f33ec400a38f003a0665c121069a87a543ac"
  evidence_status: "VERIFIED local bounded regression; provider, CI, integration, G2, and production status NOT_RUN"
---

# RCA — R3 fake-transcription runtime resolution

## Confirmed result

The six historical fake/local failures have one confirmed cause: the test
process could not resolve Python. They are not six independent FUNGWIRE or
production-transcription defects.

The frozen cfg(test) resolver probes Windows where python/python3/py, then
falls back to the R3 checkout's .venv-whisper\Scripts\python.exe. Ordinary
PATH had no Python command; that R3-local fallback was absent. Prepending the
existing ROOT venv directory to PATH in one process made unchanged source pass.

This is local fixture/regression evidence only. It is not provider, accuracy,
packaging, CI, integration, G2, or production evidence.

## Scope and custody

- Agent Maxwell /
  01a0c5e7-6c8d-7f21-b6d3-c25b264bf373; model gpt-5.6-luna, max.
- Root and frozen R3 HEAD:
  b336f33ec400a38f003a0665c121069a87a543ac.
- R3:
  C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922.
- Engine:
  C:\Users\pc\AppData\Local\Temp\codex-fung-genesis-schema-limit-r1-20260921,
  HEAD 79b41a3f4ae4026d086b634c631f4f4a7ccbd142.
- Sole target:
  C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target.
- Every run used the R3 manifest, --offline --locked, one Cargo slot, and the
  five required process-local Cargo values. The R3 worktree was pre-dirty;
  this agent made no source/runtime/fixture/CI/manifest/lock edits.
- Socrates audit:
  docs/verification/implementation-reports/2026-09-22-meeting-remaining-gates-audit.md,
  frozen v0.1.6b, SHA256
  677E47117C6B0F8575CA6365BF267EA3F2090C87619A8ECE085C5052DE8814B1,
  REPORT_LEASE_RELEASED, Socrates CLOSED. It is candidate gate inventory,
  not an acceptance gate.

## Symptom and exact six

Historical R1 recorded 491 total, 484 passed, 6 failed, 1 ignored, exit 101.
The exact six names are:

1. fungwire_server::tests::job_loop_reassembles_multi_subframe_chunk_and_returns_transcript
2. fungwire_client::tests::delegate_transcription_completes_and_writes_transcript_over_loopback
3. fungwire_server::tests::transcribing_progress_is_streamed_before_result
4. fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes
5. fungwire_client::tests::delegate_transcription_reconnects_after_early_drop_and_completes
6. fungwire_client::tests::delegated_job_persists_the_requested_executor

All use local loopback state and the dependency-free
src-tauri/tests/fixtures/fake_transcribe.py; no real capture/provider is used.

## Source-backed root cause

1. cfg(test) WhisperRuntime::for_test resolves the interpreter instead of
   trusting its supplied path.
2. Windows resolution checks where python, python3, and py, excluding
   WindowsApps aliases.
3. With ordinary PATH, resolution falls through to the absent R3-local
   .venv-whisper\Scripts\python.exe.
4. run_python_worker then emits the exact missing-runtime error, causing
   server transcribe_failed and client failed versus completed.
5. With only ROOT .venv-whisper\Scripts prepended, where python resolves the
   existing interpreter and the fake subprocess completes.
6. Production bundled-runtime lookup and fail-closed model behavior are
   unchanged; no model-profile or FUNGWIRE change is indicated.

## Interpreter and command evidence

- Interpreter:
  C:\Users\pc\workspace\fung\.venv-whisper\Scripts\python.exe
- Version: Python 3.11.9; size 103192 bytes.
- SHA256:
  5F7B89A612C9B8AF1D6456CDFCD1DBE5CA630849E79AEBCED9BEE9A6694952EC.
- Ordinary shell: where.exe existed, but where python/python3/py found none.
- Corrected shell: only the above Scripts directory was prepended process-local.
- No global PATH/config, copy, symlink, shim, download, install, or runtime
  mutation occurred.

Fresh PowerShell runs set these five values:

~~~powershell
$env:CARGO_TARGET_DIR='C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:TAURI_CONFIG='{"bundle":{"resources":[]}}'
~~~

Baseline left PATH unchanged. Corrected runs added:

~~~powershell
$env:PATH='C:\Users\pc\workspace\fung\.venv-whisper\Scripts;' + $env:PATH
~~~

Then each exact filter used:

~~~powershell
& cargo test --quiet --manifest-path 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml' --offline --locked --lib '<exact-qualified-test>' -- --exact --nocapture --color never
~~~

## Controlled chronology and counts

Two exploratory corrected-PATH greens (exec-92a8a2cd... and
exec-127cb652...) preceded the controlled baseline and are not counted.

| # | Filter | Original PATH | Corrected PATH |
|---:|---|---|---|
| 1 | server reassembly | 04:45:47.2891749–04:45:48.0820393; exit 101; 0/1/503 | 04:48:52.4143903–04:48:54.7065153; exit 0; 1/0/503 |
| 2 | client loopback completion | 04:47:47.3261493–04:47:49.6031247; exit 101; 0/1/503 | 04:49:05.4287635–04:49:09.5137631; exit 0; 1/0/503 |
| 3 | server progress | 04:47:58.4633123–04:47:59.2453686; exit 101; 0/1/503 | 04:49:19.4128600–04:49:21.4967140; exit 0; 1/0/503 |
| 4 | server resume | 04:48:08.5082897–04:48:09.4388360; exit 101; 0/1/503 | 04:49:33.2226251–04:49:36.4269451; exit 0; 1/0/503 |
| 5 | client reconnect | 04:48:19.7281449–04:48:22.4420150; exit 101; 0/1/503 | 04:49:46.3891035–04:49:50.3644151; exit 0; 1/0/503 |
| 6 | client executor persistence | 04:48:31.0049856–04:48:35.9145328; exit 101; 0/1/503 | 04:50:00.2520027–04:50:06.3810251; exit 0; 1/0/503 |

Here 0/1/503 means 0 passed, 1 failed, 503 filtered; 1/0/503 means
1 passed, 0 failed, 503 filtered. All corrected runs resolved the same
interpreter above.

## Full LIB result

After local/fake-boundary inspection, the complete default R3 LIB suite ran
once with corrected PATH and the same five values:

~~~powershell
& cargo test --quiet --manifest-path 'C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml' --offline --locked --lib -- --color never
~~~

ICT 04:51:32.5244208–04:51:49.1482104; exit 0:
504 selected, 503 passed, 0 failed, 1 ignored, 0 measured, 0 filtered.
The ignored test is the explicit real-runtime upload test; --ignored was not
used. This remains local evidence, not accuracy/provider/production evidence.

## Why the issue escaped detection

Observed evidence:

- Focused N4 G1 did not exercise the complete fake/FUNGWIRE cohort; prior
  missing-interpreter failures were recorded outside that bounded acceptance.
- The isolated R3 checkout had no local venv, while the sanitized shell had
  where.exe but no python/python3/py command.
- The historical aggregate lacked an interpreter preflight and a red/green
  PATH comparison.

Inferred prevention, not additional test evidence: require interpreter
path/version/hash, PATH scope, and selected counts as runner preflight fields;
stop as NOT_RUN when the interpreter is absent. This does not authorize
source, production, provider, or CI changes.

## Hash ledger and release

The hash-table paths below are R3-relative paths, not ROOT paths. The R3
checkout was pre-dirty; these are observed frozen-R3 bytes, not a clean-tree
claim.

| Path | SHA256 |
|---|---|
| src-tauri/src/lib.rs | 9948C2160DA422A406C5CF3F5B719E18F6C8F105ABC857E06407DFD3F3F7316F |
| src-tauri/src/fungwire_client.rs | 050A134F2FBA802E9152ADA49A7EF877F81C62BBBC05B679B6BE27EAD2623717 |
| src-tauri/src/fungwire_server.rs | C38D12D4591A21404C403E86D83088FFC8AC09D3FEFC0FE63D2153482B49DDB0 |
| src-tauri/tests/fixtures/fake_transcribe.py | 14AF0F9947B92B6BD3A7576E058C38CBD191267428147AD6B2BEF95DD0137342 |
| src-tauri/src/auth_session.rs | 55F2C89772B88A7ED9D045FBD2D0B5751448E0D779E8DE40618BDC9B3B8545BF |
| src-tauri/src/genesis_adapter.rs | 07BAF498CFE96AB7AA9863823D92C16BE7C57D93410D17AB9BF55DFBE1348C05 |
| src-tauri/Cargo.toml | ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E |
| src-tauri/Cargo.lock | 2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E |
| contracts/meeting-intelligence-v1.yaml | 31FCFDC0A444EE528AB719521BB22B9E7900542B23B7A13617C7CF67BF80BDD6 |
| tests/meetingIntelligenceContract.test.mjs | ABE6EC9DC212B1B01C7E98DBF21701A63418103B8643C54EB0BDAAF8242BCAEE |

Final check: CARGO_RUSTC_PROCESSES=0. Cargo slot released. The checkout
remained pre-dirty; this is not a clean-tree claim.

## Prevention and status

Immediate remedy: preflight existing interpreter path/version/hash, prepend
its directory only for Cargo, set all five values, run exact six, then default
LIB without --ignored; stop NOT_RUN if absent. Never copy/shim, weaken
assertions, unpin models, auto-bundle large-v3, or change global settings.

A persistent runner/CI or portable source seam is candidate-only and needs
user review under AGENTS R5; production must not honor a fake/test override.
No new source changes implemented; real provider/CI/integration/G2 remain
NOT_RUN. Local N5–N7 are not blocked by this portability issue but are
separate unrun feature work.
For complete-snapshot custody versus eight review anchors, use G1 report
lines 71–79 and the frozen Socrates audit; do not reconstruct it here.

## Version diff

0.2.1b adds the explicit escape-detection evidence/inference split,
R3-relative hash clarification, and final source/provider status wording.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-22 | candidate | Initial confirmed R3 runtime-resolution RCA and bounded local evidence | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.2.0b | 2026-09-22 | candidate | Confirmed PATH RCA with exact six-test and full-LIB evidence; frozen audit reference | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
| 0.2.1b | 2026-09-22 | candidate | Clarified escape evidence versus inferred prevention, R3-relative hashes, and final status wording | b336f33ec400a38f003a0665c121069a87a543ac | Maxwell / 01a0c5e7-6c8d-7f21-b6d3-c25b264bf373 |
