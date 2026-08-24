---
version: "0.1.0b"
created_at: "2026-08-24T09:09:38+07:00,Terra"
last_update: "2026-08-24T09:09:38+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Document-only Terra review of durable enrollment-proof nonce and native PKCE amendment"
  risk: "HIGH"
  review_head: "7bc84c199b21f2e8ca79b73221fd73cffca4ace9"
  candidate_state: "untracked working-tree file at review time"
  candidate_sha256: "5F778F970E06825B0D875B9AAE5859D8B274928C25D8BC5DE8CD59E8E64F5C33"
---

# Enrollment Proof Nonce and Native PKCE Amendment — Independent Terra Review

## Verdict

**FAIL — not ready for Boss approval.** The candidate has the right security
direction, but it does not yet define a verifier-independent canonical proof
contract, a replay reservation that is durable across pending-request lifecycle,
or a migration rule compatible with the approved forward-only posture. It
therefore cannot safely close the S2 replayable-enrollment-proof P0, and it
cannot be approved as a complete P0 remediation package.

Native PKCE is directionally sound: native ownership of the verifier, S256
challenge, authorization URL, callback, and token exchange removes the S2
browser-side handoff. That direction is **not** an implementation PASS, and its
acceptance mapping must still preserve every inherited native-login hard gate.

This was a documentation-only review. I did not inspect or modify code,
migrations, deployment state, the candidate, or prior reports. No external
action was performed.

## Evidence reviewed

- AGENTS.md and the FUNG master-plan, desktop-architecture, mobile-status, and
  desktop-progress documents required by it.
- Candidate:
  docs/specs/2026-08-24-enrollment-proof-nonce-amendment.md, SHA-256
  5F778F970E06825B0D875B9AAE5859D8B274928C25D8BC5DE8CD59E8E64F5C33.
- Review brief:
  docs/verification/implementation-reports/2026-08-24-enrollment-proof-nonce-terra-review-brief.md.
- S2 Terra FAIL at 7bc84c1:
  docs/verification/implementation-reports/2026-08-24-w1-f4-s2-terra-review.md.
- S2-F1 scope:
  docs/verification/implementation-reports/2026-08-24-w1-f4-s2-fix1-brief.md.
- Approved native-authorization and authority/schema amendments.

The candidate and the two prior amendment files were untracked working-tree
documents at review time. This report attests only to the candidate hash above;
Boss approval must name that hash or a committed successor.

## What the candidate retains correctly

| Control | Assessment | Evidence |
|---|---|---|
| Native, operation-specific signing; no raw key or generic signing oracle | **PASS in design** | Candidate section 2.2 requires a narrow native signature command and forbids a generic signing oracle. |
| Pending-only outcome; no self-service drive_trusted promotion | **PASS in design** | Candidate sections 2.3/4 and AC-GDA3-06 retain Boss-only bootstrap promotion. |
| Native PKCE ownership | **WARN — correct direction** | Candidate section 2.1 and AC-GDA3-01/02 move verifier, S256 challenge, callback, and exchange out of the WebView. |
| Server-side replay decision | **WARN — incomplete contract** | Candidate sections 2.3/3 require one transaction and one winner, but omit the durable reservation lifecycle and identity. |

## Blocking findings

| ID | Priority | Finding | Required correction before Boss approval |
|---|---|---|---|
| P0-GDA3-01 | P0 | The envelope is called “canonical,” but no normative byte encoding, field representation, domain separation, or source rule for the signed user ID is defined. An Edge verifier cannot independently reconstruct one unambiguous signed message from the document. | Add a versioned envelope table and exact canonical-byte algorithm. Define UTF-8/Unicode normalization, platform enum, label limits, timestamp precision/timezone, base64url or binary encodings, nonce length/encoding, fingerprint derivation from public-key bytes, Ed25519 input/domain separator, and rejection of unknown/duplicate/non-canonical fields. Native must derive the user ID only from its native-owned verified session; Edge must compare it to auth.uid(). |
| P0-GDA3-02 | P0 | “Unique nonce constraint/index suitable for one-use reservation” is not a durable replay design. The candidate stores proof metadata on pending enrollment but does not specify a reservation identity, uniqueness scope, consumption/expiry state, retention rule, or prohibition on cleanup/deletion while a proof can still validate. | Define an immutable reservation record (or an immutable pending record with equivalent lifecycle) keyed by a globally unique nonce hash in the enrollment-proof domain. It must survive pending approval, rejection, revocation, and ordinary cleanup until no valid proof can exist; transaction rollback must remove both reservation and pending creation, while a successful first use must make every replay a no-mutation denial. State the exact conflict behavior and retention bound. |
| P0-GDA3-03 | P0 | D-GDA3-03 conditionally edits 20260823000000_w1_device_enrollment_authority.sql. That conflicts with approved D-GDA2-08’s two forward-only migrations and is not bounded to a project-ref manifest, migration-history evidence, or a reviewed immutable artifact. “No live applied-migration evidence” is insufficient proof that no target has applied or consumed the migration. | Default to a new forward-only migration. If Boss wants an exception for an unshipped artifact, make it an explicit amendment to D-GDA2-08 and require: exact candidate/branch hash, enumerated project refs, read-only migration-history proof for every ref, confirmation that no published/merged artifact can contain the old migration, and a recorded go/no-go before any edit. Do not leave the exception as a worker inference. |

## Non-blocking but required corrections

| ID | Priority | Finding | Required correction |
|---|---|---|---|
| P1-GDA3-01 | P1 | AC-GDA3-01/02 do not explicitly inherit the full S2 hard gate 1 and AC-GDA2-09: trusted authorization origin, exact per-request redirect URI, code_challenge_method=S256, one callback/listener, strict callback shape, cancellation/timeout/replay handling, and redacted typed result. | Add an “Inherited and mandatory” mapping to D-GDA2-07, AC-GDA2-09, and S2 gate 1. Require native tests for missing/wrong S256 method, verifier mismatch, redirect mismatch, duplicate/additional callback parameters, terminal races, code replay, and secret non-disclosure. |
| P1-GDA3-02 | P1 | The candidate reopens already approved decisions as D-GDA3-01, -02, and -04, while the S2-F1 brief says the P0 fix introduces no new authority decision. D-GDA3-03 additionally widens the S2-F1 no-schema-change scope without identifying a successor work package. | Replace duplicated rows with an inherited-decision crosswalk. If the migration exception remains, make it the sole new Boss decision, explicitly amend D-GDA2-08, and place implementation in a separately bounded schema fix cycle rather than S2-F1. |
| P1-GDA3-03 | P1 | AC-GDA3-07 says “rollback” without distinguishing transactional test rollback from the approved post-commit deny-only/compensating-migration posture. Its adversarial matrix also omits the approved 50-concurrent-request proof and nonce reuse with differing envelope fields. | State that test rollback means transaction rollback only; committed migration rollback remains compensating/deny-only. Add first-use, exact replay, concurrent at least 50 identical proofs, nonce reuse across altered claims, expiry/skew, tamper, foreign session, wrong operation/platform/label, revoked/rebind, privilege, fixed-search-path, and denial-no-mutation cases. |

## Required Boss-decision shape after correction

Boss should not be asked to reapprove the already approved native-login,
pending-only, or operator-bootstrap boundaries. The corrected document should
ask for only one of these mutually exclusive migration outcomes:

1. **Forward-only default:** add a new migration for the nonce reservation and
   retain the already approved W1 migrations unchanged; or
2. **Narrow unshipped exception:** amend D-GDA2-08 with the immutable,
   all-project preflight evidence described in P0-GDA3-03 before any migration
   text is edited.

All other GDA3 items should be stated as inherited constraints, not new
authority choices.

## Required local re-review evidence

After a corrected candidate is approved and implemented in its explicitly
bounded work package, a fresh Terra review must receive:

1. Native PKCE behavioral tests proving native-only verifier generation,
   S256 URL construction, exact redirect binding, native-only token exchange,
   terminal zeroization, and no verifier/code/token in WebView-visible outputs
   or logs.
2. Native/Edge contract tests that sign the exact published envelope bytes and
   reject every non-canonical, tampered, foreign-session, expired, wrong-
   operation/platform/label, and forged-signature case before mutation.
3. PostgreSQL tests proving one durable winner, transaction atomicity,
   no-upsert/no-update replay denial, concurrency, fixed search_path, and
   least-privilege execute posture.
4. A changed-path, secret, clean-checkout, and regression evidence packet.

Passing those local tests would establish local implementation evidence only;
they would not establish deployment, provider, or production readiness.

## External gates retained

These gates remain external and must not be treated as repaired by this
document or a local test run:

1. Boss records the corrected decision against an immutable candidate hash or
   committed candidate, plus the exact project-ref manifest.
2. Per-project read-only Supabase migration-history, RLS, function-config, and
   execute-grant preflight; then separately authorized staging migration and
   Edge deployment.
3. Staging proof that authenticated WebView, anon, Data API, Edge, and foreign
   owners cannot promote/rebind/approve drive_trusted, and that the
   database-owner Boss bootstrap ceremony has one winner.
4. Google installed-app client configuration and real consent, callback, native
   PKCE exchange, refresh, revoke, upload, and digest-bound download/restore.
5. Clean-install Windows keyring migration/reconnect/restore, Android/FUNGWIRE
   delegation, signing, release, merge, and promotion evidence.

## Scope and preservation audit

- Reviewed branch: codex/backlog-truth-sync, HEAD
  7bc84c199b21f2e8ca79b73221fd73cffca4ace9.
- No code, migration, candidate-spec, deployment, prior-report, push, merge,
  PR, deletion, or external-message action was performed.
- This report is the only file added by this review. All pre-existing modified
  and untracked paths remain outside the commit.

## Version Diff

- new -> 0.1.0b: document-only Terra FAIL review records three P0
  specification blockers, required wording/scope corrections, retained
  external gates, and the exact candidate hash reviewed.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | need review | FAIL: candidate is not Boss-approval ready until its canonical envelope, durable replay reservation, and forward-only migration posture are specified. | pending | Terra |
