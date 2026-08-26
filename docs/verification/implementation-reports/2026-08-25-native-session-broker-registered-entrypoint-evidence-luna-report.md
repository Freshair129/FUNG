---
version: "1.0.0b"
created_at: "2026-08-25T17:39:01+07:00,Luna 5.6"
last_update: "2026-08-25T17:39:01+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-report"
  scope: "D-GDA6 documentation candidate only; no source/test implementation"
  risk: "HIGH"
  complexity: "C-3"
  candidate_path: "docs/specs/2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment.md"
  candidate_commit: "externally bound in final handoff; this report cannot self-embed its amended commit"
  candidate_sha256: "2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F"
  write_scope: "exactly two documentation paths"
---

# Luna 5.6 Documentation Report — D-GDA6

## Result and authority boundary

Drafted a new HIGH-risk C-3 candidate amendment, D-GDA6, for the narrow
registered-entrypoint evidence remediation required after Terra blocked D-GDA5
Fix3. This report and its companion candidate document are documentation only.
No source change, test change, dependency change, provider action, deployment,
release, push, PR, merge, or production approval has started or is authorized.

The candidate remains candidate/unapproved until a fresh Terra review covers
the exact committed bytes and Boss approves D-GDA6-01 through D-GDA6-06 with the
exact candidate commit and SHA-256.

## Root-cause basis

Terra Fix3 commit c390f1f9c33d27867f6fdef7e3713ebe3414ab02 reviewed source commit
837b04476b720553997719b9be71da9470029d6e and found three P0 evidence gaps:

1. logout, shutdown, and disconnect_drive were dead in non-test compilation
   while behavioral tests called them.
2. Drive race evidence did not execute registered-equivalent broker_drive_*,
   real DriveOperationGuard::drop, or the actual resumable provider-send
   boundary.
3. Recovery evidence called a fake-port helper and directly mutated maps instead
   of entering the same non-test startup composition route for Account and each
   registered Drive domain.

D-GDA6 addresses only those production-entrypoint/evidence gaps. It retains the
already-recorded local passes and explicitly keeps all external gates open.

## Candidate decision inventory

| Decision | Implementation-ready outcome |
|---|---|
| D-GDA6-01 | One non-test typed façade is shared by registered wrappers and deterministic tests; dead/test-only lifecycle seams are removed or converged. |
| D-GDA6-02 | One composition root defaults to NativeKeyring/NativeClock/NativeListener/NativeProvider; tests inject deterministic implementations of the same ports into the same graph. |
| D-GDA6-03 | Registered-equivalent Drive flow owns a real guard through resumable provider send and deterministic disconnect/logout/shutdown barriers. |
| D-GDA6-04 | The same non-test startup_recover route exercises Account and every registered Drive domain with injected keyring faults, restart, and post-marker fail-closed cases. |
| D-GDA6-05 | Compiler/lint/source/behavioral evidence has zero D-GDA6-attributable warnings and itemizes bounded unrelated warnings. |
| D-GDA6-06 | Fresh Terra review, exact Boss hash approval, and maximum three implementation cycles are mandatory. |

## Documentation write scope and provenance

The exclusive write set for this task is exactly:

1. docs/specs/2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment.md
2. docs/verification/implementation-reports/2026-08-25-native-session-broker-registered-entrypoint-evidence-luna-report.md

Future implementation write scope is separately limited by the candidate to:

1. src-tauri/src/auth_session.rs
2. src-tauri/src/drive_oauth.rs
3. src-tauri/src/lib.rs
4. tests/nativeSessionCustody.test.mjs
5. tests/googleDriveContract.test.mjs
6. docs/verification/implementation-reports/2026-08-25-native-session-broker-registered-entrypoint-evidence-implementation-luna-report.md

No additional path was added. Any genuinely required extra path must stop the
future implementation and receive a new amendment.

## Checks performed for this documentation task

| Check | Result | Boundary |
|---|---|---|
| Read AGENTS.md and required project entry docs | PASS | Followed docs-first, RCA, HIGH/C-3, and external-gate rules |
| Read D-GDA5 candidate and Terra Fix3 review | PASS | Used exact commit/hash and P0 findings as provenance |
| Inspected current registered command/setup inventory and relevant source/tests | PASS | Confirmed generate_handler!, startup_recover, native adapter types, Drive guard/provider symbols, and fake-port test seams |
| Current cargo check --message-format=short | PASS with pre-existing warnings | Three D-GDA5 lifecycle warnings are the target gap; unrelated warning symbols are itemized in D-GDA6 |
| User-owned dirty/untracked files | PRESERVED | No user-owned path staged, edited, deleted, or reformatted |
| Markdown/provenance review | PASS | Both new files have metadata, candidate status, version diff, changelog, exact scope, and no production claim |
| Source/test implementation | NOT STARTED | Explicitly prohibited under this task |

The current checkout was observed on branch codex/backlog-truth-sync at
c390f1f9c33d27867f6fdef7e3713ebe3414ab02 before the candidate commit.

## Candidate binding

After the final bytes of the candidate document were written, its exact SHA-256
was computed and recorded below. The candidate document deliberately does not
self-embed its own hash because that would change the bytes being hashed.

| Field | Exact value |
|---|---|
| Candidate document | docs/specs/2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment.md |
| Candidate commit | externally bound in final handoff; self-reference is intentionally not embedded |
| Candidate SHA-256 | 2D4C334A2D38AE0148296AD7DC83CA37FB890CDF4582540CCD0D018C11731F1F |
| Scope | D-GDA6-01 through D-GDA6-06; documentation only |
| Terra review | Required next; must be fresh and hash-bound |
| Boss approval | Required next; exact commit + exact SHA-256; not granted by this report |

The candidate SHA-256 above is computed from the final candidate bytes before
the focused commit. The candidate commit is bound after that commit exists.
Any later candidate byte change invalidates the binding and requires a new hash,
commit, and fresh Terra review.

## Acceptance boundary

This report is complete when the two documentation files are committed together,
the candidate SHA-256 is recorded exactly, the commit contains no other path,
and the repository remains free of implementation changes from this task. It is
not an implementation report and does not assert any AC-GDA6 criterion passed.

## Version Diff

- 1.0.0b: new Luna report for the D-GDA6 documentation candidate; records the
  exact write scope, root-cause evidence, candidate binding protocol, and the
  explicit no-implementation boundary.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0b | 2026-08-25 | candidate | Drafted D-GDA6 documentation-only remediation report; candidate hash/commit to be bound after final commit. | externally bound after commit | Luna 5.6
