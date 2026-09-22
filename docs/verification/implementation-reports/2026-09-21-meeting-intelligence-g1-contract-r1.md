---
version: "0.1.0b"
created_at: "2026-09-21T11:02:56.739+07:00,Averroes,01a0c20b-3f5b-7610-8f7b-84edcc7b5441,gpt-5.6-luna,max"
last_update: "2026-09-21T11:02:56.739+07:00,Averroes,01a0c20b-3f5b-7610-8f7b-84edcc7b5441"
status: "need review"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "implementation-report"
  scope: "N4 independent G1 contract and security verification for N3 remediation R1"
  execution_state: "REVIEW_COMPLETE"
  verification_status: "FAIL"
  decision: "N4 FAIL/BLOCKED"
  change_risk: "HIGH"
  reviewer_id: "01a0c20b-3f5b-7610-8f7b-84edcc7b5441"
  nickname: "Averroes"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  authors_closed:
    - "Sartre:01a0c1b9-b266-7332-9a87-a612b0c0ebf0"
    - "Dalton:01a0c1bb-f1a1-7d32-940d-92ba382ebce8"
---

# N4 meeting-intelligence contract/security gate — FAIL/BLOCKED

## Verdict

**N4/G1 FAIL — not accepted.** This is the local native contract/schema
foundation gate. The failure does not authorize N5/N6/N7/N9/N13, and this
report makes no Terra/N15, full-integration, provider, release, or production
claim.

The decisive HIGH finding is that the offered native authority API does not
implement the approved explicit local-owner unlock boundary for an
account-free vault. It treats a persisted `identity_vaults.state == "active"`
row, owned by the native device principal, as sufficient authority. There is
no usable native unlock session, capability, lease, or expiry in the offered
API. The crypto and atomic-contract evidence is useful, but it cannot close
the required authority boundary.

A second HIGH contract finding is that an idempotent replay with a newly
constructed commit attempt returns the new attempt transaction ID while the
durable event retains the original transaction ID. The current replay test
reuses the exact original attempt and therefore does not detect this.

## Independence and scope

| Field | Value |
|---|---|
| Reviewer | Averroes / `01a0c20b-3f5b-7610-8f7b-84edcc7b5441` |
| Model / effort | `gpt-5.6-luna` / `max` |
| Task | `N4-G1-CONTRACT` |
| Risk | C3 / HIGH |
| N3 authors | Sartre and Dalton; both closed before this review |
| Inputs | frozen repair checkout, frozen engine clone, approved local Genesis cap, SI/D8 report and source specs |
| Writes | only this report; no source, test, Cargo, lock, spec, workflow, RCA, database, keyring, UI, commit, publish, or spawn action |

The unused new entry points and absent FUNG/UI wiring were not treated as an
N4 failure by themselves. They are recorded as integration NOT_RUN and remain
downstream scope. The authority finding below is independent of that wiring
boundary because it applies to the offered native API when called.

## Frozen targets and environment

| Target | Pin and observed state |
|---|---|
| FUNG repair | `C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-repair-r1-20260921`, branch `codex/meeting-intelligence-repair-r1-20260921`, HEAD `b336f33ec400a38f003a0665c121069a87a543ac`; inherited dirty tree preserved |
| Genesis experiment | `C:/Users/pc/AppData/Local/Temp/codex-fung-genesis-schema-limit-r1-20260921`, detached original Git `79b41a3f4ae4026d086b634c631f4f4a7ccbd142`; exactly one source patch and one new regression file |
| Cargo target | `C:/Users/pc/AppData/Local/Temp/codex-fung-meeting-intelligence-contract-20260921-target` only |
| Process env | `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`; FUNG contract commands also `TAURI_CONFIG={"bundle":{"resources":[]}}` |
| Cargo mode | `--offline --locked`; no download, model/runtime staging, global config, or cache mutation |

Git used explicit `-C` and command-local `safe.directory`. Git emitted an
environment warning that `C:/Users/pc/.config/git/ignore` was inaccessible;
the commands still exited successfully. This warning is not product evidence.

Cargo metadata independently resolved both the repair FUNG manifest and the
local Genesis manifest with `source: null`. The Genesis package was therefore
verified as the exact local clone, not as a new upstream revision or portable
release pin.

## Decisive findings

### F-001 HIGH — explicit local-owner unlock is not enforceable

The approved contract requires an explicit local-owner unlock for an
account-free local vault
(`contracts/meeting-intelligence-v1.yaml:18-20` and
`docs/specs/2026-09-21-speaker-identity-domain-design.md:132-137`). The
implementation does not carry or check such an unlock proof:

- `resolve_native_identity_context` queries only `id`,
  `owner_principal_ref`, `bound_account_ref`, and `state`
  (`src-tauri/src/genesis_adapter.rs:2477-2519`), then accepts the one row
  whose persisted state is `active` (`:2499-2507`).
- The v11 `identity_vaults` schema has owner, optional account binding, state,
  and key-store namespace, but no unlock-session/capability/expiry field
  (`src-tauri/src/genesis_adapter.rs:1594-1608`).
- `capture_native_identity_context` obtains an optional cloud/native account
  ID, a device fingerprint, and an account-operation guard only when an
  account exists (`:2523-2558`). The account-free branch has no equivalent
  local-owner unlock guard.
- `test_local_identity_context` is only a test-constructed
  `TrustedIdentityContext` (`:2561-2567`). The positive fixture inserts a
  vault with `state: "active"` and uses that injected context
  (`:4078-4135`); the positive R1 path then calls the injected backend
  directly (`:4244-4281`).

The account-bound guard and bound-account mismatch tests are valuable, but
they do not repair the account-free case. Under the current offered API, an
account-free vault can be selected after logout or account transition whenever
the persisted state remains active, without an explicit local-owner unlock
event/session/capability. This directly contradicts SI §5.2 and the contract's
unlock boundary. Native OS-keyring/UI testing being NOT_RUN is a separate
limitation; this finding is a source-level enforcement failure, not a claim
about an untested OS keyring.

### F-002 HIGH — alternate-attempt replay does not preserve durable transaction identity

`query_existing_event` reads only `payload_hash`, `cursor`, and `revision_id`
(`src-tauri/src/genesis_adapter.rs:2190-2210`). On a matching replay, the
returned `CommittedMeetingEvent.transaction_id` is copied from the caller's
current `attempt.transaction_id` (`:2855-2868`). The first durable event row
stores its original transaction ID (`:2996-3008`), and uncertain commits tell
the caller to preserve that identity (`:3093-3096`).

Therefore, if an uncertain commit succeeded durably and a reopen/retry creates
a new `MeetingCommitAttempt` with the same event and payload, the response can
report the new transaction ID while the event log contains the old one. That
breaks the required stable transaction identity/reconciliation contract. The
existing test only reuses the exact same attempt and asserts equality
(`src-tauri/src/genesis_adapter.rs:5780-5785`); the reopen test likewise
reuses the original attempt (`:6220-6235`). No authored test was added by this
review.

## Bounded security and contract assessment

| Area | Independent result |
|---|---|
| H1 Genesis cap | **PASS, local-only:** the experiment changes only table guard `64 -> 128`; 128/129 table tests, unchanged query/column/PK/index guards, rejection frontier preservation, reopen, sequencing, additive, and hash guards were exercised or recorded. No portable/upstream/release qualification. |
| H2 private identity crypto | **PASS under injected fixture boundary:** XChaCha20-Poly1305, random 24-byte nonce, real key lookup, ciphertext/AAD digests, authentication, bounded pre-key envelope rejection, zeroizing buffers, redacted authorized debug, stored-link model provenance, and opaque durable output are present and tested. |
| H2 native authority | **FAIL:** F-001 leaves the approved explicit-unlock requirement unenforced for account-free vaults. |
| H2 profile/scope gate | **PASS, bounded:** decrypted profile ID/revision is checked against an existing active profile row, trusted vault, project owner scope, and current identity-link review revision; missing/deleted/wrong-vault/stale/locked/revoked cases passed. |
| Capture frontier/CAS | **PASS, bounded:** `begin_meeting_commit` captures the frontier before validation reads; guarded commit uses expected-frontier CAS and account guard checks. |
| Atomicity/replay | **PASS for the exercised same-attempt cases; FAIL for F-002 alternate-attempt identity stability.** Five writes and durable reopen behavior passed the bounded N3 tests. |
| Native production/UI integration | **NOT_RUN:** no native UI, OS keyring, user database, native app, Meet, provider, model, or device test. FUNG live capture wiring is downstream/integration scope, not an independent N4 acceptance claim. |

The adapter does not use a plaintext person ID, provider label, or ACL subject
in the typed attribution or durable event projection. The private person and
display label are encrypted before persistence, and later ASR
`revision.model_run_id` is not used as identity AAD; the stored identity-link
model provenance is used instead. The positive fixture's differing-ASR-run
assertion and stored-provenance tamper assertion passed.

## Commands and results

All commands below used the frozen target, process-only settings, offline mode,
and locked dependency resolution.

| Command/result | Exit and assertions |
|---|---|
| `node --test tests/meetingIntelligenceContract.test.mjs` | exit 0; 3 passed, 0 failed. Supplemental Node string/contract checks only. |
| `cargo check --manifest-path .../repair-r1-20260921/src-tauri/Cargo.toml --offline --locked` | exit 0; source type-checks. Warnings include unused new contract entry points; those warnings were not treated as the N4 failure. |
| FUNG Cargo test no-run, same manifest/options | exit 0; all test executables built. |
| `cargo test ... --lib r1_` | exit 0; 3 passed, 0 failed. |
| `cargo test ... --lib oversized_identity_envelope_fails_closed` | exit 0; 1 passed, 0 failed. |
| `cargo test ... --lib n3_` | exit 0; 8 passed, 0 failed. Covers additive v10 migration, five-write atomicity, replay/conflict, repeated text, stale frontier/custody, manual correction, privacy sharing, and reopen replay. |
| Genesis `cargo test ... --no-default-features --features mobile --test relational_schema_resource_limit_tests` | exit 0; 4 passed, 0 failed. Accepts 64/66/128 tables, rejects 129 without schema/WAL/frontier advance, and keeps named-query 128/129, columns 128/129, PK 4/5, index 64/65 guards. |
| Genesis `cargo test ... --no-default-features --features mobile --test schema_version_tests` | exit 0; 3 passed, 0 failed. Fresh/current/older/newer schema behavior. |
| Genesis exact combined support command with all four `--test` targets | current independent rerun exit 1 during test compilation; no test rows executed for the two serde_json-using targets. |

The current failing engine command was:

`cargo test --manifest-path C:/Users/pc/AppData/Local/Temp/codex-fung-genesis-schema-limit-r1-20260921/Cargo.toml --offline --locked --no-default-features --features mobile --test relational_schema_resource_limit_tests --test relational_u2_contract_tests --test relational_u2_tests --test schema_version_tests`

The exact compiler failure included:

- `tests/relational_u2_contract_tests.rs:87:39`: expected
  `serde_json::value::Value`, found `Value`;
- `tests/relational_u2_tests.rs:83:29`: expected
  `serde_json::value::Value`, found `Value`;
- repeated `E0308` and `E0277` errors, followed by the compiler note
  `there are multiple different versions of crate serde_json in the
  dependency graph`;
- `error: could not compile genesis-block-native (test
  "relational_u2_contract_tests") due to 12 previous errors`.

Attribution is deliberately unresolved. Both engine and FUNG lockfiles resolve
`serde_json 1.0.150` with checksum
`e8014e44b4736ed0538adeecded0fce2a272f22dc9578a7eb6b2d9993c74cfb9`; engine
`cargo tree --duplicates` did not report a serde_json version duplicate. The
verbose current test rustc invocation used:

- `--extern serde_json=.../debug/deps/libserde_json-fdbb3628868d0550.rlib`;
- `--extern genesis_block_native=.../debug/deps/libgenesis_block_native.rlib`;
- `-C metadata=878e4bf77bb1a2d2`;
- `-C extra-filename=-526f61952e6848fb`.

The shared target also contains distinct serde_json fingerprint artifacts
(`fdbb3628868d0550`, `9893f12d386cb10a`, `9ea59076039278ab`,
`454d6cf9a41e64f9`) and Genesis dep-info for both the exact local clone and a
cached Git checkout. These observations preserve a reproducibility boundary;
they do not prove that artifact reuse caused the compiler error. No second
target was created and no cache was cleaned. The frozen dependency-worker
report recorded a prior combined 17/17 run, but that historical result and
pre-existing binaries were not substituted for the current independently
failing Cargo rerun.

The previously recorded full FUNG library run remains a failure, not a pass:
exit code **101** (the process exit code, not a test-row count), 491 total,
484 passed, 6 failed, 1 ignored. Six FUNGWIRE transcription tests encountered
the deliberately absent `.venv-whisper/Scripts/python.exe`. No valid baseline
comparison exists, so these failures are not called pre-existing.

## Frozen hash ledger — before and after review

The after-readback hashes are identical to the before-review hashes. The only
file written by this review is this new report.

| Frozen path | Before SHA256 | After SHA256 |
|---|---|---|
| `src-tauri/src/genesis_adapter.rs` | `9E2448B83A4DBB2860D5ECC4F9E18899FA2E1F1565AF8095FEBDEAF9BD7E00EE` | `9E2448B83A4DBB2860D5ECC4F9E18899FA2E1F1565AF8095FEBDEAF9BD7E00EE` |
| `src-tauri/src/meeting_intelligence_schema.rs` | `80B396198647E745F26B63ED283513D2579DA199A49137C47B38B95B6B34BBAD` | `80B396198647E745F26B63ED283513D2579DA199A49137C47B38B95B6B34BBAD` |
| `contracts/meeting-intelligence-v1.yaml` | `9684757F02EFD8A0F54B43FD58111757A3CA8E80CE586AD8751ED1F2FD119B48` | `9684757F02EFD8A0F54B43FD58111757A3CA8E80CE586AD8751ED1F2FD119B48` |
| `tests/meetingIntelligenceContract.test.mjs` | `D171A4EBE50E7273DCE68574C12E9B0C01D1D64C6D5496BFA4E1C142ABA1AB73` | `D171A4EBE50E7273DCE68574C12E9B0C01D1D64C6D5496BFA4E1C142ABA1AB73` |
| `docs/verification/implementation-reports/2026-09-21-meeting-identity-custody-r1.md` | `E70B712A481AC3A3C0090F3265863FACB48A44F4D759369FA14F3BE0A220ABBA` | `E70B712A481AC3A3C0090F3265863FACB48A44F4D759369FA14F3BE0A220ABBA` |
| `src-tauri/Cargo.toml` | `ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E` | `ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E` |
| `src-tauri/Cargo.lock` | `2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E` | `2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E` |
| Genesis `src/lib.rs` | `2D17B3914BC5DD9AE5CB0EE22BF3BB5B5751FBE8EACD8DC467E190591ED94F53` | `2D17B3914BC5DD9AE5CB0EE22BF3BB5B5751FBE8EACD8DC467E190591ED94F53` |
| Genesis `tests/relational_schema_resource_limit_tests.rs` | `494CB6A4A63C79934653C879F8DB8024D966A8AF84E05D903A424F25763F9704` | `494CB6A4A63C79934653C879F8DB8024D966A8AF84E05D903A424F25763F9704` |

## Limitations and next gate

Not run by design: native OS keyring, explicit unlock UI/session lifecycle,
backup/recovery, user DB, native app/UI, real Google Meet, provider/cloud,
network/model/runtime download, mobile/device, CI, portable packaging,
release, N15/Terra, or production deployment. The missing unlock lifecycle is
not waived by those exclusions; it is the F-001 source-level blocker.

Next gate is a new authorized R1 implementation/review cycle that provides a
native explicit unlock/session/capability proof for account-free vault use,
preserves account switch/logout/lock/revoke invalidation, and makes replay
return the durable transaction identity. It must add executable coverage for
those cases and rerun the bounded contract suite in an attributable Cargo
environment. Until then, N4 remains FAIL/BLOCKED and downstream N5/6/7/N9/N13
remain locked.

## Version diff and changelog

This is a new verifier report at version `0.1.0b`; there is no prior revision
of this report path. No source or dependency version was changed.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| `0.1.0b` | `2026-09-21` | need review | Independent Luna/max N4/G1 verification; local cap bounded evidence; explicit-unlock and replay-identity blockers recorded; downstream and environment limits preserved. | `b336f33ec400a38f003a0665c121069a87a543ac` (subject HEAD; no commit created) | Averroes / `01a0c20b-3f5b-7610-8f7b-84edcc7b5441` |
