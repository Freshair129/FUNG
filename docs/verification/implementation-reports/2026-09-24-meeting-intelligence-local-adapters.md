---
version: "0.1.6b"
created_at: "2026-09-25T03:13:29+07:00,RWANG"
last_update: "2026-09-25T09:23:18+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "implementation-report"
  scope: "Approved local M1-M5 implementation and provider-neutral adapters"
  risk: "HIGH"
  language: "Thai/English"
---

# FUNG local meeting-intelligence implementation report

## Scope and outcome

The user approved M1–M5 local implementation with provider-neutral adapters,
repository audio fixtures, and external provider activation deferred. This
report records the completed local fixture campaign, account-transition
remediation and its independent source review, plus the cursor follow-up. The
follow-up received local source review; an independent review of that diff is
NOT_RUN.

**Current outcome: PASS_LOCAL_REVIEW_REMEDIATION_WITH_LIMITS.** The native
observer and stale-cursor fixes passed the consolidated local campaign. This is
not full product, provider, physical-device, installed-artifact or release
acceptance.

## Gap status

| Gap | Local result | Remaining acceptance |
| --- | --- | --- |
| GAP-07 / M1 transcript revisions | PASS — durable revisions, replay/cursor recovery, corrections and queue-gap paths exercised | Thai WER/CER, real audio, target-device latency/soak and capture acceptance are NOT_RUN |
| GAP-08 / M2 knowledge and People | PASS_LOCAL — selected local corpus, citations, typed metrics, encrypted People lifecycle/recovery and Windows PDF isolation fixtures; People, metric and draft account-transition paths fenced, with refresh-witness event handling | Private real-source UAT, native Tauri flow, cross-logon-session mutex runtime and non-Windows PDF import remain open |
| GAP-09 / M3–M5 agent/delivery/control | PARTIAL_LOCAL — a native observer tracks durable transcript cursors; explicit manual cited drafts and local preview remain available; server boundaries reject stale transcript cursors | Automatic transcript-triggered drafts are blocked because local ASR has no trusted non-self/non-agent attribution; Google Meet join/media, external send/receipt, remote enforcement and room reconciliation are NOT_RUN |
| GAP-01/02, GAP-03–06, GAP-10–12 | Not closed by this implementation | Runtime quality, provider, clean-install restore, Mobile, release, speaker quality and remaining scope/status gates stay separate |

## Verification results

| Lane | Result |
| --- | --- |
| Rust library | PASS — 577 passed, 0 failed, 1 ignored; full run used host profile access for the AppContainer case |
| Meeting-knowledge behavioral integration | PASS — 17/17 |
| Windows AppContainer probe | PASS — child denied access to a host-only file and loopback when run with per-user AppContainer profile access; restricted sandbox fails closed |
| Knowledge extractor registered suite | PASS — 7/7 with Python 3.12.14 and pypdf 6.10.0 |
| Pinned knowledge parser fixtures | PASS — 7/7 with staged CPython 3.11.9 and pypdf 6.10.0 |
| Rust validation | PASS — `cargo fmt --check`, `git diff --check`, offline locked all-target `cargo check`, all-target `cargo clippy -- -D warnings`, and local `cargo build` |
| Desktop/worker suites | PASS — all 32 registered `test:*` suites; one Docker-backed W1 test skipped because Docker was unavailable (`spawnSync docker EPERM`) |
| Desktop production build | PASS — `npm run build` (TypeScript and Vite) |
| Mock-browser UI fixture flow | PASS — selected/unlocked local vault, private cited draft, and auth-session transition cleared the draft and reset vault selection |
| Native Tauri-window and accessibility acceptance | NOT_RUN — fixture browser flow is not the installed/native Desktop surface |
| Account-transition source/security review | PASS — independently reviewed the exact post-remediation source hashes and found no blocker |
| Account-lifecycle UI fixture | PASS — fixture verified that auth-session change clears the private draft and vault selection |
| Follow-up review regressions | PASS — runtime cursor invalidation and current/stale durable cursor guards |
| Follow-up native-session-custody retry | PASS — 12/12 with isolated Cargo target and Tauri fixture config |
| Follow-up source review | PASS — reviewed the changed boundaries, race fences and fail-closed trigger status |
| Independent review of follow-up diff | NOT_RUN — no second independent review after these fixes |
| Separate-logon-session keyring concurrency | NOT_RUN — separate Windows logon-session mutex behavior was not exercised |

## Requirement evidence crosswalk

This crosswalk maps the requirement families referenced by the approved plan
to the local implementation and regression evidence. `PASS_LOCAL` means that
the in-scope local behavior has code and fixture coverage; it does not mean
every end-to-end acceptance scenario in the parent specifications passed.

| Requirement IDs | Local verdict | Local implementation boundary | Repository regression evidence | Acceptance still NOT_RUN |
| --- | --- | --- | --- | --- |
| LT-01–LT-16 | PASS_LOCAL for revisioned transcript, custody, replay, correction and gap paths | `live_meeting.rs`, `live_transcript.rs`, `genesis_adapter.rs`, and `meeting_intelligence_schema.rs` provide bounded capture processing, revisioned commits, correction, cursor replay/snapshot, and explicit gaps | `live_transcript.rs` unit cases; `genesis_adapter.rs` v12 batch, replay, correction and custody cases; `tests/transcribeLiveWindow.test.py`; `tests/meetingIntelligenceUi.test.mjs` reducer/bridge cases | Thai WER/CER, real-device capture, language-quality corpus, latency and soak acceptance |
| KE-01–KE-14 | PASS_LOCAL for selected local import, citation, metric, People and recovery fixtures | `meeting_knowledge.rs`, `meeting_knowledge_windows.rs`, `genesis_adapter.rs`, `extract_knowledge.py`, and backup modules implement selected local imports, encrypted assets, citations, metrics, People review and recovery | `tests/meeting_knowledge.rs`; `tests/knowledge_extract.test.py`; Rust People/PDF/backup fixtures; `tests/meetingIntelligenceUi.test.mjs` scoped bridge cases | Private real-source UAT, non-Windows PDF sandbox, installed-artifact flow and cross-logon-session keyring concurrency |
| MA-01–MA-18 | PASS_LOCAL for local policy, draft, approval, outbox and reconciliation boundaries | `meeting_agent.rs`, `meeting_delivery.rs`, `meeting_adapter.rs`, `meeting_intelligence_runtime.rs`, and guarded Genesis mutations provide local policy, draft, approval, outbox and receipt boundaries | Rust agent/delivery/adapter cases for committed input, ACL, limits, exact approval, uncertainty/reconciliation and unconfigured transport; `tests/meetingIntelligenceUi.test.mjs` contract cases | Real admission/media, room/audience behavior, external send/receipt, provider metering and watchdog operation |
| GM-01–GM-08 | CONTRACT_ONLY — production transport is intentionally unavailable | `meeting_adapter.rs` defines normalized provider-neutral lifecycle, ingress, destination and receipt contracts; production transport remains explicitly unconfigured | Rust adapter cases for capability state, scoped normalization, sequence/replay checks, fixture-only transport and `PROVIDER_NOT_CONFIGURED` | Any actual Google Meet/provider capability, authentication, admission, media, delivery or remote cleanup |

The Node UI suite is contract-level evidence. A separate mock-browser fixture
flow exercised the visible local panel; no native Tauri-window flow or
accessibility audit was run, so those acceptance lanes remain open. See [the approved plan](../../plans/2026-09-24-meeting-intelligence-local-adapters.md)
for the complete external, hardware and independent-review gates.

The latest security review identified an account-transition race in People
plaintext reads and private draft publication, then follow-up inspection found
the same lifecycle requirement in metric aggregation and automatic token
refresh. The source now holds lifecycle fences through result construction,
emits a transition event when refresh changes the account witness, clears
private panel state, and rejects late results. The final account-lifecycle
fixture and independent source review passed. The root cause and remediation
are recorded in the
[account-transition RCA](../../../.brain/rca/2026-09-25-meeting-account-transition-plaintext-race.md).

The test runtime runner used a process-local `TAURI_CONFIG` with bundle
resources disabled and an isolated Cargo target. The AppContainer probe fails
closed when run inside the restricted sandbox because Windows returns
`ERROR_FILE_NOT_FOUND` while creating/accessing the per-user profile store. The
same probe passed when run with host profile access; the implementation does
not fall back to host application data or broaden the host temporary-directory
ACL. The registered `knowledge-extract` suite passed 7/7 with Python 3.12.14
and `pypdf` 6.10.0; the staged CPython 3.11.9 / `pypdf` 6.10.0 parser runtime
also passed 7/7. The final Rust campaign log records 576 passed,
0 failed, 1 ignored, and 17/17 integration tests. Initial compile failures in
the new guards and a non-canonical regression-fixture root were repaired before
the successful retest; the restricted sandbox's AppContainer profile failure
was separately verified with per-user profile access. The `transcribe-concat` mock suite used the bundled
Python 3.12.14 test interpreter with Hugging Face offline mode, because that
suite injects a fake module through `PYTHONPATH` and the embedded production
interpreter isolates that path. See the [People/PDF RCA](../../../.brain/rca/2026-09-25-meeting-intelligence-ui-people-pdf.md)
and [lifecycle test RCA](../../../.brain/rca/2026-09-25-lifecycle-witness-test-race.md).

## Follow-up review remediation

The follow-up review found that agent start reported `observing` without a
native consumer for durable transcript commits, and that freshness was checked
only in the UI before preview and approval. The remediation wires post-commit
transcript cursors into the process-local runtime, cancels runs whose source
cursor advances, marks private drafts stale, removes their local in-memory
previews, and returns the status from drafting to observing.
The native draft-persistence, preview, and approval boundaries now compare the
stored draft cursor with the current durable high-water mark under a Genesis
frontier commit. The panel reports automatic triggering as blocked until a
trusted participant-attribution source exists; microphone/loopback labels are
not accepted as speaker identity. Automatic transcript-triggered draft
generation remains unavailable because trusted attribution and a production
trigger runner are not implemented. See the [observer RCA](../../../.brain/rca/2026-09-25-meeting-agent-observe-trigger-gap.md)
and [stale-preview RCA](../../../.brain/rca/2026-09-25-meeting-agent-stale-transcript-preview.md).

The first restricted-sandbox library attempt could not create the AppContainer
profile (`0x80070002`); it passed on the full rerun with host profile access.
The first `native-session-custody` run also lacked the local Tauri fixture
configuration and failed while scanning `.venv-whisper`; the isolated-target
rerun with bundle resources disabled passed 12/12. These are recorded as
environment corrections, not product failures.

## Explicitly NOT_RUN

Thai WER/CER and accuracy, automatic transcript-triggered draft generation
from authenticated non-self/non-agent speakers, real meeting capture, the
three-hour soak, physical Android, actual Google Meet admission/media, real external delivery/receipt,
provider-side cost/rate/leave enforcement, clean-install restore, non-Windows
PDF sandbox behavior, native Tauri-window interaction, accessibility,
separate-logon-session keyring concurrency, installed-artifact interaction,
deployment and release remain NOT_RUN. The repository audio fixture proves
plumbing and execution only.

## Provenance

The working tree is based on `2c2559fdd907cf94a6f38b34bbb2c6650e68a0e1` on
`codex/gap-closure-runtime-20260923`; follow-up changes were verified against
the PR head before commit. The original 47-file implementation manifest and
its hash remain recorded in the companion [evidence JSON](2026-09-24-meeting-intelligence-local-adapters.json).

## Version Diff

0.1.5b → 0.1.6b: records native transcript observation, server-side stale
cursor rejection, the passing follow-up campaign, and the explicit
trusted-attribution gate for automatic triggers.

0.1.4b → 0.1.5b: records the completed account-transition remediation,
consolidated local campaign, and independent source review; acceptance limits
remain explicit.

0.1.1b → 0.1.2b: records 7/7 parser-runtime fixtures and refreshes the Rust
campaign log; source hashes are unchanged.

0.1.0b → 0.1.1b: adds a requirement-family crosswalk and explicitly records
interactive Desktop UI/accessibility acceptance as NOT_RUN.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.6b | 2026-09-25 | candidate | Adds native committed-transcript observation and stale-cursor guards; local verification passed with explicit automatic-trigger limit | working-tree | RWANG |
| 0.1.5b | 2026-09-25 | candidate | Records final local campaign and independent source review after account-transition remediation | working-tree | RWANG |
| 0.1.4b | 2026-09-25 | candidate | Recorded account-transition plaintext finding and remediation with final test/review pending | working-tree | RWANG |
| 0.1.2b | 2026-09-25 | candidate | Added full pinned-parser fixture result and refreshed the final Rust lane log | working-tree | RWANG |
| 0.1.1b | 2026-09-25 | candidate | Added LT/KE/MA/GM evidence crosswalk and clarified unrun interactive UI acceptance | working-tree | RWANG |
| 0.1.0b | 2026-09-25 | candidate | Local M1–M5 suites and native/Desktop builds passed; independent and product-level acceptance remain open | base 2c2559f; working-tree | RWANG |
