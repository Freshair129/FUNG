---
version: "0.1.0b"
created_at: "2026-10-26"
last_update: "2026-10-26"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Read-only consumer map for source custody integration"
  language: "Thai-first"
---

# Routed recording consumer guard audit

Read-only independent source audit at `50440a1`; this is an integration map, not
implemented access enforcement or installed acceptance. Authority remains the
[routing design](../../specs/2026-10-25-dedicated-endpoint-routing-design.md).
The [custody foundation](2026-10-26-source-custody-foundation.md) does not change
these existing consumer paths. Source line numbers below are navigation hints at
the audited revision; function names are the stable reference.

## Ownership points

| Consumer | Existing owner | Required integration |
| --- | --- | --- |
| Desktop playback | `desktop_playback::prepare_playback` (~1010), `open_custodied_file` (~830), `open_source` (~895), `worker_loop` (~1598) | Check bindings and actual files before opening; quarantine cancels active playback, flushes queued PCM and closes owned handles. |
| Local HTTP audio | `local_api::build_audio` (~1071) | Authoritative source selection and file checks before returning bytes; no filename-based route ownership. |
| Local/desktop transcript | `lib::transcript_view` (~5186), local API transcript route (~859) | Gate derived payload separately from audio; a blocked media path does not block this other entry point automatically. |
| Audio exports | `audio_export::source_channels` (~419), `render_source_audio_channels` (~22) | Bound source snapshot, file revalidation and validated-frontier publication CAS; retain private staging until publication is safe. |
| Subtitles and cached artifacts | `transcript_export::load_cues` (~177), `render_subtitles` (~247), `list_export_artifacts` (~310) | Separate source admission; subtitles run before audio export. Add recording lineage for artifacts rather than deriving ownership from paths. |
| Mobile audio | `mobile::playback_manifest` (~722), `playback_segment` (~639) | Guard manifest and actual bytes independently. Multi-source layout cannot sum interleaved chunk durations as one track. |
| Mobile derived output | `mobile::timeline_output` (~1110), `story_output` (~1908), processing/diarization starts | Recording lineage, late-result suppression and publication fencing. |
| Legacy ASR | `live_meeting::fill_transcript_gaps` (~3663), detailed draft (~3854), `persist_and_emit_segments` (~4186) | Verify media before inference, suppress quarantined late events and CAS derived writes. |
| Revisioned ingest/read/replay | `genesis_adapter::commit_meeting_ingest_batch` (~10501), `commit_meeting_transcript` (~9912), `meeting_transcript_snapshot` (~11581), `replay_meeting_events` (~11467) | Guard both reads and cursor/replay paths; authoritative source epoch/checkpoint at commit. |
| Diarization and graph | `local_diarization::diarize_recording` (~192), `apply_turns` (~405), `graph_build::run_graph_build` (~469) | Check before providers/workers and fence every structural/result mutation. |
| Retrieval, summaries and agents | `meeting_intel::meeting_ask` (~227), recording QA (~698), summaries (~1609); agent ask/approve/history in `lib.rs` | Filter by recording lineage before ranking/provider context, invalidate caches and suppress pending delivery after quarantine. |

## Integration rulings

1. Completed-only custody admission must not be applied blindly to active legacy
   ASR. Initial routed capture follows the existing design's persist-first,
   enrich-after-Stop rule. Future private active-source inference requires its own
   source/epoch/checkpoint contract and cancellation, not a weakened public guard.
2. `require_capture_source_access` returns a validated frontier, not a lease.
   Ordinary `commit_rows` obtains a new frontier; checking first and then using it
   would leave a race. Derived mutations must commit against the validated frontier.
3. Quarantine must become durable, then notify recording-scoped runtime owners.
   Already delivered output cannot be recalled by a storage guard; close handles,
   discard late provider results and invalidate caches. Restart behavior derives
   from durable source state, not a process-local boolean.
4. Existing `export_artifacts` lacks recording ownership. A saved export or cached
   summary/graph may otherwise outlive quarantine. Add authoritative lineage;
   do not hide unrelated projects, notes or knowledge because one recording fails.
5. Current classifiers understand `mic`/`system`/`file`. New source directories
   must use UUID bindings. Existing millisecond validators also need exact-frame
   handling for legitimate sub-ms tails before routed media support can be claimed.
6. Preserve metadata/status/recovery and cancel access for failed recordings.
   Product MCP currently exposes jobs/control, not recording media retrieval;
   gate retry/dispatch and payloads at their actual owners. Its job result summary
   is null. The legacy mobile gateway is separate from Product MCP.

## Required race matrix

Retain legacy behavior tests and add routed cases for quarantine before open,
after snapshot, during inference, immediately before publication CAS, after
playback readiness, through a reused cache/cursor, and after storage reopen.
Use real source bindings and files, not a mocked success flag. Reuse existing
audio export mutation/rollback, playback delayed-open cancellation, recording QA
provider-stub, revisioned replay and custody late-quarantine/reopen tests.

No code in these consumers was changed by this audit. Endpoint/native/PID and
installed gates remain open; this map cannot qualify a tag or release.
