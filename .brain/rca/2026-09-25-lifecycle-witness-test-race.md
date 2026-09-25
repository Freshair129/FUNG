---
version: "0.1.0b"
created_at: "2026-09-25T03:01:52+07:00,RWANG"
last_update: "2026-09-25T03:13:29+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "auth-session-tests"
  doc_type: "rca"
  scope: "deterministic lifecycle witness concurrency fixture"
  language: "English"
---

# RCA: lifecycle witness test missed logout transition

## Symptom

The consolidated Rust suite intermittently failed
`concurrent_lifecycle_witness_reads_are_coherent_during_logout` because the
sample list did not contain `logout_pending`.

## Evidence

The test waited until the main thread observed `logout_pending`, then dropped
the account-operation guard. The logout thread could immediately publish
`signed_out`; the reader thread had no acknowledgement that it had sampled the
intermediate state before the guard was released.

## Root Cause

The test used a start barrier and an initial-reader acknowledgement, but no
transition acknowledgement. Its assertion depended on scheduler timing during
the short `logout_pending` interval.

## Why the issue escaped detection

Earlier runs scheduled the reader during that interval. A later consolidated
run scheduled the main thread and logout transition before the reader captured
the intermediate witness.

## Proposed Prevention

Have the reader signal after recording `logout_pending`; keep the operation
guard held until that acknowledgement arrives. Preserve the existing bounded
deadline and assert the final recorded witnesses remain coherent.

## Verification

The focused regression passed, and the final full library run passed with
573 tests passed, 0 failed and 1 ignored.
