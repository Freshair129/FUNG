# Thai ASR Model-Selection Qualification

**Version:** 0.1.0  
**Date:** 2026-09-30  
**Status:** Approved for bounded model selection  
**Risk:** LOW — benchmark criteria and evidence only; no runtime, schema, or route change.

## Purpose and boundary

This qualification selects a Thai ASR candidate for follow-up work. It does not qualify a FUNG application route, packaged runtime, or release. The existing Desktop ASR profile acceptance criteria in `2026-09-21-whisper-model-profiles.md` remain in force, including loading the pinned model through the FUNG batch worker and validating its `WhisperOutput` contract.

## Benchmark baseline

- Dataset: the fixed LOTUSDIS paired pilot manifest, 55 clips (11 utterances across five microphones), with the same audio IDs and references for every model.
- Baseline: FUNG `large-v3-turbo`, GPU float16.
- Primary metric: corpus-level WER using LOTUSDIS `scripts/eval.py` text cleaning, PyThaiNLP `newmm`, and Jiwer word alignment.
- Secondary metric: the existing pilot CER procedure (Unicode NFC, remove whitespace, retain punctuation).
- Microphone strata: `con123`, `lav123`, `jbl`, `bt3m`, and `bt10m`.

## Pass criteria

A candidate passes this **pilot model-selection gate** only when all conditions hold:

1. Its prediction file contains exactly one row for each of the 55 reference IDs, with no blank transcript.
2. Its aggregate WER is strictly lower than the Turbo GPU baseline.
3. Its aggregate CER is strictly lower than the Turbo GPU baseline.
4. Its WER and CER are no worse than Turbo GPU in every microphone stratum.

If multiple candidates pass, select the one with lower aggregate WER; use CER as the tie-breaker. Record latency, model revision, backend, device, and output completeness as evidence, but they do not change this accuracy gate.

## Limits

Passing means only that the candidate beat Turbo on this bounded pilot under the recorded scoring methods. The pilot is not the full LOTUSDIS test split and must not be presented as corpus-wide or production accuracy. The selected pilot has already been used for model comparison; reserve an untouched holdout for any final accuracy claim.

The current Qwen tests used the standalone Transformers API. Qwen text-only output has no qualified Thai alignment path, so passing this document does not close the FUNG worker, timestamp/alignment, app, packaging, or release gates. `F:\meeting` remains a qualitative smoke source unless it has a reviewed reference transcript.

## Pilot result — 2026-09-30

The qualification checks were run against the saved predictions and verified for all 55 IDs and 460 reference words.

| Candidate | Result | WER | CER | Notes |
| --- | --- | ---: | ---: | --- |
| Qwen3-ASR 0.6B, GPU float16 | PASS | 53.26% | 44.51% | Lower than Turbo in aggregate and all five microphone strata. |
| Qwen3-ASR 1.7B, GPU float16 | **PASS — selected** | **48.04%** | **40.00%** | Lowest WER and CER among passing candidates; lower than Turbo in all five microphone strata; 55/55 non-blank outputs. Revision `bcd2b5b7f32b480ab5790554cfa8347f246a14f3`. |
| `thai-large-candidate`, CPU | FAIL | 244.35% | 250.08% | 846 insertions; failed aggregate and microphone criteria. |

Turbo GPU baseline: WER **72.83%**, CER **59.55%**. This is a bounded model-selection pass only. The Qwen runs used the standalone Transformers API, so formal FUNG worker/runtime and timestamp/alignment qualification remain open.

Machine-readable decision and checks: `%LOCALAPPDATA%/FUNG/thai-asr-benchmark/lotusdis-th-2026-09/runs/wer-newmm-2026-09-30/qualification.json`.

## Version diff

| Version change | Change |
| --- | --- |
| none → 0.1.0 | Added a paired LOTUSDIS pilot gate for selecting a model relative to Turbo, with corpus and per-microphone WER/CER checks and explicit integration boundaries. |
