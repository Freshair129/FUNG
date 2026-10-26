---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Task 2 subplan for native-owned session lifetime and runtime admission"
  language: "Thai-first"
---

# Native-owned capture session lifecycle — Task 2 subplan

Parent authority: [dedicated endpoint design](../specs/2026-10-25-dedicated-endpoint-routing-design.md)
and [implementation plan](2026-10-25-dedicated-endpoint-routing-plan.md).
This is the next **unimplemented candidate**, following
[n03 actual-client attribution](../verification/implementation-reports/2026-10-27-native-client-session-attribution.md).
Attribution is established for n03 source A only. It is not a runtime exception,
PCM/native-negative pass, PID repair or installed acceptance.

## Authority and construction

Introduce a Windows-only internal owner that holds the actual endpoint,
IAudioClient, capture service, directly bound Control2 and fresh private GUID on
the capture MTA. Private construction/fields, no serde/Clone/raw-client/GUID export;
the COM owner cannot cross threads. A separate observer contains Weak revocation
state and an exact source/epoch/endpoint/process-generation/opaque-instance/
activation tuple. Serialized audit receipts cannot create or refresh authority.

One native **group factory** must obtain fresh strict whole-group admission,
verify all real Storage bindings and reserve unique source activations before
any worker initializes/starts. Workers consume only internal prepared group state.
Never strict-admit the whole group again from worker B after A has started:
that recreates the current bootstrap rejection. An additional worker pre-Start
check, if needed, is limited to that worker's distinct selected route while its
own client is inactive. Callers cannot supply a pure ValidatedRoutePlan or
SourcePacketScope as proof of native execution.

The custody owner must supply a narrow authoritative read of actual project/
recording/source relationships: recording is recording/routed_endpoint; source
is registered or capturing, correct endpoint kind and epoch, unclosed and not
quarantined; persisted endpoint/rate/common origin match. Use a stable read frontier,
then recheck binding fields after Start and during runtime. Ordinary checkpoint
advancement alone must not revoke a valid binding. Missing/changed/unknown data
rejects. No schema value or migration is invented by this plan.

## Runtime and lifecycle contract

- Keep public pure/initial route validation, preview and client JSON schemas strict.
  A separate internal runtime path obtains fresh native facts and authoritative
  bindings. It checks observer liveness/generation before and after evaluation.
- Only the exact live native-owned tuple may be classified as capture infrastructure.
  It does not count as an authorized renderer or make WaitingForTarget become Armed.
  Duplicate/empty/unknown/cross-process/non-S_OK and unexpected-instance checks stay.
  Same PID with a different/default instance still rejects.
- Every owner's actual private/default and pre/post direct-client proof must succeed;
  unsupported results fail. All owners register before runtime guard decisions and
  readiness, within the original common-origin deadline. No rebasing or fabricated
  startup silence. A partial/failed group cancels, revokes and drains explicitly.
- Establish Stop RAII immediately after Start. Stop/error/panic/drop revokes before
  native teardown; Weak observers cannot extend lifetime or resurrect an epoch.
  A snapshot spanning revocation must be retried or rejected. Final strict inventory
  waits for owned clients to stop/drop and their instances to become inactive/absent.
- Preserve accepted-tail ownership, source-scoped first failures and quarantine.
  Completion waits for actual producer/writer quiescence. This state adds no
  playback/export/enrichment authority or instantaneous privacy lock.

## Test-first execution sequence

1. [ ] Real SDK registration-read RED/GREEN: absent/wrong project, recording, source,
   epoch, endpoint, origin/rate, quarantined/closed/completed and mutated bindings
   reject; ordinary PCM checkpoint updates remain valid.
2. [ ] Sealed-state RED/GREEN: expired Weak, revoked/Starting/partial group, duplicate
   activation, wrong scope/generation/instance, ambiguous PID and cross-process status
   reject. Test fixtures never stand in for native proof.
3. [ ] Integrate the MTA owner and group startup without changing legacy sink-None
   capture. Audit trusted render/TTS factories: neither receives the private GUID.
   Review constructor visibility, failure cleanup and runtime race handling.
4. [ ] Frozen owned native negatives/positives: live exact instance only, same-PID
   foreign/default instance, unknown foreign process, target Stop/recreate, source
   Stop/drop and stale observer. Keep the original PCM/gap/separation criteria.
5. [ ] Two-source capture-to-custody, one-source fault, raw-tail preservation, reopen,
   played-edge and separation qualification; independent review and full checks.
6. [ ] Only then proceed to product UI/MCP execution, consumer publication fencing,
   source-aware recovery/export and actual installed workflows in the parent plan.

## Explicit feasibility boundary

A same-process renderer deliberately joining the **same** private GUID may share
the exact PID/session instance. These metadata primitives cannot distinguish that
co-membership. Measure the owned same-GUID negative and document the trusted native
capture-only construction boundary; if it cannot be enforced, keep activation
closed rather than broadening to a PID exception. No process sandbox guarantee
or capture-only-membership proof follows from n03.

No routing/default/volume setter, virtual driver, PID-mode replacement or release
waiver is included. Both PID and endpoint modes must pass installed acceptance.
