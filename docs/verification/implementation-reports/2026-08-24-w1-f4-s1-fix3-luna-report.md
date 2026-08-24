---
version: "0.1.0b"
created_at: "2026-08-24T08:45:00+07:00,Luna 5.6"
last_update: "2026-08-24T08:45:00+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S1 final executable PostgreSQL fix cycle 3"
  base_head: "54ff343"
  implementation_commit: "602050a"
  terra_addendum: "docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix1-terra-rereview.md"
---

# W1-A-F4-S1-F3 — Luna 5.6 Fix Report

## Status

`DONE_LOCAL_ONLY` — the supplemental Terra P0-04, P1-01, and P2-01 defects are
fixed in the exact three-path implementation allowlist. The corrected
migrations and committed SQL evidence execute successfully in a disposable
PostgreSQL 17.11 container. This is not a staging, deployed Edge, production,
or independent Terra approval claim.

The prior `9ef676b` P0-03 behavior is retained: a previously revoked
`connection.activate` request is denied with `connection_revoked`, and the
connection state, revocation timestamp, scopes, owner, and operation grants
are unchanged. No deployment, push, merge, PR, deletion, or external message
was performed.

## Scope and exact paths

The referenced supplemental addendum is:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix1-terra-rereview.md`

It was preserved unmodified. Its current working-tree blob hash remained
`ae32aacfa7fd890cfeafd0cdd8b4dbedfc124f84` during this cycle.

The implementation commit is `602050a` (`fix: close W1 S1 PostgreSQL evidence gaps`)
and contains exactly:

- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

The report is intentionally a separate commit and is the only other path in
scope:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix3-luna-report.md`

All pre-existing modified and untracked paths were left unstaged and present.

## Root-cause mapping

| Finding | Root cause | Resolution |
|---|---|---|
| P0-03 | The already-reviewed RPC could clear `revoked_at` for a signed `connection.activate` request. | Retained the `9ef676b` denial branch before the activation transition and kept the no-mutation adversarial SQL assertion. |
| P0-04 | `RETURNS TABLE` output names (`connection_id`, `operation`, `nonce`) collided with unqualified table references under PostgreSQL's default `plpgsql.variable_conflict = error`. The nonce conflict target also collided after the grant selectors were qualified. | Added aliases and qualified device, connection, grant, reservation, update, and replay references. Replaced the fragile `ON CONFLICT (nonce)` target with the explicitly named unique constraint `oauth_authorization_reservations_nonce_key`; no permissive conflict setting was added. |
| P1-01 | Fixed-search-path evidence reconstructed a signature from `pg_get_function_identity_arguments`, which does not match the name-less hand-built list on PostgreSQL 17. | Privilege assertions now pass `regprocedure` values, and the fixed-search-path loop resolves `pg_proc.oid` from a `regprocedure` array. |
| P2-01 | The six passing Node checks were source-pattern checks and never applied the migrations or executed the RPC/evidence SQL. | The ordinary W1 Node test now runs a disposable PostgreSQL 17 container, applies minimal prerequisites plus both migrations, executes active/replay and committed SQL evidence, asserts rollback cleanup, and explicitly skips only when Docker is unavailable. |

## RED evidence

The original source-only suite passed `6/6`, demonstrating the P2-01 blind
spot. After adding the executable test and before changing the migration, the
new test failed for the real database reason:

```text
tests 7
pass 6
fail 1
ERROR:  column reference "connection_id" is ambiguous
DETAIL:  It could refer to either a PL/pgSQL variable or a table column.
CONTEXT:  PL/pgSQL function authorize_oauth_request(...) line 88 at SQL statement
```

The independent PostgreSQL 17 reproduction of P0-04 returned:

```text
ERROR:  column reference "connection_id" is ambiguous
LINE 3:       and connection_id = v_connection.id
P004_REPRO_EXIT=3
```

The exact committed evidence SQL, before the `regprocedure` change, returned
the P1-01 failure:

```text
ERROR:  fixed search_path missing for public.create_device_enrollment_request(uuid, text, text, text, text, text)
P101_REPRO_EXIT=3
```

The RED harness used no production or staging connection.

## GREEN PostgreSQL 17 evidence

The executable gate used:

- Docker Engine `29.6.1`
- Image `postgres:17-alpine`
- Image digest `sha256:18cfe3ef5e6815560c98237d6216d1e5119702fb0f3894c8785dd58b8bbe5d73`
- Server `PostgreSQL 17.11` on `x86_64-pc-linux-musl`
- A disposable named container, removed with `docker rm --force` after each run

The Node test's final output was:

```text
tests 7
pass 7
fail 0
skipped 0
```

The executable path applied minimal local prerequisite roles/schema, then both
W1 migrations, and proved:

1. An active trusted Windows device, exact `drive.appdata` connection, and
   active `backup.write` grant are allowed.
2. The allowed operation has exactly one durable nonce reservation and one
   durable authorization decision.
3. Repeating the same nonce returns `authorized = false` and
   `authorization_replayed`, reusing the original reservation and decision.
4. Revoking the exact-provider connection causes the trigger to revoke both
   operation grants; a subsequent `connection.activate` returns
   `connection_revoked` and does not change the connection or grant snapshot.
5. The committed privilege/RLS/fixed-search-path SQL evidence passes using
   `regprocedure`/OID lookup rather than catalog string reconstruction.
6. The final rollback probe returns `0|0|0|0|0` for seeded devices,
   connections, operation grants, reservations, and decisions.

The committed SQL evidence itself now contains the active-operation,
repeated-nonce, and revoked-activation checks; the Node harness also exercises
the active/replay path before executing that committed evidence so a future
regression cannot be hidden behind a later evidence failure.

## Regression matrix

| Command | Result |
|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | **7/7 passed**, including the disposable PostgreSQL 17 gate |
| `npm run test:google-drive` | **5/5 passed** |
| `npm run test:auth` | **5/5 passed** |
| `npm run test:backup-flow` | **17/17 passed** |
| `npm run test:device-reconcile` | **6/6 passed** |
| `deno --version` | Deno **2.9.1** |
| `deno check --frozen --node-modules-dir=manual` for all three Edge entrypoints | Passed |
| `deno fmt --check --unstable-sql` for three Edge files, two migrations, and SQL evidence | **6 files checked** |
| `npx tsc --noEmit` | Passed |
| `npm run build` | Passed; Vite transformed **1,764 modules** |
| Scoped `git diff --check` | Passed |
| Staged exact-path audit | Exactly the three implementation paths in `602050a` |

## External gates and non-claims

The following remain open and were not hidden by local GREEN evidence:

- Fresh independent Terra re-review of the post-`602050a` implementation.
- Applying the corrected migrations in an explicitly approved staging project,
  including forced-failure rollback, RLS, table/function privileges,
  `proconfig`, and Data API behavior for `PUBLIC`, `anon`, `authenticated`,
  and `service_role`.
- At least 50 concurrent identical signed Edge requests covering active
  authorization, nonce replay, device/connection/grant revocation ordering,
  and the revoked-then-`connection.activate` case.
- Deployed pinned Edge revision, verified JWT/JWKS configuration, project
  linkage, clean-checkout Deno reproduction, and metadata isolation.
- Real Google consent/provider upload/download/revoke behavior and native
  keyring execution.
- Clean-install restore, Android/FUNGWIRE/device evidence, signing, release,
  and production readiness.

Docker was available for this local gate. Environments without Docker will
report the executable test as an explicit skipped test; source-pattern passes
must not be reported as database-semantic proof.

## Version Diff

- `new -> 0.1.0b`: recorded the final S1 executable PostgreSQL fix cycle,
  RED/GREEN evidence, exact commits/paths, preserved P0-03 behavior, and
  remaining external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Fixed Terra P0-04, P1-01, and P2-01 with PostgreSQL 17 executable evidence while retaining P0-03. | `602050a` / report commit | Luna 5.6 |
