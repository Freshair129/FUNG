---
version: "0.1.0b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "blocked"
superseded_by: null
attributes:
  domain: "desktop-release-acceptance"
  doc_type: "implementation-verification"
  scope: "FUNG Desktop workflow candidate build, CI, installer, isolated launch, and native audio acceptance"
  language: "Thai-first"
---

# Desktop release candidate and native UAT status

## Later remediation

The [capture follow-up](2026-10-23-capture-backpressure-remediation.md) records
the queue-overflow diagnosis, source fixes, and fresh ONIKUMA headless capture
producing three local CPU transcript segments after input-level adjustment.
Hosted frontend and Rust checks on `ee8acfb` both passed. The evidence below
remains the older installed-package snapshot; rebuilt installed acceptance
and hosted checks for the new remediation are still required.

## Outcome

The Desktop workflow candidate builds and installs as an unsigned Windows NSIS
package in an isolated app identity. Source tests, TypeScript/Vite build, Rust
library regression, and Clippy passed locally. The installed app launched with
an isolated app-data profile. Its media output path still pointed to the
standard user `Documents\fung` directory.

This is **not release acceptance**. The candidate has not completed a fresh
system-loopback or application-process-loopback recording followed by
saved-audio review and close/reopen verification. A microphone-only run in the
installed candidate completed and saved chunks, but every PCM sample was zero
and no transcript segment was produced, so the required real Live Meeting gate
failed. The app wrote those chunks under the standard `Documents\fung` media
directory even though its app-data profile was isolated. PR #91 is still Draft and
Hosted CI first failed on stale assertions left behind after the review logic
moved out of `App.tsx`. Those checks now target `RecordingReview`; that review
also exposed a real regression where diarization could be queued before the
runtime was ready. The Review surface now probes readiness and blocks the job
until it is available. The complete 37-command frontend CI sequence,
production build, and pinned MCP functional workflow pass locally on `b3e43ac`.
Hosted frontend CI passes on `b3e43ac` and `1ff6801`; Rust CI at `b3e43ac` was
cancelled when the PR advanced, and Rust CI at `1ff6801` was still running at
last check. The installed runtime manifest and packaged CPU transcription probe
also pass. No merge,
version bump, release tag, or GitHub release was created.

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-23 Asia/Bangkok |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Source/package commit | `dd9134e0c1167eae39f83151aadf05975402af80` |
| Latest candidate code/test commit | `b3e43ac4b5152d17353ec54fd4a19c492fe15f0a` |
| PR head used for the hosted checks recorded here | `1ff68019693b70321ccd0deb93e90acfcc3735b8` |
| Pull request | [#91](https://github.com/Freshair129/FUNG/pull/91), open Draft |
| Candidate app identity | `dev.fung.uat.release20261023` |
| Candidate product/version | `FUNG Release UAT 20261023`, `0.1.1` |
| Installer | `src-tauri/target/release/bundle/nsis/FUNG Release UAT 20261023_0.1.1_x64-setup.exe` |
| Installer size/hash | 1,538,782,433 bytes; SHA-256 `9CB3CB6CC485FC4F67AF22F35672165058BAB7D65099503941E017296A5A3B0B` |
| Signature | Unsigned |
| Existing latest tag | `v0.1.1` still points to its historical commit; it was not moved |

The test correction and this status report are follow-up changes after the
source/package commit above. A release package must be rebuilt from the final
merged `main` commit after the release version is selected.

## Verification

| Check | Result |
| --- | --- |
| `npm run build` | PASS; TypeScript and Vite production build, 1,820 modules |
| `npm run test:ci-coverage` | PASS, 4/4 |
| Focused frontend suites for Live routing, meeting capture/review/contracts/integration, audio visualization/output, and Product MCP | PASS; individual results are recorded in the 2026-10-22 MVP report and current task run |
| `npm run design-system:check` | PASS; 202 tokens, 58 contrast pairs, 25/25 stylesheets |
| `npm run test:design-system` | PASS, 5/5 |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | PASS, 651 passed, 0 failed, 6 ignored |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | PASS |
| Product MCP focused Rust tests | PASS; 9 `product_mcp` and 12 `product_mcp_transport` tests |
| `npm run test:desktop-bootstrap` on initial PR head | FAIL, 10/11; its recording-entry assertion expected the removed `setLiveMeetingOpen`/workspace contract |
| Updated `npm run test:desktop-bootstrap` | PASS, 11/11; verifies that recording entry navigates to the active Live surface |
| `npm run test:job-actions` on pushed follow-up | FAIL, 13/17; four assertions still expected `App.tsx` to own recording scope, transcript correction, and export UI |
| Updated `npm run test:job-actions` | PASS, 17/17; verifies the current `RecordingReview` owner, selected-pair scope, auditable correction path, and persisted artifact display |
| `npm run test:diarization` before readiness repair | FAIL at readiness-gate assertion; the moved review UI could queue without probing runtime/model readiness |
| Updated `npm run test:diarization` | PASS, 8/8 Node checks and 3/3 Python worker checks; unavailable runtime now blocks the UI action and queue path |
| `npm run build` after readiness repair | PASS; TypeScript and Vite production build, 1,820 modules |
| Full frontend CI npm command sequence | PASS, all 37 commands, including all wired suites and `test:ci-coverage`, on `b3e43ac` |
| Pinned workflow MCP acceptance at `b3e43ac` | PASS; CPU `int8` import → transcription → edit → SRT/VTT/WAV export → playback → Genesis close/reopen; 20.52 s total. Transcript keyword score is diagnostic only. |
| MCP evidence | `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-22T183437579Z-148f636a`; fixture SHA-256 `493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F` |
| PR #91 hosted frontend CI at `dd9134e` | FAIL at `test:desktop-bootstrap`; all earlier frontend steps passed |
| PR #91 hosted frontend CI at `129515b` | FAIL at `test:job-actions`; `test:desktop-bootstrap` passed, later suites were skipped |
| Isolated NSIS installation and executable process launch | PASS; installed executable started and created only `dev.fung.uat.release20261023` profile data |
| Installed Whisper runtime manifest | PASS; all 4,504 recorded files match their sizes and SHA-256 values; manifest SHA-256 `561DA25ED8C64E8F84B7007424CCBA1B63FCB34E9203F9AC2BBE3B3C6873A709` |
| Installed package offline CPU transcription probe | PASS; bundled Python 3.11.9 and faster-whisper 1.2.1 ran CPU `int8` with HF offline mode against the packaged model and returned two segments |
| Installer hash recheck | PASS; 1,538,782,433 bytes; SHA-256 `9CB3CB6CC485FC4F67AF22F35672165058BAB7D65099503941E017296A5A3B0B` |
| Visible installed-app UI | PASS; isolated unsigned candidate rendered non-empty Home and Live screens |
| Installed microphone capture | FAIL for release acceptance; run completed at 107,530 ms with 16 durable 48 kHz mono/16-bit chunks (8,842,304 bytes), all PCM samples were zero, and no transcript segments were stored. The Live surface reported a local processing/transcription failure. Chunk hashes matched the database rows and SQLite `integrity_check` returned `ok`. |
| Installed UAT media location | Recorded under `C:\Users\suanr\Documents\fung\projects\e220b14c-36a7-4f04-bff8-158df437f695\live\8bd7d44a-ebc4-4233-8367-8df3cb8a94a4\chunks`; files are preserved. The isolated app profile's `recording-output.json` points to `C:\Users\suanr\Documents\fung`. |
| PR #91 hosted frontend CI at `b3e43ac` | PASS |
| PR #91 hosted Rust CI at `b3e43ac` | Cancelled when the PR head advanced |
| PR #91 hosted frontend CI at `1ff6801` | PASS |
| PR #91 hosted Rust CI at `1ff6801` | In progress at last check before the UAT evidence update was pushed |
| Uninstall/reinstall smoke | NOT RUN |
| Installed system-loopback and application-process-loopback capture | NOT RUN |

The initial Rust full-suite run at unrestricted test concurrency had one
100-ms timeout. The exact test passed in isolation, and the complete library
suite passed on rerun with `--test-threads=4`; the first timeout is retained as
evidence rather than hidden.

## Release gates still open

- Obtain green hosted Rust check on exact PR head `1ff6801` (hosted frontend is
  green).
- Fix or explain why the installed candidate produced all-zero microphone PCM;
  then repeat microphone-only UAT with user speech, a nonzero saved waveform,
  at least one transcript segment, review playback, normal close, and relaunch.
- Point any further isolated-UAT media output to an isolated verification folder;
  the completed zero-signal test files above remain preserved in `Documents\fung`.
- Exercise system loopback and one actively producing application capture on
  the installed Windows package, confirming source isolation and saved output.
- Complete uninstall/reinstall smoke and confirm it does not depend on
  development files. The non-empty app window was observed in the current
  installed candidate.
- Three Dependabot alerts remain for `aiohttp` 3.14.3 in the opt-in
  diarization-runtime lockfile. `tauri.conf.json` does not bundle that optional
  runtime, and the installed candidate's `.venv-whisper` contains no `aiohttp`;
  keep the alerts open as a staging/runtime follow-up and re-check before that
  optional runtime is bundled or enabled by default.
- Merge only after the release acceptance checks are complete. Bump all package
  version declarations and add the release note, then rebuild and verify the
  final package from the exact merged `main` commit before creating a new tag.

Until these gates pass, the candidate remains `blocked`; the correct action is
to retain PR #91 as Draft and not create a tag or GitHub release.
