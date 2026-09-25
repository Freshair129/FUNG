# RCA: knowledge key creation race across FUNG processes

**Date:** 2026-09-25<br>
**Risk:** HIGH<br>
**Status:** Remediated and locally verified; final source review found no blocker

## Symptom

Two FUNG processes can both observe an empty OS keyring slot for the same
knowledge key reference and then call `set_secret`. The later write can replace
the earlier key while encrypted assets already reference that slot.

## Evidence

- `KnowledgeKeyringStore::create_if_absent` documents a no-replacement
  contract in `src-tauri/src/meeting_knowledge.rs`.
- `OsKnowledgeKeyBackend::create_if_absent` protects `get_secret` followed by
  `set_secret` with `NATIVE_KNOWLEDGE_KEYRING_WRITE_LOCK`, which is a Rust
  process-local `Mutex`.
- `keyring::Entry::set_secret` writes the secret after the separate existence
  check; the current implementation has no cross-process exclusion.

## Root Cause

The implementation treats a process-local mutex as if it made the external
credential-store read-then-write sequence atomic across multiple processes.
The trait promise is stronger than the production backend provides.

## Why the issue escaped detection

The keyring fake verifies collision behavior within one test process. It does
not run two independent creators against the OS credential store.

## Remediation

The Windows backend now keeps the in-process mutex and acquires a per-key
named OS mutex in the global namespace around the existence check, write and
read-back. This lets concurrent FUNG processes for the same key reference
serialize across Windows logon sessions. Lock acquisition is bounded and fails
closed; the bytes are verified before success. A subprocess regression
exercises the named mutex through a temporary marker file and never accesses
the user's credential store.

## Verification

The Windows keyring namespace regression and subprocess mutex test passed in
the 576-test Rust library campaign. The latter uses only a temporary marker
file and never reads or writes the real credential store. The production name
uses the Windows `Global\` namespace; direct simultaneous execution from two
separate Windows logon sessions remains NOT_RUN.
