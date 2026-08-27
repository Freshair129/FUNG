---
version: "0.3.0b"
created_at: "2026-08-24T10:25:00+07:00,ATHER"
last_update: "2026-08-26T23:51:37+07:00,ATHER,888aded"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "system-design"
  scope: "Durable enrollment-proof nonce and native PKCE remediation"
  risk: "HIGH"
---

# Enrollment Proof Nonce and Native PKCE Amendment

## Status and authorization boundary

Boss approved D-GDA3-01 through D-GDA3-03 against the recorded candidate bytes.
The forward migration and native/Edge contract are implemented and accepted as
local/static evidence through the later D-GDA6 review chain. This approval does
not authorize Supabase/Edge deployment, Google provider operation, device UAT,
release, promotion, or production use. This amendment changes no operator
ownership, device class, grant issuer, or deployment policy from the approved
authority/schema amendment.

## Evidence and root cause

Terra S2 review `7bc84c1` found two local P0 defects:

1. Native login does not own PKCE challenge/verifier and token exchange.
2. The enrollment proof is opaque and cannot be independently verified by the
   Edge/server path.

Luna S2-F1 discovery then confirmed that
`pending_device_enrollments` has no proof nonce or expiry columns, is unique
only on `(user_id, public_key_fingerprint)`, and
`create_device_enrollment_request(...)` has no atomic nonce reservation. An
Edge-only signature check would therefore remain replayable.

## Proposed design

### 1. Native-owned PKCE

Native generates a cryptographically random verifier and S256 challenge,
constructs the exact authorization URL, owns callback listener/state, exchanges
the code using the verifier, erases verifier/code on every terminal path, and
returns only a typed session result. WebView code never stores the verifier or
performs the code exchange.

This inherits the full approved native-login hard gate: exact native-owned
origin/redirect/port/path/state, one request/listener/callback, duplicate and
additional parameter rejection, timeout/cancel/replay rejection, and secret
erasure on success, denial, timeout, cancellation, malformed callback, exchange
failure, and application shutdown. Native derives the account user ID from the
token response and a TLS-authenticated `/auth/v1/user` lookup; browser input
cannot supply it.

### 2. Exact typed enrollment proof envelope

Native signs one canonical versioned envelope containing:

- `version = 1`
- `operation = device.enrollment.request`
- verified account user ID
- public key and fingerprint
- normalized platform and device label
- `issued_at`, `expires_at`
- 256-bit random nonce

The signature command is operation-specific; no generic signing oracle or raw
private key is exposed.

Canonical signature input is exact bytes:

1. ASCII domain prefix `FUNG\0DEVICE_ENROLLMENT\0V1\0`.
2. Fields in this fixed order: user UUID, public key, fingerprint, platform,
   device label, issued-at, expires-at, nonce.
3. UUID is 16 raw RFC-4122 bytes; Ed25519 public key and nonce are each 32 raw
   bytes; fingerprint is 32 raw SHA-256 bytes of the public key; timestamps are
   signed 64-bit Unix milliseconds in network byte order.
4. Platform is one lowercase ASCII enum value. Device label is UTF-8 NFC,
   trimmed, with control characters rejected and a 1–80 byte limit.
5. Variable-length platform and label values are prefixed by unsigned 16-bit
   network-byte-order lengths. No delimiter, JSON, locale conversion, optional
   field, or alternate encoding is accepted.
6. Transport fields use base64url without padding; the signature is Ed25519 over
   the exact concatenation above.

### 3. Durable one-use nonce-hash reservation

Before pending enrollment is created, the server must atomically reserve the
proof nonce in PostgreSQL. The reservation and pending request creation occur
in one fixed-search-path transaction/function. A unique constraint permits one
winner; replay returns a durable denial and cannot update an existing pending
request.

Use a new forward-only migration
`20260824000000_w1_enrollment_proof_nonce.sql`; never edit migration history.
Create an append-only reservation table keyed by globally unique
`nonce_hash = SHA-256(raw_nonce)` with user, fingerprint, envelope hash,
issued/expires timestamps, request ID, and decision. No client role has direct
write/update/delete. Rows are retained indefinitely for W1; a later retention
change requires a separately approved policy.

The security-definer function inserts the reservation and pending request in
one transaction. Unique conflict returns `proof_replayed` without updating the
reservation, pending request, device, or audit authority. If pending creation
fails, the whole transaction rolls back, including the reservation; retry is
allowed only because no state committed. A successful transaction permanently
consumes the nonce hash. Audit remains non-authoritative.

### 4. Verification order

The Edge handler derives user identity from the verified session, validates the
exact envelope shape and canonical bytes, checks public-key/fingerprint match,
Ed25519 signature, operation, user binding, issued-at/expiry skew, platform and
label binding, then calls the atomic database function. Denial occurs before
pending-state mutation. Audit is not replay authority.

## Proposed schema/function changes — isolated fix cycle

- Add immutable proof metadata to pending enrollment: version, operation,
  nonce hash, issued-at, expires-at, canonical envelope hash, and signature.
- Add the append-only globally unique nonce-hash reservation table in the new
  migration above.
- Extend `create_device_enrollment_request(...)` or replace it with one
  schema-qualified fixed-search-path function that reserves nonce and creates
  pending state atomically.
- Revoke direct execute from `PUBLIC`, `anon`, and unapproved roles; retain only
  the reviewed Edge/server caller path.
- Keep Boss bootstrap approval separate; a valid proof creates only `pending`
  and never `drive_trusted`.
- Deny replay, expiry, future-issued proof, tampering, foreign user, wrong
  operation/platform/label, and revoked/legacy key cases.

## Proposed implementation write set

- `supabase/migrations/20260824000000_w1_enrollment_proof_nonce.sql` (new)
- `supabase/functions/device-enrollment/index.ts`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`
- `src-tauri/src/native_auth.rs`
- `src-tauri/src/lib.rs`
- `src/lib/authFlow.ts`
- `src/lib/authParse.ts`
- `src/components/AccountLoginPanel.tsx`
- `tests/authFlow.test.mjs`
- implementation and Terra review reports only

No pairing/mobile/Drive-panel path, project ref, deployment, or external system
is authorized.

## Acceptance criteria

| ID | Criterion |
|---|---|
| AC-GDA3-01 | Native owns verifier/challenge/URL/listener/state/code exchange and erases secrets on every terminal path |
| AC-GDA3-02 | WebView cannot supply URL/redirect/state/verifier and cannot exchange the authorization code |
| AC-GDA3-03 | Proof envelope is canonical, versioned, operation-specific, account/device/label/platform/time/nonce bound, and Ed25519 verified |
| AC-GDA3-04 | Proof nonce reservation and pending creation are one transaction with one durable winner |
| AC-GDA3-05 | Replay, expiry, skew, tamper, foreign user, wrong operation/label/platform, and revoked key deny before pending mutation |
| AC-GDA3-06 | Valid proof creates only `pending`; Boss approval remains the only bootstrap promotion path |
| AC-GDA3-07 | PostgreSQL 17 executable tests cover first-use, replay, race, denial no-mutation, privileges, search path, and rollback |
| AC-GDA3-08 | Native/Rust/auth/Edge/build/secret/path tests pass and live staging/provider/device gates remain explicit |

## Decisions for Boss approval

### Inherited decisions — unchanged

| Existing decision | Inherited effect |
|---|---|
| D-GDA2-01 / -02 / -03 | Boss-only bootstrap; no automated trust promotion |
| D-GDA2-04 / -05 / -06 | Separate grants; Windows trust split; fail-closed re-enrolment |
| D-GDA2-07 | Full native-owned login boundary and adversarial lifecycle matrix |
| D-GDA2-08 | Forward-only migration history; no editing applied or committed prior migration |
| D-GDA2-09 / -10 | Reproducibility and per-project deployment/evidence ownership remain unchanged |

### New decisions

| ID | Decision | Proposed value | Status |
|---|---|---|---|
| D-GDA3-01 | Canonical proof encoding | Exact binary layout and Ed25519 domain separation defined above | approved; implemented locally |
| D-GDA3-02 | Replay authority and retention | New append-only global nonce-hash table; indefinite W1 retention; atomic pending creation | approved; implemented locally |
| D-GDA3-03 | Fix-cycle boundary | One new forward-only schema migration plus listed native/Edge/tests; fresh Luna and Terra; deployment remains separately gated | approved; local/static review complete |

## Version Diff

- `new -> 0.1.0b`: proposed durable enrollment nonce and native PKCE amendment
  after S2 P0 findings and schema discovery.
- `0.1.0b -> 0.2.0b`: added exact canonical bytes, immutable global nonce-hash
  semantics, new forward-only migration, inherited-decision crosswalk, and full
  native PKCE adversarial inheritance after Terra FAIL.
- `0.2.0b -> 0.3.0b`: truth-synced Boss approval, local implementation, and
  later D-GDA6 acceptance without promoting external or production readiness.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.3.0b | 2026-08-26 | beta | D-GDA3 approved and implemented locally; external/deployment gates remain open | `888aded` | ATHER |
| 0.2.0b | 2026-08-24 | candidate | Corrected amendment after Terra architecture FAIL | `11f8b52` | ATHER |
| 0.1.0b | 2026-08-24 | candidate | Proposed schema/native remediation for S2 P0 findings | `7bc84c1` | ATHER |
