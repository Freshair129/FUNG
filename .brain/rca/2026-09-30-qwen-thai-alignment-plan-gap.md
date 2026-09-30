# RCA: Unsupported Thai alignment assumption in the Qwen worker plan

Date: 2026-09-30  
Risk: HIGH — candidate model/runtime and transcript timing path  
Status: structural/quality pilot passed; user review found bt10m/bt3m text misses; timing review pending

## Symptom

The proposed Qwen3-ASR FUNG worker plan selected Qwen3-ForcedAligner-0.6B to
produce timestamps for Thai transcripts, then proposed converting those spans
into FUNG's Detailed-mode review proposals.

## Evidence

- The official [Qwen3-ForcedAligner model card](https://huggingface.co/Qwen/Qwen3-ForcedAligner-0.6B-hf)
  lists 11 supported languages: Chinese, English, Cantonese, French, German,
  Italian, Japanese, Korean, Portuguese, Russian, and Spanish. Thai is absent.
- The LOTUSDIS pilot report records that standalone Qwen3-ASR produced text
  without timestamp segments and that no Thai timestamp/alignment path has yet
  been qualified.
- The [Thai Wav2Vec2 CTC model card](https://huggingface.co/wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm)
  documents `AutoModelForCTC`, Apache-2.0, Thai Common Voice training, and
  PyThaiNLP word-tokenized labels. This makes it a feasibility candidate; the
  card does not claim support for forced alignment of Qwen transcripts.
- FUNG's Detailed-mode contract requires usable source-bounded time spans and
  refuses ambiguous candidate spans before creating review proposals.

## Root Cause

The plan inferred that Qwen's forced aligner covered Thai because the separate
Qwen ASR model recognizes Thai and the aligner accepts transcripts from other
ASR systems. The aligner's published language list does not include Thai, so
ASR language support was incorrectly treated as alignment-language support.

## Why the issue escaped detection

The LOTUSDIS model-selection run measured text WER/CER only. The real-meeting
comparison also produced text-only Qwen output and explicitly left Thai
alignment unresolved. The original worker plan did not make alignment language
coverage a separate pre-implementation gate.

## Proposed prevention

1. Keep Qwen3-ASR text inference independent from timestamp qualification.
2. Evaluate a pinned Thai CTC model as a feasibility candidate, validating its
   vocabulary, Thai token mapping, ordered target coverage, bounded spans, and
   five source-audio spot checks before adding it to Detailed routing.
3. If any feasibility criterion fails, preserve Qwen as text-only benchmark
   evidence and keep Detailed unavailable; do not silently fall back to another
   model.
4. Do not claim timing accuracy without reference timestamps.

## Feasibility mapping note

The pinned Thai CTC vocabulary contains U+0E33 but not U+0E4D. One of the 55
saved Qwen predictions contains the decomposed sequence U+0E4D U+0E32 in a
Thai word. For CTC target construction only, the plan now permits that one
validated sequence to map to U+0E33; emitted candidate text must retain the
original Qwen spelling. All other out-of-vocabulary input remains a hard
alignment failure.

## Local qualification result

The pinned CTC worker aligned all 55 LOTUSDIS clips with bounded, ordered,
nonzero spans and preserved Qwen output text. The FUNG-worker Qwen results
passed the paired LOTUSDIS WER/CER gate on the five microphone strata. User
listening confirms bt10m/bt3m transcription errors and a spoken/reference
variant (`อะไรเงี้ย` versus `อะไรอย่างนี้`); a five-row sensitivity score is
recorded separately without changing LOTUSDIS. This does not measure timestamp
accuracy, and word-level timing review remains open, so Detailed routing stays
disabled.
