---
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Bounded routed writer buffering and installed sustained acceptance"
  language: "English"
---

# Routed writer buffering and measurements

The endpoint drain appends each accepted native packet to the private diagnostic spool and synchronizes both spool files immediately before each coalesced batch is admitted to the source writer. Valid contiguous PCM is coalesced into at most 12,000-frame (250 ms at 48 kHz) writer packets; the 64-slot queue therefore bounds queued PCM to approximately 1.536 MB per source. A residual is submitted only after a validated native drain and its batch sync. Before that sync, the tail is volatile and is not claimed crash recoverable. A gap, source failure, queue overflow, or uncertain spool sync fails closed rather than manufacturing audio or relaxing the 48-frame acceptance bound.

Bounded per-source timing summaries record spool sync count, total and maximum, writer chunk attempts, and maximum durations for WAV finalization, fsync, hashing, custody-fence wait, and ledger commit. These are diagnostics, not proof of real-time performance or lossless capture. A fault observed after a normal chunk write and before its ledger admission prevents that admission and retains the file privately. A fault concurrent with an already-admitted ledger commit can still overlap its completion; the producer does not wait for the database commit, and runtime custody denial and durable quarantine must prevent consumer access. A recording cannot become completed before writers join.

An installed two-source sustained test of at least 60 seconds is required after the change, together with the independently defined PID and endpoint packet-edge, source isolation, recovery, and export checks. A short passing run cannot establish that a larger time-based buffer does not merely defer overflow.

See the [candidate evidence](../verification/implementation-reports/2026-11-01-endpoint-export-recovery.md).

- [x] Implement bounded coalescing, residual/fault handling and timing fields.
- [x] Independently review source and author sample-order/fault/overflow regressions.
- [x] Pass candidate 2 combined tests and strict clippy (1,110 passed, 15 ignored; strict clippy exit 0).
- [x] Pass fresh candidate 3 combined tests and strict clippy after moving sync to each batch boundary (1,112 passed, 15 ignored; strict all-target clippy exit 0).
- [x] Verify bounded installed endpoint capture (74,974 ms), two-cycle source PCM, same-recording export and separate committed-prefix crash/reopen. This does not qualify all UI, enrichment, power-loss or PID behavior; see the report for preserved failed attempts and limits.
- [ ] Pass the unchanged PID gate before merge/tag/release.
