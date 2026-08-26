---
version: "0.1.0b"
created_at: "2026-08-24T05:59:16+07:00,Luna 5.6"
last_update: "2026-08-24T05:59:16+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S1 schema and Edge authority foundation"
  implementation_commit: "304b1e4"
---

# W1-A-F4-S1 — Luna implementation report

## Status

`DONE_WITH_CONCERNS` — the approved local, project-agnostic implementation is
committed at `304b1e4` on `codex/backlog-truth-sync`. No migration, Edge
deployment, project link, push, merge, PR, release, or external message was
performed.

The database-owner boundary is expressible without a project-specific runtime
assumption: bootstrap and rebind have no execute grant for `PUBLIC`, `anon`,
`authenticated`, or `service_role`, and both functions also check the current
database owner before consuming a request.

## Root-cause mapping

| Root cause | S1 correction |
|---|---|
| Browser/WebView clients could directly create, mutate, or delete device rows. | Existing rows default to `legacy`; direct device DML is revoked; enrollment, pairing-only registration, and soft revocation use server-only RPCs. |
| A device row did not express an independent Drive authority boundary. | The exact Windows `drive_trusted` predicate requires an approved source, an unrevoked row, a public key, and a matching SHA-256 fingerprint. |
| Trusted-key rebind could become in-place key mutation. | Database-owner-only `approve_rebind_enrollment(request_id, old_device_id)` locks and soft-revokes the old row, then consumes a new request into a new `approved_rebind` identity. |
| Connection state could be treated as backup authority. | `backup.write` and `backup.restore` are independent operator-only grant rows; connection and device revocation invalidate active grants, and reconnection does not resurrect them. |
| Isolate-local replay state and audit read-then-insert were TOCTOU-prone. | A unique durable nonce reservation uses one `INSERT ... ON CONFLICT (nonce) DO NOTHING RETURNING` primitive; audit remains informational. |
| Edge dependency resolution was unpinned. | All three Edge entrypoints use `npm:@supabase/server@1.4.1`, with the reviewed root `deno.lock`. |

## Exact committed implementation paths

Commit `304b1e4` contains exactly these paths:

- `deno.lock`
- `supabase/README.md` — only the project-agnostic project-ref correction and the new W1 authority documentation were staged; the pre-existing metadata/deployment hunk was not staged.
- `supabase/functions/device-enrollment/index.ts`
- `supabase/functions/google-drive-authorize/index.ts`
- `supabase/functions/google-drive-metadata/index.ts`
- `supabase/migrations/20260823000000_w1_device_enrollment_authority.sql`
- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

The required report is intentionally being committed separately so this report
can identify the implementation commit exactly.

## RED evidence

Before implementing the migrations or Edge changes, the new contract test was
run against base `db0b949c2e899575156d07389afb2b973545da4e`:

```text
node --test tests/w1AuthoritySchema.test.mjs
tests 4, pass 1, fail 3
```

The failing assertions identified the missing enrollment migration, missing
operation-grant/reservation migration, and the unpinned authorizer with
isolate-local replay state. The SQL-evidence file assertion passed because the
read-only evidence file itself was created before implementation.

## GREEN evidence

| Check | Result |
|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | 4/4 passed |
| `npm run test:google-drive` | 5/5 passed |
| `npm run test:auth` | 5/5 passed |
| `npm run test:device-reconcile` | 6/6 passed |
| `deno check --frozen --node-modules-dir=auto` for all three Edge entrypoints | Passed |
| `deno fmt --unstable-sql --check` for three Edge files, two migrations, and SQL evidence | Passed; 6 files checked |
| `npm run build` | Passed; TypeScript plus Vite, 1,764 modules transformed |
| `git diff --check --cached` and final static diff checks | Passed |
| Secret/project-reference/log scan over implementation and evidence files | Passed; no concrete project ref, secret value, token field, provider response, bearer literal, or console logging found |
| Staged path audit | Passed; exactly 9 implementation paths, no unrelated path |

`supabase/tests/w1_authority_schema.sql` is committed as read-only staging
evidence. It checks RLS, table privileges for `PUBLIC`/`anon`/`authenticated`/
`service_role`, bootstrap/rebind/operator function privileges, fixed function
configuration, nonce uniqueness, and the atomic reservation definition. It was
not executed against a live database in this task.

## Acceptance-criteria evidence boundary

| Criterion | Local evidence | External boundary |
|---|---|---|
| AC-GDA2-01 | Device DML revocation, owner-only bootstrap/rebind, and no enrollment Edge approval path are present and contract-tested. | Live role ACL/RLS attempts remain pending. |
| AC-GDA2-02 | Edge and SQL exact predicate exclude legacy, pending, pairing-only, revoked, non-Windows, and non-approved sources. | Live protected-operation attempts remain pending. |
| AC-GDA2-03 | Request expiry, fingerprint mismatch, reuse, row locking, and database-owner checks are implemented. | Live concurrent approval proof remains pending. |
| AC-GDA2-04 | Rebind revokes the selected old trusted identity and inserts a new approved identity; no trusted key is updated in place. | Live rebind ceremony remains pending. |
| AC-GDA2-05 | Separate write/restore grants, exact-scope connection checks, and connection/device revocation triggers are implemented. | Live grant-matrix and reconnection proof remains pending. |
| AC-GDA2-06 | Authorizer maps archive read/restore to the `backup.restore` server grant; existing native restore-intent tests remain green. | Real keyring/provider ordering remains outside S1 and unverified. |
| AC-GDA2-07 | Durable unique nonce plus atomic conflict reservation is implemented and statically asserted. | The required 50-worker staging run remains pending. |
| AC-GDA2-08 | Metadata remains token-free and has no enrollment/grant/reservation/approval path; authoritative tables are default-deny. | Live privilege and audit-isolation proof remains pending. |
| AC-GDA2-10 server portion | Pairing RPCs enforce caller-owned, non-revoked device IDs, fixed search paths, and pairing-only transitions. | Live foreign-device/RPC attempts remain pending. |
| AC-GDA2-11 server portion | Authorizer returns separate connection, write-grant, and restore-grant state without provider tokens. | Deployed-function/provider behavior remains pending. |
| AC-GDA2-12 server portion | Exact Edge imports, frozen lockfile, and local build/type checks pass. | Clean-checkout installation/rebuild remains pending. |
| AC-GDA2-14 server portion | Read-only SQL evidence covers privileges, RLS, function configuration, and reservation structure. | Live staging execution remains pending. |

## Dirty/untracked path handling

The following pre-existing paths were preserved and were not staged or
committed:

- `docs/Desktop/08-real-progress.md`
- `docs/plans/2026-08-13-phase-4-google-drive-backup-mobile-account.md`
- `src/components/BackupPanel.tsx`
- `src/web/AccountSettings.tsx`
- `.brain/rca/2026-08-23-google-drive-core-security-gate-failures.md`
- `.tmp-transcript/**`
- `docs/plans/2026-08-23-fung-luna-terra-multiagent-workflow.md`
- `docs/plans/2026-08-23-recording2-smart-gift-catalog-task-breakdown.md`
- `docs/specs/2026-08-23-google-drive-authority-schema-amendment.md`
- `docs/specs/2026-08-23-google-drive-native-authorization-amendment.md`
- `src/components/GoogleDrivePanel.css`

The pre-existing dirty Google Drive metadata/deployment documentation hunk in
`supabase/README.md` remains worktree-only. Only the new W1 section and the
project-agnostic project-ref correction were staged.

## External gates and concerns

- Supabase CLI and `psql` were unavailable locally; no live SQL syntax,
  migration, RLS, grant, policy, or function-ACL execution was attempted.
- No project reference was selected. `supabase db push` and Edge deployment
  were not run.
- Boss/database-owner bootstrap and rebind, independent grant issuance, and
  50-worker identical-nonce testing require a separately approved staging
  project and operator session.
- Real Google consent, provider upload/download/revoke, native keyring access,
  clean-install restore, clean-checkout verification, Android/FUNGWIRE proof,
  and production readiness remain external gates.
- Local passing tests and a successful build do not establish staging or
  production authorization.

## Version Diff

- `new -> 0.1.0b`: recorded the local W1-A-F4-S1 schema/Edge implementation,
  RED/GREEN evidence, exact path boundary, and external-gate limitations.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | W1-A-F4-S1 local schema and Edge foundation report | `304b1e4` | Luna 5.6 |
