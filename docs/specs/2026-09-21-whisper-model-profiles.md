---
version: "0.4.4b"
created_at: "2026-09-21T00:00:00+07:00,RWANG"
last_update: "2026-09-29T23:14:00+07:00,RWANG"
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
Transformers runtime root, for example
`.venv-whisper-transformers-candidate/`, with an explicit candidate profile
such as `thai-large-candidate`. It must contain its own isolated Python
interpreter and pinned candidate dependencies alongside the model/manifest. It
must not be copied into `.venv-whisper/models/`, added to the existing
`stage_whisper_runtime.ps1` model set, mutate the operational Python runtime,
or be selected by default. Candidate batch/live scripts remain separate;
Detailed readiness and batch workers invoke the candidate interpreter
directly. The current live-meeting flow remains General-only because it writes
to the committed transcript stream, while the candidate is restricted to a
post-meeting draft. Candidate live output is not enabled by this specification.
Workers must not rely on `PYTHONPATH`: the production embedded Python ignores
environment-based import paths. They load only a local pinned path with
offline Hugging Face settings and emit the existing `WhisperOutput` contract
before candidate dependency/runtime or quality evidence is promoted.

### 2026-09-22 bounded compatibility check

The repository metadata reports `pytorch_model.bin` at 6,173,655,480 bytes;
the local C: volume had 2,048,868,352 bytes free. The current staged Python
environment has PyTorch and faster-whisper but no `transformers` module.
Therefore the candidate model was **not downloaded or partially staged** in
this run. Existing `turbo`, `medium`, `large-v3` qualification, and default
selection remain unchanged; only the explicit candidate routing and worker
resources were added.

### 2026-09-29 candidate runtime isolation finding

The FUNG worker interpreter reports `isolated=1` and
`ignore_environment=1`; it does not consume `PYTHONPATH`. Its current site
packages contain Torch `2.4.1+cpu` but not Transformers. A temporary
dependency/loading harness imported Torch `2.14.0+cu130` and Transformers
`4.57.1`, but this did not exercise the FUNG worker interpreter. Detailed
readiness and the candidate worker currently resolve the shared operational
Python executable, so model files staged separately are insufficient to make
Detailed mode runnable. The candidate interpreter/dependency runtime, local
model load, and same-clip FUNG worker benchmark remain **NOT_RUN**.

Candidate-runtime packaging is a separate release decision. The model
checkpoint alone is approximately 6.17 GB; the temporary CUDA dependency set
was approximately 3.42 GB. This spec does not authorize adding either artifact
set to the normal application bundle.

The next implementation gate is the candidate Transformers/PyTorch
dependency/runtime qualification plus transactional model staging, a manifest
containing repository, revision, license, selected-file sizes and SHA-256
digests, a local-only processor/model load probe, and a same-clip benchmark
against the existing profiles. Until that gate passes, the Thai result
remains user-supplied benchmark evidence and not FUNG runtime or
production-quality evidence.

### 2026-09-29 candidate runtime implementation

The source implementation now defines a dedicated Python 3.11.9 embedded
runtime, a CPU-only hash-locked dependency set, candidate-specific interpreter
selection for readiness and Detailed batch jobs, and a transactional
runtime/model stage with a manifest for the interpreter, dependency versions,
lock hash and selected model-file digests. Readiness checks the exact
interpreter, manifest, dependency versions, required model files and pinned
checkpoint size. The batch worker loads the local model with offline
Hugging Face settings and low-CPU-memory loading. The candidate runtime remains
excluded from the Tauri bundle. Local staging completed with the pinned
checkpoint and dependency lock, and the staged Python runtime imports the
approved dependency versions. Detailed readiness and the production Rust
worker passed an offline 30-second smoke and returned valid `WhisperOutput`.
The same clip was compared with operational Turbo on CPU; the candidate output
looked more continuous in this clip and was slower. This is one qualitative
sample without a reference transcript, so Thai accuracy remains unqualified.

## User-facing transcription modes

The Desktop exposes two task modes, separate from the machine execution
setting (`FUNG_TRANSCRIPTION_PROFILE`):

| User mode | Model route | When it runs | Transcript authority |
| --- | --- | --- | --- |
| General (`ทั่วไป`) | `large-v3-turbo` / `turbo` | Default for live and ordinary transcription | Existing committed transcript path |
| Detailed (`ละเอียด`) | `biodatlab/whisper-th-large-combined` / `thai-large-candidate` | Explicit post-meeting draft pass | Separate candidate proposals; never writes the committed projection |

The detailed mode is a candidate workflow, not a claim that the model is more
accurate on meetings. Readiness checks confirm the pinned local model files,
manifest and candidate-specific importable dependencies; they do not load the
model or qualify accuracy. The local Rust-worker smoke loaded the model and
emitted valid output, but a single clip without a reference transcript cannot
qualify Thai meeting accuracy. A later model-load failure must still fail
without downloading or falling back to another profile.

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
  Scripts/python.exe
  Lib/site-packages/         # pinned candidate-only dependencies
  models/
    whisper-th-large-combined/
  manifest.json
```

The operational CTranslate2 staging script records the model repository,
resolved revision, license, file sizes and SHA-256 digests in
`.venv-whisper/manifest.json`. The Transformers candidate stage records its
Python archive, dependency lock, package versions and model file digests in
`.venv-whisper-transformers-candidate/manifest.json`. The candidate stage must
not modify the operational runtime.

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
   closed when its candidate interpreter, dependencies or model directory is
   absent.
9. The General user mode explicitly selects `turbo`; it does not inherit a
   stale candidate environment override.
10. Detailed mode uses a candidate-specific isolated Python interpreter and
    pinned dependencies. Readiness imports its required packages through that
    interpreter, does not rely on `PYTHONPATH`, and remains unavailable unless
    the local model and manifest are present. The app never downloads, falls
    back, or edits committed transcript data while producing its draft.
11. Detailed candidate output is reviewable separately, records model
    provenance, and only an accepted proposal creates a human-reviewed
    transcript revision after an expected-revision check.
12. A local-only qualification loads the pinned model through the production
    batch worker, checks its `WhisperOutput` contract and benchmarks the same
    audio spans against the general profile. Without reference transcripts,
    report timing and qualitative output only; do not claim WER/CER or improved
    Thai accuracy.

## Risk and boundaries

Risk: HIGH for candidate-runtime integration because it adds a separately
isolated Python/dependency boundary and affects staging, process selection,
model size and future packaging. The local qualification does not include
application-bundle distribution, change the transcript schema, or change the
GenesisBlockDB boundary.

This change does not claim that `large-v3-turbo` or `large-v3` improves Thai
accuracy until the approved Thai qualification fixture produces comparable
evidence. It also does not make a GPU, VRAM capacity, installer, clean-machine
or production-readiness claim.

## Version diff

| Version | Change |
| --- | --- |
| 0.4.3b → 0.4.4b | Recorded successful offline Rust-worker model load and one same-audio Turbo CPU comparison; Thai accuracy remains unqualified without a reference transcript. |
| 0.4.2b → 0.4.3b | Staged the pinned candidate runtime/model and verified exact dependency imports plus consolidated source checks; production worker model load and same-audio comparison remain pending. |
| 0.4.1b → 0.4.2b | Implemented the isolated Python 3.11.9 candidate runtime, hash-locked CPU dependencies, transactional staging, fail-closed readiness and low-memory offline batch loading; consolidated runtime qualification is pending. |
| 0.4.0b → 0.4.1b | Approved a candidate-specific Python/dependency runtime after confirming the embedded operational interpreter ignores `PYTHONPATH`; added production-worker qualification and packaging boundaries. |
| 0.3.0b → 0.4.0b | Added approved General/Detailed task modes, fail-closed candidate readiness, separate draft/provenance, and human-review commit gate; no Thai accuracy claim. |
| 0.3.0b | Added explicit `thai-large-candidate` routing to separate batch/live Transformers workers and release-resource wiring; retained the unstaged runtime and quality-evidence gates. |
| 0.2.0b | Recorded the Thai Transformers/PyTorch checkpoint compatibility boundary, pinned revision, disk/dependency blocker, and separate opt-in candidate-lane proposal without changing operational profiles. |
| 0.1.0b | Approved two-level operational model profile design with `large-v3-turbo` default, `medium` low-resource path and `large-v3` qualification boundary. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.4.4b | 2026-09-29 | beta | Passed offline candidate Rust-worker smoke and same-audio Turbo comparison; Thai accuracy remains unqualified. | working-tree | RWANG |
| 0.4.3b | 2026-09-29 | beta | Staged the candidate runtime/model and passed source validation; production worker model load and same-audio comparison remain pending. | working-tree | RWANG |
| 0.4.2b | 2026-09-29 | beta | Implemented the isolated candidate runtime and hash-locked CPU dependencies; staging, model-load, same-audio and consolidated test evidence remain pending. | working-tree | RWANG |
| 0.4.1b | 2026-09-29 | beta | Approved an isolated candidate interpreter and recorded the confirmed runtime boundary; implementation and candidate worker qualification remain NOT_RUN. | working-tree | RWANG |
| 0.4.0b | 2026-09-26 | beta | Added General turbo and Detailed Thai-candidate post-meeting draft mode contract; local staging and Thai meeting accuracy remain unqualified. | working-tree | RWANG |
| 0.3.0b | 2026-09-22 | beta | Added opt-in candidate backend routing, worker resources, and transactional staging contract; model download and runtime qualification remain NOT_RUN. | working-tree | RWANG |
| 0.2.0b | 2026-09-22 | beta | Recorded the Thai Transformers candidate compatibility decision and bounded staging blocker; no model artifact or default/profile change. | working-tree | RWANG |
| 0.1.0b | 2026-09-21 | beta | Added the approved Whisper model-profile contract and qualification boundary. | working-tree | RWANG |
