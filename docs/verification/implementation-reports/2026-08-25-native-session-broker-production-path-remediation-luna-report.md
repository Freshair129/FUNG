---
version: "0.2.0b"
created_at: "2026-08-25T04:09:26+07:00,Luna 5.6"
last_update: "2026-08-25T04:25:23+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "documentation-draft-report"
  scope: "Documentation-fix cycle 1; Desktop/Tauri production-path remediation candidate; no implementation evidence"
  risk: "HIGH"
  complexity: "C-3"
  authorization: "Boss approved drafting only"
  candidate_path: "docs/specs/2026-08-25-native-session-broker-production-path-remediation-amendment.md"
  future_implementation_report: "docs/verification/implementation-reports/2026-08-25-native-session-broker-production-path-remediation-implementation-luna-report.md"
  approved_prior_candidate_commit: "7d48aa01c243ce5f32af1005b95b71082c5a5984"
  approved_prior_candidate_sha256: "41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4"
  latest_implementation_commit: "36fa29412fc46a764e1bccae94e44bf0d4d7a6e5"
  terra_fix3_commit: "07649e7526243446f719a2dcab63e6bba5b94285"
  candidate_commit: "externally bound after commit; candidate cannot self-embed its own commit/hash"
  candidate_sha256: "externally bound after commit; candidate cannot self-embed its own commit/hash"
  report_commit: "externally bound after commit; this draft report cannot self-embed its own commit/hash"
  superseded_draft_review_commit: "0bdf2ad525c4f9bb263e41fdb9332e2a1fb8478e"
  superseded_draft_candidate_sha256: "D590ABB67C13FC02A1AD96B2E0D6E895DCA49321C30E09F098DB5DFFF74C0172"
  superseded_draft_disposition: "Terra FAIL/BLOCK evidence; not approval"
---

# Native Session Broker Production-Path Remediation — Luna 5.6 Documentation Draft Report

## Outcome

This report is a documentation-draft report for a documentation-only,
reviewable HIGH-risk C-3 candidate amendment. It is not the future implementation
report named in the candidate write set. It does not claim implementation,
code/test changes, provider or environment configuration, deployment, release,
merge, production readiness, or approval. Boss authorized drafting only:
“approve drafting Native Session Broker production-path remediation amendment”.

The candidate is a remediation authority proposal after Terra fix3 FAIL/BLOCK and
the maximum three fix cycles. It supersedes only the failed authority for a
possible future fix4; it does not rewrite audit history or close production gates.

## Source provenance

| Role | Exact commit | Evidence used |
|---|---|---|
| Approved prior candidate | `7d48aa01c243ce5f32af1005b95b71082c5a5984` | `docs/specs/2026-08-24-native-session-broker-amendment.md`; approved SHA-256 `41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4` |
| Latest implementation | `36fa29412fc46a764e1bccae94e44bf0d4d7a6e5` | `src-tauri/src/auth_session.rs`, `src-tauri/src/drive_oauth.rs`, `src-tauri/src/lib.rs`, `tests/nativeSessionCustody.test.mjs`, and its implementation report |
| Terra fix3 review | `07649e7526243446f719a2dcab63e6bba5b94285` | `docs/verification/implementation-reports/2026-08-24-native-session-broker-terra-rereview-fix3.md`; FAIL/BLOCK, maximum fix-cycle escalation |
| Terra review of superseded draft | `0bdf2ad525c4f9bb263e41fdb9332e2a1fb8478e`; draft candidate SHA-256 `D590ABB67C13FC02A1AD96B2E0D6E895DCA49321C30E09F098DB5DFFF74C0172` | Independent FAIL/BLOCK evidence for the superseded draft; not approval and not the identity of this revised candidate |

The candidate and this draft report cannot self-embed their own final commit or
hash: those identities are externally bound after commit. The final candidate
SHA-256 is therefore an external post-commit value, not a value asserted inside
the candidate itself.

## Evidence and decision crosswalk

The read-only source audit confirmed the Terra trigger: actual Tauri commands use
separate `SessionMemory` control flow while the fourteen behavioral cases target
`LifecycleCore` and fakes; refresh/login can write before the later generation
check; callback and OAuth URL paths use ordinary `url::Url`/`String` allocations;
Drive restore accepts an ordinary recovery-phrase `String`; and Drive deletion
readback treats a read error as absence. These remain current implementation
findings, not claims that this documentation draft fixes them.

The candidate adds the smallest complete decision set:

- `D-GDA5-01`: hard-gate C-3 architecture diagram and exact registered
  command-to-engine/port mapping, with the test harness constructing the same
  production `SessionLifecycle` rather than a parallel generic core;
- `D-GDA5-02`: two coordinated `AccountSession` and
  `DriveConnection`/`DriveCredential` domains with exact generation,
  operation-ID, admission/quiescing, lock/commit-fence, and terminal-cleanup
  rules;
- `D-GDA5-03`: hard-gate B1-B5 custody classes for Tauri/Serde ingress,
  callback parser, provider response, provider form, and OS/browser handoff;
- `D-GDA5-04`: hard-gate failure-atomic immutable slots/non-secret marker,
  deterministic startup matrix, compensating cleanup and fail-closed taxonomy;
- `D-GDA5-05`: production-path behavioral evidence and bounded local write/test
  matrix;
- `D-GDA5-06`: fresh Terra review, exact-commit/hash Boss approval, provenance,
  and retained external gates.

The candidate explicitly preserves the passing `cleanup_failed` shutdown result
path, the 50-client proof of one winner/49 `proof_replayed`/zero loser mutation,
typed IPC, Browser unchanged, Mobile deferred, and the P2 warning that
`GoogleDrivePanel.tsx` passes an ignored invoke argument. The P2 warning is out
of future write scope unless separately amended and approved.

## Closure record for Terra findings

### T5-P0-02 / T5-P1-01 — one engine, architecture, and Drive domains

The candidate’s normative §3.1 names the exact current `lib.rs` registrations:

| Registered command family | Required one-engine route |
|---|---|
| `broker_session_login_begin`, `broker_session_login_cancel`, `broker_session_status`, `broker_session_logout` | `AccountSession` through `ClockPort`, `ListenerCallbackPort`, `ProviderHttpPort`, `KeyringPort`, and `CommitObservationPort` |
| `broker_enrollment_request`, `broker_enrollment_status`, `broker_device_list`, `broker_pairing_create`, `broker_pairing_poll`, `broker_pairing_reconcile`, `broker_device_revoke`, `broker_device_audit_list`, `broker_device_endpoint_publish` | `AccountSession` admission/identity boundary through provider, clock, and commit-observation ports |
| `broker_drive_connect_begin`, `broker_drive_connect_complete`, `broker_drive_connect_cancel` | `DriveConnection` OAuth/callback/activation through listener, provider, clock, keyring, and commit-fence ports |
| `broker_drive_status`, `broker_drive_disconnect` | `DriveCredential` status/revoke/cleanup through keyring and commit-fence ports |
| `broker_drive_list_archives`, `broker_drive_upload_archive` | Drive data boundary through keyring, Drive HTTP, archive/job, and commit-fence ports |
| `broker_drive_restore_intent`, `broker_drive_restore` | Drive restore boundary through keyring, Drive HTTP, archive/job, clock, and commit-fence ports; recovery phrase is B1-only |

The same engine owns domain generations, operation IDs, admission/quiescing,
lock/commit fence, publication, and terminal cleanup. Account logout/shutdown
invalidate both account and Drive operations; Drive disconnect invalidates its
Drive generation. Pending completion cannot recreate a Drive slot, marker,
in-memory credential, access token, connected status, or public success after any
of those linearization points.

### T5-P0-01 — failure-atomic credential commit/recovery

The candidate now requires versioned immutable `AccountSession` and
`DriveCredential` slots plus a non-secret marker. The only linearization point is
verified marker readback naming a valid slot. Before that point the old marker/
slot remains authoritative and failures require compensating orphan deletion plus
verified absence. After it, the new slot is authoritative but old/orphan cleanup
and verified absence are required before public success/access. Ambiguous marker
write/readback, missing/corrupt slots, delete/readback errors, and crash
transitions fail closed and preserve `credential_cleanup_failed`/`cleanup_failed`.
The candidate includes the required startup matrix and fault-injection coverage
for every write, readback, marker, delete, verify, and crash transition in both
domains.

### T5-P0-03 — bounded custody classes

The candidate replaces unqualified custody claims with B1 Tauri InvokeBody/Serde
ingress, B2 native callback listener/parser, B3 provider response/Serde, B4
provider outbound form/request, and B5 OS/browser authorization handoff. It
requires custom visitor/direct zeroizing app custody, zeroized app-owned raw
buffers, no token `serde_json::Value`/ordinary intermediary, no app-owned
callback-code/state `url::Url`, immediate disposal, redacted logs/errors,
size/time bounds, and static/behavioral/fault tests. Framework/protocol copies
are recorded by owner/lifetime/no-retention evidence; the draft does not claim
that unowned framework memory can be forcibly zeroized. Residual risk remains an
external/provider UAT record.

All four findings are closed in the candidate documentation as hard gates; none
is claimed as implemented or Terra-approved. The current source and focused
tests remain read-only evidence for the future implementation.

## Exact future scope recorded

The candidate’s future implementation write set is exactly:

1. `src-tauri/src/auth_session.rs`
2. `src-tauri/src/drive_oauth.rs`
3. `src-tauri/src/lib.rs`
4. `tests/nativeSessionCustody.test.mjs`
5. `tests/googleDriveContract.test.mjs`
6. `docs/verification/implementation-reports/2026-08-25-native-session-broker-production-path-remediation-implementation-luna-report.md`

Browser, Mobile, `GoogleDrivePanel.tsx`, provider configuration, Cargo/package/
capability/configuration files, migrations, deployment, release, and unrelated
paths are excluded. No implementation was performed in this drafting task.

## Validation performed for this draft

- Read `AGENTS.md` and all four named project-entry documents.
- Read the prior candidate, Terra fix3 report, approval record, relevant current
  Rust/Tauri/test evidence, and neighboring specification/report metadata and
  changelog conventions.
- Verified the exact registered auth/device and Drive command inventory from
  `src-tauri/src/lib.rs`, plus the current `auth_session.rs`, `drive_oauth.rs`,
  and focused tests, read-only.
- Verified the source provenance commits exist and match the exact hashes recorded
  above.
- Verified only the two authorized documentation paths are in this cycle's write
  scope. Pre-existing modified and untracked workspace paths were preserved; no
  code, tests, Browser, Mobile, provider, deployment, or external action was
  performed.
- Validation is limited to documentation/provenance. Code tests, builds, browser
  checks, provider checks, VM/device checks, and release checks were not run and
  must not be implied by this report.

## External gates remain open

Real Supabase/Edge/RLS authorization, Google provider UAT, clean Windows keyring/
VM proof, device UAT, signing/release, deployment, and production approval remain
open. Local/static evidence cannot substitute for those gates.

## Version Diff

- `0.1.0b -> 0.2.0b`: documentation-fix cycle 1 records closure of Terra
  T5-P0-01, T5-P0-02, T5-P0-03, and T5-P1-01 in the candidate documentation:
  C-3 architecture/command/port mapping, coordinated credential domains,
  failure-atomic slot/marker recovery, and B1-B5 custody boundaries are now
  explicit hard gates.
- Corrected this file's `doc_type` to `documentation-draft-report`, named the
  distinct future implementation report path, and recorded candidate/report
  identity as externally bound after commit because a document cannot self-embed
  its own final commit/hash.
- Recorded Terra commit `0bdf2ad...` and failed draft hash `D590...` as
  superseded FAIL/BLOCK evidence, not approval. Preserved all passing evidence,
  open external gates, and the bounded future write set.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-08-25 | candidate | Documentation-fix cycle 1 records closure of Terra T5-P0-01/02/03 and T5-P1-01 in the candidate text; no implementation claim and external gates remain open. | externally bound after commit; not self-embedded | Luna 5.6 |
| 0.1.0b | 2026-08-25 | candidate | Drafted provenance report for the Desktop/Tauri production-path remediation candidate; no implementation claim and external gates remain open. | superseded by 0.2.0b | Luna 5.6 |
