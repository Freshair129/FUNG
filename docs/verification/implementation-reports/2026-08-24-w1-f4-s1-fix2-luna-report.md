---
version: "0.1.0b"
created_at: "2026-08-24T07:04:43+07:00,Luna 5.6"
last_update: "2026-08-24T07:04:43+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S1-F2 P0-03 revoked connection reactivation fix"
  base_head: "41861d8"
  implementation_commit: "9ef676b"
---

# W1-A-F4-S1-F2 — Luna Fix Report

## Status

`DONE_WITH_CONCERNS` — Terra P0-03 is fixed locally within the exact three-path
implementation/test allowlist and committed as `9ef676b`. The denied activation
now produces a durable denied reservation/decision and cannot clear the prior
connection revocation. Live PostgreSQL execution, approved staging, and a fresh
independent Terra review remain external gates.

No migration, project link, deployment, push, merge, PR, deletion, or external
message was performed.

## Root cause

The authorization RPC locked the exact `google_drive` connection, but its
required-operation map only covered backup operations. Therefore a signed
`connection.activate` request against an existing revoked connection left
`v_denial_code` null, wrote an allowed decision, and reached the activation
upsert that set `status = 'active'` and `revoked_at = null`.

## Exact fix

- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql:362-368`
  now denies `connection.activate` when the locked exact-provider row is
  present and either `status = 'revoked'` or `revoked_at IS NOT NULL`, using
  denial code `connection_revoked`.
- The existing row-lock order and single reservation/decision transaction are
  unchanged. The activation upsert remains behind `if v_authorized` at
  `:441-464`, so a denied request cannot mutate connection state or grants.
- `supabase/tests/w1_authority_schema.sql:232-388` adds executable database
  evidence that seeds a trusted test device, active exact-provider connection,
  and both operation grants; revokes the connection; calls
  `connection.activate`; asserts the durable denied decision; and compares
  status, `revoked_at`, scopes, owner, and grant rows before/after. The outer
  transaction ends in `ROLLBACK`.

## TDD evidence

### RED

The adversarial contract was added before the implementation at
`tests/w1AuthoritySchema.test.mjs:136-148`.

```text
Command: node --test tests/w1AuthoritySchema.test.mjs
Result: 6 tests, 5 pass, 1 fail
Failure: W1 revoked connection activation is denied without reactivation
         (missing revoked-activation denial branch)
```

### GREEN

```text
Command: node --test tests/w1AuthoritySchema.test.mjs
Result: 6 tests, 6 pass, 0 fail
```

## Verification commands

| Check | Result |
|---|---|
| `npm run test:google-drive` | 5/5 passed |
| `npm run test:auth` | 5/5 passed |
| `npm run test:backup-flow` | 17/17 passed |
| `npm run test:device-reconcile` | 6/6 passed |
| `npx tsc --noEmit` | Passed |
| `npm run build` | Passed; Vite transformed 1,764 modules |
| `deno check --frozen --node-modules-dir=manual` for the three W1 Edge entrypoints | Passed |
| `deno fmt --check --unstable-sql` for the three Edge files, two migrations, and SQL evidence | Passed; 6 files checked |
| `git diff --check` | Passed |
| Implementation path audit | Passed; exactly three paths in `9ef676b` |

The database-level SQL evidence was committed but not run. Exact environment
evidence: `psql --version` and `supabase --version` both returned PowerShell
“The term is not recognized” errors; `docker ps` showed no containers and
`docker image ls ... postgres supabase/postgres` showed no local image. No live
PostgreSQL migration or SQL execution was attempted.

## Commits and changed paths

Implementation commit: `9ef676b494fd76ac51b1c58ba62e88344fdf8af7`, direct child
of base `41861d88a4068ed01ae3261babbec5aad4852b6f7`.

Exactly these three paths are in the implementation commit:

- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

The separate report path is:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix2-luna-report.md`

Pre-existing dirty/untracked paths were preserved and were not staged:

- `docs/Desktop/08-real-progress.md`
- `docs/plans/2026-08-13-phase-4-google-drive-backup-mobile-account.md`
- `src/components/BackupPanel.tsx`
- `src/web/AccountSettings.tsx`
- `supabase/README.md`
- `.brain/rca/2026-08-23-google-drive-core-security-gate-failures.md`
- `.tmp-transcript/`
- `docs/plans/2026-08-23-fung-luna-terra-multiagent-workflow.md`
- `docs/plans/2026-08-23-recording2-smart-gift-catalog-task-breakdown.md`
- `docs/specs/2026-08-23-google-drive-authority-schema-amendment.md`
- `docs/specs/2026-08-23-google-drive-native-authorization-amendment.md`
- `src/components/GoogleDrivePanel.css`

## Remaining external gates

1. Run the committed SQL evidence in an explicitly approved staging PostgreSQL
   project and prove migration rollback, RLS, table/function privileges,
   `proconfig`, and Data API behavior for `PUBLIC`, `anon`, `authenticated`,
   and `service_role`.
2. Run at least 50 concurrent signed requests through separate Edge workers,
   including connection revocation races and revoked-then-`connection.activate`,
   and prove zero state resurrection or protected operation after revocation.
3. Obtain a fresh independent Terra review before any staging migration,
   Edge deployment, promotion, merge, or release.
4. Verify deployed pinned Edge revisions, JWT/JWKS configuration, project
   reference isolation, and clean-checkout Deno reproduction.
5. Google consent/provider behavior, native keyring behavior, clean-install
   restore, Android/FUNGWIRE evidence, signing, and production readiness remain
   separate and unverified.

## Version Diff

- `new -> 0.1.0b`: recorded the W1-A-F4-S1-F2 P0-03 revoked-activation fix,
  RED/GREEN evidence, exact commits/paths, and open live security gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Fixed Terra P0-03: revoked exact-provider connection activation is denied without state resurrection. | `9ef676b` | Luna 5.6 |
