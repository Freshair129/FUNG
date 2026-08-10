# FUNGWIRE terminal-state race — RCA

## Symptom

GitHub Actions PR #7 failed
`fungwire_client::tests::exhausted_reconnects_marks_job_failed_and_peer_unreachable`:
the job was observed as `failed` while its paired peer was still `paired`.

## Evidence

- CI run `31337455450`, Rust job `93305535662`, failed on 2026-08-09.
- `run_transfer`'s reconnect-budget exhaustion path called `update_job(...,
  "failed", ...)` before `mark_peer_unreachable(...)`.
- The test polls for the terminal job state and then reads the peer state, so
  the independently persisted writes could be observed between those calls.

## Root Cause

The terminal job state was published before the related peer reachability
state, creating an observable cross-row race.

## Why it escaped detection

The original test used asynchronous worker writes and usually completed the
second write before the assertion ran; GitHub's runner scheduled the polling
thread in the intervening window.

## Prevention

Persist the peer's `unreachable` state before publishing the terminal job
state. Treat terminal job publication as the visibility boundary for related
failure state, and keep this regression test in the Rust CI suite.
