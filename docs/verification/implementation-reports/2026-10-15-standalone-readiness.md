# FUNG standalone import and transcription readiness — 2026-10-15

## Decision

The pinned `large-v3-turbo` CPU/int8 runtime passed both its model-load probe
and a real production path: loopback audio import, CPU transcription, terminal
job polling, transcript/audio readback, edit, SRT/VTT/WAV export, and orderly
Genesis reopen. The 48 kHz public synthetic English fixture, converted from
the original 16 kHz Windows SAPI WAV with FFmpeg, matched all 12 configured
keywords. The native debug UI completed edit/export and retained the correction
after a normal `CloseMainWindow` close and second boot.

The native restarts exposed a real durability defect: transcript correction
and export were visible before closing, but a clean restart returned the old
segment. FUNG now waits for its managed job worker and live-capture cleanup
coordinator, guards duplicate exit callbacks, and calls the pinned Genesis
`Storage::save_state()` checkpoint once after those writers finish. A required
child-process test verifies segment/proposal/audit readback after explicit
checkpoint and `process::exit` without Rust destructors. A separate ignored
negative-control probe without the final checkpoint observed the pre-edit
baseline; this is diagnostic evidence, not a permanent expectation for future
Genesis versions.

Acceptance is bounded. Native playback returned
`PLAYBACK_OUTPUT_UNSUPPORTED` with “The output device cannot play this
recording rate”; the selected output configuration did not satisfy FUNG's
exact-rate/stereo/sample-format requirements, and no sound started. The error
does not prove sample rate alone caused the rejection. Both native boots had
`localApi.running=false` and no pending jobs; enabled local API requests and
active-job shutdown were not exercised. This does not qualify Thai accuracy,
microphone/device capture, forced-crash recovery, packaged runtime, or release
readiness.

The work is tracked by [FUNG issue #79](https://github.com/Freshair129/FUNG/issues/79).
Existing [issue #41](https://github.com/Freshair129/FUNG/issues/41) concerns a
separate Genesis schema-version startup conflict. The current lockfile pin is
GenesisBlockDB commit `77e07f7b127f65a1f92cfc688ef15f322115d5f8` (v0.2.6); this
work does not upgrade it or close issue #41. All import and native tests use
disposable Genesis copies; forced-crash recovery remains unverified.

## Runtime staging evidence

| Item | Evidence |
|---|---|
| Repository base | `2bf21a06aad9b4be38e567ae6738897fa702e79f` |
| Branch | `codex/readiness-fixes-20261015` |
| Staging command | `scripts/stage_whisper_runtime.ps1 -Model large-v3-turbo -ModelRevision 0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf -HostPython G:\AI-Studio\tools\python\cpython-3.11-windows-x86_64-none\python.exe` |
| Result | Exit 0; portable `WhisperModel` load passed on CPU with `int8` compute |
| Python / faster-whisper | CPython 3.11.9 / faster-whisper 1.2.1 |
| Model | `mobiuslabsgmbh/faster-whisper-large-v3-turbo`, revision `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf`, MIT |
| Runtime | `G:\AI-Studio\development\FUNG\.venv-whisper` (ignored local staging; not a release/package proof) |
| Manifest SHA-256 | `561DA25ED8C64E8F84B7007424CCBA1B63FCB34E9203F9AC2BBE3B3C6873A709` |
| Staging receipt | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\runtime-stage-probe.json` |
| Captured-output log | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\runtime-stage-probe.log`, SHA-256 `C35EC5AE15089E8A57575208E23CC652591AFA0AD74D139CBFA7DF50E445AAF3` |

The saved log was reconstructed from the exact command output returned by the
tool because the original `Tee-Object` target had not persisted. The staging
command was not rerun to create this log. Its captured output reports 4,504
files and 1,923,953,969 bytes for the staged directory. The receipt's
1,923,041,365 `manifestBytes` sum excludes `manifest.json`; the 912,604-byte
difference equals the manifest's file size. At the earlier runtime-staging
probe, its receipt recorded production transcription **NOT_RUN**, GPU inference
**NOT_RUN**, and Ollama **NOT_TOUCHED**. The later route acceptance is recorded
separately below.

Public-safe audio fixture:

- `G:\AI-Studio\verification\2026-10-15-readiness-fixes\fixtures\standalone-audio-16k.wav`
- 14.48 seconds, mono, 16 kHz PCM; SHA-256
  `4A5E0BD7CBBBE5514F7DD8511679F3D22BC3A1E166987EA08F5113B8B1731E77`
- Generated with Windows SAPI; expected utterance is in the adjacent
  `speech-fixture.json`. This is a functional English speech fixture, not
  private user audio or Thai model-quality evidence.

The production route used a separate 48 kHz mono PCM16 fixture so native
playback validation could use the same source rate:

- `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\fixtures\standalone-audio-48k.wav`
- 695,166 frames, 14.482625 seconds; SHA-256
  `493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F`
- Derived from the original 16 kHz Windows SAPI WAV using the recorded FFmpeg
  sample-rate conversion command in `standalone-audio-48k.json`; public-safe, and
  not user audio.

## Frontend verification

The locked frontend dependency install completed with Node 24.19.0 and npm
11.17.0 (`npm ci --no-audit --no-fund`, 76 packages added). The production
frontend build passed. Four focused suites passed 26/26:

| Command | Result | Log |
|---|---:|---|
| `npm run build` | PASS | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\frontend-build.log` |
| `npm run test:recording-output` | 3/3 | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\test-recording-output.log` |
| `npm run test:local-api-client` | 8/8 | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\test-local-api-client.log` |
| `npm run test:web-recordings` | 4/4 | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\test-web-recordings.log` |
| `npm run test:callmd-history` | 11/11 | `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\test-callmd-history.log` |

The exit-code summary is `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\frontend-checks-summary.json`.
Dependency listing and install provenance are recorded in
`G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\npm-installed-tree.log`
and `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\npm-install-receipt.json`;
the receipt identifies the original install output as tool-returned summary,
not a separately persisted raw npm log.
These checks cover frontend build and contracts; they do not invoke native
import, Whisper, or a live desktop app.

## Acceptance implementation

Changed source: `src-tauri/src/local_api.rs`, inside the existing ignored
`upload_transcribes_with_the_real_runtime` test. The test checks the staged
manifest revision and fixture SHA-256, then uses a real ephemeral loopback
HTTP listener to:

1. POST the WAV to `/recordings/import` and poll `/jobs/{job_id}` until a
   terminal result.
2. Read the transcript and require at least 65% of the configured English
   keywords to match. The current matcher is ASCII-only and explicitly rejects
   non-ASCII keyword lists.
3. Fetch `/recordings/{id}/audio?channel=file` and compare the returned bytes
   with the imported fixture.
4. Apply a transcript correction through the production native storage
   function; render SRT/VTT through the production subtitle exporter and WAV
   through the production audio exporter.
5. Drain the worker/listener, reopen the same disposable Genesis store, and
   read back the completed job, edited transcript, recording, export ledger,
   and export files.

The API import, job, transcript, recording-list, and playback checks use the
production loopback HTTP listener. Edit and export call the production storage
and export functions directly because those operations do not have HTTP routes.
This is not UI click-through. Reopen is orderly, not a simulated process crash.

## Exact test contract

The ignored test refuses an existing `FUNG_ACCEPTANCE_ROOT` and requires the
explicit CPU/int8/turbo environment. It ran successfully with the 48 kHz
fixture:

```powershell
& 'G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\run-standalone-acceptance.ps1'
```

The runner verifies fixture SHA-256 and a fresh acceptance root, sets
CPU/int8/turbo, `CARGO_BUILD_JOBS=1`, and a verification-only
`CARGO_TARGET_DIR`, and captures command output and exit code to a log plus
JSON receipt. It sets `TAURI_CONFIG={"identifier":"dev.fung.readiness20261015.route48k","bundle":{"resources":[]}}`
for this non-packaging test pass; the Rust worker loads the explicitly staged
runtime and pinned model. Process-local `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` reduce symbol/link memory without changing the
repository's Cargo profiles.

Run result: `standalone-acceptance-20261015-04.json` is **PASS** and the raw
log `standalone-acceptance-20261015-04.log` reports one test passed, a 50.8
second production transcription, 12/12 expected English keywords, and
successful job/transcript/edit/export/reopen assertions. It confirms exact
audio-byte readback on `/recordings/{id}/audio?channel=file`, edited text in
SRT/VTT, WAV equality, completed export inventory, and orderly reopen of the
same pinned Genesis store. The saved export snapshot at
`standalone-acceptance-20261015-04/route-export-snapshot/manifest.json`
preserves route outputs before native export regenerated the same accepted
file paths. The 12-keyword matcher is ASCII-only; the result is not Thai ASR
quality evidence.

## Native first/second-boot boundary

The native harness uses the disposable native7 profile copied from route04,
with a separate WebView2/temp profile and recording-output root under the
verification directory. It verifies that the app-data path resolves to the
expected Genesis directory and uses only a fresh evidence output folder. It
does not use the ordinary `dev.fung.local` profile, microphone capture,
`live_smoke`, a model job, GPU inference, or Ollama. The route export snapshot
preserves route-produced SRT/VTT/WAV files before the native UI regenerates
those same disposable paths.

The current source rebuilt with `tauri build --debug --no-bundle`, one Cargo
job, debug symbols disabled, and overlay
`{"identifier":"dev.fung.readiness20261015.native7","bundle":{"resources":[]}}`.
This is a non-bundled UI acceptance build, not packaging evidence. Build log
and receipt are `native-build-20261015-07.log` and
`native-build-20261015-07-receipt.json`; the isolated executable SHA-256 is
`3E1C2758104251DAEB02E3D98F76B12D8070E7350AE5310AE648D4F78215E742`.

Run18 is recorded at
`native-first-second-boot-20261015-18/receipt.json` with status
`PASS_WITH_PLAYBACK_RATE_LIMITATION` and eight recorded checks. It opened the
preseeded recording, displayed the transcript, applied the native edit,
completed the export job, and verified SRT/VTT/WAV output. The first app
process was closed with .NET `Process.CloseMainWindow()` against its verified
PID and executable path; PID exit was observed before deleting the WebDriver
session. The second boot reopened the same Genesis profile and showed the
edited transcript and completed exports, then exited through the same normal
window-close sequence. The companion
`native-first-second-boot-20261015-18/tauri-driver.log` records one shutdown
checkpoint at frontier 53 and another at frontier 54. The app log is evidence
the checkpoint path ran; the native receipt is the transcript/export
readback evidence.

Before this fix, native run17 changed and exported a transcript edit, then
closed normally; readback run02 returned the previous segment. Those receipts
preserve the reproduction. The current run confirms the edit survives normal
app close and reopen on the pinned Genesis v0.2.6 path.

The playback UI surfaced code `PLAYBACK_OUTPUT_UNSUPPORTED` and message “The
output device cannot play this recording rate.” No sound started. The message
and status label do not prove rate alone is the cause: the selected output
configuration must meet exact rate, stereo-channel, and sample-format
requirements, and this pass did not capture the host's complete advertised
CPAL configuration list. Both boot health records show `localApi.running=false`
and `pendingJobs=0`. The enabled persistent Local API's in-flight requests and
shutdown during an active job/capture were not exercised. No forced-kill or
power-loss test was performed. The new checkpoint does not close issue #41's
separate schema-version startup conflict and does not qualify crash recovery.

## Current verification matrix

| Check | State | Evidence / boundary |
|---|---|---|
| Portable runtime staging and CPU/int8 model-load probe | **PASS** | `runtime-stage-probe.json`; its saved output log is reconstructed from tool output, as described above |
| Production loopback import and CPU transcription | **PASS** | `standalone-acceptance-20261015-04.json/.log`; real worker, 12/12 English keywords |
| Completed job, transcript/audio readback, edit, SRT/VTT/WAV export, orderly reopen | **PASS** | Same route acceptance; disposable Genesis database and output files |
| Transcript correction and process-exit checkpoint regression | **PASS** | `transcript-correction-tests-20261015.log`, 6 passed / 1 ignored; required child test validates segment, proposal, and audit after `process::exit` |
| No-final-checkpoint negative control | **DIAGNOSTIC PASS** | Ignored/manual probe observed the pre-edit baseline; it is not a contract imposed on future Genesis releases |
| Playback timeline/read bounds and whole-file playback channel | **PASS** | `playback-rounding-cargo-tests-final-20261015.log`, 22/22; `whole-file-channel-cargo-test-final-20261015.log`, 1/1 |
| Frontend and non-bundled native debug build | **PASS** | Native build 07 ran `tsc`, Vite build, and Cargo build; no installer or release bundle |
| Native UI edit/export and normal first/second boot | **PASS_WITH_PLAYBACK_RATE_LIMITATION** | Run18 recorded eight checks and exact normal close/PID exit; no sound started because the selected output configuration was unsupported |
| Enabled Local API in-flight shutdown, active job/capture shutdown | **NOT_RUN** | Native health showed `localApi.running=false`, `pendingJobs=0`; these writer paths were not active in this acceptance |
| Forced-crash recovery, issue #41, Thai accuracy, microphone/device capture, GPU inference, Ollama | **NOT_RUN** | Separate boundaries; no GPU inference or Ollama action occurred |

## Lightweight checks run

- `cargo fmt --manifest-path src-tauri/Cargo.toml` and the final `--check` —
  passed.
- Required transcript correction suite — 6 passed, 1 ignored; focused playback
  module — 22/22; whole-file playback-channel regression — 1/1.
- Ignored no-final-checkpoint diagnostic — observed original `ข้อความเดิม`
  after process exit. The required child test observed its corrected value
  only after the explicit final checkpoint.
- `tauri build --debug --no-bundle` — passed with identifier
  `dev.fung.readiness20261015.native7`, `CARGO_BUILD_JOBS=1`, and the
  non-packaging empty-resource Tauri overlay. A first attempt without the
  overlay stopped in `build.rs` because the ignored
  `.knowledge-parser-runtime` directory was absent; no runtime was staged for
  this packaging-unrelated verification.
- Native UI harness — eight checks passed with a playback limitation; both
  exact owned app PIDs exited after `Process.CloseMainWindow()` and before
  WebDriver session cleanup. See Run18 receipt and `tauri-driver.log`.
- `git diff --check` passed. Final source/artifact provenance is recorded in
  `G:\AI-Studio\verification\2026-10-15-readiness-fixes\FUNG\source-artifact-provenance.json`.
