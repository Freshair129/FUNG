---
version: "0.1.0b"
created_at: "2026-09-21T00:00:00+07:00,RWANG"
last_update: "2026-09-21T00:00:00+07:00,RWANG"
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

## Configuration contract

| Variable | Values | Default | Meaning |
| --- | --- | --- | --- |
| `FUNG_WHISPER_MODEL_PROFILE` | `turbo`, `medium` | `turbo` | Selects the bundled operational model directory |
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
| 0.1.0b | Approved two-level operational model profile design with `large-v3-turbo` default, `medium` low-resource path and `large-v3` qualification boundary. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | beta | Added the approved Whisper model-profile contract and qualification boundary. | working-tree | RWANG |
