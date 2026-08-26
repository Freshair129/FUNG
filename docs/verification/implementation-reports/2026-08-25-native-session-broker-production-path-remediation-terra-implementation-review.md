---
version: "1.0.0"
created_at: "2026-08-25T16:00:00+07:00,Terra 5.6"
last_update: "2026-08-25T16:00:00+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-implementation-review"
  scope: "Bounded review of D-GDA5 native-session-broker production-path remediation only"
  risk: "HIGH"
  complexity: "C-3"
  approved_candidate_commit: "bcc672decd3ae35cf7875ca2f984a7919aafbe6b"
  approved_candidate_sha256: "B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8"
  candidate_document_review: "baf030d65698ef2c060d10464cd85262706a27dd"
  implementation_target: "fc45d7023804472f7fc4a5d4a05978140e789d7c"
  verdict: "FAIL"
  recommendation: "BLOCK implementation acceptance; remediate under a fresh approved scope and obtain a new independent review."
---

# Native Session Broker Production-Path Remediation — Terra 5.6 Independent Implementation Review

## Verdict

**FAIL / BLOCK.** The immutable implementation target does not meet the approved
D-GDA5 production-path requirements. Three independently reproduced P0 defects
remain in the actual production call graph, with two additional P1 defects and a
materially inaccurate local-acceptance table in the implementation report.

The two bounded Node contracts pass, but local tests cannot override source P0
defects. This is not a production-readiness claim and authorizes no push, PR,
merge, deployment, release, or external action.

## Reviewed provenance

| Item | Independent result | Disposition |
|---|---|---|
| Approved candidate commit | `bcc672decd3ae35cf7875ca2f984a7919aafbe6b` exists. | PASS |
| Candidate document bytes | The candidate specification blob hashes to `B1181942C9D98601EC96D4BAB9FA81D6DFFC78FE81A098AA6F461ACA1EE976C8`, exactly matching the supplied 64-character SHA-256. | PASS |
| Candidate document review | `baf030d65698ef2c060d10464cd85262706a27dd` is the prior Terra document-only PASS; it did not certify implementation. | PASS, correctly bounded |
| Target | `fc45d7023804472f7fc4a5d4a05978140e789d7c` (`fix(auth): converge native session lifecycle production path`). | REVIEWED |
| Parent manifest | Exactly six paths: the Luna report, `auth_session.rs`, `drive_oauth.rs`, `lib.rs`, and the two named Node tests. | PASS |
| `native_auth.rs` | Absent from the target manifest; target and parent blobs are both `d8e689616310d2fdbe2a8a26a2b613c84b916ede`. | PASS |
| Candidate-to-target diff | Deliberately not used to decide the six-path implementation scope: target is not a direct child of the candidate, so it includes intervening documentation history. | LIMITATION |

## Review method and command evidence

All source conclusions below were read from `fc45d702...` with Git object
inspection, not inferred from the Luna report.

| Bounded check | Result | Limitation / interpretation |
|---|---|---|
| `git diff-tree --no-commit-id --name-status -r fc45d702...` | PASS: exactly six target-parent paths. | Scope integrity only. |
| Candidate blob SHA-256 calculation | PASS: exact expected value above. | Binds candidate documentation, not implementation correctness. |
| `git diff --check fc45d702...^ fc45d702...` | PASS. | Whitespace is not a security or lifecycle proof. |
| Target-path high-risk secret-pattern scan | No API-key, GitHub-token, OpenAI-key, or private-key pattern matched. | Bounded pattern scan; not a credential audit of the whole dirty workspace. |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Exit 0, **24 warnings**. | Reproduces P0-01 dead production seams below. |
| `node --test tests/googleDriveContract.test.mjs` | PASS: 5/5, 0.28 s. | Contract test does not prove stale-operation fencing. |
| `node --test --experimental-strip-types tests/authFlow.test.mjs` | PASS: 8/8, 0.24 s. | Auth-flow contract does not prove ticket ownership/fencing of native HTTP paths. |
| `node --test tests/nativeSessionCustody.test.mjs` | **Not rerun.** | Prior reviewer checkpoint evidence supplied for this review records a 180 s timeout. The target wrapper itself sets `timeout: 180000` at `tests/nativeSessionCustody.test.mjs:164`; that configuration is corroborated, but no new test result is claimed here. |

## Independent root-cause conclusion

**Symptom.** The target reports a shared production lifecycle and local AC-GDA5
success, while the registered production code keeps free-function lifecycle and
HTTP paths separate from the injectable test broker and permits stale credential
and Drive work.

**Evidence.** The non-test compiler warnings, production free-function call
graph, ticket lifetime, keyring protocol, and command ingress described below
are all in the immutable target.

**Root cause.** `SessionLifecycle<K,C,L,P>` is exercised through `cfg(test)`
fakes, while production adapters retain separate direct listener, HTTP, refresh,
keyring, and enrollment paths. Drive authorization is represented by a ticket
only through refresh-token acquisition rather than the full provider operation.
The resulting split cannot provide the candidate's single-engine admission,
generation, precommit, and post-operation fencing guarantees.

**Why tests escaped it.** The focused broker is constructed with
`FakeKeyring`, `FakeClock`, `FakeListener`, and `FakeProvider` at
`auth_session.rs:2352-2358`; the passing bounded Node checks are contract tests.
Neither is an execution of the actual production listener/HTTP paths.

**Required prevention.** Do not accept a repair until the exact registered
commands and the behavioral harness use one injected production engine, with
operation tickets preserved and rechecked across every provider/keyring effect,
direct zeroizing ingress, and both-domain crash/fault recovery.

## Findings

### P0-IMP-01 — Production lifecycle is not the tested injectable lifecycle

**Severity: P0. Affected gates: D-GDA5-01, D-GDA5-05; AC-GDA5-01, -02, -10.**

`cargo check` reports unused non-test port methods at
`auth_session.rs:64-73` (`KeyringPort`), `76-79` (`ClockPort`), `80-84`
(`ListenerCallbackPort`), and `90-100` (`ProviderHttpPort`). The production
path instead directly runs `spawn_listener` at `1206-1250`, `auth_user` at
`1252-1272`, `exchange_code` at `1274-1316`, `refresh_from_keyring` at
`1318+`, and `finish_login` at `1501+`. The injectable construction is only
the test broker at `2352-2358`.

This violates the candidate's prohibition on a dead lifecycle seam or
test-only success path (candidate lines 101 and 127-130). The compiler output
is direct evidence that the named production ports are not the live production
path. The P0 is independently reproduced.

### P0-IMP-02 — Drive operation ticket ends before list/upload/restore work

**Severity: P0. Affected gates: D-GDA5-02, D-GDA5-05; AC-GDA5-01, -03, -10.**

`access_token` checks and finishes its `LifecycleTicket` at
`drive_oauth.rs:914-916`, then returns a token. `broker_drive_list_archives`
starts list work only afterwards at `958-971`; upload obtains the token at
`1011` and starts blocking provider work at `1024-1039`; restore obtains the
token at `1389` and performs list/download/provider work through at least
`1395-1415`. `list_files` only calls `DriveInvocation::ensure_valid()` before
its provider call (`929-955`), and no account/Drive lifecycle ticket is held
or rechecked after provider work.

Therefore a disconnect, logout, or shutdown can win after `drive_finish` and
before or during provider work without the required post-work ticket fence.
The candidate requires each Drive boundary to reject stale/quiescing work and
prevents effects after disconnect/logout/shutdown (candidate lines 102 and
127-130). The P0 is independently reproduced.

### P0-IMP-03 — Keyring recovery and compensation are not failure-atomic for both domains

**Severity: P0. Affected gates: D-GDA5-04, D-GDA5-05; AC-GDA5-03, -05, -10.**

With a valid marker, `load_committed` at `auth_session.rs:695-717` reads only
the selected slot and neither deletes nor verifies old/orphan slots. With no
marker it scans only `1..=RECOVERY_SLOT_LIMIT` (`702-706`), a bounded range.
`compensate_commit` deletes the new slot **and the marker** at `720-739`.
`commit_credential` invokes that compensation after a new-slot failure,
readback failure, marker write/readback failure, and old-slot cleanup failure
(`755-781`). If an old marker was authoritative before a new marker had
verified linearization, compensation can delete the old authority.

Startup recovery invokes `load_committed` only for `ACCOUNT_DOMAIN` /
`ACCOUNT_MARKER` at `930-943`, not the DriveCredential domain. This conflicts
with the candidate's two-domain marker/slot order and mandatory verified
cleanup before success/access (candidate lines 299-322). The P0 is
independently reproduced.

### P1-IMP-04 — Recovery phrase has ordinary `String` ingress

**Severity: P1. Affected gates: D-GDA5-03, D-GDA5-05; AC-GDA5-04, -10.**

The registered `broker_drive_restore` command accepts `recovery_phrase:
String` at `drive_oauth.rs:1360-1368` and only wraps it in `Zeroizing` at
`1369`. Candidate B1 explicitly requires direct custom-deserialization or
equivalent zeroizing custody and prohibits retaining an ordinary app-owned
secret-bearing command parameter (candidate lines 261-273). The P1 is
independently reproduced.

### P1-IMP-05 — Enrollment/device/pairing HTTP has no lifecycle operation fence

**Severity: P1. Affected gates: D-GDA5-01, D-GDA5-02, D-GDA5-05; AC-GDA5-01, -03, -10.**

`native_post` obtains an access token with `ensure_access_token()` at
`auth_session.rs:1739-1743`, performs the HTTP request at `1746-1756`, and
deserializes/returns at `1757-1771`. It reserves no account operation ID or
epoch ticket and makes no post-operation lifecycle recheck. Registered
enrollment/device commands use this helper, for example enrollment at
`1774-1785` and device revoke at `2119-2130`. This fails the candidate's
required enrollment/device admission boundary with account logout/shutdown
invalidation (candidate lines 119-130). The P1 is independently reproduced.

### P1-IMP-06 — Luna AC-GDA5 table is materially misnumbered

**Severity: P1. Affected gate: D-GDA5-05.**

The approved candidate defines custody as AC-GDA5-04, keyring taxonomy/recovery
as -05, shutdown cleanup as -06, 50-client evidence as -07, typed IPC as -08,
and required local evidence as -10 (candidate lines 380-387). The target Luna
implementation report at lines 118-124 instead labels keyring as -04, NoEntry
as -05, custody as -06, typed IPC as -07, and Drive/50-client material as -08.
This materially misstates which acceptance criteria the claimed local evidence
addresses. The report cannot be relied on for gate disposition.

## D-GDA5 and AC-GDA5 dispositions

| Gate | Independent disposition | Basis |
|---|---|---|
| D-GDA5-01 | FAIL | P0-IMP-01 and P1-IMP-05: commands do not demonstrably use the same live injected engine. |
| D-GDA5-02 | FAIL | P0-IMP-02 and P1-IMP-05 lack full-operation generation/epoch fencing. |
| D-GDA5-03 | FAIL | P1-IMP-04 violates B1 direct-ingress custody. |
| D-GDA5-04 | FAIL | P0-IMP-03 violates both-domain failure-atomic recovery/cleanup. |
| D-GDA5-05 | FAIL | P0 evidence matrix is not production-path proof; P1-IMP-06 misnumbers AC dispositions. |
| D-GDA5-06 | PASS, provenance only | Candidate commit/hash and prior document PASS were verified; implementation acceptance is still FAIL. |
| AC-GDA5-01 | FAIL | Same-engine actual command mapping is not established. |
| AC-GDA5-02 | FAIL | Compiler warnings establish dead non-test seams. |
| AC-GDA5-03 | FAIL | Drive and enrollment stale-operation fencing is absent. |
| AC-GDA5-04 | FAIL | Ordinary recovery-phrase ingress remains. |
| AC-GDA5-05 | FAIL | Both-domain deterministic recovery/fault matrix is not implemented. |
| AC-GDA5-06 | BLOCKED | Not independently promoted; source P0s prevent acceptance of terminal cleanup claims. |
| AC-GDA5-07 | UNVERIFIED | Retained 50-client proof was not rerun in this bounded review. |
| AC-GDA5-08 | UNVERIFIED | Typed IPC was not independently rerun/audited beyond the target's limited scope. |
| AC-GDA5-09 | PASS, scope only | No Browser or Mobile path is in the six-path target manifest. |
| AC-GDA5-10 | FAIL | Limited local passes cannot satisfy it while source P0 hard gates fail; native custody test was not rerun. |
| AC-GDA5-11 | PASS, reporting boundary only | This report keeps local/static evidence separate from external gates. |
| AC-GDA5-12 | FAIL | Fresh independent implementation review has occurred and returns FAIL/BLOCK. |

## Scope integrity and external gates

The worktree contained pre-existing modified/untracked files outside the target,
including Desktop plans/status/reviews, BackupPanel, AccountSettings, Supabase
README, RCA/draft artifacts, a temporary transcript directory, and Drive CSS.
They were not inspected as target implementation, changed, staged, or included
by this review. Only this new report is to be committed.

No matching high-risk secret pattern was found in the six immutable target
paths. This narrow result does not certify the unrelated dirty workspace.

Real Supabase/Edge/RLS authorization, Google-provider UAT, clean Windows
keyring/VM behavior, supported-device UAT, signing, release, deployment,
production monitoring, and explicit production approval remain open. No local
test or source review closes any of them.

## Recommendation

**BLOCK acceptance of `fc45d702...`.** Remediation must be documented and
approved under a fresh bounded scope, then independently re-reviewed against an
exact new implementation commit. It must repair all P0 findings before a P1 or
external gate is considered. Do not represent this target as production-ready.

## Version Diff

- New independent implementation review for `fc45d702...`.
- Records three source-backed P0 failures, two source-backed P1 failures, and
  one materially inaccurate AC report mapping.
- Preserves candidate/document provenance, target scope integrity, unrelated
  workspace dirt, and all external gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-25 | stable | FAIL/BLOCK: immutable target misses D-GDA5 production-path, fencing, recovery, custody, and evidence requirements. | recorded by this review commit | Terra 5.6 |
