# Qwen Thai candidate qualification — 2026-09-30

## Decision

The local LOTUSDIS worker and comparative text-quality gates pass for the
pinned candidate. User listening feedback found Qwen transcription errors on
the bt10m and bt3m samples; word-level timing placement has not been approved.
An approved low-clarity experiment tested two candidate-only PyAV filters, but
neither met the correction gate, so raw remains the comparison baseline and the
Detailed route remains disabled. This is a bounded 55-clip pilot result, not a
corpus-wide, packaged, or production qualification.

Risk is **HIGH** for model/runtime and transcript-timing changes. The work is
local-only: General `turbo`/`medium`, live transcription, Genesis schema,
providers, and installer resources remain unchanged.

## Pinned runtime and artifacts

| Component | Value |
| --- | --- |
| Python | 3.12.10 embedded; archive SHA-256 `4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3` |
| Transformers / PyTorch | 5.16.1 / 2.14.0+cu130 |
| Execution | NVIDIA GeForce RTX 5060 Ti, `torch.float16`, CUDA 13.0 |
| Dependency lock | `scripts/qwen-candidate-requirements.txt`; SHA-256 `719504fc1f81edeb16a95c8640736ddab08e8885b30e850557471ffd853a72dc` |
| ASR | `Qwen/Qwen3-ASR-1.7B-hf`, revision `bcd2b5b7f32b480ab5790554cfa8347f246a14f3`, Apache-2.0; checkpoint 4,076,193,080 bytes, SHA-256 `2db53c7d81bd9b8cbc6a074e89be2c968a0d373fb4ee68bb1b1e14f7042dfee1` |
| Thai aligner | `wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm`, revision `18381b4cfbe8b2e7462827f2c6dea681a31ef5b9`, Apache-2.0; checkpoint 1,262,102,632 bytes, SHA-256 `f0135130a25f0f16a59cc376f886f9e54e0c1736f1b85c4c01e93b9f4cc4b090` |

Both model files were staged and hash-verified by
`scripts/stage_whisper_qwen_candidate.ps1`. The worker loaded both models from
the isolated candidate runtime with Hugging Face offline mode enabled.

## LOTUSDIS worker result

The fixture has 55 unique reference IDs across five microphones and 108.45
seconds of 16 kHz audio. All 55 WAV files matched the SHA-256 values in the
LOTUSDIS manifest. Turbo comparison used the same IDs and the existing FUNG
`large-v3-turbo` GPU worker output (`NVIDIA GeForce RTX 5060 Ti`, float16).

The Rust integration test
`staged_qwen_candidate_lotusdis_pilot_when_configured` ran the FUNG Qwen
worker and its Rust `WhisperOutput`/provenance validator on GPU. It completed
in 40.05 seconds for the 108.45 audio seconds (0.369 real-time factor),
returned 55/55 unique IDs, 0 blank outputs and 364 aligned segments, and
preserved source-bounded, ordered, nonzero spans. Qwen's text matched the
standalone pinned Qwen output exactly on all 55 clips. The test concatenated
the independent clips on a synthetic timeline with 100 ms separators; its
`durationMs` includes those separators and is not the fixture's audio duration.

WER uses the LOTUSDIS reference evaluator's `clean_text`, PyThaiNLP `newmm`,
and Jiwer `process_words`/`wer_default` (`jiwer==4.0.0`,
`pythainlp==5.3.8`). CER uses the approved pilot normalization: Unicode NFC,
remove all whitespace, retain punctuation.

| Microphone | Qwen WER | Turbo GPU WER | Qwen CER | Turbo GPU CER |
| --- | ---: | ---: | ---: | ---: |
| bt10m | 73.91% | 97.83% | 64.51% | 87.32% |
| bt3m | 64.13% | 93.48% | 54.08% | 83.94% |
| con123 | 34.78% | 48.91% | 28.17% | 38.03% |
| jbl | 35.87% | 72.83% | 28.17% | 51.55% |
| lav123 | 31.52% | 51.09% | 25.07% | 36.90% |
| **Overall** | **48.04%** | **72.83%** | **40.00%** | **59.55%** |

WER and CER are both lower overall, and neither metric is worse for Qwen in
any microphone stratum. The paired CER comparison has 44 clips better for
Qwen, 2 better for Turbo and 9 ties. The machine-readable result is
`metrics.json` in the local run directory below.

## Human listening feedback and reference sensitivity

The user confirmed that the five microphone spot-check recordings carry the
same utterance and heard it as `ทาครีมแล้วก็ไปเรียนเลยอะไรเงี้ย`. The current
LOTUSDIS reference for all five says `ทาครีมแล้วก็ไปเรียนเลยอะไรอย่างนี้`.
The user identified the Qwen bt10m and bt3m transcripts as wrong, attributing
the misses to unclear audio. The other Qwen spot-check texts are `ทาครีมแล้วก็
ไปเรียนเลยอะไรเงี้ย` for con123 and lav123, and `ทาครีมแล้วก็ไปเรียนเลยอะไร
เนี้ย` for jbl. The user did not provide a word-level timing verdict. The
official LOTUSDIS reference file was left unchanged.

A sensitivity-only score used the existing `score_lotusdis.py` normalization,
PyThaiNLP `newmm` tokenizer and Jiwer calculation, replacing the reference in
memory for only these five duplicate-utterance rows with the phrase the user
heard. It is not the official LOTUSDIS score and does not relabel the corpus:

| Scope and reference | Qwen WER / CER | Turbo GPU WER / CER |
| --- | ---: | ---: |
| Five clips, official LOTUSDIS reference | 27.50% / 24.12% | 52.50% / 43.53% |
| Five clips, user-heard phrase | 22.50% / 16.13% | 57.50% / 49.68% |
| All 55, official LOTUSDIS references | 48.04% / 40.00% | 72.83% / 59.55% |
| All 55, only five spot-check references changed for sensitivity | 47.61% / 39.43% | 73.26% / 60.23% |

The main reported result remains scored against the untouched LOTUSDIS
manifest. The sensitivity calculation shows the impact of this one human
listening correction; it is not a new baseline or an independent annotation
pass.

## Low-clarity preprocessing experiment — 2026-10-01

The pinned Qwen worker ran on the same five mic clips with `raw`, `afftdn`, and
`speechnorm`, using PyAV 18.1.0, the same Qwen/CTC model revisions, and NVIDIA
GeForce RTX 5060 Ti. `raw` reproduced the five prior candidate texts exactly. All five
source WAV SHA-256 hashes were unchanged. Each profile produced nonblank text
and ordered, source-bounded CTC spans; this does not approve word-level timing.

The official LOTUSDIS reference remained `ทาครีมแล้วก็ไปเรียนเลยอะไรอย่างนี้`.
The user-heard phrase `ทาครีมแล้วก็ไปเรียนเลยอะไรเงี้ย` was used only for the
five-row sensitivity calculation.

| Profile | Official WER / CER | User-heard sensitivity WER / CER | BT3m | BT10m |
| --- | ---: | ---: | --- | --- |
| `raw` | 27.50% / 24.12% | 22.50% / 16.13% | `ทันทีแล้วก็ไปเรียนเลย` | `พาทีแล้วก็ไปเรียนเลยอะไรอย่างเงา` |
| `afftdn` | 32.50% / 32.94% | 27.50% / 21.29% | `ผ่านชีพแล้วก็ไปเรียนเลย` | `หาซีเขาก็ไปเรียนเลย` |
| `speechnorm` | 27.50% / 24.12% | 22.50% / 16.13% | `ทันทีแล้วก็ไปเรียนเลย` | `ภาคีแล้วก็ไปเรียนเลยอะไรอย่างเงา` |

`afftdn` worsened both five-clip scores and both target transcripts.
`speechnorm` tied the raw aggregate scores but left both target errors in
place. Neither mode met the approved acceptance gate, so the 55-clip Rust
worker filter pilot was **NOT_RUN** and no transform was selected. The app's
Detailed route remains disabled. The machine-readable output and per-profile
logs are under:

`C:\Users\pc\AppData\Local\FUNG\thai-asr-benchmark\lotusdis-th-2026-09\runs\qwen-low-clarity-remediation-20261001`

## Remaining timing and route gate

LOTUSDIS contains no reference timestamps, so this run does not calculate
timestamp error. Structural alignment checks passed for the five-profile runs.
The listening feedback confirms two recognition misses but does not verify CTC
word-level timing placement, so the human alignment gate remains open.

The five exact worker outputs, clip-relative CTC spans, reference text and
source WAV paths are listed in:

`C:\Users\pc\AppData\Local\FUNG\thai-asr-benchmark\lotusdis-th-2026-09\runs\fung-qwen-worker-qualification-2026-09-30\spotcheck\review-manifest.json`

The local candidate manifest records `ctcFeasibility=passed` and
`pilotQuality=passed`; `microphoneSpotChecks=partial-human-review` and
`detailedRouting=false`. The app reports the remaining gates and refuses the
Detailed worker until timing review passes and a qualifying remediation is
accepted. The filter experiment did not qualify either tested transform.
No candidate text was written to committed transcript rows.

The FUNG worker, raw predictions, metrics, scorer and hashes are local
qualification artifacts under:

`C:\Users\pc\AppData\Local\FUNG\thai-asr-benchmark\lotusdis-th-2026-09\runs\fung-qwen-worker-qualification-2026-09-30`

## Verification

| Check | Result |
| --- | --- |
| Rust library suite | 596 passed, 0 failed, 2 ignored |
| Baseline staged Rust Qwen worker pilot (raw, 2026-09-30) | 1 passed: all 55 clips; this predates preprocessing-profile plumbing |
| Qwen worker contract tests | 9 passed |
| Thai CTC alignment tests | 9 passed |
| PyAV preprocessing integration | 1 passed: raw/afftdn/speechnorm repeatability, finite output, source/sample-count preservation |
| Five-clip profile comparison | 3 profiles, 5/5 outputs each; raw reproduced baseline; no filter qualified |
| Filtered 55-clip Rust worker pilot | NOT_RUN: no filter passed the five-clip gate |
| Five-clip human-reference sensitivity scoring | Completed for analysis only; official LOTUSDIS labels unchanged |
| Desktop production build | `npm run build` passed |
| Rust formatting / whitespace | `cargo fmt --check` and `git diff --check` passed |

For source-tree Rust tests, the checkout lacked the ignored operational
`.venv-whisper` and `.knowledge-parser-runtime` resource directories. Minimal
ignored sentinels were used only to satisfy Tauri's resource-path check; they
are not runtime contents and this does not qualify packaging or app launch.
The full Rust suite passed with those sentinels present.

The remaining route gates are word-level timing review and a qualifying
remediation for the bt10m/bt3m recognition misses. This experiment found no
qualifying built-in filter. Full LOTUSDIS evaluation, an untouched holdout,
long-recording soak, native-device acceptance, installer packaging and release
qualification remain out of this bounded pilot.

## Update record — 2026-10-01

| Artifact | Change |
| --- | --- |
| Qwen qualification | Added candidate-only `raw`/`afftdn`/`speechnorm` provenance and the negative five-clip result; no filter selected, no all-55 filtered run, and Detailed remains disabled. |
