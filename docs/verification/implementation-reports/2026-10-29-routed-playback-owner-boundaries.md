---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-30"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Completed source reader prerequisites and isolated SDK ownership test"
  language: "Thai-first"
---

# Routed playback: source access and quarantine boundaries

**Source-aware playback and quarantine cancellation are not enabled in the product.**
The private reader passed 12 fixture tests, with its subprocess helper ignored in
the parent run. The player's contract limits metadata to 20,000 descriptors and
8 MiB; it does not impose an 8 MiB total SDK allocator peak. Existing queries
materialize pages before accounting, so no overall peak-memory claim is made.
The HQL2 principal and parity investigation remains useful evidence, but an
additional 8 MiB whole-SDK peak gate was an earlier mistaken interpretation.
The reader and quarantine have focused fixture coverage, while callback queue
contention and retained-accounting follow-up remain under independent review.
Playback integration and installed acceptance are still open. Evidence:
`G:\AI-Studio\verification\fung-completed-source-reader-20261029\sdk-boundary.md`.

The first call-trace audio attempt is **inconclusive**: the PID subject timed out
before writing its trace and the reference endpoint also recorded a long gap.
It cannot prove either a PID fix or a valid comparison. No installed PID or
endpoint acceptance, merge, tag or release is claimed from this attempt.

After both HDMI endpoints were restored, fresh native diagnostic runs
`calltracebaseline03` and `calltraceheld03` each completed the separate endpoint
reference without reported faults but the PID probe failed with four and three
QPC discontinuities respectively. The offline analyzer matched 1,997 packets in
each run. The anomalous inter-packet windows were dominated by elapsed time in
the wait call; that timing also includes possible thread descheduling and does
not establish an internal Windows cause or identify missing PCM. This is
test-only component evidence, not an installed application acceptance or repair.
The independent offline PCM analysis also returned overall FAIL for both. For
the immediate-destruction arm, both cycles lost 1,984 frames at the Stop edge
and showed a 441-frame resume discrepancy. Holding the owned stopped stream
removed the measured Stop-edge loss in both cycles, but the first resume still
showed a 441-frame discrepancy and the subject still reported three gaps.
Original stereo, independent downmix and writer bytes agree in both runs; these
results do not justify changing the gap rule or retaining arbitrary app streams.

## Bounded writable-mapping probe — 2026-10-31

An independent Win32 probe used three new, owned 4 KiB synthetic files on
NTFS under `E:\verification\fung-writable-map-audit-20261031`. Each case
created a writable mapped view before attempting a `GENERIC_READ` open with
`FILE_SHARE_READ`, matching the reader's relevant sharing flags. The read open
returned Windows error 32 (`ERROR_SHARING_VIOLATION`) in all three cases: while
the writer and mapping handles were open; after closing the writer handle; and
after closing both handles while retaining the writable view. In each case the
mapped mutation and flush succeeded; once all mapping references were removed,
the reader opened and observed the changed byte.

This is a **same-process, one-host NTFS fixture**, not the Rust verified WAV
reader, a cross-process mapping test or a test on other filesystems. Because the
reader could not open during the writable view, concurrent seek/read while that
view was active was not tested. The probe does not prove protection against all
mapping or bypass mechanisms and supplies no native audio or installed-package
acceptance. The prior cross-process ordinary writer/rename tests are separate.
`probe.py` SHA-256
`CE907CBD474F8B265FEF912882C46FE70196504CBFA2D1695AABDC3ACEE93B8F`;
`receipt.json` SHA-256
`732C49CEB55E6A47FF599C07F5AB4E6298717F6D1A57D7924B716C2DB850EA4C`.

## Isolated Windows SDK ownership test

Root ran `cargo +1.99.1 test --test custody_lock_cross_process -- --nocapture
--test-threads=1` using a newly created token-checked test root under the task's
verification directory. Result **1 passed, 0 failed, 1 ignored** (the ignored test
is the explicit subprocess helper), exit0. The parent asserted seven child cases:
free writable open, writable/read-only opens blocked while owner holds the same
lock file using normal and extended path aliases, and both open modes after owner
release. Aliases were verified to have the same Windows file identity. Fresh PID
and receipt per owned child and bounded cleanup distinguish execution from skips.

Test source SHA-256 `521B83AD9D7B6948EBBF8E8134382258852A79CB278C17441C98B533162EAC39`;
log SHA-256 `291AD83E104A6EA838F5B09CC914E8B9B3736401B17DCA980D12F7464E989D7F`.
Both are retained under `G:\AI-Studio\verification\fung-completed-source-reader-20261029`.
The helper is external evidence: it hardcodes this host's task root and is not a
portable CI test. Its successful temp workspace was cleaned after child exit.
The SDK reports a common error for all lock-acquisition failures; the result
demonstrates cooperative SDK exclusion, not the precise underlying OS error,
protection against lock bypass, or playback queue cancellation.

The [candidate contract](../../specs/2026-10-29-routed-playback-quarantine-contract.md)
was independently reviewed and corrected for post-close quarantine retries,
same-handle mutation and callback/ack guarantees. Public source-selection wire,
cross-process visibility, commit/cancel/ack ordering and bounded query authority
remain owner/interface decisions; required contract/backend reviews are open.

PID #96 remains FAIL: the owned stopped-client experiment did not produce a
full passing run, and no supported mechanism to retain an arbitrary foreign
client was established. Neither installed audio mode was retested in this
checkpoint. No new package, merge, tag or release is claimed.

## Two simultaneous PID consumers, 2026-10-30

An isolated diagnostic used two PID-loopback probes against one owned renderer,
with a simultaneous dedicated-endpoint reference and repeated stop/destruction
and restart. Both PID probes exited 101 with four and five reported timeline
gaps respectively; the endpoint reference and renderer exited zero, with no
forced cleanup. Each subject's original stereo downmix and retained writer PCM
matched byte-for-byte. The marker fits at the transition edges were not credible,
so their numeric estimates are **NOT_PROVEN**, not measured edge losses. The
experiment does not establish a root cause or a repair and is not an installed
package test. Evidence is retained under
`G:\AI-Studio\verification\fung-pid96-dual-consumer-20261030\dualbaseline01`
and its adjacent subject analysis files. Both installed mode gates remain open.
The packet-QPC intervals corresponding to all four anomalies from the first
probe overlap intervals from the second; the second also has an additional
anomaly. This is a conditional timing observation pending clock comparability,
not proof of common cause or a measurement of missing PCM.

Private playback fixture coverage now includes fractional-millisecond duration
and seeking to the exact frame endpoint: the new regression was observed failing
before the change and passed afterward, along with separate duration-rounding
and one-frame queue tests. The earlier routed-source fixture suite passed 14
tests before that change. These are not actual output-device or installed-app
tests. Independent review found that slow media verification can starve the
ring and report a terminal underrun, and that close can wait for in-flight
verification. Buffering, close cancellation, output-device behavior and a
coherent state snapshot remain under review before any public activation.
After private cancellation and quiescence changes, the routed-source fixture
suite passed 24 tests and the private SPSC ring suite passed 10 tests. An
independent review confirmed the output and producer gates are acquired in
separate scopes on this cleanup path. These results do not establish survival
of failed cleanup scheduling or that a real output device has stopped.
The private close prototype also has an unresolved ownership path: dropping an
unpolled close future or failing to spawn its cleanup owner can leave no owner
to join the worker and release the capture guard. Its routed cancellation latch
does not prove cleanup, and legacy playback may continue in that failure path.
Real callback behavior, blocked OS I/O, scheduler rejection and retry after such
failures remain unverified; do not claim close lifecycle completion.

On 2026-10-30, CPU-only regression reproduced a legacy close future dropped
before polling without a cleanup owner; that test remains failing while the
central ownership redesign is unfinished. A separate regression reproduced an
open-reaper worker panic releasing its admission guard. A narrow fix retains
the guard and opening state when either join fails; four focused tests passed,
including a delayed second join and shutdown. This does not repair open-reaper
spawn failure or qualify the complete playback lifecycle. See the
[candidate ownership plan](../../plans/2026-10-30-playback-close-ownership.md).
Subsequent CPU-only work made the normal close regression pass and ran 14
focused close tests. Independent review then identified a shutdown-versus-spawn
failure race; its regression remains failing. An isolated capacity-one owner
slot passed its own fixture, but is not connected to production admission. A
separate unpolled-open regression also fails. Process-lifetime ownership,
opening/reaper handoff, and shutdown acceptance therefore remain incomplete.
