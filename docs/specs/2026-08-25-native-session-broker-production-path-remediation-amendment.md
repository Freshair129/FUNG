---
version: "0.1.0b"
created_at: "2026-08-25T04:09:26+07:00,Luna 5.6"
last_update: "2026-08-25T04:09:26+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "system-design"
  scope: "Desktop/Tauri native session broker production-path convergence; Browser unchanged; Mobile deferred"
  risk: "HIGH"
  complexity: "C-3"
  authorization: "Boss approved drafting only: approve drafting Native Session Broker production-path remediation amendment"
  remediation_stage: "candidate after Terra fix3 FAIL/BLOCK and maximum three fix cycles"
  prior_approved_candidate_commit: "7d48aa01c243ce5f32af1005b95b71082c5a5984"
  prior_approved_candidate_sha256: "41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4"
  latest_implementation_commit: "36fa29412fc46a764e1bccae94e44bf0d4d7a6e5"
  terra_fix3_commit: "07649e7526243446f719a2dcab63e6bba5b94285"
  candidate_commit: "pending commit; not an implementation authorization"
  candidate_sha256: "pending post-commit computation; not an implementation authorization"
---

# Native Session Broker Production-Path Remediation Amendment — Desktop/Tauri

## Status and authority boundary

This is a reviewable HIGH-risk C-3 remediation authority candidate. It is a
documentation-only draft after Terra fix3 returned FAIL/BLOCK and the maximum
three authorized fix cycles were exhausted. It does not authorize implementation,
code changes, test changes, configuration, migration, deployment, release,
promotion, or production approval.

The candidate supersedes only the failed remediation authority for a possible
future fix4. It does not rewrite the fix1/fix2/fix3 audit history, invalidate the
recorded local passes, or close any production or external gate.

The exact future implementation may begin only after the approval rule in
`D-GDA5-06` is satisfied. Until then, all statements below are proposed
requirements and acceptance gates, not implementation claims.

## 1. Scope, non-scope, and provenance

### 1.1 Scope

The candidate is limited to convergence of the Desktop/Tauri production path for
the native session broker. It covers the production lifecycle used by actual
Tauri commands, the native auth and Drive custody boundaries, the registered
command path, and the bounded local evidence needed to prove that the tests and
production commands exercise the same lifecycle engine.

Browser behavior is unchanged. Mobile is deferred and receives no readiness or
secure-custody claim from this candidate. No provider configuration, Supabase or
Edge deployment, Google Console action, external transport action, release action,
or production go/no-go is in scope.

### 1.2 Trigger and immutable source record

| Evidence | Immutable source | Meaning |
|---|---|---|
| Approved D-GDA4 candidate | `7d48aa01c243ce5f32af1005b95b71082c5a5984`; candidate SHA-256 `41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4` | Prior hash-bound approval record; not approval for this candidate |
| Latest implementation | `36fa29412fc46a764e1bccae94e44bf0d4d7a6e5` | Fix3 implementation evidence reviewed by Terra |
| Terra fix3 review | `07649e7526243446f719a2dcab63e6bba5b94285` | Independent FAIL/BLOCK; production lifecycle and custody hard gates remain open |

Terra fix3 specifically found that the fourteen native behavioral tests exercised
`LifecycleCore` and fakes while actual Tauri login, refresh, logout, and shutdown
paths used separate `SessionMemory` control flow. It also found stale refresh/login
commit windows, ordinary secret-bearing `url::Url` and verifier allocations,
delayed Drive recovery-phrase custody, and non-fail-closed Drive keyring
readback. The narrow shutdown `cleanup_failed` propagation, typed IPC, and live
50-client replay proof remain preserved evidence and must not regress.

## 2. Inherited controls and retained compatibility warning

The candidate inherits the approved D-GDA2 authority/grant/replay/restore-intent
controls and D-GDA3 enrollment-proof nonce controls. It does not weaken
deny-before-keyring/provider order, one-use grants, durable replay reservation,
or digest-bound restore.

The already-passing results must remain true:

- shutdown cleanup failure returns the stable `cleanup_failed` result path and is
  not converted into successful shutdown;
- the PostgreSQL 17 50-client replay proof remains one winner, 49
  `proof_replayed` results, and zero loser mutation;
- Desktop IPC remains a closed typed operation surface with no generic forwarding;
- Browser sources remain in their browser session graph and Mobile remains
  deferred;
- the current P2 compatibility warning is retained: `GoogleDrivePanel.tsx`
  passes an ignored `invoke` argument, while `googleDriveFlow.ts` does not forward
  it. This warning is explicitly out of future write scope unless a separately
  approved scope amendment includes it.

## 3. Candidate decisions for Boss approval

| ID | Decision | Normative candidate requirement | Status |
|---|---|---|---|
| D-GDA5-01 | One production lifecycle engine | Define one `SessionLifecycle` production engine/control flow in the authorized native lane. Actual Tauri auth, refresh, logout, shutdown, Drive credential operations, and their command registration call this engine. The test harness constructs that same engine through explicit injectable ports for keyring, clock, listener/callback transport, provider, and commit observation. A generic test-only core, duplicate `SessionMemory` path, dead lifecycle seam, or test-only success path is prohibited in the non-test production build. | candidate |
| D-GDA5-02 | Linearizable generation and commit protocol | Every login, refresh, logout, and shutdown operation owns a generation and operation ID. External provider work may occur outside the lifecycle lock, but every credential/state mutation passes an exact precommit check and a serialized commit fence. Logout/shutdown increment the generation and enter quiescing at their linearization point before clearing memory or deleting credentials. A stale completion cannot write staged, active, or in-memory credentials after that point. | candidate |
| D-GDA5-03 | Zeroizing custody boundary | Secret input enters zeroizing custody at Tauri command entry and remains there through parser, generator, provider request, error, cancellation, timeout, logout, and shutdown terminal paths. Callback bytes/query values, PKCE verifier, OAuth URL state/challenge, Drive recovery phrase, and token payloads must not use ordinary secret-bearing `String` or `url::Url` serialization. Static endpoint URLs and public non-secret identifiers may use ordinary types. If a provider or parser API makes an ordinary allocation unavoidable, it requires a narrowly reviewed boundary with explicit lifetime, no retention/logging, immediate zeroization or disposal, and a focused test proving the boundary; an unreviewed ordinary secret-bearing allocation is a FAIL. | candidate |
| D-GDA5-04 | Fail-closed keyring taxonomy | Distinguish `NoEntry` from transient, unavailable, read, write, delete, and absence-verification failures. `NoEntry` alone means normal absence: read returns absent, idempotent delete succeeds, and verified absence succeeds. Any transient/unavailable/read error is not treated as absence, does not permit a connected/signed-out success claim, and blocks credential use. Logout/shutdown cleanup errors return `cleanup_failed` (or the existing stable operation-specific incomplete code) and retain a non-secret failure state. Tests must inject and assert each taxonomy branch. | candidate |
| D-GDA5-05 | Production-path evidence and bounded matrix | The implementation and tests must prove command-to-engine identity, exact stale interleavings, custody boundaries, keyring taxonomy, retained local passes, and typed IPC with a bounded write/test matrix. Evidence must identify the source commit, exact commands, exit/status summaries, changed paths, and hashes without secret values. Local/static proof must remain labelled separately from external gates. | candidate |
| D-GDA5-06 | Approval, review, and external gates | Future implementation is authorized only after Boss approves this candidate’s exact Git commit plus the exact 64-character SHA-256 of this candidate file, and a fresh independent Terra document review covers the same bytes. Any candidate byte, metadata, line ending, or scope change invalidates the approval and requires a new hash and fresh Terra review. Provider, environment, VM, device, signing, release, deployment, and production gates remain separately open. | candidate |

## 4. Required production lifecycle protocol

The future engine must make the following interleavings explicit in code and
tests. “Commit point” means the first serialized point at which a credential,
session state, or terminal outcome becomes authoritative; all listed writes and
the in-memory publish are covered by the same commit fence or an equivalent
linearizable protocol.

### 4.1 Login

1. `login_begin` reserves `(generation, operationId)` and stores callback state,
   verifier, listener ownership, and deadline in zeroizing native custody.
2. Provider/browser callback and exchange run as external work. At **precommit**,
   the completion holds the engine gate and verifies the operation is still
   pending, the generation is unchanged, the callback is exact and single-use,
   and the engine is not quiescing.
3. At **commit**, staged refresh write/readback, active refresh write/readback,
   staged delete/verified absence, and authenticated in-memory publish occur in
   the defined order while logout/shutdown cannot interleave as a competing
   generation.
4. At **postcommit**, the operation is terminal, listener/callback custody is
   disposed, and only a redacted result is returned. A completion arriving after
   logout/shutdown returns a stale/transition error and performs no credential
   mutation.

### 4.2 Refresh

1. The first caller reserves one refresh flight and `(generation, operationId)`;
   other callers join it and cannot start a second provider refresh.
2. At **precommit**, the returned material is checked against the still-current
   generation and flight owner. At **commit**, the same staged/active/readback/
   delete order is performed under the commit fence, followed by the in-memory
   access-token publish.
3. At **postcommit**, all waiters receive the same redacted success or public
   error. Logout/shutdown winning before commit causes no keyring or memory
   resurrection; invalid/revoked material is deleted only through the typed
   fail-closed cleanup path.

### 4.3 Logout

1. Logout’s linearization point is the engine-gate transition to quiescing plus
   generation increment. It cancels pending callback work, prevents new protected
   work, and invalidates all earlier operation IDs.
2. It clears usable access memory, deletes active and staged credentials, and
   verifies absence. Only `NoEntry` proves absence; any other read/delete error
   returns the stable cleanup-failure result and non-secret failure state.
3. A login/refresh completion at precommit, commit, or postcommit is tested at
   each point. It must either win before logout’s linearization point and then be
   cleaned by logout, or lose after it with zero credential/state resurrection.

### 4.4 Shutdown

Shutdown uses the same engine and the same linearization protocol as logout, with
terminal `shutdown` state. It stops admission, invalidates pending generations,
disposes zeroizing custody, deletes and verifies keyring entries, and preserves
the already-passing `cleanup_failed` result propagation through the Tauri exit
hook. Shutdown is not cancellation and must not silently report success when
credential cleanup is unproven.

## 5. Secret-custody requirements

The implementation must use a byte-oriented or zeroizing native boundary at each
listed entry and parser/generator point:

| Boundary | Required future behavior | Required proof |
|---|---|---|
| Tauri command entry | Secret-bearing callback, PKCE, token, and recovery inputs are deserialized directly into a zeroizing wrapper or byte buffer; no ordinary secret-bearing command parameter is retained. | Source scan plus malformed/cancel/timeout/error tests |
| Callback parser | Parse request bytes without constructing an ordinary secret-bearing `url::Url` or persistent query `String`; decoded state/code/error material is zeroizing and single-use. | Exact callback, duplicate, malformed, oversized, and terminal-path tests |
| PKCE generator | Verifier is born in zeroizing custody; challenge derivation does not first retain an ordinary verifier `String`. | Static custody assertion and generator lifecycle test |
| OAuth authorization URL | State/challenge/callback data is encoded into a zeroizing byte/string buffer without ordinary secret-bearing `url::Url` query serialization. Public static origin and non-secret parameters remain separately allowed. | URL construction scan and no-secret-output test |
| Provider token payload | Response bytes and access/refresh/error fields use zeroizing custody; no ordinary `String::deserialize` or retained token-bearing JSON value is accepted. | Success, malformed, refresh failure, and disposal tests |
| Drive recovery phrase | Command entry receives zeroizing custody before validation or any async/provider work; it is never retained as an ordinary `String`, logged, echoed, or placed in a job/public DTO. | Restore success/failure/cancel/shutdown custody tests |
| Narrow unavoidable boundary | Only a specifically named adapter boundary may temporarily materialize ordinary bytes for a library call. It must document why it is unavoidable, prove no retention/logging, dispose immediately, and have a focused test. | Terra must review the named boundary; absence of that proof is a hard FAIL |

## 6. Keyring taxonomy and fail-closed behavior

The internal taxonomy must be explicit and testable:

| Backend result | Internal class | Read/status behavior | Delete/cleanup behavior |
|---|---|---|---|
| `NoEntry` | `Absent` | Normal absence; return no credential or the existing public `auth_required`/disconnected result | Idempotent success; absence verification passes |
| Transient backend condition | `Transient` | Return transient/unavailable public error; never use or overwrite a presumed absent credential | Return cleanup failure; do not claim signed out/shutdown success |
| Backend unavailable/locked/not configured | `Unavailable` | Return `keyring_unavailable` or operation-specific equivalent; fail closed | Return cleanup failure; retain non-secret failure state |
| Other read/decode/access failure | `ReadError` | Return a distinct read-failure public error; never interpret as `NoEntry` | Return cleanup failure; absence is unproven |
| Write failure | `WriteError` | No authenticated/connected commit | Abort commit and return storage failure; no partial success |
| Delete/absence-verification failure | `DeleteError`/`VerifyAbsentError` | No signed-out/shutdown success | Return `cleanup_failed` and preserve failure state |

The exact public code may remain operation-specific only when its mapping to this
taxonomy is documented and tested. Catch-all `Err(_) => absent` behavior is
prohibited.

## 7. Exact future implementation write set

After the approval rule is satisfied, the future implementation may modify only:

1. `src-tauri/src/auth_session.rs`
2. `src-tauri/src/drive_oauth.rs`
3. `src-tauri/src/lib.rs`
4. `tests/nativeSessionCustody.test.mjs`
5. `tests/googleDriveContract.test.mjs`
6. the future implementation report at
   `docs/verification/implementation-reports/2026-08-25-native-session-broker-production-path-remediation-luna-report.md`

No `package.json`, lockfile, Cargo/config/capability file, migration, provider
configuration, Browser file, Mobile file, `GoogleDrivePanel.tsx`, or other path
is included. If compilation or the exact production-path proof appears to
require another path, implementation stops and requests a separately approved
scope amendment; it must not expand this write set by inference.

## 8. AC-GDA5 success, exit, and evidence criteria

All criteria are exact gates for a future implementation. None is satisfied by
this drafting task.

| ID | Required success criterion |
|---|---|
| AC-GDA5-01 | Actual Tauri login, refresh, logout, shutdown, Drive credential commit/delete, and registered command paths invoke the same `SessionLifecycle` engine methods that the tests exercise through explicit injectable ports. The evidence names the shared call path and contains no separate generic test-only core. |
| AC-GDA5-02 | Non-test production build contains no parallel generic test-only core, dead lifecycle seams/methods, or unused production lifecycle authority. Compiler output and a source/command inventory prove the engine is live. |
| AC-GDA5-03 | Login, refresh, logout, and shutdown stale-interleaving tests pause at exact precommit, commit, and postcommit points and assert no stale credential, session state, staged key, active key, or public success is resurrected. |
| AC-GDA5-04 | Callback, PKCE verifier, authorization state/challenge, Drive recovery phrase, and access/refresh/token payload custody checks cover command entry, parser/generator boundaries, provider failure, malformed input, cancellation, timeout, logout, and shutdown with no ordinary secret-bearing `String`/`url::Url` path except a named Terra-reviewed boundary. |
| AC-GDA5-05 | Keyring tests distinguish `NoEntry`, transient, unavailable, read, write, delete, and absence-verification outcomes; only `NoEntry` is treated as normal absence, and all other failure classes fail closed. |
| AC-GDA5-06 | Shutdown cleanup failure still propagates as `cleanup_failed` through the production exit path; it is never converted to successful shutdown. |
| AC-GDA5-07 | The existing 50-client replay evidence remains one winner, 49 `proof_replayed`, and zero loser mutation, with no altered schema or provider deployment in this lane. |
| AC-GDA5-08 | Typed IPC remains closed and operation-specific; no generic invoke/HTTP/URL/header/token forwarding or secret-bearing legacy alias is registered. The ignored `GoogleDrivePanel.tsx` argument remains a P2 warning and is not changed. |
| AC-GDA5-09 | Browser source behavior is unchanged and Mobile remains deferred; no Browser/Mobile file is in the exact write set or claimed as ready. |
| AC-GDA5-10 | Required local evidence passes: `node --test tests/nativeSessionCustody.test.mjs`, `node --test tests/googleDriveContract.test.mjs`, `node --test tests/w1AuthoritySchema.test.mjs` (unchanged 50-client proof), `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture`, `cargo check --manifest-path src-tauri/Cargo.toml`, `npm run build`, and `git diff --check -- src-tauri/src/auth_session.rs src-tauri/src/drive_oauth.rs src-tauri/src/lib.rs tests/nativeSessionCustody.test.mjs tests/googleDriveContract.test.mjs`. Evidence records exact commit/path/hash output and contains no secrets. |
| AC-GDA5-11 | Local/static evidence is reported separately from external gates; no local command is described as real Supabase/Edge/RLS, Google provider, clean-VM keyring, device UAT, signing/release, deployment, or production approval. |
| AC-GDA5-12 | Exit requires fresh independent Terra review of the exact candidate bytes, Boss approval of the exact candidate Git commit and 64-character candidate SHA-256, and a separate disposition for every external gate. |

### 8.1 Required local command/evidence record

The future implementation report must record the exact invocation and exit result
for each applicable command, the relevant test count/output summary, changed-path
list, source commit, candidate commit, candidate file SHA-256, and `git diff
--check`. The report must distinguish:

- **Local/static:** source audit, command inventory, Rust/Node/build checks,
  lifecycle interleavings, custody checks, keyring taxonomy, typed IPC, and the
  retained 50-client proof;
- **External/open:** real Supabase/Edge/RLS authorization, Google provider UAT,
  clean Windows keyring/VM, supported-device UAT, signing/release, deployment,
  and production approval.

## 9. External gates remain open

The following are explicitly outside this candidate and remain open:

1. Real Supabase/Edge/RLS authorization, durable reservation, grant/revocation,
   audit, and deny-before-secret/provider evidence.
2. Google installed-app/provider UAT for consent, refresh, revocation,
   appDataFolder upload, digest-bound restore, and cancellation.
3. Clean Windows VM OS-keyring startup, rotation, logout, shutdown, and stale
   completion proof.
4. Supported-device UAT, signing, release artifact/publication, deployment,
   promotion, and explicit production approval.

## 10. Exact approval rule

Future implementation is permitted only after Boss approves the exact Git commit
of this candidate and the exact 64-character SHA-256 of this candidate file, and
a fresh independent Terra document review confirms the same bytes. Any candidate
change—including metadata, wording, line endings, scope, or decision IDs—invalidates
the approval and requires a new SHA-256 and fresh Terra review. A commit alone is
not approval; this draft’s approval status is not implementation evidence.

## Version Diff

- `new -> 0.1.0b`: drafted a narrow fix4 remediation authority candidate after
  Terra fix3 FAIL/BLOCK and exhaustion of the maximum three fix cycles; added the
  shared production lifecycle, linearizable commit, custody, keyring taxonomy,
  bounded evidence, exact write set, AC-GDA5 gates, and hash-bound approval rule.
- Prior D-GDA4 candidate, fix3 report, approval record, code, tests, Browser,
  Mobile, provider configuration, and external gates remain unchanged.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-25 | candidate | Drafted HIGH-risk C-3 Desktop/Tauri production-path remediation authority candidate after Terra fix3 FAIL/BLOCK; implementation and external gates remain unauthorized/open. | pending post-commit | Luna 5.6 |
