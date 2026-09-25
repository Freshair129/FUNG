# RCA: evidence authorization between retrieval and draft persistence

**Date:** 2026-09-25<br>
**Risk:** HIGH<br>
**Status:** Remediated and locally verified; final source review found no blocker

## Symptom

Selected knowledge evidence is revalidated during retrieval, but source
version, collection status and ACL revision are not checked again before the
private draft and its evidence references are committed and exposed to the UI.
The later delivery-preview path does recheck source freshness, which occurs
after the private draft has already been returned.

## Evidence

- `search_selected_meeting_knowledge` builds evidence after per-hit
  `KnowledgeReadBoundary::revalidate` in `src-tauri/src/genesis_adapter.rs`.
- `meeting_agent_ask` then builds `evidence_refs` and calls
  `persist_private_meeting_agent_draft` in `src-tauri/src/lib.rs`.
- `persist_private_meeting_agent_draft` verifies the active agent grant and
  owner session, but does not validate each evidence reference against the
  current collection, document, version, chunk and ACL state.
- `prepare_meeting_delivery` performs those freshness checks later, after a
  private draft has already been persisted and emitted.

## Root Cause

Evidence validation is split across retrieval and delivery preparation. The
draft persistence boundary trusts the earlier search result and does not bind
its commit to the current source authorization state.

## Why the issue escaped detection

The search fixtures prove per-hit read-grant revalidation and delivery
fixtures prove stale evidence is rejected before preview. No regression
revokes a source or changes its current version after retrieval but before
draft persistence.

## Remediation

Before writing the encrypted draft, the native persistence boundary now checks
that the selected collections and agent revision are unchanged, then validates
each citation against current collection ownership/status, document version
and ACL revision, exact chunk hash/locator, and private-asset project/vault/
owner/key/custody scope. The Genesis expected-frontier commit still rejects
changes after validation. A regression changes the ACL revision between
retrieval evidence and persistence validation.

## Verification

The Rust library campaign passed the ACL-revision stale-evidence regression.
The regression fixture was updated to pass a canonical data root, matching the
native owner-session invariant enforced by `knowledge_asset_path`. The final
source/security review found no blocker.
