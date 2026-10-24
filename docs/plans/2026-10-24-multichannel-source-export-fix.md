---
version: "0.1.0b"
created_at: "2026-10-24"
last_update: "2026-10-24,Codex"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-source-audio"
  doc_type: "implementation-plan"
  scope: "FUNG issue 97 multichannel export custody"
---

# Preserve every recorded audio channel in source exports

Complexity: C-2, bounded existing-workflow repair. Risk: high, source custody and durable job outcomes.

Issue: [#97](https://github.com/Freshair129/FUNG/issues/97).
Authority: [D-MVP-05 amendment](../specs/2026-08-26-fung-mvp-critical-path-amendment.md#multichannel-source-export-custody--2026-10-24).

## Scope and constraints

The installed export selected the default microphone channel and omitted populated system audio. The job also converted audio errors into successful subtitle-only completion. Preserve each captured channel without mixing. Thai UI labels, named exports, existing CSS convention, anon-key-only configuration, and surface-param routes ungated remain global constraints. There are no auth/route changes; the UI change only exposes actual artifact basenames.

## Implementation sequence

- [x] Capture RED regressions for omitted system audio, missing second-channel success, compressed timeline, metadata failure and swallowed job error; complete the final regression matrix.
- [x] Reject noncontiguous/malformed live-channel ledger timelines before concatenation; permit ordinary sub-millisecond duration rounding. Bound transcoder writes during mux, not only after completion.
- [x] Reuse canonical channel classification; prepare channels sequentially with a 512 MiB aggregate input/output guard, retaining at most one existing renderer result in memory.
- [x] Reject active recordings before preparation; completed recordings remain eligible while ASR catch-up runs. Render the exact admitted, validated input snapshot, including the same imported path for transcoding, and reject observed source-ledger mutations before publication. Bound PCM accumulation as well as imported reads and transcoder writes.
- [x] Stage all channel files. For multichannel capture, atomically publish an immutable content-hashed directory, then commit all channel-specific artifact rows together. Verify any reused set byte-for-byte; remove only the obsolete ambiguous artifact row for this recording, preserving old physical files.
- [x] Preserve legacy filename and durable ID for a single channel/import. Stage before replacing its file and restore previous bytes on metadata failure.
- [x] Make `export.render` propagate audio preparation/publication errors. Existing subtitle files are not an atomic all-job transaction; the job must explicitly fail if its audio set fails.
- [x] Display the actual artifact basename in recording review so separate WAV artifacts are distinguishable; retain the existing project-scope warning and backend interface. Parent owns this UI patch and reports its 18-test UI history regression and build passed.
- [x] Run focused audio-export/job regressions and source checks; request independent review. Installed-package export and release acceptance belong to the parent UAT run.

## Acceptance evidence

Local source verification: `cargo test --manifest-path src-tauri/Cargo.toml --lib audio_export::tests -- --nocapture` passed 10/10. An earlier matrix run was 8/10 because two new fixtures omitted required Genesis columns; complete fixtures were repaired and all ten reran. Node `jobActions.test.mjs` + `recordingOutput.test.mjs` passed 21/21, including invocation of the Python bounded-mux unit test (3/3; fake codec plumbing, not codec acceptance).

Independent review found a preflight/render race: the old renderer re-read channel inputs while capture or recovery could change them. The deterministic active-capture/new-channel/repaired-path regression failed before the fix and passed afterward (11/11 focused tests). Export now renders only validated snapshot paths and rechecks source-ledger identity before publication. Recording `completed` is an admission boundary, not an assertion that recovery/path repair can never occur; ASR-only updates are excluded from snapshot identity. A completed-recording/running-ASR positive regression and bounded PCM decoder regression also cover this boundary. The Python budget suite is now explicitly registered in `test:recording-output`; the earlier Node invocation count is historical evidence, not the final suite composition.

Final focused snapshot source run: 12/12 audio-export tests passed; `rustfmt --check` for the three affected Rust modules and `git diff --check` passed. One initial positive-test compile attempted `Display` on `AudioError`; the test was corrected to match the existing enum, then all twelve reran. This focused result does not replace the parent's combined Rust/Clippy or installed-package export checks.

The real staged `.venv-whisper` PyAV worker encoded and decoded a small synthetic WAV (`pcm_s16le`, 8,000 frames) and MP3 (`mp3float`, 22,050 frames). An 8-byte real-mux budget failed while preserving previous MP3 bytes and removing the temporary file. External receipt: `G:\AI-Studio\verification\fung-releasefix-uat-20261023\audio-export-codec-995c32ee-5235-4f63-93f1-2026111cbf36\report.json`.

Independent frozen-source re-review accepted the final race fix with no unresolved actionable findings. Combined checks and exact installed-package export remain separate gates owned by the parent delivery run. No installed or release acceptance is claimed here.
