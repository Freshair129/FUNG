---
version: "0.3.0b"
created_at: "2026-09-22T09:49:01+07:00,RWANG,base-538213ffb0d69d416fe0e3e2d9d5f4beed24305b"
last_update: "2026-09-22T14:58:44+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "acceptance-runbook"
  scope: "FUNG Phase 3 BYOM Controller Gate REQ-F-01..04"
  language: "Thai-English"
  execution_status: "NOT_RUN"
  implementation_status: "REMEDIATION_COMPLETE; TERRA_FINAL_PASS"
  controller_verdict: "NOT_RUN"
  live_provider_device_verdict: "NOT_RUN"
  complexity: "C-3"
  risk: "HIGH"
  commit_provenance: "working-tree; no new commit; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b"
  authority: "Boss/user approval required before execution"
---

# FUNG Phase 3 BYOM Acceptance Runbook / คู่มือรับรอง BYOM Phase 3

> **สถานะ:** candidate. เอกสารนี้เป็นแผนการ execute ที่พร้อมให้พิจารณาอนุมัติ
> แบบมีขอบเขต ไม่ใช่ผลการ live execute. Local AC-04/AC-06 remediation มี
> evidence และ Terra final gate เป็น FINAL PASS แต่ Controller Gate ของ Phase 3
> ยังเป็น **NOT_RUN** ในรอบนี้.

## 1. Objective and scope / วัตถุประสงค์และขอบเขต

### 1.1 Objective

ปิดเฉพาะ Phase 3 Controller Gate ของ FUNG และรับรองข้อกำหนด
`REQ-F-01..04` ด้วยหลักฐานที่สังเกตได้จาก desktop จริง, provider จริง และ
paired mobile ตามที่ระบุใน matrix ด้านล่าง โดยรักษา local-first และ fail-closed
boundary ของระบบไว้ตลอดการทดสอบ.

The target is a bounded acceptance decision for:

- `REQ-F-01`: cloud credentials are stored only in the Windows desktop OS
  keyring, are redacted in diagnostics, and never enter application or remote
  serialization paths.
- `REQ-F-02`: the fixed task path honors local → paired desktop → cloud for STT
  and desktop-local Ollama → cloud for LLM, with an explicit per-task cloud
  toggle.
- `REQ-F-03`: exercise the OpenAI STT and Anthropic LLM cloud executors with
  policy/key checks and the daily request-count cap. The AC-04 provenance and
  AC-06 atomic-reservation remediation is locally evidenced and Terra-reviewed;
  live hard-cap Controller acceptance remains **NOT_RUN** until the real rows
  and separate Boss/user approval are complete.
- `REQ-F-04`: the mobile surface reflects the paired desktop cloud policy and
  the privacy-first default remains cloud-off.

### 1.2 Explicit non-goals / สิ่งที่ไม่ใช่ผลลัพธ์ของ run นี้

This runbook does **not** authorize or claim any of the following:

- release readiness, production readiness, deployment, signing, packaging, or
  store acceptance;
- general device, Android, iOS, FUNGWIRE, clean-install, or restart readiness;
  the paired mobile is only a test fixture for the STT relay path;
- Google Meet or Live Meeting readiness, real-room behavior, or meeting-agent
  qualification;
- Supabase migration, dashboard change, Genesis schema change, or cloud storage
  activation;
- TTS acceptance. The approved Phase 3 spec limits this tier-3 executor gate to
  STT and LLM; the master-plan mention of TTS is not expanded here;
- provider price certification, dollar-budget certification, model quality
  certification, or a claim that a provider response is production-suitable;
- code changes, test changes, secret collection, secret printing, commits,
  pushes, pull requests, merges, or deployment.

Passing this run means only that the bounded Phase 3 controller evidence is
acceptable. It does not promote any other FUNG gate.

### 1.3 Complexity and risk / ระดับความซับซ้อนและความเสี่ยง

- **Complexity: C-3 — Architecture-Driven Acceptance.** The gate crosses the
  Windows OS keyring, desktop policy/storage, provider egress, FUNGWIRE/mobile
  delegation, graph provenance, concurrency, and reversible cleanup
  boundaries.
- **Risk: HIGH.** A false PASS could disclose a credential, transmit an
  unapproved recording or graph/summary payload, charge a provider, misstate
  model provenance, overstate local evidence as live acceptance, or misstate the
  atomic cap semantics.

These ratings require source-grounded evidence and explicit user approval;
they do not authorize implementation or live execution.

## 2. Current baseline and evidence boundaries / baseline ปัจจุบัน

The source documents describe implementation as complete while keeping the
Controller Gate open. This runbook preserves that distinction. The historical
defect baseline is retained below, followed by the dated current post-evidence:

| Evidence class | Source-recorded baseline | What it proves | What it does not prove |
|---|---|---|---|
| Implementation | The six approved Rust files contain the AC-04/AC-06 remediation for effective provenance, atomic cap reservation, fail-closed admission, and admission-error classification. | The reviewed local implementation surface exists. | It does not prove a real provider accepted a request or that credentials were safe in a live desktop session. |
| Automated/local/fixture | Worker/Luna reported focused tests: `cloud_config` 8, `cloud_executor` 21, `graph_build` 17, `fungwire_server` 18, `policy` 19; full `cargo test` 490 passed, 0 failed, 1 ignored; `rustfmt`, `cargo check`, and `git diff --check` passed. | Local provenance, policy, redaction, error, concurrency, and fixture paths have supporting evidence. | Loopback fakes/closed ports, local source checks, and any CI evidence are not real provider/device acceptance. |
| Real provider | **NOT_RUN** in this task. No OpenAI or Anthropic credential is requested, read, or used here. | Nothing yet. | No claim of OpenAI STT, Anthropic fallback, provider billing, or external response quality. |
| Real recording/device | **NOT_RUN** in this task. The required recording and paired mobile are future operator prerequisites. | Nothing yet. | No claim of physical-device, pairing, mobile lifecycle, or recording readiness. |
| Release/production/Meet | Outside this runbook and still separately gated. | Nothing. | No release, production, or Meet conclusion. |

The master tracker remains **IMPLEMENTATION COMPLETE / ACCEPTANCE PENDING**.
The Phase 3 plan's Controller Gate still requires a real desktop OpenAI STT path
and an Anthropic fallback path with Ollama stopped. The mobile and desktop
status documents likewise keep real provider/device evidence separate from
automated evidence. Any later operator must re-capture the live baseline;
historical CI IDs or local test counts must not be reused as live proof without
attaching the source artifact and its timestamp.

### 2.1 Historical blocker baseline / pre-remediation

Before the remediation, cloud fallback returned `runtime_location=cloud` while
the graph writer persisted Ollama provider/model/endpoint values for P3-AC-04.
The pre-remediation P3-AC-06 path used a non-transactional read/check/increment
sequence and ignored dispatch counter-write errors. Those statements are
preserved as RCA/history, not as the current implementation status.

### 2.2 Current post-evidence — 2026-09-22

The approved remediation is complete in the six Rust files named in the
implementation record. Corrected tests used loopback fakes and closed ports
only. An earlier invalid diagnostic contacted local Ollama at
`127.0.0.1:11434` because of a bad fixture; it used no cloud credentials and is
excluded from acceptance evidence. The existing real-runtime test remains
ignored.

Terra's final gate is **FINAL PASS; no HIGH blockers**. Three medium,
non-blocking follow-ups remain: a GraphBuild persistence-failure integration
seam, structured error classification instead of substring matching, and
explicit token/secret/password redaction fixtures.

No real provider, secret/keyring, CI, physical device, production, Controller
Gate, commit, push, PR, merge, or deployment proof has occurred. Local/fixture
evidence does not become live PASS. The live Controller/real-provider/device
verdict remains **NOT_RUN**.

The architecture boundary is also normative for this run: GenesisBlockDB is
the application persistence authority, cloud keys are desktop-keyring-only,
cloud is opt-in, and model runs must preserve provider/model/parameter/output
provenance. A provider status such as `configured: true` is not key material
and is the only credential-state value this runbook permits in evidence.

The current implementation evidence closes the two local remediation blockers:
`src-tauri/src/graph_build.rs:646-700` consumes secret-free effective
provenance, and `src-tauri/src/policy.rs:258-306` provides the atomic reservation
boundary used by the LLM and STT paths (`cloud_executor.rs:753-789`,
`fungwire_server.rs:1134`). This changes the implementation status of P3-AC-04
and P3-AC-06 to **remediation complete and Terra-reviewed**. It does not change
their live Controller verdict: both remain **NOT_RUN** until the runbook's real
provider rows, cleanup, and separate Boss/user approval are complete.

## 3. Authority and prerequisites / ผู้มีอำนาจและสิ่งที่ต้องเตรียม

### 3.1 Authority

Execution requires an explicit Boss/user decision recorded outside the
credential value itself. The user/Boss controls:

1. whether the live run is allowed at all;
2. entry of OpenAI and Anthropic credentials through the approved
   keyring-backed UI, stored by the Windows OS keyring;
3. cloud opt-in toggles and the daily request-count cap;
4. provider billing/spend risk and the decision to use a real recording;
5. stopping and restarting local Ollama;
6. final PASS/FAIL acceptance and any later tracker update.

The parent in this task is orchestrator/reviewer only. This document does not
grant the parent authority to request/use secrets, call a provider, alter the
keyring, alter user data, commit, push, merge, or deploy.

Provider-specific data-transfer consent is separate and mandatory. Before the
relevant row, the user/Boss must explicitly approve (a) sending the approved
recording audio to OpenAI for STT and (b) sending the approved graph/summary
input content to Anthropic for LLM fallback. Record provider, task kind,
recording/project scope, one-call limit, and consent timestamp only; do not
record the payload, key, account, or an inferred retention/region promise.

### 3.2 Required prerequisites

Do not start the live matrix until every required item is confirmed:

- Windows desktop with the approved FUNG build identified by version/source
  identity and a user-approved, reversible test workspace;
- a user-approved evidence root **outside the repository**;
- an approved short real recording fixture, or an approved live recording with
  the required consent. Record its project ID, recording ID, duration, and
  SHA-256 without copying its content into chat or Git;
- a paired, reachable mobile device for the STT delegation path. The pairing
  must be an existing approved test pair or a newly created test-only pair;
- the user has configured an OpenAI STT key and an Anthropic LLM key through
  `CloudProvidersPanel`/the approved Windows OS keyring-backed UI. The
  operator must never receive the key value in chat, a command argument, a
  screenshot, a report, or a log;
- explicit provider-specific data-transfer consent: OpenAI may receive the
  approved recording audio for the STT row, and Anthropic may receive the
  approved graph/summary content for the LLM row;
- explicit user opt-in for the STT and LLM cloud toggles, plus a daily cap that
  the user accepts as a **request count**, not a dollar amount;
- a reversible, session-scoped way to observe only the provider API destination
  under test without capturing `Authorization`, `x-api-key`, request bodies,
  audio, prompts, or key-bearing headers;
- a reversible way for the user to record whether local Ollama is running and,
  for the LLM fallback case, stop it before the one fallback attempt;
- no unrelated recording, graph, provider, pairing, or cleanup job is active;
- the user understands that a successful call may incur provider cost and that
  the run stops on ambiguity rather than retrying.

### 3.3 Credential handling rule / กฎเกี่ยวกับ secret

The user enters a credential only in the approved keyring-backed UI. The
operator must never ask the user to paste it into chat or terminal output.
Record only status such as:

```json
{"provider":"openai","task_kind":"stt","configured":true}
```

Never record, print, hash for the report, screenshot, echo, serialize, or
compare a raw key. Do not place a key in an environment variable, test fixture,
URL, custom header dump, crash report, or evidence filename. If a key appears
anywhere outside the OS keyring, stop immediately under §9.

## 4. Evidence and artifact rules / หลักฐานและ artifacts

### 4.1 Evidence root

Use an exact, user-approved evidence root such as:

```text
<approved-evidence-root>\fung-phase3-byom\<run-id>\
```

Do not use the repository as the evidence root. Do not create screenshots of a
password field or export raw keyring entries. Raw audio and sensitive
transcript/summary text stay on the user's approved local storage; use a
redacted excerpt plus a checksum where a full payload is not approved.

Recommended artifact names are stable and secret-free:

```text
P3-AC-01-status.json
P3-AC-01-redacted-ui.png
P3-AC-01-static-boundary.txt
P3-AC-03-run.json
P3-AC-03-segments-redacted.json
P3-AC-03-provenance.json
P3-AC-04-summary-redacted.json
P3-AC-07-local-fault.json
P3-AC-07-no-egress.txt
P3-AC-08-post-state.json
```

Every artifact must include `run_id`, ICT timestamp, operator, app/build
identity, acceptance ID, and evidence class. It must not include a key,
credential-bearing header, request body, raw prompt, raw audio, or an unrelated
user record.

### 4.2 Allowed versus forbidden observations

Allowed: provider/task-kind configured booleans, effective provider/model name,
task/job/recording/project IDs, policy toggles, cap and call counts, destination
host/path, HTTP status and elapsed time, redacted errors, output checksums,
provenance fields, cloud badge state, and before/after state.

Forbidden: key values or prefixes, `Authorization`/`x-api-key` values, keyring
password reads, raw request/response bodies, unredacted logs, screenshots of
password inputs, provider account identifiers, or broad filesystem/database
exports that could contain secrets.

## 5. Preflight checks / ตรวจสอบก่อนเริ่ม

The future operator records a preflight sheet before changing any setting. A
missing observation is **NOT_RUN**, not an inferred PASS.

| Check | Required observation | Record without secrets |
|---|---|---|
| Run identity | Unique `run_id`, operator, ICT start time, approved evidence root | IDs and timestamps only |
| Build | FUNG version/source identity and whether the app is the approved test build | Version/hash; no credentials |
| Provider status | OpenAI/STT and Anthropic/LLM `configured` status; all other slots if visible | `{provider, task_kind, configured}` only |
| Policy | `stt_cloud_enabled`, `llm_cloud_enabled`, `daily_cap` | Booleans and integer count |
| Counter | `calls_today` for STT and LLM before the run | Counts only; never reset by hand |
| Pairing | Mobile/desktop test pair reachable, not revoked, and test-only or user-approved | Device IDs may be recorded; no tokens or private keys |
| Recording | Project ID, recording ID, duration, source SHA-256, consent/fixture authority | IDs, duration, checksum; no raw audio in report |
| Ollama | Running/stopped state, local endpoint/model label, and who controls the transition | State and non-secret label only |
| Egress observer | Session-scoped observation is active only for the provider API host under test: OpenAI or Anthropic; custom is out of scope unless separately approved | Host/path/method/status only; no headers/bodies |
| Logs | Redaction is enabled and the evidence sink is outside the repo | Redacted excerpts only |
| Clean state | No unrelated job, pending retry, or prior test artifact will be mistaken for this run | Job IDs and exact artifact paths |

Before proceeding, the operator and Boss confirm that the starting policy,
provider statuses, counts, Ollama state, pairing state, recording checksum, and
evidence root are captured. Do not overwrite a pre-existing keyring slot or
delete a pre-existing artifact unless the user explicitly owns and approves
that state transition.

No-egress evidence is bounded to the provider host(s) under test plus local or
static source/test evidence. This runbook does not perform live Supabase,
Genesis, or local-endpoint scans as part of egress observation, and it makes no
whole-machine network claim.

## 6. Reversible setup and teardown sequence / ลำดับที่ย้อนกลับได้

### 6.1 Setup

1. Create the exact evidence root and `run_id`; verify it is outside the repo
   and contains no prior run material.
2. Capture the §5 before-state snapshot. In particular, record provider
   statuses, not key values; record current counters, not a guessed budget.
3. The user enters only the OpenAI STT and Anthropic LLM credentials in the
   approved keyring-backed UI. The operator records only the resulting
   configured statuses. If a target slot was already configured, stop and get
   an explicit user decision before overwriting it.
4. Set the user-approved cloud toggles and daily cap in the Cloud Providers
   panel. Do not edit counters directly. Keep a copy of the before policy so
   the exact toggles and cap can be restored.
5. Confirm the approved recording and paired mobile are still reachable. Do
   not create a second copy of the recording unless the user approved its exact
   path and the copy is checksum-verified.
6. Confirm the egress observer and redacted log sink are active. If the
   observer would capture a secret-bearing header or body, do not start.
7. Execute the matrix in the order in §7: keyring/redaction, cloud-off, policy
   off, one OpenAI STT call, the remediated/retested Anthropic LLM call with
   Ollama stopped, the remediated/retested atomic-cap boundary, a verified local
   no-egress fault, then cleanup. Do not run the Anthropic or cap-blocked rows
   without the separate Boss/user approval and all live prerequisites.

### 6.2 Teardown

1. Stop initiating new test jobs. Allow each current job to reach its
   documented terminal or retryable non-terminal state, and record that state.
   Do not force a retryable cloud failure to become terminal; if a result is
   ambiguous, freeze the evidence and follow §9 before cleanup.
2. Clear only keyring entries created by this run, using the existing UI. A
   pre-existing user entry is retained and its unchanged status is recorded.
3. Restore `stt_cloud_enabled`, `llm_cloud_enabled`, and `daily_cap` to the
   captured before-state. Do not delete or rewrite call-counter history in a
   non-disposable user profile.
4. Restore local Ollama to the exact captured state. If the user stopped it
   before AC-04, the user controls the restart; the operator must not claim it
   was restored until observed.
5. Revoke/remove only a test-only pairing or test-created local artifact that
   this run created. Do not revoke an existing user's device or delete an
   unrelated recording/project.
6. Remove only exact test-created temporary files and logs from the approved
   evidence/test root. Retain the redacted evidence bundle and checksums. Do
   not use broad recursive cleanup, wildcard deletion, or repository cleanup.
   Do not claim that no residual artifacts exist outside the inspected root;
   enumerate only known paths and leave unknown/unapproved paths unasserted.
7. Capture the §8 after-state and compare it with before-state. Any difference
   not authorized by the user is a stop condition and is reported as FAIL or
   NOT_RUN according to §9.

If a disposable FUNG profile was used, its exact policy/counter/artifact root
may be removed only after the path is verified as test-created and the user
approves disposal. A real user profile's durable call counts are not silently
rolled back; report them as post-state.

## 7. Mandatory evidence matrix / ตารางหลักฐานบังคับ

The `Verdict` column below is the current status of this document's execution,
not a prediction. Every row starts as **NOT_RUN** and is promoted only when the
listed artifact exists, is redacted, and passes the expected result. Existing
CI/local evidence may be attached as supporting evidence but cannot replace a
required live boundary.

| ID | Precondition | Exact action | Expected result | Evidence artifact | Verdict | Boundary |
|---|---|---|---|---|---|---|
| **P3-AC-01**<br>Keyring-only / redaction / no serialization | User has entered OpenAI/STT and Anthropic/LLM keys through the approved keyring-backed UI; cloud toggles remain off; before-state captured. | Save each credential once; close/reopen the panel; record only `configured` booleans. Inspect keyring presence through a status-only path. Read redacted diagnostics. Use the existing static/automated leak evidence and only a bounded local inspection of approved application artifacts for Genesis/application state, Supabase-facing state, and browser `localStorage`; do not perform a live Supabase/Genesis/network scan and do not copy any key. | Status shows configured/not-configured only; the raw value is never returned, logged, snapshotted, or placed in Genesis, Supabase, or `localStorage`. Key-bearing serialization is confined to the Windows OS keyring entry. No key material appears in any artifact. | `P3-AC-01-status.json`, redacted UI screenshot, `P3-AC-01-static-boundary.txt`, redacted log excerpt, supporting source/CI test reference. | **NOT_RUN** | Keyring presence is not proof of provider validity, billing, production security, or OS-account compromise resistance. The Genesis/Supabase/`localStorage` statement is local/static boundary evidence, not live egress evidence. |
| **P3-AC-02**<br>Default cloud-off / no egress | Prefer a disposable fresh FUNG profile; otherwise use a user-approved profile with both cloud toggles off. No cloud opt-in. Egress observer active. | Open the policy surface and record the default/starting state. Run one bounded local-only task, or a task that would otherwise fall back while cloud is off. Observe cloud destinations and counters. Do not select a cloud action or enter a new key for this row. | Fresh default is `stt_cloud_enabled=false`, `llm_cloud_enabled=false` (the documented default cap is 20 unless the approved profile says otherwise). No OpenAI, Anthropic, or custom cloud request occurs; cloud counts do not change. Work stays local or fails locally without silent egress. | `P3-AC-02-policy-before.json`, no-egress observation, counter snapshot, redacted job result, profile identity. | **NOT_RUN** | A local observation covers only the bounded task/session. It is not a whole-machine firewall audit or production network guarantee. An existing profile does not prove fresh-install default. |
| **P3-AC-03**<br>OpenAI STT cloud path | OpenAI/STT is configured; STT cloud is explicitly on; the user has given OpenAI recording/STT transfer consent; `calls_today < daily_cap`; approved real recording and paired reachable mobile are ready; local STT is disabled for the test or the explicit cloud delegate action is selected. | Capture counts and recording checksum. From the paired mobile, choose the explicit cloud transcription action for the approved recording. On desktop, observe the delegated job and `executor=cloud`; record only `api.openai.com/v1/audio/transcriptions`, method/status, effective model (`whisper-1` when surfaced), task/job/recording/project IDs, and redacted logs. Wait once for completion and inspect the mobile timeline. | Exactly one successful OpenAI STT call for this test; the STT count increases by one on the committed reservation before provider response and remains charged if the request errors or times out. Transcript segments arrive with timestamps and provenance; the cloud badge is visible where the UI provides it; source and output checksums/provenance are consistent; no Anthropic request or local-STT substitution is observed. | `P3-AC-03-run.json`, redacted segment/provenance artifact, source/output checksums, egress record, mobile cloud-badge evidence, before/after counter. | **NOT_RUN** | This is one approved recording and one bounded call. A pass proves the Phase 3 relay path only; it is not general mobile/device, audio-quality, release, or production readiness. |
| **P3-AC-04**<br>Anthropic LLM fallback with Ollama unavailable | Anthropic/LLM is configured; LLM cloud is explicitly on; user has given the Anthropic graph/summary transfer consent; `calls_today < daily_cap`; an approved graph/summary fixture exists; Ollama is running before-state and can be stopped by the user. The remediation is complete in the approved working tree, locally retested, and Terra-reviewed; live prerequisites are still required. | Record Ollama before-state, then the user stops Ollama. Run one graph/summary job for the approved project/recording. Observe the local connection failure, then the first-configured Anthropic request at `api.anthropic.com/v1/messages`; record provider/model, task/job/project/recording IDs, redacted logs, output checksum, and count. Inspect the persisted `model_runs` row: `runtime_location=cloud` and provider/model/endpoint must describe the effective Anthropic execution, not Ollama. Restore Ollama only after evidence is captured. | Local Ollama unavailability triggers exactly one Anthropic fallback; graph extraction/summary completes; the persisted provenance names Anthropic, the effective model, and the effective cloud endpoint; the LLM count increases by one on the committed reservation before provider response and remains charged if the request errors or times out; no OpenAI fallback occurs while Anthropic is configured first. | `P3-AC-04-run.json`, redacted graph/summary, remediated `model_runs` evidence, Ollama before/after state, bounded egress record, output checksum, counter delta. | **NOT_RUN**<br>implementation: **COMPLETE; TERRA FINAL PASS**<br>controller: **NOT_RUN** | The historical provenance blocker is remediated and Terra-reviewed. Local evidence does not replace the real Anthropic row, cleanup, and separate Boss/user acceptance. |
| **P3-AC-05**<br>Policy-off fail-closed / no provider call | **STT sub-check:** OpenAI/STT is configured, the explicit mobile cloud-transcription action is available, STT cloud is off, and the bounded provider-host observer is active. **LLM sub-check:** Anthropic/LLM is configured, LLM cloud is off, Ollama is stopped only with user approval, and the same observer is active. | **STT action:** From the paired mobile, invoke the explicit cloud transcription action once for the approved recording; do not bypass a hidden/disabled UI path or modify code. **LLM action:** With Ollama stopped, run the normal graph/summary fallback once. For both, record policy reason, destination observation, output state, and task-kind counter before/after. | **STT expected:** the cloud-designated job is blocked with `cloud_disabled` before OpenAI dispatch; no provider request, no STT count increment, and no cloud badge/output claim. **LLM expected:** Ollama connection failure is blocked with `cloud_disabled` before Anthropic dispatch; no provider request, no LLM count increment, and no partial summary/graph commit. | `P3-AC-05-stt-policy.json`, `P3-AC-05-llm-policy.json`, blocked job records, bounded no-egress observations, task-kind counter before/after, redacted classifications. | **STT verdict: NOT_RUN**<br>**LLM verdict: NOT_RUN** | If the product hides the STT cloud action while policy is off, keep the STT verdict NOT_RUN; do not force the branch. This row is two task-kind checks, not one generic policy result. |
| **P3-AC-06**<br>Daily cap atomic reservation / no over-admission | **STT sub-check:** OpenAI/STT is configured, STT cloud is on, the explicit mobile cloud-transcription action is available, and the STT counter is at or above the one shared `daily_cap`. **LLM sub-check:** Anthropic/LLM is configured, LLM cloud is on, Ollama is stopped with user approval, and the LLM counter is at or above that same shared `daily_cap`. The atomic remediation is complete, locally retested, and Terra-reviewed; no concurrent real-provider job may be created. | Use the local concurrent-boundary and counter-write evidence for the hard-cap implementation. For the live boundary, submit at most one explicit cap-blocked STT action and one explicit cap-blocked LLM fallback, without running them concurrently or retrying. Record the pre-check decision, provider-host observation, output state, and independent counter before/after. Do not hand-edit either counter. | The atomic reservation implementation admits no more than `daily_cap` reservations per task kind, while the shared `daily_cap` setting leaves STT and LLM counters independent. A blocked/reservation-failed attempt makes no provider request and consumes no slot. A committed slot is charged before provider response and remains charged after an error/timeout. | `P3-AC-06-local-boundary.json`, `P3-AC-06-counter-failure.json`, live cap-blocked artifacts if authorized, bounded no-egress observations, task-kind counter persistence, cap before/after, redacted classifications. | **STT verdict: NOT_RUN**<br>**LLM verdict: NOT_RUN**<br>implementation: **COMPLETE; TERRA FINAL PASS** | The historical non-atomic rate-guard blocker is remediated and Terra-reviewed. Local concurrency/fail-closed evidence does not replace live provider-boundary observation, cleanup, and separate Boss/user acceptance; hard-cap Controller PASS remains unavailable while this row is NOT_RUN. |
| **P3-AC-07**<br>Verified local no-egress fault / classification / rollback | No provider call is authorized for this row. A deterministic local fault that occurs before provider HTTP dispatch is available, and the bounded provider-host observer can verify zero provider egress. Capture prior output/checksum, policy, and task-kind counter state. | Trigger one verified local pre-dispatch fault, such as an approved local policy/transport/fixture failure. Record only status/classification, elapsed time, retryability, job state, counter behavior, and authorized rollback state. Do not capture a provider response body. Do not induce a provider-reported error or timeout, because it may transmit data/cost and its response text is only truncated, not reliably redacted. | Zero provider egress and zero provider cost are observed. The local classification is redacted and the counter does not increment for a call that never dispatched. Cloud failure behavior is not required to be terminal: a retryable job may remain non-terminal and may retain received segments, as documented by `src-tauri/src/fungwire_server.rs:1110-1116`. Restore only authorized local state; do not assert that all residual job files disappeared. | `P3-AC-07-local-fault.json`, `P3-AC-07-no-egress.txt`, status/classification, retryability/job state, counter before/after, authorized rollback record. | **NOT_RUN** | Provider-reported errors/timeouts are explicitly outside acceptance. If only a provider fault can be induced, record NOT_RUN; never spend or transmit data to manufacture this row. |
| **P3-AC-08**<br>Cleanup/revocation/post-state | Matrix rows are complete or a stop condition is frozen; before-state exists; test-created keyring slots and known artifacts are identified exactly. | Clear only test-created OpenAI/STT and Anthropic/LLM entries through the approved UI. Turn cloud toggles off, then restore the original cap, Ollama state, and test-only pairing/artifact state. Remove only exact test-created files. Reopen the status surface and capture after-state; verify that a later cloud attempt would be denied by the restored policy or absence of a test-created key, whichever applies, without making a provider call. | Test-created credentials are revoked/absent; pre-existing credentials and user data are untouched; policy and local runtime state match the authorized before-state; evidence contains no key; known cleanup actions are enumerated with hashes/paths. Do not claim that no residual artifact exists outside the inspected evidence/test root; an unknown or unobserved external path is NOT_RUN, not clean. | `P3-AC-08-post-state.json`, status-only screenshot, cleanup manifest, before/after diff, known-artifact hash/list, residuals-not-asserted note. | **NOT_RUN** | Do not delete pre-existing keyring slots, recordings, projects, counters, or pairings. Non-disposable daily counters may remain as observed and must be reported, not silently rewritten. Cleanup is exact-scope only; no global residual-artifact claim is permitted. |

## 8. Exact observation checklist / checklist การสังเกต

The operator completes this checklist for every relevant row. `unknown`, blank,
or secret-bearing output is not acceptable evidence.

| Observation | Exact value to record | Secret/data rule |
|---|---|---|
| Provider and model | Effective provider (`OpenAI`, `Anthropic`, or other expected label) and model name as surfaced by the app/provider; for LLM, compare persisted `model_runs` provenance with runtime location | Never record a key, account ID, or raw header; a cloud runtime with Ollama provenance is a stop/FAIL observation, while local remediation evidence alone remains NOT_RUN for live acceptance |
| Task/job IDs | STT delegated-job ID; LLM graph/summary job ID; any provider request ID only if it is non-secret and user-approved | Do not include bearer tokens or opaque credential material |
| Recording/project IDs | Approved project ID, recording ID, source channel, duration, and source SHA-256 | Keep audio outside chat/repo; no raw audio attachment |
| Policy | `stt_cloud_enabled`, `llm_cloud_enabled`, `daily_cap`, task kind, and count before/after | Integer/boolean state only |
| Local Ollama | Before/after running or stopped state, local endpoint/model label, stop/start timestamp | No local auth or environment dump |
| Request destination | Scheme/host/path, method, TLS observed, response status, elapsed time; expected OpenAI/Anthropic path | Never capture `Authorization`, `x-api-key`, query secrets, request body, audio, or prompt |
| Redacted logs | Job transition, policy reason, provider/model, status/classification, retryability, and retry count | Do not save a provider response body for the fault row; no key value or prefix |
| Output checksum/provenance | Transcript/summary/output SHA-256, segment count, timestamps, provider/model, policy decision, executor, source/job IDs | Content only in an approved local store; checksum is not a substitute for a missing output |
| Cloud call count | STT and LLM counts before/after; expected delta `+1` only after a successful provider dispatch | Never reset or hand-edit the counter |
| Mobile reflection | Paired desktop cloud status, `executor=cloud` where applicable, cloud badge, transcript arrival | This is test-fixture evidence, not device-release evidence |
| No key serialization | Local/static source/test or approved local-artifact evidence that no key-bearing field/value enters Genesis/application state, Supabase-facing state, `localStorage`, screenshots, logs, or exports | Do not perform a live Supabase/Genesis/network scan; record the bounded evidence result and paths only |
| Cleanup/post-state | Provider statuses, toggles, cap, Ollama, pairing, exact artifact list, and unauthorized differences | Delete only exact test-created targets |

For the destination checklist, expected live hosts are the provider API hosts
declared by the locked spec: OpenAI STT at
`https://api.openai.com/v1/audio/transcriptions` and Anthropic LLM at
`https://api.anthropic.com/v1/messages`. A custom endpoint is outside this
acceptance matrix unless Boss approves a separate scope; it must be HTTPS and
must be recorded without credentials.

## 9. PASS / FAIL / NOT_RUN and stop conditions

### 9.1 Verdict rules

- **PASS:** every required action for the row occurred; the expected result is
  observed; artifacts are complete, timestamped, redacted, and checksum-linked;
  the destination, provider, policy, count, provenance, and post-state agree;
  no stop condition occurred. P3-AC-04 cannot PASS until effective Anthropic
  provider/model/endpoint provenance is observed in the authorized live row;
  the local remediation and Terra review are supporting evidence only. P3-AC-06
  cannot PASS as a hard-cap Controller result while its live row remains
  NOT_RUN; the local atomic/concurrency evidence is supporting evidence only.
- **FAIL:** an observed behavior violates the row or a stop condition is
  confirmed: a key leaked, an unexpected request occurred, the wrong provider
  ran, policy/cap was bypassed, provenance/badge was lost, output was
  corrupted, an undocumented retry/reprovider switch occurred, or cleanup
  touched an unauthorized target.
- **NOT_RUN:** a prerequisite or observation was unavailable; the user did not
  authorize the call/recording; no safe fault injection existed; a UI path was
  not exposed; a provider-reported error/timeout was the only available fault
  path; the result was ambiguous; or only local/CI/fixture evidence was
  available. NOT_RUN is not PASS and must remain visible.

`BLOCKED_PENDING_REMEDIATION` is retained only as a historical gate annotation,
not a current PASS or hidden FAIL. The implementation remediation and
independent local retest are complete; the controller verdict remains NOT_RUN
until the required live evidence and approval are complete.

The Phase 3 Controller Gate can be marked PASS only when `P3-AC-01` through
`P3-AC-08` have an accepted verdict, the real OpenAI and Anthropic boundaries
are represented where required, cleanup/post-state is reviewed, and Boss/user
gives final acceptance. In this revision, P3-AC-04 and P3-AC-06 are
implementation-complete and Terra-reviewed, but their live verdicts are
**NOT_RUN**; the overall Controller Gate therefore remains NOT_RUN. A row with
supporting local or CI evidence but no required live observation remains
NOT_RUN for the controller gate.

### 9.2 Mandatory stop conditions

Stop the current run immediately, preserve only redacted evidence, and notify
the user/Boss if any of the following occurs:

1. raw key material appears in a log, screenshot, URL, header capture, error,
   serialized state, Genesis/application export, Supabase, or `localStorage`;
2. cloud egress occurs while the relevant toggle is off, the cap is reached, or
   no provider call was intended;
3. the request goes to the wrong provider/model/endpoint, or the mobile job
   claims cloud while the desktop used local execution;
4. the policy or cap is bypassed, a blocked request increments the counter, or
   an undocumented retry/reprovider switch occurs. Documented retryable cloud
   failure behavior is not itself a failure;
5. transcript/summary provenance, project/recording/job linkage, timestamps,
   cloud badge, checksum, output integrity, or data integrity is lost, or data
   corruption is observed;
6. a provider-reported error/timeout is encountered during an acceptance row,
   or a result is ambiguous; stop, record status/classification only, and do
   not repeat a provider call to make the result look deterministic;
7. a recording, project, pair, policy, counter, keyring slot, or artifact outside
   the exact approved test scope changes unexpectedly;
8. Ollama state cannot be established for the fallback or rollback step, or the
   egress observer is unavailable/secret-bearing;
9. the user withdraws consent, the cap is exhausted, the provider reports an
   account/billing concern, or the real recording is no longer approved.

After a suspected leakage, do not copy the value for diagnosis. The user/Boss
must revoke/rotate the affected credential through the provider and keyring UI
out of band. The operator records only `leakage suspected/confirmed`, the
affected acceptance ID, timestamps, and redacted artifact paths.

## 10. Rollback and budget safety / การย้อนกลับและความปลอดภัยด้านงบ

- Use at most one minimal successful test call per provider in this matrix:
  one OpenAI STT call and one Anthropic LLM call. Do not add OpenAI LLM, custom,
  or repeated provider calls without a new Boss decision.
- Policy-blocked and cap-blocked attempts must not reach the provider in the
  sequential/quiescent observation. Verify no provider-host egress before
  considering the row complete; this does not establish a concurrent hard cap.
- Use the fixed implementation timeouts: 120 seconds for STT and 60 seconds
  for LLM. Do not increase a timeout to hide a root cause.
- After an ambiguous result, stop. No repeated retries are allowed; do not
  switch providers or repeat the recording. Preserve the before/after state and
  report NOT_RUN or FAIL.
- Stop before a call when the daily request cap is reached in the sequential
  test. The one `daily_cap` setting is shared by STT and LLM, while their
  counters are independent. The current reservation is atomic and fail-closed;
  a committed slot is charged before provider response and retained after an
  error/timeout. No hard-cap Controller PASS is allowed while the live row is
  NOT_RUN.
- Record estimated spend only when the provider itself reports it in a safe,
  non-secret receipt. Do not invent a price, exchange rate, or cost estimate.
- Restore the captured policy, Ollama state, pairing state, and exact test
  artifacts. Remove only test-created keyring entries and artifacts. Do not
  hand-edit counters or delete user history to make before/after values match.
- If a provider reports a billing/authorization problem, stop provider use and
  hand control back to the user/Boss; do not attempt alternate credentials.

## 11. Roles and approval boundaries / บทบาทและอำนาจอนุมัติ

| Role | Allowed actions | Not allowed |
|---|---|---|
| Boss / user | Approve the live run, type credentials into the keyring-backed UI, consent separately to OpenAI recording/STT transfer and Anthropic graph/summary transfer, opt into cloud, choose the recording, set the cap, control provider billing, stop/start Ollama, approve cleanup, and issue final acceptance. | No requirement to disclose a key to the operator or chat. |
| Future acceptance operator | Follow this runbook, perform approved UI actions, observe the desktop/mobile path, capture redacted IDs/counts/destinations/checksums, stop on ambiguity, and prepare the evidence bundle. | Cannot request/paste/print keys, widen scope, bypass UI policy, retry ambiguity, delete user state, or claim production/device/Meet readiness. |
| Parent orchestrator/reviewer for this task | Review this one-file runbook and later review evidence against the matrix. | Does not implement code, run a provider, request/use secrets, alter keyring/user data, commit, push, merge, or deploy. |
| FUNG implementation | Existing reviewed code and automated tests are supporting evidence only. | Source/CI evidence cannot be relabeled as live Controller Gate evidence. |

## 12. Traceability / การตรวจสอบย้อนกลับ

| Runbook coverage | Normative source |
|---|---|
| Phase 3 objective, exit gate, and acceptance-pending status | `docs/plans/2026-08-09-fung-master-implementation-plan.md`, Phase 3 and §10 tracker |
| Implementation baseline and Controller Gate wording | `docs/plans/2026-08-09-phase-3-byom-cloud-keys.md`, Current Implementation Evidence and Controller Gate |
| Locked keyring, policy, executor, timeout, error, mobile, security, and testing decisions | `docs/specs/2026-08-09-phase-3-byom-cloud-keys-design.md`, §§1–17 |
| Historical baseline — P3-AC-04 provenance blocker | `src-tauri/src/graph_build.rs:665`; pre-remediation cloud runtime location was recorded while the persisted provider/model/endpoint remained Ollama-derived. Current status: see §2.2 Current post-evidence — 2026-09-22. |
| Historical baseline — P3-AC-06 daily-cap limitation | `src-tauri/src/policy.rs:39-42` and `src-tauri/src/fungwire_server.rs:1196-1234`; pre-remediation read/check/increment was non-transactional and counter-write errors were ignored. Current status: see §2.2 Current post-evidence — 2026-09-22. |
| Retryable cloud-job boundary | `src-tauri/src/fungwire_server.rs:1110-1116`; selected cloud failures may be non-terminal and may retain received segments |
| Genesis authority, provenance, loopback/local-first and cloud-off privacy defaults | `docs/Desktop/ARCHITECTURE.md`, GenesisBlockDB, BYOM Model Adapters, Security and Privacy Defaults |
| Separation of automated evidence from real provider/device/release evidence | `docs/Desktop/08-real-progress.md` Phase 3 post-merge overlay and current truth sections |
| Mobile implementation boundary and real OpenAI/Anthropic/device gaps | `docs/Mobile/IMPLEMENTATION_STATUS.md`, Current Phase 3 Post-Merge Overlay and Current Environment Recheck |
| Agent authority and documentation-first/surgical/verification rules | `AGENTS.md`, R1–R10 and Definition of Done |

Requirement mapping:

| Requirement | Mandatory rows |
|---|---|
| `REQ-F-01` | P3-AC-01, P3-AC-07, P3-AC-08 |
| `REQ-F-02` | P3-AC-02, P3-AC-03, P3-AC-04, P3-AC-05, P3-AC-06 |
| `REQ-F-03` | P3-AC-03, P3-AC-04, P3-AC-05, P3-AC-06, P3-AC-07 |
| `REQ-F-04` | P3-AC-02, P3-AC-03, P3-AC-05, P3-AC-08 |

The approved spec's fixed LLM priority is Anthropic → OpenAI → Custom when
multiple LLM providers are configured. The matrix therefore expects Anthropic
in P3-AC-04 when its slot is configured. STT OpenAI segment confidence is
expected to use the implementation's documented default (`1.0`) when surfaced;
the operator must record what the app actually returns and must not invent a
different value.

## Version Diff

| Version | Change |
|---|---|
| 0.2.0b → 0.3.0b | Added the 2026-09-22 post-evidence AC-04/AC-06 remediation update, historical-baseline traceability correction, Terra/Luna review result, and the unchanged live Controller/provider/device NOT_RUN boundary. |
| 0.1.0b → 0.2.0b | Terra correction: use supported `candidate` status with separate `NOT_RUN`, add C-3/HIGH and base-commit provenance, constrain keyring/consent/egress boundaries, block incorrect LLM provenance, correct non-atomic cap semantics/task-kind verdicts, and replace provider-fault acceptance with verified local no-egress evidence. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.3.0b | 2026-09-22 | candidate | Recorded the post-evidence AC-04/AC-06 remediation update, Terra FINAL PASS and Luna review, historical-baseline traceability, and the live Controller/provider/device NOT_RUN boundary; no production, commit, or deployment proof is claimed. | working-tree; no new commit; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b | RWANG |
| 0.2.0b | 2026-09-22 | candidate | Applied Terra's required documentation corrections; controller execution remains NOT_RUN and known P3-AC-04/P3-AC-06 blockers remain explicit. | working-tree; no new commit; base 538213ffb0d69d416fe0e3e2d9d5f4beed24305b | RWANG |
| 0.1.0b | 2026-09-22 | candidate | Added the bounded Phase 3 BYOM acceptance runbook and P3-AC-01..08 evidence matrix; controller execution remains NOT_RUN. | working-tree; no commit | RWANG |
