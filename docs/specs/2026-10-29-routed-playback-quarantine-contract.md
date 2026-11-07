---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "specification"
  scope: "Source-aware completed playback and quarantine revocation"
  language: "Thai-first"
---

# Candidate contract: routed playback and quarantine revocation

**Status: implementation proposal under review.** This document proposes observable
requirements and concrete native command additions below. It introduces no public
network route, authentication mechanism, schema, or persistence owner.
Interface exposure requires the reviews below. It does not qualify PID
capture, endpoint capture, an installed package, or a release.

## Context and authority

The user requests source-aware endpoint playback and quarantine handling as part
of the complete two-mode workflow. The [dedicated endpoint routing design](2026-10-25-dedicated-endpoint-routing-design.md)
requires trusted source identity, verified source media and quarantine revocation.
The existing [CallMD desktop contract](2026-09-17-callmd-desktop-contracts.md#7-native-playback-design-new-b-sec-1)
limits playback to `mic` / `system` / `file` channel selection. It does not yet
define a public routed-source selector. A source UUID must not be hidden in a
legacy channel, presented as a filename, or reconstructed from a display label.

Preserve Thai UI labels, named exports, CSS conventions, anon-key-only configuration,
surface-param routes ungated, SEC-1 and SEC-2 trust/admission boundaries, and the
normal profile and existing recordings. No capture fallback, fabricated PCM,
relaxed audio tolerance, target injection, implicit rerouting, or new driver.

## Proposed behavior

1. The user explicitly selects one source belonging to the selected recording.
   The authoritative source UUID, recording/project ownership, capture epoch and
   ordered chunk bindings come from the existing Genesis custody records. Labels
   and endpoint IDs are display/provenance, not authorization or source identity.
2. Routed playback opens only a completed, trusted recording and source set; the
   current conservative aggregate policy blocks a recording if any source is
   pending, active, quarantined, partial, unknown or has inconsistent bindings.
   No legacy filename inference or `file`/`system` fallback is allowed.
3. Before output and every later reopen, validate the actual custodied media via
   the same opened handle used for reading: containment under the authorized root,
   regular file and byte size, stored hash, PCM header, sample rate, exact frame
   boundaries and authoritative chunk order. Same-handle validation alone does
   not prevent in-place mutation: require a reviewed file-sharing/immutability or
   consumption-time verification strategy, including cross-process write and
   replacement tests during validation and after open. Reject mismatches; do not represent
   missing or corrupt active PCM as silence. Retain legitimate sub-millisecond
   tails in frame units rather than inferring length from rounded milliseconds.
4. Retain native playback's bounded metadata, handles, read size and queue and its
   explicit local-main-window origin/owner validation, capture/playback exclusion,
   epoch, seek and shutdown semantics under SEC-1 and SEC-2. No path or file handle
   is exposed to the renderer. Existing legacy playback remains compatible.
5. If a selected source or its aggregate recording becomes quarantined, stop
   publishing/feeding that playback, flush private queue, cancel pending opens
   and workers, close owned
   handles, and return an explicit state/error through the approved interface.
   Already rendered audio cannot be recalled. A durable quarantine and the
   acknowledgment of runtime cancellation are distinct events; do not claim that
   an asynchronous worker or a storage commit instantly retracts an output buffer.
   The interface review must define the cancellation acknowledgment and atomic
   fence, including the guarantee for callbacks after acknowledgment and treatment
   of callbacks already in flight or audio already submitted to hardware.
6. No playback success is reported for an unknown or failed source check, an
   uncertain quarantine write, stale ownership, or failed cancellation. A partial
   recording remains visible as metadata without making its PCM playable.

## Required lifecycle and race semantics

### Buffering and close: candidate for interface review

Verified chunk opening can take longer than the current underrun timeout. Before
this player is exposed, the interface and backend owners must review how to
represent verification or refill in progress without reporting source frames as
played. One candidate keeps the existing playback-state enum and adds a typed,
optional buffering reason and stream epoch; legacy client behavior and capability
handling still need review. Intentional device silence while buffering must not
advance source position or be described as source audio. Media verification,
frontier checks and quarantine rejection remain mandatory.

Pause and seek acknowledgments must fence earlier output and invalidate stale
work; audio already handed to hardware cannot be recalled. Closing successfully
requires worker, callbacks and media handles to be released and the capture
guard freed. Cancellation of ordinary close must not permanently revoke the
recording. An uninterruptible OS or storage call can delay cleanup; an interface
must not report successful closure while that cleanup remains incomplete.
The timeout, prefill/prefetch bounds, error presentation and coherent status
snapshot remain open review decisions; no public interface is changed here.

`capture_source_custody::require_capture_source_access` returns a storage frontier,
**not** an access lease. Checking it at open alone cannot protect later playback.
Existing quarantine writes flow through both `capture_source_writer` and
`routed_capture` into `quarantine_capture_source`; there is currently no player
notification attached to that custody function. A source-aware player must not be
enabled until the approved design covers every real quarantine entry path.

The implementation design must define and test ordering and ownership for:

- quarantine before selection, during delayed open or hash validation, between
  validation and installation, while playing, while filling a queue, on callback,
  during seek, and while closing or shutting down;
- a durable state change concurrent with playback admission or an existing
  playback handle, including failed/uncertain commit and idempotent retry;
- suppression of late preparation results, old queued frames and stale epochs;
  cancellation acknowledgment must state what has actually stopped and closed;
- multiple recordings and sources, so a revocation cannot cancel or disclose an
  unrelated recording; no identity is derived from mutable labels or PID alone;
- storage reopen and process restart, where durable quarantine is authoritative;
  stale process-local state cannot grant access after restart;
- any separately running writer or second process with database write access.
  A process-local observer alone cannot establish that such changes reach the
  player. The ownership and cross-process visibility mechanism remain unresolved.

Ordering must avoid waiting for a player worker or audio callback while holding
database/manager locks the worker requires. The audio callback must remain bounded
and non-blocking. The contract must state the bounded in-flight output window,
failure behavior and what the acknowledgment guarantees; already delivered PCM
is outside any retroactive revocation guarantee.

## Open owner decisions — required before public implementation

1. **Selection and wire ownership:** which existing reviewed interface owns the
   source selector and playback status, and how are routed identity and legacy
   channel selection represented without ambiguous or backward-incompatible input?
   The decision must include error/state projection and UI selection behavior.
2. **Visibility and execution scope:** can any process other than the desktop
   native owner mutate these custody records while playback runs? Identify the
   authoritative writer and cross-process change signal/fencing if applicable.
   If a single-process guarantee is proposed, its enforcement and failure modes
   must be established rather than assumed.
   A Windows isolated cross-process test of pinned SDK2cf4855 passes for normal
   and extended aliases to the same lock-file identity, including writable and
   read-only SDK opens while held and positive controls after release. This is
   evidence for cooperative SDK opens on the same lock object only; it does not
   cover lock bypass, replacement, different paths to different lock files, or
   runtime cancellation between in-process threads. The test is local evidence,
   not acceptance of the visibility decision or a portable CI test.
3. **Commit/revocation ordering:** define how admission, quarantine persistence,
   cancellation signal and acknowledgment synchronize; specify behavior when
   persistence or cleanup fails, and whether pending/in-flight output can occur.
   A validated storage frontier alone is not sufficient.
4. **Post-close integrity failures:** coordinator final checks can fail after
   `close_registered_sources` has run. A later global shared failure triggers
   `quarantine_affected_sources` with bounded retries. Whether those retries
   durably quarantine already-closed sources and complete cancellation/ack under
   errors and races needs tests; do not infer success from invocation alone.
5. **Source-set trust:** confirm whether conservative all-sources trust remains
   mandatory for selected-source playback or is amended with independently
   reviewed lineage and contamination semantics. Until then preserve the existing
   aggregate guard; never weaken it as an implementation shortcut.
6. **Bounded source-query authority (resolved scope interpretation):** retain
   existing embedded `Storage::query_relational`, namespace and aggregate guard.
   The canonical limit is 20,000 descriptors/8MiB metadata per player, not an
   8MiB peak for the entire SDK. Narrow projections and incremental result checks
   bound the retained manifest; SDK pages materialize before those checks, so no
   pre-allocation or total peak-memory guarantee is claimed. The earlier HQL2
   principal requirement was an inferred plan strengthening and is withdrawn.
   No new principal, namespace or persistence authority is introduced.

## Concrete integration proposal — continuation review required

This section fixes the implementation choices within the user's renewed request.
It remains subject to contract/backend review before activation.

- Preserve `desktop_playback_open(projectId, recordingId, channel)` for legacy
  mic/system/file. Add `desktop_playback_sources(projectId, recordingId)` and
  `desktop_playback_open_source(projectId, recordingId, sourceId, captureEpoch)`
  to the same trusted local-main-window authority and native admission guard.
  These are native commands, not HTTP routes or hosted permissions. Catalog
  items contain sourceId, captureEpoch, label, kind, sampleRate and durationFrames;
  no media path, database path, endpoint token or handle is returned.
- Preserve legacy PlaybackState fields and add optional `source` identity
  `{sourceId, captureEpoch}` (absent on legacy). Routed state uses explicit
  channel `source`, never a UUID disguised as a legacy channel. Existing control,
  status, epoch and close commands apply to the issued opaque owner handle.
  A selected source is not a person; labels come from the validated registration.
- Source selection always retains conservative aggregate trust. Register the
  revocation lease before reading authoritative state, validate the manifest and
  frontier, and check the lease before installing, publishing queues or output.
  Completed-source exact frame counts determine EOF, including sub-ms tails;
  displayed milliseconds may round down without dropping the last frame.
  Retain exact durationFrames in the native session, including control/seek;
  never reconstruct EOF by multiplying rounded durationMs. Seek(0) on a one-frame
  source must leave that frame playable. Routed output requires the source's
  exact sample rate on the selected output; if unavailable return
  PLAYBACK_OUTPUT_UNSUPPORTED. Existing legacy output behavior is unchanged.
- The lease retains the same Arc<Storage> as its native owner. A bounded weak
  registry matches exact Storage and recording identity; no global recording-ID
  match across stores. Cooperative other SDK owners are excluded by the existing
  store lock; no protection against an actor bypassing the SDK is claimed.
- Every quarantine entry uses `quarantine_capture_source`. Commit first, then
  permanently revoke associated leases on success and uncertain commit failure;
  already-quarantined retries also revoke. The output gate uses try-lock in the
  callback, which outputs silence on contention/revocation and never queries
  storage, waits, allocates or opens files. Revocation fences callbacks already
  inside the gate after the commit returns, outside SDK/manager locks. It does
  not recall audio already submitted to hardware. Return from the custody
  function acknowledges this output fence, not that all worker handles closed.
  The worker observes revocation, clears its queue, closes readers and terminates;
  existing explicit close acknowledgment remains the full cleanup guarantee.
- Quarantine failure never restores the lease. A new open must repeat durable
  aggregate admission; a changed storage frontier invalidates old preparation.
  Maintain bounded per-store recording denials even when failure preceded the
  first lease, using the existing Arc<Storage> identity at the custody hook.
  Do not evict denials at capacity: deny the affected store instead. A poisoned
  registry or exhausted identity capacity fails future admission closed.
  These denials last for that Storage lifetime and are not a durable journal;
  restart after a never-committed quarantine is not qualified by runtime tests.
  Durable retry/reconciliation and restart remain required acceptance cases.
  Validate write/delete sharing on the opened file so hashes remain meaningful
  during reads and reopen. Tests must include concurrent mutation, delayed open,
  quarantine retry, unrelated store/recording and callbacks at the fence.
- UI closes the current player before changing source and discards late opens
  using existing selection epochs. It presents the source catalog as a selector
  in the existing player, with explicit Thai unavailable/quarantine messages.
  Keep the established shell/layout and legacy channel selector.

## Verification and acceptance

Contract review must include identity/wire compatibility, SEC-1/SEC-2 scope,
source custody and cancellation ordering. Then complete the existing
`CONTRACT_TEST_REVIEW` and `BACKEND_INTERFACE_REVIEW` gates before public
backend/shared-contract implementation and UI activation. Obtain the applicable
native backend and UI reviews, followed by integrated exact-revision verification.
Approval of this candidate's direction is not a substitute for those reviews.

Required meaningful tests include actual Genesis-backed source bindings and
recordings; wrong project/recording/source/epoch; unknown, incomplete and
quarantined sources; corrupted/replaced/escaped media; exact sub-ms frame tails;
legacy regression; all races above; queue and handle cleanup; process/storage
reopen; and isolation of an unaffected recording. Exercise actual native output
and the installed local-main-window command path separately from CPU fixtures.
Full endpoint acceptance further requires two independently routed applications
plus microphone, durable capture/review, transcription, export and recovery.

Original PID #96 remains a separate required gate. Only actual installed acceptance
of **both** PID and endpoint workflows, together with required review/CI, can
authorize merge/tag/release. This candidate neither changes that condition nor
claims those capabilities are already implemented.
