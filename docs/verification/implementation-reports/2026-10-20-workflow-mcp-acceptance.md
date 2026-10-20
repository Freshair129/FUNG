---
version: "0.1.0"
created_at: "2026-10-20,Codex"
last_update: "2026-10-20,Codex"
status: "PASS"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "Local synthetic audio workflow acceptance and developer MCP"
  language: "Thai-first"
---

# FUNG workflow MCP acceptance

## Outcome

**PASS for the pinned local file-import workflow.** A real CPU transcription
run produced transcript segments, the edited transcript and audio exports
survived an orderly GenesisBlockDB close/reopen, and the MCP returned the
structured result and per-stage timings. Transcript quality is diagnostic
only. This does not change the separate Desktop/core acceptance status.

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-20 |
| Branch | `codex/readiness-fixes-20261015` |
| Source HEAD before local edits | `56c53544718989fd5429dd5071479262fab7c2ee` |
| MCP run | `workflow-2026-10-20T102112990Z-5aaaadd7` |
| Evidence folder | `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-20T102112990Z-5aaaadd7` |
| Fixture | `standalone-english-48k.wav`, 1,390,410 bytes |
| Fixture SHA-256 | `493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F` |
| Runtime | CPU / int8 / large-v3-turbo, revision `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf` |
| Resource scope | No microphone, user recording, GPU inference, or Ollama use |

## Workflow assertions

- Production loopback import created a project, recording and transcription job.
- The job completed and returned two non-empty transcript segments.
- Playback returned 1,390,410 bytes identical to the input fixture.
- A transcript edit completed through the production edit path.
- SRT and VTT exports contained the edit; WAV export matched the imported audio.
- After orderly listener/store close and Genesis reopen, the completed job,
  edited transcript, recording and exports remained available.
- `fung.workflow.list_runs` returned the passing run and its stage timings.
- The acceptance used working-tree changes on top of the recorded source HEAD;
  the implementation and its verification were not committed in this task.

## Timings

Test-internal times are milliseconds. The workflow `total` excludes Cargo and
MCP startup overhead; the runner receipt records those separately.

| Stage | Time |
| --- | ---: |
| Initial database open | 392.2 ms |
| Loopback upload | 64.8 ms |
| Transcription job | 28,852.7 ms |
| Transcript read | 49.1 ms |
| Playback read | 43.6 ms |
| Transcript edit | 25.1 ms |
| Subtitle export (SRT + VTT) | 27.1 ms |
| Audio export (WAV) | 37.0 ms |
| Database close/reopen/readback | 829.6 ms |
| Workflow total | 30,461.7 ms |
| MCP runner wall duration | 41,642 ms |

The transcript diagnostic matched 12/12 pinned English keywords and returned
two segments. This score is informational and is not an acceptance gate; this
fixture does not qualify Thai transcription.

## MCP surface and verification

The registered `fung_workflow` server exposes three fixed tools:

- `fung.workflow.describe`
- `fung.workflow.run_acceptance`
- `fung.workflow.list_runs`

The acceptance tool runs the fixed fixture and model with CPU/int8; callers
cannot pass file paths, shell commands, model names, or profiles. Outputs stay
under a new run-specific folder. The MCP test command passed 3/3, covering
stale receipt classification, modern discovery, and the legacy initialize/tools
flow. The Codex MCP registry parsed and enabled the server. The passing workflow
itself was invoked through MCP.

One earlier invocation failed because the test expected a new output root
while the runner had already created that folder for its receipt and log. The
runner now gives the acceptance test a new `acceptance-output` child folder;
the subsequent complete run passed. Earlier setup failures remain visible in
the run history rather than being removed. Stale `RUNNING` receipts older than
the runner timeout and grace period are reported as `STALE`.

## Scope limits

- This is file import, not microphone capture or system-loopback capture.
- It verifies orderly store reopening, not forced-crash recovery.
- It does not exercise the packaged FUNG UI, TTS, Thai transcription quality,
  GPU acceleration or performance tuning across multiple hardware profiles.
- The runner provides a repeatable baseline and stage timings; optimize only
  after collecting comparable runs with the same fixture and runtime revision.
- This is developer MCP tooling. The full product MCP surface remains a
  separate implementation and acceptance item.

## Artifacts and reproduction

- Run receipt: `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-20T102112990Z-5aaaadd7\run-receipt.json`
- Workflow result: `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-20T102112990Z-5aaaadd7\workflow-result.json`
- Runner log: `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-20T102112990Z-5aaaadd7\run.log`
- Runbook: [FUNG workflow MCP](../../operations/fung-workflow-mcp.md)
- Contract: [`workflow-acceptance-mcp-v1.yaml`](../../../contracts/workflow-acceptance-mcp-v1.yaml)
