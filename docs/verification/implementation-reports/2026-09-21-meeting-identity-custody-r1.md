---
version: "0.1.1b"
created_at: "2026-09-21T09:32:28+07:00,Dalton,b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-21T10:35:03+07:00,Dalton"
status: "candidate"
superseded_by: null
attributes:
  domain: "meeting-intelligence/privacy"
  doc_type: "implementation-report"
  scope: "R1 SI/D8 privacy custody repair in the approved local contract/native-adapter boundary"
  lifecycle: "candidate"
  execution_state: "REVIEW_READY_INDEPENDENT_ACCEPTANCE_PENDING"
  actualagentid: "01a0c1bb-f1a1-7d32-940d-92ba382ebce8"
  nickname: "Dalton"
  parentagentid: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  role: "ROLE-FIXER"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  risk: "HIGH"
  repair_lease: "R1-PRIVACY"
  base_snapshot: "b336f33ec400a38f003a0665c121069a87a543ac"
  worktree: "C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921"
  approved_rca_sha256: "188B439BE55C7167A1D93F4E6EAAA2D648F1453D36220415D621AD9FAB7AAC4F"
  expectedrepoformat: "FUNG implementation report with lease authority, RCA provenance, evidence, commands/exits/assertions, changed paths, NOT_RUN gates, version diff, and changelog"
  exact_write_lease: "src-tauri/src/genesis_adapter.rs; src-tauri/src/meeting_intelligence_schema.rs; contracts/meeting-intelligence-v1.yaml; tests/meetingIntelligenceContract.test.mjs; this report only"
  additional_module_lease: "NONE"
---

# Meeting identity custody R1 implementation report

## Handoff state

REVIEW_READY — independent Luna/Terra acceptance is pending. This report does
not self-accept the repair.

The accepted repair input is
C:/Users/pc/workspace/fung/.brain/rca/2026-09-21-meeting-identity-custody-plaintext.md
at SHA256
188B439BE55C7167A1D93F4E6EAAA2D648F1453D36220415D621AD9FAB7AAC4F.
The immutable repair base is
b336f33ec400a38f003a0665c121069a87a543ac. Root source, the old N3 source and
reports/G1, the RCA, approved SI/D8 specifications, workflow, and the Genesis
limit decision remain frozen.

The report authority and exact lease were written before source edits. The only
leased paths are:

1. src-tauri/src/genesis_adapter.rs
2. src-tauri/src/meeting_intelligence_schema.rs
3. contracts/meeting-intelligence-v1.yaml
4. tests/meetingIntelligenceContract.test.mjs
5. this report

No additional module was requested or edited. lib.rs, Cargo.toml, Cargo.lock,
UI, native backup/device identity, global configuration, protected roots,
userDB/app launch, provider/enrollment, external calls, commit, push, PR,
merge, release, and deploy remain out of scope.

R1-DEP froze the dependency handoff and released the sole shared build slot.
Cargo.toml SHA256 is
ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E.
Cargo.lock SHA256 is
2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E.
The retained v10 plus approved retained-v11 aggregate set is preserved. The
clone-only Genesis remedy 64 -> 128 remains R1-DEP-owned; this repair does not
reduce, partition, or drop the v11 aggregate set.

## RCA and implemented repair

The accepted H2 gap was plaintext identity/provider/ACL relationship material
in attribution and meeting persistence, including serialization of the whole
attribution into revision and event output, plaintext profile foreign keys
alongside cipher references, and a prefix-plus-digest placeholder that did not
prove ciphertext, key custody, authentication, context, or semantic identity.
The old candidate code was not treated as live-user-data proof.

The implementation now:

- stores private person/provider relationship material only in an authenticated
  XChaCha20-Poly1305 identity envelope using a random 24-byte nonce, separate
  people_metadata key references, digest checks, and AAD for optional account,
  scope, trusted vault, link entity, reviewed revision, and the stored
  speaker_identity_links.model_run_id (or none);
- rejects malformed, placeholder, or over-4096-byte ciphertext envelopes before
  key lookup/decryption; uses bounded Zeroizing plaintext/key buffers, redacted
  Debug for plaintext-bearing result types, and an explicit zeroizing
  AuthorizedPerson drop path;
- captures native authority from the verified local device principal, allowing
  an explicitly unlocked account-free local-owner vault offline; account-bound
  vaults require the matching native account operation guard. Request actor,
  ACL, provider label, display name, owner, key reference, and arbitrary
  ciphertext claims cannot create authority;
- resolves the decrypted opaque profile_id to an existing active
  participant_profiles row in the trusted vault and project owner scope, with
  matching profile revision. Missing, deleted/inactive, wrong-vault,
  wrong-scope, and revision-mismatched profiles fail closed. The approved
  opaque profile primary index remains available;
- keeps ordinary spoken transcript text readable as transcript content while
  emitting opaque attribution/revision/projection/event/WAL/audit/backup
  relationship data. Authorized decrypted Person output exists only in bounded
  native memory;
- preserves capture-frontier-before-reads, expected-frontier compare-and-swap,
  review/evidence revision checks, lock/revoke invalidation, five-write
  source/revision/projection/event/cursor atomicity, idempotent replay, changed
  payload conflict, and durable-uncertain transaction identity semantics;
- uses the injected test key backend for real encryption/decryption and
  ciphertext persistence only. The production OS keyring adapter fails closed
  when the people_metadata key is unavailable, and no test reads or writes the
  real user keyring.

The AAD model context is deliberately taken from the native stored identity-link
provenance, not the caller's later transcript ASR model_run_id. The positive
fixture reuses the reviewed link with a different ASR run. A link provenance
tamper case changes the stored identity-link model provenance and fails
authentication/context validation.

## Validation evidence

All Cargo commands used the approved existing target
C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-contract-20260921-target,
with CARGO_INCREMENTAL=0, CARGO_PROFILE_DEV_DEBUG=0,
TAURI_CONFIG={"bundle":{"resources":[]}}, --offline, and --locked. No
dependency download or model/runtime download was attempted.

~~~text
node --test tests/meetingIntelligenceContract.test.mjs
exit 0 — 3 passed, 0 failed

cargo check --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/src-tauri/Cargo.toml --offline --locked
exit 0

cargo test --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/src-tauri/Cargo.toml --offline --locked --no-run
exit 0

cargo test --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/src-tauri/Cargo.toml --offline --locked --lib r1_
exit 0 — 3 passed, 0 failed, 488 filtered out

cargo test --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/src-tauri/Cargo.toml --offline --locked --lib oversized_identity_envelope_fails_closed
exit 0 — 1 passed, 0 failed, 490 filtered out
~~~

Focused Rust assertions cover positive account-free local-owner Person use,
different ASR model-run reuse, current profile existence and eligibility,
missing/deleted/wrong-vault profile rejection, real authenticated encryption,
wrong/missing key, tamper/digest, AAD/scope/vault swap, stored model
provenance tamper, placeholder reference, request claim injection,
stale-review/lock/revoke/account-bound guard rejection, private canary absence
after reopen from durable rows and opaque event output, atomic five-write
failure behavior, replay, changed-payload conflict, and frontier/cursor
preservation.

### Full Rust library result

The full Rust library command was run and is recorded as FAILED, not as a
pass or skip:

~~~text
cargo test --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/src-tauri/Cargo.toml --offline --locked --lib
exit 101 — 491 tests: 484 passed, 6 failed, 1 ignored, 0 measured
~~~

No valid-baseline comparison was run, so these failures are not labeled
pre-existing. In this run, each failure encountered the deliberately absent
FUNG Python runtime at
C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921/.venv-whisper/Scripts/python.exe.
The exact failures and observed errors were:

- fungwire_server::tests::job_loop_reassembles_multi_subframe_chunk_and_returns_transcript
  — transcribe_failed: FUNG Python runtime is missing; the test panicked
  expecting transcribing Progress.
- fungwire_client::tests::delegate_transcription_completes_and_writes_transcript_over_loopback
  — assertion failed: job must reach completed; left "failed", right
  "completed".
- fungwire_server::tests::transcribing_progress_is_streamed_before_result
  — transcribe_failed: FUNG Python runtime is missing; the test panicked
  expecting transcribing Progress.
- fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes
  — transcribe_failed: FUNG Python runtime is missing; the test panicked
  expecting transcribing Progress.
- fungwire_client::tests::delegate_transcription_reconnects_after_early_drop_and_completes
  — assertion failed: client must reconnect after the first connection is
  dropped and still complete; left "failed", right "completed".
- fungwire_client::tests::delegated_job_persists_the_requested_executor
  — assertion failed: a local request must reach the desktop as local and be
  handled accordingly; left "failed", right "completed".

This is an environment limitation observed in this run, not a baseline
reclassification. No fake runtime, app launch, userDB launch, or model
download was used.

## Frozen path hashes

SHA256 values after implementation and before this final report rewrite:

- src-tauri/src/genesis_adapter.rs:
  9E2448B83A4DBB2860D5ECC4F9E18899FA2E1F1565AF8095FEBDEAF9BD7E00EE
- src-tauri/src/meeting_intelligence_schema.rs:
  80B396198647E745F26B63ED283513D2579DA199A49137C47B38B95B6B34BBAD
- contracts/meeting-intelligence-v1.yaml:
  9684757F02EFD8A0F54B43FD58111757A3CA8E80CE586AD8751ED1F2FD119B48
- tests/meetingIntelligenceContract.test.mjs:
  D171A4EBE50E7273DCE68574C12E9B0C01D1D64C6D5496BFA4E1C142ABA1AB73

The final report hash is emitted with the handoff after this write; it is not
embedded in itself.

## Evidence boundaries and historical correction

Native OS-keyring integration, native UI/UAT, actual backup-key recovery,
provider/cloud behavior, application/userDB launch, CI, portable packaging,
release, and production deployment are NOT_RUN. The in-memory key backend is
contract-phase behavioral evidence only and does not prove native keystore,
UAT, or backup recovery.

The frozen G1 runtime-count correction remains unchanged: source count
41 + 25 = 66, while the actual schema-install attempt counted 101 and failed.
No new runtime-66 claim is made here.

## Version diff and changelog

Version 0.1.0b -> 0.1.1b: completed the leased R1 implementation evidence;
added authenticated private identity custody, native local-owner/account guard
boundaries, stored identity-link AAD provenance, current profile eligibility,
bounded envelope rejection, redacted Debug/zeroization, executable contract
coverage, focused behavior fixtures, and explicit full-suite failure/NOT_RUN
boundaries. The Genesis cap remedy and aggregate policy remain deferred to
R1-DEP.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-21 | candidate / REVIEW_READY | R1 privacy custody implementation and evidence | b336f33ec400a38f003a0665c121069a87a543ac | Dalton / 01a0c1bb-f1a1-7d32-940d-92ba382ebce8 |
| 0.1.0b | 2026-09-21 | superseded by 0.1.1b | Lease authority recorded before source work | b336f33ec400a38f003a0665c121069a87a543ac | Dalton / 01a0c1bb-f1a1-7d32-940d-92ba382ebce8 |

Lease release: the five-path R1-PRIVACY implementation is frozen for
independent review. No source or dependency lease remains open from this
worker.
