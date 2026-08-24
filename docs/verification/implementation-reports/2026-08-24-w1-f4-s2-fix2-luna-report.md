---
version: "0.1.0b"
created_at: "2026-08-24T10:40:44+07:00,Luna 5.6"
last_update: "2026-08-24T10:40:44+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "enrollment-auth-security"
  doc_type: "implementation-report"
  scope: "W1-A-F4-S2-F2 local implementation and verification"
  candidate_version: "0.2.0b"
  candidate_sha256: "1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792"
  implementation_commits: "799a915, 4ac4e8d"
---

# W1-A-F4-S2-F2 — Luna implementation report

## Status

**LOCAL PASS.** The approved S2-F2 implementation is complete within the
allowlist. PostgreSQL 17, native Rust, auth-flow, Edge type-check, regression,
build, path, and secret checks pass. This report does not claim staging,
provider, device, deployment, or production evidence.

No out-of-scope dependency was required. PostgreSQL and native PKCE remain
completable within the approved write set.

The candidate spec was re-hashed before and after implementation:

`1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792`

The approved candidate and prior migrations were not edited.

## RED evidence retained

The test-first RED run was captured before implementation:

| Gate | RED evidence |
|---|---|
| Auth | `npm run test:auth`: 6 passed, 2 failed. The new failures were the absent native PKCE verifier/challenge path and absent canonical enrollment envelope. |
| PostgreSQL/schema | `node --test --experimental-strip-types tests/w1AuthoritySchema.test.mjs`: 6 passed, 2 failed because the approved forward migration did not yet exist; Docker/PostgreSQL 17 was available. |

## GREEN evidence

| Gate | Command | Result |
|---|---|---|
| Auth flow | `npm run test:auth` | 8/8 passed |
| PostgreSQL 17 | `node --test --experimental-strip-types tests/w1AuthoritySchema.test.mjs` | 8/8 passed; migration apply, first use, replay, concurrent one-winner/one-loser, expiry, future skew, tampered key, foreign profile, wrong version/platform, privilege/search-path, no-mutation, rollback, and final rollback probe passed |
| Native Rust | `cargo fmt -- --check; cargo test --lib -j 1 native_auth::tests --no-fail-fast` | 9/9 passed |
| Edge | `deno check --frozen --node-modules-dir=auto supabase/functions/device-enrollment/index.ts` | passed |
| Regressions | `npm run test:google-drive` | 5/5 passed |
| Regressions | `npm run test:device-reconcile` | 6/6 passed |
| Frontend build | `npm run build` | passed; 1,763 modules transformed |
| Hygiene | `git diff --check` and allowlist/secret audit | passed; no private-key, service-role, or token matches |

The PostgreSQL readiness helper was made stable against the image's transient
init-server shutdown; it now requires two consecutive accepting probes. The
temporary container was removed by the existing test cleanup path.

## Implemented paths

Implementation commit `799a915` contains the production and initial test
changes. Test-evidence commit `4ac4e8d` adds the approved concurrent and
adversarial PostgreSQL coverage.

Exact implementation/test paths:

- `src-tauri/src/native_auth.rs`
- `src-tauri/src/lib.rs`
- `src/lib/authFlow.ts`
- `src/components/AccountLoginPanel.tsx`
- `supabase/functions/device-enrollment/index.ts`
- `supabase/migrations/20260824000000_w1_enrollment_proof_nonce.sql`
- `supabase/tests/w1_authority_schema.sql`
- `tests/authFlow.test.mjs`
- `tests/w1AuthoritySchema.test.mjs`

The native command is Rust-renamed only to avoid a Tauri macro collision; its
wire command remains `device_enrollment_proof`. The legacy implementation in
`src-tauri/src/device_identity.rs` remains untouched and unregistered.

## Security implementation summary

- Native generates the PKCE verifier and S256 challenge, constructs the exact
  Google URL, owns loopback state/callback validation, exchanges the code at
  `/auth/v1/token?grant_type=pkce`, and derives the user from `/auth/v1/user`.
  The verifier and callback code use `Zeroizing` and are dropped on denial,
  malformed callback, timeout, cancellation, exchange failure, and success.
- Native signs the exact binary envelope beginning
  `FUNG\0DEVICE_ENROLLMENT\0V1\0`: raw UUID, raw public key, raw fingerprint,
  network-order u16 platform/label lengths and bytes, network-order i64 times,
  and raw nonce. Byte fields cross the boundary as unpadded base64url.
- The WebView receives only a typed native session and calls `setSession`; it
  does not exchange OAuth codes or choose the user/key/nonce/timestamps.
- Edge independently validates the exact field set, NFC/trimmed label, user,
  operation, platform, time bounds, public-key fingerprint, canonical bytes,
  and Ed25519 signature before calling the atomic RPC.
- The forward migration adds immutable proof metadata plus an indefinitely
  retained, globally unique nonce-hash reservation. The fixed-search-path
  security-definer RPC reserves with `ON CONFLICT ... DO NOTHING RETURNING`,
  raises `proof_replayed` without mutation, and rolls back reservation and
  pending request together on later failure. Only `pending` is created.

## External gates

Still open and intentionally not performed:

- Apply the forward migration and verify RLS/grants/search paths in the real
  staging Supabase project, including advisors and deployed RPC behavior.
- Deploy the Edge function and verify the deployed function/RPC pair.
- Verify real Google provider/client redirect configuration and a live browser
  PKCE callback/token exchange.
- Complete Boss/manual enrollment approval and promotion ceremony.
- Run clean-install/keyring, real-device, release, and production/UAT checks.

No deploy, push, merge, PR, deletion, or external message was performed.

## Version Diff

- `new -> 0.1.0b`: opened the Luna S2-F2 local implementation report for the
  hash-bound 0.2.0b candidate.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Local S2-F2 implementation and verification report | `799a915`, `4ac4e8d` | Luna 5.6 |
