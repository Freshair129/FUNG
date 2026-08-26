---
version: "0.2.0b"
created_at: "2026-08-24T06:52:32+07:00,Terra"
last_update: "2026-08-24T07:05:19+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Independent Terra re-review of W1-A-F4-S1-F1"
  base_commit: "4f745c7f936856ecc9c10807ed5e6b63a629cfe3"
  implementation_commit: "c59d4a400d7c6d168b59e054cf034486ea0d41fd"
  implementation_report_commit: "5c3b722835e60ca413849b914bd49b50a2230b6c"
---

# W1-A-F4-S1-F1 — Independent Terra Re-review

## Verdict

**FAIL.** Do not apply the migrations, deploy the Edge function, promote, merge,
or treat `c59d4a4` / `5c3b722` as a passed security gate.

The original migration-atomicity, split authorization, and predicate-evidence
findings are addressed at source level. However, the replacement RPC permits a
previously revoked Google Drive connection to be reactivated by a signed
`connection.activate` request. This violates the re-review packet's hard gate:
a connection revocation ordered before the authorization decision must deny, not
be cleared by that decision. The five passing schema tests are source-pattern
tests and do not exercise this state transition.

Scope reviewed: implementation `c59d4a400d7c6d168b59e054cf034486ea0d41fd`
and report `5c3b722835e60ca413849b914bd49b50a2230b6c`, against prior reviewed
base `4f745c7f936856ecc9c10807ed5e6b63a629cfe3`. No implementation file,
deployment, migration, push, merge, PR, deletion, or external message was made.

## Hard-gate ledger

| Gate | Result | Independent evidence |
|---|---|---|
| Explicit, effective top-level migration boundary | Source PASS; live unverified | Both migrations begin with `BEGIN;`, end with `COMMIT;`, and contain no static known non-transactional DDL candidate. See `20260823000000_w1_device_enrollment_authority.sql:5,809` and `20260823000001_w1_drive_authorization_policy.sql:4,504`. |
| One transaction rechecks/locks device, connection, grants; reserves nonce; writes decision | **FAIL** | The RPC has the required locks and durable insert, but its connection-activation branch authorizes a revoked connection rather than denying it (P0-03). |
| Revocation serializes predictably; pre-decision revocation denies | **FAIL** | `connection.activate` treats connection state as a prerequisite only for backup operations, then clears `revoked_at` after an allowed decision (P0-03). |
| Edge verifies signature then calls only integrated authorization RPC | Source PASS | `google-drive-authorize/index.ts:191-270` verifies the registered key/signature, then calls only `authorize_oauth_request`; no separated reservation/allow RPC remains. |
| Audit is neither replay lock nor authority | Source PASS | `authorize_oauth_request` uses the reservation unique conflict path and writes the decision at `20260823000001_w1_drive_authorization_policy.sql:378-434`; the Edge audit insert follows the RPC at `google-drive-authorize/index.ts:276-294`. |
| Predicate privilege and fixed-search-path evidence complete | Source PASS; staging unverified | `is_drive_authorized_desktop` is now included in direct privilege assertions and the function configuration list at `supabase/tests/w1_authority_schema.sql:108-123,177-207`. |
| Original S1 boundaries and no hidden source-pattern regression | **FAIL** | The exact path boundary passes, but the required semantic revocation case is absent from `tests/w1AuthoritySchema.test.mjs:68-120`; P0-03 is therefore hidden by the passing source-pattern test. |

## Findings

### P0-03 — `connection.activate` clears an earlier connection revocation

**Result: FAIL.** The RPC maps a required operation only for `backup.write`,
`backup.read`, and `backup.restore`
(`supabase/migrations/20260823000001_w1_drive_authorization_policy.sql:353-358`).
Its connection-active/exact-scope denial applies only when that derived value is
non-null (`:360-374`). Therefore a trusted device's `connection.activate`
request remains authorized even when its locked exact-provider connection is
already `revoked`.

After recording that allowed decision, the activation branch upserts the existing
connection with `status = 'active'` and `revoked_at = null`
(`20260823000001_w1_drive_authorization_policy.sql:438-461`). The Edge request
contains a device signature but passes no server-verifiable, one-time proof of a
fresh Google consent/exchange into the RPC
(`supabase/functions/google-drive-authorize/index.ts:239-270`). Thus a signed
activation submitted after a connection revocation can erase that revocation
inside the same authorization transaction.

The normal native flow calls activation after a local code exchange
(`src-tauri/src/drive_oauth.rs:677-718`), but that ordering is not an authority
input to the Edge/RPC. Direct Edge authorization remains possible for a holder
of the valid session and trusted-device signing capability. Although connection
activation does not recreate revoked operation grants, the server state no
longer preserves the connection revocation and can be used as a false active
precondition for subsequent operator action.

This contradicts the approved authority amendment's requirement that connection
revocation immediately invalidates the operations and that reconnecting does not
resurrect revoked authority (`docs/specs/2026-08-23-google-drive-authority-schema-amendment.md:91-101`), as well as the re-review hard gate requiring a revocation that wins before the decision to deny.

**Required disposition:** retain a revoked connection as denied in this RPC, or
bind a reactivation to a distinct, server-verifiable, one-time fresh-consent
ceremony that cannot be synthesized from a device signature alone. Add a
database-level adversarial test that revokes the connection before
`connection.activate`, asserts a denied decision, and asserts that neither
`status` nor `revoked_at` is changed. Re-run independent review after the fix.

### P1/P2

The supplemental independent evidence below identifies an additional P0, P1,
and P2 defect in the same frozen commit range. It supersedes this earlier
interim conclusion and does not reduce the P0 verdict.

## Acceptance-criteria evidence

| Criterion | Result | Evidence boundary |
|---|---|---|
| AC-GDA2-01 | Source PASS; live unverified | No browser/anon/authenticated/service-role execute grant to bootstrap/rebind remains in source. Staging Data API/ACL attempts are external. |
| AC-GDA2-02 | Source PASS; live unverified | The locked RPC re-evaluates the Drive desktop predicate before protected backup decisions at `20260823000001_w1_drive_authorization_policy.sql:280-299`. |
| AC-GDA2-03 / -04 | Source PASS; live unverified | Existing owner-only bootstrap/rebind functions and row locks were not broadened by this fix. |
| AC-GDA2-05 | Source PASS for independent backup write/restore grants; overall hard gate FAIL | Backup operations still require active exact scope plus the separate current grant (`:353-374`), but P0-03 violates the broader required connection-revocation serialization. |
| AC-GDA2-06 | Unchanged / local regression PASS | The Native intent/keyring implementation is outside this five-path change; `npm run test:google-drive` passed 5/5. |
| AC-GDA2-07 | Source PASS; staging concurrency unverified | The unique nonce reservation and decision are in one RPC (`:378-434`); no 50-worker live race was possible locally. |
| AC-GDA2-08 | Source PASS; live unverified | Metadata remains outside grant/reservation/bootstrap authority; static Edge scan found no old reservation or approval call. |
| AC-GDA2-10 / -11 | Source PASS; live unverified | Pairing is unchanged; Edge returns separate connection/write/restore status from the RPC. |
| AC-GDA2-12 | Local PASS; clean checkout unverified | Frozen Deno check/format passed against committed imports and lockfile. |
| AC-GDA2-14 | Source PASS; staging unverified | Committed SQL evidence now covers the predicate function's execute posture and `search_path`; no approved staging project was available. |
| AC-GDA2-15 | Local PASS | Backup flow tests passed 17/17. |

## Independent commands and results

| Command / check | Result |
|---|---|
| Commit parent/ancestry audit | `c59d4a4` is a direct child of `4f745c7`; `5c3b722` is a direct child of `c59d4a4`. |
| `git diff --name-status 4f745c7 c59d4a4` | Exactly the five implementation allowlist paths. |
| `git diff --name-status c59d4a4 5c3b722` | Exactly the requested Luna report path. |
| `git diff --check 4f745c7 5c3b722` | Passed with no output. |
| `node --test tests/w1AuthoritySchema.test.mjs` | Passed 5/5. Static contract only; it does not cover P0-03. |
| `npm run test:google-drive` | Passed 5/5. |
| `npm run test:auth` | Passed 5/5. |
| `npm run test:device-reconcile` | Passed 6/6. |
| `npm run test:backup-flow` | Passed 17/17. |
| `deno check --frozen --node-modules-dir=manual` for the three Edge entrypoints | Passed. |
| `deno fmt --check --unstable-sql` for three Edge files, two migrations, and SQL evidence | Passed; 6 files checked. |
| `npx tsc --noEmit` | Passed. |
| `npm run build` | Passed; Vite transformed 1,764 modules. |
| Scoped Edge secret/log/project-reference scan | Passed; no prohibited literal match. |
| Transaction-boundary/non-transactional-DDL scan | Source PASS: both migrations first/last code lines are `BEGIN;` / `COMMIT;`; no `CONCURRENTLY`, `VACUUM`, `CREATE DATABASE`, or `ALTER TYPE ... ADD VALUE` candidate found. |
| `supabase --version`; `psql --version` | Both unavailable. The supplemental reviewer used a disposable local PostgreSQL 17 harness with minimal prerequisite roles/schema only; no approved Supabase project was contacted or changed. |

## Changed-path and dirty-worktree audit

Implementation `c59d4a4` changes only:

- `supabase/functions/google-drive-authorize/index.ts`
- `supabase/migrations/20260823000000_w1_device_enrollment_authority.sql`
- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

Report `5c3b722` adds only:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix1-luna-report.md`

The reviewed commits do not modify historical migrations or absorb the existing
`supabase/README.md` dirty hunk. `git status --short --branch` was compared
before and after verification; all pre-existing modified and untracked user
paths remained present and untouched. This review adds only this Terra report.

## Remaining external gates

1. Fix P0-03 and obtain a new independent Terra review before any staging action.
2. In an explicitly approved staging project, apply the corrected migrations with
   a forced-failure/rollback probe, then inspect RLS, table/function privileges,
   `proconfig`, and Data API behavior for `PUBLIC`, `anon`, `authenticated`, and
   `service_role`.
3. Run at least 50 concurrent identical signed requests against device,
   connection, write-grant, and restore-grant revocation races. Include the
   revoked-then-`connection.activate` case and prove zero post-revocation
   protected operations or silent state resurrection.
4. Verify deployed pinned Edge revisions, verified JWT/JWKS configuration, no
   project-reference fallback, clean-checkout Deno reproduction, and metadata
   isolation.
5. Google consent/provider and revocation behavior, native keyring behavior,
   clean-install restore, Android/FUNGWIRE evidence, signing, and release remain
   separate and unverified.

## Supplemental independent Terra evidence — frozen `c59d4a4`

This addendum was independently reproduced after the earlier report was
created. It reviews only `c59d4a4` and `5c3b722` against `4f745c7`. Later
commits `41861d8` and `9ef676b`, plus later dirty implementation changes, are
out of scope and are not accepted as remediation here.

### Additional verdict evidence

**P0-04 — the integrated authorization RPC raises `42702` before it can lock
operation grants, reserve the nonce, or write a decision.**

`authorize_oauth_request` declares `operation`, `nonce`, and `connection_id`
as `RETURNS TABLE` output variables
(`supabase/migrations/20260823000001_w1_drive_authorization_policy.sql:233-239`).
The two grant-lock queries then use the unqualified table columns
`connection_id` and `operation` (`:326-346`), and the replay branch similarly
uses unqualified `nonce` (`:398-402`). Under PostgreSQL's default
`plpgsql.variable_conflict = error`, the first active-connection call fails:

```
ERROR: column reference "connection_id" is ambiguous
DETAIL: It could refer to either a PL/pgSQL variable or a table column.
CONTEXT: PL/pgSQL function authorize_oauth_request(...) line 88 at SQL statement
```

The device and connection `FOR UPDATE` statements run first (`:280-321`), but
the error occurs while planning the first grant query. Therefore the function
never acquires the required grant lock, creates the durable nonce reservation,
or inserts its decision. Edge maps that RPC error to
`authorization_unavailable` / HTTP 500
(`supabase/functions/google-drive-authorize/index.ts:238-253`), so this is
fail-closed rather than a demonstrated privilege escalation. It still fails
the mandatory integrated-RPC, grant-lock, durable-replay, and authorization
decision hard gates. It also means no claimed 50-request race result can prove
the actual function: the local same-nonce run produced zero reservations and
zero decisions because every invocation failed first.

**Required remediation:** alias and qualify every table reference (including
the two grant selectors and the replay selector), then run executable database
tests with PostgreSQL's default conflict setting for (1) an allowed active
backup operation, (2) a repeated nonce returning `authorization_replayed`, and
(3) revocation races. Do not set a permissive `plpgsql.variable_conflict`
configuration as a workaround.

**P1-01 — the committed privilege/search-path evidence SQL fails on PostgreSQL
17 even though the underlying source posture is present.**

`supabase/tests/w1_authority_schema.sql:177-198` compares a list of
name-less function signatures with `pg_get_function_identity_arguments`.
PostgreSQL 17 returns parameter names in that catalog representation, so the
lookup returns no row and the test raises at `:199-205`:

```
ERROR: fixed search_path missing for public.create_device_enrollment_request(uuid, text, text, text, text, text)
```

An independent OID-based catalog query in the same isolated harness found the
expected fixed search path on all three required functions and the reviewed
direct privilege posture (`PRIVILEGE_POSTURE=true`,
`FIXED_SEARCH_PATH_FUNCTIONS=3`). This is therefore an evidence-test defect,
not a finding that the source grants execution incorrectly. Nonetheless,
AC-GDA2-14 and the re-review privilege-evidence hard gate are not demonstrated
by the committed test. Match functions by `regprocedure`/OID rather than by a
hand-built signature string, then run it on the approved staging database.

**P2-01 — the five passing W1 Node tests are pattern checks and missed both
executable defects.**

`tests/w1AuthoritySchema.test.mjs:68-88` checks transaction and lock text;
`:90-133` checks Edge/evidence text. The test never invokes the SQL function
or executes `supabase/tests/w1_authority_schema.sql`. It passed 5/5 while
P0-04 and P1-01 were reproducible. Add a disposable-database integration test
to the ordinary verification path; source regular expressions alone are not
security evidence for row-lock, replay, or privilege behavior.

### Independent hard-gate and AC delta

| Item | Supplemental result | Evidence |
|---|---|---|
| Migration wrappers | PASS locally; live unverified | Both frozen migrations have `BEGIN;` / `COMMIT;`; a forced-error transaction rolled back its created test table in disposable PostgreSQL 17. |
| Row-lock / revocation ordering | Structural PASS only; integrated gate **FAIL** | The frozen function's device, connection, and grant lock order is present. An analogous `SELECT FOR UPDATE` interleaving denied an already-revoked locked connection, but P0-04 prevents the real RPC from reaching its grant lock or decision. The retained P0-03 remains independently relevant to frozen `c59d4a4`. |
| Integrated Edge RPC and audit separation | Edge source PASS; functional gate **FAIL** | Signature verification precedes the sole RPC and audit follows it (`google-drive-authorize/index.ts:189-294`); P0-04 makes that RPC unavailable for an active connection. |
| AC-GDA2-02 / -05 | **FAIL** as executable authorization evidence | The predicate is structurally rechecked, but an active trusted device cannot complete the required grant-authorized RPC path. |
| AC-GDA2-07 | **FAIL** | No successful first reservation/decision exists, and the replay branch has its own unqualified `nonce` defect. |
| AC-GDA2-08 | Source PASS; live unverified | The audit write remains after the integrated RPC and is not queried as replay authority. |
| AC-GDA2-14 | **FAIL** | The committed SQL evidence aborts before proving fixed search paths/privileges on PostgreSQL 17. |
| AC-GDA2-12 / -15 | Retain local PASS from the main report | Frozen Deno/build and backup-flow regressions passed; they do not exercise P0-04 or P1-01. |

### Supplemental commands and results

| Command / check | Result |
|---|---|
| Isolated `postgres:17-alpine` harness: apply W1 migrations after minimal Supabase-like prerequisite roles/schema | `MIGRATION_APPLY=PASS`; this was disposable local verification, not staging. |
| Exact committed `supabase/tests/w1_authority_schema.sql` in that harness | **FAIL** with the P1-01 fixed-search-path error above. |
| Independent OID-based catalog checks | `PRIVILEGE_POSTURE=true`; `FIXED_SEARCH_PATH_FUNCTIONS=3`. |
| Seeded trusted Windows device + active exact-scope connection + active write grant, then call frozen `authorize_oauth_request` | **FAIL** with `column reference "connection_id" is ambiguous`. |
| Fifty same-nonce invocations after the seeded call | **FAIL / no valid race result:** `reservations=0`, `decisions=0`; each call hit P0-04 before reservation. |
| `node --test tests/w1AuthoritySchema.test.mjs` | PASS 5/5; classified as P2-01 source-pattern-only evidence. |
| `git diff --check 4f745c7..5c3b722`; commit/path audit | PASS: five approved implementation paths in `c59d4a4`; one Luna report path in `5c3b722`. |

### Supplemental path and external-gate disposition

The immutable reviewed range remains within the approved five implementation
paths plus the Luna report path. During this review the shared worktree and
branch advanced beyond `5c3b722`; those later commits and three changed W1
paths were preserved and excluded. No source, migration, deployment, remote
service, or external project was changed by this reviewer. This report is the
only file updated for this addendum.

Before any staging action, fix P0-03, P0-04, and P1-01 in a newly approved
scope; rerun a fresh independent Terra review; then execute the corrected
database evidence and a real 50-worker signed Edge/revocation test in approved
staging. All existing provider, Google consent, deployed Edge/JWKS, Data API,
clean-install, Android/FUNGWIRE, signing, and release gates remain external and
unverified.

## Version Diff

- `new -> 0.1.0b`: independent re-review of the S1-F1 transaction fix; FAIL on
  revoked connection reactivation despite source-level fixes to the prior P0/P1
  findings.
- `0.1.0b -> 0.2.0b`: supplemental independent reproduction adds P0-04
  (non-executable integrated RPC), P1-01 (non-executable committed evidence),
  and P2-01 (source-pattern test blind spot) for the same frozen commit scope.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-08-24 | need review | Supplemental FAIL: integrated RPC throws `42702`; privilege evidence SQL fails on PostgreSQL 17. | `c59d4a4` / `5c3b722` | Terra |
| 0.1.0b | 2026-08-24 | need review | Independent FAIL: a signed `connection.activate` clears a prior server connection revocation. | `c59d4a4` / `5c3b722` | Terra |
