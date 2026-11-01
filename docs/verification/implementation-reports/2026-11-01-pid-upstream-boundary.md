---
version: "0.1.4b"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Issue 96 raw packet search and upstream diagnostic admission"
  language: "English"
---

# PID upstream packet boundary

Continuation of the [investigation plan](../../plans/2026-11-01-pid-upstream-boundary.md)
at FUNG HEAD `4950696c607c5b77eaba3840b2413ab4702beee2`, branch
`fix/issue-96-app-loopback-transition`. Existing uncommitted product work is
preserved. This investigation has not established a product repair, new installed
package acceptance, or permission to merge/tag.

## Raw received-buffer search

Evidence: `E:\verification\fung-pid96-upstream-boundary-20261101`.
The search reuses the frozen `direct-child03` and `parent-tree01` recordings from
the [root-attribution study](2026-11-01-pid-root-attribution.md); these are old
native recordings analyzed afresh, not new device runs.

The original left/right stereo and independently checked mono streams were
searched for overlapping marker windows covering the inferred Stop/Resume bands.
Whole-stream Stop matches were found in *other* epochs because the renderer
restarts the same marker. Those matches cannot establish delivery of the earlier
tail. Within the same epoch's packet-QPC envelope (start minus 200 ms through
drop plus 200 ms), every searched 480-frame Stop window scored below 0.95:

| Recording | Highest absolute Stop correlation, cycles 1 / 2 |
| --- | --- |
| direct-child03 | approximately 0.722 / 0.703 |
| parent-tree01 | approximately 0.683 / 0.703 |

The 192-frame Resume windows also lack a >=0.95 whole-stream match (best scores
approximately 0.896–0.911). There are no SILENT-flag frames in these two raw taps.
Results strengthen the inference that the expected marker regions are absent at
the received PID packet boundary in the corresponding epoch. They do **not**
identify an internal Windows component or prove an exact missing-sample count.

The diagnostic correlation threshold is not the unchanged ±48-frame acceptance
gate. Independent review records that epoch masks exclude timestamp-error flags
but do not prove continuity across discontinuity flags or QPC jumps. Finite
windows, gain/phase changes, repeated markers, timestamp uncertainty and packets
never returned by the API limit the conclusion. Original receipts and the separate
`independent-method-review.json` are retained without rewriting earlier results.

Reproduction uses Python
`C:\Users\suanr\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe`
(3.12.14), NumPy 2.3.5, `search.py` and `search-epoch.py`; runtime/script/input
hashes are in the adjacent provenance files. Root reran
`-m unittest -v test_search test_epoch`: **4 passed**. An earlier attempt using
default Python 3.14 failed imports because NumPy was absent; that was not an
analysis result. Runtime provenance is partial, not a complete wheel/DLL lock.

## Windows queue metadata and actual permission result

Evidence: `E:\verification\fung-pid96-tdh-schema-20261101`.
Read-only TDH queries returned the current Microsoft-Windows-Audio manifest:

- Event 115: cross-process instance pointer, read/write indices, frame count, QPC.
- Event 159: object pointer, frame count, APO flags, QPC.
- Event 34: instance pointer, write/read positions, dropped-byte and write counts.

These schemas contain numeric/pointer metadata, but no direct target PID/session
field for a narrowly attributed upstream packet trace. Events 24/25 have PID but
also AppId; their relationship to queue instances is not established here.
Schema availability is not runtime event attribution or proof of loss location.

The real permission probe attempted a unique, nonkernel real-time session with
**zero providers enabled**. `StartTraceW` returned **5 (ERROR_ACCESS_DENIED)**,
no handle was returned, and no trace session or audio collection was started.
The current shell is medium integrity. No elevation, existing-session changes,
policy changes or service-wide trace was attempted. The actual receipt is
`session-permission-result.json`.

## Prepared release-boundary fixture

Evidence: `E:\verification\fung-pid96-release-boundary-20261101`.
An isolated copy of the controlled renderer now stores bounded numeric records
around successful `write_to_device` calls: unwrapped source/submitted frame ranges,
padding, and QPC brackets. A preallocated 12,000-record vector per epoch avoids
JSON allocation in the measured write path. Metadata is serialized after native
Stop. Overflow/failure invalidates observation.

All comparison arms use the same binary. The sole configured variable is the
requested **0 / 10 / 50 ms** stopped-client retention before destruction. The
actual interval is measured with before/after-Drop QPC; sleep duration is not
assumed exact. No writes occur during this retention. An early source-review
finding removed JSON processing between Stop and Drop before the build.

Artifact pins:

| Artifact | SHA-256 |
| --- | --- |
| GUI renderer, `artifacts/renderer-06DCDC…exe` | `06DCDCBA24BFAD02D2168D71F3B650B5052153F2658A3DA9CE066DDDF505B4B9` |
| GUI supervisor, `supervisor-5FCA61…exe` | `5FCA612CB57B2FB28EB269E878F9067DB6DCCACD91A7F687BDF228F7F5675D24` |
| `run-release-boundary.py` | `9B4E2F778BE8C1D763D1E921168B749E3146DFEE29953F6BF25530C45CD8BD66` |

The renderer builds with
`cargo +1.99.1 build --locked --offline --release --bin fung-stop-hold-renderer -j 1`
in its isolated target. Invalid delay 11 and incompatible hold=true fail before
opening an endpoint. This builds a diagnostic helper, **not FUNG's application
package**. Its successful submission calls do not establish physical playback.
The requested lifetime control belongs to our fixture; it is not a supported way
for FUNG to retain another application's native audio client.

The `analyze-release-boundary.py` adapter pins the comparator, controller
and binaries, checks contiguous write ranges against Stop receipts, and reports
actual retention time plus marker-to-submitted-buffer spans. It retains nonzero
capture failures and the ±48-frame gate. Root reran its synthetic tests:
`-m unittest -v test_release_boundary_analysis`, **3 passed**. Independent static
review found no confirmed blocker. It subsequently processed all six native
recordings below. Product inputs matched all 258 post-format snapshot hashes; 318
local documentation paths exist (anchors/remote links were not checked).

## Earlier native admission refusal

At `2026-11-01T00:33:04Z`, both exact-ID HDMI devices LM22 and Mi Monitor report
`NotPresent`. The controller invocation for `delay0-admission01` verifies pins
then refuses with `Endpoint unavailable` **before launching any renderer or
capture process**. The report retains its additional cleanup-phase volume check
failure (`Muted read-only guard failed`); its process collection is empty. This
is a refused experiment, not an audio failure/pass. User was asked to reconnect
the devices while preparation continued.

The user subsequently reported both monitors connected. A fresh
`ready-device-inventory.json` confirms both exact-ID endpoints Active. The refused
attempt is preserved; it is not counted as one of the six completed runs below.

## Actual matched and reversed-order native comparison

Executed serially in order **0, 10, 50, 50, 10, 0 ms**, with no concurrent FFT or
build workload from this investigation. Each run uses the same pinned producer,
supervisor, cold-first diagnostic PID reader and independent LM22 endpoint
reference. No user application or microphone is captured. The analyzer runs only
after the audio processes have returned. Evidence is under the same release-boundary
root, directories `delay0-a` through `delay0-b`; summary: `six-arm-summary.json`.

| Run | Observed Stop-to-before-Drop, ms (cycles 1 / 2) | PID minus endpoint Stop edge, frames | Restart edge, frames | Whole-run verdict |
| --- | --- | --- | --- | --- |
| delay0-a | 0.0001 / 0.0001 | −1984 / −1984 | +441 / +441 | FAIL |
| delay10-a | 10.6300 / 10.1556 | −1947 / −1471* | +441 / +441.175* | FAIL |
| delay50-a | 50.3044 / 55.8476 | approximately 0 / 0 | +441 / +441 | FAIL |
| delay50-b | 50.2056 / 50.4830 | −33 / approximately 0 | +441 / +441 | FAIL |
| delay10-b | 10.3226 / 10.0867 | −1958 / −1940 | +441 / +441 | FAIL |
| delay0-b | 0.0004 / 0.0000 | −1984 / −1940 | +441 / +441 | FAIL |

*`delay10-a` cycle 2 is explicitly **NOT_PROVEN** by the comparator: the later
PID segment has a 320-frame alignment-fit residual and its restart band is outside
retained source/capture coverage. Those printed numeric estimates are diagnostic
only and do not qualify that cycle. Its whole-run FAIL remains unchanged.

All six supervisors/renderer owners return 0, all endpoint references return 0
without reported faults, and all PID readers return 101. No run needs forced
cleanup or reports a cleanup failure. The PID reports four discontinuities per
run except `delay10-a`, which reports five. The extra fifth gap is retained, not
discarded as an outlier. The ±48-frame gate is unchanged. The 50 ms arm passes
only the measured **Stop-edge component** across all four cycles; restart and
whole-run continuity still fail. This is not installed endpoint acceptance.

Independent audit (`independent-six-arm-custody-audit-v2.json`, script
`independent-receipt-audit.py`) reconciles original received stereo to independently
computed mono and every reported WAV payload in all six runs. It also checks
muted receipts, sampled nondefault/owned-session inventory, cleanup and exit
statuses. The original audit v1 remains preserved; v2 corrects the label for a
zero-length missing band, not the capture results.

The new write metadata records successful submission of all available, reported
source comparison bands before Stop, rather than relying only on a cursor sampled
at Stop. Unavailable bands are not claimed verified. Increasing stopped-client
lifetime to roughly 50–56 ms permits the PID
capture to preserve the measured tail in these runs. This supports a
lifetime-sensitive delivery/history explanation between renderer submission and
the received PID packet boundary; it does not uniquely identify a Windows queue,
prove physical playback or establish a universal 50 ms threshold. QPC/instrumentation
overhead, finite source matching, scheduling and this single host remain limits.

## Remaining product and release gates

### Resume localization and receiver-only comparison preparation

Continuation artifacts: `E:\verification\fung-pid96-resume-localization-20261101`.
This is a fresh offline analysis of the six recordings above, plus native
**configuration queries without capture**. It is not six new audio runs.

Independent packet/onset analysis, replayed by the root agent with exact matching
JSON results, finds:

- All twelve post-recreation raw first-active packets have 64 leading samples
  of magnitude at most 1, followed by 416 samples correlating essentially 1.0
  with source marker frames 480–895. Eleven whole-segment alignments are credible:
  PID onset maps to source frame 480, endpoint onset to frame 39. The twelfth
  retains the prior NOT_PROVEN alignment; a matching short window does not fix it.
- The preceding 441 raw mono samples are near zero in all twelve cases, while
  marker frames 39–479 contain substantial signal (439 samples above magnitude
  20). This weakens a threshold-only explanation; it does not establish which
  conversion or native component omitted the earlier marker.
- None of the 25 QPC leaps has independently matched marker windows on **both**
  immediately adjacent 480-frame packets. One side is low-signal or unresolved.
  It is therefore unjustified either to equate each timestamp gap with a known
  quantity of lost active PCM or to declare it safe silence.
- In 24 of 25 events, packet read-entry spacing tracks the QPC spacing to within
  1.85 ms. These are not ordinary 10 ms read observations with only renamed
  timestamps. Read-entry occurs before GetBuffer, so it cannot distinguish native
  production delays from thread scheduling. The remaining event is the retained
  `delay10-a` extra gap, about three seconds after recreation.

`root-localization-verification.json` checks original stereo-to-mono equality,
packet-byte coverage, onset indices, all 25 gap calculations and the frozen input
hashes. All 258 product inputs still match the previous post-format snapshot.
`root-review-replay.json` records exact replay of onset and cadence outputs.

A reviewed C++ self-PID configuration probe was compiled and actually executed
for requested buffer durations 0, 1 and 5 seconds. All Initialize/GetBufferSize
calls return success and **480 frames**, with no Start or capture call. This
checks stereo Float32, event-driven LOOPBACK configuration only, not a running
foreign process or every possible mode. Nonzero shared/event duration requests
are not Microsoft's recommended configuration; success does not mean the requested
duration was honored. Results: `capability/actual-query-results.json`.

Earlier [polling experiments](2026-10-25-issue-96-alternative-capture.md) already
fail with real controls; polling is not proposed again as a new repair. Earlier
Float32 v19 evidence lacks a raw-packet dump and a located source snapshot matching
its source-hash receipt, so its whole-run failure cannot explain the current
64-frame/480-frame packet shape. Its prior failure remains preserved.

Prepared a separate C++ raw receiver using one event loop for either PCM16 with
AUTO/SRC conversion or Float32 without AUTO. It preallocates bounded memory,
copies native bytes before ReleaseBuffer, records exact format/flags/QPC, binds
target process creation time, publishes readiness after Start, and exits within
its capture bound. SILENT flags explicitly describe zero-filled silent packets;
they are not presented as copied native payload. Final capture tails are censored
because this diagnostic does not drain after Stop. The production reader and
normal profiles are unchanged.

| Prepared artifact | SHA-256 |
| --- | --- |
| Raw receiver executable | `5BADEFA043865723B8931837E505B07CCEBDF51A59576514B7406C8BE0F3C4B2` |
| `receiver/receiver.cpp` | `C0E272D1336161A9F4C0FE6EC242051C846EC0B9BAF1CC2D8DB8FB48E6662E13` |
| `run-receiver-comparison.py` | `E3D94F7CB879268460B4B29E9B85800410B9EB9A4A87B226080A0128367C5F27` |

The raw receiver builds with MSVC `/W4`; invalid-format smoke exits 2 before audio.
Independent static review found no confirmed remaining launch blocker. The
analyzer retains FAIL/NOT_PROVEN, ±48 frames and all native clock faults; it never
assigns whole-mode PASS from these observations. Root ran bundled Python
`-m unittest -v test_receiver_analysis`: **5 passed**. Independent review found
missing control-WAV/stderr hashes; the receipt now hashes every input file in the
run, including control audio and identity evidence. These are synthetic/source
checks, not native continuity qualification.

The first native comparison attempt, `native/pcm16-a`, was **REFUSED** by the
unchanged preflight: both exact HDMI devices were `NotPresent`. Its controller
records `Endpoint unavailable`, a subsequent muted-guard refusal, and an empty
process collection. No renderer, receiver or endpoint-reference process launched.
This is neither a capture failure nor an audio pass. The user subsequently
confirmed reconnection; the actual comparison below preserves the refused attempt.

The proposed receiver comparison changes both representation and conversion
flags, so even a difference would concern the combined pipeline. It must not be
presented as proven resampler causation or a generally applicable product fix.
Holding a foreign session-control object has already failed in prior studies;
sharing a session GUID also requires cooperation by the rendering application,
so neither is an established arbitrary-program workaround.
Configuration interpretation follows Microsoft's [Initialize contract](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-initialize)
and [audio-session ownership model](https://learn.microsoft.com/en-us/windows/win32/coreaudio/audio-sessions).

### Actual receiver comparison after HDMI reconnection

`ready-device-inventory.json` confirms both exact LM22 and Mi Monitor endpoints
Active after the user's confirmation. Four native collections then complete
serially in A/B/B/A order on the guarded, explicit LM22 endpoint, using the same
frozen renderer, supervisor and independent endpoint reference. No FFT analysis
or build from this investigation runs concurrently with collection. Analysis runs
between collections; all raw recordings and earlier refusals remain unchanged.

| Run | Native representation / conversion flags | Stop edge difference, cycles 1 / 2 | Resume edge difference, cycles 1 / 2 | QPC gaps | Whole diagnostic |
| --- | --- | --- | --- | --- | --- |
| `pcm16-b` | PCM16, `0x88060000` (AUTO/SRC enabled) | −1984 / −1984 frames | +441 / +441 frames | 4 | FAIL |
| `float32-a` | Float32, `0x60000` (no AUTO/SRC) | −1984 / −1984 frames | +441 / +441 frames | 4 | FAIL |
| `float32-b` | Float32, `0x60000` (no AUTO/SRC) | −1984 / −1984 frames | +441 / +441 frames | 4 | FAIL |
| `pcm16-c` | PCM16, `0x88060000` (AUTO/SRC enabled) | −1984 / −1504 frames | +441 / +441 frames | 4 | FAIL |

All eight cycle alignments satisfy the comparator's credibility checks; all
violate the unchanged ±48-frame edge criterion. Every receiver reports an actual
480-frame buffer. Every supervisor, receiver and reference exits 0 with no forced
cleanup or controller cleanup failure. Reference reports contain no faults.
Successful process exits only establish collection completion, not audio success.

The independently audited original packet flags are **zero in every packet** in
all four runs. Thus the Float32 near-zero onset is not manufactured by the
diagnostic's SILENT-flag zero-fill branch. In both Float32 runs, the 441 samples
preceding each raw onset and the first 64 frames of its packet are exact zeros;
the remaining 416 samples in both channels correlate 1.0 with source frames
480–895. PCM16's preceding samples are at most magnitude 1; corresponding channel
correlations exceed 0.99999995. Float32 is analyzed before quantization, using the
fixed representation scale, not a fitted gain or synthesized missing prefix.

Independent `review/independent-abba-v3.json` verifies controller-pinned binaries
and helpers, target PID/creation time, native format/flags, buffer bounds, raw byte
coverage, reference endpoint identity and WAVs, recorded input hashes, process
exits and the observed packet onset. Root re-executes that audit into a fresh
directory and obtains exactly equal JSON; receipt: `root-abba-replay-receipt.json`.
The process/session guards are sampled checks, not continuous absence guarantees.

This reproduces the failure in a separate C++ receiver without FUNG's Rust WASAPI
wrapper, downmix, writer or timeline implementation in its acquisition path.
Removing AUTO/SRC and retaining raw Float32 does **not** remove the observed
first-period loss or QPC leaps. These observations weaken a conversion-flag-only
or FUNG-downstream-only explanation. They do not isolate a unique Windows internal
component, exclude every possible configuration, or prove behavior for every
foreign application. The owned renderer still destroys/recreates its stream;
an actual application's position-preserving pause may use a different lifecycle.

No unsuccessful receiver variant is promoted into production. No new FUNG
installer, installed PID/endpoint retest, merge or tag follows this experiment.
The next repair hypothesis needs evidence for a different upstream boundary;
repeating the same format switch or removing gap reporting is not a supported fix.

The delay is under the **source application's** native client ownership. FUNG
cannot be claimed to impose it on arbitrary foreign applications merely by
waiting longer in its own capture reader. Therefore this diagnostic change is
not promoted into product code. Restart still differs by approximately 441 frames,
and timeline gaps remain. No regression-backed arbitrary-PID repair is established.

The latest [installed PID failure and bounded endpoint/export/recovery
observations](2026-11-01-endpoint-export-recovery.md) remain separate. No product
code, FUNG installer, installed-package acceptance, merge or tag changed in this
experiment. Both complete installed modes must pass before merge/tag.

## Same-client Stop/Start discriminator — follow-up after documentation commit

Source context is documentation-only commit `17a01fb0439770b57e1f15abf18fbb7e5e61e037` plus the preserved dirty product tree. New diagnostic artifacts are under `E:\verification\fung-pid96-same-client-20261101`; production code is unchanged. The prospective `PLAN.md` distinguishes source Stop/Start on the same client from the prior destruction/recreation fixture. This is not a package repair.

The B renderer preserves the exact client, queued PCM and continuous source cursor. It calls Start directly on resume, before ordinary feeding, without Reset/reinitialization/rewind or feeding during the pause. The A renderer is the unchanged frozen destruction/recreation binary. Both use the same pinned raw Float32 receiver, supervisor, marker and independent explicit-endpoint reference. Keeping queued PCM and source position is part of the lifecycle intervention; client identity alone is not isolated from those state changes.

The initial `recreate-a` attempt was refused before GO/capture because the process-tree guard saw another process with the newly allocated renderer PID as its parent. That process's creation time preceded the suspended renderer by about four minutes, consistent with stale parent-PID reuse. No subject/reference capture launched, supervisor exited 9 cleanly, and no unrelated process was modified. The strict guard was retained unchanged; the refused receipt remains separate from actual audio runs.

Four actual runs completed serially A/B/B/A:

| Run | Source lifecycle | Stop difference, cycles 1 / 2 | Resume difference, cycles 1 / 2 | QPC gaps | Whole diagnostic |
| --- | --- | --- | --- | --- | --- |
| `recreate-b` | Destroy/recreate | −1504 / −1504 frames | +441 / +441 frames | 4 | FAIL |
| `same-client-a` | Same-client Stop/Start | approximately 0 / 0 | approximately 0 / 0 | 0 | NOT_PROVEN |
| `same-client-b` | Same-client Stop/Start | +1 / approximately 0 | −1 / −1 frames | 0 | NOT_PROVEN |
| `recreate-c` | Destroy/recreate | −1984 / −1984 frames | +441 / +441 frames | 4 | FAIL |

All eight transition alignments are credible; B's four measured edge pairs meet the unchanged ±48-frame gate. All actual controller processes exit 0 with no forced cleanup or reported reference faults. These exit codes mean collection completed. B remains NOT_PROVEN because edge parity/no QPC gaps do not prove full PCM custody, nonreplayed source continuity, isolation or installed workflow acceptance. In particular, source-coordinate overlap across B's Stop/Start can occur in both PID and endpoint reference; matching the reference does not prove that the renderer replayed no source samples.

Analyzer checks bind lifecycle label to renderer path/hash, ordered events and monotonic transition QPC brackets. B retains epoch 1, unchanged submission/source counters and a valid frozen source clock during pause; wrapped source markers are rejected. Nine analyzer tests pass, including invalid clocks, feeding during pause, reordered events and wrap rejection. New renderer builds with `cargo +1.99.1 build --locked --offline --release` into its separate target (final build exit 0, no warnings).

Renderer SHA-256: `EDE740C19B766BD757902C7900E66D0BDE7356ACF7F122C384C094AD1DE1BD20`; source SHA-256: `30B1E948146443824F6E4032692612156DBB4130A8BDAA4796BD854B748A8C9A`. `build-receipt.json` pins controller/analyzer; `study-summary.json` records the four runs and refusal. Root rechecks all 358 per-run input hashes. Guards remain sampled, final receiver tails censored and raw native observations separate from application qualification.

Inference: preserving source-client state removes the observed PID/reference edge deficit and QPC leaps in these bounded comparisons; destruction/recreation is a materially different case from same-client pause. This narrows the trigger/conditions but does not establish an internal Windows root cause or a mechanism FUNG can impose on arbitrary external applications. A follow-up could isolate Reset/queue/source-position effects while preserving identity; it has not been run. No production fix, new installed acceptance, merge or tag follows this study.

Independent offline audit (`review/independent-audit.json`) verifies all four runs' hashes, target PID/creation time, 117 inventory snapshots and mute guards, native Float32 format, packet/raw-byte custody, write continuity and source anchor correlations (minimum 0.999957). Root replay in `root-review-replay` produces identical JSON. B's endpoint source-coordinate overlap is approximately 424, 418, 400 and 428 frames across its four cycles, also reflected in PID. This is explicitly a parity result, not proof of uninterrupted/nonreplayed source delivery. No owned diagnostic/reference process remains at final inspection.

## API interpretation

Microsoft documents that [Stop](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-stop)
halts streaming through the client's connection and freezes its audio clock.
[ReleaseBuffer](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiorenderclient-releasebuffer)
releases a submitted render packet; a successful call alone is not our physical
output oracle. [IAudioClock positions](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition)
must be interpreted using their reported frequency, not assumed sample units.
