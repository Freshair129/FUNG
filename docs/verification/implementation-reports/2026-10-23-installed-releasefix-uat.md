---
version: "0.1.4b"
created_at: "2026-10-23"
last_update: "2026-10-24"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-release-acceptance"
  doc_type: "verification"
  scope: "FUNG 0.1.2 rebuilt installed-package acceptance and release gates"
  language: "Thai-first"
---

# Rebuilt Desktop package acceptance

## Current disposition — 2026-10-24

The latest installed source is `9b2ed86`. **The complete release gate is FAIL**: selected-
application capture still loses active PCM when its renderer stops/restarts
([issue #96](https://github.com/Freshair129/FUNG/issues/96)), and installed export
silently omits the system channel
([issue #97](https://github.com/Freshair129/FUNG/issues/97)). The bounded writer
fix passes continuous/stalled-writer tests, but does not close this transition
gate. Independent source review found no additional regression; independent
acceptance review retains FAIL. PR #91 remains draft; no merge, tag or publication
is claimed. The chronological sections below retain earlier package identities
and superseded results instead of relabeling them as current acceptance.

The current installed candidate was built from `9b2ed86`: unsigned UAT installer
**2,115,736,988 bytes**, SHA-256
`D0D485E3763AAB8246C14DD412266505149C02475DC051C3EE0FD5ED10DA2ED6`;
installed executable SHA-256
`BEC2EA79AC1CD927AAAADDB287B211D9FF779C9A70BA402ADB93FAE8694D51A8`.
Silent installation exited 0 and all 4,504 runtime entries matched the manifest.
The isolated profile was backed up before installation; its database integrity
was `ok`, with four recordings, nine transcript rows and 40 checksum-valid chunks.
Normal-profile data was not used.

Installed Archive list/detail correctly show a durable capture warning for the
earlier failed application-track recording `3f1c168d-afb7-4968-b796-895438f21872`.
Selecting the clean recovered recording removes the detail warning; the warning
does not leak between recordings. This was observed after cold launch of the
newly installed executable (`writer-installed-partial-warning.txt` and
`writer-installed-clean-selection.txt`).

Both active render endpoints remain muted at the user's request. Native system
and selected-process probes retain PCM under mute. No new human microphone speech
is requested during unattended testing, and microphone settings are unchanged.

### Remaining transition loss: bounded experiments rejected

The reference-correlated marker experiment keeps a separate silent renderer
active and simultaneously captures full-system audio. It therefore distinguishes
selected-process loss from an engine-wide quiet interval. The comparator has at
most 1 ms of chunk-start rounding uncertainty; the measured losses exceed it.

| Probe | Missing active frames before Stop | Missing active frames after Resume | Outcome |
| --- | ---: | ---: | --- |
| `marker-transition-v13` | 1,024 (21.3 ms) | 0 | FAIL |
| `marker-scheduling-mmcss-v15` | 1,504 (31.3 ms) | 0 | FAIL; MMCSS Capture registration succeeded |
| `marker-timeout-drain-v16` | 1,984 (41.3 ms) | 461 (9.6 ms) | FAIL; 1-ms timeout draining executed |

Reader decode/release costs approximately 28–33 microseconds. Scheduling
registration and shorter timeout draining did not remove the loss. Their
`cfg(test)` diagnostics were removed and the production file restored exactly
to `9b2ed86`; frozen external executables, packet traces, WAVs and comparison
receipts remain under the evidence root. No gap tolerance was relaxed and no
missing active audio was relabeled as silence.

An optional Microsoft reference sample was source-pinned, license-preserved and
built outside the repository. Automatic approval review rejected executing it
with `blocked by policy`; neither that executable nor its wrapper was run. These
build receipts do not establish reference-runtime behavior. No alternative path
was used to bypass that rejection.

### Installed system workflow and export failure at 9b2ed86

From installed Live, selected ONIKUMA and the default system route, started the
owned SAPI fixture under output mute, recorded 21 seconds, then pressed Stop
before the fixture ended. Recording `51abb8ba-db6e-4b5b-ab5f-bc82bd8be032`
retains six chunks: three system chunks spanning **0–21,349 ms**, and three mic
chunks spanning **0–21,310 ms**. Every checksum matches. The system chunks contain
726,081 nonzero samples with peak 31,726; the unspoken microphone contains only
221 nonzero samples with peak 5. This run proves system speech capture and mic
track persistence, not new spoken-microphone acceptance or perceptual quality.

The capture job completed at `2026-10-23T23:42:25.962515900Z`, approximately
92 seconds after Stop, without a capture-fault audit. The UI settled to stopped
and displayed eight populated system transcript rows. The local summary job also
completed. System playback opened, advanced from 0:00 to 0:10, paused at 0:15 and
closed. Output remained muted, so this verifies transport and populated media,
not an audible listening assessment.

The first transcript row was edited in the installed UI to
`The next meeting is tomorrow at ten. [UAT saved edit]`. The Export button created
SRT/VTT with that exact edit, but its sole WAV was microphone-only: 21,310 ms,
221 nonzero samples, peak 5. The speech-bearing system channel was absent. The
export job incorrectly reported completed. Source diagnosis found the playback
default `build_audio(..., None)` selects mic first; the export dispatcher also
swallows audio-render errors. Issue #97 tracks correction and rebuilt retest.
**This export run is FAIL despite successful subtitle files.**

After an orderly installed-app close and relaunch, the UI retained the new
recording and saved edit. SQLite integrity is `ok`; all five recordings, 17
transcript rows and all 46 chunk hashes are unchanged. A first launch process
ended without a captured diagnostic; the monitored retry produced the actual
Home/Archive window. The data-retention pass is not a claim that every launch
attempt succeeded. Local receipts: `writer-installed-finalized.json`,
`writer-installed-export-loss.json`, `writer-installed-exports.json`,
`writer-reopen-comparison.json`, `writer-installed-playback.png`, and
`writer-installed-reopened-edit.png`.

The actual developer MCP call passed at `9b2ed86` in
`workflow-2026-10-23T234253761Z-c1b2fc82`: fixed synthetic import, CPU transcription,
edit, subtitle/audio export, playback read and database reopen. Test-internal
duration was 28.56 seconds, with a diagnostic keyword score of 12/12. The source
fixture has a single imported channel, so this pass cannot detect issue #97 or
replace installed/native-source acceptance. Hosted frontend and Rust CI both
passed at `9b2ed86` (49 seconds and 19m 51s respectively;
[run 40039614591](https://github.com/Freshair129/FUNG/actions/runs/40039614591)).

### Installed selected-application steady-state run at 9b2ed86

After relaunch, refreshed native inventory and selected the owned speech helper
`powershell.exe` PID 30084. Recording
`fc5f23e7-8649-44a0-aa89-782a8e0aac3e` records the provenance
`remoteSourceKind=application_loopback` and that exact PID in its start audit.
The fixture continued playing until after capture Stop, so this deliberately
does not exercise the unresolved renderer Stop/Resume transition.

Both channel timelines span **0–16,360 ms** in three chunks. Application audio
contains 581,362 nonzero samples with peak 31,735 and retains its 360-ms tail;
the unspoken mic is zero. All six checksums match. Capture and summary jobs
completed; installed Live settled to stopped and displayed four populated remote
transcript rows (`writer-installed-app-finalized.png/json`). No capture-fault
audit was recorded. The native application inventory and this steady-state
installed workflow pass, while issue #96 remains FAIL. Subsequent profile totals
are six recordings, 21 transcript rows and 52 checksum-valid chunks.

### Issue #97 source correction — installed retest pending

The [bounded repair plan](../../plans/2026-10-24-multichannel-source-export-fix.md)
and canonical D-MVP-05 amendment define separate channel WAVs, stable artifact
identities, bounded preparation and explicit failure instead of skipped audio.
The source candidate prepares a complete content-hashed media directory and
commits its artifact rows together. A failed metadata commit preserves the
previous artifact set; unchanged retries verify and reuse identical media.
Legacy single-file publication restores prior bytes when metadata commit fails.
Subtitle/media/job publication is explicitly not a cross-resource transaction.

Export now requires completed native capture and renders the exact validated
chunk snapshot. This permits completed capture while CPU ASR is still running,
but rejects active capture, changed source metadata, missing media, invalid or
noncontiguous source timelines and byte-ceiling violations. The export-only
bounded stitcher leaves playback behavior unchanged. The admitted import path
is also used for transcoding. PyAV enforces its output limit during mux writes.
The Archive inventory shows actual basenames so separate WAVs are distinguishable,
while retaining its project-scope warning.

Twelve focused Rust export regressions pass, including source mutation, running
ASR, byte ceilings, timeline refusal, publication/metadata failure, retry/tamper
checks and legacy compatibility. Independent final re-review accepted the source
without remaining findings. The filename UI regression failed before its helper
was added; all 18 History tests and TypeScript/Vite then passed.

The broad package-script run initially passed 33/35 non-custody commands. It found
an unregistered Python test and an obsolete UI assertion. Explicit npm registration
and the assertion were corrected; targeted reruns pass (CI inventory, job actions,
recording output and its three Python bounded-mux tests). The original failure
receipt is retained. The first combined library/Clippy pass was 690/0/10 ignored
and 1m 11s respectively; it predates the final snapshot-race correction. After
that correction, the fresh full library passed **692 tests, zero failed, ten
ignored in 140.17 seconds**; Clippy passed in 18.15 seconds and the registered
native custody matrix passed all 12 checks. All 36 package test commands therefore
pass compositionally, retaining the original two failures and explicit reruns.
Formatting, diff checks and 101 relative file links across seven edited/new
documents passed. Installed-package export is still required; this source review
does not change the installed failure or the separate issue #96 gate. Receipts:
`final-export-snapshot-rust-tests.log`, `final-export-snapshot-clippy.log`,
`final-export-snapshot-native-custody.log`, `final-export-frontend-results.json`
and `final-export-frontend-rerun.json`.

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
| Final combined native clock, MTA inventory and safe application reader library | 670 passed, 0 failed, 9 ignored; 158.84 s at 0.1.2; `source-final-rust-tests.log` |
| Final bounded writer, partial-track projection and all-source tail library | 683 passed, 0 failed, 10 ignored; 197.87 s; `final-writer-rust-tests.log` |
| Clippy, all library targets, warnings denied | PASS at 0.1.2 |
| Final combined native patch Clippy, all library targets, warnings denied | PASS; 1m 54s; `source-final-clippy.log` |
| Final bounded-writer Clippy, all library targets, warnings denied | PASS; 40.20 s; `final-writer-clippy.log`; formatting and diff checks also pass |
| Release distribution metadata checks | 11 passed at 0.1.2 |
| Live workspace, routing and recording-output Node checks | 26 passed |
| TypeScript/Vite production build | PASS at 0.1.2 |
| Hosted CI on starting revision `6e6067c` | Frontend and Rust PASS; superseded by the new packet-handling changes |
| Hosted CI at `ec494fd` | Frontend PASS; Rust FAIL on newer Clippy `chunks_exact_to_as_chunks` in both native readers; correction pending |

## Historical installed acceptance — 5a090f6

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
| Spoken microphone → Stop → transcript | Functional output observed on ec494fd: microphone audio with peak 7,306 and populated transcript; first near-silent attempt remains historical below; corrected final package still requires retest |
| System loopback / selected application audio | NOT ACCEPTED: system alignment/startup classification (#94) and selected-application packet-position failure remain open; inventory selection passed ec494fd |
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

### Additional diagnosis and native candidate evidence

[Issue #95](https://github.com/Freshair129/FUNG/issues/95) tracks the application
inventory failure. A production native probe on an STA caller returned
`RPC_E_CHANGED_MODE (0x80010106)`; an MTA caller returned both controlled active
player PIDs 18588 and 42608. The correction isolates inventory on a named MTA worker
with balanced COM lifetime. A frontend source regression also caught the hidden
error: supported Windows previously masked an inventory failure with an empty-list
hint. The regression failed first, then routing 8/8, Live 18/18, TypeScript/Vite,
and a separate Shell/Review/History/routing/release group of 65 tests passed.

The first #94 CPAL-clock proposal was rejected by independent review: CPAL hides
native discontinuity flags, so clock jumps cannot all be claimed as known silence.
The replacement uses native WASAPI packet flags. Its first real probe failed on
startup/discontinuity and timestamp policy; it was not accepted. Subsequent changes
retain a sample clock for continuous packets, bound timestamps using measured
endpoint buffering, and handle the startup flag without discarding genuine loss.

`native-system-probe-v2` exercises the production native system route after five
seconds of initial quiet. It passed: first nonzero PCM at 5,500 ms, three chunks
covering 0–20,049 ms, 477,042 nonzero samples and no faults. The next actual run,
`native-system-probe-v3`, used initial quiet, speech, four seconds of quiet and
resumed speech. It passed: first nonzero at 3,427 ms, 20,087-ms total timeline,
458,006 nonzero samples and no faults. Per-second WAV checks confirm seconds
0/1/2 and 8/9/10 are zero, while 4/5/6 and 12/13/14 contain signal. The native
probe reports and `per-second-peaks.json` are local evidence. These runs do not
claim installed-package acceptance for the new native code.

The ec494fd startup exception was restricted to native frame index zero. A regression
first failed for an initial discontinuity at index 480, then passed with a real
0–20-ms loss interval and retained 20–30-ms audio. This avoids converting genuine
initial loss into clean silence. Five native clock/consumer regressions pass.
The process-loopback reader now uses the existing public native client interface,
checks `SILENT` before nullable pointer access, bounds packet size and releases
buffers on decode errors. Its focused Rust group passed 8 tests with the explicit
hardware inventory probe ignored. No vendoring or extra dependency is required for
that reader. Independent re-review of the combined native/UI patch found no
unresolved actionable findings. The final combined library suite passed 670 tests,
with zero failures and nine explicit ignored tests in 158.84 seconds. Clippy with
warnings denied passed in 1m 54s. New installed-package UAT remains a gate; source
review and native probes do not close it.

### ec494fd installed retest: review reopened

The installed candidate has installer SHA-256
`7263C767C5E276518D2518748B7BC1E5C04D785E34C956F88CBE06FE9E5D2588`
and executable SHA-256
`55CE51716EA5A69FFD1FF2EBDA61113A5385BA821359DB3FE9B4EF18B90E3AAD`.
All 4,504 bundled runtime entries matched their expected sizes and hashes.
The application inventory fix passed in this package: both controlled active
players were selectable and the speech player PID 19232 was explicitly selected.

Recording `3f1c168d-afb7-4968-b796-895438f21872` retained 56,560 ms of microphone
audio, including a chunk with peak 7,306 and 84,275 nonzero samples. Its microphone
transcript contains `One, two, three.` at 11,280–13,000 ms. Capture and summary jobs
completed. This demonstrates actual microphone input and transcription output;
it does not establish transcription quality or a successful application track.
That track failed after 10 ms with a native discontinuity/position error.

A simultaneous production system-route positive control retained populated audio
but failed startup classification: its first packet had render index 4,137,600,
480 frames, flag `0x1`, and a 44-ms start offset. The prior review requirement
`index == 0` was withdrawn: a loopback client can join an existing render stream,
so a nonzero first render index does not establish capture loss. Independent review
is reopened, and both native routes require correction and installed retesting.
The earlier quiet-start probes remain historical evidence only.

The developer MCP synthetic CPU workflow also passed at ec494fd in run
`workflow-2026-10-23T083815256Z-317eaec0`: transcript, edit, SRT/VTT/WAV export,
playback read, and database reopen retained outputs. Its receipt explicitly does
not replace installed microphone, system, or application capture acceptance.
Hosted frontend CI passed; hosted Rust CI failed Clippy's newer
`chunks_exact_to_as_chunks` lint in both native readers. Release remains blocked
until the corrected head passes these checks and the native capture gates.

### Native correction probes after ec494fd

The replacement startup policy uses recent observed buffer availability and measured
buffer duration, rather than requiring a zero initial render index. Its frozen
debug probe executable SHA-256 is
`DB5334FF8E528504E1D82AE4BF74097B4DEAE987C8D727DB67C57E2C0B430663`.
`native-system-active-v4` passed with audio already playing: 20,122 ms, first
signal at 42 ms, 656,768 nonzero samples, and no faults. `native-system-idle-v5`
passed initial quiet → speech → four seconds of quiet → resumed speech: 20,045 ms,
first signal at 3,460 ms, 455,950 nonzero samples, and no faults. PCM diagnostics
confirm seconds 0/1/2 and 8/9/10 are silent while both speech intervals contain
signal. These production-route probes do not replace installed-package retesting.
The intervening `native-system-idle-v4` run omitted the player's mandatory WAV
argument and therefore provided no input; it is a rejected harness run, not a
product failure or a silence acceptance result.

The selected-application diagnostic isolated its cause: packet two had 480 frames,
zero flags, device index zero again, and an advancing native QPC timestamp. The
old reader treated the virtual client's repeated zero counter as a backwards
position despite no discontinuity flag. The application clock correction and
simultaneous selected-source/distractor control remain pending.

The next selected-source run (`native-application-qpc-v2`) streamed 12 seconds of
speech but rejected a discarded 30-ms normal-stop tail. Tail persistence was
corrected with an exact 30-ms regression. The subsequent paused/resumed-source
run (`native-application-tail-v3`) retained its tail but still reported several
10–54-ms source gaps; it was not accepted. A deterministic nonperiodic PCM fixture
then independently verified real loss, rather than merely a timestamp anomaly:
`native-application-marker-v4/pcm-continuity.json` found 960 missing reference
frames (20 ms) exactly where the QPC classifier reported a 20-ms gap.

`native-application-anomaly-v5` measured a virtual native buffer of only 480 frames
(10 ms), while synchronous chunk writes took 21–58 ms. Reference correlations also
confirmed the later reported losses. The classifier is retained; increasing its
tolerance would hide real missing media. Supported buffered polling is being
measured before choosing a larger reader/writer refactor.

The user authorized unattended completion and requested silent operation. Both
active render endpoints were muted and read back as muted; microphone settings
were not changed. `native-system-muted-marker-v6` passed 20,117 ms with 963,719
nonzero samples and no faults while speakers remained muted. Process-loopback
marker probes also retained real signal under mute. Overnight tests do not
require unmuting outputs or new human speech. Prior installed microphone speech
evidence remains explicitly separate from new fixture-driven native evidence.

Independent review additionally found that channel-failure event detail was hidden
by the generic degraded state, and terminal stream faults could leave a capture
job marked completed. The Live panel now surfaces degraded/error detail; its
regression failed before implementation, then the full Live group passed 19 tests
and TypeScript/Vite passed. Durable terminal-failure reporting is now implemented:
terminal exits fail the job and degrade the revisioned session; bounded batched
audit reads project the stable optional `captureWarning` code into scoped Review
rows. All four source-fault types are covered without exposing raw error text.
The combined Review tests passed 10 native and 17 frontend checks; microphone and
system tail corrections passed 32 native checks (two hardware tests ignored).
Independent review found no unresolved actionable findings in these changes.

### Bounded application writer correction

The supported polling experiment (`native-application-polling-v6` and stress-v7)
was rejected before capture: the requested two-second buffer still measured only
480 frames. It is not counted as an audio acceptance result. Event-driven capture
was restored, and copied PCM is handed to a bounded writer queue. The reader keeps
its own clock and end position; overflow/disconnection stops the native source,
drains accepted PCM and preserves the tail before reporting explicit loss/failure.
No timestamp-gap tolerance was relaxed. Slow-write and raw trace hooks exist only
in test builds.

Frozen writer probe SHA-256:
`1B31C19D0C344E933454A868B1347DEB7D149E9979C561A96A24BC3DF6E591EF`.
`native-application-writer-v8` passed a 12,010-ms baseline with 574,947 nonzero
samples and no faults. `native-application-writer-stress-v9` passed 12,060 ms with
577,362 nonzero samples and no faults while every two-second WAV write was delayed
250 ms. Reference cross-correlation found **zero missing frames at every chunk
boundary**, including the retained 10-ms and 60-ms final tails. Source SHA-256
`FFE188C5CE2A7BCCDDACADCD5D9A415D6065A1864FA2082806B18109264C79D6`
matches both frozen runs. Independent combined source review is accepted; focused
frontend tests passed 71. The frozen full Rust library passed 683 tests with zero
failures and ten explicit ignored tests; Clippy, formatting and diff checks passed.
The 36 package test commands were verified compositionally: 35 passed in the
initial broad run; the native custody compile was cancelled by the operator to
release the Cargo slot. After limiting that runner to its registered library
entrypoints, its explicit rerun passed all 12 checks. The later 71-test frontend
group and fresh TypeScript/Vite build passed. Original cancelled-run evidence is
retained instead of relabeled as an uninterrupted clean run.

The controlled selected-speech/distractor run v11 proves source isolation and
three seconds of quiet: selected-source 2200-Hz amplitude during quiet was below
0.001 while simultaneous system control measured approximately 5,000. It still
reported two 10-ms gaps at render-stream Stop/Resume boundaries. Those transition
intervals remain under investigation and are **not** a normal pause/resume pass.
An earlier v10 attempt and the first marker-transition v12 attempt failed the
active-source preflight before capture; these are not audio acceptance results.
Rebuilt installed acceptance and transition disposition remain separate gates.

### Installed crash recovery on eae669a

An additional isolated microphone-only recording was interrupted by terminating
the identified UAT process during capture. On restart the visible recovery banner
reported recording `85920b91-e5d3-4d6c-82e3-7d4afc584479`. The app's Recover button
completed recovery of four durable chunks covering 32 seconds and processed all
four for transcription. The input was quiet, so no speech accuracy is claimed.
All 30 chunks observed before termination retained identical hashes; one additional
chunk committed before process termination, giving 31 after recovery. SQLite
integrity remained `ok`. The original capture job correctly retains its interrupted
failure history instead of being relabeled as an uninterrupted success. This proves
durable-chunk recovery, not retention of the unsaved in-memory capture tail.

The output configuration confirms media is isolated under
`C:\Users\suanr\AppData\Roaming\dev.fung.uat.releasefix20261023\recordings`.
Installed `transcribe.py` and `transcribe_live.py` hashes equal source. Local
evidence includes `installer-identity.json`, `launch-identity.json`,
`runtime-verification.json`, `installed-payload-hashes.json`, and
`installed-offline-transcript.json`. The offline fixture proves bundled runtime
operation only, not the required installed real-microphone workflow.
