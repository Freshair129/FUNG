---
version: "0.1.1b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-release-acceptance"
  doc_type: "verification"
  scope: "FUNG 0.1.2 rebuilt installed-package acceptance and release gates"
  language: "Thai-first"
---

# Rebuilt Desktop package acceptance

## Source preparation

Repository `G:\AI-Studio\development\FUNG`, branch
`codex/desktop-workflow-mcp-20261020`, starting revision `6e6067c26c91bbd1a880ab259fc1021fd4eff56a`.
The source changes accompanying this record fix two findings from independent
whole-PR review: preserve native application packet positions/discontinuities and
persist captured tails when packet reads fail. Quiet event timeouts keep polling.
Re-review found no additional actionable findings in the remediation.

Three regressions exercise packet position normalization and invalid timing,
actual WAV persistence on a packet failure, and a native-position jump producing
separate pre/post-gap chunks plus exact `application_loopback_discontinuity` evidence.
They were added before the missing helper implementations; focused tests and the
full library passed after implementation.

The package version is prepared as `0.1.2` across package manifests/locks, Cargo,
Tauri and frontend release metadata. Approximate public download size is corrected
to 1.5 GB. This version preparation does not create or publish a release.

## Source checks

| Check | Result |
| --- | --- |
| Full Rust library with application-capture corrections | 658 passed, 0 failed, 7 ignored; 197.83 s; compiled at 0.1.1 before metadata-only version preparation |
| Clippy, all library targets, warnings denied | PASS at 0.1.2 |
| Release distribution metadata checks | 11 passed at 0.1.2 |
| Live workspace, routing and recording-output Node checks | 26 passed |
| TypeScript/Vite production build | PASS at 0.1.2 |
| Hosted CI on starting revision `6e6067c` | Frontend and Rust PASS; superseded by the new packet-handling changes |

## Installed acceptance in progress

Source/package revision: `5a090f65a48513461ed49db572fb08fa6f271464`.
Installer `FUNG Capture UAT 20261023_0.1.2_x64-setup.exe`: **2,115,837,852 bytes**,
SHA-256 `CFE113D0E9554CCA49AF5A4EE3A49E62C03A1696028F6E55477D4482CB531308`,
Authenticode `NotSigned`. Silent NSIS installation exited 0 into
`G:\AI-Studio\verification\fung-releasefix-uat-20261023\installed`.
The installed executable reports product version `0.1.2`.

The user authorized rebuilding, installing and completing package acceptance,
then merging and releasing only if gates pass. The UAT package uses alternate
identity `dev.fung.uat.releasefix20261023` and product name `FUNG Capture UAT 20261023`.
Local evidence root: `G:\AI-Studio\verification\fung-releasefix-uat-20261023`.

The UAT NSIS uses no compression to reduce iteration time; the distributable must
be rebuilt from merged `main` with normal compression. The installed executable,
bundled worker scripts and manifest-listed runtime files must be verified, rather
than inferring payload equality from installer compression settings.

| Gate | State |
| --- | --- |
| Rebuilt installer and hash | PASS; unsigned, identity above |
| Installation / executable launch | PASS; installed executable launched as PID 36816 |
| Non-empty interactive app window | PASS after the user ended Dota; installed Home and Live rendered and accepted input |
| Bundled runtime manifest | PASS; all 4,504 file sizes/hashes match, manifest SHA-256 `561DA25ED8C64E8F84B7007424CCBA1B63FCB34E9203F9AC2BBE3B3C6873A709` |
| Installed offline CPU worker/model probe | PASS; installed Python/script/model transcribed the 32,994-ms SAPI fixture into 5 populated English segments with PATH restricted to Windows, empty HF cache path and offline flags |
| Spoken microphone → Stop → transcript | NOT ACCEPTED; first 81,154-ms ONIKUMA run persisted 11 checksum-valid WAVs, but peaks were at most 38/32768 and no speech segments were returned; no user speech acknowledgement arrived during this run |
| System loopback / selected application audio | System workflow produced audio/text but alignment failed (#94); selected application remains NOT RUN |
| Review playback / durable close-reopen | Transport play/pause/close and normal close/reopen PASS on eae669a; aligned timeline still blocked by #94 |
| Uninstall-reinstall preserving UAT data | PASS on eae669a: uninstall/reinstall exit 0, reopened with both recordings/eight edited transcript rows/27 checksum-valid chunks |
| Hosted CI on `5a090f6` | Frontend and Rust PASS; subsequent stop-wait correction requires new-head checks |
| Merge / final-main package / tag / publication | NOT RUN; gated by acceptance |

The superseded 0.1.1 packaging attempt was cancelled after review found packet
handling defects; it was not installed or accepted. Earlier headless microphone
evidence remains in the [capture follow-up](2026-10-23-capture-backpressure-remediation.md)
and does not replace the installed-package gates above.

The user ended the game and authorized continuing UAT. A fresh installed-window
observation showed Home and Live. The first ONIKUMA run used microphone-only,
the selected ONIKUMA endpoint and the prechecked participant notice. Stop was
accepted; native CPU catch-up transcribed all 11 chunks and completed the capture
job without a recorded stream fault. SQLite integrity was `ok`; every WAV checksum
matched. This verifies persistence of a near-silent capture, not spoken-mic acceptance.

### Installed stop-wait defect and correction

The UI stop waiter failed after its fixed 120-second deadline while bundled CPU
catch-up continued to make progress (per-chunk `transcribed_at` timestamps). The
screen displayed `Native capture did not settle in the allowed time` and the shell
retained a stopping/error guard even after native completion. Worker executable and
script paths were inside the installed package, not the source checkout.

The normal UI waiter now follows responsive authoritative native status until it
is inactive instead of imposing a duration-independent deadline. Explicit caller
timeouts and status-read failure propagation remain. A deterministic four-minute
finalization regression failed before the change; all 18 Live tests pass afterward,
including the explicit-deadline regression. TypeScript/Vite build passes. Independent
review found no actionable regression in the correction. A rebuilt installed retest
remains required; no merge or release is accepted. The first capture eventually
completed natively at `2026-10-23T07:43:41Z`, with no transcript because the captured
input was near-silent. The stale UI error required terminating only the identified
isolated UAT process after native completion. Its full profile was copied to
`profile-after-first-mic` under the evidence root before installing the correction.
This forced restart is not counted as the normal-close acceptance gate.

### Stop-wait rebuild and system-loopback run

The correction is committed as `eae669ab565961a0e542ce2d275b81afa9fecbd7`
and tracked by [issue #93](https://github.com/Freshair129/FUNG/issues/93).
The rebuilt unsigned UAT installer is 2,115,837,852 bytes, SHA-256
`C10E2648E803CE0AB30918161CFE0DD8A414E1396A3AF9E7ED0E96C634053224`.
Installation over the isolated UAT directory exited 0 and retained the first
recording/project. Installed executable SHA-256 is
`A063F9F39320EFE84D457A75DB5761C101945ED13F69026AF6C7C8402D08B62A`;
all bytes equal the release build except Tauri's three-byte `NSS` bundle marker
that the build output resets to `UNK` after bundling. Launch PID was 10972.

Additional Shell/Review/History/routing/release/native-custody tests passed 76/76.
This includes the native behavioral entrypoint matrix; it does not replace
installed device acceptance. Hosted frontend and Rust passed on `eae669a`.

System recording `e869555e-2712-4bca-9804-3fe8337e5db9` captured for 87,910 ms.
The initial fixture helper did not run because Windows PowerShell script policy
rejected it; no loopback signal was claimed from that failed helper. The explicit
test helper then played successfully as PID 10052. The measured system waveform
became visible; five checksum-valid system WAVs contain speech with peak about
3030. CPU catch-up produced six persisted transcript segments during processing.
The corrected UI continued displaying finalization beyond the former timeout.

However, the loopback WAV timeline started at 0 despite playback beginning about
49 seconds after recording start. Mic spans 0–87,910 ms but system spans
0–39,012 ms: initial render-idle time is compressed. This is a release-blocking
correctness defect, [issue #94](https://github.com/Freshair129/FUNG/issues/94).
Its correction and installed delayed-playback retest remain open. Application
process-loopback and spoken microphone acceptance are not promoted by this run.

### Completed eae669a workflow checks

The system capture eventually completed and persisted eight transcript rows. The
UI changed from finalizing to stopped, displayed the count of eight, removed the
capture guard, and allowed History navigation with no timeout/error. The native
summary job completed with three populated summary rows and model/evidence refs.
The installed local model was `qwen2.5:0.5b`. Its output echoed template phrasing
and included an unrelated-language item: this is pipeline-output evidence, not
useful-summary or language-quality acceptance (quality was explicitly deferred).

History displayed both actual recordings. Review loaded the selected full identity;
system playback opened paused at 0, played to 12 seconds, paused at 20 seconds and
closed. It also exposed the incorrect 0:39–1:27 tail gap caused by #94, so correct
timeline playback remains blocked even though transport controls work.

Through the visible UI, the second transcript row was changed from
`Please save the recording and check the trance` to
`Please save the recording and check the transcript.`. The persisted row contains
the edit. Export job completed and produced nonempty SRT/VTT containing the edited
text plus a PCM WAV of 4,219,680 frames at 48 kHz (87,910 ms). SHA-256 values and
paths are in `export-files-verified.json`; DB/output rows are in
`after-export.json` and `summary-export-result.json` under the evidence root.

The app's own Close button terminated the installed process normally with no active
capture/player. Read-only integrity check after exit was `ok`; both recordings,
eight transcript rows and 27 checksum-valid chunks remained. The profile was copied
to `profile-after-normal-close`. Silent uninstall exited 0, removed installed
`fung.exe`, and retained the UAT projection. Reinstall exited 0; installed executable
hash remained `A063F9F3...B62A`. Reopened PID 41128 showed the existing project on
Home. Read-only `after-reinstall-reopen.json` verifies integrity `ok`, both recordings,
eight transcript rows, 27 checksum-valid chunks, and completed capture/summary/export
jobs. This acceptance applies to the eae669a package, not subsequent native changes.

Selected-application preflight remains unaccepted: two owned SoundPlayer processes
(speech PID 26568, distractor tone PID 43404) were alive, but refreshing the app's
application-source inventory returned no selectable applications. No application
recording was started or falsely counted. The helpers were stopped after observing
the inventory; a native apartment/session probe is investigating the cause.

The output configuration confirms media is isolated under
`C:\Users\suanr\AppData\Roaming\dev.fung.uat.releasefix20261023\recordings`.
Installed `transcribe.py` and `transcribe_live.py` hashes equal source. Local
evidence includes `installer-identity.json`, `launch-identity.json`,
`runtime-verification.json`, `installed-payload-hashes.json`, and
`installed-offline-transcript.json`. The offline fixture proves bundled runtime
operation only, not the required installed real-microphone workflow.
