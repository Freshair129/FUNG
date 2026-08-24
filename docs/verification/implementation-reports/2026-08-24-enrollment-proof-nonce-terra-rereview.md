---
version: "0.1.0b"
created_at: "2026-08-24T09:22:28+07:00,Terra"
last_update: "2026-08-24T09:22:28+07:00,Terra"
status: "candidate"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Document-only Terra re-review of corrected enrollment proof nonce and native PKCE amendment"
  risk: "HIGH"
  review_head: "11f8b52b5047b3cc23b15d47f3bea252a2cb8621"
  candidate_state: "untracked working-tree file at re-review time"
  candidate_sha256: "1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792"
---

# Corrected Enrollment Proof Nonce Amendment — Independent Terra Re-review

## Verdict

**PASS — candidate `0.2.0b` is ready for Boss's documentation approval** when
the approval names the SHA-256 below. All five corrections required by the
re-review brief are present and consistent with the approved authority/schema
boundary.

This is a document-architecture PASS only. It is not implementation, migration,
staging, provider, device, deployment, release, or production approval.

## Evidence reviewed

- `AGENTS.md` and its required master plan, desktop architecture, mobile status,
  and desktop progress documents.
- Corrected candidate:
  `docs/specs/2026-08-24-enrollment-proof-nonce-amendment.md` version `0.2.0b`,
  SHA-256 `1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792`.
- Prior independent Terra FAIL report at commit `11f8b52` and the corrected
  re-review brief.
- The authority/schema and native-authorization amendments, S2 Terra review,
  and S2-F1 scope brief for the inherited constraints.

The candidate remains an untracked working-tree artifact. This report binds the
review to the hash above; Boss must name that hash or a committed successor in
any approval record.

## Required correction verification

| Required correction | Result | Evidence in candidate `0.2.0b` |
|---|---|---|
| Exact canonical/domain bytes and native user binding | **PASS** | Sections 1–2 define native-only verifier/session ownership, source the account user ID from the token response plus TLS-authenticated `/auth/v1/user`, and specify a versioned Ed25519 envelope with a fixed ASCII domain prefix, ordered raw fields, byte lengths, network-order timestamps/lengths, NFC label handling, and unpadded base64url transport. The Edge path derives identity from the verified session and validates user binding before mutation. |
| Immutable global nonce-hash reservation with retention, conflict, rollback, and no-mutation replay | **PASS** | Section 3 requires an append-only, globally unique `SHA-256(raw_nonce)` reservation, indefinite W1 retention, one fixed-search-path transaction, `proof_replayed` on conflict without updates, rollback of both writes on failed pending creation, and permanent consumption after success. |
| New forward-only migration | **PASS** | Section 3 names new migration `20260824000000_w1_enrollment_proof_nonce.sql` and explicitly forbids editing migration history. The proposed write set keeps that migration separate from the earlier approved migrations. |
| Inherited PKCE hard gate and test matrix | **PASS** | Section 1 carries forward the exact native origin/redirect/port/path/state, one listener/callback, strict callback-shape, terminal-path erasure, timeout/cancel/replay, and WebView exclusion gates. The inherited-decision table retains D-GDA2-07's full native-login/adversarial-lifecycle matrix; AC-GDA3-01, -02, and -08 require the corresponding native/Rust/auth/Edge/secret-path evidence. |
| Inherited-decision crosswalk and isolated schema fix cycle | **PASS** | The D-GDA2-01 through -10 crosswalk preserves Boss-only bootstrap, grant/device/rebind boundaries, forward-only posture, and external ownership. D-GDA3-01 through -03 narrow the new work to encoding, replay retention, and one separately reviewed forward-only schema cycle; they do not reopen operator ownership, device class, grant issuer, or deployment policy. |

## Architecture consistency and document preflight

- The candidate retains a pending-only enrollment outcome and preserves the
  Boss-only `drive_trusted` bootstrap boundary.
- It retains the approved fail-closed Windows/device and separate operation-grant
  posture; it introduces no pairing, mobile, Drive-panel, project-reference, or
  deployment work.
- All reviewed source documents and commits exist. The candidate has complete
  candidate front matter and no inline Markdown links requiring resolution.
- No contradiction was found between the corrected candidate and the approved
  authority/schema decisions. No implementation behavior was inferred from this
  design review.

## Residual corrections

**None block Boss's documentation approval for the five required corrections.**
The implementation packet must still execute the inherited adversarial tests,
including the at-least-50 identical-request concurrency proof and native PKCE
negative/zeroization cases; that is a subsequent verification gate, not a
candidate-document correction.

## External and execution gates retained

1. Boss records D-GDA3-01 through -03 against the exact candidate hash (or a
   committed successor) and supplies the exact Supabase project-reference
   manifest.
2. A fresh, bounded Luna implementation cycle and independent Terra code/schema
   review must complete before integration; local test success remains local
   evidence only.
3. Every target project needs read-only migration-history, RLS, function-config,
   and execute-grant preflight before separately authorized staging migration or
   Edge deployment.
4. Staging must prove that authenticated WebView, `anon`, Data API, Edge, and
   foreign owners cannot promote, rebind, or approve `drive_trusted`, while the
   Boss database-owner bootstrap remains single-winner and fail-closed.
5. Real Google installed-app consent, native PKCE exchange/refresh/revoke,
   appDataFolder upload, digest-bound download/restore, clean-install Windows
   keyring migration/reconnect/restore, physical Android/FUNGWIRE evidence,
   signing, release, merge, and promotion remain separate gates.

## Scope and preservation audit

- Reviewed branch/head: `codex/backlog-truth-sync` at
  `11f8b52b5047b3cc23b15d47f3bea252a2cb8621`.
- No candidate, code, migration, prior report, deployment, push, merge, PR,
  deletion, or external-message action was performed.
- This report is the sole file added by this re-review. Pre-existing modified
  and untracked paths remain outside its commit.

## Version Diff

- `new -> 0.1.0b`: records the independent PASS re-review of the corrected
  candidate, its immutable approval hash, the five verified corrections, and
  retained external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | candidate | PASS: corrected candidate is ready for Boss's document approval at the recorded SHA-256; implementation and external gates remain open. | pending | Terra |
