---
version: "0.1.0b"
created_at: "2026-09-24T10:12:00+07:00,RWANG,working-tree"
last_update: "2026-09-24T23:25:01+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "root-cause-analysis"
  scope: "Static review of typed knowledge metric values at rest"
  complexity: "C-2"
  risk: "HIGH"
---

# Plaintext metric observations

## Symptom

The local Actual/Budget workflow wrote a sensitive numeric observation as a
plain decimal string in `knowledge_metric_observations.decimal_value`, even
though its source document and agent drafts use the vault's encrypted custody
boundary. A database/WAL/backup reader could therefore see the typed metric
without opening the selected source.

This is a source-derived privacy finding in the approved, unaccepted working
tree. It was not observed in a released build or a production database.

## Evidence

- `src-tauri/src/genesis_adapter.rs::save_meeting_knowledge_metric` serialized
  `value.format()` directly into the `decimal_value` field.
- `compute_meeting_knowledge_metric` parsed that same field directly, so the
  arithmetic path required plaintext persistence.
- The existing `meeting_knowledge.rs` encryption envelope already authenticates
  the vault/asset context and stores only ciphertext plus a key reference.
- The accepted plan's M2 verification lane requires encrypted-storage/WAL
  canaries; the metric path had no equivalent encryption boundary.

## Root Cause

The existing typed metric schema models `decimal_value` as text, and the local
arithmetic integration treated that type as a storage format rather than an
opaque ciphertext slot. No schema change is needed to correct this: encrypt the
canonical decimal using the existing vault key and bind it to the observation
ID before writing it; decrypt only after current owner/selection validation.

## Why the issue escaped detection

The arithmetic tests exercised decimal correctness and citation lineage, while
UI contract tests covered command forwarding. Neither inspected the stored
representation for plaintext numeric canaries.

## Proposed prevention

Persist an authenticated encryption envelope in the existing text column,
using `KnowledgeEvidence` purpose and the current vault key. Bind the envelope
to the vault and observation ID, reject malformed/wrong-context data, decrypt
only inside the native compute/save boundary, and preserve decimal scale in the
encrypted payload. Add ciphertext, wrong-context, and scale-preservation
fixtures. Do not make PDF or external dispatch readiness depend on this local
metric change.

## Verification boundary

The implementation and regression coverage are present in the working tree.
No test, build, or runtime execution has run in this task turn; the consolidated
campaign remains deferred until implementation freeze.
