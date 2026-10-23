---
version: "0.1.0b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-audio-capture"
  doc_type: "verification"
  scope: "CPU capture backpressure, post-meeting prerequisites, media isolation, and ONIKUMA diagnosis"
  language: "Thai-first"
---

# Capture remediation and microphone diagnosis

## Revision and diagnosis

Repository `G:\AI-Studio\development\FUNG`, branch
`codex/desktop-workflow-mcp-20261020`, base `ee8acfb2e33c0924d2d70d7928fe022ddc83b4cc`
plus the source/tests committed with this report. PR: [#91](https://github.com/Freshair129/FUNG/pull/91).

Read-only inspection of the isolated installed candidate's Genesis projection
found four `live_meeting.capture_source_gap` events with `sample_queue_overflow`.
The capture job failed for missing media intervals, with zero stream faults.
Additional jobs failed because optional diarization dependencies and a transcript
for summarization were absent. Zero PCM alone did not explain these failures.

The coordinator synchronously loads/decodes while capture threads write to its
bounded event queue. A blocked coordinator fills that queue, blocks WAV writers,
and eventually overflows their realtime sample queues.

## Changes and tradeoffs

- CPU capture drains events and writes source audio first. The decoder starts
  during existing catch-up after Stop, including GPU profiles without staged
  CUDA DLLs. **CPU transcripts now appear after Stop, not live.** The native status
  message says so. Legacy and revisioned catch-up remain in use.
- Independent review found that revisioned replay could commit future pending
  audio before decoding it, or exceed the 64-source batch limit in long recordings.
  Catch-up now uses the current replayed chunk's original source sequence as its
  commit frontier. Later audio remains pending with its original sequence IDs.
- Post-meeting dispatch requires a nonblank transcript for the recording and
  checks optional diarization readiness. An empty transcript emits `skipped`
  rather than queuing guaranteed failures. A usable transcript can summarize
  without the unavailable speaker stage. TypeScript includes `skipped`.
- Fresh alternate app identities default media to `<app-data>/recordings`;
  `dev.fung.local` retains `Documents/fung`. Persisted destinations and old media
  are preserved. Existing alternate profiles are not automatically relocated.
- An opt-in ignored Rust probe uses production device resolution, capture,
  chunking and WAV writing. It requires a fresh absolute evidence directory and
  one explicitly named microphone. It opens no database, desktop UI, or loopback.

GPU decoding with staged CUDA remains synchronous; this change does not establish
that arbitrarily slow GPU inference is free of backpressure. Deferring CPU decode
is a capture-reliability measure, not a CPU live-transcription latency improvement.

## Real microphone evidence

The user authorized headless microphone-only capture, confirmed ONIKUMA M530, and
was prompted to speak during both 20-second runs. Windows input mute was false.

| Measurement | Original input level | Input raised to 85% |
| --- | --- | --- |
| Windows input scalar | 0.27452442 | 0.8500095 |
| Samples / chunks | 962,400 / 3 | 963,360 / 3 |
| Nonzero samples | 257 | 605,693 |
| Peak, signed 16-bit scale | 18 | 7,197 |
| RMS, PCM units | 0.12016 | 342.08202 |
| Capture faults | 0 | 0 |

The first run proves transport but is insufficient for spoken-audio acceptance.
The second has usable signal. The higher input level is retained. Spoken content
and timing were not identical, so gain is not established as the sole cause of
all historical silence.

Preserved local evidence directories:

- `G:\AI-Studio\verification\fung-mic-fix-20261023-ee44819b322f41248ac863788e243d13`
- `G:\AI-Studio\verification\fung-mic-gain85-20261023-d94ba1c8057445cea81f9dfa4afbf0be`

Each contains three WAV chunks and `probe-report.json`. The first also contains
the read/write volume diagnostic `mic-volume.ps1`; the second contains
`transcript.json`. Audio is not added to Git.

The second run's three WAVs were passed together to
`.venv-whisper/Scripts/python.exe scripts/transcribe.py`, with
`--model .venv-whisper/models/large-v3-turbo --language th --profile cpu` and
`HF_HUB_OFFLINE=1`. It returned **3 populated segments**, duration **20,070 ms**.
Recognized speech includes the device name and the change from 27% to 85%.
This proves newly captured audio → local CPU transcription with real output;
verbatim accuracy is not accepted or optimized by this test.

## Verification

| Check | Result |
| --- | --- |
| New regressions before helpers | Failed because guarded initialization, media-root and prerequisite helpers were absent |
| Full Rust library before the review-driven replay-frontier fix | 654 passed, 0 failed, 7 ignored; 201.28 s |
| Final `cargo test --manifest-path src-tauri/Cargo.toml --lib live_meeting::tests:: -- --test-threads=4` | 28 passed, 0 failed, 1 ignored; includes progressive 100-fragment replay with a gap |
| `live_meeting::tests::native_microphone_probe --ignored --nocapture` | Two real-device runs, no capture faults; second has usable speech |
| Local CPU transcription of second run | 3 populated segments, exit 0 |
| `node --test tests/liveCaptureRouting.test.mjs tests/callmdLiveWorkspace.test.mjs tests/recordingOutput.test.mjs tests/diarizationPackaging.test.mjs` | 34 passed |
| `node --test tests/summaryScoping.test.mjs tests/meetingIntelligenceUi.test.mjs` | 19 passed |
| `npm run build` | PASS, TypeScript/Vite, 1,820 modules |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | PASS |
| Hosted frontend and Rust on base `ee8acfb` | Both PASS; precede this remediation |

The new long-recording regression first failed with the old global frontier
(99 instead of 1); it passes with per-chunk replay and verifies no future coverage,
no duplicate committed source sequences, and bounded incremental batches. Final
independent re-review reported no remaining actionable findings. Verification is
composed: the full library run predates this final replay correction, followed by
the focused live suite and Clippy on the correction.

Regressions cover avoiding CPU decoder initialization (including GPU fallback),
GPU initializer error propagation, recording-scoped prerequisite selection, and
alternate media-root persistence across reload. The mic probe is ignored in
ordinary suites because it records a real microphone.

## Release boundary

This is source plus headless real-device evidence. A rebuilt installed package's
Live UI → Stop → persisted transcript → playback → close/relaunch remains open,
as do installed system/app loopback and uninstall/reinstall. Hosted checks must
run on the remediation commit. The prior installed package and its historical
files are preserved. No tag, version bump, merge, or release is created here.
