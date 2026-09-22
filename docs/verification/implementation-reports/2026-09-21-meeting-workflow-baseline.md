---
version: "0.1.1b"
created_at: "2026-09-21T05:37:17.176+07:00, RWANG, commit b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-21T05:54:10.961+07:00, RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-report"
  scope: "WF-BASELINE"
  evidence_mode: "read-only"
---

# WF-BASELINE — Meeting-intelligence workflow baseline

## Status and boundary

**Final status: DONE_WITH_OPEN_CONCERNS.**

This is a frozen, read-only pre-report inventory for the requested meeting-intelligence workflow. It records the repository state observed before these two report files were added. It does not implement, repair, approve, dispatch, or qualify any feature or provider. G1 and G2 are **NOT_ISSUED**; this report does not issue either gate as PASS.

The requested workflow configuration is recorded literally:

| Role | Model | Reasoning |
|---|---|---|
| Implementation workers | gpt-5.6-luna | max |
| First verification gate | a different independent gpt-5.6-luna worker | max |
| Final verification gate | gpt-5.6-terra | max |
| Parent | orchestration and risk review only | no source or documentation writing |

No worker, provider, model download, UI, network, real-meeting, build, Cargo, deployment, commit, push, or PR action was performed for this baseline. The only writes authorized by this task are this Markdown report and its JSON companion.

## Snapshot

| Field | Observed value |
|---|---|
| Working directory | C:/Users/pc/workspace/fung |
| Branch | main |
| Full HEAD | b336f33ec400a38f003a0665c121069a87a543ac |
| Capture time | 2026-09-21T05:37:17.176+07:00 |
| Pre-report tracked status | 27 modified, 3 deleted |
| Pre-report untracked status | 10 paths |
| Pre-report total dirty paths | 40 |
| Existing deleted AIOS paths | preserved as deletions; not hashed |
| Report-write scope | the two report paths named in this task only |

The snapshot was taken before adding these report files. Existing user edits and deleted AIOS files are retained. No credential or secret contents were read; .env.example is represented only by path, size, and digest.

## Post-write verification boundary

The final artifact check ran after the frozen snapshot. git status --short exited 0 and showed the same 40 pre-report paths, the two authorized report additions, and one additional concurrent untracked path: docs/plans/2026-09-21-meeting-intelligence-multiagent-workflow.md. That workflow-plan path was not in the pre-report snapshot, was not created, read, hashed, or edited by this task, and is not assumed frozen. The current status therefore contains 43 paths; the baseline itself remains the 40-path pre-report snapshot. The parent must refresh the workflow manifest before relying on that separate in-progress document.

The JSON companion parsed successfully with PowerShell ConvertFrom-Json (exit 0). An initial apply_patch wrapper attempt failed before execution because embedded Markdown backticks were interpreted as JavaScript template-literal syntax; it had no filesystem effect, and the bounded retry succeeded.

## Current dirty path inventory

The complete pre-report path list was:

Modified:

- .env.example
- docs/Desktop/07-meeting-mode.md
- docs/Desktop/08-real-progress.md
- docs/Desktop/ARCHITECTURE.md
- docs/Desktop/AUDIO_AI_PIPELINE.md
- docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md
- docs/Desktop/PRODUCT_SPEC.md
- docs/Desktop/ZOOM_INTEGRATION_SETUP.md
- docs/ai-system/README.md
- docs/ai-system/agent-architecture.md
- docs/ai-system/data-pipeline.md
- docs/ai-system/ethics-governance.md
- docs/ai-system/evaluation-plan.md
- docs/appendices/E-egress-register.md
- docs/architecture/README.md
- docs/plans/2026-08-09-fung-master-implementation-plan.md
- docs/specs/2026-08-11-live-meeting-external-retrieval-design.md
- docs/specs/2026-08-23-speaker-identification-and-voice-profile-spec.md
- scripts/stage_diarization_runtime.ps1
- scripts/stage_whisper_runtime.ps1
- scripts/transcribe.py
- scripts/transcribe_live.py
- src-tauri/src/lib.rs
- src-tauri/src/live_meeting.rs
- src-tauri/src/meeting_intel.rs
- tests/diarizationPackaging.test.mjs
- tests/transcribeConcatOnly.test.py

Deleted (preserved, not hashed):

- AIOS/CORE/CONTEXT_LOADING_RULES.md
- AIOS/CORE/SHARED_CONTEXT.md
- AIOS/WORKFLOW/COMPLEXITY_BASED_WORKFLOW.md

Untracked:

- docs/architecture/MEETING_INTELLIGENCE_DOMAINS.md
- docs/decisions/2026-09-21-google-meet-agent-api-strategy.md
- docs/specs/2026-09-21-fung-meeting-transcript-pipeline-adaptation.md
- docs/specs/2026-09-21-live-meeting-transcription-spec.md
- docs/specs/2026-09-21-meeting-agent-participation-spec.md
- docs/specs/2026-09-21-meeting-knowledge-evidence-spec.md
- docs/specs/2026-09-21-pyannote-diarization-runtime.md
- docs/specs/2026-09-21-speaker-identity-domain-design.md
- docs/specs/2026-09-21-whisper-model-profiles.md
- scripts/diarization-runtime-requirements.txt

Tracked diff summary at the snapshot was 30 files changed, 1072 insertions(+), 276 deletions(-). Git emitted only LF-to-CRLF working-copy warnings for scripts/stage_diarization_runtime.ps1 and scripts/stage_whisper_runtime.ps1.

## Protected dirty product/config/test hashes

These are exact SHA256 digests of existing dirty product, runtime/configuration, and test files. They are a manifest, not a dump of file contents.

| Path | Status | Bytes | SHA256 |
|---|---:|---:|---|
| .env.example | M | 1512 | 2e9e16dbe7fb3e16d560e67e5c31f5287fc1085ee1e1bb18d7bf63911ffc327c |
| scripts/stage_diarization_runtime.ps1 | M | 14915 | 9add03d5d6efdf6ae9a641ed6d56e7d862b431d40e47ec865946d8831ad9e277 |
| scripts/stage_whisper_runtime.ps1 | M | 9110 | 0a32086037d88817bb09f862374145f28852d630f4a259f7833ce87790bb2aaf |
| scripts/transcribe.py | M | 11340 | 46578cae1ef2b3dc7df5cec0f646891b9e4090abf16a79d7fe018ff2876674df |
| scripts/transcribe_live.py | M | 5027 | 46698f796dc726cbf4817d0048be95bb27589fc664bd88314f2840d2e8694eb1 |
| scripts/diarization-runtime-requirements.txt | ?? | 9722 | ec90d85ebe8ea13b3e39d9db9693bbe31638dc3da6a4ce2851e87e4ffa3cc978 |
| src-tauri/src/lib.rs | M | 164157 | 9948c2160da422a406c5cf3f5b719e18f6c8f105abc857e06407dfd3f3f7316f |
| src-tauri/src/live_meeting.rs | M | 107730 | efff8ef8068317d1a4ddf4734a5f597ad9624c96490af3b334468955e1cf3aaf |
| src-tauri/src/meeting_intel.rs | M | 86403 | eaa1a181069697dcb5ad80c6cc7a71548e827ccea7f30c7f110d153c1389d9f5 |
| tests/diarizationPackaging.test.mjs | M | 6632 | a5399509d306ec98ebdef90e295eaec6b0cbf91ad13128c607ecc6849b0e68d8 |
| tests/transcribeConcatOnly.test.py | M | 7009 | 3f9244e579127480a61671823d1eab8c7c2a9faa835523a763da71c2619f39ab |

The three deleted AIOS files have no digest in this baseline because they do not exist in the working tree. No multi-gigabyte model or build cache was recursively hashed.

## Exact input-document manifest

The following relevant entry, architecture, parent, adjacent, and newly written 2026-09-21 feature/domain documents were present at snapshot. Hashes cover the complete existing files at that time.

| Path | Version/status observed | Bytes | SHA256 |
|---|---|---:|---|
| docs/architecture/MEETING_INTELLIGENCE_DOMAINS.md | v0.1.0b, candidate | 14698 | c4ad32aba6b2c26686a298846c553d84d5ada199c80c795c70fa596d3ec23b6f |
| docs/decisions/2026-09-21-google-meet-agent-api-strategy.md | v0.1.0b, candidate | 14042 | 8cdf28b258d5ab61609ab4699f69efd22a18ff394e49f88bdc3409bcbbea06e5 |
| docs/specs/2026-09-21-fung-meeting-transcript-pipeline-adaptation.md | v0.2.2b, beta | 14739 | 616aeeeb8b018c0bde4b25556fdb5d64038687394b36c32e139a0d1d361a29cd |
| docs/specs/2026-09-21-live-meeting-transcription-spec.md | v0.1.0b, candidate | 18105 | 6fb143593d9f6f9b71160efeaa4f8ae95e951f2d84b9e4dd4424e9d9569e754f |
| docs/specs/2026-09-21-meeting-agent-participation-spec.md | v0.1.0b, candidate | 24931 | c62746282920c8b32d393576383279f89d6e63bef9ce7ad2877059abb279261e |
| docs/specs/2026-09-21-meeting-knowledge-evidence-spec.md | v0.1.0b, candidate | 17890 | c98d754c5db068561177164cb0500b842305546e71ce5823be0d2374a311847d |
| docs/specs/2026-09-21-pyannote-diarization-runtime.md | v0.1.0, beta; local runtime approval noted in doc | 7588 | eb02ea0038fc8b67191b955c13aed6e4336ac9aaf36bcb19030bf2a6e174ca58 |
| docs/specs/2026-09-21-speaker-identity-domain-design.md | v0.2.0b, candidate | 71687 | 60ce5789f7ed5e10eefc7d67d41f29aa2549df075c726f383f22c4380a1b3359 |
| docs/specs/2026-09-21-whisper-model-profiles.md | v0.1.0b, beta | 4712 | 2eb3709adcacc6dfe591ae681cef3e95c742e5e40647afa76481ee9fffb94d91 |
| docs/plans/2026-08-09-fung-master-implementation-plan.md | v1.6.0b, need review | 43075 | 7c0081099a915f5e773e1f3cc3bf8d29aa4070050d3d3732a267b2858c94c5bb |
| docs/Desktop/ARCHITECTURE.md | v1.1.0b, candidate | 10548 | a0c6ea25e5537cc20cd0143de8c31fe6f849f4a33d8891880fd426d44ccfc9c7 |
| docs/Mobile/IMPLEMENTATION_STATUS.md | v0.4.3b, beta | 22478 | ddc3877fb0e6b15044876def37de4cec5d56eb59cee0cafdd0ecc37adaeaeca5 |
| docs/Desktop/08-real-progress.md | v0.2.35b, beta | 90105 | 5ec314aaec4d18ef6ec20ffe8973cd3559d4e21a90107c694570abb747295019 |
| docs/Desktop/07-meeting-mode.md | adjacent meeting document | 10699 | 19e9fe322fb399740e0247da07b77bf7984b3456494726a35d2e586c7d056041 |
| docs/Desktop/AUDIO_AI_PIPELINE.md | adjacent pipeline document | 6660 | 8cf7091315d59a0d6248335dd1564db02868c843abc0f725d9f337d7da71eb12 |
| docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md | adjacent external-retrieval document | 19106 | b39713bde25862ead669de443eb6f27a9cd62f98f8cd68891e7d9bdc75bdb73f |
| docs/specs/2026-08-05-zoom-meeting-ingestion-design.md | adjacent provider design | 15079 | c11f519f54f0963e544e9ed04693480f87a1d75f944822cf3081a2db2b428d5a |
| docs/specs/2026-08-23-speaker-identification-and-voice-profile-spec.md | adjacent speaker specification | 27716 | da101a8f2988b6767ed095917489d40c5d9792768e6d15e31c999b9128b5fa64 |
| docs/ai-system/agent-architecture.md | adjacent governance/architecture | 4895 | 96aa92f0af900314bbee8018173955ac933e75897fcba98f128fb3835a4a2c3b |
| docs/ai-system/ethics-governance.md | adjacent governance | 4839 | c46f439ac6af5bbae04cd6d0eb2a647a95b2e29c6eb92d9961cbac844d7f5bde |

The newly written 2026-09-21 domain, decision, pipeline, live transcript, agent participation, knowledge evidence, pyannote, speaker identity, and Whisper profile documents are all included above. They remain candidate/beta documentation and are not implementation evidence.

## Source seams and ownership partitions

| Actual seam | Current evidence | Workflow partition/risk |
|---|---|---|
| src-tauri/src/lib.rs | Dirty shared module registry, AppState, Genesis opening, worker/runtime wiring, and invoke registration. | High-contention shared seam. Workers must coordinate ownership and avoid unrelated edits. |
| src-tauri/src/genesis_adapter.rs | Clean; current schema chain is v1-v10 and is the single Genesis transaction/query boundary. Proposed meeting-session, revision, knowledge, People, agent, and delivery tables are absent. | Any schema work requires a separately approved migration/rollback task. Do not infer implementation from the target domain map. |
| src/tauri.ts | Clean bridge exposes existing live start/stop/status, segment events, recording-scoped ask, summaries, and read-only external MCP commands. | No live-transcript-v2, People, knowledge collection, Meeting Agent, gateway, or outbox bridge exists. |
| src-tauri/src/live_meeting.rs | Dirty; local mic/system capture, persisted audio chunks, persistent worker, and 8-second chunk constant. Labels are capture provenance (เรา/อีกฝ่าย), not person identity. | Do not conflate local capture labels with People/provider identity. Provisional revisions, cursor/replay/recovery, and live provider media are not present. |
| src-tauri/src/meeting_intel.rs | Dirty; existing meeting_ask_recording validates the project/recording pair and queries recording-scoped transcript rows with graph and live-tail excluded. Existing post-meeting queue keeps optional diarization before summary. | Recording-scoped QA must not broaden into whole-project, web, Drive, or live provider retrieval. Preserve optional diarization nonblocking behavior unless a new approved contract says otherwise. |
| src-tauri/src/job_engine.rs | Clean; one serial worker and a closed five-kind set: SummaryGenerate, TranscriptRetry, GraphBuild, SpeakerDiarize, ExportRender. Current jobs require recording input. | No durable live revision DAG, gateway outbox, or new meeting-agent job vocabulary exists. |
| scripts/transcribe.py and scripts/transcribe_live.py | Dirty; one-shot faster-Whisper and persistent JSONL chunk worker. Current progress documents CHUNK_MS=8_000; no provisional revision stream. | Model/runtime workers are a shared seam. No model download or runtime rebuild was authorized. |
| src-tauri/src/diarization.rs and src-tauri/src/local_diarization.rs | Clean; optional local pyannote path is far-side/system-channel and emits anonymous speaker clusters. | People/voice identity is a separate review/consent/qualification concern; diarization is not identity proof. |
| src-tauri/src/external_mcp_commands.rs | Clean; existing external MCP surface is read-only and per-call approved. | Candidate Meet response/publication must not be treated as existing capability. |
| src-tauri/src/mobile.rs and src-tauri/src/fungwire_server.rs | Existing local/mobile or LAN gateways only. | Neither is evidence of a public Google Meet gateway, provider adapter, WSS/HTTPS deployment, or outbox. |

Searches across the bounded source/scripts/tests/contracts scope found no implementation hits for the candidate identifiers meeting_agent, meeting_delivery, knowledge_collections, transcript_revisions, live-transcript-v2, participant_session, or voice_identity. Existing generic gateway/media-fetch/websocket references are not Meet-gateway evidence.

## Feature and provider boundary

The specs are documentation-only candidate contracts except where existing local behavior is explicitly identified. The most important partition evidence is:

- **Approved local pipeline versus candidate live/gateway/outbox:** existing local recording/transcription/optional diarization is not live Meet ingestion, a public gateway, or authorized publication.
- **Recording-scoped QA:** meeting_ask_recording is bounded to a project/recording transcript scope, with graph and live-tail excluded; the knowledge spec explicitly must not broaden this into whole-disk, whole-meeting, Drive-backup, web, or default external search.
- **People versus provider speaker:** provider participant IDs/names and local mic/system provenance are not verified Person records. Speaker clusters, voice profiles, provider participants, and People require distinct consent and review.
- **Gateway versus local LAN/FUNGWIRE:** the strategy document requires any public gateway to be separately qualified for provider/account/region/retention/pricing/privacy and authenticated WSS/HTTPS behavior. Existing local gateway code is not that qualification.
- **Shared Genesis boundary:** the target domain model proposes new aggregates, but the current Genesis schema and command bridge do not contain them.

## Prerequisite gates and approval audit

| Gate or approval | Baseline finding |
|---|---|
| Exact feature approval | Not established for the meeting-intelligence feature set. The 2026-09-21 docs are candidate/beta and the parent plan records documentation only. |
| Workflow authorization | Granted only for this bounded baseline inventory and report write. It does not authorize implementation, provider setup, model download, deployment, or gate approval. |
| M0 provider/privacy/API qualification | Open. Provider, deployment, region, budget, retention, privacy, and actual Meet API capability spike are pending. |
| M1 live transcript | NOT_IMPLEMENTED; current local worker is 8-second chunk/committed-segment behavior. |
| M2 People and knowledge | NOT_IMPLEMENTED; proposed contracts and ACL/evidence/metric work remain unimplemented. |
| M3 Meet observe/draft | NOT_IMPLEMENTED; no Meet join/media/chat adapter is proven. |
| M4 approved same-room output | NOT_IMPLEMENTED; no authorized publication/outbox/receipt path is proven. |
| M5 bounded proactive behavior | NOT_IMPLEMENTED. |
| M6 optional external text/link | NOT_IMPLEMENTED/optional and outside the current evidence. |
| Pyannote local runtime | The spec records user approval for an optional local runtime, but staged model weights, inference quality, identity qualification, and packaged acceptance remain unproven. This does not approve the overall feature or provider path. |
| Whisper profiles | The spec names large-v3-turbo default and medium alternate, but the progress doc says artifacts and acceptance are not staged/proven. No download was attempted. |
| G1/G2 | NOT_ISSUED; this baseline does not issue PASS. |

Exact feature approval must therefore remain a separate decision from approval to run this documentation workflow. A workflow manifest or worker authorization cannot close M0-M6 or authorize external publication.

## Runtime and manifest boundary

.venv-whisper/models/small exists. No recursive model/build-cache hash was taken. .venv-whisper/manifest.json was not present at the checked path; runtime/manifest.json exists as a CUDA runtime manifest, not proof that the requested Whisper model artifacts are staged. No HF token, .env, .env.local, or other secret value was read.

package.json declares the local scripts test:egress, test:diarization, test:transcribe-concat, and build, plus other contract suites. src-tauri/Cargo.toml exists with a pinned Genesis dependency and normal Rust dependencies, but no Meet/Recall provider adapter or gateway dependency was found. No build or Cargo command was run.

## Checks and evidence

| Check | Exit code | Current-run result | Boundary |
|---|---:|---|---|
| git diff --check | 0 | Passed; only LF-to-CRLF warnings for the two staging PowerShell files | Git’s check did not include untracked files. The hash manifest covers untracked input specs/runtime requirements. |
| node --test tests/egressRegister.test.mjs | 0 | 8 passed, 0 failed, 0 skipped; duration 120.4657 ms | Local egress contract only; not Meet/provider/gateway/real-room proof. |

Historical evidence mentioned in the current progress documentation is kept separate and was not rerun: focused Rust 19/19, diarization packaging 8/8, a PowerShell parser check, a prior diff check, and a pyannote import probe. Historical import/probe evidence does not prove model-weight access, inference quality, speaker accuracy, packaged acceptance, or real meeting behavior.

No Cargo/build, model staging/download, provider/API, browser/UI, live meeting, gateway, external message, deployment, or network check was performed.

## Future isolated-worktree seeding guidance

No worktree was created by this baseline. A future authorized worker should seed an isolated worktree from the exact base b336f33ec400a38f003a0665c121069a87a543ac only after carrying this verified artifact manifest:

1. Record branch, HEAD, dirty status, every tracked modified/deleted path, every untracked path, and the protected/input SHA256 values above.
2. Create the isolated worktree from the exact base commit without inventing a commit or mutating the source checkout.
3. Overlay only the approved tracked diffs and the enumerated untracked specs/runtime requirement files from the manifest. Preserve the three AIOS deletions as deletions; do not restore or discard them.
4. Recompute the listed hashes in the isolated worktree and fail closed on any mismatch, missing path, extra path, or changed deletion map.
5. Keep model/build caches outside the manifest unless a later task explicitly scopes a bounded artifact file; never blanket-hash multi-GB caches.
6. Attach the resulting manifest to the worker task and treat any later working-tree change as a new snapshot requiring re-verification.

This is seeding guidance only. No source checkout, worktree, commit, or artifact was created here.

## Open concerns for the parent

- The parent must keep implementation workers from treating candidate docs as approved schemas, commands, or provider capability.
- Shared ownership of lib.rs, genesis_adapter.rs, tauri.ts, and the model worker/runtime needs explicit partitioning before implementation.
- The provider/privacy/API spike and exact feature approval are still open.
- Recording-scoped QA, People/provider speaker separation, and local-versus-gateway/outbox separation are the highest partition risks.
- Runtime/model staging, real-room/provider, packaged, deployment, and publication evidence remain open.
- Dirty-state overlay must be treated as an artifact manifest, not as an invented commit or a clean baseline.

## Authorized output paths

Only these two report paths were authorized for this task:

- docs/verification/implementation-reports/2026-09-21-meeting-workflow-baseline.md
- docs/verification/implementation-reports/2026-09-21-meeting-workflow-baseline.json

## Version Diff

- New 0.1.0b implementation-report baseline, updated to 0.1.1b to record post-write status boundaries.
- Added a frozen pre-report snapshot, exact dirty/input hash manifests, source-seam partition evidence, gate/approval boundary, lightweight checks, and future worktree seeding guidance.
- Recorded the concurrent workflow-plan path observed only after the frozen snapshot and the artifact-level verification results.
- No product, source, test, workflow-plan, or existing specification file was edited by this task.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-21 | beta | Recorded post-write artifact verification and the separate concurrent workflow-plan path without adopting or editing it. | b336f33ec400a38f003a0665c121069a87a543ac | RWANG |
| 0.1.0b | 2026-09-21 | beta | Created the read-only WF-BASELINE handoff for meeting-intelligence workflow planning. | b336f33ec400a38f003a0665c121069a87a543ac | RWANG |
