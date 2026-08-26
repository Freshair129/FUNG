---
version: "0.1.0b"
created_at: "2026-08-23T19:16:58+07:00,ATHER"
last_update: "2026-08-24T05:17:54+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "technical-design"
  scope: "HIGH-risk authority and schema amendment after W1-A-F2 Terra failure"
---

# Google Drive Authority and Schema Amendment

## 1. Document control

| Field | Value |
|---|---|
| Parent | `2026-08-23-google-drive-native-authorization-amendment.md` |
| Trigger | Terra FAIL on `7d6c045`; architecture WARN for amendment drafting |
| Risk | HIGH — device authority, RLS/grants, migrations, replay, OAuth |
| State | Approved for local implementation; deployment remains unauthorized |
| Implementation base | `db0b949c2e899575156d07389afb2b973545da4e` |

This amendment does not revoke the approved OS-keyring decision. It adds the
server authority and schema controls that the earlier amendment lacked.

## 2. Problem statement

Local tests passed, but independent review proved that the server could accept
a device key enrolled by the authenticated WebView itself. The implementation
also grouped write and restore behind the same active connection and used a
non-atomic replay check. Therefore commit `7d6c045` is not an acceptable
security baseline and must not be deployed, pushed, merged, or promoted.

## 3. Proposed architecture

### 3.1 W1 root of trust

W1 uses a manual, Boss-controlled database-owner bootstrap ceremony:

1. Native creates the Ed25519 key in the OS keyring and signs a canonical,
   short-lived pending-enrollment challenge.
2. The verified Supabase session may create only a non-authoritative pending
   request. It cannot write or mutate a trusted `devices` row.
3. Boss verifies the account, request ID, fingerprint, device label/platform,
   and expiry out-of-band against the physical desktop and the server pending
   row.
4. Boss invokes `approve_bootstrap_enrollment(request_id)` from a controlled
   database-owner/operator session. The function has no execute grant for
   `PUBLIC`, `anon`, `authenticated`, service-role Edge Functions, or the Data
   API.
5. One transaction consumes the request and creates the `drive_trusted` device.
   Reuse, expiry, mismatch, or concurrent approval fails closed.

This operator ceremony is the W1 operational root of trust, not a second
application IAM. OS-keyring Ed25519 possession is not hardware attestation.
Automated/self-service first-device enrollment requires a later approved TPM,
Secure Enclave, Android Keystore, WebAuthn, or equivalent attestation design.

Additional Drive desktops use the same Boss ceremony in W1. Generic WebView-
invokable signing is not an approval mechanism.

### 3.2 Device classes

One Supabase `devices` store remains authoritative for account-owned device
records. The server controls these states:

- `legacy`: existing row; never accepted for Drive.
- `pairing_only`: desktop/mobile/FUNGWIRE registration; never accepted for Drive.
- `drive_trusted`: manually approved Windows desktop.
- `revoked`: denied for every protected operation.

The exact Drive predicate is:

```text
platform = windows
AND authority_state = drive_trusted
AND enrollment_source IN (boss_bootstrap, approved_rebind)
AND revoked_at IS NULL
AND public_key IS NOT NULL
AND sha256(public_key_bytes) = public_key_fingerprint
```

FUNGWIRE/Noise, Genesis, local paired-device state, LAN endpoints, pairing
sessions, Android rows, and client-authored audit events never satisfy this
predicate and never become a second authorization store.

### 3.3 Independent operation grants

- An active exact-scope Google Drive connection grants no backup operation.
- `backup.write` and `backup.restore` are separate, default-deny server rows.
- W1 grants and revokes them only through the controlled operator session.
- `backup.read`/archive listing requires an active `backup.restore` grant.
- Restore requires both the server `backup.restore` grant and the existing
  native one-time archive/target-bound intent.
- Connection or device revocation immediately invalidates both operations.
  Reconnection does not resurrect old grants.

A later self-service grant flow requires a separately approved native OS user-
presence or platform-attestation design.

### 3.4 Atomic replay reservation

Every signed authorization request uses one durable database reservation with a
unique nonce/request key. The server performs `INSERT ... ON CONFLICT DO
NOTHING RETURNING` in one transaction. Only the returned winner continues.
Audit rows record outcomes but are never replay locks. Isolate-local maps and
audit read-then-insert checks are removed.

### 3.5 Native-owned account login

Native creates `{request_id, exact loopback port, path, state, expiry}`, builds
the Google/Supabase URL from native configuration, opens it, and accepts exactly
one matching callback. Caller-supplied URLs, arbitrary ports, duplicate or
additional credential parameters, wrong state/path/listener, timeout,
cancellation, and replay fail closed. WebView receives only a typed result tied
to the native request ID.

## 4. Forward-only migrations

### 4.1 `20260823000000_w1_device_enrollment_authority.sql`

- Add server-controlled authority state/source and timestamps; mark every
  existing row `legacy` without automatic promotion.
- Add immutable pending-enrollment requests with expiry, native proof,
  approver provenance, consumption, and anti-rebind uniqueness.
- Revoke direct `devices` `INSERT`, `UPDATE`, and `DELETE` from `PUBLIC`, `anon`,
  and `authenticated`; retain narrow owner read only.
- Use server-owned soft revocation. Rebind revokes the old identity and consumes
  a newly approved request; it never mutates a trusted key in place.
- Add fixed/search-path, schema-qualified enrollment/rebind/revoke functions
  with explicit `PUBLIC` execute revocation and narrow grants.
- Keep the Boss bootstrap function database-owner-only and unavailable through
  Edge/Data API roles.
- Harden retained pairing RPC ownership/non-revocation checks. Pairing may
  produce only `pairing_only`.
- Audit `handle_new_user` and every `SECURITY DEFINER` function for fixed
  `search_path`, schema qualification, and explicit execute posture.

### 4.2 `20260823000001_w1_drive_authorization_policy.sql`

- Add independent `backup.write`/`backup.restore` operation grants with status,
  actor provenance, timestamps, and no direct client mutation.
- Add unique durable authorization reservations and a single atomic reserve
  function returning exactly one winner.
- Add server-created authorization decision/audit linkage; client audit remains
  informational.
- Enable RLS and default-deny grants on every new table/function.
- Abort preflight on incompatible duplicate state or unexpected privileges;
  never delete rows or convert active connections into grants.

Both migrations are transactional, forward-only, and applied to staging only
after read-only schema/policy/grant preflight. Rollback after commit uses a
reviewed compensating migration or deny-only application state. It never
restores browser device writes, legacy acceptance, permissive execute grants,
or audit-based replay handling.

## 5. Exact implementation write set

### Documentation and schema

- This amendment.
- `docs/specs/2026-08-23-google-drive-native-authorization-amendment.md`
- `docs/specs/2026-08-23-phase-4-google-drive-oauth-iam-handshake-spec.md`
- `supabase/README.md` after reconciling its existing dirty owner change.
- `supabase/migrations/20260823000000_w1_device_enrollment_authority.sql` (new).
- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql` (new).
- `supabase/tests/w1_authority_schema.sql` (new).

### Edge and reproducibility

- `supabase/functions/device-enrollment/index.ts` (new).
- `supabase/functions/google-drive-authorize/index.ts`.
- `supabase/functions/google-drive-metadata/index.ts`.
- `deno.lock` with exact pinned Edge imports.

### Native and UI

- `src-tauri/src/device_identity.rs`.
- `src-tauri/src/native_auth.rs`.
- `src-tauri/src/lib.rs`.
- `src-tauri/src/drive_oauth.rs`.
- `src/lib/authFlow.ts`.
- `src/lib/authParse.ts`.
- `src/lib/googleDriveFlow.ts`.
- `src/lib/deviceReconcile.ts`.
- `src/components/AccountLoginPanel.tsx`.
- `src/components/DevicePairingPanel.tsx`.
- `src/components/GoogleDrivePanel.tsx`.
- `src/components/GoogleDrivePanel.css`.
- `src/mobile/MobileApp.tsx`.
- `src/web/Dashboard.tsx`.

### Tests and reports

- `tests/googleDriveContract.test.mjs`.
- `tests/authFlow.test.mjs`.
- `tests/deviceReconcile.test.mjs`.
- `tests/w1AuthoritySchema.test.mjs` (new).
- RCA and implementation/review reports for this lane only.

No change is authorized for Cargo/package manifests, Tauri capabilities,
`backup.rs`, `filesystem_backup.rs`, Recording2, Smart Gift,
`.tmp-transcript/**`, or unrelated UI paths unless a later reviewed dependency
proves it necessary.

## 6. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-GDA2-01 | `anon`, authenticated WebView, another owner, Edge, and Data API cannot create/mutate/delete/rebind/approve a `drive_trusted` device |
| AC-GDA2-02 | Pending, pairing-only, legacy, revoked, Android, FUNGWIRE, and Genesis records cannot obtain Drive authorization |
| AC-GDA2-03 | Boss bootstrap is database-owner-only; mismatch, expiry, reuse, cross-user, and concurrent approval fail with one winner at most |
| AC-GDA2-04 | Trusted-key rebind revokes the old identity and requires a new approved request; no in-place public-key mutation exists |
| AC-GDA2-05 | Connection-only denies write and restore; write-only and restore-only grants remain independent and revocable |
| AC-GDA2-06 | Restore requires both its server grant and one-use native archive/target intent before keyring/provider access |
| AC-GDA2-07 | Every signed request uses one atomic durable reservation; at least 50 concurrent identical requests produce one winner at most |
| AC-GDA2-08 | Metadata/client audit cannot enroll, grant, reserve, activate, revoke, or authorize protected state |
| AC-GDA2-09 | Native account login rejects arbitrary URL/port/path/state, duplicate/additional credential parameters, cross-listener callback, timeout, cancellation, and replay |
| AC-GDA2-10 | Pairing RPCs verify caller-owned non-revoked device IDs, fixed search path, and explicit function privileges |
| AC-GDA2-11 | Server status distinguishes connection, write grant, and restore grant without exposing or relying on provider tokens |
| AC-GDA2-12 | Exact Edge versions, committed frozen lockfile, tracked stylesheet, and clean-checkout build are reproducible |
| AC-GDA2-13 | Secure OS-keyring migration/readback/delete receives behavioral tests; file helper/source pattern alone is insufficient |
| AC-GDA2-14 | RLS, table/function privileges, function configuration, secret scan, and deny-before-keyring/provider spies pass in staging |
| AC-GDA2-15 | Existing encryption, digest, appDataFolder, cancellation, and clean-target restore tests remain green |

## 7. Success criteria and evidence boundary

1. Fresh Luna implements only the approved write set and commits a reviewable
   patch without rewriting `617eba0`, `7d6c045`, or `db0b949`.
2. Terra passes code, schema, privilege, adversarial concurrency, and clean-
   checkout review before integration.
3. Local passing tests remain local evidence. Staging migration/RLS/grant proof,
   real Google OAuth/provider proof, clean-install keyring migration, physical
   Android/FUNGWIRE, signing, and release remain separate external gates.
4. Old clients and absent schema/functions fail closed; no compatibility mode
   restores rejected browser authority.
5. No migration, deployment, push, merge, PR, or release occurs from this
   approval unless separately authorized.

## 8. Decision register

| ID | Decision | Approved selection | Status |
|---|---|---|---|
| D-GDA2-01 | First-device authority | Manual database-owner-only Boss bootstrap | approved |
| D-GDA2-02 | Additional W1 Drive desktops | Same Boss bootstrap; self-service deferred | approved |
| D-GDA2-03 | Automated bootstrap | Deferred; platform attestation required before automation | approved |
| D-GDA2-04 | Operation grant issuer | Operator-only, separate write/restore grants | approved |
| D-GDA2-05 | Device split | Windows `drive_trusted`; Android/FUNGWIRE `pairing_only` | approved |
| D-GDA2-06 | Legacy/rebind policy | Fail closed, explicit re-enrolment, soft revoke, no auto-promotion | approved |
| D-GDA2-07 | Native login boundary | Native-owned exact URL/listener/request state | approved |
| D-GDA2-08 | Migration/rollback | Two forward-only migrations; deny-only/compensating rollback | approved |
| D-GDA2-09 | Reproducibility | Pin Edge import; commit `deno.lock` and panel CSS; clean checkout | approved |
| D-GDA2-10 | External ownership | Bootstrap operator: Boss; Edge deployer: Boss; project applicability: all projects; RLS/grant evidence: Boss with Terra review gate | approved for implementation |

## 9. Approval and execution gate

Boss approved D-GDA2-01 through D-GDA2-10 on 2026-08-24. The implementation
must remain project-agnostic and fail closed for every project. `all projects`
does not authorize bulk deployment: exact Supabase project refs, per-project
read-only preflight, and a separate deployment approval are mandatory before
any migration or Edge deployment.

After this approval:

1. Fresh Luna receives a bounded implementation packet.
2. Terra performs an independent security review.
3. Failed hard gates return to fresh Luna fix cycles, maximum three.
4. Codex performs only final diff/path/evidence integration and writes no code.
5. External staging/deployment actions remain separately gated and require an
   enumerated project-ref manifest.

The existing commits `617eba0`, `7d6c045`, and `db0b949` remain unpushed and
unmerged. They are implementation evidence, not an accepted security baseline.

## Version Diff

- `new -> 0.1.0b`: proposed the server-owned device bootstrap, independent
  operation grants, atomic replay reservation, native login boundary, forward-
  only migrations, and reproducible clean-checkout gate after Terra review.
- `0.1.0b`: Boss approved all decisions for local, project-agnostic
  implementation; named Boss as bootstrap/deployment owner and Boss + Terra as
  the RLS/grant evidence gate. Bulk deployment remains unauthorized.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Approved local implementation for all-project applicability; deployment still gated | working-tree | ATHER |
| 0.1.0b | 2026-08-23 | candidate | HIGH-risk authority/schema amendment for Boss approval | `db0b949` | ATHER |
