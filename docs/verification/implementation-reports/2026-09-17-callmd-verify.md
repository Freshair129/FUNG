---
version: "0.1.1b"
created_at: "2026-09-17T08:41:22+07:00,Codex VERIFY,UNCOMMITTED base 376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T08:49:00+07:00,Codex documentation-only metadata correction"
status: "beta"
superseded_by: null
base_sha: "376ef30db13670e4dea816ceff440f44ce73fffd"
branch: "codex/callmd-ui-dag"
workflow: "0.2.1b"
result: "DONE_WITH_CONCERNS"
attributes:
  domain: "desktop-product"
  doc_type: "verification-report"
  scope: "Read-only VERIFY of frozen INTEGRATE candidate; generated outputs and evidence only"
  complexity: "C-3"
  risk: "HIGH native/runtime acceptance boundary"
  code_writes: false
  source_snapshot: "frozen 35-artifact manifest"
  worker_agent_id: "01a0acf8-3fbe-7a20-9f96-b5b7b8be3141"
  worker_requested_model: "gpt-5.6-luna"
  worker_requested_reasoning_effort: "max"
---

# CallMD P1-B frozen integrated verification report

## Result and authority

**DONE_WITH_CONCERNS.** Read-only VERIFY was explicitly activated on the frozen
`INTEGRATE` handoff under workflow `0.2.1b`. This report is evidence delivery,
not self-acceptance and not a release decision.

- Repository root: `C:/Users/pc/.codex/worktrees/9000/fung`
- Base/HEAD: `376ef30db13670e4dea816ceff440f44ce73fffd`
- Branch: `codex/callmd-ui-dag`
- Source snapshot: the exact frozen 35-artifact manifest in this report;
  final readback was `35/35 MATCH`.
- Git: no commit, stage, merge, push, reset, checkout, deployment, or hosted-CI
  action was performed.
- Source: no source, package, lockfile, CI, schema, security, CSP, capability,
  or configuration file was changed by VERIFY.
- Authorized writes: this report, root `dist/`, the approved baseline Cargo
  target, task logs, and the empty CI prerequisite directories.

The integrated report remains `DONE_WITH_CONCERNS`; its mounted React fixture
observation was correctly attributed to the worker and was not treated as
controller evidence. Main later independently reproduced that component-only
fixture, recorded below. The current result does not waive the open native,
strict-Clippy, actual-Whisper, App-browser, hosted-CI, device, provider, or
production gates.

## Evidence vocabulary and current boundary

| Tier | Current result | What it proves | What it does not prove |
|---|---|---|---|
| Frozen source/hash | PASS, 35/35 | The integrated candidate bytes stayed unchanged | Correctness or runtime acceptance |
| Node package tests | PASS, 26 Node scripts; 195 pass, 1 skip | Existing and integrated deterministic/component contract assertions | Browser paint, full App flow, Tauri/native, device, hosted CI |
| Python concat test | PASS, 5/5 with bundled interpreter | The deterministic concat test under the permitted fallback | An actual Whisper runtime/model |
| Frontend build | PASS, `tsc` plus Vite; 1812 modules transformed | The frozen frontend compiles and produces `dist/` | Packaged Tauri cold boot or native behavior |
| Rust focused tests | PASS, recording/playback/intel/live/native behavioral filters | Local deterministic Rust contract and behavioral evidence | CPAL/device/audio or packaged native runtime |
| Rust full CI test | FAIL, 465 pass/6 fail/1 ignored | The exact failure boundary on the candidate | A source regression; failures are missing actual Whisper runtime gates |
| Strict CI Clippy | FAIL, untouched auth/backup warnings | The exact unresolved lint gate | Permission to edit or waive those files |
| Main real-React fixture | PASS, component-only; six visible PASS rows | Independent mounted `RecordingReview` registrar cleanup evidence | Full App flow, native, or Node CI execution |
| Main App browser/CUA | FAIL, three confirmed Live/Shell defects | Separate controller-owned browser evidence | Node/Rust pass or a reason to alter this frozen root |

All evidence below binds the current frozen integrated snapshot. The isolated
LiveFIX2 worktree is not in this root and does not alter these results. Any
future source transfer, including the LiveFIX2 correction, invalidates the
affected descendant evidence and requires a fresh scoped reverify.

## Environment and leases

| Item | Observed value / disposition |
|---|---|
| Node/npm | Node `24.19.0`, npm `11.17.0` |
| Rust/Cargo | rustc `1.98.0 (88d9e12ae 2026-08-18)`, cargo `1.98.0 (797e8a9bc 2026-08-05)` |
| Toolchain warning | rustup reported it could not canonicalize `C:\Users\pc`; version commands still exited 0 |
| Normal Python | `Get-Command python` returned unavailable |
| Python fallback | `C:\Users\pc\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe`, version `3.12.14` |
| Cargo target | `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\baseline-376ef30\src-tauri\target` only for authoritative Cargo runs |
| Root target | `src-tauri\target` absent at final cleanup; no root Cargo cache retained |
| Empty prerequisites | `.venv-whisper/` and `runtime/` were created empty as allowed by CI; no Whisper/model/runtime asset was installed or fabricated |
| Native/process access | No Tauri launch, CPAL/device/audio/provider/network/userdata/credential access; no competing Cargo/rustc/clippy process at final check |

## NPM test commands

The package and CI inventories contain the same 27 `test:*` scripts. Each
distinct registered script was run once for the totals below. Every command
exited 0. Counts are parsed from the command's saved output, not inferred from
the script name.

| Command | Runner | Exit | Tests | Pass | Fail | Skipped |
|---|---|---:|---:|---:|---:|---:|
| `npm run test:ci-coverage` | Node | 0 | 4 | 4 | 0 | 0 |
| `npm run test:callmd-contracts` | Node | 0 | 13 | 13 | 0 | 0 |
| `npm run test:callmd-shell` | Node | 0 | 6 | 6 | 0 | 0 |
| `npm run test:callmd-live` | Node | 0 | 11 | 11 | 0 | 0 |
| `npm run test:callmd-history` | Node | 0 | 11 | 11 | 0 | 0 |
| `npm run test:callmd-integration` | Node | 0 | 5 | 5 | 0 | 0 |
| `npm run test:mobile` | Node | 0 | 5 | 5 | 0 | 0 |
| `npm run test:design-system` | Node | 0 | 2 | 2 | 0 | 0 |
| `npm run test:auth` | Node | 0 | 8 | 8 | 0 | 0 |
| `npm run test:w1-authority-schema` | Node | 0 | 7 | 6 | 0 | 1 |
| `npm run test:backup-flow` | Node | 0 | 17 | 17 | 0 | 0 |
| `npm run test:device-reconcile` | Node | 0 | 6 | 6 | 0 | 0 |
| `npm run test:desktop-bootstrap` | Node | 0 | 10 | 10 | 0 | 0 |
| `npm run test:release` | Node | 0 | 7 | 7 | 0 | 0 |
| `npm run test:external-tools` | Node | 0 | 5 | 5 | 0 | 0 |
| `npm run test:traceability` | Node | 0 | 1 | 1 | 0 | 0 |
| `npm run test:recovery` | Node | 0 | 7 | 7 | 0 | 0 |
| `npm run test:job-actions` | Node | 0 | 17 | 17 | 0 | 0 |
| `npm run test:summary-scoping` | Node | 0 | 6 | 6 | 0 | 0 |
| `npm run test:diarization` | Node | 0 | 8 | 8 | 0 | 0 |
| `npm run test:egress` | Node | 0 | 8 | 8 | 0 | 0 |
| `npm run test:local-api-client` | Node | 0 | 8 | 8 | 0 | 0 |
| `npm run test:web-recordings` | Node | 0 | 4 | 4 | 0 | 0 |
| `npm run test:device-authority` | Node | 0 | 3 | 3 | 0 | 0 |
| `npm run test:audio-viz` | Node | 0 | 6 | 6 | 0 | 0 |
| `C:\Users\pc\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe tests/transcribeConcatOnly.test.py` | Bundled Python fallback | 0 | 5 | 5 | 0 | 0 |
| `npm run test:native-session-custody` | Node; scoped Cargo child | 0 | 11 | 11 | 0 | 0 |

NPM totals: **27 distinct registered scripts; 201 test cases, 200 passed,
0 failed, 1 skipped, 0 todo**. Node totals are 196 cases with 195 passed and
1 skipped. The five new integrated CallMD suites account for 46/46 passed
cases; the remaining Node suites are existing baseline/retained-surface tests.
The Python command differs from the registered script because the normal
`python` executable was unavailable; no installation was attempted.

### Custody command containment correction

The first batch wrapper accidentally invoked `npm run test:native-session-custody`
without the scoped Cargo environment. It exited 0 with 11/11, but it is
**excluded from authoritative evidence** because its child Cargo process wrote
an unscoped ignored root target. That exact generated target was removed after
the process exited. The command was rerun under the exclusive baseline target;
the row above is the authoritative 11/11 result. A later test helper created
four root `target/test-fixtures` binaries even with the scoped Cargo cache; those
four generated files were also removed. No source or user data was touched.

## Frontend build and static checks

| Command | Exit | Evidence |
|---|---:|---|
| `npm run build` | 0 | `tsc && vite build`; Vite `8.1.3`; 1812 modules transformed; root `dist/` generated |
| `git diff --check` | 0 | No whitespace errors in the frozen working-tree diff |

The build is local frontend evidence only. It is not packaged/native cold-boot,
browser, device, hosted-CI, or production evidence.

## Cargo commands and counts

All authoritative Cargo commands used the actual root manifest
`src-tauri/Cargo.toml` with only the scoped `CARGO_TARGET_DIR` shown above.

| Command | Exit | Actual result |
|---|---:|---|
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` | 0 | PASS; only the rustup canonicalization warning was observed |
| `cargo check --manifest-path src-tauri/Cargo.toml --lib` | 0 | PASS; existing dead-code warnings in `backup.rs` remained visible |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | 101 | FAIL on untouched known gates: `auth_session.rs:3384` unused `domain`; `backup.rs:28` unused `RESTORE_INTENT_TTL`; `backup.rs:95-99` unread fields; `backup.rs:110`, `114`, `137` unused methods |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 101 | 472 total: 465 passed, 6 failed, 1 ignored |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib recording_review::tests` | 0 | 6 passed, 0 failed, 466 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib desktop_playback::tests` | 0 | 20 passed, 0 failed, 452 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib meeting_intel::tests` | 0 | 18 passed, 0 failed, 454 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib live_meeting::tests` | 0 | 16 passed, 0 failed, 456 filtered |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib native_behavioral_` | 0 | 22 passed, 0 failed, 450 filtered |

The six full-suite failures are the known missing actual Python/Whisper runtime
gate, each reporting the empty disposable path
`C:\Users\pc\.codex\worktrees\9000\fung\.venv-whisper\Scripts\python.exe`:

- `fungwire_client::tests::delegate_transcription_completes_and_writes_transcript_over_loopback`
- `fungwire_client::tests::delegate_transcription_reconnects_after_early_drop_and_completes`
- `fungwire_client::tests::delegated_job_persists_the_requested_executor`
- `fungwire_server::tests::job_loop_reassembles_multi-subframe_chunk_and_returns_transcript`
- `fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes`
- `fungwire_server::tests::transcribing_progress_is_streamed_before_result`

These are environment-gated failures, not waived. No Python runtime, model,
Whisper package, assertion, gate, auth code, or backup code was changed.
The supplied known hashes remained exact:

- `src-tauri/src/auth_session.rs` — `ec31b2e8aff4d6c039b4f807875b72adb65805cea8aab1b6c28e3d69f184c018`
- `src-tauri/src/backup.rs` — `0f7b77400bf35c86511a37f426f0719e8e4d051ef76f9252a6283fb4c79d1cce`

## Independent main observations

These observations are separate from my Node/Rust command counts.

### Component-only mounted registrar fixture — independent PASS

Main independently ran the unchanged root integration fixture using real React
`RecordingReview`. CUA observed `FIXTURE_COMPLETE` with six visible PASS rows;
the final counters `list/release/get/transcript/summaries/exports` were each
`2`; retained-B-after-unmount reported `delta=0/0`; and console warnings/errors
were empty. The observed script hash was
`2da26221cbbcba6805377edba8ad2eb4a826a9afab740e70f460b8be32758ddd`.

This closes the component-only mounted-history registrar proof independently,
subject to source-oracle audit. It is not the full `App` flow, not Node CI
execution, not native Tauri, not device/audio, and not production evidence.

### Main App browser/CUA — FAIL, separate gate

Main's independent frozen-App browser run remains a separate `FAIL` with three
confirmed Live/Shell defects. The initial observation included Home reporting
native unavailable while the capture strip reported `preparing/active`, and a
hidden Live overlay whose computed display remained `flex`. The controller owns
the RCA/dispatch under
`.brain/rca/2026-09-17-callmd-integrated-live-lifecycle.md` and has isolated
LiveFIX2/Shell fixes running outside this root. VERIFY performed no browser,
CUA, native-app, or source-fix action and does not convert this failure into a
Node/Rust result.

## Open gates and disposition

- **Open:** strict CI Clippy on untouched `auth_session.rs` and `backup.rs`.
- **Open:** six FUNGWIRE tests requiring the actual Python/Whisper runtime.
- **Open:** Main App browser/CUA Live/Shell FAIL3 and its future isolated fix/reverify.
- **NOT_RUN:** Tauri/native cold boot, real capture/reopen, CPAL output, physical
  device, microphone/system audio, real provider, network request, userdata,
  credentials, packaged runtime, hosted CI, and production.
- **Not waived:** no source fix, lint suppression, assertion weakening, test
  removal, dependency install, model/runtime fabrication, or stale evidence reuse.
- **DoD:** full implementation/acceptance DoD is not met while these gates are
  open. This report must proceed to the designated independent integration
  reviewer; it is not final acceptance.

## Frozen 35-artifact SHA-256 manifest

Final readback after all authorized tests: **35/35 match**.

| Path | SHA-256 |
|---|---|
| `.github/workflows/ci.yml` | `3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a` |
| `package.json` | `1d00347d00ab805c28bd8887d716d6af2a602255763f257cc92d0be4daf75b2d` |
| `tests/ciCoverage.test.mjs` | `2839a68b71308e164562a8f133abed45948636b353c4ea69f91cec2b68f11197` |
| `tests/nativeSessionCustody.test.mjs` | `fe8af82184b1729abf48bae7c8883708def93817e98026701a66407db6a4d8c4` |
| `docs/verification/implementation-reports/2026-09-17-callmd-baseline.md` | `c4ac4fd88e935bc7a261bc67de7bfcb2b69cdecf225f9297156edf506bff7289` |
| `tests/callmdDesktopContracts.test.mjs` | `d6d53a7531e24492ce2ceddf76e58def018602a0cdca33754c1f8bf3e2369ee6` |
| `docs/verification/implementation-reports/2026-09-17-callmd-contract-tests.md` | `e067b84d2b38540efd22d17898c671450306ffa1f305b7c2053a6e819ee84efd` |
| `src/tauri.ts` | `816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe` |
| `src/components/desktop/contracts.ts` | `7f2bd4191f804fca5788684a5891471fda70d713110c5e75b3b5f67870cbae81` |
| `docs/verification/implementation-reports/2026-09-17-callmd-shared-contract.md` | `f8cb9c003d931868229d3ac5d07fbfb9234c403563337b35aa6458744cdbf229` |
| `src/components/desktop/DesktopShell.tsx` | `d9f69e2951b88d8eb51572656695d8c5c38be4d47ca3f2bb8d478452e4ff72f0` |
| `src/components/desktop/DesktopShell.css` | `c2de7032b06e92ca8126f1b680c5a26686108cf5c6363587805e9e3f3c911d7e` |
| `tests/callmdDesktopShell.test.mjs` | `d66520ab6fbb4118958bca5bc460e0ba5a46c585b01f3dbed2a8c9b702baf158` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-shell.md` | `42c95b657c5357f7efb868e3277e4db4555ef3540cd9a0b269316b937bdc3a3c` |
| `src/components/LiveMeetingPanel.tsx` | `a2a72af87552a9dfa2366de21b3b1f2c5e80b8fd257826457aed1aa3d912d7ca` |
| `src/components/LiveMeetingPanel.css` | `5d929f202a2e5bd929e8fb4cc74742ce658e612681046b954b160b95f38249a4` |
| `src/components/desktop/LiveWorkspace.tsx` | `9a46ffa5694ebea88efbf72fcdee27d26e40de960e55143a641489963547c48c` |
| `src/components/desktop/LiveWorkspace.css` | `a629bf1b2bd5c84539adeb3d34d225930826da9c91293d5cc53fefa8dfe813bd` |
| `tests/callmdLiveWorkspace.test.mjs` | `7a2881f7f4afecb275f44a07e3b3f30d796ed203bb682ac4e761eb004abdbca1` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | `4a4f1e92602cd9ba2d4054b830ef80567449468a6eedf3c6f49eb42f46530bd5` |
| `src/components/desktop/RecordingReview.tsx` | `8e391a68235b0be710ed3e152d63380d16dc25a570c55bde8a7e647f4265537d` |
| `src/components/desktop/RecordingReview.css` | `19c92aed68dc112feb142ccf346278684c1ac5f9d95a5e13ad6c85d279caee73` |
| `tests/callmdRecordingReview.test.mjs` | `9c8963cc56febb26786e30f21929ffdb59cf29d8b85cfb99b55129d7ea1483d3` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-history.md` | `eea3dec8901c95a896eb61d027f7e14721261506da56bbbca8487028452bf229` |
| `src-tauri/src/recording_review.rs` | `460e68324427ac358627ac3c6c18e468ad27d668385ef2e3317b49622df3e43e` |
| `src-tauri/src/desktop_playback.rs` | `b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae` |
| `src-tauri/src/lib.rs` | `2ebb0abcfef27cc5c600ca73b38af6051727c74613a91f974eacf562dc8de7b5` |
| `src-tauri/src/meeting_intel.rs` | `cc87422b81a29fdb32b3af18b58e37799b26d7a22617c6f765cae3481a388477` |
| `src-tauri/src/live_meeting.rs` | `66e85d914ca468a140f8cfd56d30072ccbc8cbd808b0b3560aa359075f5677c5` |
| `docs/verification/implementation-reports/2026-09-17-callmd-backend-recording.md` | `d7b5495d205ec0437ed21564f49a850981b8c8cdfa59d05015c171cddd97d078` |
| `src/App.tsx` | `c23c879088dab4656bbd7c5f639c953839d2a2a3798591d71f610bd7314a7716` |
| `src/styles.css` | `2e54342531d62fb5d1b920d975df9880c27e76e525df219176c96cf4ce4fc14b` |
| `src/components/InstrumentRail.tsx` | `312de9bfc874df16b76442ffcee71f7e98fd97400e358ce80ea25384439e05df` |
| `tests/callmdDesktopIntegration.test.mjs` | `2da26221cbbcba6805377edba8ad2eb4a826a9afab740e70f460b8be32758ddd` |
| `docs/verification/implementation-reports/2026-09-17-callmd-integrate.md` | `7e5d9c099c71a1ff2df5e379c765ce5b478b7c893bc74783cc3065d53d9c2ed3` |

Machine-readable task logs are under
`output/callmd-verify/tasklogs/`, including `npm-summary.json`,
`npm-test-counts.json`, `cargo/cargo-summary.json`, and
`frozen35-final-hash-manifest.txt`.

## Version diff and changelog

- `0.1.0b -> 0.1.1b`: controller corrects lifecycle status to the document
  schema's beta and records the actual worker dispatch ID/requested model.
  The separate result remains DONE_WITH_CONCERNS; counts/source hashes unchanged.

- `new -> 0.1.0b`: activated the workflow `0.2.1b` read-only VERIFY exception
  on the frozen integrated handoff; recorded 27 npm scripts, frontend build,
  exact Cargo gates, focused counts, independent fixture/browser boundaries,
  generated-output custody, and the final 35-hash readback.
- Product version remains `0.1.1`.
- No source, dependency, lockfile, schema, security, CSP, capability, commit,
  or release change was made.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Normalize metadata only; preserve worker result and evidence | UNCOMMITTED; base 376ef30 | Codex recorder |
| 0.1.0b | 2026-09-17 | beta | Read-only frozen integrated verification; all evidence and open gates retained | UNCOMMITTED; base 376ef30 | Luna VERIFY |
