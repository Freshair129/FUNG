---
version: "0.1.0b"
created_at: "2026-09-21T08:36:40+07:00,Newton"
last_update: "2026-09-21T08:42:35+07:00,Newton"
status: "need review"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "rca"
  scope: "N3 Genesis schema resource budget"
  execution_state: "FROZEN/BLOCKED"
  verification_status: "FAIL"
---

# RCA — N3 meeting schema resource budget

Author worker agent/session: `01a0c16c-ab83-7972-aa36-0936e1a95878` (Newton).
Parent orchestrator/risk-review task: `01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75`.

## Symptom

The N3 Rust Genesis tests cannot open/install the isolated worktree database
after the approved LT/KE/MA/SI v11 aggregate interfaces are registered. The
engine returns:

```text
REL_SCHEMA_VALIDATION_FAILED: schema resource limit exceeded
```

This occurs before the meeting fixture is installed and before any N3
`commit_transaction` is attempted.

## Evidence

The exact pinned engine is the cached Genesis checkout at revision
`79b41a3f4ae4026d086b634c631f4f4a7ccbd142`. Its `src/lib.rs:3584` guard is:

```rust
if package.tables.len() > 64 || package.named_queries.len() > 128
```

The executable schema-chain test printed:

```text
N3_RUNTIME_SCHEMA_COUNTS v10=41 v11=66 added=25
```

The count was obtained at runtime from `schema_v10().tables.len()` and
`schema().tables.len()`, not from a text-only estimate. The approved-scope
budget arithmetic is:

```text
v10 baseline                         41
original v11 additions               31
pre-deferral v11 total               72
remove two compact KE bridge tables  -2
defer three biometric tables         -3
defer optional voice_consents        -1
retained post-deferral v11           66
Genesis package limit                64
remaining overage                    2
```

Commands and exits:

```text
cargo check --lib --locked --offline --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921\src-tauri\Cargo.toml
exit 1: Tauri build script reports resource path ..\.venv-whisper doesn't exist

TAURI_CONFIG='{"bundle":{"resources":[]}}' cargo check --lib --locked --offline --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921\src-tauri\Cargo.toml
exit 0: Rust type-check succeeds with the process-only test override

cargo test --lib --locked --offline --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921\src-tauri\Cargo.toml the_schema_chain_advances_one_version_at_a_time -- --nocapture
exit 0: runtime count test prints v10=41 v11=66 added=25

cargo test --lib --locked --offline --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921\src-tauri\Cargo.toml n3_atomic_commit_covers_custody_projection_event_and_cursor_after_reopen -- --nocapture
exit 1: schema install fails with REL_SCHEMA_VALIDATION_FAILED before fixture setup

cargo test --lib --locked --offline --manifest-path C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921\src-tauri\Cargo.toml n3_ke_sharing_and_si_private_relationships_remain_fail_closed -- --nocapture
exit 0: one pure KE/SI fail-closed test passes
```

The test-only `TAURI_CONFIG` override replaced `bundle.resources` with `[]` in
the process environment only. It did not edit tracked configuration, package
files, dependencies, runtime directories or model caches, and it is not
packaged-runtime proof.

## Root Cause

The v11 package is cumulative with the frozen v10 relational schema. The
approved canonical KE aggregates, LT atomicity aggregates, MA/D13 delivery
aggregates, and SI/D8 participant/review boundary cannot all fit below the
pinned Genesis package table limit after only the genuinely out-of-scope
biometric/enrollment deferrals. This is a supported-engine resource-budget
conflict, not a Rust type error, dependency cache substitution, or failed
transaction rollback.

## Why the issue escaped detection

The initial offline preflight stopped earlier in the Tauri build script because
the snapshot intentionally lacks `..\\.venv-whisper`; it did not reach Genesis
schema validation. The compile-only TAURI override exposed Rust compilation,
but the first runtime schema-install tests were needed to exercise the pinned
engine’s cumulative package limit. Textual table counts were not accepted as
the final evidence; the count test then measured the package at runtime.

## Proposed prevention

1. Add a pre-registration runtime budget assertion to the N3 schema test and
   fail with the exact package count and limit before any fixture or migration
   work begins.
2. Make the workflow gate require `schema_vN.tables.len() <= 64` and
   `named_queries.len() <= 128` against the exact pinned Genesis revision.
3. Keep aggregate-to-table mapping review separate from implementation and
   require Boss approval for any aggregate deferral, namespace split, or
   Genesis dependency change.
4. Keep N6/N7 `NOT_READY` while the shared schema budget is unresolved; their
   disjoint leases must not add shared Genesis tables.

## Options requiring Boss approval

1. Approve a new pinned Genesis revision or formal engine contract that raises
   the package table limit, then rerun migration, idempotence, replay, reopen,
   crash-boundary and scope/custody tests.
2. Approve a namespace/package redesign and separately prove FK/ACL behavior
   plus SP-LT one-transaction atomicity across the proposed boundary. No
   namespace-splitting feasibility is asserted here; it was not tested.
3. Approve a named product-scope reduction of one retained SI/D8 or MA/KE
   aggregate. This would change the approved fanout contract and must not be
   selected by this worker.

The snapshot therefore remains `FROZEN/BLOCKED`. No dependency, workflow,
specification, Git metadata, root checkout, user database, provider, network,
model or deployment state was changed by this RCA.
