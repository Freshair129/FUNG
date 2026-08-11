---
version: "0.1.0b"
created_at: "2026-08-12T05:35:00+07:00,Agent: ATHER"
last_update: "2026-08-12T05:35:00+07:00,Agent: ATHER"
status: "beta"
attributes:
  domain: "test-reliability"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: FUNGWIRE loopback tests timed out on the full Windows CI suite

## Symptom

GitHub Actions run `31541234799` failed the Rust job while the same feature
passed locally. The two failures were:

- `fungwire_client::tests::delegate_transcription_completes_and_writes_transcript_over_loopback`
- `fungwire_client::tests::delegate_transcription_reconnects_after_early_drop_and_completes`

Both assertions saw an empty terminal state rather than `completed`.

## Evidence

- CI job `93943843843` reported `193 passed; 2 failed`.
- Both tests poll `delegated_jobs` 100 times with a 100ms sleep, a 10-second
  total bound.
- The failures occur only in the full parallel suite; each test passes when
  run alone, and the exact full command
  `cargo test --manifest-path src-tauri/Cargo.toml` passes locally with
  `195 passed; 0 failed`.
- The failing tests exercise a real loopback server and Python worker process;
  CI logs show no protocol or worker error before the polling bound expires.

## Root Cause

The test-only terminal-state bound was calibrated to a warm local machine.
Under the Windows CI runner, the complete Rust suite starts the loopback
server, Noise transport, Genesis writes, and Python subprocesses concurrently.
That scheduling and process/IO startup variance can exceed 10 seconds even
when the job is healthy. The assertion therefore fired before the job reached
its real terminal state.

## Why The Issue Escaped Detection

Local full-suite and individual-test runs were used, but the Windows CI
runner's cold, parallel scheduling profile was not represented by the 10-second
test bound.

## Fix

Increase the bounded poll window for the three successful loopback e2e checks
to 600 polls (60 seconds at 100ms). This changes test tolerance only; the
production reconnect/read timeouts and protocol behavior are unchanged.

## Proposed Prevention

- Keep full Rust CI as a required merge check.
- Run the exact workflow command locally before publishing changes that touch
  FUNGWIRE tests.
- Keep e2e waits bounded, but size them for the slowest supported CI runner
  rather than a warm developer machine.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Added RCA for the Windows CI loopback test timeout and bounded-wait fix. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-12 | beta | Recorded FUNGWIRE e2e CI timeout RCA and prevention. | N/A | ATHER |
