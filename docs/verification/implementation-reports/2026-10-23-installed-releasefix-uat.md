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
| System loopback / selected application audio | NOT RUN |
| Review playback / durable close-reopen | NOT RUN |
| Uninstall-reinstall preserving UAT data | NOT RUN |
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

The output configuration confirms media is isolated under
`C:\Users\suanr\AppData\Roaming\dev.fung.uat.releasefix20261023\recordings`.
Installed `transcribe.py` and `transcribe_live.py` hashes equal source. Local
evidence includes `installer-identity.json`, `launch-identity.json`,
`runtime-verification.json`, `installed-payload-hashes.json`, and
`installed-offline-transcript.json`. The offline fixture proves bundled runtime
operation only, not the required installed real-microphone workflow.
