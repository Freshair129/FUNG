---
version: "0.4.0b"
created_at: "2026-09-21T00:00:00+07:00,RWANG"
last_update: "2026-09-26T04:12:59+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "technical-design"
  scope: "FUNG Desktop Whisper model profiles"
  language: "Thai"
---

# FUNG Whisper Model Profiles

## Decision

FUNG has two operational transcription profiles:

| Profile | Model | Default execution | Intended machine | Distribution |
| --- | --- | --- | --- | --- |
| `turbo` | `large-v3-turbo` | CUDA `float16` when GPU is selected; CPU `int8` remains an explicit compatibility path | Default quality/speed balance on a CUDA-capable desktop | Bundled when the runtime is staged |
| `medium` | `medium` | CPU `int8`; CUDA `int8_float16` is available for low-VRAM GPUs | Lower VRAM or CPU-only machines | Bundled as the alternate operational model |

`turbo` is the default model profile. The existing execution setting
`FUNG_TRANSCRIPTION_PROFILE` remains `cpu` or `gpu`; model selection is kept
separate so existing device behavior is not silently redefined.

The `large-v3` model is a qualification-only reference. It is never selected
by the normal desktop profile, is not required in the normal bundle, and is
run explicitly by the qualification command against the same approved fixture
and decoding settings as the operational profiles.

## Candidate Thai Transformers lane

`biodatlab/whisper-th-large-combined` is accepted as a candidate Thai model,
not as a third CTranslate2 operational profile. The pinned Hugging Face
revision is `b751db1e8dbfee6561de22ca99fe070282fcf459`. The model card declares
`WhisperForConditionalGeneration` with Transformers/PyTorch and the model
repository contains `pytorch_model.bin`; it is a fine-tune of
`openai/whisper-large-v2` under Apache-2.0. This format is incompatible with
the current `faster-whisper`/CTranslate2 staging contract, which expects a
local CTranslate2 directory containing `model.bin` and its tokenizer files.

The safe integration boundary is a separately identifiable, opt-in
Transformers backend and model/manifest root, for example
`.venv-whisper-transformers-candidate/`, with an explicit candidate profile
such as `thai-large-candidate`. It must not be copied into
`.venv-whisper/models/`, added to the existing `stage_whisper_runtime.ps1`
model set, or selected by default. The candidate batch/live routing is now
wired to separate worker scripts and reuses the selected Python interpreter
only after the candidate dependency preflight passes; the model and manifest
remain separate. The workers must load only a local pinned path with offline
Hugging Face settings and emit the existing `WhisperOutput` contract before
candidate dependency/runtime or quality evidence is promoted.

### 2026-09-22 bounded compatibility check

The repository metadata reports `pytorch_model.bin` at 6,173,655,480 bytes;
the local C: volume had 2,048,868,352 bytes free. The current staged Python
environment has PyTorch and faster-whisper but no `transformers` module.
Therefore the candidate model was **not downloaded or partially staged** in
this run. Existing `turbo`, `medium`, `large-v3` qualification, and default
selection remain unchanged; only the explicit candidate routing and worker
resources were added.

The next implementation gate is the candidate Transformers/PyTorch
dependency/runtime qualification plus transactional model staging, a manifest
containing repository, revision, license, selected-file sizes and SHA-256
digests, a local-only processor/model load probe, and a same-clip benchmark
against the existing profiles. Until that gate passes, the Thai result
remains user-supplied benchmark evidence and not FUNG runtime or
production-quality evidence.

## User-facing transcription modes

The Desktop exposes two task modes, separate from the machine execution
setting (`FUNG_TRANSCRIPTION_PROFILE`):

| User mode | Model route | When it runs | Transcript authority |
| --- | --- | --- | --- |
| General (`ทั่วไป`) | `large-v3-turbo` / `turbo` | Default for live and ordinary transcription | Existing committed transcript path |
| Detailed (`ละเอียด`) | `biodatlab/whisper-th-large-combined` / `thai-large-candidate` | Explicit post-meeting draft pass | Separate candidate proposals; never writes the committed projection |

The detailed mode is a candidate workflow, not a claim that the model is more
accurate on meetings. The earlier external comparison had no reference
transcript and did not run through FUNG's Transformers worker, so it cannot
qualify Thai meeting accuracy. Readiness checks confirm the pinned local model
files, manifest and importable dependencies; they do not load the model or
qualify accuracy. A ready candidate can still fail during model load, and
that job must fail without downloading or falling back to another profile.

Each detailed run records its model/backend/revision and produces separate
proposals against the current transcript. Proposal review is scoped to the
same recording and expected transcript revision. Accepting a proposal creates
a human-reviewed transcript revision; rejecting it leaves the committed
transcript unchanged. A stale candidate must fail closed and cannot overwrite
a correction or a newer ASR result.

## Configuration contract

| Variable | Values | Default | Meaning |
| --- | --- | --- | --- |
| `FUNG_WHISPER_MODEL_PROFILE` | `turbo`, `medium`; `thai-large-candidate` (candidate only) | `turbo` | Selects an operational model directory or the separate opt-in Transformers candidate |
| `FUNG_TRANSCRIPTION_PROFILE` | `cpu`, `gpu` | `cpu` | Selects the execution device and CUDA DLL path |
| `FUNG_TRANSCRIPTION_COMPUTE_TYPE` | faster-whisper compute type | profile-derived | Optional override; `int8_float16` is the low-VRAM CUDA setting |

`large-v3` may be passed directly to the worker for qualification only. The
qualification run must use a local pinned model path and must not rely on an
implicit Hugging Face download.

## Runtime layout and provenance

The staged runtime uses one canonical directory per model:

```text
.venv-whisper/
  models/
    large-v3-turbo/
    medium/
    large-v3/             # optional qualification-only staging

.venv-whisper-transformers-candidate/
  models/
    whisper-th-large-combined/
  manifest.json
```

The staging script records the model repository, resolved revision, license,
file sizes and SHA-256 digests in `.venv-whisper/manifest.json`. Model staging
must preserve an existing runtime so `turbo` and `medium` can be staged by
separate invocations without duplicating Python dependencies.

The accepted CTranslate2 repository for `large-v3-turbo` and the exact
revision for every model must be recorded in the manifest before a runtime
can be called packaged evidence. `large-v3` reference evidence remains
separate from release readiness.

## Acceptance criteria

1. A fresh worker defaults to `large-v3-turbo`/`turbo` and resolves the
   matching local model path without network access.
2. Selecting `medium` resolves only the `medium` model directory and uses
   CPU `int8` by default.
3. Selecting `medium` with GPU execution can use `int8_float16` without
   changing the CUDA DLL validation boundary.
4. Missing model directories fail closed with an actionable error; no worker
   silently downloads a model during a local transcription job.
5. The staging script can add a model without deleting the existing Python
   runtime or other staged model directories.
6. Unit tests cover profile validation, model-path resolution and compute-type
   selection. Worker tests cover the default and `medium` arguments with a
   fake faster-whisper module.
7. GPU smoke evidence is recorded for `large-v3-turbo`; CPU `int8` evidence is
   recorded for `medium`; `large-v3` is reported only after an explicit
   qualification run.
8. `thai-large-candidate` routes only to the Transformers workers and fails
   closed when its separate runtime/model directory is absent.
9. The General user mode explicitly selects `turbo`; it does not inherit a
   stale candidate environment override.
10. Detailed mode is unavailable unless the pinned local model, manifest and
    required dependencies are present; its readiness result makes no model-load
    or accuracy claim, and it never downloads, falls back, or edits committed
    transcript data while producing its draft.
11. Detailed candidate output is reviewable separately, records model
    provenance, and only an accepted proposal creates a human-reviewed
    transcript revision after an expected-revision check.

## Risk and boundaries

Risk: MEDIUM. The change affects bundled model size, CUDA/CPU resource use,
packaging and qualification evidence, but does not change the transcript
schema or the GenesisBlockDB boundary.

This change does not claim that `large-v3-turbo` or `large-v3` improves Thai
accuracy until the approved Thai qualification fixture produces comparable
evidence. It also does not make a GPU, VRAM capacity, installer, clean-machine
or production-readiness claim.

## Version diff

| Version | Change |
| --- | --- |
| 0.3.0b → 0.4.0b | Added approved General/Detailed task modes, fail-closed candidate readiness, separate draft/provenance, and human-review commit gate; no Thai accuracy claim. |
| 0.3.0b | Added explicit `thai-large-candidate` routing to separate batch/live Transformers workers and release-resource wiring; retained the unstaged runtime and quality-evidence gates. |
| 0.2.0b | Recorded the Thai Transformers/PyTorch checkpoint compatibility boundary, pinned revision, disk/dependency blocker, and separate opt-in candidate-lane proposal without changing operational profiles. |
| 0.1.0b | Approved two-level operational model profile design with `large-v3-turbo` default, `medium` low-resource path and `large-v3` qualification boundary. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.4.0b | 2026-09-26 | beta | Added General turbo and Detailed Thai-candidate post-meeting draft mode contract; local staging and Thai meeting accuracy remain unqualified. | working-tree | RWANG |
| 0.3.0b | 2026-09-22 | beta | Added opt-in candidate backend routing, worker resources, and transactional staging contract; model download and runtime qualification remain NOT_RUN. | working-tree | RWANG |
| 0.2.0b | 2026-09-22 | beta | Recorded the Thai Transformers candidate compatibility decision and bounded staging blocker; no model artifact or default/profile change. | working-tree | RWANG |
| 0.1.0b | 2026-09-21 | beta | Added the approved Whisper model-profile contract and qualification boundary. | working-tree | RWANG |
