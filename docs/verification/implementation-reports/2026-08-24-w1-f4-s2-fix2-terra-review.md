---
version: "0.1.0b"
created_at: "2026-08-24T10:55:01+07:00,Terra"
last_update: "2026-08-24T10:55:01+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "implementation-review"
  scope: "Independent Terra S2-F2 PKCE and enrollment-proof security review"
  risk: "HIGH"
  review_base: "8625615e583cf777e085674c057989a160828787"
  implementation_commits: "799a915e9570f11d35bdb9b70b809d775f54f197, 4ac4e8de46523731cbfa512dc5769986b426675d"
  luna_report_commit: "b73b828f3312b1a237e9266d54f42c4f09927fda"
  candidate_sha256: "1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792"
---

# W1-A-F4-S2-F2 — Independent Terra Security Review

## Verdict

**FAIL — S2-F2 must not advance to integration or S3.** The PostgreSQL,
native, auth, Edge type, regression, build, path, candidate-hash, and
literal-secret checks are locally green, but two P0 PKCE secret-boundary
failures remain. The implementation serializes Supabase access and refresh
tokens into a WebView event, and it retains ordinary non-zeroized copies of the
OAuth authorization code. Both violate the approved native-login hard gate.

This is an independent local review only. It is not staging, provider, device,
deployment, release, merge, or production evidence.

## Reviewed provenance and scope

- Base: `8625615`.
- Prior S2 FAIL: `7bc84c1`.
- Reviewed implementation: `799a915` and test evidence `4ac4e8d`.
- Reviewed Luna report: `b73b828`.
- Approved candidate: `docs/specs/2026-08-24-enrollment-proof-nonce-amendment.md`
  version `0.2.0b`, SHA-256
  `1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792`.
  The independently computed hash matches the approval record.
- Reviewed the approval record, the preceding Terra FAIL and PASS re-review,
  the S2-F2 implementation brief/Luna report, and the inherited native and
  authority/schema amendments.

`8625615..4ac4e8d` changes exactly the nine permitted implementation/test
paths. `4ac4e8d..b73b828` adds only the Luna report. Existing dirty and
untracked paths were preserved; this report is the only review mutation.

## Gate results

| Gate | Result | Independent evidence |
|---|---|---|
| Candidate hash / approved boundary | PASS | `Get-FileHash` matched the approval-record SHA-256 exactly. |
| Commit/path/diff hygiene | PASS | Exact nine-path allowlist and report-only range checks passed; `git diff --check 8625615 4ac4e8d` passed. |
| Literal secret scan | PASS, insufficient | Scoped diff scan found no JWT, Google API key, service-role/secret key, or private-key literal. It does not override P0-01's runtime DTO exposure. |
| Auth-flow tests | PASS | `npm run test:auth`: 8/8. |
| PostgreSQL 17 | PASS, coverage gap below | `node --test --experimental-strip-types tests/w1AuthoritySchema.test.mjs`: 8/8 using `postgres:17-alpine`; forward migration, replay, two-client race, privilege/search-path, no-mutation, and rollback checks passed. |
| Native Rust / PKCE static unit coverage | PASS, P0-02 remains | `cargo fmt -- --check` passed; `cargo test --lib -j 1 native_auth::tests --no-fail-fast`: 9/9. |
| Edge | PASS for type-check only | `deno check --frozen --node-modules-dir=auto supabase/functions/device-enrollment/index.ts` passed. |
| Regressions | PASS | `npm run test:google-drive`: 5/5; `npm run test:device-reconcile`: 6/6. |
| Frontend build | PASS | `npm run build` passed; 1,763 modules transformed. |

## Findings

| ID | Priority | Finding | Evidence | Required correction |
|---|---|---|---|---|
| P0-S2F2-01 | P0 | The native PKCE exchange returns Supabase bearer credentials through the serializable `auth-callback` event. `AuthSession` holds `access_token` and `refresh_token`, the event emits it to the WebView, and `authFlow` accepts both fields then calls `supabase.auth.setSession`. This directly contradicts the approved invariant that Supabase tokens never enter public DTOs. | `src-tauri/src/native_auth.rs:57-70,580-582,711-717`; `src/lib/authFlow.ts:13-21,68-92`; inherited invariant at `docs/specs/2026-08-23-google-drive-native-authorization-amendment.md:117-119`. | Keep access/refresh tokens in native protected custody. Replace the public event payload with a redacted, request-bound completion result and add behavioral tests that prove no token appears in Tauri events, WebView-visible DTOs, logs, or frontend storage. |
| P0-S2F2-02 | P0 | The authorization code is copied into ordinary `String` allocations before the later `Zeroizing` copy is created. The parsed query pairs retain the code, and the loopback HTTP target/callback URL also contain it. Dropping those ordinary strings does not erase them; `Zeroizing::new(code.to_owned())` wipes only the later copy. | `src-tauri/src/native_auth.rs:171-255,585-630,762-768`; candidate hard gate at `docs/specs/2026-08-24-enrollment-proof-nonce-amendment.md:42-50,154`. | Parse and carry code-bearing callback material in a zeroizing representation from receipt through exchange; avoid ordinary copies and prove zeroization/terminal-path handling for success, denial, malformed callback, timeout, cancellation, exchange failure, and shutdown. |
| P1-S2F2-01 | P1 | The executable race has exactly two concurrent callers, not the inherited minimum of 50 identical requests. It establishes a useful two-client one-winner result, but it does not close AC-GDA2-07. | `tests/w1AuthoritySchema.test.mjs:470-486`; inherited AC-GDA2-07 at `docs/specs/2026-08-23-google-drive-authority-schema-amendment.md:218`. | Run and assert at least 50 identical concurrent proof submissions, with exactly one durable winner, all losers `proof_replayed`, and no loser-side mutation. |

No P2 finding is required for this review. The Rust test output contains
pre-existing unused legacy enrollment-helper warnings; the approved S2-F2
boundary intentionally leaves that unregistered legacy code untouched.

## Root cause

The implementation treats a typed session object as safe to send across the
native/WebView boundary. Its type contains bearer credentials, so serialization
turns it into the prohibited public DTO. Separately, `Zeroizing` was applied
only after URL/query parsing had already made non-zeroized code copies. The
source-pattern auth test checks for `Zeroizing` and `setSession`, but does not
assert either custody or erasure invariant.

## Required next action

1. Open a fresh, tightly scoped S2-F2 remediation packet for P0-S2F2-01 and
   P0-S2F2-02; do not change the approved candidate, migration, or prior
   reports.
2. Add behavioral secret-boundary and terminal-erasure tests plus the required
   50-client PostgreSQL race evidence.
3. Obtain a fresh independent Terra re-review before S3 or any integration.

## External gates retained

- Per-project read-only migration-history, RLS, exposed-schema/Data API,
  function-config, and execute-grant preflight; separately authorized staging
  migration and advisor evidence.
- Edge deployment and deployed RPC/Ed25519 behavior verification.
- Real Google installed-app configuration, consent, native callback/token
  exchange, refresh, revoke, upload, and digest-bound restore.
- Boss-only pending-enrollment approval/promotion ceremony.
- Clean-install Windows keyring migration/reconnect/restore, physical
  Android/FUNGWIRE validation, signing, release, merge, and promotion.

No deployment, push, merge, PR, deletion, or external message was performed.

## Version Diff

- `new -> 0.1.0b`: independent S2-F2 FAIL records two PKCE secret-custody and
  erasure P0s, a 50-client race-evidence P1, all completed local gate results,
  and retained external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | need review | FAIL: public token DTO and incomplete authorization-code erasure block S2-F2; 50-client race evidence is also absent. | pending | Terra |
