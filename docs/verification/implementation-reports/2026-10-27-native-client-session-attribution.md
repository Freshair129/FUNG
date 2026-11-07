---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Passive native client attribution, portable probe fixtures and failed n03"
  language: "Thai-first"
---

# Native capture-client session attribution

Successor to source checkpoint `a604da5` and the
[n01/n02 report](2026-10-26-routed-native-seam-n01-diagnostic.md).
Task 2 is **PARTIAL**. This checkpoint identifies one endpoint guard failure; it
does not fix PID #96 or implement a runtime session exception. Both PID and
dedicated-endpoint installed acceptance remain mandatory before release.

## Source change

Only the optional raw endpoint sink initializes a fresh private session GUID.
An internal MTA-local diagnostic collector reads the actual client's Control2
before and after Start, preserving raw GetProcessId HRESULT, opaque session IDs,
process creation identity and manager-private/default observations. IDs are
bounded at 8,192 UTF-8 bytes and the passive receipt at 128 KiB. The GUID has a
fingerprint; receipts are explicitly `authority=false` and never authorize capture.

The sink retains metadata independently of PCM queue/drain. Its diagnostic getter
is test-only. The native probe includes the receipt in per-source diagnostics.
`RunningClient` owns cleanup immediately after successful Start, before fallible
audit work. Audit calls consume the original readiness deadline; origins, flags,
format, packet clock and gap/tail rules are not relaxed. Legacy sink-None capture
keeps its existing initialization. Initial and runtime route policy remain strict.

## n03 — actual client identified, capture still fails

Evidence root:
`G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe`.
Root executed the frozen n03 test binary with the unchanged owned two-muted-HDMI
controller. Binary SHA-256:
`9621F77F6788FBB11EC9CDBCD277832F0C3CA8160CC077B578E0AFE9BE0E2548`.
The run failed before common-barrier publication, exit 101, retaining complete
report/query/closure/reopen diagnostics. No Stop/recreate or PCM acceptance ran.

Independent analysis recomputed exact opaque equality instead of trusting saved
comparison booleans or parsing IDs. On source A
`e2a4316e-08e8-4272-9cc8-daabedf81872`, the rejected active session equals the
actual client's direct GetService instance before and after Start and the private
manager instance. The process-default instance differs and remains inactive.
Own raw PID status is S_OK, PID 25852, creation FILETIME
`134375095247637467`; path/process generation agrees with the guard's same-input
owner and controller. Source, epoch, endpoint, 48-kHz rate and common origin agree
with authoritative registration. This establishes actual client attribution for
**n03 only**, not historical n01/n02 attribution or capture-only session membership.

Source B has its own scoped client receipt; the first guard observed its own
session inactive and accepted the intended renderer. Its later no-visible-PCM
result follows the global stop and is not startup-loss proof. Both sources remain
quarantined, drained and native-terminated, with zero visible/durable meeting
frames. Access is denied; canonical/reopened snapshots are byte-identical and
hashes match. No supplemental/query failure remains. Selected endpoint volume
samples match, muted at 1.0; owned helpers exited without forced kills.

Receipts: `runs/n03/report.json`, `controller-runs/n03/controller-report.json`,
`n03-independent-analysis.json` and `.md`. These prove preservation and attribution
of a failed partial recording, not successful capture, source isolation or release.

## CI fixture correction and checks

Hosted CI at `a604da5` failed one ordinary environment test: its positive fixture
required this machine's G-drive verification directory. The run retained
779 passed, 1 failed, 12 ignored; log
`ci-a604da5-rust-failure.log` at the native-probe evidence root. No test is skipped
to fix it. The private validation helper now accepts an explicit test authority;
ordinary tests use a canonical temporary directory. The native entrypoint still
unconditionally supplies `G:\AI-Studio\verification`, and the positive test also
asserts that entrypoint rejects its temporary fixture. Path aliases, binary hashes,
distinct endpoints, unknown fields and absent-root checks remain enforced.

Evidence under `G:\AI-Studio\verification\fung-raw-capture-audit-20261027`:

| Check | Result and scope |
| --- | --- |
| External actual-module harness | RED retained, then 26 pure tests passed; not hardware |
| Integrated native audit tests | 2 passed |
| Portable authority regression | RED retained, then probe subset 9 passed / 1 native ignored |
| Frozen binary packet/adapter tests | 24 packet and 15 adapter tests passed |
| Frozen environment validation | 2 passed; no capture |
| Strict all-target Clippy | Passed; earlier unused-getter concern resolved with test-only visibility |
| Integrated full library suite | **783 passed, 0 failed, 12 ignored**, 194.52 seconds; `integrated-full-library.log` |

Formatting and diff checks passed; 268 changed-document relative file targets
resolved (`integrated-doc-links.json`, anchors/rendering not assessed). Exact source
hashes and verification receipts are in `integrated-verification.json` at the audit
evidence root. Hosted CI for this successor is a separate gate; the
earlier hosted failure remains retained rather than replaced by local evidence.

## Remaining work and execution boundary

The [next lifecycle subplan](../../plans/2026-10-27-native-capture-session-lifecycle-plan.md)
must recognize only an actual live native-owned client
with exact source/epoch/endpoint/process-generation/instance binding, revocation
and startup/shutdown barriers. A serialized passive audit is not such authority.
Private GUID metadata cannot distinguish a deliberately joined same-process
renderer sharing that exact GUID; trusted capture-only construction boundaries
and native render-negative tests remain required. No whole-PID exception follows.
Consumer fencing, Live/MCP routing, recovery/export and installed qualification
remain open. PID #96 is independently unresolved.

A separate retained-renderer-client PID experiment is an unexecuted external
draft. Six pure tests, Clippy and a release build passed, but final controller
validation and a continuous-cursor analysis are incomplete. Automatic approval
review rejected its combined controller-finalization/README/parser/pure-test
command before dispatch with only `blocked by policy`. The rejected action was
not retried through another mechanism; no PID hardware result is claimed.

No new installer, merge, tag or release is qualified by this checkpoint.
