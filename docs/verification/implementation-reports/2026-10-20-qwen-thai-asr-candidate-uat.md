---
version: "1.0.0"
created_at: "2026-10-20,Codex"
last_update: "2026-10-20,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio"
  doc_type: "implementation-report"
  scope: "FUNG isolated Qwen3-ASR Thai model staging and microphone candidate pilot"
  language: "Thai-first"
---

# FUNG Qwen3-ASR Thai candidate pilot UAT

## Outcome

**Staging passed; Thai quality and full-recording alignment remain NOT QUALIFIED.**
The pinned Qwen3-ASR and Thai CTC models were staged in an isolated local
runtime and both checkpoint SHA-256 values were verified independently. The
ASR plus CTC pipeline completed on original microphone chunks 2–3, producing
29 aligned segments. On all six original chunks, ASR completed, but the
candidate worker failed closed when the CTC vocabulary could not represent
U+0E46 (Thai repetition mark). An ASR-only pass on all six chunks scored 0.097
against the requested phrase. This is one microphone sample, not a benchmark
or acceptance of Thai transcription.

The candidate remains isolated: Detailed routing is false, Live was not
changed, and no model or runtime was added to the installer. Overall FUNG
Desktop/core acceptance remains NOT PASSED.

## Source and environment

- Repository: G:\AI-Studio\development\FUNG
- Branch: codex/readiness-fixes-20261015
- HEAD: 56c53544718989fd5429dd5071479262fab7c2ee; existing local working-tree
  changes were preserved. No application source or settings were changed for
  this candidate run.
- Stage script: scripts/stage_whisper_qwen_candidate.ps1
- Isolated runtime: G:\AI-Studio\development\FUNG\.venv-whisper-qwen-candidate
- uv 0.12.19 was installed at G:\AI-Studio\tools\uv without a persistent PATH
  update. The staging script installed the pinned Python 3.12.10 runtime and
  hash-locked dependencies. Runtime versions: PyTorch 2.14.0+cu130 and
  Transformers 5.16.1.
- Inference was local and offline with HF_HUB_OFFLINE=1. Device: NVIDIA
  GeForce RTX 3060, torch.float16.
- CPU inference was not run: free system RAM was about 4.04 GiB at preflight,
  below the worker's 10 GiB CPU guard. GPU preflight had about 10.6 GiB free,
  above the 6 GiB guard.
- Candidate manifest: G:\AI-Studio\development\FUNG\.venv-whisper-qwen-candidate\manifest.json
- Sanitized evidence copy: G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\qwen-candidate-uat-20261020

## Pinned model artifacts

| Role | Repository and revision | Bytes | SHA-256 | License |
| --- | --- | ---: | --- | --- |
| Thai ASR | Qwen/Qwen3-ASR-1.7B-hf at bcd2b5b7f32b480ab5790554cfa8347f246a14f3 | 4,076,193,080 | 2db53c7d81bd9b8cbc6a074e89be2c968a0d373fb4ee68bb1b1e14f7042dfee1 | Apache-2.0 |
| Thai CTC feasibility aligner | wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm at 18381b4cfbe8b2e7462827f2c6dea681a31ef5b9 | 1,262,102,632 | f0135130a25f0f16a59cc376f886f9e54e0c1736f1b85c4c01e93b9f4cc4b090 | Apache-2.0 |

Both staged file sizes and SHA-256 values matched the repository-pinned manifest.
The runtime manifest keeps Detailed routing disabled.

## Input and method

The input was the existing user-confirmed ONIKUMA M530 microphone capture,
recording f8663395-7e09-40b4-a3ee-b5510ad14a44: 41,940 ms, mono PCM16 at
48 kHz, split into six original contiguous chunks. The source profile, app
database, and original microphone files were not modified.

The candidate used the repository worker scripts/transcribe_qwen_detailed.py,
Thai language, raw audio preprocessing, local model paths, and GPU profile.
Two runs are recorded:

1. Chunks 2–3 passed both Qwen ASR and Thai CTC alignment. The aligned output
   contained 29 segments, and the two chunks together scored 0.121 character
   similarity. Chunk 2 scored 0.167; chunk 3 scored 0.200.
2. All six chunks completed Qwen ASR, then the full worker stopped at CTC
   target construction with “Thai CTC vocabulary cannot represent U+0E46”.
   No aligned full-capture output was emitted.
3. A separate ASR-only pass completed all six chunks in 35.2 seconds and scored
   0.097 joined similarity. Per-chunk scores were 0.147, 0.167, 0.200, 0.074,
   0.074, and 0.074; the joined hypothesis contained 161 normalized
   characters.

Similarity uses Python SequenceMatcher after removing whitespace, punctuation,
and symbols, taking the best match against the Thai-only and FUNG-included
requested-phrase variants. Transcript text was held in process memory for
scoring only. No raw transcript was displayed, written to a report, or saved
in the evidence receipt. The temporary manifests and derived concatenated WAV
were removed after the runs; the original capture remains untouched.

## Comparison and interpretation

The previous packaged Whisper diagnostics scored 0.065 when the six chunks
were transcribed independently and joined, and 0.110 when the full 41.94-second
capture was concatenated and transcribed as one waveform. For Whisper chunks 2
and 3, the best VAD-threshold results were 0.094 and 0.174. Qwen's six-chunk
ASR-only score is a small improvement over the independently chunked Whisper
score, while remaining below the single-waveform Whisper diagnostic; those
input arrangements differ, so this is not a controlled model benchmark.
Qwen's chunk-level scores are also low. The spoken Thai quality gate remains
FAIL / OPEN.

The full worker's CTC failure is consistent with its fail-closed vocabulary
check and the qualification plan's requirement to reject unsupported alignment
targets. No Unicode substitution or transcript rewrite was applied. The fixed
multi-microphone LOTUSDIS pilot, full CTC feasibility campaign, human audio
review, and Detailed-mode routing acceptance were not performed by this
single-sample UAT.

## Resource and release state

The inference processes exited after each run. Afterward, no FUNG or candidate
Python process remained; GPU use returned to the desktop baseline of 1,446 MiB
used out of 12,288 MiB at 3% utilization. The candidate weights and isolated
runtime remain staged on G for further approved qualification.

No commit, push, tag, release, or application package change was made. The
candidate is not enabled in FUNG Live or Detailed routing. See the
[Qwen worker qualification plan](../../plans/2026-09-30-qwen3-asr-fung-worker-qualification.md)
for the remaining gates and the [Desktop core UAT follow-up](2026-10-19-desktop-core-uat-followup.md)
for the overall release state.

## Supplemental full-waveform and preprocessing sweep — 2026-10-20

After the initial pilot, the same six contiguous ONIKUMA chunks were decoded
and concatenated to one 41,940 ms waveform. The pinned Qwen model ran locally
on the RTX 3060 in float16 with Hugging Face offline mode. I compared the
existing `raw` path with the repository's PyAV `afftdn` and `speechnorm`
preprocessing options. The metric uses the same normalized character
similarity and Thai-only/FUNG-included reference variants as the first pilot.

| Profile | Similarity | Normalized hypothesis characters | Generation time |
| --- | ---: | ---: | ---: |
| `raw` | 0.125 | 90 | 5.89 s |
| `afftdn` | 0.132 | 84 | 3.78 s |
| `speechnorm` | 0.130 | 86 | 3.69 s |

Generation time excludes model loading and preprocessing. The best score is a
small single-sample change and remains inadequate; this does not qualify Thai
recognition or show a controlled model benchmark. No transcript text was
printed, saved, or added to evidence. The sanitized receipt and diagnostic
script are under
`G:\AI-Studio\verification\2026-10-20-fung-pr80-core-uat\artifacts\qwen-candidate-uat-20261020`.
The original chunks and FUNG Live configuration were not modified. Candidate
Thai quality remains **FAIL / OPEN**, and Detailed routing remains disabled.
