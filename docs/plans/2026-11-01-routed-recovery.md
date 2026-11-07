---
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-07"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Source-owned routed recovery"
  language: "Thai-first"
---

# Source-owned routed recovery implementation plan

**Goal:** Recover a verified committed routed prefix without inferring ownership
or timeline from file names, and retain uncommitted evidence for review.

**Architecture:** The native recovery caller must exclusively admit recovery
against live capture/playback. Custody validates exact recording, source epoch,
bindings, checkpoints, media integrity and frontier before an expected-frontier
transaction. A process-local custody gate serializes quarantine and publication;
it does not serialize other processes. A missing, corrupt or ambiguous source is
never silently completed. Unbound files are never adopted or deleted.

**Tech stack:** Rust, Genesis transactions, existing source custody and WAV
validation, isolated temporary Storage fixtures.

## Tasks

- [ ] Add failing isolated tests for complete committed prefixes, retry/reopen,
  unbound files, corrupt/missing chunks, mismatched epoch/sequence/checkpoints,
  racing frontier, and active capture exclusion. Confirm intended RED.
- [x] Add a narrowly scoped source custody gate; cover quarantine ordering and
  poison/failure without holding SDK or callback locks while waiting.
- [x] Implement checked recovery and durable evidence with a single expected-
  frontier transaction; quarantine uncertain/corrupt authority, preserve files,
  and do not report full recovery where unbound evidence exists.
- [x] Route the explicit native action through exclusive ownership; test retry,
  transactional failure and stale frontier. Coordinate command integration.
- [x] Independently review, run targeted CPU tests, record unresolved external
  writer, filesystem race and actual installed crash/reopen acceptance gates.

The original RED checkpoint is not retrospectively claimed. Candidate 2's final
library suite passes 1,110 tests (15 ignored), and its installed committed-prefix
recovery plus unchanged-file retry passes for two sources and six chunks. See the
[verification report](../verification/implementation-reports/2026-11-01-endpoint-export-recovery.md)
for the preserved harness failures and corrected retries. This does not qualify
the subsequently changed candidate 3, power-loss recovery, uncommitted-tail
recovery, or the remaining PID/sustained endpoint gates.

No filename-based adoption, silence synthesis, device operation, merge or release
is authorized by this plan. A refusal is not evidence of successful recovery.

Recovery retains each verified deny-write source file handle through the single
expected-frontier commit. The bounded maximum is derived from three sources and
the 24-hour `MAX_MS` duration at two-second chunks:
`3 * ceil(86,400,000 / 2,000) = 129,600` simultaneous file handles (43,200 per
source, including its final partial chunk). The same cap bounds source-export
file locks; those paths retain bare `File` handles after dropping WAV readers
and their 64 KiB buffers. This is the custody window for one atomic commit, not
a claim that the operating system cannot independently exhaust its handle
resources; any such failure remains closed and never publishes an unverified
recovery or export.

For newly started routed capture, the private accepted-packet spool is stored
under `diagnostics/routed/<recording>/sources/<source>/epoch-<epoch>` within
the project. The spool is retained diagnostic evidence, never an authoritative
chunk or an input to recovery or export. Recovery verifies and finalizes only
ledger-bound chunks under `live/<recording>`; uncommitted audio in a spool is
not included in the recovered duration. Existing recordings and in-tree legacy
spools are not moved or exempted: unbound files in `live` still require review.
This path change itself is not proof of installed crash recovery or audio quality.
