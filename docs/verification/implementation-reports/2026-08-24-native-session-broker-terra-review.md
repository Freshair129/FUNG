---
version: "0.1.0b"
created_at: "2026-08-24T14:12:21+07:00,Terra"
last_update: "2026-08-24T14:12:21+07:00,Terra"
status: "need review"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-review"
  scope: "Independent documentation-only architecture and security review of the Desktop/Tauri native session broker candidate"
  risk: "HIGH"
  review_base: "3e2b38c9d8eed0e638a93b8bd67dc8dad873c373"
  luna_evidence_commit: "b73b828f3312b1a237e9266d54f42c4f09927fda"
  candidate_sha256: "B2C89EBAFEE7CB0AF1648F656A802DE8CF921203AA418A1351E459382010935B"
  review_mode: "docs-only"
---

# Desktop Native Session Broker — Independent Terra Architecture/Security Review

## Verdict

**FAIL — candidate `0.1.0b` is not ready for Boss approval.** It correctly
identifies the two S2-F2 P0 failures and proposes the right direction: native
credential custody, redacted IPC, no generic proxy, and Desktop-only scope.
However, the design does not yet define the exact broker surface, credential
lifecycle, inherited Drive authority controls, or approval provenance needed to
turn those intentions into a reviewable HIGH-risk implementation boundary.

No code, configuration, test, candidate, or prior report was modified by this
review. This verdict is documentation architecture evidence only; it is not a
deployment, migration, provider, device, release, merge, or production result.

## Evidence reviewed

- Candidate: `docs/specs/2026-08-24-native-session-broker-amendment.md`,
  version `0.1.0b`, untracked working-tree artifact, SHA-256
  `B2C89EBAFEE7CB0AF1648F656A802DE8CF921203AA418A1351E459382010935B`.
- S2-F2 Terra FAIL `3e2b38c`, including its public-token-DTO and ordinary
  authorization-code-copy P0 findings; Luna local implementation report
  `b73b828`.
- The approved Luna-Terra workflow; the approved authority/schema and native
  authorization amendments; and the corrected enrollment-proof nonce candidate
  plus its prior Terra re-review.
- The AGENTS-required master plan, Desktop architecture, Mobile implementation
  status, and Desktop real-progress truth documents.
- Current source only to validate the Luna consumer-discovery claim. The
  inspected desktop paths are `src/lib/authFlow.ts`,
  `src/components/AccountLoginPanel.tsx`,
  `src/components/DevicePairingPanel.tsx`, `src/lib/googleDriveFlow.ts`,
  `src-tauri/src/native_auth.rs`, `src-tauri/src/drive_oauth.rs`, and
  `src-tauri/src/lib.rs`.

The candidate's stated root cause is supported by current code: the WebView
currently receives the auth callback, calls `supabase.auth.setSession`, passes
`sessionProof` into enrollment and Drive commands, and performs direct
Supabase pairing/device work. The Tauri invoke registry still exposes the old
auth, enrollment, and Drive command family. The candidate must therefore prove
replacement of every path, not merely introduce a new preferred path.

## Findings

| ID | Priority | Finding and evidence | Required correction |
|---|---|---|---|
| P0-NSB-01 | P0 | The proposed "closed allowlist" is not an exact allowlist. Candidate §§3–4 names broad categories, then requires endpoint/method/schema/body allowlists, but supplies no command, request, response, error, or retirement matrix. Current consumers cover login/session state, enrollment proof and pending enrollment, pairing endpoint publication/reconciliation/create/poll/revoke/audit, Drive connect/cancel/status/disconnect/list/upload/restore-intent/restore, and their registered native commands. Without an authoritative mapping, an old `sessionProof` command or a new broad DTO can survive beside the broker. | Add a normative operation table for every current desktop call and native command: stable broker command name; caller-provided fields and validation; native-derived account/request/device state; exact server/native action; required D-GDA authority/grant/intent; redacted response and public errors; cancellation/idempotency; and whether the old command is replaced, retained with its safe new contract, or deregistered. Add a strict no-duplicate-path rule and acceptance tests that inventory the registered Tauri commands and reject `sessionProof`, token DTOs, `setSession`, and the legacy `auth-callback` session event in the Desktop graph. |
| P0-NSB-02 | P0 | Candidate invariant 2 says refresh tokens live in the keyring and access tokens in zeroizing native memory, while AC-GDA4-03 requires refresh survival and logout/shutdown clearing. It never defines startup rehydration, refresh-token rotation/persistence order, concurrent refresh single-flight behavior, failure disposition, native request ownership, or the cancellation/shutdown linearization point. These omissions leave the core custody invariant untestable and risk reintroducing a browser session after restart or failure. | Specify the native session state machine and terminal transitions: signed-out, login-pending, authenticated, refreshing, refresh-failed, logout, and shutdown. Define native generation/binding of request IDs; keyring read/write/readback/replace/delete order; zeroization boundaries; refresh failure's fail-closed outcome; one in-flight refresh policy; and what every command returns while a transition is in progress. Extend AC-GDA4-03/-08 with behavioral success, refresh rotation/failure, restart, logout, shutdown, cancellation, and race checks that observe no secret-bearing event, DTO, log, storage, or browser session. |
| P0-NSB-03 | P0 | The candidate does not explicitly inherit the approved Drive authority model. It says that required backup/restore calls are broker operations, but does not require the existing per-request server decision, Windows `drive_trusted` predicate, independent `backup.write`/`backup.restore` grants, one-use in-memory authorization context, deny-before-keyring/provider ordering, durable replay reservation, or archive/target-bound restore intent. Current `drive_oauth.rs` applies these as operation-specific checks before status, connection, list, upload, and restore actions; a generic native HTTPS replacement could silently weaken them. | Add a D-GDA2/D-GDA3 inheritance crosswalk and a Drive-operation matrix. It must state that the broker preserves server-derived user/device/connection/operation authority, one-use context, separate grants, durable replay semantics, and the native restore intent; that no request reaches the keyring or provider before denial; and that existing migration/schema contracts are unchanged. AC-GDA4-05/-07/-09 must require the inherited 50-client one-winner proof and negative authorization/order tests, not just functional parity. |
| P1-NSB-01 | P1 | Platform separation is asserted but not designed. The candidate changes shared `src/lib/supabase.ts` and `src/lib/authFlow.ts` while declaring browser routes unchanged and Mobile deferred. Today the shared client is consumed by Desktop, browser, and Mobile, and `src/mobile/MobileApp.tsx` imports `authFlow.ts` for the same callback listener. A generic runtime branch could either leave the browser client reachable in Desktop or alter the deferred Mobile flow. | Name the platform-specific adapter modules and import rule, state how the Desktop bundle excludes authenticated browser-session behavior, and identify which existing public APIs remain browser/Mobile-compatible. Add a static import/build-contract test proving Desktop components cannot import the browser session client, while browser and Mobile retain their specified behavior without claiming Mobile readiness. Expand the write set only if that exact separation requires it. |
| P1-NSB-02 | P1 | Approval and dependency provenance are incomplete. The candidate calls the enrollment nonce migration "approved," but its source amendment still records `candidate` status and candidate D-GDA3 decisions. The prior Terra re-review requires Boss to bind any approval to the exact candidate hash. This candidate contains neither its review base nor a Luna-discovery evidence reference/hash, so a later edit could be mistaken for the reviewed design. No repository-backed D-GDA3 approval record was found in this review. | Add a document-control table naming each inherited amendment, decision IDs, approval state, immutable hash/commit, and supersession effect. Either cite the Boss D-GDA3 approval record or retain it explicitly as pending; do not label it approved by implication. Bind Boss's D-GDA4 decision to this candidate hash (or a committed successor) and require a fresh Terra review for any content-hash change. |

## Required correction gate

Before resubmitting this candidate to Boss, revise the candidate documentation
only and provide a fresh immutable hash. A re-review must confirm all of the
following:

1. Every current Desktop auth, enrollment, pairing, device, Drive, backup, and
   restore path has one typed broker contract and no legacy bypass.
2. Native custody is implementable across login, restart, refresh rotation,
   failure, logout, cancellation, and shutdown without serializing secrets.
3. The approved D-GDA2/D-GDA3 authority, replay, grant, and restore-intent
   constraints remain mandatory for every brokered Drive operation.
4. Browser and deferred Mobile behavior are isolated by an explicit import and
   build boundary, with a testable Desktop exclusion rule.
5. Boss can approve a precise content hash with a truthful inherited-decision
   record; a hash change restarts the Terra document review.

## External gates retained

- Boss documentation approval after the P0/P1 corrections and an immutable
  candidate hash; then a fresh bounded Luna implementation and independent
  Terra code/schema review before integration.
- Per-project, read-only Supabase migration-history, RLS, exposed-schema/Data
  API, function configuration, advisor, and execute-grant preflight before any
  separately authorized staging migration or Edge deployment.
- Deployed Edge/RPC verification that `anon`, authenticated WebView, foreign
  owners, Data API, and Edge roles cannot promote, rebind, grant, or bypass the
  required device/operation authority.
- Real Google installed-app configuration and consent; native PKCE callback,
  token exchange/refresh/revoke; Drive appDataFolder upload; and digest-bound
  restore evidence.
- Clean-install Windows keyring migration/reconnect/restore, physical
  Android/FUNGWIRE validation, signing, release, merge, and promotion.

## Scope and preservation audit

- Reviewed branch/head: `codex/backlog-truth-sync` at `3e2b38c`.
- Existing modified and untracked paths, including the candidate and unrelated
  documentation/code changes, were preserved.
- This report is the only review mutation. No deployment, push, merge, PR,
  deletion, or external message was performed.

## Version Diff

- `new -> 0.1.0b`: records an independent FAIL-for-correction of the Desktop
  native session broker candidate: it preserves the desired secret-boundary
  direction but lacks the exact broker, lifecycle, inheritance, platform, and
  approval contracts required for Boss approval.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | need review | FAIL: exact broker/lifecycle/authority/provenance corrections are required before Boss approval. | pending | Terra |
