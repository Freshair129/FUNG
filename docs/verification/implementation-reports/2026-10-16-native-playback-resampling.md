---
version: "1.0.0b"
created_at: "2026-10-16T17:45:20+07:00,Codex"
last_update: "2026-10-16,Codex"
status: "local native output smoke passed; GUI and package gates open"
superseded_by: null
attributes:
  domain: "desktop-playback-verification"
  doc_type: "implementation-report"
  scope: "FUNG native playback output negotiation and resampling"
---

# Native Playback Resampling Verification — 2026-10-16

## Finding

The Windows WASAPI default output device on this host is `Speakers (Scarlett Solo
USB)`. Its supported output configurations are stereo at 44.1 kHz. Recording
sources accepted by FUNG range from 8 kHz to 96 kHz, so requiring an exact
source-rate match returned `PLAYBACK_OUTPUT_UNSUPPORTED` for rates the device
does not expose.

## Change

Playback now selects the closest supported stereo rate and sample format,
resamples source PCM with linear interpolation in the output callback, and
keeps `positionMs` on the original recording timeline. The worker resets its
read-ahead cursor from the consumed position when playback pauses, so clearing
the buffer does not skip audio on resume. Seek and pause advance the stream
epoch so stale callback work cannot overwrite the new playback position.

## Evidence

- The default output device reported only 44,100 Hz stereo, including F32 and
  I16 configurations. The temporary device-probe example was removed after the
  check.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib
  desktop_playback::tests::` — 25 passed, 1 ignored.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib
  native_output_resampling_smoke_plays_and_resumes_a_local_fixture -- --ignored
  --nocapture` — 1 passed on the default output device. The production playback
  worker played a generated 500 ms, 16 kHz stereo WAV through the 44.1 kHz
  output, paused, held position, resumed from that position, and reached the
  exact source duration. The temporary fixture is held in an auto-cleaned temp
  directory.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets --
  -D warnings` — passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` — passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — 619 passed, 2
  timing tests failed under parallel load, 6 ignored. The two failing tests
  passed individually when rerun with their exact filters:
  `desktop_playback::tests::one_open_deadline_expires_delayed_readiness_without_a_second_window`
  and
  `external_mcp_transport::tests::stdio_adapter_timeout_and_cancel_are_bounded_and_cleanup_the_child`.

## Limits

This verifies the native CPAL playback worker and its resampling path on the
local output device. It does not verify playback through the FUNG window or
installed package, nor playback of a user recording. Native visual/feature
acceptance, installer clean-install, and remaining API, capture, crash-recovery,
Thai quality, provider, and release gates remain open.
