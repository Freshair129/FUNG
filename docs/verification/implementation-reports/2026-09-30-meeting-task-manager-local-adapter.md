---
version: "0.1.0"
status: local-source-and-fixture-tests-passed
complexity: C-3
risk: HIGH
---

# Meeting & Task Manager local adapter — verification

Base: `1ca55145263dd3070376d57086347cb5690a17ea`, isolated uncommitted branch
`feature/meeting-task-manager-local` at `C:/Users/pc/workspace/fung-meeting-task-manager`.
[Approved bounded plan](../../plans/2026-09-30-meeting-task-manager-local-adapter.md).

## Version diff

Before: local API exposed recordings, audio, legacy transcript and upload/jobs.
After: bearer-header-protected, loopback-only integration capabilities,
revision-aware snapshots and bounded local action proposals. No migration,
native transcript writes, task writes or provider activation are introduced.

- `meeting_task_manager.rs`: source scope/identity/hash, native/legacy snapshots,
  reviewed input checks, local inference, exact quote evidence and deterministic
  request-derived batch/proposal IDs. All proposals remain subject to review.
- `local_api.rs`: authenticated namespace dispatch; integration request body
  cap applied before allocation; integration routes refuse non-loopback local
  addresses. Existing recording APIs remain unchanged.
- `meeting_agent_model.rs`: two existing restricted transport helpers become
  crate-visible; no endpoint policy or cloud fallback behavior changes.
- `lib.rs`: registers the additive module.

## Validation — 2026-09-30

Cargo compiled the library test target from the isolated worktree. The test-only
process set `TAURI_CONFIG={"bundle":{"resources":[]}}` so source verification
does not require copying the unstaged Whisper bundle. No tracked Tauri config
was changed. This does not establish bundled/installed runtime readiness.

| Check | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib meeting_task -- --test-threads=1` | 15 passed, 0 failed, 1 manual browser harness ignored |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib local_api::tests -- --test-threads=1` | 25 passed, 0 failed, 2 ignored (manual harness and pre-existing real-Whisper test) |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib meeting_agent_model::tests -- --test-threads=1` | 5 passed, 0 failed |
| `rustfmt --check --edition 2021` on changed Rust modules | passed |
| `git diff --check` | passed |
| Independent source review | capability error-redaction and test-harness socket-shutdown findings fixed; both re-reviews accepted |

The first test filter includes two tests also counted by `local_api::tests`;
do not sum the rows as distinct tests. Coverage includes native human revisions
superseding legacy text, empty and bounded transcripts, review/source scope,
timecode/hash tampering, UTF-8 text bounds, malformed/unsupported output,
fabricated quotes, stale source before and after inference, deterministic
identities, no-model behavior, bearer-only auth, denied origins, and real socket
snapshot/CORS/body-framing checks. Existing upload/audio/range/job tests passed.

The review-time fix and prevention are documented in
[capability error RCA](../../../.brain/rca/2026-09-30-meeting-task-capability-errors.md).
The QA-only lifetime fix is recorded in
[fixture shutdown RCA](../../../.brain/rca/2026-09-30-meeting-task-qa-fixture-shutdown.md).

## Consumer contract and practical limits

All paths are relative to `/integrations/meeting-task-manager/v1`:

- GET `/capabilities`: `protocolVersion: 1`, non-secret `sourceInstanceId`,
  snapshot modes, `actionDraft.available/readiness/reasonCode/modelName`, limits.
- GET `/recordings/{id}/snapshot`: schema v1, native revision/cursor or explicit
  legacy nulls, content hash, source segments and coverage. The native owner
  exposes at most its latest 200 utterances. Legacy snapshots return at most
  their first 200 ordered segments. `coverage.status` is `bounded` at a window
  limit or `unknown`; neither means complete speech coverage. Unknown meeting
  start/timezone are null. Native speaker labels remain null rather than
  inventing names from cluster IDs.
- POST `/action-drafts`: strict request schema from the canonical spec;
  `reviewedSegments` contain only `segmentId`, `startMs`, `endMs`, `text` and
  optional `speakerLabel`. Do not send entire snapshot segment objects.
  `reviewHash` is lowercase SHA-256 of compact UTF-8 JSON array
  `[reviewRevisionId, segments.map(s => [s.segmentId,s.startMs,s.endMs,s.text,s.speakerLabel ?? null])]`.
- Limits: 256 KiB HTTP input, 200 reviewed segments, 96,000 UTF-8 text bytes,
  30 output proposals, 8 evidence quotes per proposal and bounded model output.
- Model uses configured `ollama-summary-intent` endpoint and exact model name,
  with FUNG's existing default only when no model name is configured. A missing
  model blocks AI drafting. No model is downloaded or substituted.
- Every quote is checked against the reviewed segment. Suggested owner/deadline
  text must occur in evidence; date stays null for human resolution. Speakers
  are never assigned automatically. `modelProvenance.persistedInFung` is false:
  the consumer owns the imported draft record, not a new FUNG database row.
- Same request/source/review identity gives the same batch/proposal IDs.
  Inference is not durably cached by FUNG; a retry may produce different text.
  The consumer must retain same-key/different-payload Conflict handling instead
  of overwriting or silently committing a changed proposal set.

## Unrun / not claimed

Real local model inference, real audio → Whisper → browser end-to-end, the user's
running desktop, packaged/installed runtime, hosted CI, deployment and production
are not verified by this patch. The live FUNG was not started, stopped, replaced
or reconfigured; the primary worktree was not edited. New routes require a build
of this source before the running desktop can serve them. No commit or push was
performed, and external dispatch stays disabled.

## Explicit browser QA fixture amendment

At the integrating parent's request, an ignored test
`local_api::tests::meeting_task_browser_fixture` supplies a bounded browser QA
server using the exact production handler/adapter against a disposable Genesis
ledger, synthetic WAV/transcript and an Ollama-shaped local transport fixture.
It is compiled only for tests, requires explicit port/stop-file environment
variables, expires after 15 minutes, and does not open the user's FUNG ledger.
The model name is `qa-fixture:not-real`; proposed titles identify QA fixture
output. Audio import is unavailable because no Whisper runtime is attached.

Run explicitly from this worktree, with an unused loopback port and an absent
stop file:

```powershell
$env:TAURI_CONFIG='{"bundle":{"resources":[]}}'
$env:FUNG_MEETING_TASK_QA_PORT='4330'
$env:FUNG_MEETING_TASK_QA_STOP_FILE='C:/Users/pc/workspace/fung-meeting-task-manager/target/meeting-task-qa.stop'
cargo test --manifest-path src-tauri/Cargo.toml --lib meeting_task_browser_fixture -- --ignored --nocapture --test-threads=1
```

Synthetic connect URL: `http://127.0.0.1:4330/#meeting-task-qa-fixture-token`.
The literal token is exclusively for this synthetic QA fixture. The existing
local-dev origin policy permits the isolated browser QA app on `127.0.0.1:4320`.
Creating the configured stop file stops the harness early; otherwise it expires.
The stop path closes an active fixture-model socket before joining, so incomplete
input cannot extend the lifetime indefinitely.

A Node HTTP check passed through production routes and model transport:
capabilities/snapshot/action draft all 200, canonical Node review SHA-256
accepted by Rust, edited Thai evidence quoted exactly, retry batch identity
stable, original snapshot unchanged. This is fixture-model transport evidence,
not real AI inference, Whisper or installed-desktop evidence.

The integrating parent verified browser interaction on the isolated QA app:
connect, import `rec-live`, edit Thai text/speaker, save review, draft, choose
Chef as R and Must for the week, commit one task, replay without duplicates,
reload with persistence, and re-import without duplicate source records. Source
quote/task backlink was checked. Authenticated blob audio played on the `mic`
channel. The fixture's `sourceMode: legacy` is intentional; native revision
behavior is covered by the separate Rust fixture. This is browser fixture
evidence, not installed FUNG/model/audio-upload acceptance.

After browser QA the listener was stopped. Final Rust filters above were rerun
on the source including the socket shutdown fix. A second short harness run
held an incomplete model HTTP request open, then wrote the stop marker: the
socket closed in 40 ms and the test ended successfully. Final check found zero
listeners on the QA API/model ports. No QA server remains running.

Existing recording API compatibility: `recordings[].channels` is a string array
containing available `mic`, `system` and/or `file` values. Omitting `channel`
lets the server choose; do not assume all recordings have `file`. Existing
`X-Fung-Filename` is not URI-decoded; the consumer retains original Unicode
filenames locally and uses a neutral ASCII upload header for compatibility.

## Desktop build handoff

This work compiled `src-tauri/target/debug/deps/fung_lib-28832547a0759014.exe`,
the Rust test executable. It did not produce or launch a desktop EXE.
`cargo build --manifest-path src-tauri/Cargo.toml --bin fung` would produce
`src-tauri/target/debug/fung.exe`; that path is an expected output, not a built
artifact claim. Normal desktop builds require frontend dependencies/assets and
the configured runtime resources (including `.venv-whisper`), which are not
staged in this isolated checkout. `npm run desktop` is the repository's dev
entry point once prerequisites are prepared. Do not interpret the source-test
resource override above as a packaged runtime or start a competing desktop
against the user's active ledger without an explicit runtime handoff.
