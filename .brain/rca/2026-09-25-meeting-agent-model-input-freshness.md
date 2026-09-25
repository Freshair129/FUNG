# RCA: non-cited model evidence could bypass the freshness gate

**Date:** 2026-09-25
**Risk:** HIGH
**Status:** Remediated and locally verified

## Symptom

During source review, a model proposal could cite one of eight retrieved hits
while using information from the other seven. The first implementation passed
only cited hits to draft persistence, so a source or ACL change on a non-cited
hit during model inference would not stop the draft.

## Evidence

- `meeting_agent_model::generate` sends up to eight retrieved excerpts.
- The initial `meeting_agent_ask` loop built `evidence_refs` only from model
  `refs`, and `persist_private_meeting_agent_draft` validated only those refs.
- Native preview read the persisted cited refs, so it could not recheck a
  non-cited source that may have influenced the answer.

## Root Cause

The model input set and the citation set were treated as the same set. They
serve different purposes: every input source is a freshness dependency, while
citations describe which sources the model explicitly named.

## Why the issue escaped detection

The original fixture covered invalid or duplicate citation IDs but did not
change an uncited source after retrieval. The extractive path naturally uses
every displayed hit, so this distinction did not exist there.

## Remediation and prevention

Persist metadata for every model input hit separately from cited evidence.
Native validates all model input refs under the draft commit frontier and
again before local preview; approval reruns the preview gate. Cited refs must
be a nonempty subset of that input set. No excerpt or private answer is added
to plaintext run metadata.

## Verification

The model-input subset regression and the final Rust library suite passed:
583 tests passed, 1 ignored, with the sandbox-only AppContainer probe excluded.
The `meeting_knowledge` integration suite passed 16/16 with that probe
excluded. No real local-model quality or real-meeting acceptance is claimed.
