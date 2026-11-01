---
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification-report"
  scope: "Routed source export and recovery, with PID release gate retained"
  language: "English"
---

# Routed export and recovery candidate

The endpoint workflow previously reached the legacy export/recovery rejection.
This candidate adds explicit routed consumers while preserving that rejection
for legacy filename-based consumers. It does not establish a PID capture repair.

## Implementation

- Schema 17 adds typed source/epoch export bindings, output checksum/size,
  source manifest hash and frontier. Existing export artifacts remain compatible.
- Export verifies committed per-source chunks and writes separate mono PCM WAVs.
  Missing coverage is refused, not filled or mixed. An already verified export
  is reused without creating another staging directory. Failed/uncertain stages
  are retained; their cleanup remains an explicit reconciliation task.
- Recovery verifies a committed prefix against the known project root, source
  epoch, checkpoints, checksums and WAV frames before a conditional transaction.
  Unbound files are retained and return `needs_review`; proven corrupt media is
  quarantined. Verified prefix completion carries an interruption audit and is
  not a claim that uncommitted or future audio was recovered.
- Recovery reserves the native capture/playback owner for the whole operation.
  Export, recovery and quarantine serialize their final mutation through a
  process-local custody gate. Runtime denial is latched before quarantine releases
  that gate, including an uncertain durable commit.
- Source and output read handles deny new writes/deletes on Windows and are held
  through the final gate/transaction. This is not an atomic global filesystem lock.
- UI recovery wording distinguishes the routed states. Routed export currently
  exports WAV only and explicitly reports that source-aware subtitles are not
  exported; routed transcript recovery is skipped explicitly.

## Limits and remaining acceptance

Recovery retains up to 1,024 chunk handles (approximately 11 minutes 23 seconds
with three two-second sources). Larger sessions are refused. Export is limited
to 512 MiB per attempt and the completed manifest's descriptor bound. Failed
manual retries can retain disk data. The global custody gate may delay quarantine
while output files are hashed; no bounded wall-clock I/O claim is made.

The directory scan does not exclude a separate process creating an unbound file
after inspection. The process-local gate does not coordinate arbitrary external
filesystem writers. Copies already obtained outside FUNG cannot be revoked.

The installed candidate must still prove fresh capture, source separation,
playback, export/retry, forced-crash recovery and reopening on the new schema.
No current evidence qualifies full endpoint workflow or PID #96. Both modes must
pass before merge/tag/release; the original ±48-frame PID edge gate is unchanged.

## Evidence

- Frontend build passed and the recovery/desktop contract/review checks passed
  53/53 after the routed response union was added.
- The first focused recovery snapshot passed 11/11. Export code/schema changed
  during that compilation, so this is not verification of the final combined tree.
- Fresh read-only device admission at `E:\verification\fung-source-workflow-20261101`
  found LM22 and Mi Monitor active, nondefault, with no active renderer sessions,
  and muted. These are point-in-time observations, not audio acceptance.
- A pre-directory-fix complete library run finished **1,100 passed, one failed,
  15 ignored** in 350.26 seconds. The schema-chain test still omitted version 16
  while production installation included it; the test chain was corrected.
  This failed run is retained, not promoted to a complete-suite pass.
- Focused source export 5/5, schema checks 2/2, export listing 2/2, recovery 11/11,
  custody gate 1/1, Windows held file 1/1 and deterministic deadline 1/1 passed
  before the subsequent directory implementation changes.
- The final combined candidate passed **1,103 library tests, zero failed,
  15 ignored** in 388.07 seconds, followed by strict all-target clippy with
  `-D warnings` (both exit 0). These are source checks, not installed acceptance.
  `candidate-final-inputs.json` binds 257 application/configuration inputs;
  dependency/toolchain and bundled model bytes are outside that manifest.
- The own-process RAW capability probe returned `E_NOINTERFACE` for IAudioClient2
  before any stream initialization. It supplies no PID repair; see the
  [PID diagnostic](2026-10-29-pid-stop-hold-diagnostic.md).

Plans: [source export](../../plans/2026-11-01-routed-source-export.md),
[source recovery](../../plans/2026-11-01-routed-recovery.md).

### Directory mutation review

The first package build was cancelled before installation after review found
path-based output creation could follow a concurrently changed ancestor. Holding
directory handles without delete sharing prevents rename, but actual owned-temp
Windows probes demonstrated that `FILE_WRITE_ATTRIBUTES` can still set a junction
on an empty held directory, even without write sharing. Therefore that pinning
candidate does not close the namespace race.

An owned-temp `NtCreateFile` probe using a held `RootDirectory`, a single relative
name, `OBJ_DONT_REPARSE` and `FILE_OPEN_REPARSE_POINT` successfully created ordinary
children. After the held empty parent was converted to a junction, the relative
create returned `0xC0000280` (`STATUS_REPARSE_POINT_NOT_RESOLVED`) without creating
the child in the target. A pathname control did create the target child. Nested
relative directory `OPEN_IF` and file `CREATE` controls also passed. These are
host-specific API experiments, not product or all-Windows-version qualification.
The final candidate integrates handle-relative creation and reuse, including
returned-handle reparse/identity checks. Its seven Windows source-export tests
passed in the combined suite above. An existing-junction regression is integrated;
the exact convert-after-pin race was exercised by the external API experiment.
The replacement package build and installed acceptance are separate gates.

Scripts and evidence are in `E:\verification\fung-source-workflow-20261101`;
only owned temporary directories were modified. The standalone exported-PCM
verifier has four passing fixture tests, including swapped-source output with
updated output checksums. It has not yet qualified an installed export.

### Installed candidate 1 (before diagnostic-directory separation)

The release-profile NSIS build completed. Installer SHA-256:
`ABDE9974752A83D2964E46699D052C185EC1B28A75EA3047E42B7BB9B9E53989`.
Installed executable SHA-256:
`5CDC2B4DAB1A625492E9E1E2CADC67D618F39124DB01AB5A7D9AC6469300CB01`.
An initial exact-hash comparison failed because Tauri patches the bundle marker
from `__TAURI_BUNDLE_TYPE_VAR_UNK` to `__TAURI_BUNDLE_TYPE_VAR_NSS` in the packaged
executable. Byte comparison confirmed exactly those three changed bytes and
identical remaining content. Both the failed receipt and explicit reconciliation
are retained; the original isolated profile was unchanged during installation.

The installed main WebView exported the old two-source recording twice, reused
the same typed artifact identities/paths/checksums, completed paused A/B/A source
playback commands, and exited normally (code 0). This proves those backend flows;
PCM byte correspondence and fresh capture/recovery are separate gates.
The subsequent offline verifier passed exact source PCM correspondence for both
exports using a disposable copy of a stopped-profile snapshot and typed source
bindings. It does not establish independent journal verification or new capture.
Opening a read-only SQLite connection modified only the first backup's SHM file;
the live profile was unchanged. That backup was preserved, a fresh prelaunch
backup taken, and analysis now uses a disposable copy rather than the guard copy.

Fresh installed PID capture (`23267c7b-10c2-4d7c-b93d-8bffe731a82f`) still failed
the frozen PCM edge test: Stop differences −2,464/−1,984 frames and restart
differences +921/+441 frames, against the unchanged ±48-frame limit. The app
exited normally, but capture carried `capture_source_fault`. Controller completion
does not convert this into a capture or release pass.

The separate session-control-pin experiment also failed: Stop differences
−1,984/−1,945 frames and restart +441/+441 frames. The owned sidecar exited 0 and
logged sampled session references; neither continuous pin coverage nor a PID
repair is established. It did not change the PCM oracle or sample repair policy.

Fresh installed endpoint capture `9fdf12d1-a27d-485d-942b-3dbf298ef060` failed
before the visible-frame gate: the 64-packet writer queue overflowed at frame
126,960. The runtime retained the terminal packet, stopped capture and reported
partial state. The controller then deliberately terminated its owned UAT app;
the launcher message about early app exit is not evidence of a spontaneous crash.
Owned producers exited 0 and controller cleanup reported no failures. This run
does not qualify endpoint PCM, export of the new recording, or crash recovery.
Candidate 2 adds bounded 250 ms packet coalescing and timing observations, but
requires fresh sustained installed testing rather than inferring success from
the earlier endpoint component pass.

Runtime diagnostic spools inside the legacy `live` recording tree are not bound
WAVs and therefore keep recovery at `needs_review`. Candidate 2 source work
separates new non-authoritative diagnostics from that tree; candidate 1 evidence
does not qualify candidate 2, which requires fresh checks and packaging.

### Candidate 2 source and verification boundary

New private spools are created under the project's `diagnostics/routed` tree,
outside authoritative `live` recordings, using shared handle-relative filesystem
helpers also used by export. Existing files are not moved or deleted; unknown
files still inside `live` keep recovery at `needs_review`. Diagnostics never
become recovery/export input or a substitute for uncommitted audio.

The routed writer receives at most 12,000 contiguous frames per packet. Its
64-slot queue then holds at most 768,000 mono samples (1.536 MB) from this caller,
plus bounded assembly/terminal buffers. Per-native-packet spool sync is retained.
A fault observed after file write but before ledger admission prevents that
admission. A commit already admitted may overlap fault notification; final
quarantine and completed-source admission remain necessary. No absolute
zero-commit-after-fault guarantee is claimed.

The initial combined run finished **1,109 passed, one failed, 15 ignored** in
278.71 seconds. The new overflow test incorrectly required an orphan file even
though the existing writer can retain the terminal PCM in quarantined bound
chunks. The correction checks every PCM sample of all three packets and denied
consumer access. Only `routed_capture_tests.rs` changed; it is absent from the
release dependency file. Build-start and final-test source manifests and the
test-only transition receipt are preserved separately in
`E:\verification\fung-source-workflow2-20261101`. Final rerun/clippy and installed
sustained capture/recovery remain pending at this checkpoint.

The final combined rerun subsequently passed **1,110 tests, zero failed,
15 ignored** in 226.40 seconds. Strict all-target clippy with `-D warnings`
passed (exit 0), and the NSIS package completed. Its installer SHA-256 is
`A11C531C179787E323E7B1D3658C9A7B1F4A7E162AC1E6E4555C25EDCDE0115F`;
expected installed executable SHA-256 after the exact NSIS marker patch is
`115231BB8FC5BE38969BDCB13CA3C2B5F3FAC9E8B7E4FE756BD889D491C047D6`.
These source/package checks do not close the installed audio gates.

### Candidate 2 installed sustained capture and recovery diagnostics

Installation verified the expected executable bytes and preserved the isolated
profile. The new sustained endpoint run did **not** pass: recording
`1ffc6b04-9b1e-4888-93a7-0a4fadd7fbcb` stopped with a native packet-sink
`QueueFull` after about 20.25 seconds of durable PCM per source. Each source
reported 1,938 accepted native packets and 972,000 durable frames. Maximum spool
sync took 144,552/151,736 microseconds; maximum ledger commit took
377,396/456,689 microseconds. These observed timings support investigating the
per-packet sync load; they do not establish exclusive causality. The controller
terminated only its owned app after failure, and both owned producers exited 0
without cleanup failures. No sustained-capture or new-recording export pass is
claimed.

A separate short capture deliberately terminated the same installed app after
observing at least 96,000 committed frames per source. The controller completed,
but the launcher incorrectly retained its normal-close exit-zero assertion after
`TerminateProcess(1)`. Raw failure evidence remains unchanged; the separately
reviewed `deliberate-crash-adjudication.json` identifies only that exact harness
defect. It does not qualify recovery. The first reopen attempt then failed in
the verifier before recovery because its ordinary Windows file open could not
read a 271-character path. This is a harness path-handling failure, not evidence
that the audio file was missing. Fresh reopen verification is still required.

### Candidate 3 batch durability change

The next candidate appends each native packet privately and syncs both spool
files immediately before each at-most-250-ms coalesced writer transfer, including
the validated final residual. A failed sync prevents transfer and poisons the
source; the pending PCM is retained privately. Fault state is checked again after
the potentially blocking sync. The interval after that check is not an atomic
fault/publication barrier; writer admission and quarantine guards remain required.
Before batch sync, the uncommitted tail is volatile and is not claimed recoverable
after a crash or power failure. Diagnostics never become authoritative recovery
input. Independent static review found and verified the post-sync recheck fix;
fresh compilation, tests and installed sustained acceptance are pending.

### Candidate 2 recovery and recovered export follow-through

After correcting only the verifier's long-path handling, independent review
checked the fresh retry harness and its pinned adjudication evidence. Both
installed reopen runs returned `recovered_verified` for recording
`7b4ab53d-ffd6-4bbb-864a-1da7678f352c`: two sources, six committed chunks and
6,000 ms. All six WAV hashes remained unchanged across both runs, the second
recovery returned the same result, and each owned app exited 0. This verifies a
committed prefix after the deliberate process termination; it is not a
power-loss test or recovery of uncommitted tail audio.

The installed recovered-recording export then passed two-source WAV generation,
repeat artifact reuse and paused playback commands. A separate verifier compared
each entire exported PCM payload with its own ordered source chunks and typed
export binding: `PASS_SOURCE_PCM_CORRESPONDENCE`, two sources. The verifier used
a disposable copy of a stopped-profile snapshot, and the source/guard snapshot
were unchanged afterward. This correspondence check reads the projection and
actual WAV bytes, not an independently reconstructed journal. Evidence remains
under `E:\verification\fung-source-workflow2-20261101` in `audio-reopen1retry`,
`audio-reopen2retry` and `export-recovered-run`. Candidate 2 still fails sustained
capture, and these results do not qualify candidate 3 or PID capture.

The same installed candidate subsequently passed a negative consumer test of
both sources from the failed sustained recording. Catalog and both source-open
requests returned the expected source-trust errors. One deliberately created
export job (`ae33b80e-a1e4-41f3-ab79-11405d680f00`) failed with the exact
`routed source not completed/trusted` message; no artifact IDs for those two
sources appeared before or after the job. The failed job remains in the isolated
profile for audit. `errorCode` was null, so the harness checks the actual persisted
error message and does not invent a structured code. The owned app exited 0.
A separate stopped-profile projection copy contains both sources in `quarantined`
state with their original `QueueFull` reasons. This is persisted projection
evidence, not independent journal reconstruction. Receipts are in `audio-negative`.

### Candidate 3 source/package checkpoint

The complete four-worker library run passed **1,112 tests, zero failed,
15 ignored** in 367.20 seconds. An earlier serial run was deliberately interrupted
to avoid unnecessarily serializing storage fixtures; its log, nonzero exit and
owned-process interruption receipt are retained, not counted as a passing run.
The build-start and final manifests match across 258 app source/config inputs
(toolchain/dependency/model bytes are outside that manifest). The NSIS build
completed, with installer SHA-256
`65932D86B55B7771F0A706133B87258A8AF1D22CE16A4A043BF614F2595BA12A`
and expected installed executable SHA-256
`0CEB57CF6500E2CC4DC99A9FDF831348885FFE617EC8C885818F1836F4303CFD`.
Strict all-target clippy with `-D warnings` subsequently passed (exit 0, 1m 34s).
Installed acceptance is still pending at this checkpoint.
Evidence root: `E:\verification\fung-source-workflow3-20261101`.

### Candidate 3 installed observations

Installation matched the expected executable hash and left the isolated profile
unchanged. Endpoint recording `44bdaefe-2ec0-4c71-8ed5-b21d6e155642` completed
at 71,891 ms, including a 60.55-second sustained hold after two transition cycles.
Both sources drained and finalized without a capture fault: 36 chunks each,
3,450,783/3,450,682 durable frames, and 7,039 accepted native packets each.
The batch sync count was 288 per source; maximum observed batch sync durations
were 4,071/5,128 microseconds. This is one host observation, not a timing guarantee.
The full covered native-to-WAV PCM comparison passed, as did installed two-source
export/reuse/paused commands and exact source-to-export PCM correspondence.

The original 30-second reference repeats during the longer third epoch, however;
the frozen linear matcher cannot credibly align that epoch and reports
`NOT_PROVEN` for the second transition and leakage. That original result is
retained. It is not promoted to a full PCM pass. A separate 180-second marker
protocol preserves the analyzer and the 48-frame/0.001 gates while avoiding a
repeat during the capture interval.

A fresh deliberate crash on candidate 3 (`f35d9d17-3448-4b0f-9de1-f91c394ce2b2`)
and two clean reopen runs verified a six-second prefix: two sources, six chunks,
all hashes unchanged and identical repeated recovery outcome. Recovered-recording
export/reuse/paused commands and exact source-to-export PCM also passed. No claim
is made for an uncommitted tail or power-loss recovery.

Fresh installed PID recording `c47e1fb7-c35c-4849-8a36-a1c9f347d9e5` still fails:
Stop differences are −1,504/−1,984 frames and restart differences +441/+441.
The app exits 0 and owned cleanup completes, but the recording carries
`capture_source_fault`. The unchanged ±48-frame gate is not met. This candidate
does not fix issue 96, and merge/tag/release remain blocked.

### Test-fixture failures and artifact incident

The first 180-second-marker attempt stops **before capture**, because the owned
producer still limits input WAV duration to 60 seconds despite its 180-second
execution limit. No recording was started. Its exit 1 and the launcher's subsequent
owned-app termination are preserved in
`E:\verification\fung-source-workflow3-longmarker-20261101`.

While preparing the corrected fixture, an agent mistakenly reused its previous
Cargo target and overwrote producer artifact `C223F10D...`. Rebuilding the old
source did not restore identical bytes. The original artifact remains unavailable;
historical capture/PCM evidence and its pins are unchanged, and old pinned native
harnesses refuse the replacement. Details and the explicitly named non-original
rebuild are in `E:\verification\fung-owned-producer-longwav-20261101`.
The new fixture is built in a separate target and staged outside Cargo output at
an immutable SHA-named path (`D22F735B...`). Its only production-source change is
the WAV length cap and error text; five instrumented pre-device loader cases pass.
Those checks are not a live-audio pass. Fresh retry evidence is kept separately
under `E:\verification\fung-source-workflow3-longmarker-retry-20261101`.

### Candidate 3 long-marker endpoint and export result

The fresh retry uses the same installed executable (`0CEB57CF...`) and the new
immutable producer (`D22F735B...`). Recording
`ffda653e-21d4-47f8-8c7c-6ae9f62a929a` completes **74,974 ms**, including a
62.125-second hold after two Stop/Resume cycles. Both owned producers and the app
close cleanly, with no capture failure or cleanup errors. An independent review
checks candidate/helper/controller/input identities, route/source/epoch bindings,
owned render PIDs and the unchanged policy against the saved receipts.

The frozen analyzer reports **PASS for both source PCM components and all four
transition checks**. Stop errors are approximately zero; Resume errors are about
41 and 27 frames respectively, within the unchanged ±48-frame gate. Leakage
upper bounds are `1.0695938044881877e-6` and `8.846469705034992e-7`, below 0.001.
Covered native PCM reconciles with all corresponding WAV samples, with no covered
sample mismatch or native samples beyond the WAV. The analyzer's overall verdict
remains `NOT_PROVEN`: it does not establish ledger, UI, export or recovery by
itself. Its uncovered idle placeholders are not independent proof of silence.

The marker duration is 180 seconds so the third active epoch does not repeat.
Regenerating the original 30-second recipe matches the old files sample-for-sample;
the first 30 seconds of the new 180-second FFT-normalized files are **not** the
old files. The marker fixture and producer WAV-duration limit changed; analyzer
matching math and PCM thresholds were not relaxed. The original repeated-marker
result and failed loader attempt remain.

Installed export from this same recording then passes two-source WAV creation,
repeat artifact reuse and paused backend playback commands. Full exported PCM
matches the ordered committed source chunks and typed source/epoch bindings for
both sources (`PASS_SOURCE_PCM_CORRESPONDENCE`). The owned app exits 0 without
forced termination; its stopped-profile snapshot remains unchanged after the
disposable-copy verifier. Evidence is under the retry root in `audio-endpoint`
and `export-endpoint-run`, including `pcm-verification.json`.

Together with the separate six-second crash/reopen/recovered-export observations
above, this qualifies the measured endpoint backend paths. It does not establish
audible/rendered playback UI, transcription/enrichment, independent journal replay,
power-loss recovery, arbitrary-duration recording, or physical isolation against
other programs deliberately routing to the same endpoint. PID still fails, so
**issue 96 and both-mode release acceptance remain open; no merge or tag**.

An independent PID follow-up review found no supported corrective API/configuration
in the approaches investigated that preserves arbitrary external-client stream
boundaries. This is a limit of the investigation, not proof that Windows is the
cause or that a repair is impossible. Endpoint batching does not repair the PID
edges. Comparing the frozen protocol on another Windows host/build would help
separate host-specific behavior, but the user has only this machine; that comparison
is not available and is not counted as executed. Endpoint mixing, manufactured PCM
or a relaxed edge tolerance cannot substitute for the required PID result.

### Formatting and source provenance after installed tests

The installed tests above are bound to the frozen 258-input candidate manifest.
A subsequent `cargo fmt --check` found import/module ordering and line wrapping
in five files. All 258 original inputs were hash-verified and copied into
`preformat-source-snapshot` before formatting; the external UAT config has an
explicit snapshot-index entry. The first snapshot attempt stopped on that external
path before any formatting; the completed retry verified every copied file.
Formatting now passes. The current formatted tree has not been packaged or
installed again; do not label the existing installed evidence as exact-byte
qualification of this later tree. The frozen manifests, installer and source
snapshot remain available for reproducing the tested candidate.

After formatting, strict all-target clippy (`-D warnings`) also exits 0. The
1,112-pass library run belongs to the frozen pre-format candidate; it is not
represented as a new post-format test run. `postformat-only.diff` records only
the five formatter changes. Local link-path verification across the eight work
documents checks 309 targets with zero missing; anchors and remote links are not
checked. The final process inventory finds no remaining owned FUNG UAT app or
marker producer. No commit, merge, tag or release was performed in this work.
