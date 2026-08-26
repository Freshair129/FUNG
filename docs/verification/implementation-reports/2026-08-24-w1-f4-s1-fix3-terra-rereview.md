---
version: "0.1.0b"
created_at: "2026-08-24T07:44:39+07:00,Terra"
last_update: "2026-08-24T07:44:39+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Final independent Terra re-review of W1-A-F4-S1-F3"
  base_commit: "54ff343675d963e59d70fb52bfd32a543d4dc857"
  implementation_commit: "602050ad2dce65fdd147ec783644a7c46abcc08e"
  implementation_report_commit: "10d4a03211824464a2c108ee6fcf1527e584b8a9"
  prior_terra_addendum: "2026-08-24-w1-f4-s1-fix1-terra-rereview.md@0.2.0b"
---

# W1-A-F4-S1-F3 — Final Independent Terra Re-review

## Verdict

**WARN — all local hard gates PASS; external security and release gates remain
open.** Do not treat this as staging, deployment, Google-provider, production,
or release approval.

I independently reviewed implementation `602050a` and Luna report `10d4a03`
against `54ff343`, inspected the committed SQL and Node harness, and reran the
ordinary disposable PostgreSQL 17 gate. The prior Terra addendum at
`2026-08-24-w1-f4-s1-fix1-terra-rereview.md` remains unmodified and excluded
from this change. No new local P0, P1, or P2 finding was identified.

No implementation file, prior Terra addendum, deployment, migration against a
Supabase project, push, merge, PR, deletion, or external message was made by
this reviewer.

## Addendum reconciliation

| Prior finding | Result | Independent evidence |
|---|---|---|
| P0-03 — revoked `connection.activate` could clear revocation | **PASS locally** | The locked revoked-connection branch denies before the activation upsert (`20260823000001_w1_drive_authorization_policy.sql:363-369,446-470`). The PostgreSQL 17 test passed its durable denied-decision and exact before/after connection-and-grant assertions. |
| P0-04 — output/table-column ambiguity stopped the integrated RPC | **PASS locally** | Device, connection, both grant selectors, reservation replay selector, reservation update, and connection updates are alias-qualified (`:284-345,388-442,447-478`). An active exact-scope `backup.write` reached one durable allowed reservation/decision in the executable gate. |
| P1-01 — privilege/search-path evidence failed on PostgreSQL 17 | **PASS locally** | The evidence now stores `regprocedure` values and resolves `pg_proc.oid` (`supabase/tests/w1_authority_schema.sql:177-205`). The exact committed SQL evidence passed in PostgreSQL 17.11. |
| P2-01 — Node checks were source-pattern-only | **PASS locally** | `tests/w1AuthoritySchema.test.mjs` now starts a disposable `postgres:17-alpine` container, applies prerequisites and both W1 migrations, executes active/replay and committed-evidence SQL, then checks rollback cleanup. The ordinary suite passed 7/7 without a Docker skip. |

## Hard-gate ledger

| Gate | Result | Evidence boundary |
|---|---|---|
| PostgreSQL 17 executable gate with ordinary PL/pgSQL behavior | **PASS locally** | Docker Engine 29.6.1 and `postgres:17-alpine` (`PostgreSQL 17.11`) were available. `node --test tests/w1AuthoritySchema.test.mjs` passed 7/7. The reviewed migration contains no `SET plpgsql.variable_conflict` workaround. |
| Transactional migrations and active grant path | **PASS locally** | Both W1 migrations have explicit outer `BEGIN;` / `COMMIT;`; no static non-transactional DDL candidate was found. The executable gate applied both migrations and allowed active exact-scope `backup.write` through the grant lock, reservation, and decision path. |
| Durable one-winner nonce replay | **PASS locally** | The explicit nonce constraint is used by `ON CONFLICT ON CONSTRAINT`; the executable test asserts one reservation/decision and a repeat result of `authorization_replayed`. |
| Revoked activation cannot mutate authority | **PASS locally** | The committed SQL evidence seeds, revokes, activates, checks a durable `connection_revoked` denial, and compares status, timestamp, scope, owner, and grant snapshot before/after. |
| Every conflicting table reference is qualified | **PASS locally** | Source inspection confirms aliases on device, connection, grants, reservation insertion/replay/update, and connection transitions; the previously ambiguous active/replay execution now completes. |
| Privilege, RLS, and fixed search-path evidence uses OID semantics | **PASS locally** | Exact `supabase/tests/w1_authority_schema.sql` passed in the PostgreSQL 17 gate using `regprocedure`/OID lookup. |
| Rollback leaves no seeded authority rows | **PASS locally** | The executable gate asserts `0|0|0|0|0` for devices, connections, grants, reservations, and decisions after its rollback probe. |
| Original S1 regressions, format/type/build, scope, secrets, and dirty-work preservation | **PASS locally** | All commands below passed; reviewed commits meet the exact three-implementation-path plus one-report-path allowlist. |

## Commands and results

| Command / check | Result |
|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | **PASS — 7/7**, including the disposable PostgreSQL 17 migration, active/replay, committed-evidence, and rollback gate; no skips. |
| `npm run test:google-drive` | **PASS — 5/5**. |
| `npm run test:auth` | **PASS — 5/5**. |
| `npm run test:backup-flow` | **PASS — 17/17**. |
| `npm run test:device-reconcile` | **PASS — 6/6**. |
| `deno check --frozen --node-modules-dir=manual` for `device-enrollment`, `google-drive-authorize`, and `google-drive-metadata` | **PASS**. |
| `deno fmt --check --unstable-sql` for the three Edge entrypoints, two W1 migrations, and W1 SQL evidence | **PASS — 6 files checked**. |
| `npx tsc --noEmit` | **PASS**. |
| `npm run build` | **PASS — 1,764 modules transformed**. |
| `git diff --check 54ff343..10d4a03` | **PASS**. |
| Transaction/default-conflict/static non-transactional-DDL audit | **PASS** — explicit wrapper, no `SET plpgsql.variable_conflict`, and no `CREATE INDEX CONCURRENTLY`, `VACUUM`, `CREATE DATABASE`, or `ALTER TYPE ... ADD VALUE` candidate. |
| Scoped project-reference/hardcoded-credential and Edge-console scan | **PASS** — no executable project reference, hardcoded Supabase credential literal, or Edge `console.*` call. |

## Changed-path and dirty-worktree audit

Implementation `602050a` changes exactly:

- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

Report `10d4a03` adds exactly:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix3-luna-report.md`

The prior Terra addendum is not changed in `54ff343..10d4a03`. Its pre-existing
dirty working-tree modification, along with every other modified or untracked
user path present before this review, was preserved and left unstaged. This
review adds only this report.

## Remaining external gates

1. In an explicitly approved staging project, apply the reviewed migrations and
   perform a forced-failure rollback probe; inspect RLS, table/function
   privileges, `proconfig`, and Data API behavior for `PUBLIC`, `anon`,
   `authenticated`, and `service_role`.
2. Run at least 50 concurrent identical signed Edge requests covering active
   authorization, nonce replay, device/connection/grant revocation ordering,
   and revoked-then-`connection.activate` behavior.
3. Verify the deployed pinned Edge revision, JWT/JWKS configuration, project
   linkage, clean-checkout Deno reproduction, and metadata isolation.
4. Prove real Google consent, upload/download/revoke, native keyring,
   clean-install restore, Android/FUNGWIRE device flow, signing, and release
   acceptance separately.

## Version Diff

- `new -> 0.1.0b`: final independent review reconciles P0-03, P0-04, P1-01,
  and P2-01 as locally executable PASS; retained external gates yield WARN.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | need review | Local hard gates pass; staging, provider, deployment, and release evidence remains open. | `602050a` / `10d4a03` | Terra |
