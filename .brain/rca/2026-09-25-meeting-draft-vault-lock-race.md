# RCA: private draft delivery after owner vault lock

**Date:** 2026-09-25<br>
**Risk:** HIGH<br>
**Status:** Vault-lock race remediated; separate account-transition finding is tracked in [the account lifecycle RCA](2026-09-25-meeting-account-transition-plaintext-race.md)

## Symptom

A draft operation can finish around an owner-vault lock. The native command
checks the run and commits the encrypted draft, then separately updates
runtime state and emits the plaintext draft event. A lock can clear runtime
state after the update but before the event is emitted; the frontend accepts a
matching project/recording draft event without checking whether the local
vault session is still unlocked.

## Evidence

- `meeting_agent_ask` clones the owner session before retrieval, persists the
  draft, updates `meeting_intelligence` runtime, and emits the plaintext event
  in separate steps in `src-tauri/src/lib.rs`.
- `meeting_local_owner_lock` locks the owner session and calls
  `clear_sensitive()` on the runtime in `src-tauri/src/lib.rs`.
- The Desktop `onDraft` subscription checks project/recording and transcript
  cursor, but has no owner-vault session check in
  `src/components/MeetingIntelligencePanel.tsx`.

## Root Cause

The owner-session lock and the final runtime update/event emission do not share
one serialization boundary. The database commit fence protects the encrypted
asset write, but it does not cover the later plaintext event publication.
Frontend event acceptance is scoped to the meeting, not to the local unlock
state.

## Why the issue escaped detection

The existing tests cover vault-operation fences and panel unmount/stale
subscriptions separately. They do not interleave a draft result with a vault
lock or deliver a queued draft event after the UI has locked the vault.

## Remediation

The ask path now reacquires the owner-session mutex and holds a native
operation fence through the final runtime insertion and draft event emission.
The panel tracks unlock state in a ref, clears draft/preview immediately when
locking begins, and drops delayed results/events after lock. The browser UI
fixture now injects a delayed draft after lock and reports whether it was
ignored.

## Verification

The mock-browser fixture flow passed the lock-then-delayed-draft check and
confirmed the private draft remained absent. An earlier source review
confirmed the native owner mutex and publication fence cover runtime
insertion and event emission; a later frozen-hash review found a distinct
account logout/switch race. That finding is being closed separately before
security acceptance. Native Tauri-window interaction remains NOT_RUN.
