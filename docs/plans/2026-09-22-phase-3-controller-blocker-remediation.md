---
version: "0.2.0b"
created_at: "2026-09-22T10:46:30+07:00,RWANG,base-538213f"
last_update: "2026-09-22T14:58:44+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "remediation-plan"
  scope: "FUNG Phase 3 Controller Gate blockers P3-AC-04 and P3-AC-06"
  language: "Thai-English"
  execution_status: "LIVE_CONTROLLER_GATE_NOT_RUN"
  implementation_status: "LOCAL_REMEDIATION_COMPLETE; TERRA_FINAL_PASS"
  live_acceptance_status: "NOT_RUN"
  complexity: "C-3"
  risk: "HIGH"
  commit_provenance: "working-tree; no new commit; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b"
  authority: "documentation record; live provider execution requires separate Boss/user approval"
---

# FUNG Phase 3 Controller Blocker Remediation Plan

This is a beta, source-evidenced remediation record for the two former Phase 3
Controller Gate blockers. Their pre-remediation symptoms and RCA remain below
as historical baseline evidence:

- P3-AC-04: effective LLM provider/model/endpoint provenance was false after a
  cloud fallback.
- P3-AC-06: the daily cloud cap was a non-atomic rate guard rather than a
  hard, fail-closed admission invariant.

The approved remediation is now present in the six approved Rust files and has
local evidence plus a Terra final gate of **FINAL PASS; no HIGH blockers**.
The live Controller Gate, real-provider execution, secret/keyring use, device,
CI, production, Git, and deployment evidence remain **NOT_RUN**. This document
does not authorize any of those actions.

The RCA record is intentionally embedded here because the exact-one-new-file
constraint permits no separate RCA artifact.

## 1. Authority, baseline, and success criteria

### 1.1 Authority and evidence boundary

The plan is aligned to:

- the Phase 3 section and execution protocol in
  docs/plans/2026-08-09-fung-master-implementation-plan.md;
- the desktop persistence/provenance boundary in
  docs/Desktop/ARCHITECTURE.md;
- the evidence-only mobile status in docs/Mobile/IMPLEMENTATION_STATUS.md;
- the current desktop truth in docs/Desktop/08-real-progress.md;
- the approved Phase 3 design in
  docs/specs/2026-08-09-phase-3-byom-cloud-keys-design.md;
- the implementation plan in
  docs/plans/2026-08-09-phase-3-byom-cloud-keys.md; and
- the existing, preserved runbook in
  docs/plans/2026-09-22-phase-3-byom-acceptance-runbook.md.

The runbook remains the acceptance authority for the later controller run. The
implementation remediation and local retest are now complete and Terra-reviewed;
the live P3-AC-04 and P3-AC-06 verdicts remain **NOT_RUN** until the required
real-provider rows, cleanup, and separate Boss/user approval are complete.

### 1.2 Definition of done for this remediation

The remediation is ready for a later Controller Gate only when all of the
following are true:

1. A successful local graph build persists the same local provider ID, model,
   endpoint, and runtime location as before.
2. A successful cloud graph fallback persists the effective cloud provider,
   effective model, fixed/configured cloud endpoint, and
   runtime_location=cloud in model_runs, with no API key or prompt in any
   persisted provenance.
3. Local failures, policy blocks, cloud errors, and provenance-persistence
   errors do not create a false model_runs row and do not trigger an
   undocumented provider switch or automatic retry.
4. Every allowed STT or LLM cloud dispatch reserves its task-kind cap slot
   atomically before provider dispatch. A reservation or counter persistence
   error prevents provider contact.
5. Concurrent reservations cannot admit more than daily_cap STT calls or more
   than daily_cap LLM calls for the local calendar day. The one shared
   daily_cap setting applies independently to the STT and LLM rows, as required
   by the approved design; it is not a combined STT+LLM total.
6. Focused tests pass for provenance, local regression, concurrent cap
   boundaries, counter-write failure, independent counters, error paths, and
   key leakage. Tests use local fixtures/fake HTTP only.
7. Terra has independently reviewed the implementation and test evidence, the
   acceptance runbook has been updated in a separately approved documentation
   change, and Boss/user has approved moving to live acceptance.

No real provider call, secret/keyring use, CI run, physical-device run,
production action, Controller Gate run, commit, push, PR, merge, or deployment
is part of the recorded implementation or local verification evidence.

## 2. Historical baseline and root-cause analysis / pre-remediation 2026-09-22

The following subsections preserve the source-grounded baseline captured before
the remediation. They describe the former defect and why it escaped detection;
they are not the current implementation verdict. Current post-evidence is in
§2.3.

### 2.1 P3-AC-04 — historical false effective-provider provenance

#### Symptom

Before remediation, when local Ollama was unreachable and the configured cloud
LLM succeeded, the fallback correctly returned runtime_location=cloud, but the
persisted model_runs row could still claim the local Ollama provider, local
model, and local endpoint. Graph/summary completion therefore could not be
accepted as truthful provenance.

#### Evidence

1. src-tauri/src/graph_build.rs:645-656 receives only the runtime-location
   value from cloud_executor::call_llm_with_fallback.
2. src-tauri/src/graph_build.rs:660-665 persists provider_id
   "ollama-summary-intent", the local model variable, and the local endpoint
   unconditionally while using the returned runtime_location.
3. src-tauri/src/cloud_executor.rs:395-428 defines RUNTIME_LOCAL and
   RUNTIME_CLOUD and returns only a text plus that location, so the effective
   provider/model/endpoint does not cross the fallback boundary.
4. The pre-remediation runbook recorded the same defect at its source evidence:
   graph_build.rs:665 writes the Ollama identity even when runtime location is
   cloud, and marks P3-AC-04 BLOCKED_PENDING_REMEDIATION.
5. model_runs requires provider_id, model_name, task_kind,
   runtime_location, parameters_json, and a provider_id foreign key; see
   src-tauri/src/genesis_adapter.rs:350-366. A cloud row cannot truthfully
   retain the local provider identity.

#### Root cause

The pre-remediation fallback API propagated transport location but not a
secret-free effective execution descriptor. graph_build.rs owned the Genesis
write and therefore could not receive the key-bearing CloudProviderConfig under
the existing static leak guard, but it had no separate non-secret provenance
DTO to use. The persistence code consequently reused the local variables
captured before fallback. The runtime label and the provider/model/endpoint
fields were allowed to disagree.

#### Why the issue escaped detection

The existing cloud tests prove that a fake cloud response is returned and that
the fallback reports RUNTIME_CLOUD, for example
src-tauri/src/cloud_executor.rs:673-696. They do not run a successful graph
build through the Genesis model_runs write and assert the stored provider,
model, endpoint, and runtime-location tuple. The local regression test at
src-tauri/src/graph_build.rs:870-914 covers preservation of a prior extraction
on failure, not cloud provenance after success. The existing static key-leak
test protects the key boundary but cannot prove semantic provider identity.

#### Prevention

The required prevention was to use a secret-free effective-execution value as
the only provenance input to graph persistence; test the full mapping and the
stored model_runs row for both local and cloud success, plus no-row/error
behavior; and retain the static key-leak guard with a sentinel-key assertion.
The delivered implementation and local evidence for this prevention are
recorded in §2.3 and the current runbook matrix.

### 2.2 P3-AC-06 — historical non-atomic daily cap and ignored counter failures

#### Symptom

Before remediation, concurrent STT or LLM dispatches could all read the same
count below the cap, pass the policy check, and contact a provider before their
separate increments serialized. A counter write could also fail after a paid
cloud call while the caller ignored the error. The cap was therefore a
best-effort rate guard, not a hard fail-closed invariant.

#### Evidence

1. src-tauri/src/policy.rs:35-43 explicitly documents that the
   read/check/increment sequence is non-transactional and can exceed the cap
   under simultaneous FUNGWIRE connections.
2. src-tauri/src/policy.rs:195-220 implements calls_today as a SELECT and
   increment_calls_today as a later UPSERT; there is no reservation held
   across the provider boundary.
3. src-tauri/src/fungwire_server.rs:1130-1155 reads calls_today and calls
   decide_cloud_tier before dispatch, while
   src-tauri/src/fungwire_server.rs:1228-1234 calls increment_calls_today
   after the cloud result and discards its Result with let _.
4. src-tauri/src/cloud_executor.rs:443-456 does the same for LLM: it calls
   the provider first and ignores increment_calls_today errors.
5. The current test
   src-tauri/src/fungwire_server.rs:2592-2735 is named
   cloud_stt_result_survives_a_failed_counter_increment and intentionally
   accepts a successful cloud result despite a failed counter write. That
   expectation is the defect for a hard fail-closed cap.
6. Existing policy tests are sequential/in-memory checks at
   src-tauri/src/policy.rs:227-410. They prove task-kind independence and
   ordinary database errors for a read, but not concurrent admission or a
   failed write before provider contact.

#### Root cause

Admission is split into a stale read/check and a later write. The provider
request occurs in the gap, and the write is treated as advisory. There is no
database transaction that reserves one slot before either cloud executor can
send data, and there is no defined behavior for an ambiguous or failed
counter write.

#### Why the issue escaped detection

The original design and tests treated the daily count as a rate-limit-style
guardrail. Fake HTTP tests ran one request at a time, and the former
counter-write test codified successful-result preservation rather than
fail-closed admission. No test started multiple connections against the same
WAL database at the cap boundary.

#### Prevention

The required prevention was one SQLite transaction as the authoritative
admission boundary: reserve a task-kind slot before provider dispatch, return
every transaction error, and never call a provider when the reservation did not
commit. The delivered implementation and local evidence are recorded in §2.3
and the current runbook matrix.

### 2.3 Current post-evidence — 2026-09-22

The six approved Rust files now contain the AC-04/AC-06 remediation:

| Path | Current remediation boundary |
|---|---|
| `src-tauri/src/cloud_config.rs` | Treats blank cloud keys as unconfigured while preserving the keyring-only boundary. |
| `src-tauri/src/cloud_executor.rs` | Returns secret-free effective LLM provenance and reserves the LLM cap before cloud dispatch; admission errors are fail-closed and classified as non-retryable. |
| `src-tauri/src/graph_build.rs` | Persists effective local/cloud provider, model, endpoint, and runtime provenance; cloud metadata is sanitized before Genesis persistence. |
| `src-tauri/src/fungwire_server.rs` | Reserves the STT cap before dispatch and propagates reservation failures without provider contact. |
| `src-tauri/src/job_engine.rs` | Prevents cloud-admission errors from being reclassified as retryable local transport failures. |
| `src-tauri/src/policy.rs` | Uses the atomic task-kind reservation boundary with independent STT/LLM counters and fail-closed persistence handling. |

Worker/Luna reported local evidence is:

- focused tests: `cloud_config` 8, `cloud_executor` 21, `graph_build` 17,
  `fungwire_server` 18, and `policy` 19;
- full `cargo test`: 490 passed, 0 failed, 1 ignored;
- `rustfmt`, `cargo check`, and `git diff --check`: passed.

The corrected tests use loopback fakes and closed ports only. An earlier
invalid diagnostic contacted local Ollama at `127.0.0.1:11434` because of a bad
fixture; it used no cloud credentials and is excluded from acceptance evidence.
The existing real-runtime test remains ignored. This is local fixture/source
evidence, not real-provider, keyring, CI, device, production, or Controller
Gate evidence.

Terra's final gate is **FINAL PASS; no HIGH blockers**. Three medium,
non-blocking follow-ups remain recorded for later work:

1. Add a GraphBuild persistence-failure integration seam.
2. Replace substring-based error classification with structured error types or
   equivalent structured classification.
3. Add explicit token/secret/password redaction fixtures.

No real provider, secret/keyring, CI, physical device, production,
Controller Gate, commit, push, PR, merge, or deployment proof has occurred.
The live Controller/real-provider/device verdict remains **NOT_RUN**.

## 3. Chosen minimal remediation design

### 3.1 Design invariants

- Cloud keys remain only in the existing desktop OS keyring. No key-bearing
  type crosses into Genesis persistence, model-run provenance, errors,
  diagnostics, tests, or the acceptance artifacts.
- No provider priority, endpoint, timeout, UI, FUNGWIRE wire format, or local
  fallback trigger is changed.
- Existing local Ollama graph behavior remains local-first: only the existing
  connection-failure condition can reach cloud; timeout, bad status, malformed
  response, and other local errors remain local errors.
- P3-AC-04 uses the existing model_runs schema. No Genesis migration,
  Supabase migration, manifest change, or lockfile change is required.
- P3-AC-06 uses the existing tier_policy and cloud_call_counter tables. No
  new table is required.
- A failed admission is not retried against another provider. A provider
  response error is not automatically retried.
- The implementation is tested only with in-memory/file-backed local SQLite
  and loopback fake HTTP. No real key, provider, recording, or cloud endpoint
  is used.

### 3.2 Approved implementation scope and delivered source boundary

This is the complete approved source/test scope. The remediation is present in
these six dirty working-tree files; the table records the delivered boundary
and the explicit non-change contract. It is not a write authorization for this
documentation update or for live acceptance.

| Path | Delivered change | Explicit non-change |
|---|---|---|
| src-tauri/src/cloud_executor.rs | Returns secret-free effective LLM provenance, maps effective provider metadata, reserves the LLM cap before dispatch, propagates admission errors, and covers fallback/provenance behavior with local fixtures. | Does not expose or serialize CloudProviderConfig from the graph writer; provider order, HTTP paths, headers, and timeouts are unchanged. |
| src-tauri/src/graph_build.rs | Consumes the secret-free execution result, persists effective local/cloud provider metadata and model-run provenance, preserves local values, and covers persisted-row/error behavior. | Does not import or serialize CloudProviderConfig; graph extraction, prompt, cleanup ordering, and local Ollama selection are unchanged. |
| src-tauri/src/policy.rs | Provides the authoritative atomic reservation function and typed fail-closed errors; task-kind rows remain independent under one daily_cap and are covered by SQLite/concurrency tests. | Keyring storage, TTS/media-fetch policy, and the approved daily-cap meaning are unchanged. |
| src-tauri/src/fungwire_server.rs | Replaces the STT read/check/late-increment path with reservation-before-dispatch and explicit error propagation. | FUNGWIRE frames, pairing/revocation checks, local transcription, resume, keepalive, and cleanup semantics are unchanged. |
| src-tauri/src/job_engine.rs | Keeps cloud-admission failures non-retryable instead of reclassifying their local transport wording as a retryable failure. | General job retry/backoff behavior is unchanged. |
| src-tauri/src/cloud_config.rs | Preserves redacted configuration handling and treats blank keys as unconfigured for the fail-closed boundary. | No keyring format or provider configuration change. |

No other source path was in scope. In particular, no lib.rs, fungwire.rs,
Genesis schema definition, package manifest, Cargo.lock, frontend/mobile UI,
or unrelated dirty/untracked path was edited for this remediation. The current
documentation update is limited to the two requested plan files.

## 4. AC-04 remediation contract and delivered behavior

### 4.1 Secret-free effective execution contract

cloud_executor.rs returns a value conceptually equivalent to:

~~~rust
struct EffectiveLlmExecution {
    text: String,
    provider_id: String,
    provider_label: String,
    model_name: String,
    endpoint: String,
    runtime_location: &'static str,
}
~~~

The current value does not contain api_key, prompt, response body, account ID,
request headers, or any CloudProviderConfig field that carries a key. The cloud
module constructs this value internally, while graph_build.rs receives only
this sanitized value.

The deterministic mapping is:

| Execution | provider_id | model_name | endpoint | runtime_location |
|---|---|---|---|---|
| Existing local Ollama success | ollama-summary-intent | The model already resolved by llm_provider_config | The local Ollama endpoint already resolved by llm_provider_config | local |
| Anthropic cloud success | cloud-anthropic-summary-intent | The configured override or the existing Anthropic default actually sent | https://api.anthropic.com/v1/messages | cloud |
| OpenAI LLM cloud success | cloud-openai-summary-intent | The configured override or the existing OpenAI default actually sent | https://api.openai.com/v1/chat/completions | cloud |
| Custom LLM cloud success | cloud-custom-summary-intent | The explicit sentinel custom-unspecified because the current Custom config has no model field | The configured HTTPS endpoint, normalized without userinfo/query/fragment | cloud |

The custom sentinel is an explicit statement that the current custom
contract does not identify a model; it must not be presented as a verified
provider model. AC-04 live acceptance is the Anthropic path and must not use
the custom path to claim effective-model proof.

### 4.2 Persistence behavior

1. Local success uses the same model_runs values as today:
   provider_id=ollama-summary-intent, the resolved local model, the resolved
   local endpoint in parameters_json, and runtime_location=local.
2. Cloud success first upserts a deterministic, sanitized model_providers row
   for the effective cloud provider. Its label, runtime_location=cloud, kind,
   model, and endpoint are non-secret metadata only; config_json must never
   contain a key.
3. The model_providers upsert must precede the model_runs upsert in the same
   Genesis commit batch so the existing provider_id foreign key is satisfied.
4. The model_runs row then uses the effective provider_id, model_name,
   runtime_location=cloud, input/output references, and a parameters_json
   endpoint that matches the actual cloud transport. The local row shape and
   local endpoint behavior are unchanged.
5. The graph writer persists only the sanitized execution value. The static
   leak test continues to reject any source file that combines a
   key-bearing CloudProviderConfig with Genesis persistence calls.
6. If parsing fails, the local/cloud call fails, or the model-provider/model-run
   commit fails, no automatic second provider call is made. The error is
   returned with the existing redacted/truncated policy, and no false
   successful model_runs row is created. A provider may already have received
   the request in the last case; the later live acceptance must classify that
   as a failed evidence row rather than retrying it.

### 4.3 Local and cloud success/error paths

| Path | Required result |
|---|---|
| Local Ollama success | Return text plus local provenance; persist the same local row as before. No cloud counter reservation. |
| Local connection failure, cloud disabled/no key/cap reached | Return the existing blocked reason; no cloud request, no model_runs row, no cloud counter reservation. |
| Local timeout, bad status, malformed response, or other non-connection error | Return the original local error unchanged; do not cloud-fallback, reserve, or persist a new model run. |
| Cloud success | Return text plus cloud provenance; persist effective cloud provider/model/endpoint and runtime_location=cloud. |
| Cloud HTTP/error/timeout | Return the existing redacted cloud error; do not switch provider, do not create a successful model_runs row, and retain the AC-06 slot because a request may have been accepted. |
| Cloud success followed by provenance persistence failure | Do not retry the provider. Surface a distinct persistence failure and keep the later Controller Gate blocked; no false provenance PASS. |

## 5. AC-06 remediation contract and delivered behavior

### 5.1 Authoritative reservation semantics

The pure decide_cloud_tier matrix remains useful for unit reasoning, but it is
no longer the final admission boundary when followed by a separate calls_today
read and increment. Both STT and LLM dispatch paths call one authoritative
policy reservation function after the key-configured boolean is known and
before their cloud executor can send a request.

Chosen semantics: the cap slot is committed before the provider response, not
after it. The existing count becomes the number of admitted outbound cloud
dispatch slots for that task kind and local calendar day. This is deliberate:
once a request is admitted, a later HTTP error, timeout, process crash, or lost
response cannot prove that the provider did not receive or bill the request.
Keeping the slot is conservative and fail-closed.

There is no success-finalization write and no best-effort post-response
increment. Consequently, there is no counter write that can be ignored after a
provider request. A known policy block or reservation failure consumes no slot;
an admitted dispatch consumes one slot even if the provider response is an
error. The acceptance runbook must be updated to state this count meaning
before live execution.

The implementation deliberately does not add an ambiguous release path. All
local setup that can be validated cheaply should occur before the final
reservation boundary. If a future path can prove that no provider request was
started after reservation, a separate guarded release may be considered in a
new approved design; this remediation does not decrement a slot merely
because a provider returned an error. A process crash leaves the slot charged
until the local date changes, which may under-utilize the cap but cannot
overrun it.

### 5.2 Transaction and SQL contract

The implemented policy::reserve_cloud_call uses the same paired_devices.db
connection configuration already established by paired_devices_connection_at:
SQLite WAL, foreign keys on, and the existing bounded busy timeout.

The implementation:

1. Ensures the existing policy/counter tables exist. If table setup fails, it
   returns a policy error and does not call a provider.
2. Start BEGIN IMMEDIATE. This serializes all competing reservations,
   including an STT reservation racing an LLM reservation, on the same
   database.
3. Read the current tier_policy row inside the transaction. An absent row
   means the existing default: both cloud toggles off and daily_cap=20.
   Read the task-specific toggle and the current daily_cap, not a stale
   caller snapshot. The key-configured input is a boolean; the key value
   never enters this function.
4. Returns cloud_disabled or no_key_configured without changing the counter
   when the relevant toggle/key condition blocks the task.
5. Returns cap_reached without changing the counter when daily_cap is zero or
   the task-kind count is already at/above the cap.
6. For an allowed task, atomically execute the equivalent of:

~~~sql
INSERT INTO cloud_call_counter (task_kind, call_date, count)
VALUES (?task_kind, ?local_date, 1)
ON CONFLICT (task_kind, call_date)
DO UPDATE SET count = cloud_call_counter.count + 1
WHERE cloud_call_counter.count < ?daily_cap;
~~~

The implementation checks the affected-row count. Exactly one changed row
means the reservation succeeded. Zero means cap_reached and the transaction
rolls back. Any SQLite error, including a busy/locked timeout, is a policy_error
and rolls back or leaves no committed increment.
7. It commits before returning Allow. A commit error is not ignored and the
   caller does not dispatch; an ambiguous commit is treated conservatively as
   admission failure and does not trigger a retry.

The SQL is per task kind and per local YYYY-MM-DD row. The single
tier_policy.daily_cap value is shared as a setting, but STT and LLM counts
remain independent. Filling the STT row does not consume the LLM row, and
vice versa; each row is individually bounded by the same cap.

calls_today remains a read-only status query for the UI and evidence. It is not
used as the authorization decision immediately before a cloud call.
increment_calls_today is not used by the two dispatch paths, and no caller
discards a counter Result in this flow.

### 5.3 STT and LLM call-site behavior

- STT: fungwire_server.rs now replaces the read/check/late-increment flow with
  reserve_cloud_call before cloud_executor::dispatch_stt. A reservation
  failure maps to a redacted policy error and no provider socket is opened. The
  existing local executor path is untouched.
- LLM: cloud_executor.rs reserves immediately before dispatch_llm after
  the local Ollama connection failure has been classified and the effective
  cloud config is selected. A successful local call never reserves. A blocked
  or failed reservation never calls dispatch_llm.
- Neither path may silently retry after policy, reservation, provider, or
  provenance-persistence failure.
- Because reservation is committed before the request, a provider error or
  timeout retains the slot. Tests and the later runbook must show this
  explicitly rather than treating the counter as success-only.

### 5.4 Failure behavior

| Failure point | Provider contact | Counter behavior | Job/result behavior |
|---|---|---|---|
| Toggle off, no key, cap reached | None | Unchanged | Existing blocked reason; no cloud result. |
| Table setup, transaction, SQL, busy, or commit failure | None | No admitted slot, or conservatively unknown after an ambiguous commit; never retry provider | Redacted policy/usage error; fail closed. |
| Reservation committed, provider returns success | One possible request | Slot retained | Return result and continue existing success path. |
| Reservation committed, provider returns HTTP error/timeout | One possible request | Slot retained | Return redacted error; no automatic retry. |
| Reservation committed, process/transport fails after dispatch begins | One possible request | Slot retained | Existing retryability rules may classify the job, but no silent provider repeat. |

The former test that accepted a result after a failed late counter increment
was replaced. A counter write failure now happens before provider contact, so
the assertion is no provider contact and a fail-closed error.

## 6. Local verification evidence and test traceability

The red/green test plan below is now covered by the reported local evidence.
All corrected provider behavior is represented by loopback fake HTTP or closed
ports only. No API key or real provider endpoint was used. The focused counts,
full-suite result, formatting, and cargo-check result are recorded in §2.3.

| ID | Target and current anchor | Red/green acceptance assertion |
|---|---|---|
| T04-1 | cloud_executor.rs fallback/provenance implementation around 734-789 and focused module suite | A cloud success returns text plus effective provider ID, model, endpoint, and cloud location; the value contains no key, prompt, or headers. Local evidence reports 21 focused tests. |
| T04-2 | graph_build.rs persistence implementation around 646-700 and focused tests around 1050-1260 | A fake cloud response writes sanitized cloud provider metadata and effective model-run provenance; a local build writes the unchanged Ollama tuple. Local evidence reports 17 focused tests. |
| T04-3 | graph_build.rs failure/error tests | Local failure preserves prior extraction; cloud error/timeout creates no successful provenance row, does not switch provider, and does not retry. |
| T04-4 | graph_build.rs local error classification tests | Local timeout and bad response remain non-fallback errors; a refused Ollama connection remains the fallback trigger. |
| T06-1 | policy.rs reservation implementation at 258-306 and concurrency tests at 572-628 | File-backed SQLite reservations at the cap boundary yield exactly daily_cap admissions per task kind and never exceed the cap. Local evidence reports 19 focused policy tests. |
| T06-2 | fungwire_server.rs reservation path around 1134 and counter-failure tests | Counter-reservation failure produces policy_error/fail-closed behavior, zero fake-provider connections, and no count increment. Local evidence reports 18 focused tests. |
| T06-3 | cloud_executor.rs and fungwire_server.rs dispatch tests | A successful local LLM/STT path never reserves; an admitted cloud error retains its reserved slot and is not automatically retried. |
| T06-4 | policy.rs independent-counter tests | Filling STT to its cap still permits an LLM reservation under the same daily_cap, and vice versa; no cross-kind counter is used. |
| T06-5 | cloud_config.rs redaction/configuration tests and graph provenance tests | Blank keys fail closed and sentinel key material is absent from serialized provenance, provider config, model-run parameters, and redacted errors. Local evidence reports 8 focused cloud_config tests. |
| T06-6 | policy.rs transaction-error tests | Malformed counter schema, rejected write, lock/busy failure, or commit failure returns an error and never reaches the fake provider. |
| T06-7 | job_engine.rs admission classification at 229 and regression test at 1130 | Cloud-admission errors remain non-retryable even when their text contains a local transport marker. |

The worker/Luna report records focused tests for `cloud_config` 8,
`cloud_executor` 21, `graph_build` 17, `fungwire_server` 18, and `policy` 19;
full `cargo test` records 490 passed, 0 failed, and 1 ignored. `rustfmt`,
`cargo check`, and `git diff --check` also passed. These are local supporting
results only; the earlier invalid `127.0.0.1:11434` diagnostic is excluded and
the existing real-runtime test remains ignored.

## 7. Current gates, rollback, schema impact, and spend safety

### 7.1 Current gates and remaining live gate

1. The exact source/test scope was approved and the six-file remediation is
   present in the shared working tree.
2. Local focused/full verification and formatting/check evidence are recorded
   in §2.3 and §6.
3. Terra's final gate is **FINAL PASS; no HIGH blockers**. The three medium
   follow-ups in §2.3 are non-blocking and do not authorize scope expansion.
4. This two-document update records the final counter semantics, persisted
   provenance, concurrent-boundary evidence, counter-write stop condition, and
   the remaining live boundary.
5. Only after separate Boss/user approval, provider-specific consent, all
   prerequisites, and a clean acceptance decision may the later runbook request
   one bounded OpenAI STT call and one bounded Anthropic LLM call.

### 7.2 Rollback

- Before any real provider execution, rollback is a branch/code rollback only;
  no user data, keyring state, provider state, or counters are touched.
- If the remediation tests fail, do not merge and keep P3-AC-04/P3-AC-06
  blocked. Do not weaken assertions or increase timeouts.
- If the new code has run locally, disable cloud admission and retain the
  counter rows. Do not manually decrement or reset them.
- The new count meaning is intentionally conservative. If code is reverted
  after a reservation was committed, the old code may interpret a retained
  slot as a successful call; this can reduce availability but cannot create
  extra allowance. Re-enable the old cloud path only after a separate review;
  the preferred recovery is the forward fix.
- A failed live acceptance row stops the run. Do not repeat the provider call
  to make the result deterministic; follow the existing runbook cleanup and
  credential-revocation rules.

### 7.3 Schema and migration impact

- No GenesisBlockDB schema version or migration changes.
- No Supabase migration, dashboard change, mobile schema change, manifest, or
  lockfile change.
- Existing paired_devices.db tables are reused. BEGIN IMMEDIATE changes the
  write protocol, not the table shape.
- Cloud provider metadata rows created for truthful model_runs foreign keys are
  ordinary sanitized application rows; no keyring material is stored there.
- Existing counter rows are retained. Their documented meaning changes from
  successful-only increments to committed outbound admission slots. This is
  why the acceptance runbook must be updated before live execution.

### 7.4 Provider-spend safety

- No provider request is allowed without a committed cap reservation.
- Reservation transaction errors fail closed before egress.
- A reserved slot is retained after any request may have started, including
  provider error, timeout, lost response, or process crash.
- No automatic retry, provider switch, or counter reset is permitted.
- Local verification uses fake HTTP only; the first real provider execution is
  a later, separately authorized Controller Gate action.

## 8. Explicit out of scope

- UI redesign, new settings surfaces, mobile UX changes, or cloud-badge
  redesign.
- Any new cloud provider, provider-priority change, endpoint/timeout change,
  or cloud relay/NAT traversal.
- TTS policy or executor changes.
- Google Meet, Live Meeting, meeting-agent, or real-room acceptance.
- Release, signing, packaging, production deployment, clean-install,
  physical-device, Android, or general FUNGWIRE acceptance.
- Real provider execution, key collection/inspection, secret use, billing
  validation, provider-quality certification, or real recording transfer.
- Genesis/Supabase migrations, schema redesign, counter cleanup, or dollar
  cost estimation.
- Source, test, manifest, lockfile, UI, provider, keyring, device, Git, CI,
  deployment, and production changes beyond the two requested documentation
  files.
- Cleanup, formatting, refactoring, or changes to unrelated dirty/untracked
  paths.

## 9. Implementation DAG and Terra review gates

The DAG below is retained for traceability. Its implementation and local
verification path through C1/T2 is complete in the six approved Rust files;
R0 is the current separately scoped documentation update. The Boss/user gate
remains open for live acceptance.

~~~text
G0  Approve this remediation plan and exact write scope
 |
 +--> A0  Write AC-04 red tests
 |     files: cloud_executor.rs, graph_build.rs, cloud_config.rs tests
 |       |
 |       +--> A1  Add secret-free effective execution contract
 |             |
 |             +--> A2  Persist cloud provider/model/endpoint and preserve local row
 |
 +--> B0  Write AC-06 red tests
       files: policy.rs, fungwire_server.rs tests
         |
         +--> B1  Add BEGIN IMMEDIATE atomic reservation SQL
               |
               +--> B2  Wire STT admission and explicit persistence errors

A2 + B2 --> C0  Serial LLM reservation wiring in cloud_executor.rs
              |
              +--> C1  Focused local/fake-HTTP/concurrency verification
                    |
                    +--> T0  Terra AC-04 provenance/security review
                    |         |
                    |         +--> T1  Terra AC-06 transaction/concurrency review
                    |                   |
                    |                   +--> T2  Terra integrated regression and
                    |                             secret-boundary review
                    |                              |
                    |                              +--> R0  Separate approved
                    |                                        acceptance-runbook update
                    |                                        |
                    |                                        +--> Boss/user gate:
                    |                                             no real provider
                    |                                             execution before
                    |                                             acceptance starts
~~~

### Terra gate criteria

- Terra final gate: **FINAL PASS; no HIGH blockers**.
- Terra-AC04: final review confirms that local values are unchanged, cloud
  values come from the effective transport, provider_id foreign keys remain
  valid, and no key-bearing value crosses into Genesis or errors.
- Terra-AC06: final review confirms that the transaction is the admission
  boundary, affected-row handling is exact, task-kind counters are independent,
  all persistence failures fail closed, and concurrent tests exercise one
  shared WAL file.
- Terra-integrated: final review confirms no ignored counter Result remains on
  either cloud path, no hidden retry/provider switch was added, and the focused
  tests use only local fixtures/closed ports.
- Terra-documentation: the runbook update retains NOT_RUN until live
  prerequisites exist and does not promote local test evidence to
  provider/device/production acceptance.

## 10. Acceptance-runbook update and remaining live boundary

This approved two-document update records the required post-code evidence
without authorizing live execution:

1. Replace the former P3-AC-04 blocker note with the exact persisted provenance
   observation: effective provider ID/label, model, endpoint, and
   runtime_location=cloud, with no key-bearing fields.
2. Update P3-AC-06 from the former sequential rate-guard observation to the atomic
   reservation contract and attach the concurrent-boundary test artifact.
3. State that daily_cap is one shared setting applied independently to STT and
   LLM counters.
4. State that a committed reservation is counted before provider response and
   retained after provider error/timeout because provider contact is
   unknowable; blocked/reservation-failed attempts do not contact a provider.
5. Replace the former successful-result-after-counter-write-failure
   expectation with fail-closed no-egress evidence.
6. Add the reservation/counter-write failure and no-key-leakage observations
   to the mandatory stop/evidence checklist.
7. Keep P3-AC-04 and P3-AC-06 Controller verdicts **NOT_RUN** until the
   required real OpenAI/Anthropic rows, cleanup, and Boss/user acceptance are
   complete. The local remediation and Terra review do not change this.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b → 0.2.0b | Added dated post-remediation evidence for the six approved Rust files, reported local focused/full verification, Terra FINAL PASS with no HIGH blockers, and three non-blocking follow-ups; preserved the historical RCA and kept live acceptance NOT_RUN. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-22 | beta | Recorded the delivered AC-04/AC-06 remediation boundary, worker/Luna local evidence (focused suites and 490/0/1 full cargo result), loopback/closed-port test limits, Terra FINAL PASS with no HIGH blockers, and medium follow-ups; live Controller/provider/device evidence remains NOT_RUN. | working-tree; no new commit; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b | RWANG |
| 0.1.0b | 2026-09-22 | candidate | Documented source-grounded RCA, minimal AC-04 provenance remediation, atomic AC-06 cap reservation design, test-first gates, rollback, scope boundaries, and Terra review DAG. No source, test, runbook, provider, secret, commit, or deployment action performed. | working-tree; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b | RWANG |
