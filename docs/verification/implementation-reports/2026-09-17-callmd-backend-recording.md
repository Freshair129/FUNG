---
version: "0.1.3b"
created_at: "2026-09-17T07:30:06+07:00,gpt-5.6-luna/max BACKEND FIX3 implementer,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T07:30:06+07:00,gpt-5.6-luna/max BACKEND FIX3 implementer"
status: "beta"
superseded_by: null
base_sha: "376ef30db13670e4dea816ceff440f44ce73fffd"
attributes:
  domain: "FUNG desktop"
  doc_type: "implementation-report"
  scope: "Approved P1-B BACKEND_RECORDING FIX3 evidence correction; no new feature"
  risk: "MEDIUM minimal production-used async adapter and deterministic test seams"
  cycle: "FIX3 of maximum 3 unsuccessful FIX cycles; initial review not counted"
---

# CallMD backend recording FIX3 implementation report

## Result and authority

FIX3 closes the two bounded regression-evidence gaps identified by the two
independent FIX2 reviewers. It preserves the existing native playback/Q&A
behavior and adds only deterministic evidence paths; this is an uncommitted
implementation handoff, not a native or production acceptance claim.

- Exact cwd/worktree: C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30.
- Git root: C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30.
- HEAD/base: 376ef30db13670e4dea816ceff440f44ce73fffd.
- Implementer: gpt-5.6-luna/max BACKEND FIX3 implementer.
- Parent-authority contracts SHA-256: 6f2b55293430048bfff08cc7b5914c06d85823c37a737d442fdb346f0ea55921.
- Parent-authority acceptance v0.1.3b SHA-256: 8686bf25f8aa9accf170e4d41409dab777246178e942178dfee7ee656a0e613c.
- The acceptance hash above is the original receipt; the dfee sequence is
  intentional and the prior 65-character transcription was not copied.
- FIX3 write scope was exactly these three paths:
  src-tauri/src/desktop_playback.rs,
  src-tauri/src/meeting_intel.rs, and this report.
- The frozen prior inputs were preserved: recording_review.rs, lib.rs, and
  live_meeting.rs remain at their supplied hashes below. Existing unrelated
  dirty files were not edited.
- No commit, stage, merge, push, release, install, dependency, lock, schema,
  auth, backup, config, CSP, capability, UI, shared, or CI change was made.
  No real app/device/audio/provider/network/userdata/credential action was
  taken. The orchestrator did not edit source.

## Closed FIX3 findings

### A — checked recording Q&A has a real no-provider-call regression

The existing RecordingLlmStub remains the provider-call seam used by the
production checked pipeline. The new test
recording_qa_checked_pipeline_skips_provider_without_matching_evidence
calls literal meeting_ask_recording_checked with a valid project/recording
pair twice: once with no transcript rows and once with a nonmatching transcript
row. Both results assert status=insufficient_evidence, empty answer/sources,
model=None, and zero captured provider calls. The adversarial production-path
capture test remains retained and still verifies the exact pair-scoped
INPUT_JSON body, source IDs, and exclusion of same-project, foreign-project,
graph, and live-tail bait.

The two stub-backed tests now use a test-only mutex because the original global
stub registry was not parallel-test safe. This changes no production behavior.

### B — delayed playback tests use the actual owned async orchestration

OpenPlaybackIo is the minimal production-used seam: production commands
provide the real prepare_playback and spawn_worker; tests provide only a
prepared payload and deterministic worker/readiness I/O. The production
MAX_OPEN_TIMEOUT remains 10 seconds. Tests may pass a short budget through
the task solely to keep deadline evidence bounded.

dispatch_open_playback is extracted from the public command and calls the
literal production open_playback_owned inside spawn_blocking. The public
desktop_playback_open command calls that adapter unchanged. The matching
dispatch_close_playback adapter contains the same async cleanup body used by
desktop_playback_close; it joins the worker before releasing the native
admission guard and marking the owner-bound handle closed.

The four new production-path tests cover delayed preparation/readiness, total
deadline expiry, destroy during open with late readiness, and active delayed
close. They assert dispatch remains responsive while work is pending, no late
install occurs after disposal, close does not acknowledge or release admission
before worker quiescence, and eventual owned cleanup joins and releases.
No CPAL device, WebviewWindow, app launch, or real audio is used.

## Literal test-to-production call map

| Test evidence | Actual production call chain | Required oracle |
|---|---|---|
| desktop_playback::tests::production_open_dispatch_runs_delayed_prepare_and_readiness_without_cpal at desktop_playback.rs:2863 | spawn_open_dispatch -> dispatch_open_playback -> spawn_blocking -> open_playback_owned -> injected prep/worker I/O; public desktop_playback_open at :1957 calls the same dispatch_open_playback | Pending during prep/readiness, then owned install and successful close |
| production_open_dispatch_total_deadline_reaps_delayed_preparation at :2917 | Same literal dispatch_open_playback -> open_playback_owned; short test budget only | PLAYBACK_OPEN_TIMEOUT; guard stays held until preparation join, then manager can reopen |
| production_open_dispatch_rejects_destroyed_late_readiness_without_install at :2954 | Same literal dispatch_open_playback -> open_playback_owned; PlaybackManager::shutdown cancels the real lease/generation | NATIVE_UNAVAILABLE, no active session, late readiness cannot install, reaper closes/joins worker before release |
| production_close_dispatch_waits_for_worker_quiescence_before_release at desktop_playback.rs:2990 | take_for_close (the public command prelude) -> dispatch_close_playback at :2036 -> spawn_blocking -> cleanup_session -> worker join -> mark_closed; public desktop_playback_close at :2021 calls the same adapter | Close task remains pending, guard remains PlaybackBusy, then closed receipt/guard release follows quiescence |
| meeting_intel::tests::recording_qa_checked_pipeline_skips_provider_without_matching_evidence at meeting_intel.rs:2005 | Literal meeting_ask_recording_checked at :698; public meeting_ask_recording at :680 calls that same checked function | Two valid-pair no-evidence cases return insufficient_evidence, model=None, empty sources, zero stub calls |
| recording_qa_production_path_captures_only_pair_scoped_outbound_body at meeting_intel.rs:1882 | Literal meeting_ask_recording_checked at :698 -> call_recording_llm -> existing RecordingLlmStub | Existing adversarial pair-scoped outbound/citation capture remains green |

The playback test helper does not copy the open/close orchestration: its
dispatch_open_playback call enters the production adapter, which enters the
literal owned function. The injected seam stops only at preparation payload
and worker readiness/close I/O, before any CPAL operation.

## Preserved prior corrections

- Serialized playback epochs, pair/cursor invariants, actual frame duration via
  hound::WavReader::duration(), source-deleted PLAYBACK_SOURCE_MISSING,
  path custody, late-source handling, and actual handle custody are retained.
- Generation/disposal cancellation, one shared production 10-second deadline,
  late-install rejection, reaper ownership, and off-dispatch cleanup remain in
  the production pipeline.
- The withdrawn same-handle finding is not reintroduced.
- Recording-scoped Q&A still validates the pair first, excludes graph/live-tail
  inputs, sends only selected pair evidence, validates returned references, and
  makes no delimiter/citation safety guarantee.

## Exact final hashes

The three FIX3-owned source/report paths and the three frozen prior inputs have
these SHA-256 values after finalization:

- src-tauri/src/recording_review.rs — 460e68324427ac358627ac3c6c18e468ad27d668385ef2e3317b49622df3e43e (frozen unchanged).
- src-tauri/src/desktop_playback.rs — b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae.
- src-tauri/src/lib.rs — 2ebb0abcfef27cc5c600ca73b38af6051727c74613a91f974eacf562dc8de7b5 (frozen unchanged).
- src-tauri/src/meeting_intel.rs — cc87422b81a29fdb32b3af18b58e37799b26d7a22617c6f765cae3481a388477.
- src-tauri/src/live_meeting.rs — 66e85d914ca468a140f8cfd56d30072ccbc8cbd808b0b3560aa359075f5677c5 (frozen unchanged).
- This report’s self-hash is emitted by the final hash command after the
  report is finalized and is intentionally not embedded self-referentially.

## Verification evidence

All commands ran with the exact cwd above and used temporary local Genesis/WAV
fixtures only.

| Command | Result |
|---|---|
| cargo fmt --manifest-path src-tauri/Cargo.toml -- --check | PASS. |
| cargo check --manifest-path src-tauri/Cargo.toml --lib | PASS; only existing unused/dead-code warnings in backup.rs. |
| git diff --check | PASS after final report write. |
| cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings | FAIL only in untouched src-tauri/src/auth_session.rs:3384 and src-tauri/src/backup.rs:28,95-99,110,114,137; no suppression or out-of-scope fix was made. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib recording_review::tests -- --nocapture | PASS, 6/6. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib desktop_playback::tests -- --nocapture | PASS, 20/20; 16 prior + 4 FIX3 production-path tests. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib meeting_intel::tests | PASS, 18/18; 17 prior + 1 FIX3 checked-pipeline test. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib live_meeting::tests -- --nocapture | PASS, 16/16. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib native_behavioral_ -- --nocapture | PASS, 22/22. |
| cargo test --manifest-path src-tauri/Cargo.toml --lib | Exit 1: 472 total, 465 passed, 6 failed, 1 ignored. |

The six full-suite failures are the same FUNGWIRE transcription tests blocked
by the absent actual runtime at
C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\baseline-376ef30\.venv-whisper\Scripts\python.exe:

- fungwire_server::tests::job_loop_reassembles_multi_subframe_chunk_and_returns_transcript
- fungwire_server::tests::transcribing_progress_is_streamed_before_result
- fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes
- fungwire_client::tests::delegate_transcription_completes_and_writes_transcript_over_loopback
- fungwire_client::tests::delegate_transcription_reconnects_after_early_drop_and_completes
- fungwire_client::tests::delegated_job_persists_the_requested_executor

No Whisper runtime was installed, mocked, or faked.

## Acceptance boundary

Source-level and deterministic local Rust evidence is complete for this FIX3
lease. Native application/package launch, CPAL output device, microphone or
system audio, physical device, real provider, credentials, network, userdata,
hosted CI, packaged runtime, and production/release acceptance remain
NOT_RUN. The result must not be promoted to native or production readiness.

## Version diff / changelog

- 0.1.2b -> 0.1.3b: FIX3 added the actual checked-pipeline no-evidence
  provider-call regression and drove delayed playback open/close through the
  production-owned async adapters with bounded deterministic I/O. No feature or
  contract change was made.
- Existing FIX2 tests and prior corrections remain retained; the baseline full
  result was 467 total (460 passed, 6 failed, 1 ignored), and FIX3 adds five
  tests for the final 472 total.
- Result remains uncommitted and requires the independent review requested by
  the governing RCA.

| Version | Date | Status | Summary | Commit | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Approved P1-B backend recording implementation baseline | UNCOMMITTED; base 376ef30 | Luna backend worker |
| 0.1.1b | 2026-09-17 | beta | FIX1 bounded deterministic regression evidence | UNCOMMITTED; base 376ef30 | Luna backend worker |
| 0.1.2b | 2026-09-17 | beta | FIX2 owned async dispatch, cancellation/quiescence, and actual-path Q&A evidence | UNCOMMITTED; base 376ef30 | gpt-5.6-luna/max BACKEND FIX2 implementer |
| 0.1.3b | 2026-09-17 | beta | FIX3 closes actual checked-Q&A and production open/close path evidence gaps | UNCOMMITTED; base 376ef30 | gpt-5.6-luna/max BACKEND FIX3 implementer |
