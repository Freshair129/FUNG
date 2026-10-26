---
version: "0.1.0b"
created_at: "2026-10-26"
last_update: "2026-10-26"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Bounded exact-frame native source writer, without capture activation"
  language: "Thai-first"
---

# Exact-frame source writer foundation

Baseline `50440a1`; working-tree successor adds public trusted in-process
`capture_source_writer` APIs. There is no capture executor, IPC/UI/MCP command,
installed test or #96 repair. Task 2 remains **PARTIAL**; both installed modes
must pass before merge/tag/release.

## Implemented contract

- One recording-scoped writer group owns a shared source-commit fence. Each
  already registered source has a distinct worker, source-owned directory and
  exact-frame mono PCM16 packet stream. Source/epoch/rate/origin must match storage.
- Producer admission performs no filesystem or database calls. Queue capacity is
  explicit 1–64 packets, each at most two seconds, including allocation capacity.
  At the admitted maximum 192 kHz, queued PCM is at most 49,152,000 bytes per
  source. Bounded work/terminal packets and two assembly buffers keep PCM storage
  below 64 MiB per source, excluding allocator/metadata overhead. Typical 10-ms
  packets provide 640 ms queue time, not 64 seconds.
- Writes retain two-second chunks and every nonempty tail, including one frame.
  WAV finalize/flush/sync/rename/hash precedes the authoritative custody append;
  audio, binding and checkpoints commit atomically. No frame is inferred from ms.
- Wrong identity, overlap, missing startup, gaps, queue overflow and native fault
  signals retain the first bounded failure outside the queue. No missing audio is
  fabricated. Accepted queued PCM drains privately; forward terminal evidence may
  be retained after quarantine. Unbound finalized files remain explicit orphans.
- After observing failure, the worker attempts durable quarantine before further
  processing. A producer fault may arrive during an already in-flight commit;
  that commit may finish first. This is **not instantaneous durable cancellation**.
  No enrichment, publication or recording completion occurs in the writer.
- Unknown append outcomes reconcile once with the same stable attempt. If both
  append/reconciliation fail, durability counters are confirmed lower bounds and
  `durable_counts_known=false`; no guessed successful transaction. Unrelated
  global Genesis writes may cause a conservative CAS failure/quarantine.
- `finish` drains one writer only. Coordinator must stop native producers, inspect
  every report and drain all workers before closing the whole source set once.
  Failed or unpersisted quarantine must block healthy completion; rows alone do
  not establish that all captured PCM reached durable storage.

Windows local drive paths are normalized only after canonical authority checks.
UNC/device namespaces remain rejected. Production queries use the Genesis
adapter's namespace constant, not a separately hardcoded namespace.

## Verification

Evidence root: `G:\AI-Studio\verification\fung-endpoint-routing-20261026-writer-draft`.
External draft uses the actual pinned Genesis SDK and real temporary WAV files.
Its retained tests cover initial RED, local-drive path integration failure,
native first-fault delivery, multibyte audit bounds and Vec allocation capacity.
Final external suite: 25 passed (11 writer, 13 custody, one SDK migration), strict
Clippy passed. This separate harness omits product-specific schema adoption.

Integrated real FUNG writer subset: **11 passed**, `integrated-writer-tests.log`.
Integrated independent review accepts the adopted code: only the external header
and namespace-owner reference changed; no capture callers or whole-recording
completion were introduced. Full integrated regression passes **733 tests, zero
failures, 11 ignored** in 334.78 s (`integrated-full-library.log`); ignored checks
are not acceptance. Strict all-target Clippy passes (`integrated-clippy.log`),
as do formatting and whitespace checks. No additional native or installed test
was run by these commands.

## Open integration

The native exact-frame packet/barrier seam is still external. Its native start
must not be later than the chosen meeting origin without an explicit failure;
legitimate private preroll clipping needs retained sample-phase provenance.
Different per-device origins or invented initial silence cannot establish sync.

No session/device guard, native stop wiring, source-aware recovery, consumer
publication/cancellation or capture activation is implied. See the
[consumer audit](2026-10-26-routed-consumer-guard-audit.md) before enabling routes.
Actual native and installed acceptance remain unrun for this writer.
