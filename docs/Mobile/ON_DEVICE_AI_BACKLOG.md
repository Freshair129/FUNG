---
version: "0.1.0b"
created_at: "2026-07-21T04:46:21+07:00,ATHER"
last_update: "2026-07-21T05:09:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "implementation-backlog"
  scope: "FUNG Mobile Android on-device AI runtime"
  language: "Thai"
---

# FUNG Mobile — On-device AI Implementation Backlog

## 1. Backlog Rules

- This backlog implements `ON_DEVICE_AI_RUNTIME_SPEC.md` and follows `ON_DEVICE_AI_ROADMAP.md`.
- A ticket marked `Blocked` requires an explicit decision/evidence; it cannot be bypassed with a mock package or fake model result.
- Every accepted inference result must use the existing Genesis transaction/provenance boundary.
- Estimates are engineering effort ranges for one experienced engineer; they are not delivery commitments.

## 2. Epics and Priority

| Epic | Objective | Priority | Depends on |
| --- | --- | --- | --- |
| E0 | decisions, licenses and fixtures | P0 | approval |
| E1 | native capability broker and diagnostics | P0 | E0 policy baseline |
| E2 | package trust and lifecycle | P0 | E0 license/catalog |
| E3 | Whisper.cpp offline STT | P0 | E1/E2 |
| E4 | ONNX embeddings and semantic retrieval | P1 | E1/E2 |
| E5 | llama.cpp bounded local LLM | P2 | E1/E2 |
| E6 | physical device qualification and release governance | P0 | E3; repeated for E4/E5 |

## 3. E0 — Decision, License and Evaluation Baseline

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-001 | Select candidate Thai STT packages and tokenizer artifacts | P0 | 0.5d | Product/Legal | package identity, Thai claim and license evidence recorded |
| ODAI-002 | Select candidate embedding encoder and target vector dimension | P0 | 0.5d | Technical owner | model/card/license and vector-space contract approved |
| ODAI-003 | Select local LLM candidates and bounded use cases | P1 | 1d | Product/Safety | max input/output, prohibited uses and Thai evaluation criteria recorded |
| ODAI-004 | Create checksum-versioned Thai audio/transcript fixture set | P0 | 1–2d | privacy review | consent/source record, expected transcript and scoring rubric exist |
| ODAI-005 | Define reference-device list for Core/Lite/Standard/Pro | P0 | 0.5d | QA/Product | at least one physical Android device per AI tier chosen |
| ODAI-006 | Approve resource, telemetry and diagnostic redaction policy | P0 | 0.5d | Security/Product | retention, export and no-content logging decisions signed off |

## 4. E1 — Capability Broker and Native Host

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-101 | Define runtime-neutral task, capability and cancellation contracts | P0 | 1d | ODAI-006 | task state machine covers queued/running/deferred/cancelled/failed/completed |
| ODAI-102 | Implement Android native runtime host boundary for arm64 workers | P0 | 2–3d | ODAI-101 | native load/unload and structured error mapping pass on device |
| ODAI-103 | Implement capability broker queue and capture-priority arbiter | P0 | 2d | ODAI-101 | active capture prevents heavy inference; cancellation leaves no accepted partial result |
| ODAI-104 | Implement device inspection and short benchmark record | P0 | 1–2d | ODAI-101 | effective tier derives from OS/ABI/RAM/storage plus benchmark evidence |
| ODAI-105 | Implement thermal, battery and memory gate adapter | P0 | 1–2d | ODAI-103 | moderate/severe/low-battery/memory pressure actions are observable and testable |
| ODAI-106 | Add redacted local runtime diagnostics and feature flags | P0 | 1d | ODAI-104 | no source content/path is exposed; each runtime can be disabled independently |

## 5. E2 — Package Trust and Lifecycle

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-201 | Define signed manifest and catalog verification contract | P0 | 1d | ODAI-001/002 | manifest includes hash, size, license, tier and runtime identity |
| ODAI-202 | Implement app-private staging, hash verification and atomic activation | P0 | 2d | ODAI-201 | corrupt/cancelled artifact is never selectable |
| ODAI-203 | Implement storage reservation, uninstall and stale-artifact cleanup | P0 | 1–2d | ODAI-202 | low-space failure is truthful; uninstall preserves historical provenance |
| ODAI-204 | Persist package/install/benchmark state through Genesis | P0 | 1–2d | ODAI-101 | all package state survives app restart and has audit/provenance |
| ODAI-205 | Build package consent, license summary and unavailable-state UI | P0 | 1–2d | ODAI-201 | user sees local runtime, size, license and eligibility before install/start |
| ODAI-206 | Test tamper, signature failure, revoked package and downgrade paths | P0 | 1d | ODAI-202 | negative suite passes without data loss |

## 6. E3 — Offline Thai STT with Whisper.cpp

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-301 | Integrate Whisper.cpp arm64 CPU worker behind native host | P0 | 2–3d | ODAI-102 | load/run/cancel/unload succeeds on physical AI Lite device |
| ODAI-302 | Implement audio preparation and chunk-to-STT job boundary | P0 | 1–2d | ODAI-103 | immutable source manifest is passed; active capture remains protected |
| ODAI-303 | Import transcript revisions with timing and model provenance | P0 | 2d | ODAI-204/302 | one accepted result uses one Genesis transaction and source remains unchanged |
| ODAI-304 | Add Thai STT Lite package and offline UI flow | P0 | 1–2d | ODAI-201/301 | airplane-mode transcription and named unavailable state work |
| ODAI-305 | Add Thai STT Standard package and quality selection | P0 | 1–2d | ODAI-004/304 | model selection is tier-gated and transcript scoring is recorded |
| ODAI-306 | Add cancellation/retry/Desktop delegation UX | P0 | 1d | ODAI-303 | interruption does not duplicate transcript or hide source access |
| ODAI-307 | Run STT correctness, OOM, low-storage and airplane-mode suite | P0 | 2d | ODAI-304/305 | all required P3 evidence attached to release packet |

## 7. E4 — Local Embeddings and Semantic Retrieval

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-401 | Integrate ONNX Runtime Mobile encoder behind native host | P1 | 2–3d | ODAI-102 | CPU inference and clean unload pass on AI Lite device |
| ODAI-402 | Define embedding input normalization and declared vector space | P1 | 1d | ODAI-002 | vector dimension/model/version and source revision mapping are immutable |
| ODAI-403 | Commit embeddings and provenance through Genesis vector contract | P1 | 2d | ODAI-204/402 | vector result shares source/model-run identity and frontier |
| ODAI-404 | Add semantic retrieval UI with evidence/provenance labels | P1 | 1–2d | ODAI-403 | candidates never become graph facts without user confirmation |
| ODAI-405 | Implement index rebuild, package change and deletion behavior | P1 | 2d | ODAI-403 | stale vectors are visibly unavailable or rebuilt; no orphan source link |
| ODAI-406 | Benchmark 100-note corpus on AI Lite/Standard | P1 | 1d | ODAI-404 | tier time/memory/thermal gates meet specification |

## 8. E5 — Bounded Local LLM with llama.cpp

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-501 | Integrate llama.cpp arm64 worker with streaming/cancel contract | P2 | 2–3d | ODAI-102 | start/cancel/unload has no leak or app crash on AI Standard device |
| ODAI-502 | Implement bounded evidence-grounded prompt contract | P2 | 1–2d | ODAI-003 | input/output/context limits and source citation requirements enforced |
| ODAI-503 | Persist summaries/proposals with inference and evidence provenance | P2 | 2d | ODAI-204/502 | no generated result appears as verified fact |
| ODAI-504 | Add LLM Lite package and AI Standard eligibility UX | P2 | 1–2d | ODAI-201/501 | unsupported device gets clear defer/Desktop action |
| ODAI-505 | Add LLM Standard package for AI Pro only | P2 | 1–2d | ODAI-004/504 | tier gate, token cap and thermal stop behavior enforced |
| ODAI-506 | Run Thai quality, safety, cancellation and low-battery suite | P2 | 2d | ODAI-503/504 | release evidence meets P5 requirements |

## 9. E6 — Qualification, Rollout and Operations

| ID | Backlog item | Priority | Estimate | Dependency | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| ODAI-601 | Build physical-device benchmark harness and evidence packet | P0 | 2d | ODAI-004/005/104 | captures runtime/package/version, thermal, battery, RAM band and checksums |
| ODAI-602 | Run 30-minute STT battery/thermal qualification per enabled tier | P0 | 2d | ODAI-307/601 | V1 gates pass or package is withheld for that tier |
| ODAI-603 | Run embedding and LLM qualification when each is enabled | P1 | 2d each | ODAI-406/506/601 | same evidence and defer rules pass |
| ODAI-604 | Validate no-model Core regression, active-capture arbitration and recovery | P0 | 1–2d | ODAI-103/307 | existing standalone core passes unchanged |
| ODAI-605 | Execute revoke/disable rollback drill | P0 | 1d | ODAI-206/601 | new inference stops, source/provenance remains readable |
| ODAI-606 | Complete security/privacy/license release review | P0 | 1d | E0–E3 evidence | package release authorization recorded |
| ODAI-607 | Produce support matrix, model catalog and user-facing truth copy | P0 | 1d | ODAI-602/606 | docs distinguish Core, supported tier and Desktop-delegated states |

## 10. Backlog Completion Criteria

An epic is complete only when its tickets, stated exit evidence and dependent documentation are complete. “Model loads on one phone” is not completion. The first shippable slice is E0–E3 plus the STT portions of E6; embeddings and local LLM are independent future releases.

## Version Diff

### `0.0.0` → `0.1.0b`

- Added a dependency-ordered implementation backlog for Android STT, embeddings, local LLM and device qualification.
- Made legal, provenance, resource and physical-device evidence explicit blockers rather than post-release cleanup.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-21 | beta | Approved actionable on-device AI backlog; P1 capability work started | pending | ATHER |
