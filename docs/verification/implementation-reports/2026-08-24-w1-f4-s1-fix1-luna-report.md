---
version: "0.1.0b"
created_at: "2026-08-24T06:39:02+07:00,Luna 5.6"
last_update: "2026-08-24T06:39:02+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S1-F1 transactional authorization fix"
  base_head: "4f745c7"
  implementation_commit: "c59d4a4"
---

# W1-A-F4-S1-F1 — Luna Fix Report

## Status

`DONE_WITH_CONCERNS` — Terra P0-01, P0-02, and P1-01 were fixed within the
five-path implementation allowlist and committed as `c59d4a4`. Local source,
contract, build, format, and regression evidence is green. Live SQL/RLS/grant,
staging concurrency, deployment, and a new independent Terra review remain
external gates.

No migration, project link, deployment, push, merge, PR, deletion, or external
message was performed. The report is written as a separate allowlisted follow-up
path and was not part of the implementation commit.

## Root-cause mapping

| Finding | Root cause | Fix |
|---|---|---|
| P0-01 | Both new migrations began persistent DDL without a verifiable top-level transaction wrapper. | Added explicit `BEGIN;` as the first code statement and `COMMIT;` as the last code statement in both migrations. |
| P0-02 | Edge read device/connection/grant state, reserved a nonce, then recorded an allow decision through separate calls; revocation could win between those steps. | Replaced the separated path with `public.authorize_oauth_request(...)`, which rechecks and locks authoritative state, reserves the nonce, writes the allowed/denied decision, and performs connection state transitions in one database transaction. Edge only fetches public-key material, verifies the signature, then calls this RPC. |
| P1-01 | `is_drive_authorized_desktop(uuid, uuid)` was absent from committed privilege and fixed-search-path staging evidence. | Added direct execute assertions for `PUBLIC`/`anon`/`authenticated` denial and `service_role` allowance, and added the predicate function to the fixed-configuration list. |

## Transaction and locking boundary

The authorization RPC uses this deterministic order:

`device row → exact-provider connection row → backup.write grant → backup.restore grant → unique nonce reservation → server decision`

The device row is locked before `is_drive_authorized_desktop` is evaluated
again, and the stored public key/fingerprint are compared with the
signature-verified request. The connection is locked and checked for active
status, no revocation, and exactly `drive.appdata`; the required independent
grant is then locked and checked. The unique nonce insert uses
`ON CONFLICT (nonce) DO NOTHING RETURNING`; a conflict returns a denied replay
result. The decision row and reservation status are written in the same
transaction. Connection activation/revocation is also kept inside that RPC;
reconnection does not recreate revoked grants.

Device, connection, and grant revocations that acquire their authoritative row
lock first serialize before the decision and are observed as denial. A
revocation that acquires its lock after the authorization transaction is ordered
after that decision. Audit rows remain informational and are never authority or
the replay lock.

## RED evidence

Before implementation, after adding the regression contracts to the existing
test path:

```text
node --test tests/w1AuthoritySchema.test.mjs
tests 5, pass 2, fail 3
```

The failures were the missing `BEGIN; … COMMIT;` migration boundary, the
missing single authorization RPC/continued separated Edge path, and the
missing `is_drive_authorized_desktop` staging evidence.

## GREEN evidence

| Check | Result |
|---|---|
| `node --test tests/w1AuthoritySchema.test.mjs` | 5/5 passed |
| `npm run test:google-drive` | 5/5 passed |
| `npm run test:auth` | 5/5 passed |
| `npm run test:device-reconcile` | 6/6 passed |
| `npm run test:backup-flow` | 17/17 passed |
| `deno check --frozen --node-modules-dir=manual` for all three Edge entrypoints | Passed |
| `deno fmt --check --unstable-sql` for the three Edge files, two migrations, and SQL evidence | Passed; 6 files checked |
| `npm run build` | Passed; 1,764 modules transformed |
| `git diff --check` and staged diff check | Passed |
| Secret/project-reference/log scan | Passed; no prohibited literal match |
| Exact implementation allowlist audit | Passed; exactly 5 implementation paths staged |

## Implementation commit and paths

Commit `c59d4a4` (`fix: make W1 Drive authorization transactional`) contains
exactly these five paths:

- `supabase/migrations/20260823000000_w1_device_enrollment_authority.sql`
- `supabase/migrations/20260823000001_w1_drive_authorization_policy.sql`
- `supabase/functions/google-drive-authorize/index.ts`
- `supabase/tests/w1_authority_schema.sql`
- `tests/w1AuthoritySchema.test.mjs`

The required report path is:

- `docs/verification/implementation-reports/2026-08-24-w1-f4-s1-fix1-luna-report.md`

Pre-existing dirty and untracked paths were not staged or committed.

## External gates and concerns

- `psql` and the Supabase CLI are unavailable locally; no live migration,
  SQL execution, RLS, privilege, Data API, or staging rollback probe was run.
- The required 50-worker identical-request/revocation race remains a staging
  test and was not claimed from static source contracts.
- A fresh independent Terra review is required before any staging migration,
  Edge deployment, promotion, merge, or release.
- Google consent/provider behavior, keyring behavior, clean-install restore,
  Android/FUNGWIRE evidence, signing, and production readiness remain outside
  this fix cycle.

## Version Diff

- `new -> 0.1.0b`: recorded the W1-A-F4-S1-F1 transactional authorization fix,
  RED/GREEN evidence, exact commit/path boundary, and external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Fixed Terra P0-01, P0-02, and P1-01 within the five-path allowlist. | `c59d4a4` | Luna 5.6 |
