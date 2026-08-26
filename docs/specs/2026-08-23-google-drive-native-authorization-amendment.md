---
version: "0.1.0b"
created_at: "2026-08-23T16:47:01+07:00,ATHER"
last_update: "2026-08-23T16:54:34+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "technical-design"
  scope: "HIGH-risk amendment to FUNG Google Drive native authorization"
---

# Google Drive Native Authorization Amendment

## 1. Document control

| Field | Value |
|---|---|
| Parent specification | `docs/specs/2026-08-23-phase-4-google-drive-oauth-iam-handshake-spec.md` |
| Trigger | Terra review of commit `617eba0` |
| Risk | **HIGH** — session/device authority, proof-of-possession, provider token custody, restore authorization |
| Status | Candidate; implementation blocked pending Boss approval |
| Proposed design | Revised Design A: server-verified, native-only authorization context |

This amendment supersedes the parent specification only for native
authorization, device proof, restore intent, OAuth cancellation, and trusted URL
opening. Existing encryption, appDataFolder, Genesis restore, and external-gate
boundaries remain unchanged.

## 2. Problem statement

The local Google Drive implementation passes automated tests but accepts
WebView-supplied user/device values as keyring/provider authority, checks OAuth
cancellation outside the token-persistence transition, and grants an
unrestricted WebView URL opener. These are local security defects, not external
UAT gaps.

## 3. Decision proposal

Adopt revised Design A:

1. The WebView supplies only an ephemeral current Supabase session proof and a
   requested operation. It supplies no authoritative user, device, connection,
   keyring-slot, permission, provider origin, or restore-confirmation value.
2. Native derives the Desktop public-key fingerprint from its local key and
   proves possession by signing a canonical short-lived authorization request.
3. `google-drive-authorize` verifies the Supabase session, derives `auth.uid()`,
   verifies the current user-owned non-revoked device/public key and signature,
   verifies active Drive connection/scope, and decides the named operation.
4. Native constructs an in-process `AuthorizedDriveContext` bound to provider,
   user, device fingerprint, connection, operation, expiry, nonce, and one
   invocation. It is never serialized to the WebView.
5. Every keyring read/write/delete, refresh, list, upload, and restore requires a
   fresh context. Denial occurs before keyring or provider access.
6. Restore also consumes a native one-time intent bound to the selected archive,
   clean target, expiry, and operation.

Design B signed bearer permits is rejected for this slice because it introduces
signing-key rotation and replay complexity without preventing a compromised
currently authenticated WebView from initiating an operation.

## 4. Authority model

```mermaid
sequenceDiagram
    participant W as WebView
    participant N as Native FUNG
    participant A as Supabase Auth
    participant Z as google-drive-authorize
    participant K as OS Keyring
    participant D as Drive appDataFolder

    W->>N: session proof + requested operation
    N->>N: derive fingerprint, nonce, timestamp, signature
    N->>Z: bearer session + signed canonical request
    Z->>A: verify session; derive auth.uid()
    Z->>Z: verify device owner, public key, revocation, signature
    Z->>Z: verify connection, exact scope, named operation
    Z-->>N: verified short-lived context data
    N->>N: create one-invocation AuthorizedDriveContext
    N->>K: keyring operation only after authorization
    N->>D: exact authorized Drive operation
    N->>N: consume context and restore intent
```

### Trust boundaries

| Value | Authority |
|---|---|
| User | Supabase verified session and `auth.uid()` |
| Device | Native key proof plus current user-owned non-revoked `devices` row |
| Provider connection | Server-verified user-owned active `google_drive` row with exact scope |
| Operation | Server decision for exactly `backup.write` or `backup.restore` |
| Keyring slot | Derived only from verified native context |
| Restore approval | Native archive/target-bound one-time intent |
| OAuth URL | Native trusted-provider registry and exact endpoint/path validation |

## 5. Security invariants

1. Frontend identity fields are never authorization inputs.
2. Missing/mismatched device public key, fingerprint, signature, session,
   connection, scope, operation, expiry, or nonce fails closed.
3. Authorization context is memory-only, non-serializable, one invocation, and
   never exposed through a Tauri response/event/log.
4. `backup.write` and `backup.restore` are independent server decisions. An
   active connection alone grants neither.
5. Restore requires a second native intent; bypassing UI confirmation cannot
   restore.
6. Metadata/audit endpoints cannot activate or assert an authoritative
   connection from a client request.
7. OAuth cancellation and completion share one terminal-state linearization
   point. Cancellation before irreversible keyring commit prevents persistence;
   completion after commit reports completed rather than cancelled.
8. No WebView command/capability can open arbitrary URLs.
9. Supabase authorization origin and provider URL registry are native-configured,
   never caller-supplied.
10. Provider tokens, Supabase tokens, codes, signatures, private keys, recovery
    phrases, plaintext archives, and raw responses never enter logs, Genesis,
    Supabase metadata, mobile payloads, or public DTOs.

## 6. Device key decision

Recommended: migrate the existing Ed25519 private key to OS-protected keyring
before it becomes Drive authorization proof, preserving the same public key and
fingerprint. Migration must verify keyring readback before removing the legacy
plaintext key and must fail without changing device identity.

Alternative for local beta only: retain the file-backed key with verified
current-user-only ACL and keep Drive authorization explicitly non-production.
This alternative does not satisfy production hardening and requires a later
migration gate.

Boss approved the recommended OS-protected keyring migration on 2026-08-23.
The file-backed local-beta alternative is rejected for this implementation.

## 7. Amended write scope

### Security lane

- `.env.example`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/native_auth.rs` (new)
- `src-tauri/src/device_identity.rs`
- `src-tauri/src/drive_oauth.rs`
- `src-tauri/src/backup.rs`
- `src-tauri/src/filesystem_backup.rs`
- `src-tauri/src/lib.rs`
- `src/components/AccountLoginPanel.tsx`
- `src/components/GoogleDrivePanel.tsx`
- `src/lib/authFlow.ts`
- `src/lib/googleDriveFlow.ts`
- `supabase/functions/google-drive-authorize/index.ts` (new)
- `supabase/functions/google-drive-metadata/index.ts`
- `tests/googleDriveContract.test.mjs`
- Parent specification and this amendment
- RCA and implementation report updates only

All Recording2/Smart Gift and `.tmp-transcript/**` paths remain forbidden. Other
Google Drive UI/status files return to a later evidence lane after this security
lane passes Terra.

## 8. Acceptance Criteria

| ID | Criterion |
|---|---|
| AC-GDA-01 | No Drive command accepts frontend user/device/connection/capability as authority |
| AC-GDA-02 | Missing/expired session and foreign/revoked/mismatched device fail before keyring/provider access |
| AC-GDA-03 | Missing/mismatched public key, forged signature, replayed/expired request, and fingerprint mismatch fail closed |
| AC-GDA-04 | Inactive/foreign/wrong-scope connection fails before keyring/provider access |
| AC-GDA-05 | Denied write and denied restore fail independently before keyring/provider access |
| AC-GDA-06 | AuthorizedDriveContext is memory-only, non-serializable, operation-bound, expiring, and one-use |
| AC-GDA-07 | Restore without valid native archive/target-bound intent is impossible even if WebView confirmation is bypassed |
| AC-GDA-08 | Browser metadata/audit calls cannot create or activate connection authority |
| AC-GDA-09 | Cancellation tests cover callback receipt, exchange start/end, and immediately before keyring commit |
| AC-GDA-10 | No frontend arbitrary `openUrl` route remains; untrusted URL input is rejected natively |
| AC-GDA-11 | Original encryption, digest, appDataFolder, and clean-target restore tests remain green |
| AC-GDA-12 | Secret scans show no prohibited credential, key, signature, recovery, or plaintext persistence/logging |

## 9. Success Criteria

1. New negative tests fail against `617eba0` and pass after the security fix.
2. Full Rust, frontend build, auth, Google Drive, backup, and capability tests
   pass after the fix.
3. Existing Supabase IAM remains the account authority; no second IAM store,
   FUNGWIRE authority, or Genesis authorization projection is introduced.
4. Keyring/provider spies prove authorization denial occurs before any protected
   operation.
5. Terra passes the security lane and later integration review.
6. Local evidence remains distinct from undeployed Supabase, real Google,
   clean-install, Android/FUNGWIRE, signing, and release proof.

## 10. Boss decision register

| ID | Decision | Approved selection | Status |
|---|---|---|---|
| D-GDA-01 | Design A revised vs signed permits | Revised Design A | approved |
| D-GDA-02 | Authorization unavailable behavior | Fail closed | approved |
| D-GDA-03 | Operation grant | Server decides write/restore per request; active connection is insufficient | approved |
| D-GDA-04 | Device private-key custody | Move existing key to OS keyring before Drive authorization | approved |
| D-GDA-05 | Restore confirmation | Native one-time archive/target-bound intent | approved |
| D-GDA-06 | Trusted URL opening | Native registry only; remove arbitrary WebView opener | approved |
| D-GDA-07 | Edge Function deployment | Code may be implemented after spec; deployment remains a separate external gate | approved |

## 11. Implementation and rollback gates

- Boss approved D-GDA-01 through D-GDA-07 with OS keyring on 2026-08-23.
- A fresh Luna worker implements the amended security lane.
- Terra reviews before any remaining UI/evidence lane starts.
- The existing commit `617eba0` remains unpushed and unmerged; it is not an
  accepted security baseline.
- If the amendment is rejected, do not add further code. The controller will
  propose a non-destructive Git disposition for `617eba0` separately.

## 12. External gates retained

- Google installed-app client/consent configuration.
- Supabase function deployment and production RLS verification.
- Real Google consent/upload/download/refresh/revoke.
- Clean-install reconnect and restore.
- Physical Android/FUNGWIRE delegation.
- Production signing, release, and merge approval.

## Version Diff

- `new -> 0.1.0b`: proposed revised Design-A native authorization, device proof,
  operation capability, restore intent, cancellation, and trusted URL boundaries
  after Luna discovery and Terra architecture review.
- `0.1.0b`: Boss approved D-GDA-01 through D-GDA-07 and selected OS keyring.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | beta | Boss approved revised Design A with OS keyring | working-tree | ATHER |
| 0.1.0b | 2026-08-23 | candidate | HIGH-risk Google Drive native authorization amendment for Boss approval | `617eba0` | ATHER |
