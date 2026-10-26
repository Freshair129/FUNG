---
version: "0.1.0b"
created_at: "2026-10-26"
last_update: "2026-10-27"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Exact-frame source seams and failed n01 native diagnostic"
  language: "Thai-first"
---

# Routed native seams and n01 diagnostic

Working-tree successor to `8bf5af1`; source identities are recorded below. This
report separates reusable Rust seams, a test-only native probe and the failed
owned native run. Task 2 remains **PARTIAL**. Neither installed endpoint nor
original PID acceptance has passed; both remain release blockers by user decision.
Authority: [routing design](../../specs/2026-10-25-dedicated-endpoint-routing-design.md)
and [implementation plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md).

## Source implementation boundary

- `live_system_audio` accepts a typed opaque exact endpoint ID with no default
  fallback. Existing friendly-name behavior remains available for legacy callers.
  The optional private sink uses the same native reader/PacketClock, not a second
  decoder or a reconstruction from floor-millisecond chunks.
- `endpoint_packet_sink` binds UUID source, integer epoch, rate and one explicitly
  supplied common future QPC barrier. Post-Start readiness and actual nonempty
  prebarrier stream observation are separate requirements. Ceil-frame private
  preroll clipping retains original native span/QPC/index/flags, clipped counts
  and rational phase. The pure transform's residue is below one sample; physical
  cross-device alignment and original timestamp uncertainty remain native gates.
- Late start is StartupMissing; late first observation or a negative first QPC
  anchor clamped by the legacy clock is StartupUnverified. Buffered PCM remains
  private evidence. No missing interval, initial silence or clean synchronization
  is invented. Queue length **and allocation capacity** are bounded; final tails,
  exact gaps and first failures survive finalization.
- `capture_source_writer::try_push_recoverable` transfers a rejected packet's
  original ownership back or reports the single terminal packet already retained
  by the writer. Returned ownership is not validation or publication authority.
  A fault never reopens normal writer admission. An upstream accepted queue still
  needs explicit private draining after quarantine; its bytes are not covered by
  the writer's internal accepted-queue drain.
- `routed_native_probe` is compiled only under `cfg(all(test, windows))`. It is an
  explicitly invoked ignored owned-endpoint experiment, not a Live/UI/MCP capture
  command. Its private diagnostics/reopen scaffolding cannot establish production
  consumer fencing, recovery/export support or installed acceptance.

## Executed n01 — failed, audio acceptance not assessed

Evidence root:
`G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe`.
Controller `controller-runs/n01/controller-report.json` records an owned two-HDMI
run without keepers, 12,000-ms requested native duration and 5,000-ms declared
lead. The probe exited 101 before a valid ready receipt; `executionCompleted=false`
and audio acceptance is explicitly NOT_ASSESSED. This is not a source-separation,
Stop/recreate or native synchronization PASS. Post-volume comparison passed in
that controller receipt; no broader device/settings result is inferred.

The failed private DB remains under `runs/n01`. A separate copied-DB, native
read-only Genesis SDK diagnostic at `diagnostics/n01-sdk-copy-01` recovered the
persisted first causes without directly querying SQLite or migrating the DB:

| Recorded order | Persisted result | Interpretation boundary |
| --- | --- | --- |
| Source A, `33e46cd5-4f7b-4504-b1db-44f9c60901e7` | `UnknownSession`: active session is not an explicitly allowed current renderer, at `2026-10-26T16:13:33.593849100+00:00` | Primary persisted guard failure. The rejected runtime owner/PID/session identity was not persisted; the DB does not identify the owner or prove why it appeared. |
| Source B, `7b9a544a-9000-4e90-af42-364887e63245` | source ended without visible PCM, at `2026-10-26T16:13:33.623652+00:00` | Later stop consequence, not independent proof of startup failure. |

At frontier 22 both source rows are quarantined, the recording is partial and
there are zero authoritative audio/source-chunk rows. Registration metadata names
the intended admitted renderers; it does not identify the later rejected owner.
Do not infer that FUNG's own loopback capture caused the rejection or broaden
allowed sessions from this evidence. Owner attribution and any policy decision
need separately preserved native observations.

Read-only evidence: `native-sdk-evidence.json`, `diagnostic-summary.json` and
`evidence-hashes.json` in that diagnostic directory. Their receipts report the
original hashes unchanged and the copied preexisting eleven files unchanged;
SDK-created projection sidecars are disclosed separately. SDK revision is
`2cf485573d48d0e5e2ae76db647ec4c4dee543c3`.

## Separate m01 metadata experiment

Root executed the independently reviewed, frozen session inspector on the exact
LM22 endpoint after native preflight required active/render, muted, nondefault
and known idle. This apparatus Initialized/Started/Stopped an unread loopback
client; it did not read PCM, render audio, change settings or control the desktop.
Evidence: `G:\AI-Studio\verification\fung-native-session-inspector-20261026`.

Both null-GUID and fresh private-GUID cases completed. Independently recomputed
opaque instance equality found exactly one active enumerated instance matching
the client's own GetService session at all three observations, with raw
GetProcessId S_OK and the inspector's own current PID. The private instance equaled
the manager's private-GUID instance and differed from its process-default instance;
the default instance remained inactive in those private-case observations. Both
cases returned to known idle after successful Stop/drop. Mute samples before,
between and after remained true; final default roles were not measured. Requested
0/50/200-ms observation deadlines were not hard timing bounds: actual Start-return
to Stop-return durations were 260.538 ms and 307.7819 ms.

This proves that this owned loopback client can itself appear active in the session
inventory and that the private/default instances were distinct in this experiment.
It does **not** retrospectively identify n01's rejected owner, prove source
separation or authorize excluding all sessions owned by a FUNG PID. A narrowly
owned native-session lifetime contract and negative tests must precede any runtime
admission change; initial client-supplied route validation stays strict.

- Frozen inspector EXE SHA-256: `4B2EC9F20D5EC36CF592AB3F709B24B8B21F35511BA5E7FEBE039E0BF9E7D042`.
- `m01.json` SHA-256: `AD41DD09D60B6E924A6090832C918EEB12A13B65071AABAAA2149F57BB503E6A`.
- Independent analysis: `m01-independent-analysis.json` and `.md` in that directory.

## Diagnostic defect and source verification

The original probe's final chunk query requested limit 10,000, exceeding the
actual Genesis SDK limit 1..1,000. This secondary query failure masked the useful
first-cause report; it does not explain or repair the UnknownSession guard failure.
The corrected probe uses bounded recording-scoped paging and persists collected
source diagnostics before later queries/cleanup, so subsequent query errors stay
supplemental rather than replacing the first cause.

| Evidence | Recorded result | Scope |
| --- | --- | --- |
| `integrated-sdk-limit-red.log` | FAIL: actual SDK `REL_QUERY_LIMIT_EXCEEDED` | Retained regression reproduces the original 10,000-row request; no audio. |
| `integrated-probe-sdk-green.log` | 6 passed, 1 ignored | Probe module source/input/SDK checks; ignored native experiment is not acceptance. |
| `G:\AI-Studio\verification\fung-writer-recoverable-20261026/integrated-recoverable-tests.log` | 17 passed | Integrated writer subset, including ownership-preserving rejection; no new native qualification. |
| `G:\AI-Studio\verification\fung-endpoint-routing-20261026-task2/endpoint-adapter/anchor-clamp-stage/final-tests.txt` | 38 passed | External combined pure seam/selector/legacy helper harness; not the full integrated product suite. |

The integrated full Rust library suite at the source snapshot below completed
with **777 passed, 0 failed, 12 ignored** in 806.43 seconds; receipt
`integrated-full-library-current.log`. Ignored native/provider/manual tests remain
unrun, not acceptance passes. Strict all-target Clippy at that snapshot failed
only on the test probe's nine-argument `source_worker`; receipt
`integrated-clippy-current.log`. A structural correction needs separate verification
before any clean-check assertion. Formatting and diff checks passed; 220 changed
documentation relative file links resolved (anchors/rendering not assessed).
Earlier 733-test writer results remain historical. No inferred CI, installed or
release pass follows from any of these source checks.

## Remaining gates

### n02 exact rejected-owner follow-up — 2026-10-27 local

The diagnostic-only successor obtains one fresh inventory and validates that
same object. Selected-source evidence is retained on rejection; unrelated
endpoint sessions are omitted. Observer process metadata never enters the route's
authorized renderer/child list. The regression was measured RED when rejected
endpoint evidence was deliberately omitted, then GREEN after restoring it.
The final probe subset passed **9 tests, 0 failed, 1 native test ignored**;
all-target Clippy passed after mechanically grouping worker inputs. Receipts:
`n02-exact-admission-diagnostics/integrated-exact-rejection-{red,green}.log`
and `integrated-clippy-green.log`. These follow the earlier full-suite snapshot;
the full suite was not repeated after these test-only diagnostics/refactoring.
Successor probe source SHA-256:
`0316FCD9778FF0A77E790CFEF96F993476F78ED94F0C20214064DD58E2931397`.

Root then executed frozen native binary SHA-256
`71131D7049E0F830B70C9304E290730135E2CAF54E1458627ECB0221E25DB3DF`
under the unchanged owned/muted two-endpoint controller. Run `n02` again failed
before ready/common-barrier publication. Its complete report, collected source
diagnostics, chunk query and reopen receipts were retained; the chunk query
succeeded with no rows and supplemental failures were empty.

The first rejected source was `cf06d93b-e7fc-4c54-8011-58d0b03b0b6f` on LM22.
Its same-input snapshot contains the allowed renderer PID 672 active, plus an
unallowed active session whose complete owner identity equals the observer's
identity: PID 18124, creation FILETIME `134375078905355023`, image volume serial
`1560106192`, file ID `00050000000b0d00` and the frozen n02 probe image path.
This identifies the rejected **process generation** in n02 without parsing opaque
session IDs or guessing from PID alone. The snapshot on source B saw that same
process's session inactive and accepted the intended renderer at that moment;
the global stop still prevented a completed capture. This does not identify
n01's historical rejected owner or prove timing/startup correctness on source B.

The n02 capture client did not publish its own GetService-derived instance proof.
Thus process attribution is established, while exact client attribution and a
safe private-session runtime exception remain open. Preserve strict route
validation until an audit-only client-bound observation and independent review
support a narrowly scoped native-lifetime design. Controller evidence reports
post-volume checks passed and owned cleanup completed; no Stop/recreate cycles,
PCM qualification or installed acceptance were reached.
Independent recomputation and immutable file receipts are recorded in
`n02-independent-analysis.json` and `.md` at the native-probe evidence root.

Identify the rejected native session owner without weakening route admission;
retain a failed run before another owned native test. Then qualify common-origin
startup, two-source PCM separation/played edges, Stop/recreate, keeper lifecycle,
contamination, accepted-tail custody and reopen. Complete consumer publication/
cancellation/lineage fencing and real Live/MCP route integration before rebuilt
installed acceptance. The original PID #96 failure remains unresolved. No new
package, merge, tag or release is qualified by these diagnostics.

## Source snapshot

SHA-256 identities at report preparation, not a committed-tree assertion:

- `src-tauri/src/endpoint_packet_sink.rs`: `F3595FB3D3A9B31A21967174C93485AC065F2B89D0BD6A5495F1E7DC734B2889`
- `src-tauri/src/live_system_audio.rs`: `A07A25BFA8A373FD938C47F9B7EF3B7C93A4A594B45847AF60C366514D6E452A`
- `src-tauri/src/capture_source_writer.rs`: `51ACD3F1D52A43C6871A32EE2BFE56F5FF43C1189AC5461B5A59D77E99A8DB16`
- `src-tauri/src/routed_native_probe.rs`: `ACDBEA2DA2DBB165258B0B1D80EB59A0CA922885AF60301144CC47E244FDCD85`
