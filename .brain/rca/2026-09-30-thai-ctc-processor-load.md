# RCA: Thai CTC feasibility processor dependency

Date: 2026-09-30  
Risk: LOW — local qualification script only  
Status: Fixed; verified by the staged 55-clip FUNG worker pilot

## Symptom

The local CTC feasibility run failed while loading the checkpoint processor,
before any audio was aligned.

## Evidence

- `AutoProcessor.from_pretrained` selected `Wav2Vec2ProcessorWithLM` and raised
  an import error because `pyctcdecode` is not installed.
- The pinned checkpoint's `preprocessor_config.json` identifies the feature
  extractor and its `tokenizer_config.json` identifies the tokenizer.
- This feasibility path uses model logits plus CTC Viterbi alignment and does
  not use the checkpoint's language-model decoder.

## Root Cause

The qualification script loaded the checkpoint's full processor wrapper even
though this path needs only its feature extractor and tokenizer. That wrapper
introduced an unused optional `pyctcdecode` dependency.

## Why the issue escaped detection

The script had not yet been run against the downloaded checkpoint when the
loader was written; the first real-model invocation exposed the wrapper's
optional dependency.

## Proposed prevention

Load `AutoFeatureExtractor` and `AutoTokenizer` directly for CTC Viterbi
alignment, and keep the local feasibility runtime limited to required
dependencies.
