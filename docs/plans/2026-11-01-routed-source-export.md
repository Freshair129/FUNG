---
version: "0.1.0b"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: "draft"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "implementation-plan"
  scope: "Completed source-aware export"
  language: "English"
---

# Completed routed-source export

The existing legacy exporter explicitly rejects routed recordings. Add one WAV
per completed `(recording, source ID, epoch)` using the completed custody
manifest and verified chunk handles. Never derive identity from filenames, mix
sources, fabricate samples, or silently replace missing coverage with silence.
All sources in an export batch either publish together or the attempt fails.

Use an additive Genesis schema table `source_export_bindings` to bind each
artifact to its recording, source, epoch, content hash, byte length and source
frontier. Keep existing `export_artifacts` rows compatible for legacy clients.
Deterministic source identity in filenames is only a display convention; the
typed binding is the provenance authority. Genesis schema upgrade must preserve
existing rows and recover old v16 stores.

Prepare files in a unique source-set directory that is absent from the artifact
inventory until its conditional metadata transaction commits. Do not rename an
open directory as a publication step. Validate content and WAV frame counts with
`completed_source_media::open_verified_chunk` and respect bounded storage and
metadata. Recheck the source frontier and scope immediately before publication.
Coordinate the conditional Genesis transaction with a process-local global source-custody mutation gate
shared by quarantine and recovery; do not reuse the playback callback gate as
an unreviewed export fence. Genesis's exclusive database writer lock and
transaction frontier CAS constrain other Storage instances; no cross-process
mutex or post-export external copy revocation is claimed. If publish or commit
is uncertain, preserve the staged/published set for explicit reconciliation;
never report success merely because the file exists. A visible directory is not
evidence that its artifact transaction committed.

On Windows, open the volume anchor and traverse individual components relative
to retained directory handles. Create/open exports, source-set directories and
WAVs with `NtCreateFile` using a single basename, `RootDirectory`,
`OBJ_DONT_REPARSE` and `FILE_OPEN_REPARSE_POINT`; check returned handle attributes
and final identity. Existing reparse objects may open successfully and must still
be rejected by their attributes. Keep writable output handles (deny other write
and delete access) alive through WAV finalization, checksumming and commit. Reuse
opens are relative too. Directory sharing alone cannot prevent junction mutation:
the owned-temp probes demonstrate a `FILE_WRITE_ATTRIBUTES` bypass. The relative
API's success/refusal is measured on this host; no all-Windows-version guarantee
is claimed, and failures remain explicit refusals.

The export registers a source playback revocation lease before manifest
admission and rechecks it inside the final custody gate. Quarantine must latch
runtime denial under that same gate before it releases the gate even when its
SDK commit returns an uncertain error; output quiescence happens after gate
release. This prevents a failed quarantine transaction from leaving a completed
projection temporarily eligible for export. The gate is process-local, coarse
and deliberately does not authorize generic cross-process mutation.

Artifact listing must join the typed bindings back to fresh source custody:
omit bindings for explicitly quarantined/partial recordings and propagate
storage or consistency errors rather than falling back to legacy listing.
Copies already obtained outside FUNG cannot be retracted by this mechanism.

The installed UAT must prove typed source/epoch bindings, WAV bytes and coverage,
quarantine races and failure retention, source separation and restart recovery.
The original PID gate and endpoint capture/mic/transcription/recovery gates are
unchanged. A source manifest alone does not establish that undocumented clock
gaps are silent: reject unproven missing coverage instead of filling it.

## Work and verification

- [ ] RED: v16→v17 upgrade retains existing rows and creates typed FK/indexes.
- [x] GREEN: additive schema and migration tests.
- [ ] RED: per-source WAV, checksum and typed binding with no mixed PCM.
- [ ] RED: reject missing/unverified coverage and quarantined source; wrong scope.
- [ ] RED: quarantine/publication race, stale frontier, commit failure/retry,
      uncertain publication and unrelated recording isolation.
- [x] GREEN: source-aware export through existing job; preserve legacy dispatch.
- [x] Independent source/schema/recovery review and exact-tree CPU tests.
- [ ] Installed end-to-end device, export and recovery UAT; keep release gated.

The combined candidate passed 1,103 library tests (15 ignored) and strict
all-target clippy on 2026-11-01. Unchecked RED entries are not retrospective
claims of an observed pre-implementation failure. See the
[verification report](../verification/implementation-reports/2026-11-01-endpoint-export-recovery.md)
for actual evidence and remaining acceptance.
