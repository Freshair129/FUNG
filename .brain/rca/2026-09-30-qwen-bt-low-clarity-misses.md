# RCA: Qwen lexical misses on Bluetooth microphone samples

Date: 2026-09-30
Risk: HIGH — local Detailed ASR input and transcript quality
Status: two sample misses confirmed by user; acoustic cause not isolated; approved filter experiment completed with no qualifying transform

## Symptom

The pinned Qwen Detailed worker mis-transcribed two of five microphone
recordings of the same short utterance. The user's listening correction is
`ทาครีมแล้วก็ไปเรียนเลยอะไรเงี้ย`. Qwen returned `พาทีแล้วก็ไปเรียนเลยอะไรอย่างเงา`
for `Hijack_S010_T038_BT10m_chunk220` and `ทันทีแล้วก็ไปเรียนเลย` for
`Hijack_S010_T038_BT3m_chunk220`.

## Evidence

- The user listened to the five microphone examples, confirmed that they carry
  the same utterance, and identified the bt10m and bt3m Qwen outputs as wrong
  because the audio was unclear. This is perceptual evidence from the user;
  no independent intelligibility score has been produced.
- The official LOTUSDIS reference for all five examples is
  `ทาครีมแล้วก็ไปเรียนเลยอะไรอย่างนี้`. The official manifest remains
  unchanged. A separate sensitivity score using the user's phrase for only
  those five IDs is recorded in
  `docs/verification/implementation-reports/2026-09-30-qwen-thai-candidate.md`.
- The full 55-clip comparison passed its approved **relative** criterion:
  aggregate WER/CER were lower than Turbo GPU and neither metric was worse in
  any microphone group. The criterion has no absolute WER/CER ceiling and no
  per-clip human text-accuracy requirement.
- `scripts/transcribe_qwen_detailed.py` decodes each source to 16 kHz mono,
  sends that unconditioned signal directly to Qwen, then passes Qwen's text to
  the Thai CTC alignment stage. The aligner creates spans for supplied text;
  it cannot correct a lexical ASR error.
- The pinned runtime already includes PyAV 18.1.0. Its `afftdn` and
  `speechnorm` filters are available locally, but the Qwen worker does not use
  them and no noise-reduction dependency was added.
- The two WAVs are both 16 kHz mono and 2.13 seconds. The sample-level check
  found at most one full-scale sample per clip, so gross digital clipping is
  not established. No validated SNR or speech-intelligibility measure has
  isolated the acoustic cause.

## Root Cause

The **confirmed qualification-process cause** is that the accepted gate
compared Qwen with Turbo by aggregate and microphone-stratum WER/CER only. It
did not require a per-clip transcript check or an absolute quality floor, so
the candidate passed while two reviewed clips contained material text errors.

The **specific acoustic cause** of the bt10m/bt3m errors is not yet confirmed.
The user reports low clarity, but the available measurements do not distinguish
Bluetooth codec loss, microphone placement, background noise, or another cause.
The current pipeline also has no candidate-side enhancement stage.

## Why the issue escaped detection

The first qualification campaign evaluated all 55 clips numerically against
the official LOTUSDIS manifest and compared only with Turbo. The five-clip
listening review happened after that gate passed. The official reference also
uses `อะไรอย่างนี้`, while the user hears the colloquial variant `อะไรเงี้ย`;
the difference was not separately recorded before scoring.

## Proposed Prevention

- Keep Detailed routing disabled until the two low-clarity samples are
  remediated and the five source-timing samples receive qualitative review.
- Preserve official LOTUSDIS labels. Keep user-heard variants as a separate,
  clearly marked sensitivity/adjudication record.
- Run the bounded candidate-only experiment in
  `docs/plans/2026-09-30-qwen-low-clarity-remediation.md` before changing the
  worker. Never overwrite source audio or apply the candidate transform to
  General/live transcription.
- Add a human text/timing gate alongside aggregate WER/CER so a relative win
  against a weak baseline cannot independently enable Detailed routing.
- The 2026-10-01 `afftdn`/`speechnorm` experiment did not qualify a transform:
  `afftdn` worsened the five-clip score and `speechnorm` left both reviewed
  misses unresolved. Keep raw Qwen as benchmark evidence and Detailed routing
  disabled; see the [qualification report](../../docs/verification/implementation-reports/2026-09-30-qwen-thai-candidate.md).
