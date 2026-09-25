// The meeting-intelligence adapter is staged as a bounded foundation before
// the production meeting workflow wires every contract into a live caller.
// Keep its tested candidate surface visible without weakening clippy elsewhere.
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use crate::meeting_intelligence_schema;
use crate::meeting_intelligence_schema::{
    audio_range_is_covered, canonical_sha256, open_person_identity, AtomicMeetingRequest,
    AuthorizedPerson, CommittedMeetingEvent, IdentityAadContext, IdentityKeyBackend,
    MeetingCommitAttempt, MeetingIngestBatchRequest, MeetingIngestBatchResult, MeetingReplayCursor,
    MeetingReplayPage, MeetingRevisionRequest, MeetingScope, MeetingTranscriptSnapshot,
    OsPeopleMetadataKeyBackend, ParticipantAttribution, PrivateIdentityReference,
    SourceCoverageInput, SourceCoverageKind, TranscriptOrigin,
};
use genesis_block_native::{
    BatchInput, EdgeInput, GenesisTransaction, NodeInput, RelationalColumn, RelationalColumnType,
    RelationalFilter, RelationalForeignKey, RelationalIndex, RelationalMutationGroup,
    RelationalMutationKind, RelationalQuery, RelationalRowMutation, RelationalSchemaPackage,
    RelationalTable, Storage,
};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, Weak,
    },
};
use uuid::Uuid;
use zeroize::Zeroize;

pub(crate) const NAMESPACE: &str = "fung_mobile";

/// The most rows one relational query can return — now a page size.
///
/// `Storage::query_relational` rejects any limit outside `1..1000`, and
/// `RelationalFilter` is equality-only. Since the engine grew
/// `RelationalQuery::offset`, this is no longer a ceiling on what a caller
/// can read: [`query_all`] pages past it. It remains the bound on a single
/// read.
///
/// Stated once here because it had been stated eight times: `ENGINE_ROW_CAP`,
/// `QUERY_ROW_CEILING`, `QUERY_LIMIT`, `GENESIS_QUERY_LIMIT`,
/// `SEGMENT_READ_CAP`, and a bare `1000`. Six names for one engine constant
/// is how two readers came to hit it without noticing.
pub(crate) const ROW_CAP: u32 = 1000;

/// Every row the filters match, read in [`ROW_CAP`]-sized pages.
///
/// Use this for any read whose row count is driven by how long a recording
/// is — transcript segments, audio chunks. Use [`query`] when the limit is a
/// genuine "give me at most N" — a single row by id, the top 12 matches.
///
/// Passing an offset makes the engine order each page by the base table's
/// primary key, so consecutive pages partition the result set rather than
/// sampling an unordered scan. Callers that need a domain order (`start_ms`,
/// `sequence_no`) still sort the collected rows themselves, as before.
pub(crate) fn query_all(
    storage: &Storage,
    table: &str,
    columns: &[&str],
    filters: Vec<RelationalFilter>,
) -> Result<Vec<Value>, String> {
    let mut rows: Vec<Value> = Vec::new();
    let mut offset: u32 = 0;
    loop {
        let page = query_at(storage, table, columns, filters.clone(), ROW_CAP, offset)?;
        let filled = page.len() as u32 >= ROW_CAP;
        rows.extend(page);
        // A page that came back short is the last one. A page that came back
        // exactly full is indistinguishable from a cut one, so read on: the
        // next page is empty when the boundary was exact.
        if !filled {
            return Ok(rows);
        }
        offset = offset
            .checked_add(ROW_CAP)
            .ok_or_else(|| "relational read exceeded the addressable offset range".to_string())?;
    }
}

fn required(name: &str, column_type: RelationalColumnType) -> RelationalColumn {
    RelationalColumn::required(name, column_type)
}

fn nullable(name: &str, column_type: RelationalColumnType) -> RelationalColumn {
    RelationalColumn {
        name: name.to_string(),
        column_type,
        nullable: true,
        default: None,
    }
}

fn table(
    name: &str,
    columns: Vec<RelationalColumn>,
    foreign_keys: Vec<RelationalForeignKey>,
    indexes: Vec<RelationalIndex>,
) -> RelationalTable {
    RelationalTable {
        name: name.to_string(),
        columns,
        primary_key: vec!["id".to_string()],
        foreign_keys,
        indexes,
    }
}

fn fk(column: &str, referenced_table: &str) -> RelationalForeignKey {
    RelationalForeignKey {
        columns: vec![column.to_string()],
        referenced_table: referenced_table.to_string(),
        referenced_columns: vec!["id".to_string()],
    }
}

fn schema_v1() -> RelationalSchemaPackage {
    use RelationalColumnType::{Real, Text};
    RelationalSchemaPackage {
        namespace: NAMESPACE.to_string(),
        schema_version: 1,
        previous_version: None,
        package_id: "79b5fb04-9eb0-43b4-8c01-6d1b040ebdc9".to_string(),
        schema_hash: String::new(),
        tables: vec![
            table(
                "projects",
                vec![
                    required("id", Text),
                    required("name", Text),
                    required("storage_path", Text),
                    required("created_at", Text),
                    required("updated_at", Text),
                ],
                vec![],
                vec![],
            ),
            table(
                "notes",
                vec![
                    required("id", Text),
                    required("project_id", Text),
                    required("title", Text),
                    required("current_revision_id", Text),
                    required("created_at", Text),
                    required("updated_at", Text),
                ],
                vec![fk("project_id", "projects")],
                vec![],
            ),
            table(
                "note_revisions",
                vec![
                    required("id", Text),
                    required("note_id", Text),
                    required("body", Text),
                    nullable("evidence_label", Text),
                    required("author_device_id", Text),
                    required("logical_clock", Text),
                    required("created_at", Text),
                ],
                vec![fk("note_id", "notes")],
                vec![],
            ),
            table(
                "graph_nodes",
                vec![
                    required("id", Text),
                    required("project_id", Text),
                    required("entity_type", Text),
                    required("entity_id", Text),
                    required("label", Text),
                    required("position_x", Real),
                    required("position_y", Real),
                    required("created_at", Text),
                    required("updated_at", Text),
                ],
                vec![fk("project_id", "projects")],
                vec![],
            ),
            table(
                "graph_edges",
                vec![
                    required("id", Text),
                    required("project_id", Text),
                    required("source_node_id", Text),
                    required("target_node_id", Text),
                    required("predicate", Text),
                    required("epistemic_status", Text),
                    required("provenance_json", Text),
                    required("created_at", Text),
                    required("updated_at", Text),
                ],
                vec![
                    fk("project_id", "projects"),
                    fk("source_node_id", "graph_nodes"),
                    fk("target_node_id", "graph_nodes"),
                ],
                vec![],
            ),
            table(
                "mutation_log",
                vec![
                    required("id", Text),
                    required("project_id", Text),
                    required("device_id", Text),
                    required("logical_clock", Text),
                    required("entity_type", Text),
                    required("entity_id", Text),
                    required("operation", Text),
                    required("payload_json", Text),
                    required("created_at", Text),
                ],
                vec![fk("project_id", "projects")],
                vec![],
            ),
        ],
        named_queries: vec![],
    }
}

fn schema_v2() -> RelationalSchemaPackage {
    use RelationalColumnType::{Integer, Text};
    let mut package = schema_v1();
    package.schema_version = 2;
    package.previous_version = Some(1);
    package.tables[0]
        .columns
        .push(nullable("active_recording_id", Text));
    package.tables.extend([
        table(
            "recordings",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("source", Text),
                nullable("input_path", Text),
                required("canonical_audio_path", Text),
                required("status", Text),
                required("duration_ms", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
        table(
            "mobile_recording_checkpoints",
            vec![
                required("id", Text),
                required("recording_id", Text),
                required("safe_offset_ms", Integer),
                required("segment_count", Integer),
                nullable("last_checksum", Text),
                required("updated_at", Text),
            ],
            vec![fk("recording_id", "recordings")],
            vec![],
        ),
        table(
            "audio_chunks",
            vec![
                required("id", Text),
                required("recording_id", Text),
                required("sequence_no", Integer),
                required("file_path", Text),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("byte_size", Integer),
                required("checksum", Text),
                required("created_at", Text),
            ],
            vec![fk("recording_id", "recordings")],
            vec![],
        ),
    ]);
    package
}

fn schema_v3() -> RelationalSchemaPackage {
    use RelationalColumnType::{Boolean, Integer, Json, Real, Text};
    let mut package = schema_v2();
    package.schema_version = 3;
    package.previous_version = Some(2);
    package.tables.extend([
        table(
            "paired_devices",
            vec![
                required("id", Text),
                required("name", Text),
                required("endpoint", Text),
                required("trust_state", Text),
                required("pairing_proof_hash", Text),
                required("capabilities_json", Json),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "capability_grants",
            vec![
                required("id", Text),
                required("device_id", Text),
                required("project_id", Text),
                required("capabilities_json", Json),
                nullable("expires_at", Text),
                nullable("revoked_at", Text),
                required("created_at", Text),
            ],
            vec![
                fk("device_id", "paired_devices"),
                fk("project_id", "projects"),
            ],
            vec![],
        ),
        table(
            "delegated_jobs",
            vec![
                required("id", Text),
                required("project_id", Text),
                nullable("executor_device_id", Text),
                required("operation", Text),
                required("state", Text),
                required("progress", Integer),
                required("input_manifest_hash", Text),
                nullable("checkpoint_json", Json),
                required("observed_at", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("executor_device_id", "paired_devices"),
            ],
            vec![],
        ),
        table(
            "speakers",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("key", Text),
                required("display_name", Text),
                nullable("confidence", Real),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
        table(
            "model_providers",
            vec![
                required("id", Text),
                required("label", Text),
                required("runtime_location", Text),
                required("kind", Text),
                required("enabled", Boolean),
                required("config_json", Json),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "model_runs",
            vec![
                required("id", Text),
                required("recording_id", Text),
                required("provider_id", Text),
                required("model_name", Text),
                required("task_kind", Text),
                required("runtime_location", Text),
                required("input_ref", Text),
                required("output_ref", Text),
                required("parameters_json", Json),
                required("created_at", Text),
            ],
            vec![
                fk("recording_id", "recordings"),
                fk("provider_id", "model_providers"),
            ],
            vec![],
        ),
        table(
            "speaker_turns",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("speaker_id", Text),
                required("start_ms", Integer),
                required("end_ms", Integer),
                nullable("confidence", Real),
                required("status", Text),
                nullable("model_run_id", Text),
                required("overlap", Boolean),
                required("revision", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("speaker_id", "speakers"),
                fk("model_run_id", "model_runs"),
            ],
            vec![],
        ),
        table(
            "speaker_timeline_revisions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("operation", Text),
                required("payload_json", Json),
                required("created_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
        table(
            "waveform_tiles",
            vec![
                required("id", Text),
                required("recording_id", Text),
                required("zoom_level", Integer),
                required("tile_index", Integer),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("peaks_json", Json),
                required("checksum", Text),
                required("created_at", Text),
            ],
            vec![fk("recording_id", "recordings")],
            vec![],
        ),
        table(
            "story_sequences",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("title", Text),
                required("duration_ms", Integer),
                required("current_revision", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
        table(
            "story_clips",
            vec![
                required("id", Text),
                required("sequence_id", Text),
                nullable("source_turn_id", Text),
                required("source_recording_id", Text),
                required("source_start_ms", Integer),
                required("source_end_ms", Integer),
                required("timeline_start_ms", Integer),
                required("speaker_id", Text),
                nullable("effect_chain_id", Text),
                required("revision", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("sequence_id", "story_sequences"),
                fk("source_turn_id", "speaker_turns"),
                fk("source_recording_id", "recordings"),
                fk("speaker_id", "speakers"),
            ],
            vec![],
        ),
        table(
            "story_revisions",
            vec![
                required("id", Text),
                required("sequence_id", Text),
                required("operation", Text),
                required("before_json", Json),
                required("after_json", Json),
                required("applied", Boolean),
                required("author_device_id", Text),
                required("created_at", Text),
            ],
            vec![fk("sequence_id", "story_sequences")],
            vec![],
        ),
        table(
            "voice_profiles",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("display_name", Text),
                required("rights_basis", Text),
                required("rights_evidence_ref", Text),
                required("rights_state", Text),
                nullable("provider_id", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("provider_id", "model_providers"),
            ],
            vec![],
        ),
        table(
            "effect_chains",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("owner_kind", Text),
                required("owner_id", Text),
                required("label", Text),
                required("bypassed", Boolean),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
        table(
            "effect_nodes",
            vec![
                required("id", Text),
                required("chain_id", Text),
                required("position", Integer),
                required("kind", Text),
                required("parameters_json", Json),
                required("bypassed", Boolean),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("chain_id", "effect_chains")],
            vec![],
        ),
        table(
            "model_packages",
            vec![
                required("id", Text),
                required("label", Text),
                required("provider_kind", Text),
                required("model_version", Text),
                required("size_bytes", Integer),
                nullable("checksum", Text),
                required("runtime_location", Text),
                required("install_state", Text),
                required("compatibility_json", Json),
                required("languages_json", Json),
                nullable("license_ref", Text),
                required("observed_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "transcript_segments",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                nullable("speaker_id", Text),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("text", Text),
                nullable("confidence", Real),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("speaker_id", "speakers"),
            ],
            vec![],
        ),
        table(
            "transcript_refinement_proposals",
            vec![
                required("id", Text),
                required("project_id", Text),
                nullable("transcript_segment_id", Text),
                required("original_text", Text),
                required("proposed_text", Text),
                required("policy", Text),
                nullable("model_run_id", Text),
                required("status", Text),
                nullable("reviewed_at", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("transcript_segment_id", "transcript_segments"),
                fk("model_run_id", "model_runs"),
            ],
            vec![],
        ),
        table(
            "agent_voice_grants",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("mcp_client_id", Text),
                required("voice_profile_id", Text),
                required("capability", Text),
                required("granted_at", Text),
                nullable("expires_at", Text),
                nullable("revoked_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("voice_profile_id", "voice_profiles"),
            ],
            vec![],
        ),
        table(
            "agent_voice_sessions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("mcp_client_id", Text),
                required("voice_profile_id", Text),
                required("grant_id", Text),
                required("requested_text_hash", Text),
                required("state", Text),
                required("retain_output", Boolean),
                nullable("stop_actor", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("voice_profile_id", "voice_profiles"),
                fk("grant_id", "agent_voice_grants"),
            ],
            vec![],
        ),
        table(
            "jobs",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("type", Text),
                required("status", Text),
                required("progress", Integer),
                required("input_refs_json", Json),
                required("output_refs_json", Json),
                nullable("provider_id", Text),
                nullable("error_code", Text),
                nullable("error_message", Text),
                required("attempt_no", Integer),
                nullable("started_at", Text),
                nullable("finished_at", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("provider_id", "model_providers"),
            ],
            vec![],
        ),
        table(
            "job_events",
            vec![
                required("id", Text),
                required("job_id", Text),
                required("status", Text),
                required("message", Text),
                required("created_at", Text),
            ],
            vec![fk("job_id", "jobs")],
            vec![],
        ),
        table(
            "audit_events",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("event_type", Text),
                required("actor", Text),
                required("payload_json", Json),
                required("created_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
    ]);
    package
}

fn schema_v4() -> RelationalSchemaPackage {
    use RelationalColumnType::{Json, Text};
    let mut package = schema_v3();
    package.schema_version = 4;
    package.previous_version = Some(3);
    package.tables.extend([
        table(
            "external_connections",
            vec![
                required("id", Text),
                required("provider", Text),
                required("account_label", Text),
                required("status", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "external_imports",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("provider", Text),
                required("external_uuid", Text),
                required("recording_id", Text),
                required("payload_json", Json),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
            ],
            vec![],
        ),
    ]);
    package
}

fn schema_v5() -> RelationalSchemaPackage {
    use RelationalColumnType::{Integer, Text};
    let mut package = schema_v4();
    package.schema_version = 5;
    package.previous_version = Some(4);
    package.tables.push(table(
        "tts_test_results",
        vec![
            required("id", Text),
            required("provider_id", Text),
            required("status", Text),
            nullable("latency_ms", Integer),
            nullable("sample_audio_path", Text),
            nullable("error_message", Text),
            required("tested_at", Text),
        ],
        vec![fk("provider_id", "model_providers")],
        vec![],
    ));
    package
}

/// Phase 2 FUNGWIRE: the mobile side of a pairing needs the desktop peer's
/// ed25519 public key cached locally for the Noise KK handshake, mirroring
/// desktop's `paired_devices.db` (see Task 5 report). Column is nullable —
/// rows written before this task, or pairings completed before the peer's
/// key was fetched, simply carry `public_key = NULL`.
fn schema_v6() -> RelationalSchemaPackage {
    use RelationalColumnType::Text;
    let mut package = schema_v5();
    package.schema_version = 6;
    package.previous_version = Some(5);
    if let Some(paired_devices) = package
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == "paired_devices")
    {
        paired_devices.columns.push(nullable("public_key", Text));
    }
    package
}

/// Phase 3 BYOM: the desktop FUNGWIRE worker needs to record, per delegated
/// job, whether it ran on the local pipeline or via a cloud provider — the
/// mobile client persists this so the "☁ คลาวด์" badge (spec §10) survives
/// an app restart/reconnect, not just the in-flight wire manifest.
fn schema_v7() -> RelationalSchemaPackage {
    use RelationalColumnType::Text;
    let mut package = schema_v6();
    package.schema_version = 7;
    package.previous_version = Some(6);
    if let Some(delegated_jobs) = package
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == "delegated_jobs")
    {
        delegated_jobs.columns.push(nullable("executor", Text));
    }
    package
}

/// Live Meeting MVP: `summaries` and `export_artifacts` are specified in the
/// entity contract (and existed in the retired SQLite DDL) but never had
/// Genesis tables — nothing could persist a summary or an export until now.
/// Column sets mirror the contract exactly; `export_artifacts.source_layer_id`
/// stays a plain nullable text ref because `audio_layers` has no Genesis
/// table yet.
fn schema_v8() -> RelationalSchemaPackage {
    use RelationalColumnType::{Json, Text};
    let mut package = schema_v7();
    package.schema_version = 8;
    package.previous_version = Some(7);
    package.tables.extend([
        table(
            "summaries",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("kind", Text),
                required("content", Text),
                required("evidence_refs_json", Json),
                required("model_run_id", Text),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("model_run_id", "model_runs"),
            ],
            vec![],
        ),
        table(
            "export_artifacts",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("kind", Text),
                required("file_path", Text),
                nullable("source_layer_id", Text),
                required("created_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![],
        ),
    ]);
    package
}

/// Controlled external MCP retrieval metadata. This schema stores connector
/// identity, grants, immutable previews, runs, and sanitized results only.
/// Credential values remain OS-keyring owned; `credential_ref` is a nullable
/// non-secret locator so rows created before v9 continue to upgrade safely.
fn schema_v9() -> RelationalSchemaPackage {
    use RelationalColumnType::{Integer, Json, Text};
    let mut package = schema_v8();
    package.schema_version = 9;
    package.previous_version = Some(8);

    if let Some(external_connections) = package
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == "external_connections")
    {
        external_connections.columns.extend([
            nullable("transport", Text),
            nullable("endpoint", Text),
            nullable("credential_ref", Text),
            nullable("capabilities_json", Json),
        ]);
    }

    package.tables.extend([
        table(
            "meeting_tool_grants",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("connector_id", Text),
                required("capabilities_json", Json),
                required("granted_at", Text),
                required("expires_at", Text),
                nullable("revoked_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("connector_id", "external_connections"),
            ],
            vec![],
        ),
        table(
            "external_tool_previews",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("connector_id", Text),
                required("tool_name", Text),
                required("capability", Text),
                required("arguments_hash", Text),
                required("approved_fields_json", Json),
                required("evidence_refs_json", Json),
                required("state", Text),
                required("expires_at", Text),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("connector_id", "external_connections"),
            ],
            vec![],
        ),
        table(
            "external_tool_runs",
            vec![
                required("id", Text),
                required("preview_id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("connector_id", Text),
                required("tool_name", Text),
                required("capability", Text),
                required("request_hash", Text),
                nullable("output_hash", Text),
                required("status", Text),
                required("started_at", Text),
                nullable("finished_at", Text),
                nullable("error_code", Text),
                nullable("result_ref", Text),
            ],
            vec![
                fk("preview_id", "external_tool_previews"),
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("connector_id", "external_connections"),
            ],
            vec![],
        ),
        table(
            "external_tool_results",
            vec![
                required("id", Text),
                required("run_id", Text),
                required("mime_type", Text),
                required("sanitized_payload_json", Json),
                required("source_refs_json", Json),
                required("byte_size", Integer),
                required("created_at", Text),
            ],
            vec![fk("run_id", "external_tool_runs")],
            vec![],
        ),
    ]);
    package
}

/// Two columns the transcript pipeline had been doing without.
///
/// `recordings.language` — the language a session was captured in was handed
/// to the whisper worker and then forgotten, so any later pass over the same
/// audio (the catch-up transcription, a recovered recording) ran with no
/// language at all. Whisper re-detects per chunk, and on a short or noisy
/// chunk that produces confident text in the wrong language rather than an
/// error, which is worse than no text.
///
/// `audio_chunks.transcribed_at` — a chunk counted as needing transcription
/// whenever no segment covered it, which is indistinguishable from a chunk
/// that *was* transcribed and turned out to be silence. Those re-queued on
/// every catch-up pass forever. The stamp records that the transcriber has
/// seen the chunk, which is a different fact from whether it produced words.
///
/// Both nullable, so existing rows keep working. A recording written before
/// this carries `language = NULL` and behaves exactly as it does today, and a
/// chunk with `transcribed_at = NULL` that already has segments is still
/// recognised as covered. Only a pre-existing *silent* chunk is offered once
/// more, and that pass stamps it — the migration heals itself rather than
/// needing a backfill.
fn schema_v10() -> RelationalSchemaPackage {
    use RelationalColumnType::Text;
    let mut package = schema_v9();
    package.schema_version = 10;
    package.previous_version = Some(9);
    for (table_name, column) in [
        ("recordings", "language"),
        ("audio_chunks", "transcribed_at"),
    ] {
        let target = package
            .tables
            .iter_mut()
            .find(|candidate| candidate.name == table_name)
            .expect("table must exist in the previous schema version");
        target.columns.push(nullable(column, Text));
    }
    package
}

/// Meeting-intelligence v1 is an add-only extension of the frozen v10 chain.
/// The tables keep source custody, revision history, canonical projection,
/// evidence policy, agent intent, destination binding, and identity proposals
/// separate so later lanes cannot infer authority from a convenient label.
fn schema_v11() -> RelationalSchemaPackage {
    use RelationalColumnType::{Boolean, Integer, Json, Real, Text};
    let mut package = schema_v10();
    package.schema_version = 11;
    package.previous_version = Some(10);
    package.package_id = "4f8f0f5e-4d7d-4ef6-a2b6-2e5cfb6c7e11".to_string();
    package.tables.extend([
        table(
            "meeting_sessions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("session_generation", Integer),
                required("source_mode", Text),
                required("state", Text),
                required("owner_scope", Text),
                required("policy_version", Text),
                required("revision", Integer),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_sessions_recording_generation".to_string(),
                columns: vec!["recording_id".to_string(), "session_generation".to_string()],
                unique: true,
            }],
        ),
        table(
            "meeting_source_sessions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_kind", Text),
                required("source_generation", Integer),
                required("state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                nullable("ended_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_source_sessions_scope".to_string(),
                columns: vec![
                    "meeting_session_id".to_string(),
                    "source_generation".to_string(),
                ],
                unique: true,
            }],
        ),
        table(
            "meeting_source_coverage",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("track_id", Text),
                required("source_generation", Integer),
                required("sequence_no", Integer),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("coverage_kind", Text),
                nullable("audio_chunk_id", Text),
                nullable("file_path", Text),
                nullable("byte_size", Integer),
                nullable("checksum", Text),
                nullable("gap_reason", Text),
                required("payload_hash", Text),
                required("contract_version", Integer),
                required("finalized_at", Text),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
                fk("audio_chunk_id", "audio_chunks"),
            ],
            vec![
                RelationalIndex {
                    name: "idx_meeting_source_coverage_sequence".to_string(),
                    columns: vec![
                        "source_session_id".to_string(),
                        "track_id".to_string(),
                        "source_generation".to_string(),
                        "sequence_no".to_string(),
                    ],
                    unique: true,
                },
                RelationalIndex {
                    name: "idx_meeting_source_coverage_range".to_string(),
                    columns: vec![
                        "recording_id".to_string(),
                        "source_session_id".to_string(),
                        "track_id".to_string(),
                        "source_generation".to_string(),
                        "start_ms".to_string(),
                    ],
                    unique: false,
                },
            ],
        ),
        table(
            "meeting_source_cursors",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("track_id", Text),
                required("source_generation", Integer),
                required("last_sequence", Integer),
                required("last_end_ms", Integer),
                required("last_event_cursor", Integer),
                required("contract_version", Integer),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_source_cursors_scope".to_string(),
                columns: vec![
                    "source_session_id".to_string(),
                    "track_id".to_string(),
                    "source_generation".to_string(),
                ],
                unique: true,
            }],
        ),
        table(
            "transcript_revisions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("utterance_id", Text),
                required("revision", Integer),
                nullable("supersedes_revision", Integer),
                nullable("expected_revision", Integer),
                required("state", Text),
                required("origin", Text),
                required("raw_text", Text),
                required("effective_text", Text),
                nullable("language", Text),
                nullable("confidence", Real),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("audio_refs_json", Json),
                required("attribution_json", Json),
                nullable("model_run_id", Text),
                required("review_state", Text),
                required("payload_hash", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
                fk("model_run_id", "model_runs"),
            ],
            vec![RelationalIndex {
                name: "idx_transcript_revisions_utterance_revision".to_string(),
                columns: vec![
                    "meeting_session_id".to_string(),
                    "utterance_id".to_string(),
                    "revision".to_string(),
                ],
                unique: true,
            }],
        ),
        table(
            "transcript_projection",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("utterance_id", Text),
                required("revision_id", Text),
                required("revision", Integer),
                required("state", Text),
                required("effective_text", Text),
                nullable("language", Text),
                nullable("confidence", Real),
                required("review_state", Text),
                required("source_event_id", Text),
                required("contract_version", Integer),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("revision_id", "transcript_revisions"),
            ],
            vec![RelationalIndex {
                name: "idx_transcript_projection_recording".to_string(),
                columns: vec!["recording_id".to_string(), "revision".to_string()],
                unique: false,
            }],
        ),
        table(
            "transcript_event_log",
            vec![
                required("id", Text),
                required("transaction_id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("source_generation", Integer),
                required("cursor", Integer),
                required("event_type", Text),
                required("revision_id", Text),
                required("payload_json", Json),
                required("payload_hash", Text),
                required("contract_version", Integer),
                required("committed_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
                fk("revision_id", "transcript_revisions"),
            ],
            vec![RelationalIndex {
                name: "idx_transcript_event_log_recording_cursor".to_string(),
                columns: vec!["recording_id".to_string(), "cursor".to_string()],
                unique: true,
            }],
        ),
        table(
            "knowledge_collections",
            vec![
                required("id", Text),
                nullable("project_id", Text),
                required("owner_scope", Text),
                required("classification", Text),
                required("read_policy_ref", Text),
                required("share_policy_ref", Text),
                required("revision", Integer),
                required("status", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("project_id", "projects")],
            vec![RelationalIndex {
                name: "idx_knowledge_collections_owner".to_string(),
                columns: vec!["owner_scope".to_string(), "id".to_string()],
                unique: true,
            }],
        ),
        table(
            "knowledge_documents",
            vec![
                required("id", Text),
                required("collection_id", Text),
                required("source_kind", Text),
                required("source_ref", Text),
                required("title_ref", Text),
                nullable("current_version_id", Text),
                required("acl_revision", Integer),
                required("status", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("collection_id", "knowledge_collections")],
            vec![RelationalIndex {
                name: "idx_knowledge_documents_collection".to_string(),
                columns: vec!["collection_id".to_string(), "id".to_string()],
                unique: true,
            }],
        ),
        table(
            "knowledge_document_versions",
            vec![
                required("id", Text),
                required("document_id", Text),
                required("version_no", Integer),
                required("version_label", Text),
                required("content_hash", Text),
                required("custody_ref", Text),
                required("mime_type", Text),
                nullable("source_modified_at", Text),
                required("ingested_at", Text),
                required("parser_version", Text),
                required("validity_json", Json),
                required("state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![fk("document_id", "knowledge_documents")],
            vec![RelationalIndex {
                name: "idx_knowledge_document_versions_document".to_string(),
                columns: vec!["document_id".to_string(), "version_no".to_string()],
                unique: true,
            }],
        ),
        table(
            "knowledge_chunks",
            vec![
                required("id", Text),
                required("version_id", Text),
                required("text_ref", Text),
                required("locator_json", Json),
                nullable("token_start", Integer),
                nullable("token_end", Integer),
                nullable("byte_start", Integer),
                nullable("byte_end", Integer),
                required("extraction_quality_json", Json),
                required("index_generation", Integer),
                required("content_hash", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![fk("version_id", "knowledge_document_versions")],
            vec![RelationalIndex {
                name: "idx_knowledge_chunks_version".to_string(),
                columns: vec!["version_id".to_string(), "id".to_string()],
                unique: true,
            }],
        ),
        table(
            "knowledge_metric_observations",
            vec![
                required("id", Text),
                required("version_id", Text),
                required("metric_key", Text),
                required("organization_ref", Text),
                nullable("period_start", Text),
                nullable("period_end", Text),
                required("calendar", Text),
                required("decimal_value", Text),
                required("unit", Text),
                nullable("currency", Text),
                required("scale", Text),
                required("actual_budget", Text),
                required("locator_json", Json),
                nullable("extraction_confidence", Real),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![fk("version_id", "knowledge_document_versions")],
            vec![],
        ),
        table(
            "knowledge_index_runs",
            vec![
                required("id", Text),
                required("input_versions_json", Json),
                required("model_fingerprint", Text),
                required("config_fingerprint", Text),
                required("state", Text),
                required("coverage_json", Json),
                nullable("errors_json", Json),
                required("timings_json", Json),
                required("contract_version", Integer),
                required("created_at", Text),
                nullable("completed_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "knowledge_evidence_bundles",
            vec![
                required("id", Text),
                required("project_id", Text),
                nullable("meeting_session_id", Text),
                required("query_ref", Text),
                nullable("trigger_ref", Text),
                required("policy_snapshot_json", Json),
                required("acl_snapshot_json", Json),
                required("selected_refs_json", Json),
                required("output_hash", Text),
                required("state", Text),
                required("share_state", Text),
                nullable("expires_at", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("meeting_session_id", "meeting_sessions"),
            ],
            vec![],
        ),
        table(
            "meeting_agent_grants",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("meeting_session_id", Text),
                required("owner_scope", Text),
                required("mode", Text),
                required("capabilities_json", Json),
                required("collection_ids_json", Json),
                required("destination_policy_json", Json),
                required("policy_version", Text),
                required("state", Text),
                required("expected_revision", Integer),
                required("granted_at", Text),
                nullable("expires_at", Text),
                nullable("revoked_at", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("meeting_session_id", "meeting_sessions"),
            ],
            vec![],
        ),
        table(
            "meeting_agent_runs",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                nullable("grant_id", Text),
                required("trigger_id", Text),
                required("transcript_cursor", Integer),
                required("transcript_revision_set_json", Json),
                required("evidence_ids_json", Json),
                required("policy_version", Text),
                required("state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("grant_id", "meeting_agent_grants"),
            ],
            vec![],
        ),
        table(
            "meeting_destinations",
            vec![
                required("id", Text),
                nullable("agent_run_id", Text),
                required("provider_account_ref_ciphertext_ref", Text),
                required("provider_account_ref_sha256", Text),
                required("occurrence_key", Text),
                required("channel_type", Text),
                required("channel_id_ciphertext_ref", Text),
                required("channel_id_sha256", Text),
                nullable("thread_id_ciphertext_ref", Text),
                required("audience_policy_revision", Integer),
                required("approval_snapshot_json", Json),
                required("state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![fk("agent_run_id", "meeting_agent_runs")],
            vec![],
        ),
        table(
            "meeting_delivery_outbox",
            vec![
                required("id", Text),
                required("agent_run_id", Text),
                required("destination_id", Text),
                required("artifact_revision", Integer),
                nullable("evidence_bundle_id", Text),
                nullable("grant_id", Text),
                required("audience_policy_revision", Integer),
                required("payload_ciphertext_ref", Text),
                required("payload_hash", Text),
                required("idempotency_key", Text),
                required("attempt_no", Integer),
                nullable("lease_owner", Text),
                nullable("lease_expires_at", Text),
                required("state", Text),
                nullable("receipt_ref", Text),
                nullable("external_message_id_ref", Text),
                nullable("external_upload_id_ref", Text),
                nullable("last_error_code", Text),
                nullable("last_error_message", Text),
                nullable("unknown_at", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("agent_run_id", "meeting_agent_runs"),
                fk("destination_id", "meeting_destinations"),
                fk("evidence_bundle_id", "knowledge_evidence_bundles"),
                fk("grant_id", "meeting_agent_grants"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_delivery_idempotency".to_string(),
                columns: vec!["idempotency_key".to_string()],
                unique: true,
            }],
        ),
        table(
            "meeting_delivery_receipts",
            vec![
                required("id", Text),
                required("outbox_id", Text),
                required("provider_ref", Text),
                required("state", Text),
                nullable("external_message_id_ref", Text),
                nullable("external_upload_id_ref", Text),
                nullable("receipt_ciphertext_ref", Text),
                nullable("receipt_hash", Text),
                required("observed_at", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![fk("outbox_id", "meeting_delivery_outbox")],
            vec![],
        ),
        table(
            "meeting_participant_sessions",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                nullable("provider_account_ref_ciphertext_ref", Text),
                nullable("conference_occurrence_ref_ciphertext_ref", Text),
                nullable("provider_participant_ref_ciphertext", Text),
                nullable("provider_participant_ref_sha256", Text),
                nullable("provider_label_ciphertext_ref", Text),
                nullable("provider_label_ciphertext_sha256", Text),
                nullable("provider_label_ciphertext_json", Text),
                required("join_generation", Integer),
                required("source_generation", Integer),
                required("source_evidence_revision", Integer),
                required("identity_state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
            ],
            vec![],
        ),
        table(
            "meeting_participant_evidence",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("participant_session_id", Text),
                required("track_id", Text),
                required("source_generation", Integer),
                required("speaker_id", Text),
                required("start_ms", Integer),
                required("end_ms", Integer),
                required("source_kind", Text),
                required("label_snapshot_ref", Text),
                required("evidence_revision", Integer),
                required("source_digest", Text),
                required("attribution_state", Text),
                required("contract_version", Integer),
                required("created_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
                fk("participant_session_id", "meeting_participant_sessions"),
                fk("speaker_id", "speakers"),
            ],
            vec![],
        ),
        table(
            "identity_vaults",
            vec![
                required("id", Text),
                required("owner_principal_ref", Text),
                nullable("bound_account_ref", Text),
                nullable("self_person_ref_ciphertext_ref", Text),
                nullable("self_person_ref_ciphertext_sha256", Text),
                nullable("self_person_ref_ciphertext_json", Text),
                required("state", Text),
                required("key_store_namespace", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![],
            vec![],
        ),
        table(
            "participant_profiles",
            vec![
                required("id", Text),
                required("vault_id", Text),
                required("owner_scope", Text),
                required("profile_payload_ciphertext_ref", Text),
                required("profile_ciphertext_sha256", Text),
                required("key_ref", Text),
                required("status", Text),
                required("revision", Integer),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![fk("vault_id", "identity_vaults")],
            vec![RelationalIndex {
                name: "idx_participant_profiles_owner".to_string(),
                columns: vec!["owner_scope".to_string(), "id".to_string()],
                unique: true,
            }],
        ),
        table(
            "recording_participants",
            vec![
                required("id", Text),
                required("vault_id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                nullable("person_ref_ciphertext_ref", Text),
                nullable("person_ref_ciphertext_sha256", Text),
                nullable("person_ref_ciphertext_json", Text),
                nullable("role_alias_ciphertext_ref", Text),
                required("match_enabled", Boolean),
                nullable("consent_ref_ciphertext_ref", Text),
                required("revision", Integer),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("vault_id", "identity_vaults"),
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
            ],
            vec![],
        ),
        table(
            "speaker_identity_links",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("speaker_id", Text),
                required("vault_id", Text),
                nullable("person_ref_ciphertext_ref", Text),
                nullable("person_ref_ciphertext_sha256", Text),
                nullable("person_ref_ciphertext_json", Text),
                nullable("person_ref_key_ref", Text),
                required("match_source", Text),
                required("status", Text),
                nullable("match_score", Real),
                nullable("threshold_policy_id", Text),
                nullable("model_run_id", Text),
                nullable("evidence_ciphertext_ref", Text),
                nullable("evidence_ciphertext_sha256", Text),
                nullable("reviewer_ref_ciphertext_ref", Text),
                nullable("actor_ref_ciphertext_ref", Text),
                nullable("expected_revision", Integer),
                nullable("supersedes_id", Text),
                required("d8_policy_revision", Integer),
                required("revision", Integer),
                nullable("locked_at", Text),
                nullable("revoked_at", Text),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("vault_id", "identity_vaults"),
                fk("model_run_id", "model_runs"),
            ],
            vec![],
        ),
    ]);
    package
}

pub(crate) fn schema() -> RelationalSchemaPackage {
    schema_v13()
}

/// Link an optional model run to a proposal draft while preserving v1–v12.
fn schema_v13() -> RelationalSchemaPackage {
    let mut package = schema_v12();
    package.schema_version = 13;
    package.previous_version = Some(12);
    package.package_id = "9f2465d6-ec66-4fef-b77e-2547c5a576ab".to_string();
    let runs = package
        .tables
        .iter_mut()
        .find(|table| table.name == "meeting_agent_runs")
        .expect("v12 meeting_agent_runs schema");
    runs.columns
        .push(nullable("model_run_id", RelationalColumnType::Text));
    package
}

/// Add recording-wide transcript order and opaque encrypted-asset metadata.
/// Historical v1–v11 definitions remain unchanged.
fn schema_v12() -> RelationalSchemaPackage {
    use RelationalColumnType::{Boolean, Integer, Json, Text};
    let mut package = schema_v11();
    package.schema_version = 12;
    package.previous_version = Some(11);
    package.package_id = "4a3850d4-c912-4a15-8e89-3d1e20e4f3c7".to_string();
    package.tables.extend([
        table(
            "meeting_recording_cursors",
            vec![
                required("id", Text),
                required("recording_id", Text),
                required("last_cursor", Integer),
                required("minimum_retained_cursor", Integer),
                required("revision", Integer),
                required("contract_version", Integer),
                required("adoption_complete", Boolean),
                required("updated_at", Text),
            ],
            vec![fk("recording_id", "recordings")],
            vec![RelationalIndex {
                name: "idx_meeting_recording_cursor_recording".to_string(),
                columns: vec!["recording_id".to_string()],
                unique: true,
            }],
        ),
        table(
            "meeting_input_batches",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("source_session_id", Text),
                required("source_generation", Integer),
                required("first_sequence", Integer),
                required("last_sequence", Integer),
                required("operation_id", Text),
                required("payload_hash", Text),
                required("first_cursor", Integer),
                required("last_cursor", Integer),
                required("result_json", Json),
                required("transaction_id", Text),
                required("contract_version", Integer),
                required("committed_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_input_batches_source_sequence".to_string(),
                columns: vec![
                    "source_session_id".to_string(),
                    "source_generation".to_string(),
                    "first_sequence".to_string(),
                    "last_sequence".to_string(),
                ],
                unique: true,
            }],
        ),
        table(
            "meeting_control_events",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                nullable("source_session_id", Text),
                nullable("source_generation", Integer),
                required("cursor", Integer),
                required("event_type", Text),
                nullable("batch_id", Text),
                nullable("revision_id", Text),
                required("payload_json", Json),
                required("payload_hash", Text),
                required("transaction_id", Text),
                required("contract_version", Integer),
                required("committed_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("source_session_id", "meeting_source_sessions"),
                fk("batch_id", "meeting_input_batches"),
                fk("revision_id", "transcript_revisions"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_control_events_recording_cursor".to_string(),
                columns: vec!["recording_id".to_string(), "cursor".to_string()],
                unique: true,
            }],
        ),
        table(
            "meeting_session_contexts",
            vec![
                required("id", Text),
                required("project_id", Text),
                required("recording_id", Text),
                required("meeting_session_id", Text),
                required("owner_scope", Text),
                nullable("vault_id", Text),
                nullable("account_ref", Text),
                required("selected_collection_ids_json", Json),
                nullable("private_context_asset_id", Text),
                required("temporal_context_json", Json),
                required("policy_json", Json),
                required("state", Text),
                required("revision", Integer),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("recording_id", "recordings"),
                fk("meeting_session_id", "meeting_sessions"),
                fk("vault_id", "identity_vaults"),
                fk("private_context_asset_id", "meeting_private_assets"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_session_context_session".to_string(),
                columns: vec!["meeting_session_id".to_string()],
                unique: true,
            }],
        ),
        table(
            "meeting_private_assets",
            vec![
                required("id", Text),
                required("project_id", Text),
                nullable("vault_id", Text),
                required("owner_scope", Text),
                required("purpose", Text),
                required("entity_id", Text),
                required("entity_revision", Integer),
                required("custody_ref", Text),
                required("key_ref", Text),
                required("ciphertext_sha256", Text),
                required("ciphertext_bytes", Integer),
                required("state", Text),
                required("retention_json", Json),
                required("contract_version", Integer),
                required("created_at", Text),
                required("updated_at", Text),
            ],
            vec![
                fk("project_id", "projects"),
                fk("vault_id", "identity_vaults"),
            ],
            vec![RelationalIndex {
                name: "idx_meeting_private_assets_entity_revision".to_string(),
                columns: vec![
                    "project_id".to_string(),
                    "purpose".to_string(),
                    "entity_id".to_string(),
                    "entity_revision".to_string(),
                ],
                unique: true,
            }],
        ),
    ]);
    package
}

/// Registers the schema chain stepwise. Genesis requires a fresh database to
/// start at version 1 and advance one version at a time; on an existing
/// database the already-registered steps report a version conflict, which is
/// expected and skipped. Any other error — and any failure on the final
/// (current) package — is fatal.
pub(crate) fn install(storage: &Storage) -> Result<(), String> {
    let packages = [
        schema_v1(),
        schema_v2(),
        schema_v3(),
        schema_v4(),
        schema_v5(),
        schema_v6(),
        schema_v7(),
        schema_v8(),
        schema_v9(),
        schema_v10(),
        schema_v11(),
        schema_v12(),
        schema(),
    ];
    let last_index = packages.len() - 1;
    for (index, package) in packages.into_iter().enumerate() {
        match storage.register_relational_schema(package) {
            Ok(_) => {}
            Err(error) => {
                let message = error.to_string();
                if index < last_index && message.contains("REL_SCHEMA_VERSION_CONFLICT") {
                    continue; // step already registered on an existing database
                }
                return Err(message);
            }
        }
    }
    adopt_meeting_control_stream(storage)
}

/// Backfills the recording-wide v12 event stream atomically per recording.
/// Existing v1-v11 rows remain untouched; every adopted reference is stable,
/// and the completion marker commits with the events and cursor allocator.
fn adopt_meeting_control_stream(storage: &Storage) -> Result<(), String> {
    let recordings = query_all(
        storage,
        "recordings",
        &["id", "project_id", "created_at", "updated_at"],
        vec![],
    )?;
    for recording in recordings {
        let recording_id = string(&recording, "recordings.id")?;
        let project_id = string(&recording, "recordings.project_id")?;
        let updated_at = string(&recording, "recordings.updated_at")?;
        let transaction_id = format!(
            "mi-v12-adopt-{}",
            Sha256::digest(recording_id.as_bytes())[..16]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let expected_frontier = storage.txn_frontier();
        let cursor_id = format!("recording::{recording_id}");
        let existing = query(
            storage,
            "meeting_recording_cursors",
            &["adoption_complete"],
            vec![eq("meeting_recording_cursors", "id", json!(&cursor_id))],
            1,
        )?;
        if existing
            .first()
            .and_then(|row| row.get("meeting_recording_cursors.adoption_complete"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            continue;
        }

        let legacy_events = query_all(
            storage,
            "transcript_event_log",
            &[
                "id",
                "transaction_id",
                "project_id",
                "recording_id",
                "meeting_session_id",
                "source_session_id",
                "source_generation",
                "cursor",
                "event_type",
                "revision_id",
                "payload_json",
                "payload_hash",
                "contract_version",
                "committed_at",
            ],
            vec![eq(
                "transcript_event_log",
                "recording_id",
                json!(&recording_id),
            )],
        )?;
        let mut mutations = Vec::new();
        let mut cursor_values = legacy_events
            .iter()
            .map(|row| integer(row, "transcript_event_log.cursor"))
            .collect::<Result<Vec<_>, _>>()?;
        cursor_values.sort_unstable();
        for row in &legacy_events {
            let legacy_id = string(row, "transcript_event_log.id")?;
            mutations.push(upsert(
                "meeting_control_events",
                json!({
                    "id": format!("legacy-control::{legacy_id}"),
                    "project_id": string(row, "transcript_event_log.project_id")?,
                    "recording_id": recording_id,
                    "meeting_session_id": string(row, "transcript_event_log.meeting_session_id")?,
                    "source_session_id": row.get("transcript_event_log.source_session_id").cloned().unwrap_or(Value::Null),
                    "source_generation": row.get("transcript_event_log.source_generation").cloned().unwrap_or(Value::Null),
                    "cursor": integer(row, "transcript_event_log.cursor")?,
                    "event_type": string(row, "transcript_event_log.event_type")?,
                    "batch_id": null,
                    "revision_id": string(row, "transcript_event_log.revision_id")?,
                    "payload_json": row.get("transcript_event_log.payload_json").cloned().unwrap_or(Value::Null),
                    "payload_hash": string(row, "transcript_event_log.payload_hash")?,
                    "transaction_id": string(row, "transcript_event_log.transaction_id")?,
                    "contract_version": row.get("transcript_event_log.contract_version").cloned().unwrap_or(json!(1)),
                    "committed_at": string(row, "transcript_event_log.committed_at")?,
                }),
            ));
        }

        let mut snapshot_cursor = None;
        if legacy_events.is_empty() {
            let legacy_segments = query(
                storage,
                "transcript_segments",
                &["id"],
                vec![eq(
                    "transcript_segments",
                    "recording_id",
                    json!(&recording_id),
                )],
                1,
            )?;
            if !legacy_segments.is_empty() {
                let sessions = query(
                    storage,
                    "meeting_sessions",
                    &["id", "project_id", "recording_id"],
                    vec![eq("meeting_sessions", "recording_id", json!(&recording_id))],
                    1,
                )?;
                let session_id = if let Some(row) = sessions.first() {
                    string(row, "meeting_sessions.id")?
                } else {
                    let id = format!("legacy-session::{recording_id}");
                    mutations.push(upsert(
                        "meeting_sessions",
                        json!({
                            "id": id,
                            "project_id": project_id,
                            "recording_id": recording_id,
                            "session_generation": 1,
                            "source_mode": "legacy_read_only",
                            "state": "legacy_snapshot",
                            "owner_scope": "legacy_unverified",
                            "policy_version": "legacy_snapshot_v1",
                            "revision": 1,
                            "contract_version": 1,
                            "created_at": string(&recording, "recordings.created_at")?,
                            "updated_at": updated_at,
                        }),
                    ));
                    id
                };
                let event_id = format!("legacy-snapshot::{recording_id}");
                let payload = json!({
                    "contract_version": 1,
                    "event_type": "legacy_snapshot",
                    "recording_id": recording_id,
                    "source": "transcript_segments",
                    "read_only": true,
                    "coverage_verified": false,
                    "asr_provenance": "unknown",
                });
                let payload_hash = canonical_sha256(&payload)?;
                mutations.push(upsert(
                    "meeting_control_events",
                    json!({
                        "id": format!("legacy-control::{event_id}"),
                        "project_id": project_id,
                        "recording_id": recording_id,
                        "meeting_session_id": session_id,
                        "source_session_id": null,
                        "source_generation": null,
                        "cursor": 0,
                        "event_type": "legacy_snapshot",
                        "batch_id": null,
                        "revision_id": null,
                        "payload_json": payload,
                        "payload_hash": payload_hash,
                        "transaction_id": transaction_id,
                        "contract_version": 1,
                        "committed_at": updated_at,
                    }),
                ));
                snapshot_cursor = Some(0_i64);
            }
        }

        let last_cursor = cursor_values
            .last()
            .copied()
            .or(snapshot_cursor)
            .unwrap_or(-1);
        let minimum_retained_cursor = cursor_values
            .first()
            .copied()
            .or(snapshot_cursor)
            .unwrap_or(0);
        mutations.push(upsert(
            "meeting_recording_cursors",
            json!({
                "id": cursor_id,
                "recording_id": recording_id,
                "last_cursor": last_cursor,
                "minimum_retained_cursor": minimum_retained_cursor,
                "revision": 1,
                "contract_version": 1,
                "adoption_complete": true,
                "updated_at": updated_at,
            }),
        ));
        storage
            .commit_transaction(GenesisTransaction {
                transaction_id,
                expected_frontier: Some(expected_frontier),
                relational: vec![RelationalMutationGroup {
                    namespace: NAMESPACE.to_string(),
                    mutations,
                }],
                graph: BatchInput {
                    nodes: vec![],
                    edges: vec![],
                },
                vectors: vec![],
            })
            .map_err(|error| format!("meeting v12 stream adoption failed: {error}"))?;
    }
    Ok(())
}

/// SQLite has no Json/Boolean storage classes, so the retired `fung.db`
/// holds JSON as TEXT (`'[]'`, `'{}'`) and booleans as INTEGER 0/1. Genesis
/// enforces column types strictly (REL_TYPE_MISMATCH), which made every
/// legacy import crash the app at startup — the marker file was never
/// written, so it crashed on every subsequent launch too. Convert values to
/// the target column's type instead of passing raw SQLite storage through.
fn coerce_legacy_value(value: Value, column_type: &RelationalColumnType) -> Value {
    use RelationalColumnType::{Boolean, Integer, Json, Real};
    match (column_type, value) {
        (Json, Value::String(raw)) => {
            serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw))
        }
        (Boolean, Value::Number(number)) => {
            json!(number.as_i64().map(|n| n != 0).unwrap_or(false))
        }
        (Boolean, Value::String(raw)) => json!(raw == "true" || raw == "1"),
        (Real, Value::Number(number)) if number.is_i64() => {
            json!(number.as_i64().map(|n| n as f64).unwrap_or(0.0))
        }
        (Integer, Value::Number(number)) if number.is_f64() => match number.as_f64() {
            Some(float) if float.fract() == 0.0 => json!(float as i64),
            _ => Value::Number(number),
        },
        (_, value) => value,
    }
}

/// One-way compatibility import. The retired SQLite file is opened read-only;
/// every imported row becomes a normal signed Genesis transaction.
pub(crate) fn import_legacy_sqlite(storage: &Storage, path: &Path) -> Result<usize, String> {
    if !path.is_file() {
        return Ok(0);
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let package = schema();
    let priority = |name: &str| match name {
        "projects" => 0,
        "recordings" => 1,
        "paired_devices" | "model_providers" | "speakers" | "external_connections" => 2,
        "notes"
        | "mobile_recording_checkpoints"
        | "audio_chunks"
        | "delegated_jobs"
        | "jobs"
        | "meeting_tool_grants"
        | "external_tool_previews" => 3,
        "note_revisions"
        | "graph_nodes"
        | "model_runs"
        | "transcript_segments"
        | "story_sequences"
        | "voice_profiles"
        | "effect_chains"
        | "job_events"
        | "audit_events" => 4,
        "graph_edges"
        | "speaker_turns"
        | "waveform_tiles"
        | "story_clips"
        | "transcript_refinement_proposals"
        | "agent_voice_grants"
        | "effect_nodes"
        | "external_imports"
        | "external_tool_runs" => 5,
        "story_revisions"
        | "agent_voice_sessions"
        | "capability_grants"
        | "mutation_log"
        | "external_tool_results" => 6,
        _ => 7,
    };
    let mut tables = package.tables.clone();
    tables.sort_by_key(|table| priority(&table.name));
    let mut mutations = Vec::new();
    for table in tables {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [&table.name],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !exists {
            continue;
        }
        let mut column_statement = connection
            .prepare(&format!("PRAGMA table_info(\"{}\")", table.name))
            .map_err(|error| error.to_string())?;
        let existing_columns = column_statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .collect::<std::collections::HashSet<_>>();
        let columns = table
            .columns
            .iter()
            .filter(|column| existing_columns.contains(&column.name))
            .collect::<Vec<_>>();
        if columns.is_empty() {
            continue;
        }
        let sql = format!(
            "SELECT {} FROM \"{}\"",
            columns
                .iter()
                .map(|column| format!("\"{}\"", column.name))
                .collect::<Vec<_>>()
                .join(","),
            table.name
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let mut object = serde_json::Map::new();
                for (index, column) in columns.iter().enumerate() {
                    let value = match row.get_ref(index)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(value) => json!(value),
                        ValueRef::Real(value) => json!(value),
                        ValueRef::Text(value) => {
                            Value::String(String::from_utf8_lossy(value).into_owned())
                        }
                        ValueRef::Blob(value) => {
                            Value::Array(value.iter().copied().map(Value::from).collect())
                        }
                    };
                    object.insert(
                        column.name.clone(),
                        coerce_legacy_value(value, &column.column_type),
                    );
                }
                if table.name == "mobile_recording_checkpoints" && !object.contains_key("id") {
                    if let Some(value) = object.get("recording_id").cloned() {
                        object.insert("id".to_string(), value);
                    }
                }
                Ok(Value::Object(object))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            mutations.push(upsert(&table.name, row.map_err(|error| error.to_string())?));
        }
    }
    let count = mutations.len();
    if count > 0 {
        commit_rows(storage, mutations)?;
    }
    Ok(count)
}

pub(crate) fn delete(table: &str, id: &str) -> RelationalRowMutation {
    RelationalRowMutation {
        table: table.to_string(),
        kind: RelationalMutationKind::Delete,
        values: json!({}),
        key: Some(json!({"id": id})),
    }
}

pub(crate) fn commit_rows(
    storage: &Storage,
    mutations: Vec<RelationalRowMutation>,
) -> Result<(), String> {
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: Uuid::new_v4().to_string(),
            expected_frontier: None,
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations,
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn commit_rows_at_frontier(
    storage: &Storage,
    expected_frontier: u64,
    mutations: Vec<RelationalRowMutation>,
) -> Result<(), String> {
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: Uuid::new_v4().to_string(),
            expected_frontier: Some(expected_frontier),
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations,
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Capture the transaction identity and frontier before any validation read.
/// The returned value is caller-owned and must be reused after a possible
/// durable-commit uncertainty; regenerating any of its fields would change the
/// Genesis transaction payload and defeat identity-based reconciliation.
pub(crate) fn begin_meeting_commit(
    storage: &Storage,
    transaction_id: &str,
    committed_at: &str,
) -> Result<MeetingCommitAttempt, String> {
    if transaction_id.is_empty() || transaction_id.len() > 128 {
        return Err("invalid meeting transaction_id".to_string());
    }
    if committed_at.is_empty() {
        return Err("meeting committed_at is required".to_string());
    }
    Ok(MeetingCommitAttempt {
        transaction_id: transaction_id.to_string(),
        // This is deliberately the first storage read. All later scope and
        // custody reads are protected by the CAS in commit_transaction.
        expected_frontier: storage.txn_frontier(),
        committed_at: committed_at.to_string(),
    })
}

#[derive(Clone, Debug)]
struct VerifiedSourceCoverage {
    id: String,
    source_session_id: String,
    track_id: String,
    source_generation: i64,
    sequence_no: i64,
    start_ms: i64,
    end_ms: i64,
    kind: SourceCoverageKind,
    audio_chunk_id: Option<String>,
    file_path: Option<String>,
    byte_size: Option<i64>,
    checksum: Option<String>,
    gap_reason: Option<String>,
}

fn sha256_file(path: &str) -> Result<(i64, String), String> {
    let candidate = Path::new(path);
    if !candidate.is_absolute() || path.contains("://") || path.starts_with("\\\\") {
        return Err("audio custody path must be an absolute local path".to_string());
    }
    let before = std::fs::metadata(candidate)
        .map_err(|error| format!("audio custody file is not finalized/readable: {error}"))?;
    if !before.is_file() || before.len() == 0 {
        return Err("audio custody file must be a non-empty regular file".to_string());
    }
    let mut file = File::open(candidate)
        .map_err(|error| format!("audio custody file cannot be opened: {error}"))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_i64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("audio custody file read failed: {error}"))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as i64)
            .ok_or_else(|| "audio custody byte count overflow".to_string())?;
        hasher.update(&buffer[..read]);
    }
    let after = std::fs::metadata(candidate)
        .map_err(|error| format!("audio custody file changed during verification: {error}"))?;
    if before.len() != after.len() || before.len() != bytes as u64 {
        return Err("audio custody file changed during checksum verification".to_string());
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

fn verify_audio_coverage(
    storage: &Storage,
    scope: &MeetingScope,
    source: &SourceCoverageInput,
) -> Result<VerifiedSourceCoverage, String> {
    if source.kind == SourceCoverageKind::Gap {
        source.validate(scope)?;
        return Ok(VerifiedSourceCoverage {
            id: source.id.clone(),
            source_session_id: source.source_session_id.clone(),
            track_id: source.track_id.clone(),
            source_generation: source.source_generation,
            sequence_no: source.sequence_no,
            start_ms: source.start_ms,
            end_ms: source.end_ms,
            kind: SourceCoverageKind::Gap,
            audio_chunk_id: None,
            file_path: None,
            byte_size: None,
            checksum: None,
            gap_reason: source.gap_reason.clone(),
        });
    }

    let chunk_id = source
        .audio_chunk_id
        .as_deref()
        .ok_or_else(|| "audio coverage is missing audio_chunk_id".to_string())?;
    let row = query(
        storage,
        "audio_chunks",
        &[
            "recording_id",
            "file_path",
            "start_ms",
            "end_ms",
            "byte_size",
            "checksum",
        ],
        vec![eq("audio_chunks", "id", json!(chunk_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "audio coverage references a missing audio chunk".to_string())?;
    let recording_id = string(&row, "audio_chunks.recording_id")?;
    if recording_id != scope.recording_id {
        return Err("audio chunk recording scope mismatch".to_string());
    }
    let stored_path = string(&row, "audio_chunks.file_path")?;
    let stored_start = integer(&row, "audio_chunks.start_ms")?;
    let stored_end = integer(&row, "audio_chunks.end_ms")?;
    let stored_byte_size = integer(&row, "audio_chunks.byte_size")?;
    let stored_checksum = string(&row, "audio_chunks.checksum")?.to_ascii_lowercase();
    if stored_end <= stored_start || stored_byte_size <= 0 {
        return Err("audio chunk is not finalized with a valid range".to_string());
    }
    if source.start_ms != stored_start || source.end_ms != stored_end {
        return Err("caller audio range does not match durable audio chunk".to_string());
    }
    if source
        .file_path
        .as_deref()
        .is_some_and(|value| value != stored_path)
        || source
            .byte_size
            .is_some_and(|value| value != stored_byte_size)
        || source
            .checksum
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase() != stored_checksum)
    {
        return Err("caller audio custody assertion differs from durable chunk".to_string());
    }
    let (actual_byte_size, actual_checksum) = sha256_file(&stored_path)?;
    if actual_byte_size != stored_byte_size || actual_checksum != stored_checksum {
        return Err(
            "audio chunk checksum or byte size does not match the finalized file".to_string(),
        );
    }
    Ok(VerifiedSourceCoverage {
        id: source.id.clone(),
        source_session_id: source.source_session_id.clone(),
        track_id: source.track_id.clone(),
        source_generation: source.source_generation,
        sequence_no: source.sequence_no,
        start_ms: stored_start,
        end_ms: stored_end,
        kind: SourceCoverageKind::Audio,
        audio_chunk_id: Some(chunk_id.to_string()),
        file_path: Some(stored_path),
        byte_size: Some(actual_byte_size),
        checksum: Some(actual_checksum),
        gap_reason: None,
    })
}

fn require_meeting_scope(storage: &Storage, scope: &MeetingScope) -> Result<(), String> {
    let project = query(
        storage,
        "projects",
        &["id"],
        vec![eq("projects", "id", json!(&scope.project_id))],
        1,
    )?;
    if project.is_empty() {
        return Err("meeting scope references a missing project".to_string());
    }
    let recording = query(
        storage,
        "recordings",
        &["project_id"],
        vec![eq("recordings", "id", json!(&scope.recording_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "meeting scope references a missing recording".to_string())?;
    if string(&recording, "recordings.project_id")? != scope.project_id {
        return Err("recording is outside the requested project".to_string());
    }
    let session = query(
        storage,
        "meeting_sessions",
        &[
            "project_id",
            "recording_id",
            "session_generation",
            "revision",
            "state",
        ],
        vec![eq(
            "meeting_sessions",
            "id",
            json!(&scope.meeting_session_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "meeting scope references a missing meeting session".to_string())?;
    if string(&session, "meeting_sessions.project_id")? != scope.project_id
        || string(&session, "meeting_sessions.recording_id")? != scope.recording_id
        || integer(&session, "meeting_sessions.session_generation")? < 1
    {
        return Err("meeting session scope/generation mismatch".to_string());
    }
    let source_session = query(
        storage,
        "meeting_source_sessions",
        &[
            "project_id",
            "recording_id",
            "meeting_session_id",
            "source_generation",
        ],
        vec![eq(
            "meeting_source_sessions",
            "id",
            json!(&scope.source_session_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "meeting scope references a missing source session".to_string())?;
    if string(&source_session, "meeting_source_sessions.project_id")? != scope.project_id
        || string(&source_session, "meeting_source_sessions.recording_id")? != scope.recording_id
        || string(
            &source_session,
            "meeting_source_sessions.meeting_session_id",
        )? != scope.meeting_session_id
        || integer(&source_session, "meeting_source_sessions.source_generation")?
            != scope.source_generation
    {
        return Err("source session scope/generation mismatch".to_string());
    }
    Ok(())
}

fn source_cursor_id(scope: &MeetingScope) -> String {
    format!(
        "{}::{}::{}",
        scope.source_session_id, scope.track_id, scope.source_generation
    )
}

fn query_existing_event(
    storage: &Storage,
    event_id: &str,
) -> Result<Option<(String, String, i64, String)>, String> {
    query(
        storage,
        "transcript_event_log",
        &["transaction_id", "payload_hash", "cursor", "revision_id"],
        vec![eq("transcript_event_log", "id", json!(event_id))],
        1,
    )?
    .into_iter()
    .map(|row| {
        Ok((
            string(&row, "transcript_event_log.transaction_id")?,
            string(&row, "transcript_event_log.payload_hash")?,
            integer(&row, "transcript_event_log.cursor")?,
            string(&row, "transcript_event_log.revision_id")?,
        ))
    })
    .next()
    .transpose()
}

fn coverage_payload(source: &VerifiedSourceCoverage) -> Value {
    json!({
        "id": source.id,
        "source_session_id": source.source_session_id,
        "track_id": source.track_id,
        "source_generation": source.source_generation,
        "sequence_no": source.sequence_no,
        "start_ms": source.start_ms,
        "end_ms": source.end_ms,
        "coverage_kind": source.kind.as_str(),
        "audio_chunk_id": source.audio_chunk_id,
        "file_path": source.file_path,
        "byte_size": source.byte_size,
        "checksum": source.checksum,
        "gap_reason": source.gap_reason,
    })
}

fn check_coverage_conflicts(
    storage: &Storage,
    scope: &MeetingScope,
    verified: &[VerifiedSourceCoverage],
    revision_start: i64,
    revision_end: i64,
) -> Result<(), String> {
    let existing = query_all(
        storage,
        "meeting_source_coverage",
        &[
            "id",
            "source_session_id",
            "track_id",
            "source_generation",
            "sequence_no",
            "start_ms",
            "end_ms",
            "coverage_kind",
            "payload_hash",
        ],
        vec![eq(
            "meeting_source_coverage",
            "source_session_id",
            json!(&scope.source_session_id),
        )],
    )?;
    let mut audio_spans = Vec::new();
    for row in &existing {
        let same_scope = string(row, "meeting_source_coverage.track_id")? == scope.track_id
            && integer(row, "meeting_source_coverage.source_generation")?
                == scope.source_generation;
        if !same_scope {
            continue;
        }
        if string(row, "meeting_source_coverage.coverage_kind")? == "audio" {
            audio_spans.push((
                integer(row, "meeting_source_coverage.start_ms")?,
                integer(row, "meeting_source_coverage.end_ms")?,
            ));
        }
        for source in verified {
            if string(row, "meeting_source_coverage.id")? == source.id {
                let expected_hash = canonical_sha256(&coverage_payload(source))?;
                if string(row, "meeting_source_coverage.payload_hash")? != expected_hash {
                    return Err("source coverage identity conflict".to_string());
                }
                continue;
            }
            let same_sequence =
                integer(row, "meeting_source_coverage.sequence_no")? == source.sequence_no;
            let overlaps = integer(row, "meeting_source_coverage.start_ms")? < source.end_ms
                && source.start_ms < integer(row, "meeting_source_coverage.end_ms")?;
            if same_sequence || overlaps {
                return Err("source coverage range or sequence conflict".to_string());
            }
        }
    }
    for source in verified {
        if source.kind == SourceCoverageKind::Audio {
            audio_spans.push((source.start_ms, source.end_ms));
        }
    }
    if !audio_range_is_covered(&audio_spans, revision_start, revision_end) {
        return Err("transcript revision is not covered by finalized audio".to_string());
    }
    Ok(())
}

fn check_new_coverage_conflicts(
    storage: &Storage,
    scope: &MeetingScope,
    verified: &[VerifiedSourceCoverage],
) -> Result<(), String> {
    let existing = query_all(
        storage,
        "meeting_source_coverage",
        &[
            "id",
            "track_id",
            "source_generation",
            "sequence_no",
            "start_ms",
            "end_ms",
            "payload_hash",
        ],
        vec![eq(
            "meeting_source_coverage",
            "source_session_id",
            json!(&scope.source_session_id),
        )],
    )?;
    for row in existing {
        if string(&row, "meeting_source_coverage.track_id")? != scope.track_id
            || integer(&row, "meeting_source_coverage.source_generation")?
                != scope.source_generation
        {
            continue;
        }
        for source in verified {
            if string(&row, "meeting_source_coverage.id")? == source.id {
                if string(&row, "meeting_source_coverage.payload_hash")?
                    != canonical_sha256(&coverage_payload(source))?
                {
                    return Err("source coverage identity conflict".to_string());
                }
                return Err(
                    "SOURCE_CURSOR_CONFLICT: committed coverage is already durable".to_string(),
                );
            }
            let same_sequence =
                integer(&row, "meeting_source_coverage.sequence_no")? == source.sequence_no;
            let overlaps = integer(&row, "meeting_source_coverage.start_ms")? < source.end_ms
                && source.start_ms < integer(&row, "meeting_source_coverage.end_ms")?;
            if same_sequence || overlaps {
                return Err("source coverage range or sequence conflict".to_string());
            }
        }
    }
    Ok(())
}

fn load_committed_coverage(
    storage: &Storage,
    scope: &MeetingScope,
    coverage_id: &str,
) -> Result<VerifiedSourceCoverage, String> {
    let row = query(
        storage,
        "meeting_source_coverage",
        &[
            "project_id",
            "recording_id",
            "meeting_session_id",
            "source_session_id",
            "track_id",
            "source_generation",
            "sequence_no",
            "start_ms",
            "end_ms",
            "coverage_kind",
            "audio_chunk_id",
            "file_path",
            "byte_size",
            "checksum",
            "gap_reason",
            "payload_hash",
        ],
        vec![eq("meeting_source_coverage", "id", json!(coverage_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "revision references missing durable source coverage".to_string())?;
    if string(&row, "meeting_source_coverage.project_id")? != scope.project_id
        || string(&row, "meeting_source_coverage.recording_id")? != scope.recording_id
        || string(&row, "meeting_source_coverage.meeting_session_id")? != scope.meeting_session_id
        || string(&row, "meeting_source_coverage.source_session_id")? != scope.source_session_id
        || string(&row, "meeting_source_coverage.track_id")? != scope.track_id
        || integer(&row, "meeting_source_coverage.source_generation")? != scope.source_generation
    {
        return Err("revision coverage scope/generation mismatch".to_string());
    }
    let kind = match string(&row, "meeting_source_coverage.coverage_kind")?.as_str() {
        "audio" => SourceCoverageKind::Audio,
        "gap" => SourceCoverageKind::Gap,
        _ => return Err("stored source coverage kind is invalid".to_string()),
    };
    let input = SourceCoverageInput {
        id: coverage_id.to_string(),
        source_session_id: scope.source_session_id.clone(),
        track_id: scope.track_id.clone(),
        source_generation: scope.source_generation,
        sequence_no: integer(&row, "meeting_source_coverage.sequence_no")?,
        start_ms: integer(&row, "meeting_source_coverage.start_ms")?,
        end_ms: integer(&row, "meeting_source_coverage.end_ms")?,
        kind,
        audio_chunk_id: row
            .get("meeting_source_coverage.audio_chunk_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        file_path: row
            .get("meeting_source_coverage.file_path")
            .and_then(Value::as_str)
            .map(str::to_string),
        byte_size: row
            .get("meeting_source_coverage.byte_size")
            .and_then(Value::as_i64),
        checksum: row
            .get("meeting_source_coverage.checksum")
            .and_then(Value::as_str)
            .map(str::to_string),
        gap_reason: row
            .get("meeting_source_coverage.gap_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    let verified = verify_audio_coverage(storage, scope, &input)?;
    if string(&row, "meeting_source_coverage.payload_hash")?
        != canonical_sha256(&coverage_payload(&verified))?
    {
        return Err("durable source coverage integrity check failed".to_string());
    }
    Ok(verified)
}

fn check_revision_conflicts(
    storage: &Storage,
    request: &AtomicMeetingRequest,
) -> Result<(), String> {
    let rows = query_all(
        storage,
        "transcript_revisions",
        &["id", "revision", "origin", "review_state", "payload_hash"],
        vec![
            eq(
                "transcript_revisions",
                "meeting_session_id",
                json!(&request.scope.meeting_session_id),
            ),
            eq(
                "transcript_revisions",
                "utterance_id",
                json!(&request.revision.utterance_id),
            ),
        ],
    )?;
    if rows.iter().any(|row| {
        row.get("transcript_revisions.id").and_then(Value::as_str)
            == Some(request.revision.id.as_str())
    }) {
        return Err(
            "transcript revision identity already exists; reconcile exact attempt".to_string(),
        );
    }
    let current = rows
        .iter()
        .filter_map(|row| {
            row.get("transcript_revisions.revision")
                .and_then(Value::as_i64)
        })
        .max()
        .unwrap_or(0);
    if request.revision.revision != current + 1 {
        return Err("REVISION_CONFLICT: revision is not the next monotonic revision".to_string());
    }
    if request.revision.expected_revision != Some(current) && request.revision.revision > 1 {
        return Err("REVISION_CONFLICT: expectedRevision is stale or missing".to_string());
    }
    if request.revision.origin.is_manual() && request.revision.expected_revision != Some(current) {
        return Err("REVISION_CONFLICT: manual correction requires expectedRevision".to_string());
    }
    let current_review_state = rows
        .iter()
        .filter(|row| {
            row.get("transcript_revisions.revision")
                .and_then(Value::as_i64)
                == Some(current)
        })
        .filter_map(|row| {
            row.get("transcript_revisions.review_state")
                .and_then(Value::as_str)
        })
        .next();
    if current_review_state == Some("reviewed") && !request.revision.origin.is_manual() {
        return Err(
            "REVISION_CONFLICT: late ASR/refinement cannot overwrite manual correction".to_string(),
        );
    }
    if request.revision.origin.is_manual() && request.revision.review_state != "reviewed" {
        return Err("manual correction must create a reviewed revision".to_string());
    }
    if !request.revision.origin.is_manual() && request.revision.review_state != "unreviewed" {
        return Err("ASR revisions must remain unreviewed".to_string());
    }
    if request.revision.supersedes_revision != (current > 0).then_some(current) {
        return Err("revision supersedes_revision does not match the current revision".to_string());
    }
    Ok(())
}

fn check_knowledge_scope(storage: &Storage, request: &AtomicMeetingRequest) -> Result<(), String> {
    let Some(knowledge) = &request.knowledge else {
        return Ok(());
    };
    let collection = query(
        storage,
        "knowledge_collections",
        &["project_id", "status"],
        vec![eq(
            "knowledge_collections",
            "id",
            json!(&knowledge.collection_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "knowledge evidence references a missing collection".to_string())?;
    if collection
        .get("knowledge_collections.project_id")
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
        .is_some_and(|project_id| project_id != request.scope.project_id)
        || string(&collection, "knowledge_collections.status")? != "active"
    {
        return Err("knowledge collection scope is not currently readable".to_string());
    }
    let document = query(
        storage,
        "knowledge_documents",
        &["collection_id", "status"],
        vec![eq(
            "knowledge_documents",
            "id",
            json!(&knowledge.document_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "knowledge evidence references a missing document".to_string())?;
    if string(&document, "knowledge_documents.collection_id")? != knowledge.collection_id
        || string(&document, "knowledge_documents.status")? != "active"
    {
        return Err("knowledge document scope is not currently readable".to_string());
    }
    let version = query(
        storage,
        "knowledge_document_versions",
        &["document_id", "version_label", "state"],
        vec![eq(
            "knowledge_document_versions",
            "id",
            json!(&knowledge.document_version_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "knowledge evidence references a missing document version".to_string())?;
    if string(&version, "knowledge_document_versions.document_id")? != knowledge.document_id
        || string(&version, "knowledge_document_versions.version_label")?
            != knowledge.source_version
        || string(&version, "knowledge_document_versions.state")? != "active"
    {
        return Err("knowledge document version is not currently readable".to_string());
    }
    if let Some(bundle_id) = &knowledge.evidence_bundle_id {
        let bundle = query(
            storage,
            "knowledge_evidence_bundles",
            &["project_id", "meeting_session_id", "state", "share_state"],
            vec![eq("knowledge_evidence_bundles", "id", json!(bundle_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "knowledge evidence references a missing evidence bundle".to_string())?;
        if string(&bundle, "knowledge_evidence_bundles.project_id")? != request.scope.project_id
            || bundle
                .get("knowledge_evidence_bundles.meeting_session_id")
                .filter(|value| !value.is_null())
                .and_then(Value::as_str)
                .is_some_and(|session_id| session_id != request.scope.meeting_session_id)
            || string(&bundle, "knowledge_evidence_bundles.state")? == "revoked"
            || string(&bundle, "knowledge_evidence_bundles.share_state")? == "denied"
        {
            return Err(
                "knowledge evidence bundle is outside the current read/share scope".to_string(),
            );
        }
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TrustedIdentityContext {
    pub(crate) owner_principal_ref: String,
    pub(crate) account_ref: Option<String>,
    pub(crate) vault_id: String,
}

const LOCAL_OWNER_SESSION_ACTIVE: u8 = 0;
const LOCAL_OWNER_SESSION_LOCKED: u8 = 1;
const LOCAL_OWNER_SESSION_REVOKED: u8 = 2;
static NEXT_LOCAL_OWNER_SESSION_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct LocalOwnerAuthorityKey {
    data_root: PathBuf,
    vault_id: String,
    owner_principal_ref: String,
}

#[derive(Clone, Copy)]
struct LocalOwnerAuthorityState {
    generation: u64,
    state: u8,
}

struct LocalOwnerVaultAuthority {
    data_root: PathBuf,
    vault_id: String,
    owner_principal_ref: String,
    fence: RwLock<()>,
    state: Mutex<LocalOwnerAuthorityState>,
}

const MAX_LOCAL_OWNER_AUTHORITIES: usize = 128;
static LOCAL_OWNER_AUTHORITIES: OnceLock<
    Mutex<HashMap<LocalOwnerAuthorityKey, Weak<LocalOwnerVaultAuthority>>>,
> = OnceLock::new();

fn canonical_data_root(storage: &Storage) -> Result<PathBuf, String> {
    std::fs::canonicalize(&storage.path)
        .map_err(|_| "native local-owner data root is unavailable".to_string())
}

fn canonical_native_identity_root(storage: &Storage) -> Result<PathBuf, String> {
    let path = storage
        .path
        .parent()
        .ok_or_else(|| "native local-owner identity root is unavailable".to_string())?;
    std::fs::canonicalize(path)
        .map_err(|_| "native local-owner identity root is unavailable".to_string())
}

fn local_owner_authority(
    data_root: &Path,
    context: &TrustedIdentityContext,
) -> Result<Arc<LocalOwnerVaultAuthority>, String> {
    let key = LocalOwnerAuthorityKey {
        data_root: data_root.to_path_buf(),
        vault_id: context.vault_id.clone(),
        owner_principal_ref: context.owner_principal_ref.clone(),
    };
    let registry = LOCAL_OWNER_AUTHORITIES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = registry
        .lock()
        .map_err(|_| "native local-owner authority is unavailable".to_string())?;
    registry.retain(|_, authority| authority.strong_count() != 0);
    if let Some(authority) = registry.get(&key).and_then(Weak::upgrade) {
        return Ok(authority);
    }
    if registry.len() >= MAX_LOCAL_OWNER_AUTHORITIES {
        return Err("native local-owner authority capacity exhausted".to_string());
    }
    let authority = Arc::new(LocalOwnerVaultAuthority {
        data_root: key.data_root.clone(),
        vault_id: key.vault_id.clone(),
        owner_principal_ref: key.owner_principal_ref.clone(),
        fence: RwLock::new(()),
        state: Mutex::new(LocalOwnerAuthorityState {
            generation: 0,
            state: LOCAL_OWNER_SESSION_LOCKED,
        }),
    });
    registry.insert(key, Arc::downgrade(&authority));
    Ok(authority)
}

impl LocalOwnerVaultAuthority {
    fn activate(&self) -> Result<u64, String> {
        let _fence = self
            .fence
            .write()
            .map_err(|_| "native local-owner authority is unavailable".to_string())?;
        let generation = NEXT_LOCAL_OWNER_SESSION_GENERATION
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "native local-owner authority is unavailable".to_string())?;
        state.generation = generation;
        state.state = LOCAL_OWNER_SESSION_ACTIVE;
        Ok(generation)
    }

    fn invalidate(&self, state_value: u8) {
        let Ok(_fence) = self.fence.write() else {
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.generation = NEXT_LOCAL_OWNER_SESSION_GENERATION
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        state.state = state_value;
    }

    fn ensure_active(&self, session_generation: u64) -> Result<(), String> {
        let _fence = self
            .fence
            .read()
            .map_err(|_| "native local-owner authority is unavailable".to_string())?;
        self.ensure_active_under_fence(session_generation)
    }

    fn ensure_active_under_fence(&self, session_generation: u64) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "native local-owner authority is unavailable".to_string())?;
        if state.generation != session_generation {
            return Err("native local-owner unlock session is stale".to_string());
        }
        match state.state {
            LOCAL_OWNER_SESSION_ACTIVE => Ok(()),
            LOCAL_OWNER_SESSION_LOCKED => {
                Err("native local-owner unlock session is locked".to_string())
            }
            LOCAL_OWNER_SESSION_REVOKED => {
                Err("native local-owner unlock session is revoked".to_string())
            }
            _ => Err("native local-owner unlock session is invalid".to_string()),
        }
    }
}

pub(crate) trait LocalOwnerIdentitySource {
    fn owner_principal_ref(&self, data_root: &Path) -> Result<String, String>;
}

struct NativeDeviceOwnerIdentitySource;

impl LocalOwnerIdentitySource for NativeDeviceOwnerIdentitySource {
    fn owner_principal_ref(&self, data_root: &Path) -> Result<String, String> {
        let (_, fingerprint) = crate::device_identity::authorization_identity_in_dir(data_root)
            .map_err(|_| "native local-owner identity is unavailable".to_string())?;
        Ok(format!("principal:device:{fingerprint}"))
    }
}

trait LifecycleWitnessSource {
    fn read(&self) -> Result<crate::auth_session::LifecycleWitness, String>;
}

struct NativeLifecycleWitnessSource;

impl LifecycleWitnessSource for NativeLifecycleWitnessSource {
    fn read(&self) -> Result<crate::auth_session::LifecycleWitness, String> {
        crate::auth_session::read_lifecycle_witness()
    }
}

#[derive(Clone)]
pub(crate) struct NativeOwnerUnlockSession {
    context: TrustedIdentityContext,
    account_witness: crate::auth_session::LifecycleWitness,
    session_generation: u64,
    data_root: PathBuf,
    authority: Arc<LocalOwnerVaultAuthority>,
}

struct NativeOwnerOperationFence<'a> {
    _read_guard: RwLockReadGuard<'a, ()>,
}

pub(crate) struct NativeOwnerPublicationFence<'a> {
    _operation_fence: NativeOwnerOperationFence<'a>,
}

impl NativeOwnerUnlockSession {
    pub(crate) fn lock(&self) {
        self.authority.invalidate(LOCAL_OWNER_SESSION_LOCKED);
    }

    pub(crate) fn revoke(&self) {
        self.authority.invalidate(LOCAL_OWNER_SESSION_REVOKED);
    }

    fn ensure_active(&self) -> Result<(), String> {
        if self.session_generation == 0 {
            return Err("native local-owner unlock session is invalid".to_string());
        }
        self.authority.ensure_active(self.session_generation)
    }

    fn begin_operation_fence(&self) -> Result<NativeOwnerOperationFence<'_>, String> {
        let read_guard = self
            .authority
            .fence
            .read()
            .map_err(|_| "native local-owner authority is unavailable".to_string())?;
        self.authority
            .ensure_active_under_fence(self.session_generation)?;
        Ok(NativeOwnerOperationFence {
            _read_guard: read_guard,
        })
    }

    pub(crate) fn fence_sensitive_publication<'a>(
        &'a self,
        storage: &Storage,
        expected: &NativeOwnerUnlockSession,
    ) -> Result<NativeOwnerPublicationFence<'a>, String> {
        if self.session_generation != expected.session_generation
            || !Arc::ptr_eq(&self.authority, &expected.authority)
        {
            return Err("LOCAL_OWNER_LOCKED".to_string());
        }
        let operation_fence = self.begin_operation_fence()?;
        revalidate_native_local_owner_session_under_fence(
            storage,
            self,
            &NativeLifecycleWitnessSource,
        )?;
        Ok(NativeOwnerPublicationFence {
            _operation_fence: operation_fence,
        })
    }

    fn context(&self) -> &TrustedIdentityContext {
        &self.context
    }

    pub(crate) fn owner_scope(&self) -> String {
        local_owner_collection_scope(&self.context.vault_id)
    }

    pub(crate) fn account_lifecycle_witness(&self) -> &crate::auth_session::LifecycleWitness {
        &self.account_witness
    }

    pub(crate) fn backup_identity(&self) -> Result<(String, String), String> {
        let binding = native_owner_backup_binding(
            &self.context.owner_principal_ref,
            self.context.account_ref.as_deref(),
        )?;
        Ok((self.context.vault_id.clone(), binding))
    }

    fn account_witness_matches(&self, current: &crate::auth_session::LifecycleWitness) -> bool {
        &self.account_witness == current
    }
}

fn native_owner_backup_binding(
    owner_principal_ref: &str,
    account_ref: Option<&str>,
) -> Result<String, String> {
    let identity = serde_json::to_vec(&(owner_principal_ref, account_ref))
        .map_err(|_| "PRIVATE_BACKUP_OWNER_BINDING_INVALID".to_string())?;
    Ok(format!(
        "binding:{}",
        crate::backup_payload::sha256_hex(&identity)
    ))
}

pub(crate) fn unlock_native_local_owner(
    storage: &Storage,
) -> Result<NativeOwnerUnlockSession, String> {
    unlock_native_local_owner_selected(storage, None)
}

pub(crate) fn unlock_native_local_owner_selected(
    storage: &Storage,
    selected_vault_id: Option<&str>,
) -> Result<NativeOwnerUnlockSession, String> {
    let identity_source = NativeDeviceOwnerIdentitySource;
    let lifecycle_source = NativeLifecycleWitnessSource;
    unlock_native_local_owner_with_sources_and_selection(
        storage,
        &identity_source,
        &lifecycle_source,
        selected_vault_id,
    )
}

fn unlock_native_local_owner_with_sources(
    storage: &Storage,
    identity_source: &dyn LocalOwnerIdentitySource,
    lifecycle_source: &dyn LifecycleWitnessSource,
) -> Result<NativeOwnerUnlockSession, String> {
    unlock_native_local_owner_with_sources_and_selection(
        storage,
        identity_source,
        lifecycle_source,
        None,
    )
}

fn unlock_native_local_owner_with_sources_and_selection(
    storage: &Storage,
    identity_source: &dyn LocalOwnerIdentitySource,
    lifecycle_source: &dyn LifecycleWitnessSource,
    selected_vault_id: Option<&str>,
) -> Result<NativeOwnerUnlockSession, String> {
    let witness = lifecycle_source.read()?;
    let data_root = canonical_data_root(storage)?;
    let native_identity_root = canonical_native_identity_root(storage)?;
    let owner_principal_ref = identity_source.owner_principal_ref(&native_identity_root)?;
    if !owner_principal_ref.starts_with("principal:device:") {
        return Err("native local-owner identity is invalid".to_string());
    }
    let context = resolve_native_identity_context(
        storage,
        &owner_principal_ref,
        witness.native_user_id.as_deref(),
        selected_vault_id,
    )?;
    if let Some(bound_account_ref) = context.account_ref.as_deref() {
        if witness.native_user_id.as_deref() != Some(bound_account_ref)
            || witness.state != "authenticated"
        {
            return Err("bound native account is not authenticated".to_string());
        }
    }
    let authority = local_owner_authority(&data_root, &context)?;
    let session_generation = authority.activate()?;
    Ok(NativeOwnerUnlockSession {
        context,
        account_witness: witness,
        session_generation,
        data_root,
        authority,
    })
}

fn ensure_session_data_root(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
) -> Result<(), String> {
    let current_data_root = canonical_data_root(storage)?;
    if current_data_root != session.data_root
        || session.authority.data_root != session.data_root
        || session.authority.vault_id != session.context.vault_id
        || session.authority.owner_principal_ref != session.context.owner_principal_ref
    {
        return Err("native local-owner unlock is bound to a different data root".to_string());
    }
    Ok(())
}

fn revalidate_native_local_owner_session(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    lifecycle_source: &dyn LifecycleWitnessSource,
) -> Result<(), String> {
    ensure_session_data_root(storage, session)?;
    let _operation_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(storage, session, lifecycle_source)
}

fn revalidate_native_local_owner_session_under_fence(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    lifecycle_source: &dyn LifecycleWitnessSource,
) -> Result<(), String> {
    ensure_session_data_root(storage, session)?;
    session
        .authority
        .ensure_active_under_fence(session.session_generation)?;
    let current_witness = lifecycle_source.read()?;
    if !session.account_witness_matches(&current_witness) {
        return Err("native account lifecycle witness was invalidated".to_string());
    }
    let vault = query(
        storage,
        "identity_vaults",
        &["owner_principal_ref", "bound_account_ref", "state"],
        vec![eq(
            "identity_vaults",
            "id",
            json!(&session.context.vault_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "native local-owner vault is missing".to_string())?;
    let bound_account_ref =
        optional_row_value(&vault, "identity_vaults.bound_account_ref").and_then(Value::as_str);
    if string(&vault, "identity_vaults.owner_principal_ref")? != session.context.owner_principal_ref
        || bound_account_ref != session.context.account_ref.as_deref()
        || string(&vault, "identity_vaults.state")? != "active"
    {
        return Err(
            "native local-owner vault is locked, revoked, or outside the trusted owner".to_string(),
        );
    }
    if let Some(bound_account_ref) = bound_account_ref {
        if current_witness.native_user_id.as_deref() != Some(bound_account_ref)
            || current_witness.state != "authenticated"
        {
            return Err("bound native account is not authenticated".to_string());
        }
    }
    Ok(())
}

struct NativeIdentityCapture {
    context: TrustedIdentityContext,
    account_guard: Option<crate::auth_session::AccountOperationGuard>,
}

fn resolve_native_identity_context(
    storage: &Storage,
    owner_principal_ref: &str,
    active_account_ref: Option<&str>,
    selected_vault_id: Option<&str>,
) -> Result<TrustedIdentityContext, String> {
    let vaults = query_all(
        storage,
        "identity_vaults",
        &["id", "owner_principal_ref", "bound_account_ref", "state"],
        vec![eq(
            "identity_vaults",
            "owner_principal_ref",
            json!(owner_principal_ref),
        )],
    )?;
    let active = vaults
        .into_iter()
        .filter(|row| {
            if selected_vault_id.is_some_and(|selected| {
                row.get("identity_vaults.id").and_then(Value::as_str) != Some(selected)
            }) {
                return false;
            }
            if row.get("identity_vaults.state").and_then(Value::as_str) != Some("active") {
                return false;
            }
            let bound_account = row
                .get("identity_vaults.bound_account_ref")
                .and_then(Value::as_str);
            bound_account.is_none() || active_account_ref == bound_account
        })
        .collect::<Vec<_>>();
    if active.len() != 1 {
        return Err(
            "native local-owner vault is missing, locked, revoked, or ambiguous".to_string(),
        );
    }
    let bound_account_ref = active[0]
        .get("identity_vaults.bound_account_ref")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Ok(TrustedIdentityContext {
        owner_principal_ref: owner_principal_ref.to_string(),
        account_ref: bound_account_ref,
        vault_id: string(&active[0], "identity_vaults.id")?,
    })
}

pub(crate) fn native_local_owner_vault_options(storage: &Storage) -> Result<Vec<Value>, String> {
    let identity_source = NativeDeviceOwnerIdentitySource;
    let lifecycle_source = NativeLifecycleWitnessSource;
    let identity_root = canonical_native_identity_root(storage)?;
    let owner_principal_ref = identity_source.owner_principal_ref(&identity_root)?;
    let witness = lifecycle_source.read()?;
    let rows = query_all(
        storage,
        "identity_vaults",
        &["id", "bound_account_ref", "state"],
        vec![eq(
            "identity_vaults",
            "owner_principal_ref",
            json!(&owner_principal_ref),
        )],
    )?;
    rows.into_iter()
        .filter(|row| {
            row.get("identity_vaults.state").and_then(Value::as_str) == Some("active")
                && row
                    .get("identity_vaults.bound_account_ref")
                    .and_then(Value::as_str)
                    .is_none_or(|bound| witness.native_user_id.as_deref() == Some(bound))
        })
        .map(|row| {
            let vault_id = string(&row, "identity_vaults.id")?;
            let key_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(format!(
                "knowledge:{vault_id}"
            ))?;
            let key_available = match crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)
            {
                Ok(key) => {
                    drop(key);
                    true
                }
                Err(error) if error == "KEY_UNAVAILABLE" => false,
                Err(error) => return Err(error),
            };
            Ok(json!({
                "vaultId": vault_id,
                "keyAvailable": key_available,
                "accountBound": row.get("identity_vaults.bound_account_ref").and_then(Value::as_str).is_some(),
            }))
        })
        .collect()
}

fn commit_owner_vault_state(
    storage: &Storage,
    transaction_id: &str,
    expected_frontier: u64,
    row: Value,
) -> Result<(), String> {
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: transaction_id.to_string(),
            expected_frontier: Some(expected_frontier),
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations: vec![upsert("identity_vaults", row)],
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn commit_native_owner_mutations(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    attempt: &MeetingCommitAttempt,
    mutations: Vec<RelationalRowMutation>,
) -> Result<u64, String> {
    let native_capture = capture_native_identity_context(storage, session)?;
    let lifecycle_source = NativeLifecycleWitnessSource;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(storage, session, &lifecycle_source)?;
    let mut transaction = Some(GenesisTransaction {
        transaction_id: attempt.transaction_id.clone(),
        expected_frontier: Some(attempt.expected_frontier),
        relational: vec![RelationalMutationGroup {
            namespace: NAMESPACE.to_string(),
            mutations,
        }],
        graph: BatchInput {
            nodes: vec![],
            edges: vec![],
        },
        vectors: vec![],
    });
    let mut commit_result = None;
    let mut commit_operation = || -> Result<(), String> {
        revalidate_native_local_owner_session_under_fence(storage, session, &lifecycle_source)?;
        if let Some(account_guard) = native_capture.account_guard.as_ref() {
            account_guard
                .check()
                .map_err(|_| "native account operation was invalidated".to_string())?;
        }
        match storage
            .commit_transaction(
                transaction
                    .take()
                    .ok_or_else(|| "native owner commit was invoked more than once".to_string())?,
            )
            .map_err(meeting_commit_error)
        {
            Ok(commit) => {
                commit_result = Some(Ok(commit.commit_sequence));
                Ok(())
            }
            Err(error) => {
                commit_result = Some(Err(error.clone()));
                Err(error)
            }
        }
    };
    let fence_result = if let Some(account_guard) = native_capture.account_guard.as_ref() {
        account_guard.with_account_lifecycle_fence(&session.account_witness, commit_operation)
    } else {
        commit_operation()
    };
    match fence_result {
        Ok(()) => {
            commit_result.ok_or_else(|| "native owner commit fence did not execute".to_string())?
        }
        Err(_) => match commit_result {
            Some(result) => result,
            None => Err("native account operation was invalidated".to_string()),
        },
    }
}

fn local_owner_collection_scope(vault_id: &str) -> String {
    format!("local-owner:{vault_id}")
}

fn meeting_session_for_recording(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Value, String> {
    let mut sessions = query_all(
        storage,
        "meeting_sessions",
        &[
            "id",
            "project_id",
            "recording_id",
            "session_generation",
            "state",
            "revision",
        ],
        vec![
            eq("meeting_sessions", "project_id", json!(project_id)),
            eq("meeting_sessions", "recording_id", json!(recording_id)),
        ],
    )?;
    sessions
        .sort_by_key(|row| integer(row, "meeting_sessions.session_generation").unwrap_or_default());
    sessions
        .pop()
        .ok_or_else(|| "MEETING_SESSION_UNAVAILABLE".to_string())
}

fn require_recording_project_scope(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<(), String> {
    let recordings = query(
        storage,
        "recordings",
        &["id"],
        vec![
            eq("recordings", "project_id", json!(project_id)),
            eq("recordings", "id", json!(recording_id)),
        ],
        1,
    )?;
    if recordings.is_empty() {
        return Err("MEETING_RECORDING_SCOPE_DENIED".to_string());
    }
    Ok(())
}

fn meeting_context_row(
    storage: &Storage,
    meeting_session_id: &str,
) -> Result<Option<Value>, String> {
    Ok(query(
        storage,
        "meeting_session_contexts",
        &[
            "id",
            "project_id",
            "recording_id",
            "meeting_session_id",
            "owner_scope",
            "vault_id",
            "account_ref",
            "selected_collection_ids_json",
            "private_context_asset_id",
            "temporal_context_json",
            "policy_json",
            "state",
            "revision",
            "created_at",
            "updated_at",
        ],
        vec![eq(
            "meeting_session_contexts",
            "meeting_session_id",
            json!(meeting_session_id),
        )],
        1,
    )?
    .into_iter()
    .next())
}

fn selected_collections_from_context(row: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let Some(value) = row.get("meeting_session_contexts.selected_collection_ids_json") else {
        return Ok(Vec::new());
    };
    serde_json::from_value(value.clone())
        .map_err(|_| "MEETING_KNOWLEDGE_SELECTION_INVALID".to_string())
}

fn validate_project_exists(storage: &Storage, project_id: &str) -> Result<(), String> {
    if project_id.is_empty() || project_id.len() > 128 {
        return Err("MEETING_PROJECT_INVALID".to_string());
    }
    let project = query(
        storage,
        "projects",
        &["id"],
        vec![eq("projects", "id", json!(project_id))],
        1,
    )?;
    if project.is_empty() {
        return Err("MEETING_PROJECT_UNAVAILABLE".to_string());
    }
    Ok(())
}

pub(crate) fn create_meeting_knowledge_collection(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingKnowledgeCollectionCreateRequest,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeCollectionCreateResult, String> {
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-knowledge-collection-create::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let classification = request.classification.as_str();
    if !matches!(classification, "internal" | "confidential") {
        return Err("MEETING_KNOWLEDGE_CLASSIFICATION_UNSUPPORTED".to_string());
    }
    if let Some(project_id) = request.project_id.as_deref() {
        validate_project_exists(storage, project_id)?;
    }
    let collection_id = Uuid::new_v4().to_string();
    let timestamp = attempt.committed_at.clone();
    let row = json!({
        "id": collection_id,
        "project_id": request.project_id,
        "owner_scope": local_owner_collection_scope(&trusted.vault_id),
        "classification": classification,
        "read_policy_ref": format!("local-read:{}", Uuid::new_v4()),
        "share_policy_ref": format!("local-share-deny:{}", Uuid::new_v4()),
        "revision": 1,
        "status": "active",
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": timestamp,
    });
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![upsert("knowledge_collections", row)],
    )?;
    Ok(meeting_intelligence_schema::MeetingKnowledgeCollectionCreateResult { collection_id })
}

pub(crate) fn list_meeting_knowledge_collections(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<meeting_intelligence_schema::MeetingKnowledgeCollectionSummary>, String> {
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    validate_project_exists(storage, project_id)?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let context = meeting_context_row(storage, &string(&meeting, "meeting_sessions.id")?)?;
    if let Some(context) = context.as_ref() {
        if string(context, "meeting_session_contexts.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
            || context
                .get("meeting_session_contexts.vault_id")
                .and_then(Value::as_str)
                != Some(trusted.vault_id.as_str())
        {
            return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
        }
    }
    let selected = selected_collections_from_context(context.as_ref())?
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let rows = query(
        storage,
        "knowledge_collections",
        &[
            "id",
            "project_id",
            "owner_scope",
            "classification",
            "revision",
            "status",
        ],
        vec![eq(
            "knowledge_collections",
            "owner_scope",
            json!(&owner_scope),
        )],
        256,
    )?;
    let mut output = Vec::new();
    for row in rows {
        let collection_id = string(&row, "knowledge_collections.id")?;
        let collection_project = row
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if collection_project.is_some_and(|bound| bound != project_id)
            || string(&row, "knowledge_collections.status")? != "active"
        {
            continue;
        }
        let key_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(format!(
            "knowledge:{}",
            trusted.vault_id
        ))?;
        let readable = crate::meeting_knowledge::OsKnowledgeKeyBackend
            .read(&key_ref)
            .map(|key| {
                drop(key);
                true
            })
            .unwrap_or(false);
        let document_count = query(
            storage,
            "knowledge_documents",
            &["id"],
            vec![
                eq(
                    "knowledge_documents",
                    "collection_id",
                    json!(&collection_id),
                ),
                eq("knowledge_documents", "status", json!("active")),
            ],
            1000,
        )?
        .len() as u32;
        let label_suffix = collection_id.chars().take(8).collect::<String>();
        output.push(
            meeting_intelligence_schema::MeetingKnowledgeCollectionSummary {
                collection_id: collection_id.clone(),
                label: format!("คลังในเครื่อง {label_suffix}"),
                readable,
                selected: selected.contains(&collection_id),
                classification: string(&row, "knowledge_collections.classification")?,
                revision: integer(&row, "knowledge_collections.revision")?,
                document_count,
            },
        );
    }
    Ok(output)
}

pub(crate) fn set_meeting_knowledge_selection(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingKnowledgeSelectionCommand,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeSelectionReceipt, String> {
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-knowledge-selection::{}", request.request_id),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    validate_project_exists(storage, &request.project_id)?;
    if request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.collection_ids.len() > 64
    {
        return Err("MEETING_KNOWLEDGE_SELECTION_INVALID".to_string());
    }
    let meeting =
        meeting_session_for_recording(storage, &request.project_id, &request.recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let existing = meeting_context_row(storage, &meeting_session_id)?;
    let current_revision = existing
        .as_ref()
        .map(|row| integer(row, "meeting_session_contexts.revision"))
        .transpose()?
        .unwrap_or(0);
    if current_revision != request.expected_revision {
        return Err("MEETING_KNOWLEDGE_SELECTION_REVISION_CONFLICT".to_string());
    }
    let unique = request
        .collection_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != request.collection_ids.len() {
        return Err("MEETING_KNOWLEDGE_SELECTION_DUPLICATE".to_string());
    }
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    for collection_id in &unique {
        let row = query(
            storage,
            "knowledge_collections",
            &["id", "project_id", "owner_scope", "status"],
            vec![eq("knowledge_collections", "id", json!(collection_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_KNOWLEDGE_COLLECTION_UNAVAILABLE".to_string())?;
        let collection_project = row
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&row, "knowledge_collections.owner_scope")? != owner_scope
            || string(&row, "knowledge_collections.status")? != "active"
            || collection_project.is_some_and(|bound| bound != request.project_id)
        {
            return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
        }
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    drop(key);
    let now = attempt.committed_at.clone();
    let row = json!({
        "id": meeting_session_id,
        "project_id": request.project_id,
        "recording_id": request.recording_id,
        "meeting_session_id": meeting_session_id,
        "owner_scope": owner_scope,
        "vault_id": trusted.vault_id,
        "account_ref": trusted.account_ref,
        "selected_collection_ids_json": serde_json::to_value(&request.collection_ids).map_err(|_| "MEETING_KNOWLEDGE_SELECTION_INVALID".to_string())?,
        "private_context_asset_id": existing.as_ref().and_then(|row| row.get("meeting_session_contexts.private_context_asset_id")).cloned().unwrap_or(Value::Null),
        "temporal_context_json": existing.as_ref().and_then(|row| row.get("meeting_session_contexts.temporal_context_json")).cloned().unwrap_or(json!({"meeting_date": null, "timezone": null})),
        "policy_json": existing.as_ref().and_then(|row| row.get("meeting_session_contexts.policy_json")).cloned().unwrap_or(json!({})),
        "state": "active",
        "revision": current_revision + 1,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": existing.as_ref().and_then(|row| row.get("meeting_session_contexts.created_at")).and_then(Value::as_str).unwrap_or(&now),
        "updated_at": now,
    });
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![upsert("meeting_session_contexts", row)],
    )?;
    Ok(
        meeting_intelligence_schema::MeetingKnowledgeSelectionReceipt {
            revision: current_revision + 1,
            selected_collection_ids: request.collection_ids.clone(),
        },
    )
}

pub(crate) fn meeting_knowledge_selection_state(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
) -> Result<(i64, Vec<String>), String> {
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let context = meeting_context_row(storage, &meeting_session_id)?;
    if let Some(context) = context.as_ref() {
        if string(context, "meeting_session_contexts.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
            || context
                .get("meeting_session_contexts.vault_id")
                .and_then(Value::as_str)
                != Some(trusted.vault_id.as_str())
        {
            return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
        }
    }
    let revision = context
        .as_ref()
        .map(|row| integer(row, "meeting_session_contexts.revision"))
        .transpose()?
        .unwrap_or(0);
    Ok((
        revision,
        selected_collections_from_context(context.as_ref())?,
    ))
}

fn knowledge_asset_path(data_root: &Path, asset_id: &str) -> Result<(PathBuf, String), String> {
    if asset_id.is_empty()
        || asset_id.len() > 128
        || !asset_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("KNOWLEDGE_ASSET_REFERENCE_INVALID".to_string());
    }
    let asset_root = data_root.join("meeting-assets");
    let knowledge_root = asset_root.join("knowledge");
    for directory in [&asset_root, &knowledge_root] {
        if let Ok(metadata) = std::fs::symlink_metadata(directory) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string());
            }
        }
        std::fs::create_dir_all(directory)
            .map_err(|_| "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    }
    let canonical_root = std::fs::canonicalize(&knowledge_root)
        .map_err(|_| "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    if !canonical_root.starts_with(data_root) {
        return Err("KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string());
    }
    let relative = format!("meeting-assets/knowledge/{asset_id}.enc");
    Ok((canonical_root.join(format!("{asset_id}.enc")), relative))
}

fn people_asset_path(data_root: &Path, asset_id: &str) -> Result<(PathBuf, String), String> {
    if Uuid::parse_str(asset_id).is_err() {
        return Err("PEOPLE_ASSET_REFERENCE_INVALID".to_string());
    }
    let asset_root = data_root.join("meeting-assets");
    let people_root = asset_root.join("people");
    for directory in [&asset_root, &people_root] {
        if let Ok(metadata) = std::fs::symlink_metadata(directory) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string());
            }
        }
        std::fs::create_dir_all(directory)
            .map_err(|_| "PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    }
    let canonical_root = std::fs::canonicalize(&people_root)
        .map_err(|_| "PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    if !canonical_root.starts_with(data_root) {
        return Err("PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string());
    }
    let filename = format!("{asset_id}.enc");
    Ok((
        canonical_root.join(&filename),
        format!("meeting-assets/people/{filename}"),
    ))
}

fn read_bounded_private_asset(
    path: &Path,
    max_bytes: u64,
    error_code: &str,
) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
    let mut file = File::open(path).map_err(|_| error_code.to_string())?;
    let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(64 * 1024));
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| error_code.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(error_code.to_string());
    }
    Ok(bytes)
}

fn write_people_profile_asset(
    data_root: &Path,
    asset_id: &str,
    reference: &PrivateIdentityReference,
) -> Result<(String, i64, Value), String> {
    use std::io::Write;

    let (path, relative) = people_asset_path(data_root, asset_id)?;
    let encoded = zeroize::Zeroizing::new(
        serde_json::to_vec(reference)
            .map_err(|_| "PEOPLE_ASSET_SERIALIZATION_FAILED".to_string())?,
    );
    let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|_| "PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    let result = file
        .write_all(&encoded)
        .and_then(|_| file.sync_all())
        .map_err(|_| "PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    if path.exists() {
        let _ = std::fs::remove_file(&temp_path);
        return Err("PEOPLE_ASSET_REFERENCE_COLLISION".to_string());
    }
    std::fs::rename(&temp_path, &path)
        .map_err(|_| "PEOPLE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    let ciphertext_bytes = i64::try_from(reference.envelope.ciphertext.len())
        .map_err(|_| "PEOPLE_ASSET_SIZE_INVALID".to_string())?;
    let retention = json!({
        "format_version": reference.envelope.version,
        "nonce": reference.envelope.nonce,
        "aad_sha256": reference.envelope.aad_sha256,
    });
    Ok((relative, ciphertext_bytes, retention))
}

fn write_encrypted_knowledge_asset(
    data_root: &Path,
    asset_id: &str,
    key: &[u8; 32],
    context: &crate::meeting_knowledge::PrivateAssetContext,
    plaintext: &[u8],
) -> Result<(String, String, i64, Value), String> {
    use std::io::Write;

    let (path, relative) = knowledge_asset_path(data_root, asset_id)?;
    if path.exists() {
        let bytes = read_bounded_private_asset(
            &path,
            crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
            "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE",
        )?;
        let payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
            serde_json::from_slice(bytes.as_slice())
                .map_err(|_| "KNOWLEDGE_ASSET_COLLISION".to_string())?;
        let open = crate::meeting_knowledge::open_private_payload(key, context, &payload)?;
        if open.as_slice() != plaintext {
            return Err("KNOWLEDGE_ASSET_COLLISION".to_string());
        }
        let length = i64::try_from(payload.ciphertext.len())
            .map_err(|_| "KNOWLEDGE_ASSET_SIZE_INVALID".to_string())?;
        let retention = json!({
            "format_version": payload.version,
            "nonce": payload.nonce,
            "aad_sha256": payload.aad_sha256,
        });
        return Ok((relative, payload.ciphertext_sha256, length, retention));
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", context.vault_id))?;
    let payload = crate::meeting_knowledge::seal_private_payload(key, key_ref, context, plaintext)?;
    let encoded = zeroize::Zeroizing::new(
        serde_json::to_vec(&payload)
            .map_err(|_| "KNOWLEDGE_ASSET_SERIALIZATION_FAILED".to_string())?,
    );
    let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|_| "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string())?;
    let write_result = file
        .write_all(&encoded)
        .and_then(|_| file.sync_all())
        .map_err(|_| "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE".to_string());
    drop(file);
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(rename_error) = std::fs::rename(&temp_path, &path) {
        let _ = std::fs::remove_file(&temp_path);
        if path.is_file() {
            let bytes = read_bounded_private_asset(
                &path,
                crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
                "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE",
            )?;
            let existing: crate::meeting_knowledge::EncryptedKnowledgePayload =
                serde_json::from_slice(bytes.as_slice())
                    .map_err(|_| "KNOWLEDGE_ASSET_COLLISION".to_string())?;
            let open = crate::meeting_knowledge::open_private_payload(key, context, &existing)?;
            if open.as_slice() == plaintext {
                let length = i64::try_from(existing.ciphertext.len())
                    .map_err(|_| "KNOWLEDGE_ASSET_SIZE_INVALID".to_string())?;
                let retention = json!({
                    "format_version": existing.version,
                    "nonce": existing.nonce,
                    "aad_sha256": existing.aad_sha256,
                });
                return Ok((relative, existing.ciphertext_sha256, length, retention));
            }
            return Err("KNOWLEDGE_ASSET_COLLISION".to_string());
        }
        return Err(format!(
            "KNOWLEDGE_ASSET_CUSTODY_UNAVAILABLE: {rename_error}"
        ));
    }
    let length = i64::try_from(payload.ciphertext.len())
        .map_err(|_| "KNOWLEDGE_ASSET_SIZE_INVALID".to_string())?;
    let retention = json!({
        "format_version": payload.version,
        "nonce": payload.nonce,
        "aad_sha256": payload.aad_sha256,
    });
    Ok((relative, payload.ciphertext_sha256, length, retention))
}

fn load_private_asset_key(
    vault_id: &str,
    key_ref: &str,
) -> Result<zeroize::Zeroizing<[u8; 32]>, String> {
    let knowledge_ref = format!("knowledge:{vault_id}");
    if key_ref == knowledge_ref {
        let typed_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(knowledge_ref)?;
        return crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&typed_ref);
    }
    let people_ref = people_metadata_key_ref(vault_id)?;
    if key_ref == people_ref {
        let backend = OsPeopleMetadataKeyBackend;
        let secret = zeroize::Zeroizing::new(
            backend
                .get_key(key_ref)?
                .ok_or_else(|| "KEY_UNAVAILABLE".to_string())?,
        );
        if secret.len() != 32 {
            return Err("KEY_UNAVAILABLE".to_string());
        }
        let mut key = zeroize::Zeroizing::new([0_u8; 32]);
        key.copy_from_slice(&secret);
        return Ok(key);
    }
    Err("PRIVATE_BACKUP_KEY_REF_INVALID".to_string())
}

pub(crate) fn export_private_meeting_assets_for_backup(
    storage: &Storage,
    session: Option<&NativeOwnerUnlockSession>,
    archive_id: &str,
    recovery_phrase: &str,
) -> Result<Option<crate::backup_payload::PrivateMeetingAssetsBundle>, String> {
    let rows = query_all(
        storage,
        "meeting_private_assets",
        &[
            "id",
            "project_id",
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "ciphertext_sha256",
            "ciphertext_bytes",
            "state",
        ],
        vec![],
    )?;
    let rows = rows
        .into_iter()
        .filter(|row| {
            row.get("meeting_private_assets.state")
                .and_then(Value::as_str)
                == Some("active")
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(None);
    }
    let session = session.ok_or_else(|| "LOCAL_OWNER_LOCKED_FOR_PRIVATE_BACKUP".to_string())?;
    let (vault_id, owner_binding) = session.backup_identity()?;
    let owner_scope = local_owner_collection_scope(&vault_id);
    if rows.iter().any(|row| {
        row.get("meeting_private_assets.vault_id")
            .and_then(Value::as_str)
            != Some(vault_id.as_str())
            || row
                .get("meeting_private_assets.owner_scope")
                .and_then(Value::as_str)
                != Some(owner_scope.as_str())
    }) {
        return Err("PRIVATE_BACKUP_CONTAINS_OTHER_OWNER_ASSETS".to_string());
    }
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    if trusted.vault_id != vault_id {
        return Err("PRIVATE_BACKUP_VAULT_SCOPE_MISMATCH".to_string());
    }
    let mut keys = BTreeMap::<String, zeroize::Zeroizing<[u8; 32]>>::new();
    let mut assets = Vec::with_capacity(rows.len());
    for row in rows {
        let asset_id = string(&row, "meeting_private_assets.id")?;
        let entity_id = string(&row, "meeting_private_assets.entity_id")?;
        let purpose_name = string(&row, "meeting_private_assets.purpose")?;
        let key_ref = string(&row, "meeting_private_assets.key_ref")?;
        let (backup_purpose, decrypt_purpose) = match purpose_name.as_str() {
            "knowledge_document" => (
                crate::backup_payload::PrivateMeetingAssetPurpose::KnowledgeDocument,
                Some(crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument),
            ),
            "agent_draft" => (
                crate::backup_payload::PrivateMeetingAssetPurpose::AgentDraft,
                Some(crate::meeting_knowledge::PrivateAssetPurpose::AgentDraft),
            ),
            "delivery_preview" => (
                crate::backup_payload::PrivateMeetingAssetPurpose::DeliveryPreview,
                Some(crate::meeting_knowledge::PrivateAssetPurpose::DeliveryPreview),
            ),
            "people_profile" => (
                crate::backup_payload::PrivateMeetingAssetPurpose::PeopleProfile,
                None,
            ),
            _ => return Err("PRIVATE_BACKUP_ASSET_PURPOSE_UNSUPPORTED".to_string()),
        };
        if string(&row, "meeting_private_assets.owner_scope")? != owner_scope {
            return Err("PRIVATE_BACKUP_ASSET_OWNER_MISMATCH".to_string());
        }
        let expected_key_ref = match purpose_name.as_str() {
            "people_profile" => people_metadata_key_ref(&vault_id)?,
            _ => format!("knowledge:{vault_id}"),
        };
        if key_ref != expected_key_ref {
            return Err("PRIVATE_BACKUP_ASSET_KEY_REF_MISMATCH".to_string());
        }
        if !keys.contains_key(&key_ref) {
            keys.insert(
                key_ref.clone(),
                load_private_asset_key(&vault_id, &key_ref)?,
            );
        }
        let key = keys
            .get(&key_ref)
            .ok_or_else(|| "PRIVATE_BACKUP_KEY_UNAVAILABLE".to_string())?;
        let (path, expected_custody_ref) = if purpose_name == "people_profile" {
            people_asset_path(&session.data_root, &asset_id)?
        } else {
            knowledge_asset_path(&session.data_root, &asset_id)?
        };
        if string(&row, "meeting_private_assets.custody_ref")? != expected_custody_ref {
            return Err("PRIVATE_BACKUP_ASSET_PATH_INVALID".to_string());
        }
        let file_size = std::fs::metadata(&path)
            .map_err(|_| "PRIVATE_BACKUP_ASSET_UNAVAILABLE".to_string())?
            .len();
        if file_size > crate::backup_payload::MAX_PRIVATE_ASSET_BYTES {
            return Err("PRIVATE_BACKUP_ASSET_TOO_LARGE".to_string());
        }
        let encoded = read_bounded_private_asset(
            &path,
            crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
            "PRIVATE_BACKUP_ASSET_UNAVAILABLE",
        )?;
        let version = if let Some(decrypt_purpose) = decrypt_purpose {
            let payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
                serde_json::from_slice(&encoded)
                    .map_err(|_| "PRIVATE_BACKUP_ASSET_MALFORMED".to_string())?;
            if payload.key_ref.as_str() != key_ref
                || payload.ciphertext_sha256
                    != string(&row, "meeting_private_assets.ciphertext_sha256")?
                || crate::backup_payload::sha256_hex(&payload.ciphertext)
                    != payload.ciphertext_sha256
                || i64::try_from(payload.ciphertext.len()).ok()
                    != Some(integer(&row, "meeting_private_assets.ciphertext_bytes")?)
            {
                return Err("PRIVATE_BACKUP_ASSET_DIGEST_MISMATCH".to_string());
            }
            let context = crate::meeting_knowledge::PrivateAssetContext {
                vault_id: vault_id.clone(),
                entity_id: entity_id.clone(),
                version: u64::from(payload.version),
                purpose: decrypt_purpose,
            };
            let verified = crate::meeting_knowledge::open_private_payload(key, &context, &payload)?;
            drop(verified);
            payload.version
        } else {
            let profile = find_people_profile(storage, &trusted, &entity_id)?
                .ok_or_else(|| "PEOPLE_PROFILE_NOT_FOUND".to_string())?;
            let payload = read_people_profile_payload(
                storage,
                &trusted,
                &profile,
                &OsPeopleMetadataKeyBackend,
            )?;
            let revision = integer(&row, "meeting_private_assets.entity_revision")?;
            if integer(&profile, "participant_profiles.revision")? != revision
                || string(
                    &profile,
                    "participant_profiles.profile_payload_ciphertext_ref",
                )? != asset_id
                || string(&profile, "participant_profiles.profile_ciphertext_sha256")?
                    != string(&row, "meeting_private_assets.ciphertext_sha256")?
            {
                return Err("PRIVATE_BACKUP_PEOPLE_PROFILE_MISMATCH".to_string());
            }
            drop(payload);
            1
        };
        let ciphertext_sha256 = crate::backup_payload::sha256_hex(&encoded);
        assets.push(crate::backup_payload::EncryptedMeetingAsset {
            asset_ref: asset_id,
            entity_id,
            purpose: backup_purpose,
            version: u32::from(version),
            vault_id: vault_id.clone(),
            account_binding: owner_binding.clone(),
            ciphertext_sha256,
            encrypted_bytes: encoded.to_vec(),
        });
    }
    let mut recovery_packages = Vec::with_capacity(keys.len());
    for (key_ref, key) in keys {
        let encrypted_key = crate::backup_archive::encrypt_native_recovery_package(
            recovery_phrase,
            archive_id,
            &key_ref,
            &vault_id,
            &owner_binding,
            key.as_ref(),
        )
        .map_err(|_| "PRIVATE_BACKUP_KEY_WRAPPING_FAILED".to_string())?;
        recovery_packages.push(crate::backup_payload::EncryptedNativeRecoveryPackage {
            package_ref: key_ref,
            version: 1,
            vault_id: vault_id.clone(),
            account_binding: owner_binding.clone(),
            ciphertext_sha256: crate::backup_payload::sha256_hex(&encrypted_key),
            encrypted_bytes: encrypted_key,
        });
    }
    let mut packages = recovery_packages.into_iter();
    let recovery_package = packages
        .next()
        .ok_or_else(|| "PRIVATE_BACKUP_KEY_UNAVAILABLE".to_string())?;
    Ok(Some(crate::backup_payload::PrivateMeetingAssetsBundle {
        contract_version: 1,
        vault_id,
        account_binding: owner_binding,
        assets,
        recovery_package,
        additional_recovery_packages: packages.collect(),
    }))
}

pub(crate) fn validate_restored_private_meeting_assets(
    storage: &Storage,
    bundle: &crate::backup_payload::PrivateMeetingAssetsBundle,
) -> Result<(), String> {
    let vault = query(
        storage,
        "identity_vaults",
        &["owner_principal_ref", "bound_account_ref", "state"],
        vec![eq("identity_vaults", "id", json!(&bundle.vault_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "PRIVATE_BACKUP_VAULT_MISSING".to_string())?;
    if string(&vault, "identity_vaults.state")? != "active" {
        return Err("PRIVATE_BACKUP_VAULT_NOT_ACTIVE".to_string());
    }
    let owner_principal_ref = string(&vault, "identity_vaults.owner_principal_ref")?;
    let account_ref = vault
        .get("identity_vaults.bound_account_ref")
        .and_then(Value::as_str);
    if native_owner_backup_binding(&owner_principal_ref, account_ref)? != bundle.account_binding {
        return Err("PRIVATE_BACKUP_OWNER_BINDING_MISMATCH".to_string());
    }
    let expected_scope = local_owner_collection_scope(&bundle.vault_id);
    let people_key_ref = people_metadata_key_ref(&bundle.vault_id)?;
    let knowledge_key_ref = format!("knowledge:{}", bundle.vault_id);
    let mut package_refs = std::collections::BTreeSet::new();
    for package in
        std::iter::once(&bundle.recovery_package).chain(bundle.additional_recovery_packages.iter())
    {
        if !matches!(package.package_ref.as_str(), ref_value if ref_value == knowledge_key_ref || ref_value == people_key_ref)
            || package.vault_id != bundle.vault_id
            || package.account_binding != bundle.account_binding
            || package.ciphertext_sha256
                != crate::backup_payload::sha256_hex(&package.encrypted_bytes)
            || !package_refs.insert(package.package_ref.clone())
        {
            return Err("PRIVATE_BACKUP_RECOVERY_KEY_REF_INVALID".to_string());
        }
    }
    let rows = query_all(
        storage,
        "meeting_private_assets",
        &[
            "id",
            "owner_scope",
            "purpose",
            "entity_id",
            "custody_ref",
            "key_ref",
            "ciphertext_sha256",
            "ciphertext_bytes",
            "state",
        ],
        vec![
            eq(
                "meeting_private_assets",
                "vault_id",
                json!(&bundle.vault_id),
            ),
            eq("meeting_private_assets", "state", json!("active")),
        ],
    )?;
    if rows.len() != bundle.assets.len() {
        return Err("PRIVATE_BACKUP_ASSET_SET_MISMATCH".to_string());
    }
    let mut asset_key_refs = std::collections::BTreeSet::new();
    for asset in &bundle.assets {
        let row = rows
            .iter()
            .find(|row| {
                row.get("meeting_private_assets.id").and_then(Value::as_str)
                    == Some(asset.asset_ref.as_str())
            })
            .ok_or_else(|| "PRIVATE_BACKUP_ASSET_MISSING".to_string())?;
        if asset.ciphertext_sha256 != crate::backup_payload::sha256_hex(&asset.encrypted_bytes) {
            return Err("PRIVATE_BACKUP_ASSET_DIGEST_MISMATCH".to_string());
        }
        let (purpose, expected_key_ref) = match asset.purpose {
            crate::backup_payload::PrivateMeetingAssetPurpose::KnowledgeDocument => {
                ("knowledge_document", knowledge_key_ref.as_str())
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::AgentDraft => {
                ("agent_draft", knowledge_key_ref.as_str())
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::DeliveryPreview => {
                ("delivery_preview", knowledge_key_ref.as_str())
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::PeopleProfile => {
                ("people_profile", people_key_ref.as_str())
            }
            _ => return Err("PRIVATE_BACKUP_ASSET_PURPOSE_UNSUPPORTED".to_string()),
        };
        asset_key_refs.insert(expected_key_ref.to_string());
        let (expected_custody_ref, embedded_key_ref, inner_digest_matches, inner_size_matches) =
            if asset.purpose == crate::backup_payload::PrivateMeetingAssetPurpose::PeopleProfile {
                let profile = query(
                    storage,
                    "participant_profiles",
                    &[
                        "id",
                        "vault_id",
                        "owner_scope",
                        "profile_payload_ciphertext_ref",
                        "profile_ciphertext_sha256",
                        "key_ref",
                        "revision",
                    ],
                    vec![eq("participant_profiles", "id", json!(&asset.entity_id))],
                    1,
                )?
                .into_iter()
                .next()
                .ok_or_else(|| "PRIVATE_BACKUP_PEOPLE_PROFILE_MISSING".to_string())?;
                let reference: PrivateIdentityReference =
                    serde_json::from_slice(&asset.encrypted_bytes)
                        .map_err(|_| "PRIVATE_BACKUP_PEOPLE_PROFILE_MALFORMED".to_string())?;
                let revision = integer(&profile, "participant_profiles.revision")?;
                let (path, relative) =
                    people_asset_path(&canonical_data_root(storage)?, &asset.asset_ref)?;
                let _ = path;
                if string(&profile, "participant_profiles.vault_id")? != bundle.vault_id
                    || string(&profile, "participant_profiles.owner_scope")? != expected_scope
                    || string(
                        &profile,
                        "participant_profiles.profile_payload_ciphertext_ref",
                    )? != asset.asset_ref
                    || string(&profile, "participant_profiles.key_ref")? != people_key_ref
                    || string(&profile, "participant_profiles.profile_ciphertext_sha256")?
                        != reference.ciphertext_sha256
                    || reference.envelope.key_ref != people_key_ref
                    || reference.envelope.ciphertext.len() as i64
                        != integer(row, "meeting_private_assets.ciphertext_bytes")?
                    || revision != integer(row, "meeting_private_assets.entity_revision")?
                    || asset.entity_id != string(row, "meeting_private_assets.entity_id")?
                {
                    return Err("PRIVATE_BACKUP_PEOPLE_PROFILE_METADATA_MISMATCH".to_string());
                }
                (
                    relative,
                    reference.envelope.key_ref,
                    reference.ciphertext_sha256
                        == string(row, "meeting_private_assets.ciphertext_sha256")?,
                    reference.envelope.ciphertext.len() as i64
                        == integer(row, "meeting_private_assets.ciphertext_bytes")?,
                )
            } else {
                let payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
                    serde_json::from_slice(&asset.encrypted_bytes)
                        .map_err(|_| "PRIVATE_BACKUP_ASSET_MALFORMED".to_string())?;
                let (_, relative) =
                    knowledge_asset_path(&canonical_data_root(storage)?, &asset.asset_ref)?;
                (
                    relative,
                    payload.key_ref.as_str().to_string(),
                    payload.ciphertext_sha256
                        == string(row, "meeting_private_assets.ciphertext_sha256")?
                        && crate::backup_payload::sha256_hex(&payload.ciphertext)
                            == payload.ciphertext_sha256,
                    i64::try_from(payload.ciphertext.len()).ok()
                        == Some(integer(row, "meeting_private_assets.ciphertext_bytes")?),
                )
            };
        if string(row, "meeting_private_assets.owner_scope")? != expected_scope
            || string(row, "meeting_private_assets.purpose")? != purpose
            || string(row, "meeting_private_assets.entity_id")? != asset.entity_id
            || string(row, "meeting_private_assets.custody_ref")? != expected_custody_ref
            || string(row, "meeting_private_assets.key_ref")? != expected_key_ref
            || string(row, "meeting_private_assets.state")? != "active"
            || embedded_key_ref != expected_key_ref
            || !inner_digest_matches
            || !inner_size_matches
            || asset.version == 0
        {
            return Err("PRIVATE_BACKUP_ASSET_METADATA_MISMATCH".to_string());
        }
    }
    if asset_key_refs != package_refs {
        return Err("PRIVATE_BACKUP_RECOVERY_KEY_SET_MISMATCH".to_string());
    }
    Ok(())
}

pub(crate) fn verify_recovered_private_meeting_key(
    storage: &Storage,
    restored_storage: &Storage,
    session: &NativeOwnerUnlockSession,
    archive_id: &str,
    recovery_phrase: &str,
    bundle: &crate::backup_payload::PrivateMeetingAssetsBundle,
) -> Result<bool, String> {
    let (vault_id, owner_binding) = session.backup_identity()?;
    validate_restored_private_meeting_assets(restored_storage, bundle)?;
    if bundle.vault_id != vault_id || bundle.account_binding != owner_binding {
        return Ok(false);
    }
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let people_key_ref = people_metadata_key_ref(&vault_id)?;
    let knowledge_key_ref = format!("knowledge:{vault_id}");
    let mut recovered_keys = BTreeMap::<String, zeroize::Zeroizing<[u8; 32]>>::new();
    for package in
        std::iter::once(&bundle.recovery_package).chain(bundle.additional_recovery_packages.iter())
    {
        if package.vault_id != vault_id
            || package.account_binding != owner_binding
            || package.ciphertext_sha256
                != crate::backup_payload::sha256_hex(&package.encrypted_bytes)
            || (package.package_ref != knowledge_key_ref && package.package_ref != people_key_ref)
            || recovered_keys.contains_key(&package.package_ref)
        {
            return Err("PRIVATE_BACKUP_RECOVERY_PACKAGE_INVALID".to_string());
        }
        let recovered = crate::backup_archive::decrypt_native_recovery_package(
            &package.encrypted_bytes,
            recovery_phrase,
            archive_id,
            &package.package_ref,
            &package.vault_id,
            &package.account_binding,
        )
        .map_err(|_| "PRIVATE_BACKUP_RECOVERY_PACKAGE_AUTHENTICATION_FAILED".to_string())?;
        recovered_keys.insert(package.package_ref.clone(), recovered);
    }
    let trusted = capture_native_identity_context(storage, session)?.context;
    if trusted.vault_id != vault_id {
        return Err("PRIVATE_BACKUP_VAULT_SCOPE_MISMATCH".to_string());
    }
    for asset in &bundle.assets {
        let purpose = match asset.purpose {
            crate::backup_payload::PrivateMeetingAssetPurpose::KnowledgeDocument => {
                crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::AgentDraft => {
                crate::meeting_knowledge::PrivateAssetPurpose::AgentDraft
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::DeliveryPreview => {
                crate::meeting_knowledge::PrivateAssetPurpose::DeliveryPreview
            }
            crate::backup_payload::PrivateMeetingAssetPurpose::PeopleProfile => {
                let profile = query(
                    restored_storage,
                    "participant_profiles",
                    &[
                        "id",
                        "revision",
                        "vault_id",
                        "profile_payload_ciphertext_ref",
                    ],
                    vec![eq("participant_profiles", "id", json!(&asset.entity_id))],
                    1,
                )?
                .into_iter()
                .next()
                .ok_or_else(|| "PRIVATE_BACKUP_PEOPLE_PROFILE_MISSING".to_string())?;
                let revision = integer(&profile, "participant_profiles.revision")?;
                let reference: PrivateIdentityReference =
                    serde_json::from_slice(&asset.encrypted_bytes)
                        .map_err(|_| "PRIVATE_BACKUP_PEOPLE_PROFILE_MALFORMED".to_string())?;
                let context = people_profile_context(&trusted, &asset.entity_id, revision);
                let key_ref = people_key_ref.clone();
                let key = recovered_keys
                    .get(&key_ref)
                    .ok_or_else(|| "PRIVATE_BACKUP_RECOVERY_KEY_MISSING".to_string())?;
                let mut payload = meeting_intelligence_schema::open_person_profile_with_key(
                    &reference,
                    &context,
                    &asset.entity_id,
                    revision,
                    key.as_ref(),
                )?;
                if payload.profile_id != string(&profile, "participant_profiles.id")?
                    || payload.vault_id != vault_id
                {
                    payload.zeroize();
                    return Err("PRIVATE_BACKUP_PEOPLE_PROFILE_SCOPE_MISMATCH".to_string());
                }
                payload.zeroize();
                continue;
            }
            _ => return Err("PRIVATE_BACKUP_ASSET_PURPOSE_UNSUPPORTED".to_string()),
        };
        let payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
            serde_json::from_slice(&asset.encrypted_bytes)
                .map_err(|_| "PRIVATE_BACKUP_ASSET_MALFORMED".to_string())?;
        let context = crate::meeting_knowledge::PrivateAssetContext {
            vault_id: asset.vault_id.clone(),
            entity_id: asset.entity_id.clone(),
            version: u64::from(asset.version),
            purpose,
        };
        let key_ref = string_by_asset_purpose(asset, &vault_id)?;
        let key = recovered_keys
            .get(&key_ref)
            .ok_or_else(|| "PRIVATE_BACKUP_RECOVERY_KEY_MISSING".to_string())?;
        let verified = crate::meeting_knowledge::open_private_payload(key, &context, &payload)?;
        drop(verified);
    }
    for (key_ref, key) in &recovered_keys {
        if key_ref == &knowledge_key_ref {
            let key_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(key_ref.clone())?;
            crate::meeting_knowledge::OsKnowledgeKeyBackend.restore(&key_ref, key)?;
        } else if key_ref == &people_key_ref {
            OsPeopleMetadataKeyBackend.restore_key(key_ref, key.as_ref())?;
        } else {
            return Err("PRIVATE_BACKUP_RECOVERY_KEY_REF_INVALID".to_string());
        }
    }
    Ok(true)
}

fn string_by_asset_purpose(
    asset: &crate::backup_payload::EncryptedMeetingAsset,
    vault_id: &str,
) -> Result<String, String> {
    match asset.purpose {
        crate::backup_payload::PrivateMeetingAssetPurpose::PeopleProfile => {
            people_metadata_key_ref(vault_id)
        }
        _ => Ok(format!("knowledge:{vault_id}")),
    }
}

fn knowledge_citation_locator_receipt(
    row: &Value,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeCitationLocator, String> {
    let chunk_id = string(row, "knowledge_chunks.id")?;
    let locator: crate::meeting_knowledge::CitationLocator = serde_json::from_value(
        row.get("knowledge_chunks.locator_json")
            .cloned()
            .ok_or_else(|| "KNOWLEDGE_CITATION_LOCATOR_UNAVAILABLE".to_string())?,
    )
    .map_err(|_| "KNOWLEDGE_CITATION_LOCATOR_INVALID".to_string())?;
    let label = match locator {
        crate::meeting_knowledge::CitationLocator::TextSpan {
            start_line,
            end_line,
            start_char,
            end_char,
        } => format!("บรรทัด {start_line}–{end_line} · อักขระ {start_char}–{end_char}"),
        crate::meeting_knowledge::CitationLocator::PdfPage {
            page_number,
            start_char,
            end_char,
        } => format!("หน้า {page_number} · อักขระ {start_char}–{end_char}"),
        crate::meeting_knowledge::CitationLocator::SpreadsheetCellRange { sheet_ref, range } => {
            format!("ชีต {sheet_ref} · {range}")
        }
        crate::meeting_knowledge::CitationLocator::TranscriptRange {
            recording_id,
            revision_id,
            start_ms,
            end_ms,
        } => format!("{recording_id} · {revision_id} · {start_ms}–{end_ms} ms"),
    };
    Ok(meeting_intelligence_schema::MeetingKnowledgeCitationLocator { chunk_id, label })
}

pub(crate) fn import_selected_knowledge_document(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    collection_id: &str,
    project_id: &str,
    selected_file: &Path,
    parser_python: &Path,
    parser_script: &Path,
    pdf_parser_runtime: Option<&Path>,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeImportResult, String> {
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-knowledge-import::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let collection = query(
        storage,
        "knowledge_collections",
        &[
            "id",
            "project_id",
            "owner_scope",
            "classification",
            "status",
        ],
        vec![eq("knowledge_collections", "id", json!(collection_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_COLLECTION_UNAVAILABLE".to_string())?;
    if string(&collection, "knowledge_collections.owner_scope")?
        != local_owner_collection_scope(&trusted.vault_id)
        || string(&collection, "knowledge_collections.status")? != "active"
    {
        return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
    }
    validate_project_exists(storage, project_id)?;
    let collection_project = collection
        .get("knowledge_collections.project_id")
        .filter(|value| !value.is_null())
        .and_then(Value::as_str);
    if collection_project.is_some_and(|bound| bound != project_id) {
        return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
    }
    let file_path = selected_file
        .canonicalize()
        .map_err(|_| "KNOWLEDGE_SELECTED_FILE_UNAVAILABLE".to_string())?;
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "txt" | "md" | "pdf") {
        return Err("KNOWLEDGE_DOCUMENT_FORMAT_UNSUPPORTED".to_string());
    }
    let selected_root = file_path
        .parent()
        .ok_or_else(|| "KNOWLEDGE_SELECTED_FILE_UNAVAILABLE".to_string())?
        .canonicalize()
        .map_err(|_| "KNOWLEDGE_SELECTED_FILE_UNAVAILABLE".to_string())?;
    drop(owner_fence);

    let parsed = crate::meeting_knowledge::parse_selected_file(
        &crate::meeting_knowledge::ParserInvocation {
            python: parser_python.to_path_buf(),
            script: parser_script.to_path_buf(),
            pdf_parser_runtime: pdf_parser_runtime.map(Path::to_path_buf),
            selected_root,
            selected_file: file_path.clone(),
        },
    )?;
    session.ensure_active()?;
    ensure_session_data_root(storage, session)?;
    let mut source_file =
        File::open(&file_path).map_err(|_| "KNOWLEDGE_SELECTED_FILE_UNAVAILABLE".to_string())?;
    let mut raw_bytes = zeroize::Zeroizing::new(Vec::with_capacity(
        parsed
            .source_bytes
            .min(crate::meeting_knowledge::MAX_IMPORT_BYTES) as usize,
    ));
    source_file
        .by_ref()
        .take(crate::meeting_knowledge::MAX_IMPORT_BYTES + 1)
        .read_to_end(&mut raw_bytes)
        .map_err(|_| "KNOWLEDGE_SELECTED_FILE_UNAVAILABLE".to_string())?;
    if raw_bytes.len() as u64 != parsed.source_bytes
        || format!("{:x}", Sha256::digest(raw_bytes.as_slice())) != parsed.content_sha256
    {
        return Err("KNOWLEDGE_SOURCE_CHANGED_DURING_IMPORT".to_string());
    }
    let duplicate = query(
        storage,
        "knowledge_documents",
        &["id", "current_version_id", "status"],
        vec![
            eq("knowledge_documents", "collection_id", json!(collection_id)),
            eq(
                "knowledge_documents",
                "source_ref",
                json!(&parsed.content_sha256),
            ),
            eq("knowledge_documents", "status", json!("active")),
        ],
        1,
    )?
    .into_iter()
    .next();
    if let Some(document) = duplicate {
        if let Some(version_id) = document
            .get("knowledge_documents.current_version_id")
            .and_then(Value::as_str)
        {
            let chunks = query(
                storage,
                "knowledge_chunks",
                &["id", "locator_json"],
                vec![eq("knowledge_chunks", "version_id", json!(version_id))],
                3000,
            )?;
            let citation_locators = chunks
                .iter()
                .map(knowledge_citation_locator_receipt)
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(meeting_intelligence_schema::MeetingKnowledgeImportResult {
                collection_id: collection_id.to_string(),
                document_id: string(&document, "knowledge_documents.id")?,
                version_id: version_id.to_string(),
                chunk_count: chunks.len() as u32,
                content_sha256: parsed.content_sha256.clone(),
                parser_version: parsed.parser_version.clone(),
                warnings: parsed.warnings.clone(),
                citation_locators,
            });
        }
    }
    let mut chunks = crate::meeting_knowledge::chunk_document(&parsed);
    if chunks.is_empty() {
        return Err("KNOWLEDGE_DOCUMENT_HAS_NO_TEXT".to_string());
    }
    let document_id = format!("doc-{collection_id}-{}", &parsed.content_sha256[..32]);
    let version_id = format!("ver-{document_id}-1");
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let source_context = crate::meeting_knowledge::PrivateAssetContext {
        vault_id: trusted.vault_id.clone(),
        entity_id: document_id.clone(),
        version: 1,
        purpose: crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument,
    };
    let source_asset_id = format!("asset-{version_id}-source");
    let (source_custody_ref, source_ciphertext_sha256, source_bytes, source_retention) =
        write_encrypted_knowledge_asset(
            &session.data_root,
            &source_asset_id,
            &key,
            &source_context,
            &raw_bytes,
        )?;
    let timestamp = attempt.committed_at.clone();
    let mut mutations = vec![
        upsert(
            "knowledge_documents",
            json!({
                "id": document_id,
                "collection_id": collection_id,
                "source_kind": "selected_file",
                "source_ref": parsed.content_sha256,
                "title_ref": source_asset_id,
                "current_version_id": version_id,
                "acl_revision": 1,
                "status": "active",
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ),
        upsert(
            "knowledge_document_versions",
            json!({
                "id": version_id,
                "document_id": document_id,
                "version_no": 1,
                "version_label": parsed.content_sha256,
                "content_hash": parsed.content_sha256,
                "custody_ref": source_asset_id,
                "mime_type": parsed.mime_type,
                "source_modified_at": std::fs::metadata(&file_path).ok().and_then(|metadata| metadata.modified().ok()).map(chrono::DateTime::<chrono::Utc>::from).map(|value| value.to_rfc3339()),
                "ingested_at": timestamp,
                "parser_version": parsed.parser_version,
                "validity_json": {"parser_fingerprint": parsed.parser_fingerprint, "dependency": parsed.dependency, "warnings": parsed.warnings},
                "state": "active",
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
            }),
        ),
        upsert(
            "meeting_private_assets",
            json!({
                "id": source_asset_id,
                "project_id": project_id,
                "vault_id": trusted.vault_id,
                "owner_scope": local_owner_collection_scope(&trusted.vault_id),
                "purpose": "knowledge_document",
                "entity_id": document_id,
                "entity_revision": 1,
                "custody_ref": source_custody_ref,
                "key_ref": key_ref.as_str(),
                "ciphertext_sha256": source_ciphertext_sha256,
                "ciphertext_bytes": source_bytes,
                "state": "active",
                "retention_json": source_retention,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ),
    ];
    let mut citation_locators = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter_mut().enumerate() {
        let chunk_id = format!("chunk-{version_id}-{index}");
        let content_hash = format!("{:x}", Sha256::digest(chunk.text.as_bytes()));
        let context = crate::meeting_knowledge::PrivateAssetContext {
            vault_id: trusted.vault_id.clone(),
            entity_id: chunk_id.clone(),
            version: 1,
            purpose: crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument,
        };
        let (custody_ref, ciphertext_sha256, ciphertext_bytes, retention) =
            write_encrypted_knowledge_asset(
                &session.data_root,
                &chunk_id,
                &key,
                &context,
                chunk.text.as_bytes(),
            )?;
        let locator = chunk.locator.as_citation_locator()?;
        mutations.push(upsert(
            "meeting_private_assets",
            json!({
                "id": chunk_id,
                "project_id": project_id,
                "vault_id": trusted.vault_id,
                "owner_scope": local_owner_collection_scope(&trusted.vault_id),
                "purpose": "knowledge_document",
                "entity_id": chunk_id,
                "entity_revision": 1,
                "custody_ref": custody_ref,
                "key_ref": key_ref.as_str(),
                "ciphertext_sha256": ciphertext_sha256,
                "ciphertext_bytes": ciphertext_bytes,
                "state": "active",
                "retention_json": retention,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ));
        let knowledge_chunk_id = format!("knowledge-chunk-{chunk_id}");
        let locator_row = json!({"knowledge_chunks.id": knowledge_chunk_id, "knowledge_chunks.locator_json": locator});
        citation_locators.push(knowledge_citation_locator_receipt(&locator_row)?);
        mutations.push(upsert(
            "knowledge_chunks",
            json!({
                "id": knowledge_chunk_id,
                "version_id": version_id,
                "text_ref": chunk_id,
                "locator_json": locator,
                "token_start": null,
                "token_end": null,
                "byte_start": null,
                "byte_end": null,
                "extraction_quality_json": {"parser_version": parsed.parser_version, "warnings": parsed.warnings},
                "index_generation": 1,
                "content_hash": content_hash,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
            }),
        ));
    }
    session.ensure_active()?;
    let _ = commit_native_owner_mutations(storage, session, &attempt, mutations)?;
    Ok(meeting_intelligence_schema::MeetingKnowledgeImportResult {
        collection_id: collection_id.to_string(),
        document_id,
        version_id,
        chunk_count: chunks.len() as u32,
        content_sha256: parsed.content_sha256,
        parser_version: parsed.parser_version,
        warnings: parsed.warnings,
        citation_locators,
    })
}

fn validate_meeting_metric_selection(
    storage: &Storage,
    vault_id: &str,
    project_id: &str,
    recording_id: &str,
    collection_ids: &[String],
) -> Result<std::collections::BTreeSet<String>, String> {
    if collection_ids.is_empty() || collection_ids.len() > 64 {
        return Err("MEETING_KNOWLEDGE_SELECTION_REQUIRED".to_string());
    }
    let requested = collection_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if requested.len() != collection_ids.len() {
        return Err("MEETING_KNOWLEDGE_SELECTION_INVALID".to_string());
    }
    validate_project_exists(storage, project_id)?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let context = meeting_context_row(storage, &meeting_session_id)?
        .ok_or_else(|| "MEETING_KNOWLEDGE_SELECTION_REQUIRED".to_string())?;
    if string(&context, "meeting_session_contexts.owner_scope")?
        != local_owner_collection_scope(vault_id)
        || context
            .get("meeting_session_contexts.vault_id")
            .and_then(Value::as_str)
            != Some(vault_id)
        || string(&context, "meeting_session_contexts.project_id")? != project_id
        || string(&context, "meeting_session_contexts.recording_id")? != recording_id
    {
        return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
    }
    let selected = selected_collections_from_context(Some(&context))?;
    let persisted = selected
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if selected.len() != collection_ids.len() || persisted != requested {
        return Err("MEETING_KNOWLEDGE_SELECTION_STALE".to_string());
    }
    for collection_id in &selected {
        let collection = query(
            storage,
            "knowledge_collections",
            &["project_id", "owner_scope", "status"],
            vec![eq("knowledge_collections", "id", json!(collection_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string())?;
        let collection_project = collection
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&collection, "knowledge_collections.owner_scope")?
            != local_owner_collection_scope(vault_id)
            || collection_project.is_some_and(|value| value != project_id)
            || string(&collection, "knowledge_collections.status")? != "active"
        {
            return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
        }
    }
    Ok(requested)
}

fn current_metric_citation(
    storage: &Storage,
    vault_id: &str,
    project_id: &str,
    selected: &std::collections::BTreeSet<String>,
    version_id: &str,
    chunk_id: &str,
    retrieved_at: &str,
    read_grant_id: &str,
) -> Result<crate::meeting_knowledge::EvidenceCitation, String> {
    use crate::meeting_knowledge::{CitationLocator, EvidenceCitation};

    let version = query(
        storage,
        "knowledge_document_versions",
        &["document_id", "version_no", "version_label", "state"],
        vec![eq("knowledge_document_versions", "id", json!(version_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_VERSION_UNAVAILABLE".to_string())?;
    if string(&version, "knowledge_document_versions.state")? != "active" {
        return Err("MEETING_KNOWLEDGE_EVIDENCE_STALE".to_string());
    }
    let document_id = string(&version, "knowledge_document_versions.document_id")?;
    let document = query(
        storage,
        "knowledge_documents",
        &[
            "collection_id",
            "current_version_id",
            "acl_revision",
            "status",
        ],
        vec![eq("knowledge_documents", "id", json!(&document_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_DOCUMENT_UNAVAILABLE".to_string())?;
    let collection_id = string(&document, "knowledge_documents.collection_id")?;
    if !selected.contains(&collection_id)
        || document
            .get("knowledge_documents.current_version_id")
            .and_then(Value::as_str)
            != Some(version_id)
        || string(&document, "knowledge_documents.status")? != "active"
    {
        return Err("MEETING_KNOWLEDGE_EVIDENCE_STALE".to_string());
    }
    let collection = query(
        storage,
        "knowledge_collections",
        &["project_id", "owner_scope", "status"],
        vec![eq("knowledge_collections", "id", json!(&collection_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string())?;
    let collection_project = collection
        .get("knowledge_collections.project_id")
        .filter(|value| !value.is_null())
        .and_then(Value::as_str);
    if string(&collection, "knowledge_collections.owner_scope")?
        != local_owner_collection_scope(vault_id)
        || collection_project.is_some_and(|value| value != project_id)
        || string(&collection, "knowledge_collections.status")? != "active"
    {
        return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
    }
    let chunk = query(
        storage,
        "knowledge_chunks",
        &["version_id", "text_ref", "locator_json", "content_hash"],
        vec![
            eq("knowledge_chunks", "id", json!(chunk_id)),
            eq("knowledge_chunks", "version_id", json!(version_id)),
        ],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_CITATION_LOCATOR_UNAVAILABLE".to_string())?;
    let asset_id = string(&chunk, "knowledge_chunks.text_ref")?;
    let asset = query(
        storage,
        "meeting_private_assets",
        &[
            "project_id",
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "state",
        ],
        vec![eq("meeting_private_assets", "id", json!(&asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_KNOWLEDGE_ASSET_UNAVAILABLE".to_string())?;
    if string(&asset, "meeting_private_assets.project_id")? != project_id
        || asset
            .get("meeting_private_assets.vault_id")
            .and_then(Value::as_str)
            != Some(vault_id)
        || string(&asset, "meeting_private_assets.owner_scope")?
            != local_owner_collection_scope(vault_id)
        || string(&asset, "meeting_private_assets.purpose")? != "knowledge_document"
        || string(&asset, "meeting_private_assets.entity_id")? != asset_id
        || string(&asset, "meeting_private_assets.state")? != "active"
    {
        return Err("MEETING_KNOWLEDGE_ASSET_UNAVAILABLE".to_string());
    }
    let locator: CitationLocator = serde_json::from_value(
        chunk
            .get("knowledge_chunks.locator_json")
            .cloned()
            .ok_or_else(|| "KNOWLEDGE_CITATION_LOCATOR_UNAVAILABLE".to_string())?,
    )
    .map_err(|_| "KNOWLEDGE_CITATION_LOCATOR_INVALID".to_string())?;
    let citation = EvidenceCitation {
        collection_id,
        document_id,
        document_version_id: version_id.to_string(),
        document_version_number: u64::try_from(integer(
            &version,
            "knowledge_document_versions.version_no",
        )?)
        .map_err(|_| "MEETING_KNOWLEDGE_VERSION_UNAVAILABLE".to_string())?,
        source_version: string(&version, "knowledge_document_versions.version_label")?,
        content_sha256: string(&chunk, "knowledge_chunks.content_hash")?,
        locator,
        retrieved_at: retrieved_at.to_string(),
        read_grant_id: read_grant_id.to_string(),
        acl_revision: u64::try_from(integer(&document, "knowledge_documents.acl_revision")?)
            .map_err(|_| "MEETING_KNOWLEDGE_ACL_UNAVAILABLE".to_string())?,
    };
    citation.validate()?;
    Ok(citation)
}

pub(crate) fn save_meeting_knowledge_metric(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingKnowledgeMetricSaveRequest,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeMetricSaveResult, String> {
    use crate::meeting_knowledge::{Decimal, MetricObservation};

    if request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.request_id.chars().any(char::is_control)
    {
        return Err("MEETING_KNOWLEDGE_METRIC_REQUEST_INVALID".to_string());
    }
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-metric-save::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let selected = validate_meeting_metric_selection(
        storage,
        &trusted.vault_id,
        &request.project_id,
        &request.recording_id,
        &request.collection_ids,
    )?;
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let digest = format!("{:x}", Sha256::digest(request.request_id.as_bytes()));
    let observation_id = format!("metric-{}", &digest[..32]);
    let read_grant_digest = format!("{:x}", Sha256::digest(observation_id.as_bytes()));
    let read_grant_id = format!("read:{}", &read_grant_digest[..32]);
    let citation = current_metric_citation(
        storage,
        &trusted.vault_id,
        &request.project_id,
        &selected,
        &request.document_version_id,
        &request.chunk_id,
        &attempt.committed_at,
        &read_grant_id,
    )?;
    let value = Decimal::parse(&request.value)?;
    let observation = MetricObservation {
        metric_key: request.metric_key.clone(),
        organization_ref: request.organization_ref.clone(),
        period_start: request.period_start.clone(),
        period_end: request.period_end.clone(),
        calendar: request.calendar.clone(),
        unit: request.unit.clone(),
        currency: request.currency.clone(),
        scale: request.scale.clone(),
        basis: request.basis.clone(),
        value,
        citation: citation.clone(),
    };
    crate::meeting_knowledge::validate_metric_observation(&observation)
        .map_err(|_| "MEETING_KNOWLEDGE_METRIC_INVALID".to_string())?;
    let expected_locator = serde_json::to_value(&citation.locator)
        .map_err(|_| "KNOWLEDGE_CITATION_LOCATOR_INVALID".to_string())?;
    let basis = match &observation.basis {
        crate::meeting_knowledge::MetricBasis::Actual => "actual",
        crate::meeting_knowledge::MetricBasis::Budget => "budget",
    };
    let existing = query(
        storage,
        "knowledge_metric_observations",
        &[
            "version_id",
            "metric_key",
            "organization_ref",
            "period_start",
            "period_end",
            "calendar",
            "decimal_value",
            "unit",
            "currency",
            "scale",
            "actual_budget",
            "locator_json",
            "created_at",
        ],
        vec![eq(
            "knowledge_metric_observations",
            "id",
            json!(&observation_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    if let Some(row) = existing {
        let existing_value = crate::meeting_knowledge::open_metric_observation_value(
            &key,
            &key_ref,
            &trusted.vault_id,
            &observation_id,
            &string(&row, "knowledge_metric_observations.decimal_value")?,
        )?;
        let same = string(&row, "knowledge_metric_observations.version_id")?
            == request.document_version_id
            && string(&row, "knowledge_metric_observations.metric_key")? == request.metric_key
            && string(&row, "knowledge_metric_observations.organization_ref")?
                == request.organization_ref
            && row
                .get("knowledge_metric_observations.period_start")
                .and_then(Value::as_str)
                == Some(request.period_start.as_str())
            && row
                .get("knowledge_metric_observations.period_end")
                .and_then(Value::as_str)
                == Some(request.period_end.as_str())
            && string(&row, "knowledge_metric_observations.calendar")? == request.calendar
            && existing_value == value
            && string(&row, "knowledge_metric_observations.unit")? == request.unit
            && row
                .get("knowledge_metric_observations.currency")
                .and_then(Value::as_str)
                == request.currency.as_deref()
            && string(&row, "knowledge_metric_observations.scale")? == request.scale
            && string(&row, "knowledge_metric_observations.actual_budget")? == basis
            && row.get("knowledge_metric_observations.locator_json") == Some(&expected_locator);
        if !same {
            return Err("MEETING_KNOWLEDGE_METRIC_REQUEST_ID_REUSED".to_string());
        }
        let mut citation = citation;
        citation.retrieved_at = string(&row, "knowledge_metric_observations.created_at")?;
        citation.read_grant_id = read_grant_id;
        return Ok(
            meeting_intelligence_schema::MeetingKnowledgeMetricSaveResult {
                observation_id,
                basis: observation.basis,
                value,
                citation,
            },
        );
    }
    let same_source_rows = query(
        storage,
        "knowledge_metric_observations",
        &["id", "decimal_value", "locator_json", "created_at"],
        vec![
            eq(
                "knowledge_metric_observations",
                "version_id",
                json!(&request.document_version_id),
            ),
            eq(
                "knowledge_metric_observations",
                "metric_key",
                json!(&request.metric_key),
            ),
            eq(
                "knowledge_metric_observations",
                "organization_ref",
                json!(&request.organization_ref),
            ),
            eq(
                "knowledge_metric_observations",
                "period_start",
                json!(&request.period_start),
            ),
            eq(
                "knowledge_metric_observations",
                "period_end",
                json!(&request.period_end),
            ),
            eq(
                "knowledge_metric_observations",
                "calendar",
                json!(&request.calendar),
            ),
            eq(
                "knowledge_metric_observations",
                "unit",
                json!(&request.unit),
            ),
            eq(
                "knowledge_metric_observations",
                "currency",
                json!(&request.currency),
            ),
            eq(
                "knowledge_metric_observations",
                "scale",
                json!(&request.scale),
            ),
            eq(
                "knowledge_metric_observations",
                "actual_budget",
                json!(basis),
            ),
        ],
        2001,
    )?;
    if same_source_rows.len() > 2000 {
        return Err("MEETING_KNOWLEDGE_METRIC_AMBIGUOUS".to_string());
    }
    for row in same_source_rows {
        if row.get("knowledge_metric_observations.locator_json") != Some(&expected_locator) {
            continue;
        }
        let existing_id = string(&row, "knowledge_metric_observations.id")?;
        let existing_value = crate::meeting_knowledge::open_metric_observation_value(
            &key,
            &key_ref,
            &trusted.vault_id,
            &existing_id,
            &string(&row, "knowledge_metric_observations.decimal_value")?,
        )?;
        if existing_value != value {
            return Err("MEETING_KNOWLEDGE_METRIC_SOURCE_CONFLICT".to_string());
        }
        let duplicate_digest = format!("{:x}", Sha256::digest(existing_id.as_bytes()));
        let mut duplicate_citation = citation.clone();
        duplicate_citation.retrieved_at = string(&row, "knowledge_metric_observations.created_at")?;
        duplicate_citation.read_grant_id = format!("read:{}", &duplicate_digest[..32]);
        return Ok(
            meeting_intelligence_schema::MeetingKnowledgeMetricSaveResult {
                observation_id: existing_id,
                basis: observation.basis,
                value,
                citation: duplicate_citation,
            },
        );
    }
    let encrypted_value = crate::meeting_knowledge::seal_metric_observation_value(
        &key,
        &key_ref,
        &trusted.vault_id,
        &observation_id,
        value,
    )?;
    let row = json!({
        "id": observation_id,
        "version_id": request.document_version_id,
        "metric_key": observation.metric_key,
        "organization_ref": observation.organization_ref,
        "period_start": observation.period_start,
        "period_end": observation.period_end,
        "calendar": observation.calendar,
        "decimal_value": encrypted_value,
        "unit": observation.unit,
        "currency": observation.currency,
        "scale": observation.scale,
        "actual_budget": basis,
        "locator_json": citation.locator,
        "extraction_confidence": null,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": attempt.committed_at,
    });
    session.ensure_active()?;
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![upsert("knowledge_metric_observations", row)],
    )?;
    Ok(
        meeting_intelligence_schema::MeetingKnowledgeMetricSaveResult {
            observation_id,
            basis: observation.basis,
            value,
            citation,
        },
    )
}

pub(crate) fn compute_meeting_knowledge_metric(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingKnowledgeMetricComputeRequest,
) -> Result<meeting_intelligence_schema::MeetingKnowledgeMetricComputeResult, String> {
    use crate::meeting_knowledge::{MetricBasis, MetricObservation};

    let native_capture = capture_native_identity_context(storage, session)?;
    let trusted = native_capture.context;
    let account_guard = native_capture
        .account_guard
        .as_ref()
        .ok_or_else(|| "native account operation is unavailable".to_string())?;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    account_guard.with_account_lifecycle_fence(&session.account_witness, || {
        let selected = validate_meeting_metric_selection(
            storage,
            &trusted.vault_id,
            &request.project_id,
            &request.recording_id,
            &request.collection_ids,
        )?;
        let key_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(format!(
            "knowledge:{}",
            trusted.vault_id
        ))?;
        let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
        let rows = query(
            storage,
            "knowledge_metric_observations",
            &[
                "id",
                "version_id",
                "metric_key",
                "organization_ref",
                "period_start",
                "period_end",
                "calendar",
                "decimal_value",
                "unit",
                "currency",
                "scale",
                "actual_budget",
                "locator_json",
            ],
            vec![
                eq(
                    "knowledge_metric_observations",
                    "metric_key",
                    json!(&request.metric_key),
                ),
                eq(
                    "knowledge_metric_observations",
                    "organization_ref",
                    json!(&request.organization_ref),
                ),
                eq(
                    "knowledge_metric_observations",
                    "period_start",
                    json!(&request.period_start),
                ),
                eq(
                    "knowledge_metric_observations",
                    "period_end",
                    json!(&request.period_end),
                ),
                eq(
                    "knowledge_metric_observations",
                    "calendar",
                    json!(&request.calendar),
                ),
                eq(
                    "knowledge_metric_observations",
                    "unit",
                    json!(&request.unit),
                ),
                eq(
                    "knowledge_metric_observations",
                    "currency",
                    json!(&request.currency),
                ),
                eq(
                    "knowledge_metric_observations",
                    "scale",
                    json!(&request.scale),
                ),
            ],
            2001,
        )?;
        if rows.len() > 2000 {
            return Err("MEETING_KNOWLEDGE_METRIC_AMBIGUOUS".to_string());
        }
        let mut actuals = Vec::new();
        let mut budgets = Vec::new();
        for row in rows {
            let period_start = row
                .get("knowledge_metric_observations.period_start")
                .and_then(Value::as_str);
            let period_end = row
                .get("knowledge_metric_observations.period_end")
                .and_then(Value::as_str);
            let currency = row
                .get("knowledge_metric_observations.currency")
                .and_then(Value::as_str);
            if period_start != Some(request.period_start.as_str())
                || period_end != Some(request.period_end.as_str())
                || string(&row, "knowledge_metric_observations.calendar")? != request.calendar
                || string(&row, "knowledge_metric_observations.unit")? != request.unit
                || currency != request.currency.as_deref()
                || string(&row, "knowledge_metric_observations.scale")? != request.scale
            {
                continue;
            }
            let locator_value = row
                .get("knowledge_metric_observations.locator_json")
                .cloned()
                .ok_or_else(|| "KNOWLEDGE_CITATION_LOCATOR_UNAVAILABLE".to_string())?;
            let version_id = string(&row, "knowledge_metric_observations.version_id")?;
            let chunks = query(
                storage,
                "knowledge_chunks",
                &["id", "locator_json"],
                vec![eq("knowledge_chunks", "version_id", json!(&version_id))],
                3000,
            )?;
            let Some(chunk) = chunks
                .iter()
                .find(|item| item.get("knowledge_chunks.locator_json") == Some(&locator_value))
            else {
                continue;
            };
            let chunk_id = string(chunk, "knowledge_chunks.id")?;
            let read_grant_id = format!("read:{}", Uuid::new_v4());
            let citation = match current_metric_citation(
                storage,
                &trusted.vault_id,
                &request.project_id,
                &selected,
                &version_id,
                &chunk_id,
                &chrono::Utc::now().to_rfc3339(),
                &read_grant_id,
            ) {
                Ok(value) => value,
                Err(error)
                    if error.starts_with("MEETING_KNOWLEDGE_EVIDENCE_STALE")
                        || error.starts_with("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE") =>
                {
                    continue
                }
                Err(error) => return Err(error),
            };
            let basis = match string(&row, "knowledge_metric_observations.actual_budget")?.as_str()
            {
                "actual" => MetricBasis::Actual,
                "budget" => MetricBasis::Budget,
                _ => return Err("MEETING_KNOWLEDGE_METRIC_BASIS_INVALID".to_string()),
            };
            let observation_id = string(&row, "knowledge_metric_observations.id")?;
            let value = crate::meeting_knowledge::open_metric_observation_value(
                &key,
                &key_ref,
                &trusted.vault_id,
                &observation_id,
                &string(&row, "knowledge_metric_observations.decimal_value")?,
            )?;
            let observation = MetricObservation {
                metric_key: request.metric_key.clone(),
                organization_ref: request.organization_ref.clone(),
                period_start: request.period_start.clone(),
                period_end: request.period_end.clone(),
                calendar: request.calendar.clone(),
                unit: request.unit.clone(),
                currency: request.currency.clone(),
                scale: request.scale.clone(),
                basis,
                value,
                citation,
            };
            crate::meeting_knowledge::validate_metric_observation(&observation)
                .map_err(|_| "MEETING_KNOWLEDGE_METRIC_INVALID".to_string())?;
            match observation.basis {
                MetricBasis::Actual => actuals.push(observation),
                MetricBasis::Budget => budgets.push(observation),
            }
        }
        let computation =
            crate::meeting_knowledge::compute_selected_actual_vs_budget(&actuals, &budgets)
                .map_err(|error| {
                    match error {
            crate::meeting_knowledge::MetricArithmeticError::InvalidEvidence => {
                "MEETING_KNOWLEDGE_METRIC_INVALID"
            }
            crate::meeting_knowledge::MetricArithmeticError::IncompatibleObservations => {
                "MEETING_KNOWLEDGE_METRIC_AMBIGUOUS"
            }
            crate::meeting_knowledge::MetricArithmeticError::ConflictingSources => {
                "MEETING_KNOWLEDGE_METRIC_CONFLICT"
            }
            crate::meeting_knowledge::MetricArithmeticError::NotComputable => {
                "MEETING_KNOWLEDGE_METRIC_NOT_COMPUTABLE"
            }
            crate::meeting_knowledge::MetricArithmeticError::Overflow => {
                "MEETING_KNOWLEDGE_METRIC_OVERFLOW"
            }
        }
        .to_string()
                })?;
        session.ensure_active()?;
        Ok(
            meeting_intelligence_schema::MeetingKnowledgeMetricComputeResult {
                metric_key: request.metric_key.clone(),
                organization_ref: request.organization_ref.clone(),
                period_start: request.period_start.clone(),
                period_end: request.period_end.clone(),
                percentage: computation.percentage,
                citations: computation.citations,
            },
        )
    })
}

struct NativeKnowledgeReadBoundary<'a> {
    storage: &'a Storage,
    session: &'a NativeOwnerUnlockSession,
    project_id: &'a str,
    meeting_session_id: &'a str,
    selected: std::collections::BTreeSet<String>,
}

impl NativeKnowledgeReadBoundary<'_> {
    fn source_is_current(
        &self,
        grant: &crate::meeting_knowledge::KnowledgeReadGrant,
    ) -> Result<bool, String> {
        let trusted = capture_native_identity_context(self.storage, self.session)?.context;
        if grant.owner_principal_ref != trusted.owner_principal_ref
            || grant.vault_id != trusted.vault_id
            || grant.account_generation != self.session.account_witness.account_generation
            || grant.identity_generation != self.session.session_generation
            || !self.selected.contains(&grant.collection_id)
        {
            return Ok(false);
        }
        let collection = query(
            self.storage,
            "knowledge_collections",
            &["project_id", "owner_scope", "revision", "status"],
            vec![eq(
                "knowledge_collections",
                "id",
                json!(&grant.collection_id),
            )],
            1,
        )?
        .into_iter()
        .next();
        let Some(collection) = collection else {
            return Ok(false);
        };
        let collection_project = collection
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&collection, "knowledge_collections.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
            || collection_project.is_some_and(|project| project != self.project_id)
            || string(&collection, "knowledge_collections.status")? != "active"
        {
            return Ok(false);
        }
        let document = query(
            self.storage,
            "knowledge_documents",
            &[
                "collection_id",
                "current_version_id",
                "acl_revision",
                "status",
            ],
            vec![eq("knowledge_documents", "id", json!(&grant.document_id))],
            1,
        )?
        .into_iter()
        .next();
        let Some(document) = document else {
            return Ok(false);
        };
        if string(&document, "knowledge_documents.collection_id")? != grant.collection_id
            || document
                .get("knowledge_documents.current_version_id")
                .and_then(Value::as_str)
                != Some(grant.document_version_id.as_str())
            || integer(&document, "knowledge_documents.acl_revision")? as u64 != grant.acl_revision
            || string(&document, "knowledge_documents.status")? != "active"
        {
            return Ok(false);
        }
        let version = query(
            self.storage,
            "knowledge_document_versions",
            &["document_id", "state"],
            vec![eq(
                "knowledge_document_versions",
                "id",
                json!(&grant.document_version_id),
            )],
            1,
        )?
        .into_iter()
        .next();
        let Some(version) = version else {
            return Ok(false);
        };
        if string(&version, "knowledge_document_versions.document_id")? != grant.document_id
            || string(&version, "knowledge_document_versions.state")? != "active"
        {
            return Ok(false);
        }
        let asset = query(
            self.storage,
            "meeting_private_assets",
            &["project_id", "vault_id", "owner_scope", "purpose", "state"],
            vec![eq(
                "meeting_private_assets",
                "id",
                json!(&grant.private_asset_id),
            )],
            1,
        )?
        .into_iter()
        .next();
        let Some(asset) = asset else {
            return Ok(false);
        };
        Ok(
            string(&asset, "meeting_private_assets.project_id")? == self.project_id
                && asset
                    .get("meeting_private_assets.vault_id")
                    .and_then(Value::as_str)
                    == Some(trusted.vault_id.as_str())
                && string(&asset, "meeting_private_assets.owner_scope")?
                    == local_owner_collection_scope(&trusted.vault_id)
                && string(&asset, "meeting_private_assets.purpose")? == "knowledge_document"
                && string(&asset, "meeting_private_assets.state")? == "active",
        )
    }
}

impl crate::meeting_knowledge::KnowledgeReadBoundary for NativeKnowledgeReadBoundary<'_> {
    fn authorize(
        &self,
        query: &crate::meeting_knowledge::KnowledgeQuery,
        selected_collections: &std::collections::BTreeSet<String>,
        chunk: &crate::meeting_knowledge::StoredKnowledgeChunk,
    ) -> Result<Option<crate::meeting_knowledge::KnowledgeReadGrant>, String> {
        if query.meeting_session_id != self.meeting_session_id
            || selected_collections != &self.selected
            || !self.selected.contains(&chunk.collection_id)
        {
            return Ok(None);
        }
        let grant = crate::meeting_knowledge::KnowledgeReadGrant {
            grant_id: format!("read:{}", Uuid::new_v4()),
            owner_principal_ref: self.session.context.owner_principal_ref.clone(),
            vault_id: self.session.context.vault_id.clone(),
            account_generation: self.session.account_witness.account_generation,
            identity_generation: self.session.session_generation,
            private_asset_id: chunk.id.clone(),
            collection_id: chunk.collection_id.clone(),
            document_id: chunk.document_id.clone(),
            document_version_id: chunk.document_version_id.clone(),
            acl_revision: chunk.acl_revision,
        };
        if self.source_is_current(&grant)? {
            Ok(Some(grant))
        } else {
            Ok(None)
        }
    }

    fn open(
        &self,
        grant: &crate::meeting_knowledge::KnowledgeReadGrant,
        context: &crate::meeting_knowledge::PrivateAssetContext,
        key_ref: &crate::meeting_knowledge::KnowledgeKeyRef,
        payload: &crate::meeting_knowledge::EncryptedKnowledgePayload,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
        if context.vault_id != grant.vault_id
            || context.entity_id != grant.private_asset_id
            || context.purpose != crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument
            || key_ref.as_str() != format!("knowledge:{}", grant.vault_id)
            || &payload.key_ref != key_ref
            || !self.source_is_current(grant)?
        {
            return Err("MEETING_KNOWLEDGE_READ_REVOKED".to_string());
        }
        let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(key_ref)?;
        let plaintext = crate::meeting_knowledge::open_private_payload(&key, context, payload)?;
        if !self.source_is_current(grant)? {
            return Err("MEETING_KNOWLEDGE_READ_REVOKED".to_string());
        }
        Ok(plaintext)
    }

    fn revalidate(
        &self,
        grant: &crate::meeting_knowledge::KnowledgeReadGrant,
    ) -> Result<bool, String> {
        self.source_is_current(grant)
    }
}

pub(crate) fn search_selected_meeting_knowledge(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    collection_ids: &[String],
    query_text: &str,
) -> Result<crate::meeting_knowledge::KnowledgeSearchResponse, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    if collection_ids.is_empty() || collection_ids.len() > 64 {
        return Err("MEETING_KNOWLEDGE_SELECTION_REQUIRED".to_string());
    }
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    validate_project_exists(storage, project_id)?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let context = meeting_context_row(storage, &meeting_session_id)?
        .ok_or_else(|| "MEETING_KNOWLEDGE_SELECTION_REQUIRED".to_string())?;
    if string(&context, "meeting_session_contexts.owner_scope")?
        != local_owner_collection_scope(&trusted.vault_id)
        || context
            .get("meeting_session_contexts.vault_id")
            .and_then(Value::as_str)
            != Some(trusted.vault_id.as_str())
        || string(&context, "meeting_session_contexts.project_id")? != project_id
        || string(&context, "meeting_session_contexts.recording_id")? != recording_id
    {
        return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
    }
    let selected = selected_collections_from_context(Some(&context))?;
    let requested = collection_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let persisted = selected
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if selected.len() != collection_ids.len() || requested != persisted {
        return Err("MEETING_KNOWLEDGE_SELECTION_STALE".to_string());
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let mut chunks = Vec::new();
    for collection_id in &selected {
        let collection = query(
            storage,
            "knowledge_collections",
            &[
                "project_id",
                "owner_scope",
                "classification",
                "revision",
                "status",
            ],
            vec![eq("knowledge_collections", "id", json!(collection_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string())?;
        let collection_project = collection
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&collection, "knowledge_collections.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
            || collection_project.is_some_and(|bound| bound != project_id)
            || string(&collection, "knowledge_collections.status")? != "active"
        {
            return Err("MEETING_KNOWLEDGE_COLLECTION_NOT_READABLE".to_string());
        }
        let documents = query(
            storage,
            "knowledge_documents",
            &["id", "current_version_id", "acl_revision", "status"],
            vec![
                eq("knowledge_documents", "collection_id", json!(collection_id)),
                eq("knowledge_documents", "status", json!("active")),
            ],
            1000,
        )?;
        for document in documents {
            let document_id = string(&document, "knowledge_documents.id")?;
            let version_id = document
                .get("knowledge_documents.current_version_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "MEETING_KNOWLEDGE_VERSION_UNAVAILABLE".to_string())?;
            let version = query(
                storage,
                "knowledge_document_versions",
                &["version_no", "version_label", "content_hash", "state"],
                vec![eq("knowledge_document_versions", "id", json!(version_id))],
                1,
            )?
            .into_iter()
            .next()
            .ok_or_else(|| "MEETING_KNOWLEDGE_VERSION_UNAVAILABLE".to_string())?;
            if string(&version, "knowledge_document_versions.state")? != "active" {
                continue;
            }
            let chunks_rows = query(
                storage,
                "knowledge_chunks",
                &["id", "text_ref", "locator_json", "content_hash"],
                vec![eq("knowledge_chunks", "version_id", json!(version_id))],
                1000,
            )?;
            for chunk_row in chunks_rows {
                if std::time::Instant::now() >= deadline {
                    return Err("MEETING_KNOWLEDGE_SEARCH_TIMEOUT".to_string());
                }
                if chunks.len() >= 2000 {
                    return Err("MEETING_KNOWLEDGE_CANDIDATE_LIMIT".to_string());
                }
                let asset_id = string(&chunk_row, "knowledge_chunks.text_ref")?;
                let asset = query(
                    storage,
                    "meeting_private_assets",
                    &[
                        "project_id",
                        "vault_id",
                        "owner_scope",
                        "purpose",
                        "entity_id",
                        "entity_revision",
                        "custody_ref",
                        "key_ref",
                        "ciphertext_sha256",
                        "state",
                    ],
                    vec![eq("meeting_private_assets", "id", json!(&asset_id))],
                    1,
                )?
                .into_iter()
                .next()
                .ok_or_else(|| "MEETING_KNOWLEDGE_ASSET_UNAVAILABLE".to_string())?;
                if string(&asset, "meeting_private_assets.project_id")? != project_id
                    || asset
                        .get("meeting_private_assets.vault_id")
                        .and_then(Value::as_str)
                        != Some(trusted.vault_id.as_str())
                    || string(&asset, "meeting_private_assets.owner_scope")?
                        != local_owner_collection_scope(&trusted.vault_id)
                    || string(&asset, "meeting_private_assets.purpose")? != "knowledge_document"
                    || string(&asset, "meeting_private_assets.entity_id")? != asset_id
                    || string(&asset, "meeting_private_assets.key_ref")? != key_ref.as_str()
                    || string(&asset, "meeting_private_assets.state")? != "active"
                {
                    return Err("MEETING_KNOWLEDGE_ASSET_NOT_READABLE".to_string());
                }
                let asset_revision = integer(&asset, "meeting_private_assets.entity_revision")?;
                if asset_revision < 1 {
                    return Err("MEETING_KNOWLEDGE_ASSET_VERSION_INVALID".to_string());
                }
                let (asset_path, expected_custody_ref) =
                    knowledge_asset_path(&session.data_root, &asset_id)?;
                if string(&asset, "meeting_private_assets.custody_ref")? != expected_custody_ref {
                    return Err("MEETING_KNOWLEDGE_ASSET_PATH_INVALID".to_string());
                }
                let encoded = read_bounded_private_asset(
                    &asset_path,
                    crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
                    "MEETING_KNOWLEDGE_ASSET_SIZE_INVALID",
                )?;
                let payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
                    serde_json::from_slice(encoded.as_slice())
                        .map_err(|_| "MEETING_KNOWLEDGE_ASSET_INVALID".to_string())?;
                if payload.key_ref != key_ref
                    || payload.ciphertext_sha256
                        != string(&asset, "meeting_private_assets.ciphertext_sha256")?
                {
                    return Err("MEETING_KNOWLEDGE_ASSET_HASH_MISMATCH".to_string());
                }
                let context = crate::meeting_knowledge::PrivateAssetContext {
                    vault_id: trusted.vault_id.clone(),
                    entity_id: asset_id.clone(),
                    version: asset_revision as u64,
                    purpose: crate::meeting_knowledge::PrivateAssetPurpose::KnowledgeDocument,
                };
                let locator_value = chunk_row
                    .get("knowledge_chunks.locator_json")
                    .cloned()
                    .ok_or_else(|| "MEETING_KNOWLEDGE_LOCATOR_INVALID".to_string())?;
                let locator: crate::meeting_knowledge::CitationLocator =
                    serde_json::from_value(locator_value)
                        .map_err(|_| "MEETING_KNOWLEDGE_LOCATOR_INVALID".to_string())?;
                chunks.push(crate::meeting_knowledge::StoredKnowledgeChunk {
                    id: asset_id,
                    collection_id: collection_id.clone(),
                    document_id: document_id.clone(),
                    document_version_id: version_id.to_string(),
                    document_version_number: integer(
                        &version,
                        "knowledge_document_versions.version_no",
                    )? as u64,
                    source_version: string(&version, "knowledge_document_versions.version_label")?,
                    content_sha256: string(&chunk_row, "knowledge_chunks.content_hash")?,
                    acl_revision: integer(&document, "knowledge_documents.acl_revision")? as u64,
                    key_ref: key_ref.clone(),
                    private_context: context,
                    locator,
                    payload,
                });
            }
        }
    }
    drop(key);
    drop(_owner_fence);
    let boundary = NativeKnowledgeReadBoundary {
        storage,
        session,
        project_id,
        meeting_session_id: &meeting_session_id,
        selected: persisted,
    };
    let mut knowledge_query = crate::meeting_knowledge::KnowledgeQuery::local(
        meeting_session_id.clone(),
        query_text.to_string(),
        selected,
    );
    knowledge_query.deadline = deadline;
    crate::meeting_knowledge::search_selected_knowledge(&boundary, &chunks, &knowledge_query)
}

fn validate_meeting_agent_request_id(request_id: &str) -> Result<(), String> {
    if request_id.is_empty()
        || request_id.len() > 128
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("MEETING_AGENT_REQUEST_ID_INVALID".to_string());
    }
    Ok(())
}

fn current_meeting_agent_grant(
    storage: &Storage,
    meeting_session_id: &str,
    owner_scope: &str,
) -> Result<Option<Value>, String> {
    let mut rows = query_all(
        storage,
        "meeting_agent_grants",
        &[
            "id",
            "owner_scope",
            "mode",
            "capabilities_json",
            "collection_ids_json",
            "destination_policy_json",
            "policy_version",
            "state",
            "expected_revision",
            "granted_at",
            "expires_at",
            "revoked_at",
            "created_at",
        ],
        vec![eq(
            "meeting_agent_grants",
            "meeting_session_id",
            json!(meeting_session_id),
        )],
    )?;
    rows.retain(|row| {
        row.get("meeting_agent_grants.owner_scope")
            .and_then(Value::as_str)
            == Some(owner_scope)
    });
    rows.sort_by_key(|row| {
        integer(row, "meeting_agent_grants.expected_revision").unwrap_or_default()
    });
    Ok(rows.pop())
}

pub(crate) fn meeting_agent_policy_snapshot(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
    owner_scope: Option<&str>,
) -> Result<Option<Value>, String> {
    let mut sessions = query_all(
        storage,
        "meeting_sessions",
        &["id", "session_generation"],
        vec![
            eq("meeting_sessions", "project_id", json!(project_id)),
            eq("meeting_sessions", "recording_id", json!(recording_id)),
        ],
    )?;
    sessions
        .sort_by_key(|row| integer(row, "meeting_sessions.session_generation").unwrap_or_default());
    let Some(meeting) = sessions.pop() else {
        return Ok(None);
    };
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    match owner_scope {
        Some(owner_scope) => current_meeting_agent_grant(storage, &meeting_session_id, owner_scope),
        None => {
            let mut rows = query_all(
                storage,
                "meeting_agent_grants",
                &[
                    "id",
                    "owner_scope",
                    "mode",
                    "capabilities_json",
                    "collection_ids_json",
                    "destination_policy_json",
                    "policy_version",
                    "state",
                    "expected_revision",
                    "granted_at",
                    "expires_at",
                    "revoked_at",
                    "created_at",
                ],
                vec![eq(
                    "meeting_agent_grants",
                    "meeting_session_id",
                    json!(meeting_session_id),
                )],
            )?;
            rows.sort_by_key(|row| {
                integer(row, "meeting_agent_grants.expected_revision").unwrap_or_default()
            });
            Ok(rows.pop())
        }
    }
}

pub(crate) fn commit_meeting_agent_policy(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    request_id: &str,
    expected_revision: u64,
    mode: &str,
    state: &str,
    allowed_topics: &[String],
    expires_at: Option<&str>,
) -> Result<(u64, String, String, Option<String>), String> {
    validate_meeting_agent_request_id(request_id)?;
    if !matches!(mode, "off" | "observe" | "draft")
        || !matches!(state, "active" | "paused" | "stopped" | "revoked")
        || allowed_topics.len() > 32
    {
        return Err("MEETING_AGENT_POLICY_INVALID".to_string());
    }
    let mut unique_topics = std::collections::BTreeSet::new();
    for topic in allowed_topics {
        if topic.is_empty()
            || topic.chars().count() > 64
            || topic.chars().any(char::is_control)
            || !unique_topics.insert(topic)
            || crate::meeting_agent::topic_ref_hash(topic).is_err()
        {
            return Err("MEETING_AGENT_TOPIC_INVALID".to_string());
        }
    }
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    validate_project_exists(storage, project_id)?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let existing = current_meeting_agent_grant(storage, &meeting_session_id, &owner_scope)?;
    let current_revision = existing
        .as_ref()
        .map(|row| integer(row, "meeting_agent_grants.expected_revision"))
        .transpose()?
        .unwrap_or(0)
        .max(0) as u64;
    if current_revision != expected_revision {
        return Err("STALE_MEETING_AGENT_REVISION".to_string());
    }
    let context = meeting_context_row(storage, &meeting_session_id)?;
    if let Some(context) = context.as_ref() {
        if string(context, "meeting_session_contexts.owner_scope")? != owner_scope
            || context
                .get("meeting_session_contexts.vault_id")
                .and_then(Value::as_str)
                != Some(trusted.vault_id.as_str())
        {
            return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
        }
    }
    let collection_ids = selected_collections_from_context(context.as_ref())?;
    let now = chrono::Utc::now();
    let expiry = if matches!(state, "active" | "paused") && mode != "off" {
        if let Some(expires_at) = expires_at {
            let parsed = chrono::DateTime::parse_from_rfc3339(expires_at)
                .map_err(|_| "MEETING_AGENT_EXPIRY_INVALID".to_string())?
                .with_timezone(&chrono::Utc);
            if parsed <= now || parsed > now + chrono::Duration::hours(4) {
                return Err("MEETING_AGENT_EXPIRY_OUT_OF_RANGE".to_string());
            }
            Some(parsed.to_rfc3339())
        } else {
            Some((now + chrono::Duration::hours(4)).to_rfc3339())
        }
    } else {
        None
    };
    let next_revision = current_revision.saturating_add(1);
    let grant_id = existing
        .as_ref()
        .map(|row| string(row, "meeting_agent_grants.id"))
        .transpose()?
        .unwrap_or_else(|| format!("agent-grant-{}", Uuid::new_v4()));
    let timestamp = now.to_rfc3339();
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-agent-policy::{request_id}"),
        &timestamp,
    )?;
    let granted_at = existing
        .as_ref()
        .and_then(|row| row.get("meeting_agent_grants.granted_at"))
        .and_then(Value::as_str)
        .unwrap_or(&timestamp);
    let grant_row = json!({
        "id": grant_id,
        "project_id": project_id,
        "meeting_session_id": meeting_session_id,
        "owner_scope": owner_scope,
        "mode": mode,
        "capabilities_json": {"localAgent": true, "transcriptRead": true, "knowledgeRead": true, "externalDispatch": false, "allowedTopics": allowed_topics},
        "collection_ids_json": collection_ids,
        "destination_policy_json": {"externalDispatch": false, "approvalScope": "local_preview_only"},
        "policy_version": "meeting-local/1",
        "state": state,
        "expected_revision": next_revision,
        "granted_at": granted_at,
        "expires_at": expiry,
        "revoked_at": matches!(state, "revoked" | "stopped").then_some(timestamp.clone()),
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": existing.as_ref().and_then(|row| row.get("meeting_agent_grants.created_at")).and_then(Value::as_str).unwrap_or(&timestamp),
    });
    let run_id = format!("agent-run-{request_id}");
    let run_row = json!({
        "id": run_id,
        "project_id": project_id,
        "recording_id": recording_id,
        "meeting_session_id": meeting_session_id,
        "grant_id": grant_id,
        "trigger_id": request_id,
        "transcript_cursor": -1,
        "transcript_revision_set_json": [],
        "evidence_ids_json": [],
        "policy_version": "meeting-local/1",
        "state": "mode_changed",
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
    });
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![
            upsert("meeting_agent_grants", grant_row),
            upsert("meeting_agent_runs", run_row),
        ],
    )?;
    Ok((next_revision, grant_id, meeting_session_id, expiry))
}

fn require_current_meeting_transcript_cursor(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
    expected_cursor: i64,
) -> Result<(), String> {
    if expected_cursor < -1 {
        return Err("MEETING_TRANSCRIPT_CURSOR_STALE".to_string());
    }
    let snapshot = meeting_transcript_snapshot(storage, project_id, recording_id)?;
    if snapshot.high_watermark != expected_cursor {
        return Err("MEETING_TRANSCRIPT_CURSOR_STALE".to_string());
    }
    Ok(())
}

pub(crate) struct MeetingAgentModelProvenance {
    pub id: String,
    pub model_name: String,
    pub input_hash: String,
    pub output_hash: String,
    pub provider_config_hash: String,
}

pub(crate) fn ensure_meeting_agent_request_unused(
    storage: &Storage,
    request_id: &str,
) -> Result<(), String> {
    validate_meeting_agent_request_id(request_id)?;
    if !query(
        storage,
        "meeting_agent_runs",
        &["id"],
        vec![eq("meeting_agent_runs", "trigger_id", json!(request_id))],
        1,
    )?
    .is_empty()
    {
        return Err("MEETING_AGENT_REQUEST_DUPLICATE".to_string());
    }
    Ok(())
}

fn validate_model_reference_set(
    model_input_refs: Option<&Value>,
    model: Option<&MeetingAgentModelProvenance>,
    cited_refs: &Value,
) -> Result<(), String> {
    if model.is_some() != model_input_refs.is_some() {
        return Err("MEETING_AGENT_MODEL_INPUT_INVALID".to_string());
    }
    if let Some(input_refs) = model_input_refs {
        let all = input_refs
            .as_array()
            .ok_or_else(|| "MEETING_AGENT_MODEL_INPUT_INVALID".to_string())?;
        let cited = cited_refs
            .as_array()
            .ok_or_else(|| "MEETING_AGENT_MODEL_REFS_INVALID".to_string())?;
        if cited.is_empty() || cited.iter().any(|item| !all.contains(item)) {
            return Err("MEETING_AGENT_MODEL_REFS_INVALID".to_string());
        }
    }
    Ok(())
}

pub(crate) fn persist_private_meeting_agent_draft(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    expected_agent_revision: u64,
    expected_collection_ids: &[String],
    request_id: &str,
    draft_id: &str,
    draft_text: &str,
    draft_expires_at: &str,
    transcript_cursor: i64,
    transcript_revisions: Value,
    evidence_refs: Value,
    model_input_refs: Option<&Value>,
    model: Option<&MeetingAgentModelProvenance>,
) -> Result<String, String> {
    validate_meeting_agent_request_id(request_id)?;
    validate_model_reference_set(model_input_refs, model, &evidence_refs)?;
    if draft_text.is_empty() || draft_text.chars().count() > 12_000 {
        return Err("MEETING_AGENT_DRAFT_SIZE_INVALID".to_string());
    }
    let draft_expiry = chrono::DateTime::parse_from_rfc3339(draft_expires_at)
        .map_err(|_| "MEETING_AGENT_EXPIRY_INVALID".to_string())?
        .with_timezone(&chrono::Utc);
    if draft_expiry <= chrono::Utc::now() {
        return Err("MEETING_AGENT_EXPIRY_INVALID".to_string());
    }
    let trusted = capture_native_identity_context(storage, session)?.context;
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-agent-draft::{request_id}"),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    require_current_meeting_transcript_cursor(
        storage,
        project_id,
        recording_id,
        transcript_cursor,
    )?;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let context = meeting_context_row(storage, &meeting_session_id)?
        .ok_or_else(|| "MEETING_KNOWLEDGE_SELECTION_REQUIRED".to_string())?;
    if string(&context, "meeting_session_contexts.owner_scope")? != owner_scope
        || context
            .get("meeting_session_contexts.vault_id")
            .and_then(Value::as_str)
            != Some(trusted.vault_id.as_str())
        || string(&context, "meeting_session_contexts.project_id")? != project_id
        || string(&context, "meeting_session_contexts.recording_id")? != recording_id
    {
        return Err("MEETING_CONTEXT_OWNER_MISMATCH".to_string());
    }
    let current_selected = selected_collections_from_context(Some(&context))?;
    let expected_selected = expected_collection_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if expected_selected.is_empty()
        || expected_selected.len() > 64
        || expected_selected.len() != expected_collection_ids.len()
        || current_selected.len() != expected_collection_ids.len()
        || current_selected
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != expected_selected
    {
        return Err("MEETING_KNOWLEDGE_SELECTION_STALE".to_string());
    }
    let grant = current_meeting_agent_grant(storage, &meeting_session_id, &owner_scope)?
        .ok_or_else(|| "MEETING_AGENT_GRANT_UNAVAILABLE".to_string())?;
    let grant_expiry_text = grant
        .get("meeting_agent_grants.expires_at")
        .and_then(Value::as_str)
        .ok_or_else(|| "MEETING_AGENT_EXPIRY_INVALID".to_string())?;
    let grant_expiry = chrono::DateTime::parse_from_rfc3339(grant_expiry_text)
        .map_err(|_| "MEETING_AGENT_EXPIRY_INVALID".to_string())?
        .with_timezone(&chrono::Utc);
    if string(&grant, "meeting_agent_grants.mode")? != "draft"
        || string(&grant, "meeting_agent_grants.state")? != "active"
        || integer(&grant, "meeting_agent_grants.expected_revision")? as u64
            != expected_agent_revision
        || grant_expiry <= chrono::Utc::now()
    {
        return Err("MEETING_AGENT_DRAFT_NOT_AUTHORIZED".to_string());
    }
    validate_current_meeting_agent_evidence(
        storage,
        project_id,
        &trusted.vault_id,
        &owner_scope,
        &expected_selected,
        model_input_refs.unwrap_or(&evidence_refs),
        &session.data_root,
    )?;
    if let Some(model) = model {
        if crate::meeting_agent_model::provider_config_hash(storage)? != model.provider_config_hash
        {
            return Err("MEETING_AGENT_MODEL_CONFIG_STALE".to_string());
        }
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let asset_id = format!("agent-draft-{draft_id}");
    let context = crate::meeting_knowledge::PrivateAssetContext {
        vault_id: trusted.vault_id.clone(),
        entity_id: draft_id.to_string(),
        version: 1,
        purpose: crate::meeting_knowledge::PrivateAssetPurpose::AgentDraft,
    };
    let (custody_ref, ciphertext_sha256, ciphertext_bytes, mut retention) =
        write_encrypted_knowledge_asset(
            &session.data_root,
            &asset_id,
            &key,
            &context,
            draft_text.as_bytes(),
        )?;
    retention["expiresAt"] = json!(draft_expiry.to_rfc3339());
    let timestamp = attempt.committed_at.clone();
    let asset_row = json!({
        "id": asset_id,
        "project_id": project_id,
        "vault_id": trusted.vault_id,
        "owner_scope": owner_scope,
        "purpose": "agent_draft",
        "entity_id": draft_id,
        "entity_revision": 1,
        "custody_ref": custody_ref,
        "key_ref": key_ref.as_str(),
        "ciphertext_sha256": ciphertext_sha256,
        "ciphertext_bytes": ciphertext_bytes,
        "state": "active",
        "retention_json": retention,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": timestamp,
    });
    let run_id = format!("agent-run-{request_id}");
    let run_row = json!({
        "id": run_id,
        "project_id": project_id,
        "recording_id": recording_id,
        "meeting_session_id": meeting_session_id,
        "grant_id": grant.get("meeting_agent_grants.id"),
        "model_run_id": model.map(|model| &model.id),
        "trigger_id": request_id,
        "transcript_cursor": transcript_cursor,
        "transcript_revision_set_json": transcript_revisions,
        "evidence_ids_json": {"privateAssetId": asset_id, "evidence": evidence_refs, "modelInputEvidence": model_input_refs},
        "policy_version": "meeting-local/1",
        "state": "draft_created",
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
    });
    let mut mutations = vec![upsert("meeting_private_assets", asset_row)];
    if let Some(model) = model {
        mutations.push(upsert("model_runs", json!({
            "id": model.id,
            "recording_id": recording_id,
            "provider_id": "ollama-summary-intent",
            "model_name": model.model_name,
            "task_kind": "meeting_agent_proposal",
            "runtime_location": "local",
            "input_ref": format!("sha256:{}", model.input_hash),
            "output_ref": format!("sha256:{}", model.output_hash),
            "parameters_json": {"inputSha256": model.input_hash, "outputSha256": model.output_hash, "maxOutputTokens": 768},
            "created_at": timestamp,
        })));
    }
    mutations.push(upsert("meeting_agent_runs", run_row));
    commit_native_owner_mutations(storage, session, &attempt, mutations)?;
    Ok(asset_id)
}

fn validate_current_meeting_agent_evidence(
    storage: &Storage,
    project_id: &str,
    vault_id: &str,
    owner_scope: &str,
    selected: &std::collections::BTreeSet<String>,
    evidence_refs: &Value,
    data_root: &Path,
) -> Result<(), String> {
    let evidence = evidence_refs
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 8)
        .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    for item in evidence {
        let collection_id = item
            .get("collectionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let document_id = item
            .get("documentId")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let version_id = item
            .get("versionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let content_hash = item
            .get("contentSha256")
            .and_then(Value::as_str)
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let locator_data = item
            .get("locatorData")
            .filter(|value| !value.is_null())
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let locator = item
            .get("locator")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        if !item
            .get("readGrantId")
            .and_then(Value::as_str)
            .is_some_and(|value| {
                value.len() <= 64
                    && value.strip_prefix("read:").is_some_and(|suffix| {
                        !suffix.is_empty()
                            && suffix.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
                            })
                    })
            })
        {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }
        let evidence_key = (collection_id, document_id, version_id, locator);
        if !selected.contains(collection_id) || !seen.insert(evidence_key) {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }

        let document = query(
            storage,
            "knowledge_documents",
            &[
                "collection_id",
                "current_version_id",
                "acl_revision",
                "status",
            ],
            vec![eq("knowledge_documents", "id", json!(document_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        if string(&document, "knowledge_documents.collection_id")? != collection_id
            || document
                .get("knowledge_documents.current_version_id")
                .and_then(Value::as_str)
                != Some(version_id)
            || item.get("aclRevision").and_then(Value::as_i64)
                != document
                    .get("knowledge_documents.acl_revision")
                    .and_then(Value::as_i64)
            || string(&document, "knowledge_documents.status")? != "active"
        {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }

        let collection = query(
            storage,
            "knowledge_collections",
            &["project_id", "owner_scope", "status"],
            vec![eq("knowledge_collections", "id", json!(collection_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let collection_project = collection
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&collection, "knowledge_collections.owner_scope")? != owner_scope
            || string(&collection, "knowledge_collections.status")? != "active"
            || collection_project.is_some_and(|bound| bound != project_id)
        {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }

        let version = query(
            storage,
            "knowledge_document_versions",
            &["document_id", "version_no", "version_label", "state"],
            vec![eq("knowledge_document_versions", "id", json!(version_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        if string(&version, "knowledge_document_versions.document_id")? != document_id
            || string(&version, "knowledge_document_versions.state")? != "active"
            || item.get("documentVersionNumber").and_then(Value::as_i64)
                != version
                    .get("knowledge_document_versions.version_no")
                    .and_then(Value::as_i64)
            || item.get("sourceVersion").and_then(Value::as_str)
                != version
                    .get("knowledge_document_versions.version_label")
                    .and_then(Value::as_str)
        {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }

        let chunks = query(
            storage,
            "knowledge_chunks",
            &["content_hash", "locator_json", "text_ref"],
            vec![eq("knowledge_chunks", "version_id", json!(version_id))],
            1000,
        )?;
        let chunk = chunks
            .iter()
            .find(|row| {
                row.get("knowledge_chunks.content_hash")
                    .and_then(Value::as_str)
                    == Some(content_hash)
                    && row.get("knowledge_chunks.locator_json") == Some(locator_data)
            })
            .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let asset_id = string(chunk, "knowledge_chunks.text_ref")?;
        let asset = query(
            storage,
            "meeting_private_assets",
            &[
                "project_id",
                "vault_id",
                "owner_scope",
                "purpose",
                "entity_id",
                "custody_ref",
                "key_ref",
                "state",
            ],
            vec![eq("meeting_private_assets", "id", json!(asset_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_AGENT_EVIDENCE_STALE".to_string())?;
        let (_, expected_custody_ref) = knowledge_asset_path(data_root, &asset_id)?;
        if string(&asset, "meeting_private_assets.project_id")? != project_id
            || asset
                .get("meeting_private_assets.vault_id")
                .and_then(Value::as_str)
                != Some(vault_id)
            || string(&asset, "meeting_private_assets.owner_scope")? != owner_scope
            || string(&asset, "meeting_private_assets.purpose")? != "knowledge_document"
            || string(&asset, "meeting_private_assets.entity_id")? != asset_id
            || string(&asset, "meeting_private_assets.custody_ref")? != expected_custody_ref
            || string(&asset, "meeting_private_assets.key_ref")? != format!("knowledge:{vault_id}")
            || string(&asset, "meeting_private_assets.state")? != "active"
        {
            return Err("MEETING_AGENT_EVIDENCE_STALE".to_string());
        }
    }
    Ok(())
}

pub(crate) fn commit_meeting_agent_run_event(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    request_id: &str,
    state: &str,
    transcript_cursor: i64,
    evidence_refs: Value,
) -> Result<(), String> {
    validate_meeting_agent_request_id(request_id)?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let attempt = begin_meeting_commit(
        storage,
        &format!("meeting-agent-event::{request_id}"),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let grant = current_meeting_agent_grant(storage, &meeting_session_id, &owner_scope)?;
    let row = json!({
        "id": format!("agent-run-{request_id}"),
        "project_id": project_id,
        "recording_id": recording_id,
        "meeting_session_id": meeting_session_id,
        "grant_id": grant.as_ref().and_then(|row| row.get("meeting_agent_grants.id")),
        "trigger_id": request_id,
        "transcript_cursor": transcript_cursor,
        "transcript_revision_set_json": [],
        "evidence_ids_json": evidence_refs,
        "policy_version": "meeting-local/1",
        "state": state,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": attempt.committed_at,
    });
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![upsert("meeting_agent_runs", row)],
    )?;
    Ok(())
}

pub(crate) fn list_meeting_agent_runs(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    limit: u32,
) -> Result<Vec<meeting_intelligence_schema::MeetingAgentHistoryEntry>, String> {
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let grants = query_all(
        storage,
        "meeting_agent_grants",
        &["id", "owner_scope"],
        vec![eq(
            "meeting_agent_grants",
            "meeting_session_id",
            json!(&meeting_session_id),
        )],
    )?
    .into_iter()
    .filter(|row| {
        row.get("meeting_agent_grants.owner_scope")
            .and_then(Value::as_str)
            == Some(&owner_scope)
    })
    .filter_map(|row| {
        row.get("meeting_agent_grants.id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
    .collect::<std::collections::BTreeSet<_>>();
    let mut rows = query_all(
        storage,
        "meeting_agent_runs",
        &[
            "id",
            "grant_id",
            "transcript_cursor",
            "evidence_ids_json",
            "state",
            "created_at",
        ],
        vec![
            eq(
                "meeting_agent_runs",
                "meeting_session_id",
                json!(&meeting_session_id),
            ),
            eq("meeting_agent_runs", "project_id", json!(project_id)),
            eq("meeting_agent_runs", "recording_id", json!(recording_id)),
        ],
    )?;
    rows.retain(|row| {
        row.get("meeting_agent_runs.grant_id")
            .and_then(Value::as_str)
            .is_some_and(|grant_id| grants.contains(grant_id))
    });
    rows.sort_by(|left, right| {
        right
            .get("meeting_agent_runs.created_at")
            .and_then(Value::as_str)
            .cmp(
                &left
                    .get("meeting_agent_runs.created_at")
                    .and_then(Value::as_str),
            )
    });
    rows.truncate(limit.clamp(1, 100) as usize);
    rows.into_iter()
        .map(|row| {
            let state = string(&row, "meeting_agent_runs.state")?;
            let kind = match state.as_str() {
                "mode_changed" => "mode_changed",
                "draft_created" => "draft_created",
                "draft_blocked" => "draft_blocked",
                "preview_created" => "preview_created",
                "approval_recorded" => "approval_recorded",
                "revoked" => "revoked",
                _ => "draft_blocked",
            };
            let evidence = row
                .get("meeting_agent_runs.evidence_ids_json")
                .cloned()
                .unwrap_or(Value::Null);
            Ok(meeting_intelligence_schema::MeetingAgentHistoryEntry {
                id: string(&row, "meeting_agent_runs.id")?,
                kind: kind.to_string(),
                state,
                created_at: string(&row, "meeting_agent_runs.created_at")?,
                transcript_cursor: row
                    .get("meeting_agent_runs.transcript_cursor")
                    .and_then(Value::as_i64)
                    .filter(|cursor| *cursor >= 0),
                evidence_count: evidence
                    .get("evidence")
                    .and_then(Value::as_array)
                    .map(|items| items.len() as u32)
                    .unwrap_or(0),
            })
        })
        .collect()
}

pub(crate) fn persist_local_meeting_delivery_preview(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    request_id: &str,
    expected_revision: u64,
    draft_id: &str,
    draft_revision: u64,
    draft_expires_at: &str,
    payload_bytes: &[u8],
) -> Result<meeting_intelligence_schema::MeetingAgentDeliveryPreview, String> {
    validate_meeting_agent_request_id(request_id)?;
    if draft_id.is_empty()
        || draft_id.len() > 128
        || !draft_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || draft_revision == 0
        || payload_bytes.is_empty()
        || payload_bytes.len() > 100_000
    {
        return Err("MEETING_DELIVERY_PREVIEW_INVALID".to_string());
    }
    let expires_at = chrono::DateTime::parse_from_rfc3339(draft_expires_at)
        .map_err(|_| "MEETING_AGENT_DRAFT_STALE".to_string())?
        .with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    if expires_at <= now {
        return Err("MEETING_AGENT_DRAFT_STALE".to_string());
    }
    let transaction_suffix = crate::backup_payload::sha256_hex(request_id.as_bytes());
    let attempt = begin_meeting_commit(
        storage,
        &format!("mi-delivery-preview-{}", &transaction_suffix[..40]),
        &now.to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    validate_project_exists(storage, project_id)?;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let grant = current_meeting_agent_grant(storage, &meeting_session_id, &owner_scope)?
        .ok_or_else(|| "MEETING_AGENT_GRANT_UNAVAILABLE".to_string())?;
    if integer(&grant, "meeting_agent_grants.expected_revision")? as u64 != expected_revision
        || string(&grant, "meeting_agent_grants.mode")? != "draft"
        || string(&grant, "meeting_agent_grants.state")? != "active"
        || grant
            .get("meeting_agent_grants.expires_at")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_none_or(|expiry| expiry.with_timezone(&chrono::Utc) <= now)
    {
        return Err("MEETING_DELIVERY_PREVIEW_STALE".to_string());
    }

    let payload: Value = serde_json::from_slice(payload_bytes)
        .map_err(|_| "MEETING_DELIVERY_PAYLOAD_INVALID".to_string())?;
    let payload_text = payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "MEETING_DELIVERY_PAYLOAD_INVALID".to_string())?;
    let citations = payload
        .get("citations")
        .and_then(Value::as_array)
        .ok_or_else(|| "MEETING_DELIVERY_PAYLOAD_INVALID".to_string())?;
    if payload_text.is_empty() || citations.is_empty() || citations.len() > 8 {
        return Err("MEETING_DELIVERY_PAYLOAD_INVALID".to_string());
    }

    let draft_asset_id = format!("agent-draft-{draft_id}");
    let draft_asset = query(
        storage,
        "meeting_private_assets",
        &[
            "id",
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "state",
            "retention_json",
        ],
        vec![eq("meeting_private_assets", "id", json!(&draft_asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_AGENT_DRAFT_STALE".to_string())?;
    if string(&draft_asset, "meeting_private_assets.vault_id")? != trusted.vault_id
        || string(&draft_asset, "meeting_private_assets.owner_scope")? != owner_scope
        || string(&draft_asset, "meeting_private_assets.purpose")? != "agent_draft"
        || string(&draft_asset, "meeting_private_assets.entity_id")? != draft_id
        || integer(&draft_asset, "meeting_private_assets.entity_revision")? as u64 != draft_revision
        || string(&draft_asset, "meeting_private_assets.key_ref")?
            != format!("knowledge:{}", trusted.vault_id)
        || string(&draft_asset, "meeting_private_assets.state")? != "active"
    {
        return Err("MEETING_AGENT_DRAFT_STALE".to_string());
    }
    let stored_expiry = draft_asset
        .get("meeting_private_assets.retention_json")
        .and_then(|value| value.get("expiresAt"))
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(|| "MEETING_AGENT_DRAFT_STALE".to_string())?
        .with_timezone(&chrono::Utc);
    if stored_expiry != expires_at {
        return Err("MEETING_AGENT_DRAFT_STALE".to_string());
    }
    let (_, expected_draft_custody_ref) =
        knowledge_asset_path(&session.data_root, &draft_asset_id)?;
    if string(&draft_asset, "meeting_private_assets.custody_ref")? != expected_draft_custody_ref {
        return Err("MEETING_AGENT_DRAFT_STALE".to_string());
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let (draft_path, _) = knowledge_asset_path(&session.data_root, &draft_asset_id)?;
    let draft_encoded = read_bounded_private_asset(
        &draft_path,
        crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
        "MEETING_AGENT_DRAFT_STALE",
    )?;
    let draft_payload: crate::meeting_knowledge::EncryptedKnowledgePayload =
        serde_json::from_slice(draft_encoded.as_slice())
            .map_err(|_| "MEETING_AGENT_DRAFT_STALE".to_string())?;
    let draft_context = crate::meeting_knowledge::PrivateAssetContext {
        vault_id: trusted.vault_id.clone(),
        entity_id: draft_id.to_string(),
        version: draft_revision,
        purpose: crate::meeting_knowledge::PrivateAssetPurpose::AgentDraft,
    };
    let stored_draft =
        crate::meeting_knowledge::open_private_payload(&key, &draft_context, &draft_payload)?;
    if stored_draft.as_slice() != payload_text.as_bytes() {
        return Err("MEETING_DELIVERY_PAYLOAD_DRAFT_MISMATCH".to_string());
    }

    let runs = query_all(
        storage,
        "meeting_agent_runs",
        &[
            "id",
            "grant_id",
            "model_run_id",
            "evidence_ids_json",
            "state",
            "transcript_cursor",
        ],
        vec![
            eq(
                "meeting_agent_runs",
                "meeting_session_id",
                json!(&meeting_session_id),
            ),
            eq("meeting_agent_runs", "project_id", json!(project_id)),
            eq("meeting_agent_runs", "recording_id", json!(recording_id)),
        ],
    )?;
    let draft_run = runs
        .iter()
        .find(|row| {
            row.get("meeting_agent_runs.state").and_then(Value::as_str) == Some("draft_created")
                && row
                    .get("meeting_agent_runs.evidence_ids_json")
                    .and_then(|value| value.get("privateAssetId"))
                    .and_then(Value::as_str)
                    == Some(draft_asset_id.as_str())
        })
        .ok_or_else(|| "MEETING_AGENT_DRAFT_EVIDENCE_UNAVAILABLE".to_string())?;
    require_current_meeting_transcript_cursor(
        storage,
        project_id,
        recording_id,
        integer(draft_run, "meeting_agent_runs.transcript_cursor")?,
    )?;
    if draft_run
        .get("meeting_agent_runs.grant_id")
        .and_then(Value::as_str)
        != grant.get("meeting_agent_grants.id").and_then(Value::as_str)
    {
        return Err("MEETING_AGENT_DRAFT_STALE".to_string());
    }
    let evidence = draft_run
        .get("meeting_agent_runs.evidence_ids_json")
        .and_then(|value| value.get("evidence"))
        .and_then(Value::as_array)
        .ok_or_else(|| "MEETING_AGENT_DRAFT_EVIDENCE_UNAVAILABLE".to_string())?;
    if evidence.len() != citations.len() || evidence.is_empty() {
        return Err("MEETING_DELIVERY_CITATION_MISMATCH".to_string());
    }
    let context = meeting_context_row(storage, &meeting_session_id)?;
    let selected = selected_collections_from_context(context.as_ref())?
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if draft_run
        .get("meeting_agent_runs.model_run_id")
        .and_then(Value::as_str)
        .is_some()
    {
        let model_inputs = draft_run
            .get("meeting_agent_runs.evidence_ids_json")
            .and_then(|value| value.get("modelInputEvidence"))
            .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        validate_current_meeting_agent_evidence(
            storage,
            project_id,
            &trusted.vault_id,
            &owner_scope,
            &selected,
            model_inputs,
            &session.data_root,
        )
        .map_err(|_| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
    }
    for citation in citations {
        let document_id = citation
            .get("documentId")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_DELIVERY_CITATION_MISMATCH".to_string())?;
        let version_id = citation
            .get("versionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_DELIVERY_CITATION_MISMATCH".to_string())?;
        let locator = citation
            .get("locator")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_DELIVERY_CITATION_MISMATCH".to_string())?;
        let matching_evidence = evidence.iter().any(|item| {
            item.get("documentId").and_then(Value::as_str) == Some(document_id)
                && item.get("versionId").and_then(Value::as_str) == Some(version_id)
                && item.get("locator").and_then(Value::as_str) == Some(locator)
        });
        if !matching_evidence {
            return Err("MEETING_DELIVERY_CITATION_MISMATCH".to_string());
        }
        let doc = query(
            storage,
            "knowledge_documents",
            &[
                "collection_id",
                "current_version_id",
                "acl_revision",
                "status",
            ],
            vec![eq("knowledge_documents", "id", json!(document_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        let collection_id = string(&doc, "knowledge_documents.collection_id")?;
        if !selected.contains(&collection_id)
            || string(&doc, "knowledge_documents.current_version_id")? != version_id
            || string(&doc, "knowledge_documents.status")? != "active"
        {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        }
        let evidence_item = evidence
            .iter()
            .find(|item| {
                item.get("documentId").and_then(Value::as_str) == Some(document_id)
                    && item.get("versionId").and_then(Value::as_str) == Some(version_id)
                    && item.get("locator").and_then(Value::as_str) == Some(locator)
            })
            .ok_or_else(|| "MEETING_DELIVERY_CITATION_MISMATCH".to_string())?;
        if evidence_item.get("aclRevision").and_then(Value::as_i64)
            != doc
                .get("knowledge_documents.acl_revision")
                .and_then(Value::as_i64)
        {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        }
        let collection = query(
            storage,
            "knowledge_collections",
            &["owner_scope", "project_id", "status"],
            vec![eq("knowledge_collections", "id", json!(&collection_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        let collection_project = collection
            .get("knowledge_collections.project_id")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str);
        if string(&collection, "knowledge_collections.owner_scope")? != owner_scope
            || string(&collection, "knowledge_collections.status")? != "active"
            || collection_project.is_some_and(|bound| bound != project_id)
        {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        }
        let version = query(
            storage,
            "knowledge_document_versions",
            &["version_no", "version_label", "state"],
            vec![eq("knowledge_document_versions", "id", json!(version_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        if string(&version, "knowledge_document_versions.state")? != "active"
            || evidence_item
                .get("documentVersionNumber")
                .and_then(Value::as_i64)
                != version
                    .get("knowledge_document_versions.version_no")
                    .and_then(Value::as_i64)
            || evidence_item.get("sourceVersion").and_then(Value::as_str)
                != version
                    .get("knowledge_document_versions.version_label")
                    .and_then(Value::as_str)
        {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        }
        let locator_data = evidence_item
            .get("locatorData")
            .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        let content_hash = evidence_item
            .get("contentSha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        let chunks = query(
            storage,
            "knowledge_chunks",
            &["content_hash", "locator_json", "text_ref"],
            vec![eq("knowledge_chunks", "version_id", json!(version_id))],
            1000,
        )?;
        let current_chunk = chunks.iter().find(|row| {
            row.get("knowledge_chunks.content_hash")
                .and_then(Value::as_str)
                == Some(content_hash)
                && row.get("knowledge_chunks.locator_json") == Some(locator_data)
        });
        let Some(current_chunk) = current_chunk else {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        };
        let chunk_asset_id = string(current_chunk, "knowledge_chunks.text_ref")?;
        let chunk_asset = query(
            storage,
            "meeting_private_assets",
            &["project_id", "vault_id", "owner_scope", "purpose", "state"],
            vec![eq("meeting_private_assets", "id", json!(chunk_asset_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "MEETING_DELIVERY_EVIDENCE_STALE".to_string())?;
        if string(&chunk_asset, "meeting_private_assets.project_id")? != project_id
            || chunk_asset
                .get("meeting_private_assets.vault_id")
                .and_then(Value::as_str)
                != Some(trusted.vault_id.as_str())
            || string(&chunk_asset, "meeting_private_assets.owner_scope")? != owner_scope
            || string(&chunk_asset, "meeting_private_assets.purpose")? != "knowledge_document"
            || string(&chunk_asset, "meeting_private_assets.state")? != "active"
        {
            return Err("MEETING_DELIVERY_EVIDENCE_STALE".to_string());
        }
    }

    let request_hash = crate::backup_payload::sha256_hex(request_id.as_bytes());
    let intent_id = format!("delivery-{}", &request_hash[..32]);
    if let Some(existing) = query(
        storage,
        "meeting_delivery_outbox",
        &["id", "destination_id", "payload_hash", "state"],
        vec![eq("meeting_delivery_outbox", "id", json!(&intent_id))],
        1,
    )?
    .into_iter()
    .next()
    {
        let payload_hash = crate::backup_payload::sha256_hex(payload_bytes);
        if existing
            .get("meeting_delivery_outbox.payload_hash")
            .and_then(Value::as_str)
            != Some(payload_hash.as_str())
        {
            return Err("MEETING_DELIVERY_IDEMPOTENCY_CONFLICT".to_string());
        }
        return Ok(meeting_delivery_preview_from_row(&existing));
    }

    let selected_binding = crate::backup_payload::sha256_hex(
        serde_json::to_vec(&selected)
            .map_err(|_| "MEETING_DELIVERY_EVIDENCE_UNAVAILABLE".to_string())?
            .as_slice(),
    );
    let payload_hash = crate::backup_payload::sha256_hex(payload_bytes);
    let asset_id = format!("delivery-preview-{}", &request_hash[..24]);
    let context = crate::meeting_knowledge::PrivateAssetContext {
        vault_id: trusted.vault_id.clone(),
        entity_id: intent_id.clone(),
        version: draft_revision,
        purpose: crate::meeting_knowledge::PrivateAssetPurpose::DeliveryPreview,
    };
    let (custody_ref, ciphertext_sha256, ciphertext_bytes, mut retention) =
        write_encrypted_knowledge_asset(
            &session.data_root,
            &asset_id,
            &key,
            &context,
            payload_bytes,
        )?;
    retention["expires_at"] = json!(expires_at.to_rfc3339());
    let timestamp = attempt.committed_at.clone();
    let grant_id = string(&grant, "meeting_agent_grants.id")?;
    let run_id = format!("agent-run-delivery-preview-{}", &request_hash[..24]);
    let destination_id = format!("local-destination-{}", &request_hash[..24]);
    let asset_row = json!({
        "id": asset_id,
        "project_id": project_id,
        "vault_id": trusted.vault_id,
        "owner_scope": owner_scope,
        "purpose": "delivery_preview",
        "entity_id": intent_id,
        "entity_revision": draft_revision,
        "custody_ref": custody_ref,
        "key_ref": key_ref.as_str(),
        "ciphertext_sha256": ciphertext_sha256,
        "ciphertext_bytes": ciphertext_bytes,
        "state": "active",
        "retention_json": retention,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": timestamp,
    });
    let destination_row = json!({
        "id": destination_id,
        "agent_run_id": run_id,
        "provider_account_ref_ciphertext_ref": "local-preview-only",
        "provider_account_ref_sha256": selected_binding,
        "occurrence_key": crate::backup_payload::sha256_hex(meeting_session_id.as_bytes()),
        "channel_type": "local_preview_only",
        "channel_id_ciphertext_ref": "local-preview-only",
        "channel_id_sha256": crate::backup_payload::sha256_hex(intent_id.as_bytes()),
        "thread_id_ciphertext_ref": null,
        "audience_policy_revision": expected_revision,
        "approval_snapshot_json": {
            "version": 1,
            "scope": "local_preview_only",
            "payloadHash": payload_hash,
            "draftId": draft_id,
            "draftRevision": draft_revision,
            "expiresAt": expires_at.to_rfc3339(),
            "state": "awaiting_approval"
        },
        "state": "awaiting_approval",
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
    });
    let outbox_row = json!({
        "id": intent_id,
        "agent_run_id": run_id,
        "destination_id": destination_id,
        "artifact_revision": draft_revision,
        "evidence_bundle_id": null,
        "grant_id": grant_id,
        "audience_policy_revision": expected_revision,
        "payload_ciphertext_ref": asset_id,
        "payload_hash": payload_hash,
        "idempotency_key": request_hash,
        "attempt_no": 0,
        "lease_owner": null,
        "lease_expires_at": null,
        "state": "awaiting_approval",
        "receipt_ref": null,
        "external_message_id_ref": null,
        "external_upload_id_ref": null,
        "last_error_code": null,
        "last_error_message": null,
        "unknown_at": null,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": timestamp,
    });
    let run_row = json!({
        "id": run_id,
        "project_id": project_id,
        "recording_id": recording_id,
        "meeting_session_id": meeting_session_id,
        "grant_id": grant_id,
        "trigger_id": request_id,
        "transcript_cursor": draft_run
            .get("meeting_agent_runs.transcript_cursor")
            .cloned()
            .unwrap_or(json!(-1)),
        "transcript_revision_set_json": draft_run
            .get("meeting_agent_runs.transcript_revision_set_json")
            .cloned()
            .unwrap_or(json!([])),
        "evidence_ids_json": {"deliveryIntentId": intent_id, "draftId": draft_id},
        "policy_version": "meeting-local/1",
        "state": "preview_created",
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
    });
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![
            upsert("meeting_private_assets", asset_row),
            upsert("meeting_agent_runs", run_row),
            upsert("meeting_destinations", destination_row),
            upsert("meeting_delivery_outbox", outbox_row),
        ],
    )?;
    Ok(meeting_intelligence_schema::MeetingAgentDeliveryPreview {
        intent_id,
        payload_hash,
        destination_summary: "ตัวอย่างในเครื่อง · ไม่มีผู้รับหรือช่องทางภายนอก".to_string(),
        state: "awaiting_approval".to_string(),
        approval_scope: "local_preview_only".to_string(),
        external_dispatch_available: false,
    })
}

pub(crate) fn approve_local_meeting_delivery_preview(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    project_id: &str,
    recording_id: &str,
    request_id: &str,
    expected_revision: u64,
    intent_id: &str,
    approved_payload_hash: &str,
) -> Result<meeting_intelligence_schema::MeetingAgentDeliveryPreview, String> {
    validate_meeting_agent_request_id(request_id)?;
    let now = chrono::Utc::now();
    let transaction_suffix = crate::backup_payload::sha256_hex(request_id.as_bytes());
    let attempt = begin_meeting_commit(
        storage,
        &format!("mi-delivery-approve-{}", &transaction_suffix[..40]),
        &now.to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let meeting = meeting_session_for_recording(storage, project_id, recording_id)?;
    let meeting_session_id = string(&meeting, "meeting_sessions.id")?;
    let owner_scope = local_owner_collection_scope(&trusted.vault_id);
    let grant = current_meeting_agent_grant(storage, &meeting_session_id, &owner_scope)?
        .ok_or_else(|| "MEETING_AGENT_GRANT_UNAVAILABLE".to_string())?;
    if integer(&grant, "meeting_agent_grants.expected_revision")? as u64 != expected_revision
        || string(&grant, "meeting_agent_grants.mode")? != "draft"
        || string(&grant, "meeting_agent_grants.state")? != "active"
        || grant
            .get("meeting_agent_grants.expires_at")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_none_or(|expiry| expiry.with_timezone(&chrono::Utc) <= now)
    {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let outbox = query(
        storage,
        "meeting_delivery_outbox",
        &[
            "id",
            "agent_run_id",
            "destination_id",
            "artifact_revision",
            "evidence_bundle_id",
            "grant_id",
            "audience_policy_revision",
            "payload_ciphertext_ref",
            "payload_hash",
            "idempotency_key",
            "attempt_no",
            "lease_owner",
            "lease_expires_at",
            "state",
            "receipt_ref",
            "external_message_id_ref",
            "external_upload_id_ref",
            "last_error_code",
            "last_error_message",
            "unknown_at",
            "contract_version",
            "created_at",
            "updated_at",
        ],
        vec![eq("meeting_delivery_outbox", "id", json!(intent_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_DELIVERY_PREVIEW_UNAVAILABLE".to_string())?;
    if string(&outbox, "meeting_delivery_outbox.payload_hash")? != approved_payload_hash
        || outbox
            .get("meeting_delivery_outbox.grant_id")
            .and_then(Value::as_str)
            != grant.get("meeting_agent_grants.id").and_then(Value::as_str)
    {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let run_id = string(&outbox, "meeting_delivery_outbox.agent_run_id")?;
    let approval_run = query(
        storage,
        "meeting_agent_runs",
        &[
            "project_id",
            "recording_id",
            "meeting_session_id",
            "trigger_id",
            "transcript_cursor",
        ],
        vec![eq("meeting_agent_runs", "id", json!(&run_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_DELIVERY_RUN_UNAVAILABLE".to_string())?;
    if string(&approval_run, "meeting_agent_runs.project_id")? != project_id
        || string(&approval_run, "meeting_agent_runs.recording_id")? != recording_id
        || string(&approval_run, "meeting_agent_runs.meeting_session_id")? != meeting_session_id
    {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    require_current_meeting_transcript_cursor(
        storage,
        project_id,
        recording_id,
        integer(&approval_run, "meeting_agent_runs.transcript_cursor")?,
    )?;
    let current_state = string(&outbox, "meeting_delivery_outbox.state")?;
    if current_state == "approved_local_only" {
        return Ok(meeting_delivery_preview_from_row(&outbox));
    }
    if current_state != "awaiting_approval" {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let asset_id = string(&outbox, "meeting_delivery_outbox.payload_ciphertext_ref")?;
    let asset = query(
        storage,
        "meeting_private_assets",
        &[
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "state",
            "retention_json",
        ],
        vec![eq("meeting_private_assets", "id", json!(&asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_DELIVERY_PAYLOAD_UNAVAILABLE".to_string())?;
    if string(&asset, "meeting_private_assets.vault_id")? != trusted.vault_id
        || string(&asset, "meeting_private_assets.owner_scope")? != owner_scope
        || string(&asset, "meeting_private_assets.purpose")? != "delivery_preview"
        || string(&asset, "meeting_private_assets.entity_id")? != intent_id
        || integer(&asset, "meeting_private_assets.entity_revision")?
            != integer(&outbox, "meeting_delivery_outbox.artifact_revision")?
        || string(&asset, "meeting_private_assets.key_ref")?
            != format!("knowledge:{}", trusted.vault_id)
        || string(&asset, "meeting_private_assets.state")? != "active"
    {
        return Err("MEETING_DELIVERY_PAYLOAD_UNAVAILABLE".to_string());
    }
    let expires_at = asset
        .get("meeting_private_assets.retention_json")
        .and_then(|value| value.get("expires_at"))
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(|| "MEETING_DELIVERY_APPROVAL_STALE".to_string())?
        .with_timezone(&chrono::Utc);
    if expires_at <= now {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let (_, expected_custody_ref) = knowledge_asset_path(&session.data_root, &asset_id)?;
    if string(&asset, "meeting_private_assets.custody_ref")? != expected_custody_ref {
        return Err("MEETING_DELIVERY_PAYLOAD_UNAVAILABLE".to_string());
    }
    let key_ref =
        crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{}", trusted.vault_id))?;
    let key = crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
    let (path, _) = knowledge_asset_path(&session.data_root, &asset_id)?;
    let encoded = read_bounded_private_asset(
        &path,
        crate::backup_payload::MAX_PRIVATE_ASSET_BYTES,
        "MEETING_DELIVERY_PAYLOAD_UNAVAILABLE",
    )?;
    let encrypted: crate::meeting_knowledge::EncryptedKnowledgePayload =
        serde_json::from_slice(encoded.as_slice())
            .map_err(|_| "MEETING_DELIVERY_PAYLOAD_UNAVAILABLE".to_string())?;
    let context = crate::meeting_knowledge::PrivateAssetContext {
        vault_id: trusted.vault_id.clone(),
        entity_id: intent_id.to_string(),
        version: integer(&outbox, "meeting_delivery_outbox.artifact_revision")? as u64,
        purpose: crate::meeting_knowledge::PrivateAssetPurpose::DeliveryPreview,
    };
    let payload = crate::meeting_knowledge::open_private_payload(&key, &context, &encrypted)?;
    if crate::backup_payload::sha256_hex(&payload) != approved_payload_hash {
        return Err("MEETING_DELIVERY_PAYLOAD_HASH_MISMATCH".to_string());
    }
    let destination_id = string(&outbox, "meeting_delivery_outbox.destination_id")?;
    let destination = query(
        storage,
        "meeting_destinations",
        &[
            "id",
            "agent_run_id",
            "provider_account_ref_ciphertext_ref",
            "provider_account_ref_sha256",
            "occurrence_key",
            "channel_type",
            "channel_id_ciphertext_ref",
            "channel_id_sha256",
            "thread_id_ciphertext_ref",
            "audience_policy_revision",
            "approval_snapshot_json",
            "state",
            "contract_version",
            "created_at",
        ],
        vec![eq("meeting_destinations", "id", json!(&destination_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_DELIVERY_DESTINATION_UNAVAILABLE".to_string())?;
    let mut approval = destination
        .get("meeting_destinations.approval_snapshot_json")
        .cloned()
        .ok_or_else(|| "MEETING_DELIVERY_DESTINATION_UNAVAILABLE".to_string())?;
    if approval.get("scope").and_then(Value::as_str) != Some("local_preview_only")
        || approval.get("payloadHash").and_then(Value::as_str) != Some(approved_payload_hash)
        || approval
            .get("expiresAt")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_none_or(|expiry| expiry.with_timezone(&chrono::Utc) != expires_at)
    {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let draft_id = approval
        .get("draftId")
        .and_then(Value::as_str)
        .ok_or_else(|| "MEETING_DELIVERY_APPROVAL_STALE".to_string())?
        .to_string();
    let draft_revision = approval
        .get("draftRevision")
        .and_then(Value::as_u64)
        .filter(|revision| *revision > 0)
        .ok_or_else(|| "MEETING_DELIVERY_APPROVAL_STALE".to_string())?;
    if draft_revision != integer(&outbox, "meeting_delivery_outbox.artifact_revision")? as u64 {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    let draft_expires_at = approval
        .get("expiresAt")
        .and_then(Value::as_str)
        .ok_or_else(|| "MEETING_DELIVERY_APPROVAL_STALE".to_string())?
        .to_string();
    let preview_request_id = string(&approval_run, "meeting_agent_runs.trigger_id")?;
    let persisted_preview = persist_local_meeting_delivery_preview(
        storage,
        session,
        project_id,
        recording_id,
        &preview_request_id,
        expected_revision,
        &draft_id,
        draft_revision,
        &draft_expires_at,
        &payload,
    )?;
    if persisted_preview.intent_id != intent_id
        || persisted_preview.payload_hash != approved_payload_hash
    {
        return Err("MEETING_DELIVERY_APPROVAL_STALE".to_string());
    }
    approval["state"] = json!("approved_local_only");
    approval["approvedAt"] = json!(attempt.committed_at);
    let mut destination_row = Value::Object(serde_json::Map::new());
    if let Some(values) = destination.as_object() {
        for (key, value) in values {
            let field = key.strip_prefix("meeting_destinations.").unwrap_or(key);
            destination_row[field] = value.clone();
        }
    }
    destination_row["approval_snapshot_json"] = approval;
    destination_row["state"] = json!("approved_local_only");
    let mut outbox_row = Value::Object(serde_json::Map::new());
    if let Some(values) = outbox.as_object() {
        for (key, value) in values {
            let field = key.strip_prefix("meeting_delivery_outbox.").unwrap_or(key);
            outbox_row[field] = value.clone();
        }
    }
    outbox_row["state"] = json!("approved_local_only");
    outbox_row["updated_at"] = json!(attempt.committed_at);
    let mut run_row = query(
        storage,
        "meeting_agent_runs",
        &[
            "id",
            "project_id",
            "recording_id",
            "meeting_session_id",
            "grant_id",
            "trigger_id",
            "transcript_cursor",
            "transcript_revision_set_json",
            "evidence_ids_json",
            "policy_version",
            "contract_version",
            "created_at",
        ],
        vec![eq("meeting_agent_runs", "id", json!(&run_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "MEETING_DELIVERY_RUN_UNAVAILABLE".to_string())?;
    run_row["meeting_agent_runs.state"] = json!("approval_recorded");
    let mut run_values = Value::Object(serde_json::Map::new());
    if let Some(values) = run_row.as_object() {
        for (key, value) in values {
            let field = key.strip_prefix("meeting_agent_runs.").unwrap_or(key);
            run_values[field] = value.clone();
        }
    }
    run_values["state"] = json!("approval_recorded");
    commit_native_owner_mutations(
        storage,
        session,
        &attempt,
        vec![
            upsert("meeting_delivery_outbox", outbox_row),
            upsert("meeting_destinations", destination_row),
            upsert("meeting_agent_runs", run_values),
        ],
    )?;
    Ok(meeting_intelligence_schema::MeetingAgentDeliveryPreview {
        intent_id: intent_id.to_string(),
        payload_hash: approved_payload_hash.to_string(),
        destination_summary: "ตัวอย่างในเครื่อง · ไม่มีผู้รับหรือช่องทางภายนอก".to_string(),
        state: "approved_local_only".to_string(),
        approval_scope: "local_preview_only".to_string(),
        external_dispatch_available: false,
    })
}

fn meeting_delivery_preview_from_row(
    row: &Value,
) -> meeting_intelligence_schema::MeetingAgentDeliveryPreview {
    meeting_intelligence_schema::MeetingAgentDeliveryPreview {
        intent_id: row
            .get("meeting_delivery_outbox.id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        payload_hash: row
            .get("meeting_delivery_outbox.payload_hash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        destination_summary: "ตัวอย่างในเครื่อง · ไม่มีผู้รับหรือช่องทางภายนอก".to_string(),
        state: row
            .get("meeting_delivery_outbox.state")
            .and_then(Value::as_str)
            .unwrap_or("blocked")
            .to_string(),
        approval_scope: "local_preview_only".to_string(),
        external_dispatch_available: false,
    }
}

/// Explicit first-use provisioning. The Genesis intent is committed before the
/// purpose-separated key is created, so retrying can reconcile the same vault
/// reference after interruption without replacing a key or creating a second
/// active vault.
pub(crate) fn provision_native_local_owner_vault(storage: &Storage) -> Result<String, String> {
    let owner_frontier = storage.txn_frontier();
    let identity_source = NativeDeviceOwnerIdentitySource;
    let lifecycle_source = NativeLifecycleWitnessSource;
    let identity_root = canonical_native_identity_root(storage)?;
    let owner_principal_ref = identity_source.owner_principal_ref(&identity_root)?;
    let witness = lifecycle_source.read()?;
    let matching = query_all(
        storage,
        "identity_vaults",
        &["id", "bound_account_ref", "state", "key_store_namespace"],
        vec![eq(
            "identity_vaults",
            "owner_principal_ref",
            json!(&owner_principal_ref),
        )],
    )?
    .into_iter()
    .filter(|row| {
        let account_matches = row
            .get("identity_vaults.bound_account_ref")
            .and_then(Value::as_str)
            .is_none_or(|bound| witness.native_user_id.as_deref() == Some(bound));
        account_matches
            && matches!(
                row.get("identity_vaults.state").and_then(Value::as_str),
                Some("active" | "provisioning")
            )
    })
    .collect::<Vec<_>>();
    let active = matching
        .iter()
        .filter(|row| row.get("identity_vaults.state").and_then(Value::as_str) == Some("active"))
        .collect::<Vec<_>>();
    if active.len() > 1 {
        return Err("LOCAL_OWNER_VAULT_SELECTION_REQUIRED".to_string());
    }
    if let Some(row) = active.first() {
        let vault_id = string(row, "identity_vaults.id")?;
        let key_ref =
            crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{vault_id}"))?;
        crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref)?;
        return Ok(vault_id);
    }
    let provisioning = matching
        .iter()
        .filter(|row| {
            row.get("identity_vaults.state").and_then(Value::as_str) == Some("provisioning")
        })
        .collect::<Vec<_>>();
    if provisioning.len() > 1 {
        return Err("LOCAL_OWNER_VAULT_PROVISIONING_AMBIGUOUS".to_string());
    }
    let vault_id = if let Some(row) = provisioning.first() {
        string(row, "identity_vaults.id")?
    } else {
        let vault_id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        let provisional_row = json!({
            "id": vault_id,
            "owner_principal_ref": owner_principal_ref,
            "bound_account_ref": null,
            "self_person_ref_ciphertext_ref": null,
            "self_person_ref_ciphertext_sha256": null,
            "self_person_ref_ciphertext_json": null,
            "state": "provisioning",
            "key_store_namespace": "people_metadata",
            "contract_version": 1,
            "created_at": created_at,
            "updated_at": created_at,
        });
        commit_owner_vault_state(
            storage,
            &format!("local-owner-vault-provision::{vault_id}"),
            owner_frontier,
            provisional_row,
        )
        .map_err(|error| {
            format!(
                "LOCAL_OWNER_VAULT_PROVISION_UNCERTAIN: retain vault id for retry {vault_id}; {error}"
            )
        })?;
        vault_id
    };
    let key_ref = crate::meeting_knowledge::KnowledgeKeyRef::new(format!("knowledge:{vault_id}"))?;
    match crate::meeting_knowledge::OsKnowledgeKeyBackend.read(&key_ref) {
        Ok(key) => drop(key),
        Err(error) if error == "KEY_UNAVAILABLE" => {
            crate::meeting_knowledge::OsKnowledgeKeyBackend.create(&key_ref)?;
        }
        Err(error) => return Err(error),
    }

    for _ in 0..3 {
        let expected_frontier = storage.txn_frontier();
        let row = query(
            storage,
            "identity_vaults",
            &[
                "owner_principal_ref",
                "state",
                "bound_account_ref",
                "created_at",
            ],
            vec![eq("identity_vaults", "id", json!(&vault_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "LOCAL_OWNER_VAULT_PROVISION_INTENT_MISSING".to_string())?;
        if string(&row, "identity_vaults.owner_principal_ref")? != owner_principal_ref {
            return Err("LOCAL_OWNER_VAULT_OWNER_MISMATCH".to_string());
        }
        match string(&row, "identity_vaults.state")?.as_str() {
            "active" => return Ok(vault_id),
            "provisioning" => {}
            _ => return Err("LOCAL_OWNER_VAULT_STATE_INVALID".to_string()),
        }
        if row
            .get("identity_vaults.bound_account_ref")
            .and_then(Value::as_str)
            .is_some()
        {
            return Err("LOCAL_OWNER_VAULT_ACCOUNT_BINDING_CHANGED".to_string());
        }
        let updated_at = chrono::Utc::now().to_rfc3339();
        let active_row = json!({
            "id": vault_id,
            "owner_principal_ref": owner_principal_ref,
            "bound_account_ref": null,
            "self_person_ref_ciphertext_ref": null,
            "self_person_ref_ciphertext_sha256": null,
            "self_person_ref_ciphertext_json": null,
            "state": "active",
            "key_store_namespace": "people_metadata",
            "contract_version": 1,
            "created_at": row.get("identity_vaults.created_at").cloned().unwrap_or(Value::Null),
            "updated_at": updated_at,
        });
        let transaction_id = format!("local-owner-vault-activate::{vault_id}::{expected_frontier}");
        match commit_owner_vault_state(storage, &transaction_id, expected_frontier, active_row) {
            Ok(()) => return Ok(vault_id),
            Err(error) if error.contains("expected frontier conflict") => continue,
            Err(error) => {
                let latest = query(
                    storage,
                    "identity_vaults",
                    &["state"],
                    vec![eq("identity_vaults", "id", json!(&vault_id))],
                    1,
                )?;
                if latest
                    .first()
                    .and_then(|row| row.get("identity_vaults.state"))
                    .and_then(Value::as_str)
                    == Some("active")
                {
                    return Ok(vault_id);
                }
                return Err(format!(
                    "LOCAL_OWNER_VAULT_ACTIVATION_UNCERTAIN: retain vault id for retry {vault_id}; {error}"
                ));
            }
        }
    }
    Err("LOCAL_OWNER_VAULT_ACTIVATION_CONFLICT".to_string())
}

fn capture_native_identity_context(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
) -> Result<NativeIdentityCapture, String> {
    let lifecycle_source = NativeLifecycleWitnessSource;
    revalidate_native_local_owner_session(storage, session, &lifecycle_source)?;
    let account_guard = crate::auth_session::account_begin_operation()
        .map_err(|_| "native account operation is unavailable".to_string())?;
    capture_native_identity_context_with_source(
        storage,
        session,
        &lifecycle_source,
        Some(account_guard),
    )
}

fn capture_native_identity_context_with_source(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    lifecycle_source: &dyn LifecycleWitnessSource,
    account_guard: Option<crate::auth_session::AccountOperationGuard>,
) -> Result<NativeIdentityCapture, String> {
    if account_guard.is_none() {
        return Err("native account operation is unavailable".to_string());
    }
    revalidate_native_local_owner_session(storage, session, lifecycle_source)?;
    if let Some(account_guard) = account_guard.as_ref() {
        account_guard
            .check()
            .map_err(|_| "native account operation was invalidated".to_string())?;
    }
    revalidate_native_local_owner_session(storage, session, lifecycle_source)?;
    Ok(NativeIdentityCapture {
        context: session.context.clone(),
        account_guard,
    })
}

#[cfg(test)]
fn test_local_identity_context(vault_id: &str) -> TrustedIdentityContext {
    TrustedIdentityContext {
        owner_principal_ref: "principal:device:test-owner".to_string(),
        account_ref: None,
        vault_id: vault_id.to_string(),
    }
}

fn optional_row_value<'a>(row: &'a Value, key: &str) -> Option<&'a Value> {
    row.get(key).filter(|value| !value.is_null())
}

fn identity_scope_key(scope: &MeetingScope) -> String {
    format!(
        "{}::{}::{}::{}::{}::{}",
        scope.project_id,
        scope.recording_id,
        scope.meeting_session_id,
        scope.source_session_id,
        scope.track_id,
        scope.source_generation
    )
}

fn people_metadata_key_ref(vault_id: &str) -> Result<String, String> {
    let vault =
        Uuid::parse_str(vault_id).map_err(|_| "PEOPLE_VAULT_REFERENCE_INVALID".to_string())?;
    Ok(format!("people_metadata:{}", vault.simple()))
}

fn people_profile_context(
    trusted: &TrustedIdentityContext,
    profile_id: &str,
    revision: i64,
) -> IdentityAadContext {
    IdentityAadContext {
        account_ref: trusted.account_ref.clone(),
        scope: local_owner_collection_scope(&trusted.vault_id),
        vault_id: trusted.vault_id.clone(),
        entity_id: profile_id.to_string(),
        revision,
        model_context: "people_profile".to_string(),
    }
}

fn read_people_profile_payload(
    storage: &Storage,
    trusted: &TrustedIdentityContext,
    profile_row: &Value,
    backend: &dyn IdentityKeyBackend,
) -> Result<meeting_intelligence_schema::PersonProfilePayload, String> {
    let profile_id = string(profile_row, "participant_profiles.id")?;
    let asset_id = string(
        profile_row,
        "participant_profiles.profile_payload_ciphertext_ref",
    )?;
    let key_ref = people_metadata_key_ref(&trusted.vault_id)?;
    if string(profile_row, "participant_profiles.vault_id")? != trusted.vault_id
        || string(profile_row, "participant_profiles.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
        || string(profile_row, "participant_profiles.key_ref")? != key_ref
    {
        return Err("PEOPLE_PROFILE_OWNER_MISMATCH".to_string());
    }
    let asset = query(
        storage,
        "meeting_private_assets",
        &[
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "ciphertext_sha256",
            "state",
        ],
        vec![eq("meeting_private_assets", "id", json!(asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "PEOPLE_PROFILE_ASSET_MISSING".to_string())?;
    let (path, relative) = people_asset_path(&canonical_data_root(storage)?, &asset_id)?;
    let revision = integer(profile_row, "participant_profiles.revision")?;
    if string(&asset, "meeting_private_assets.vault_id")? != trusted.vault_id
        || string(&asset, "meeting_private_assets.owner_scope")?
            != local_owner_collection_scope(&trusted.vault_id)
        || string(&asset, "meeting_private_assets.purpose")? != "people_profile"
        || string(&asset, "meeting_private_assets.entity_id")? != profile_id
        || integer(&asset, "meeting_private_assets.entity_revision")? != revision
        || string(&asset, "meeting_private_assets.custody_ref")? != relative
        || string(&asset, "meeting_private_assets.key_ref")? != key_ref
        || string(&asset, "meeting_private_assets.state")? != "active"
        || string(&asset, "meeting_private_assets.ciphertext_sha256")?
            != string(
                profile_row,
                "participant_profiles.profile_ciphertext_sha256",
            )?
    {
        return Err("PEOPLE_PROFILE_ASSET_METADATA_MISMATCH".to_string());
    }
    let metadata =
        std::fs::metadata(&path).map_err(|_| "PEOPLE_PROFILE_ASSET_UNAVAILABLE".to_string())?;
    if metadata.len() == 0 || metadata.len() > 16 * 1024 {
        return Err("PEOPLE_PROFILE_ASSET_SIZE_INVALID".to_string());
    }
    let encoded =
        read_bounded_private_asset(&path, 16 * 1024, "PEOPLE_PROFILE_ASSET_SIZE_INVALID")?;
    let reference: PrivateIdentityReference = serde_json::from_slice(&encoded)
        .map_err(|_| "PEOPLE_PROFILE_ASSET_MALFORMED".to_string())?;
    if reference.ciphertext_sha256
        != string(
            profile_row,
            "participant_profiles.profile_ciphertext_sha256",
        )?
        || reference.envelope.key_ref != key_ref
    {
        return Err("PEOPLE_PROFILE_ASSET_DIGEST_MISMATCH".to_string());
    }
    let context = people_profile_context(trusted, &profile_id, revision);
    meeting_intelligence_schema::open_person_profile(
        &reference,
        &context,
        &profile_id,
        revision,
        backend,
    )
}

fn profile_asset_row(
    asset_id: &str,
    project_id: &str,
    trusted: &TrustedIdentityContext,
    profile_id: &str,
    revision: i64,
    key_ref: &str,
    relative_path: &str,
    ciphertext_sha256: &str,
    ciphertext_bytes: i64,
    retention: Value,
    timestamp: &str,
) -> Value {
    json!({
        "id": asset_id,
        "project_id": project_id,
        "vault_id": trusted.vault_id,
        "owner_scope": local_owner_collection_scope(&trusted.vault_id),
        "purpose": "people_profile",
        "entity_id": profile_id,
        "entity_revision": revision,
        "custody_ref": relative_path,
        "key_ref": key_ref,
        "ciphertext_sha256": ciphertext_sha256,
        "ciphertext_bytes": ciphertext_bytes,
        "state": "active",
        "retention_json": retention,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": timestamp,
    })
}

fn people_profile_row(
    profile_id: &str,
    trusted: &TrustedIdentityContext,
    asset_id: &str,
    ciphertext_sha256: &str,
    key_ref: &str,
    status: &str,
    revision: i64,
    created_at: &str,
    updated_at: &str,
) -> Value {
    json!({
        "id": profile_id,
        "vault_id": trusted.vault_id,
        "owner_scope": local_owner_collection_scope(&trusted.vault_id),
        "profile_payload_ciphertext_ref": asset_id,
        "profile_ciphertext_sha256": ciphertext_sha256,
        "key_ref": key_ref,
        "status": status,
        "revision": revision,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": created_at,
        "updated_at": updated_at,
    })
}

fn current_speaker_evidence(
    storage: &Storage,
    scope: &MeetingScope,
    speaker_id: &str,
) -> Result<(i64, String), String> {
    scope.validate()?;
    if speaker_id.trim().is_empty() || speaker_id.len() > 256 {
        return Err("PEOPLE_SPEAKER_ID_INVALID".to_string());
    }
    let snapshot = meeting_transcript_snapshot(storage, &scope.project_id, &scope.recording_id)?;
    let mut evidence = snapshot
        .utterances
        .iter()
        .filter(|row| row.get("speaker_id").and_then(Value::as_str) == Some(speaker_id))
        .filter_map(|row| {
            let row_scope = row
                .get("scope")
                .and_then(|value| serde_json::from_value::<MeetingScope>(value.clone()).ok())?;
            (row_scope == *scope).then(|| {
                json!({
                    "utterance_id": row.get("utterance_id"),
                    "revision_id": row.get("revision_id"),
                    "revision": row.get("revision"),
                    "start_ms": row.get("start_ms"),
                    "end_ms": row.get("end_ms"),
                    "state": row.get("state"),
                })
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.get("utterance_id")
            .and_then(Value::as_str)
            .cmp(&right.get("utterance_id").and_then(Value::as_str))
    });
    if evidence.is_empty() || snapshot.high_watermark < 0 {
        return Err("PEOPLE_SPEAKER_EVIDENCE_UNAVAILABLE".to_string());
    }
    let bytes =
        serde_json::to_vec(&evidence).map_err(|_| "PEOPLE_SPEAKER_EVIDENCE_INVALID".to_string())?;
    Ok((
        snapshot.high_watermark,
        format!("{:x}", Sha256::digest(bytes)),
    ))
}

fn evidence_reference(cursor: i64, digest: &str) -> String {
    format!("local-revision:{cursor}:{digest}")
}

fn parse_evidence_reference(value: &str) -> Result<(i64, String), String> {
    let rest = value
        .strip_prefix("local-revision:")
        .ok_or_else(|| "PEOPLE_EVIDENCE_REFERENCE_INVALID".to_string())?;
    let (cursor, digest) = rest
        .split_once(':')
        .ok_or_else(|| "PEOPLE_EVIDENCE_REFERENCE_INVALID".to_string())?;
    let cursor = cursor
        .parse::<i64>()
        .ok()
        .filter(|cursor| *cursor >= 0)
        .ok_or_else(|| "PEOPLE_EVIDENCE_REFERENCE_INVALID".to_string())?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("PEOPLE_EVIDENCE_REFERENCE_INVALID".to_string());
    }
    Ok((cursor, digest.to_string()))
}

fn manual_identity_model_context(speaker_id: &str, evidence_digest: &str) -> String {
    format!("manual_review:{speaker_id}:{evidence_digest}")
}

fn native_people_profile(
    storage: &Storage,
    trusted: &TrustedIdentityContext,
    row: &Value,
    backend: &dyn IdentityKeyBackend,
) -> Result<meeting_intelligence_schema::MeetingPeopleProfile, String> {
    let payload = read_people_profile_payload(storage, trusted, row, backend)?;
    let profile = meeting_intelligence_schema::MeetingPeopleProfile {
        profile_id: payload.profile_id.clone(),
        display_name: payload.display_name.clone(),
        revision: payload.revision,
    };
    let mut payload = payload;
    zeroize::Zeroize::zeroize(&mut payload);
    Ok(profile)
}

fn find_people_profile(
    storage: &Storage,
    trusted: &TrustedIdentityContext,
    profile_id: &str,
) -> Result<Option<Value>, String> {
    let row = query(
        storage,
        "participant_profiles",
        &[
            "id",
            "vault_id",
            "owner_scope",
            "profile_payload_ciphertext_ref",
            "profile_ciphertext_sha256",
            "key_ref",
            "status",
            "revision",
            "contract_version",
            "created_at",
            "updated_at",
        ],
        vec![eq("participant_profiles", "id", json!(profile_id))],
        1,
    )?
    .into_iter()
    .next();
    if let Some(row) = row.as_ref() {
        if string(row, "participant_profiles.vault_id")? != trusted.vault_id
            || string(row, "participant_profiles.owner_scope")?
                != local_owner_collection_scope(&trusted.vault_id)
        {
            return Err("PEOPLE_PROFILE_OWNER_MISMATCH".to_string());
        }
    }
    Ok(row)
}

pub(crate) fn list_meeting_people(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleListRequest,
) -> Result<meeting_intelligence_schema::MeetingPeopleSnapshot, String> {
    request.scope.validate()?;
    require_meeting_scope(storage, &request.scope)?;
    let native_capture = capture_native_identity_context(storage, session)?;
    let trusted = native_capture.context.clone();
    let account_guard = native_capture
        .account_guard
        .as_ref()
        .ok_or_else(|| "native account operation is unavailable".to_string())?;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    account_guard.with_account_lifecycle_fence(&session.account_witness, || {
        let backend = OsPeopleMetadataKeyBackend;
        let profile_rows = query_all(
            storage,
            "participant_profiles",
            &[
                "id",
                "vault_id",
                "owner_scope",
                "profile_payload_ciphertext_ref",
                "profile_ciphertext_sha256",
                "key_ref",
                "status",
                "revision",
                "contract_version",
                "created_at",
                "updated_at",
            ],
            vec![
                eq("participant_profiles", "vault_id", json!(&trusted.vault_id)),
                eq("participant_profiles", "status", json!("active")),
            ],
        )?;
        let mut profiles = Vec::with_capacity(profile_rows.len());
        for row in &profile_rows {
            profiles.push(native_people_profile(storage, &trusted, row, &backend)?);
        }

        let transcript = meeting_transcript_snapshot(
            storage,
            &request.scope.project_id,
            &request.scope.recording_id,
        )?;
        let mut speakers_by_id = BTreeMap::<String, String>::new();
        for utterance in &transcript.utterances {
            if utterance
                .get("speaker_id")
                .and_then(Value::as_str)
                .is_none()
            {
                continue;
            }
            let Some(scope_value) = utterance.get("scope") else {
                continue;
            };
            let Ok(utterance_scope) = serde_json::from_value::<MeetingScope>(scope_value.clone())
            else {
                continue;
            };
            if utterance_scope != request.scope {
                continue;
            }
            let speaker_id = utterance
                .get("speaker_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let label = utterance
                .get("speaker_label")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("ผู้พูด {}", speakers_by_id.len() + 1));
            speakers_by_id.entry(speaker_id).or_insert(label);
        }
        let mut speakers = Vec::with_capacity(speakers_by_id.len());
        for (speaker_id, display_label) in speakers_by_id {
            let (evidence_revision, _) =
                current_speaker_evidence(storage, &request.scope, &speaker_id)?;
            speakers.push(meeting_intelligence_schema::MeetingPeopleSpeaker {
                speaker_id,
                display_label,
                evidence_revision,
            });
        }

        let link_rows = query_all(
            storage,
            "speaker_identity_links",
            &[
                "id",
                "project_id",
                "recording_id",
                "meeting_session_id",
                "speaker_id",
                "vault_id",
                "person_ref_ciphertext_ref",
                "person_ref_ciphertext_sha256",
                "person_ref_ciphertext_json",
                "person_ref_key_ref",
                "match_source",
                "status",
                "model_run_id",
                "evidence_ciphertext_ref",
                "evidence_ciphertext_sha256",
                "expected_revision",
                "revision",
                "locked_at",
                "revoked_at",
            ],
            vec![
                eq(
                    "speaker_identity_links",
                    "project_id",
                    json!(&request.scope.project_id),
                ),
                eq(
                    "speaker_identity_links",
                    "recording_id",
                    json!(&request.scope.recording_id),
                ),
                eq(
                    "speaker_identity_links",
                    "meeting_session_id",
                    json!(&request.scope.meeting_session_id),
                ),
                eq(
                    "speaker_identity_links",
                    "vault_id",
                    json!(&trusted.vault_id),
                ),
            ],
        )?;
        let mut links = Vec::with_capacity(link_rows.len());
        for row in &link_rows {
            let link_id = string(row, "speaker_identity_links.id")?;
            let speaker_id = string(row, "speaker_identity_links.speaker_id")?;
            let status = string(row, "speaker_identity_links.status")?;
            let revision = integer(row, "speaker_identity_links.revision")?;
            let source = string(row, "speaker_identity_links.match_source")?;
            let (evidence_revision, digest, mut stale) = if source == "manual_review" {
                let (stored_cursor, stored_digest) = parse_evidence_reference(&string(
                    row,
                    "speaker_identity_links.evidence_ciphertext_ref",
                )?)?;
                let current = current_speaker_evidence(storage, &request.scope, &speaker_id);
                let stale = current
                    .as_ref()
                    .map(|(cursor, current_digest)| {
                        *cursor != stored_cursor || current_digest != &stored_digest
                    })
                    .unwrap_or(true);
                (stored_cursor, stored_digest, stale)
            } else {
                (0, String::new(), false)
            };
            let reference = reference_from_identity_row(row)?;
            let context = IdentityAadContext {
                account_ref: trusted.account_ref.clone(),
                scope: identity_scope_key(&request.scope),
                vault_id: trusted.vault_id.clone(),
                entity_id: link_id.clone(),
                revision,
                model_context: if source == "manual_review" {
                    manual_identity_model_context(&speaker_id, &digest)
                } else {
                    optional_row_value(row, "speaker_identity_links.model_run_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| "none".to_string())
                },
            };
            let person = open_person_identity(&reference, &context, &link_id, revision, &backend)?;
            if source == "manual_review" {
                let current_profile = find_people_profile(storage, &trusted, &person.profile_id)?;
                let profile_is_current = current_profile.is_some_and(|profile| {
                    profile
                        .get("participant_profiles.status")
                        .and_then(Value::as_str)
                        == Some("active")
                        && profile
                            .get("participant_profiles.revision")
                            .and_then(Value::as_i64)
                            == Some(person.profile_revision)
                });
                stale |= !profile_is_current;
            }
            let reported_status =
                if stale && matches!(status.as_str(), "pending_review" | "confirmed") {
                    "stale".to_string()
                } else {
                    status
                };
            links.push(meeting_intelligence_schema::MeetingPeopleLink {
                link_id,
                speaker_id,
                profile_id: person.profile_id.clone(),
                display_name: person.display_name.clone(),
                status: reported_status,
                revision,
                evidence_revision,
            });
        }
        Ok(meeting_intelligence_schema::MeetingPeopleSnapshot {
            vault_id: trusted.vault_id,
            profiles,
            speakers,
            links,
        })
    })
}

pub(crate) fn create_meeting_people_profile(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleProfileCreateRequest,
) -> Result<meeting_intelligence_schema::MeetingPeopleProfile, String> {
    if request.display_name.trim().is_empty()
        || request.display_name.trim().len() > 256
        || request.display_name.chars().any(char::is_control)
    {
        return Err("PEOPLE_PROFILE_NAME_INVALID".to_string());
    }
    validate_project_exists(storage, &request.project_id)?;
    let attempt = begin_meeting_commit(
        storage,
        &format!("people-profile-create::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let key_ref = people_metadata_key_ref(&trusted.vault_id)?;
    let backend = OsPeopleMetadataKeyBackend;
    let created_key = zeroize::Zeroizing::new(backend.ensure_key(&key_ref)?);
    if created_key.len() != 32 {
        return Err("identity key has invalid length".to_string());
    }
    let profile_uuid = Uuid::new_v4().simple().to_string();
    let profile_id = format!("profile:{profile_uuid}");
    let person_id = Uuid::new_v4().simple().to_string();
    let asset_id = Uuid::new_v4().simple().to_string();
    let created_at = attempt.committed_at.clone();
    let payload = meeting_intelligence_schema::PersonProfilePayload {
        profile_id: profile_id.clone(),
        person_id,
        display_name: request.display_name.trim().to_string(),
        vault_id: trusted.vault_id.clone(),
        revision: 1,
    };
    let context = people_profile_context(&trusted, &profile_id, 1);
    let reference =
        meeting_intelligence_schema::seal_person_profile(&payload, &context, &key_ref, &backend)?;
    let (relative, ciphertext_bytes, retention) =
        write_people_profile_asset(&session.data_root, &asset_id, &reference)?;
    let profile_row = people_profile_row(
        &profile_id,
        &trusted,
        &asset_id,
        &reference.ciphertext_sha256,
        &key_ref,
        "active",
        1,
        &created_at,
        &created_at,
    );
    let asset_row = profile_asset_row(
        &asset_id,
        &request.project_id,
        &trusted,
        &profile_id,
        1,
        &key_ref,
        &relative,
        &reference.ciphertext_sha256,
        ciphertext_bytes,
        retention,
        &created_at,
    );
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    commit_rows_at_frontier(
        storage,
        attempt.expected_frontier,
        vec![
            upsert("participant_profiles", profile_row),
            upsert("meeting_private_assets", asset_row),
        ],
    )?;
    let mut payload = payload;
    zeroize::Zeroize::zeroize(&mut payload);
    Ok(meeting_intelligence_schema::MeetingPeopleProfile {
        profile_id,
        display_name: request.display_name.trim().to_string(),
        revision: 1,
    })
}

pub(crate) fn update_meeting_people_profile(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleProfileUpdateRequest,
) -> Result<meeting_intelligence_schema::MeetingPeopleProfile, String> {
    if request.display_name.trim().is_empty()
        || request.display_name.trim().len() > 256
        || request.display_name.chars().any(char::is_control)
    {
        return Err("PEOPLE_PROFILE_NAME_INVALID".to_string());
    }
    validate_project_exists(storage, &request.project_id)?;
    let attempt = begin_meeting_commit(
        storage,
        &format!("people-profile-update::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let profile = find_people_profile(storage, &trusted, &request.profile_id)?
        .ok_or_else(|| "PEOPLE_PROFILE_NOT_FOUND".to_string())?;
    if string(&profile, "participant_profiles.status")? != "active"
        || integer(&profile, "participant_profiles.revision")? != request.expected_revision
    {
        return Err("PEOPLE_PROFILE_REVISION_CONFLICT".to_string());
    }
    let backend = OsPeopleMetadataKeyBackend;
    let mut previous = read_people_profile_payload(storage, &trusted, &profile, &backend)?;
    let revision = request
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| "PEOPLE_PROFILE_REVISION_OVERFLOW".to_string())?;
    let profile_id = request.profile_id.clone();
    let asset_id = Uuid::new_v4().simple().to_string();
    let key_ref = people_metadata_key_ref(&trusted.vault_id)?;
    let payload = meeting_intelligence_schema::PersonProfilePayload {
        profile_id: profile_id.clone(),
        person_id: previous.person_id.clone(),
        display_name: request.display_name.trim().to_string(),
        vault_id: trusted.vault_id.clone(),
        revision,
    };
    let context = people_profile_context(&trusted, &profile_id, revision);
    let reference =
        meeting_intelligence_schema::seal_person_profile(&payload, &context, &key_ref, &backend)?;
    let (relative, ciphertext_bytes, retention) =
        write_people_profile_asset(&session.data_root, &asset_id, &reference)?;
    let timestamp = attempt.committed_at.clone();
    let old_asset_id = string(
        &profile,
        "participant_profiles.profile_payload_ciphertext_ref",
    )?;
    let old_asset = query(
        storage,
        "meeting_private_assets",
        &[
            "id",
            "project_id",
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "ciphertext_sha256",
            "ciphertext_bytes",
            "state",
            "retention_json",
            "contract_version",
            "created_at",
            "updated_at",
        ],
        vec![eq("meeting_private_assets", "id", json!(&old_asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "PEOPLE_PROFILE_ASSET_MISSING".to_string())?;
    let superseded_asset = json!({
        "id": old_asset_id,
        "project_id": old_asset.get("meeting_private_assets.project_id"),
        "vault_id": old_asset.get("meeting_private_assets.vault_id"),
        "owner_scope": old_asset.get("meeting_private_assets.owner_scope"),
        "purpose": old_asset.get("meeting_private_assets.purpose"),
        "entity_id": old_asset.get("meeting_private_assets.entity_id"),
        "entity_revision": old_asset.get("meeting_private_assets.entity_revision"),
        "custody_ref": old_asset.get("meeting_private_assets.custody_ref"),
        "key_ref": old_asset.get("meeting_private_assets.key_ref"),
        "ciphertext_sha256": old_asset.get("meeting_private_assets.ciphertext_sha256"),
        "ciphertext_bytes": old_asset.get("meeting_private_assets.ciphertext_bytes"),
        "state": "superseded",
        "retention_json": old_asset.get("meeting_private_assets.retention_json"),
        "contract_version": old_asset.get("meeting_private_assets.contract_version"),
        "created_at": old_asset.get("meeting_private_assets.created_at"),
        "updated_at": timestamp,
    });
    let updated_profile = people_profile_row(
        &profile_id,
        &trusted,
        &asset_id,
        &reference.ciphertext_sha256,
        &key_ref,
        "active",
        revision,
        &string(&profile, "participant_profiles.created_at")?,
        &timestamp,
    );
    let new_asset = profile_asset_row(
        &asset_id,
        &request.project_id,
        &trusted,
        &profile_id,
        revision,
        &key_ref,
        &relative,
        &reference.ciphertext_sha256,
        ciphertext_bytes,
        retention,
        &timestamp,
    );
    commit_rows_at_frontier(
        storage,
        attempt.expected_frontier,
        vec![
            upsert("meeting_private_assets", superseded_asset),
            upsert("meeting_private_assets", new_asset),
            upsert("participant_profiles", updated_profile),
        ],
    )?;
    previous.zeroize();
    let mut payload = payload;
    zeroize::Zeroize::zeroize(&mut payload);
    Ok(meeting_intelligence_schema::MeetingPeopleProfile {
        profile_id,
        display_name: request.display_name.trim().to_string(),
        revision,
    })
}

pub(crate) fn archive_meeting_people_profile(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleProfileArchiveRequest,
) -> Result<(), String> {
    let attempt = begin_meeting_commit(
        storage,
        &format!("people-profile-archive::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let profile = find_people_profile(storage, &trusted, &request.profile_id)?
        .ok_or_else(|| "PEOPLE_PROFILE_NOT_FOUND".to_string())?;
    if string(&profile, "participant_profiles.status")? != "active"
        || integer(&profile, "participant_profiles.revision")? != request.expected_revision
    {
        return Err("PEOPLE_PROFILE_REVISION_CONFLICT".to_string());
    }
    let revision = request
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| "PEOPLE_PROFILE_REVISION_OVERFLOW".to_string())?;
    let backend = OsPeopleMetadataKeyBackend;
    let mut previous = read_people_profile_payload(storage, &trusted, &profile, &backend)?;
    let asset_id = Uuid::new_v4().simple().to_string();
    let profile_id = request.profile_id.clone();
    let key_ref = string(&profile, "participant_profiles.key_ref")?;
    let payload = meeting_intelligence_schema::PersonProfilePayload {
        profile_id: profile_id.clone(),
        person_id: previous.person_id.clone(),
        display_name: previous.display_name.clone(),
        vault_id: trusted.vault_id.clone(),
        revision,
    };
    let context = people_profile_context(&trusted, &profile_id, revision);
    let reference =
        meeting_intelligence_schema::seal_person_profile(&payload, &context, &key_ref, &backend)?;
    let (relative, ciphertext_bytes, retention) =
        write_people_profile_asset(&session.data_root, &asset_id, &reference)?;
    let old_asset_id = string(
        &profile,
        "participant_profiles.profile_payload_ciphertext_ref",
    )?;
    let old_asset = query(
        storage,
        "meeting_private_assets",
        &[
            "id",
            "project_id",
            "vault_id",
            "owner_scope",
            "purpose",
            "entity_id",
            "entity_revision",
            "custody_ref",
            "key_ref",
            "ciphertext_sha256",
            "ciphertext_bytes",
            "state",
            "retention_json",
            "contract_version",
            "created_at",
            "updated_at",
        ],
        vec![eq("meeting_private_assets", "id", json!(&old_asset_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "PEOPLE_PROFILE_ASSET_MISSING".to_string())?;
    let timestamp = attempt.committed_at.clone();
    let superseded_asset = json!({
        "id": old_asset_id,
        "project_id": old_asset.get("meeting_private_assets.project_id"),
        "vault_id": old_asset.get("meeting_private_assets.vault_id"),
        "owner_scope": old_asset.get("meeting_private_assets.owner_scope"),
        "purpose": old_asset.get("meeting_private_assets.purpose"),
        "entity_id": old_asset.get("meeting_private_assets.entity_id"),
        "entity_revision": old_asset.get("meeting_private_assets.entity_revision"),
        "custody_ref": old_asset.get("meeting_private_assets.custody_ref"),
        "key_ref": old_asset.get("meeting_private_assets.key_ref"),
        "ciphertext_sha256": old_asset.get("meeting_private_assets.ciphertext_sha256"),
        "ciphertext_bytes": old_asset.get("meeting_private_assets.ciphertext_bytes"),
        "state": "superseded",
        "retention_json": old_asset.get("meeting_private_assets.retention_json"),
        "contract_version": old_asset.get("meeting_private_assets.contract_version"),
        "created_at": old_asset.get("meeting_private_assets.created_at"),
        "updated_at": timestamp,
    });
    let updated = people_profile_row(
        &request.profile_id,
        &trusted,
        &asset_id,
        &reference.ciphertext_sha256,
        &key_ref,
        "archived",
        revision,
        &string(&profile, "participant_profiles.created_at")?,
        &timestamp,
    );
    let new_asset = profile_asset_row(
        &asset_id,
        &string(&old_asset, "meeting_private_assets.project_id")?,
        &trusted,
        &profile_id,
        revision,
        &key_ref,
        &relative,
        &reference.ciphertext_sha256,
        ciphertext_bytes,
        retention,
        &timestamp,
    );
    commit_rows_at_frontier(
        storage,
        attempt.expected_frontier,
        vec![
            upsert("meeting_private_assets", superseded_asset),
            upsert("meeting_private_assets", new_asset),
            upsert("participant_profiles", updated),
        ],
    )?;
    previous.zeroize();
    let mut payload = payload;
    zeroize::Zeroize::zeroize(&mut payload);
    Ok(())
}

pub(crate) fn propose_meeting_people_link(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleLinkProposalRequest,
) -> Result<meeting_intelligence_schema::MeetingPeopleLink, String> {
    request.scope.validate()?;
    require_meeting_scope(storage, &request.scope)?;
    let (evidence_revision, evidence_digest) =
        current_speaker_evidence(storage, &request.scope, &request.speaker_id)?;
    if evidence_revision != request.expected_evidence_revision {
        return Err("PEOPLE_SPEAKER_EVIDENCE_REVISION_CONFLICT".to_string());
    }
    let attempt = begin_meeting_commit(
        storage,
        &format!("people-link-propose::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let backend = OsPeopleMetadataKeyBackend;
    let profile = find_people_profile(storage, &trusted, &request.profile_id)?
        .ok_or_else(|| "PEOPLE_PROFILE_NOT_FOUND".to_string())?;
    if string(&profile, "participant_profiles.status")? != "active" {
        return Err("PEOPLE_PROFILE_NOT_ACTIVE".to_string());
    }
    let profile_payload = read_people_profile_payload(storage, &trusted, &profile, &backend)?;
    let duplicate = query_all(
        storage,
        "speaker_identity_links",
        &["id", "speaker_id", "status"],
        vec![
            eq(
                "speaker_identity_links.project_id",
                "project_id",
                json!(&request.scope.project_id),
            ),
            eq(
                "speaker_identity_links.recording_id",
                "recording_id",
                json!(&request.scope.recording_id),
            ),
            eq(
                "speaker_identity_links.meeting_session_id",
                "meeting_session_id",
                json!(&request.scope.meeting_session_id),
            ),
            eq(
                "speaker_identity_links.speaker_id",
                "speaker_id",
                json!(&request.speaker_id),
            ),
        ],
    )?;
    if duplicate.iter().any(|row| {
        matches!(
            row.get("speaker_identity_links.status")
                .and_then(Value::as_str),
            Some("pending_review" | "confirmed")
        )
    }) {
        return Err("PEOPLE_SPEAKER_LINK_ALREADY_EXISTS".to_string());
    }
    let session_row = query(
        storage,
        "meeting_sessions",
        &["revision"],
        vec![eq(
            "meeting_sessions",
            "id",
            json!(&request.scope.meeting_session_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "meeting scope references a missing meeting session".to_string())?;
    let link_id = format!("identity-link:{}", Uuid::new_v4().simple());
    let profile_revision = integer(&profile, "participant_profiles.revision")?;
    let payload = meeting_intelligence_schema::PersonIdentityPayload {
        link_id: link_id.clone(),
        profile_id: profile_payload.profile_id.clone(),
        person_id: profile_payload.person_id.clone(),
        display_name: profile_payload.display_name.clone(),
        account_ref: trusted.account_ref.clone(),
        vault_id: trusted.vault_id.clone(),
        relationship_revision: 1,
        profile_revision,
    };
    let context = IdentityAadContext {
        account_ref: trusted.account_ref.clone(),
        scope: identity_scope_key(&request.scope),
        vault_id: trusted.vault_id.clone(),
        entity_id: link_id.clone(),
        revision: 1,
        model_context: manual_identity_model_context(&request.speaker_id, &evidence_digest),
    };
    let key_ref = people_metadata_key_ref(&trusted.vault_id)?;
    let reference =
        meeting_intelligence_schema::seal_person_identity(&payload, &context, &key_ref, &backend)?;
    let envelope_json = serde_json::to_string(&reference.envelope)
        .map_err(|_| "PEOPLE_LINK_SERIALIZATION_FAILED".to_string())?;
    let timestamp = attempt.committed_at.clone();
    let row = json!({
        "id": link_id,
        "project_id": request.scope.project_id,
        "recording_id": request.scope.recording_id,
        "meeting_session_id": request.scope.meeting_session_id,
        "speaker_id": request.speaker_id,
        "vault_id": trusted.vault_id,
        "person_ref_ciphertext_ref": reference.encrypted_blob_ref,
        "person_ref_ciphertext_sha256": reference.ciphertext_sha256,
        "person_ref_ciphertext_json": envelope_json,
        "person_ref_key_ref": key_ref,
        "match_source": "manual_review",
        "status": "pending_review",
        "match_score": null,
        "threshold_policy_id": null,
        "model_run_id": null,
        "evidence_ciphertext_ref": evidence_reference(evidence_revision, &evidence_digest),
        "evidence_ciphertext_sha256": evidence_digest,
        "reviewer_ref_ciphertext_ref": null,
        "actor_ref_ciphertext_ref": null,
        "expected_revision": 0,
        "supersedes_id": null,
        "d8_policy_revision": integer(&session_row, "meeting_sessions.revision")?,
        "revision": 1,
        "locked_at": null,
        "revoked_at": null,
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "created_at": timestamp,
        "updated_at": attempt.committed_at,
    });
    commit_rows_at_frontier(
        storage,
        attempt.expected_frontier,
        vec![upsert("speaker_identity_links", row)],
    )?;
    let result = meeting_intelligence_schema::MeetingPeopleLink {
        link_id,
        speaker_id: request.speaker_id.clone(),
        profile_id: profile_payload.profile_id.clone(),
        display_name: profile_payload.display_name.clone(),
        status: "pending_review".to_string(),
        revision: 1,
        evidence_revision,
    };
    Ok(result)
}

pub(crate) fn mutate_meeting_people_link(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    request: &meeting_intelligence_schema::MeetingPeopleLinkMutationRequest,
    action: &str,
) -> Result<(), String> {
    if !matches!(action, "confirm" | "reject" | "unlink") {
        return Err("PEOPLE_LINK_ACTION_UNSUPPORTED".to_string());
    }
    request.scope.validate()?;
    require_meeting_scope(storage, &request.scope)?;
    let attempt = begin_meeting_commit(
        storage,
        &format!("people-link-{action}::{}", Uuid::new_v4()),
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let trusted = capture_native_identity_context(storage, session)?.context;
    let _owner_fence = session.begin_operation_fence()?;
    revalidate_native_local_owner_session_under_fence(
        storage,
        session,
        &NativeLifecycleWitnessSource,
    )?;
    let row = query(
        storage,
        "speaker_identity_links",
        &[
            "id",
            "project_id",
            "recording_id",
            "meeting_session_id",
            "speaker_id",
            "vault_id",
            "person_ref_ciphertext_ref",
            "person_ref_ciphertext_sha256",
            "person_ref_ciphertext_json",
            "person_ref_key_ref",
            "match_source",
            "status",
            "match_score",
            "threshold_policy_id",
            "model_run_id",
            "evidence_ciphertext_ref",
            "evidence_ciphertext_sha256",
            "reviewer_ref_ciphertext_ref",
            "actor_ref_ciphertext_ref",
            "expected_revision",
            "supersedes_id",
            "d8_policy_revision",
            "revision",
            "locked_at",
            "revoked_at",
            "contract_version",
            "created_at",
            "updated_at",
        ],
        vec![eq("speaker_identity_links", "id", json!(&request.link_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "PEOPLE_LINK_NOT_FOUND".to_string())?;
    let revision = integer(&row, "speaker_identity_links.revision")?;
    if string(&row, "speaker_identity_links.project_id")? != request.scope.project_id
        || string(&row, "speaker_identity_links.recording_id")? != request.scope.recording_id
        || string(&row, "speaker_identity_links.meeting_session_id")?
            != request.scope.meeting_session_id
        || string(&row, "speaker_identity_links.vault_id")? != trusted.vault_id
        || string(&row, "speaker_identity_links.match_source")? != "manual_review"
    {
        return Err("PEOPLE_LINK_SCOPE_INVALID".to_string());
    }
    if revision != request.expected_revision {
        return Err("PEOPLE_LINK_REVISION_CONFLICT".to_string());
    }
    let status = string(&row, "speaker_identity_links.status")?;
    match action {
        "confirm" if status != "pending_review" => {
            return Err("PEOPLE_LINK_STATE_CONFLICT".to_string());
        }
        "reject" if status != "pending_review" => {
            return Err("PEOPLE_LINK_STATE_CONFLICT".to_string());
        }
        "unlink" if status != "confirmed" => {
            return Err("PEOPLE_LINK_STATE_CONFLICT".to_string());
        }
        _ => {}
    }
    let next_revision = revision
        .checked_add(1)
        .ok_or_else(|| "PEOPLE_LINK_REVISION_OVERFLOW".to_string())?;
    let mut mutations = Vec::new();
    if action == "confirm" {
        let speaker_id = string(&row, "speaker_identity_links.speaker_id")?;
        let (cursor, digest) = parse_evidence_reference(&string(
            &row,
            "speaker_identity_links.evidence_ciphertext_ref",
        )?)?;
        if digest != string(&row, "speaker_identity_links.evidence_ciphertext_sha256")? {
            return Err("PEOPLE_LINK_EVIDENCE_INVALID".to_string());
        }
        let (current_cursor, current_digest) =
            current_speaker_evidence(storage, &request.scope, &speaker_id)?;
        if cursor != current_cursor || digest != current_digest {
            return Err("PEOPLE_LINK_EVIDENCE_STALE".to_string());
        }
        let reference = reference_from_identity_row(&row)?;
        let old_context = IdentityAadContext {
            account_ref: trusted.account_ref.clone(),
            scope: identity_scope_key(&request.scope),
            vault_id: trusted.vault_id.clone(),
            entity_id: request.link_id.clone(),
            revision,
            model_context: manual_identity_model_context(&speaker_id, &digest),
        };
        let backend = OsPeopleMetadataKeyBackend;
        let existing_person = open_person_identity(
            &reference,
            &old_context,
            &request.link_id,
            revision,
            &backend,
        )?;
        let profile = find_people_profile(storage, &trusted, &existing_person.profile_id)?
            .ok_or_else(|| "PEOPLE_PROFILE_NOT_FOUND".to_string())?;
        if string(&profile, "participant_profiles.status")? != "active"
            || integer(&profile, "participant_profiles.revision")?
                != existing_person.profile_revision
        {
            return Err("PEOPLE_LINK_PROFILE_REVISION_STALE".to_string());
        }
        let profile_payload = read_people_profile_payload(storage, &trusted, &profile, &backend)?;
        if profile_payload.person_id != existing_person.person_id
            || profile_payload.display_name != existing_person.display_name
        {
            return Err("PEOPLE_LINK_PROFILE_REVISION_STALE".to_string());
        }
        let mut next_payload = meeting_intelligence_schema::PersonIdentityPayload {
            link_id: request.link_id.clone(),
            profile_id: profile_payload.profile_id.clone(),
            person_id: profile_payload.person_id.clone(),
            display_name: profile_payload.display_name.clone(),
            account_ref: trusted.account_ref.clone(),
            vault_id: trusted.vault_id.clone(),
            relationship_revision: next_revision,
            profile_revision: profile_payload.revision,
        };
        let next_context = IdentityAadContext {
            revision: next_revision,
            ..old_context
        };
        let next_reference = meeting_intelligence_schema::seal_person_identity(
            &next_payload,
            &next_context,
            &people_metadata_key_ref(&trusted.vault_id)?,
            &backend,
        )?;
        let mut values = row
            .as_object()
            .ok_or_else(|| "PEOPLE_LINK_ROW_MALFORMED".to_string())?
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("speaker_identity_links.")
                    .map(|column| (column.to_string(), value.clone()))
            })
            .collect::<serde_json::Map<String, Value>>();
        values.insert("status".to_string(), json!("confirmed"));
        values.insert("revision".to_string(), json!(next_revision));
        values.insert("expected_revision".to_string(), json!(revision));
        values.insert("updated_at".to_string(), json!(&attempt.committed_at));
        values.insert(
            "person_ref_ciphertext_ref".to_string(),
            json!(next_reference.encrypted_blob_ref),
        );
        values.insert(
            "person_ref_ciphertext_sha256".to_string(),
            json!(next_reference.ciphertext_sha256),
        );
        values.insert(
            "person_ref_ciphertext_json".to_string(),
            json!(serde_json::to_string(&next_reference.envelope)
                .map_err(|_| "PEOPLE_LINK_SERIALIZATION_FAILED".to_string())?),
        );
        values.insert(
            "person_ref_key_ref".to_string(),
            json!(next_reference.envelope.key_ref),
        );
        mutations.push(upsert("speaker_identity_links", Value::Object(values)));
        zeroize::Zeroize::zeroize(&mut next_payload);
    } else {
        let mut values = row
            .as_object()
            .ok_or_else(|| "PEOPLE_LINK_ROW_MALFORMED".to_string())?
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("speaker_identity_links.")
                    .map(|column| (column.to_string(), value.clone()))
            })
            .collect::<serde_json::Map<String, Value>>();
        values.insert(
            "status".to_string(),
            json!(if action == "reject" {
                "rejected"
            } else {
                "revoked"
            }),
        );
        values.insert("revision".to_string(), json!(next_revision));
        values.insert("expected_revision".to_string(), json!(revision));
        values.insert("updated_at".to_string(), json!(&attempt.committed_at));
        if action == "unlink" {
            values.insert("revoked_at".to_string(), json!(&attempt.committed_at));
        }
        mutations.push(upsert("speaker_identity_links", Value::Object(values)));
    }
    commit_rows_at_frontier(storage, attempt.expected_frontier, mutations)
}

fn reference_from_identity_row(row: &Value) -> Result<PrivateIdentityReference, String> {
    let encrypted_blob_ref = string(row, "speaker_identity_links.person_ref_ciphertext_ref")?;
    let ciphertext_sha256 = string(row, "speaker_identity_links.person_ref_ciphertext_sha256")?;
    let envelope_json = string(row, "speaker_identity_links.person_ref_ciphertext_json")?;
    let envelope = serde_json::from_str(&envelope_json)
        .map_err(|_| "identity envelope persistence is malformed".to_string())?;
    let reference = PrivateIdentityReference {
        encrypted_blob_ref,
        ciphertext_sha256,
        envelope,
    };
    meeting_intelligence_schema::validate_private_identity_reference(&reference)?;
    if let Some(row_key_ref) =
        optional_row_value(row, "speaker_identity_links.person_ref_key_ref").and_then(Value::as_str)
    {
        if row_key_ref != reference.envelope.key_ref {
            return Err("identity key reference persistence mismatch".to_string());
        }
    } else {
        return Err("identity key reference is missing".to_string());
    }
    Ok(reference)
}

fn opaque_reference_payload(reference: Option<&PrivateIdentityReference>) -> Result<Value, String> {
    reference
        .map(|reference| {
            meeting_intelligence_schema::validate_private_identity_reference(reference)?;
            Ok(json!({
                "encrypted_blob_ref": reference.encrypted_blob_ref,
                "ciphertext_sha256": reference.ciphertext_sha256,
            }))
        })
        .transpose()
        .map(|value| value.unwrap_or(Value::Null))
}

fn opaque_attribution_payload(attribution: &ParticipantAttribution) -> Result<Value, String> {
    Ok(json!({
        "kind": attribution.kind,
        "participant_session_id": attribution.participant_session_id,
        "speaker_cluster_id": attribution.speaker_cluster_id,
        "label_snapshot_ref": attribution.label_snapshot_ref,
        "provider_ref_ciphertext": opaque_reference_payload(
            attribution.provider_ref_ciphertext.as_ref(),
        )?,
        "identity_link_id": attribution.identity_link_id,
        "identity_expected_revision": attribution.identity_expected_revision,
        "person_ref_ciphertext": opaque_reference_payload(
            attribution.person_ref_ciphertext.as_ref(),
        )?,
        "evidence_revision": attribution.evidence_revision,
    }))
}

fn check_identity_link_scope(
    storage: &Storage,
    request: &AtomicMeetingRequest,
    trusted: Option<&TrustedIdentityContext>,
    key_backend: &dyn IdentityKeyBackend,
) -> Result<Option<AuthorizedPerson>, String> {
    let Some(link_id) = request.attribution.identity_link_id.as_deref() else {
        return Ok(None);
    };
    let trusted = trusted.ok_or_else(|| "trusted identity context is required".to_string())?;
    let row = query(
        storage,
        "speaker_identity_links",
        &[
            "project_id",
            "recording_id",
            "meeting_session_id",
            "speaker_id",
            "vault_id",
            "status",
            "revision",
            "model_run_id",
            "match_source",
            "evidence_ciphertext_ref",
            "evidence_ciphertext_sha256",
            "locked_at",
            "revoked_at",
            "person_ref_ciphertext_ref",
            "person_ref_ciphertext_sha256",
            "person_ref_ciphertext_json",
            "person_ref_key_ref",
        ],
        vec![eq("speaker_identity_links", "id", json!(link_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "identity link is not present in this local meeting scope".to_string())?;
    if string(&row, "speaker_identity_links.recording_id")? != request.scope.recording_id
        || string(&row, "speaker_identity_links.meeting_session_id")?
            != request.scope.meeting_session_id
        || string(&row, "speaker_identity_links.vault_id")? != trusted.vault_id
        || string(&row, "speaker_identity_links.status")? != "confirmed"
        || optional_row_value(&row, "speaker_identity_links.locked_at").is_some()
        || optional_row_value(&row, "speaker_identity_links.revoked_at").is_some()
    {
        return Err("identity link is outside the trusted active review scope".to_string());
    }
    let revision = integer(&row, "speaker_identity_links.revision")?;
    if request.attribution.identity_expected_revision != Some(revision)
        || request.attribution.evidence_revision != revision
    {
        return Err("identity link review revision is stale".to_string());
    }
    let vault = query(
        storage,
        "identity_vaults",
        &["owner_principal_ref", "bound_account_ref", "state"],
        vec![eq("identity_vaults", "id", json!(&trusted.vault_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "identity vault is missing".to_string())?;
    let bound_account_ref =
        optional_row_value(&vault, "identity_vaults.bound_account_ref").and_then(Value::as_str);
    if string(&vault, "identity_vaults.owner_principal_ref")? != trusted.owner_principal_ref
        || bound_account_ref != trusted.account_ref.as_deref()
        || string(&vault, "identity_vaults.state")? != "active"
    {
        return Err("identity vault is locked, revoked, or outside the trusted owner".to_string());
    }
    let stored = reference_from_identity_row(&row)?;
    if let Some(claimed) = request.attribution.person_ref_ciphertext.as_ref() {
        if claimed != &stored {
            return Err(
                "caller identity ciphertext claim does not match native custody".to_string(),
            );
        }
    }
    let match_source = string(&row, "speaker_identity_links.match_source")?;
    let model_context = if match_source == "manual_review" {
        let speaker_id = string(&row, "speaker_identity_links.speaker_id")?;
        let (cursor, digest) = parse_evidence_reference(&string(
            &row,
            "speaker_identity_links.evidence_ciphertext_ref",
        )?)?;
        let stored_digest = string(&row, "speaker_identity_links.evidence_ciphertext_sha256")?;
        let (current_cursor, current_digest) =
            current_speaker_evidence(storage, &request.scope, &speaker_id)?;
        if cursor != current_cursor || digest != current_digest || stored_digest != digest {
            return Err("identity link evidence revision is stale".to_string());
        }
        manual_identity_model_context(&speaker_id, &digest)
    } else {
        optional_row_value(&row, "speaker_identity_links.model_run_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "none".to_string())
    };
    let context = IdentityAadContext {
        account_ref: trusted.account_ref.clone(),
        scope: identity_scope_key(&request.scope),
        vault_id: trusted.vault_id.clone(),
        entity_id: link_id.to_string(),
        revision,
        model_context,
    };
    let person = open_person_identity(&stored, &context, link_id, revision, key_backend)?;
    let profile = find_people_profile(storage, trusted, &person.profile_id)?
        .ok_or_else(|| "identity profile is missing".to_string())?;
    if string(&profile, "participant_profiles.status")? != "active"
        || integer(&profile, "participant_profiles.revision")? != person.profile_revision
    {
        return Err("identity profile is not eligible for the trusted scope".to_string());
    }
    let profile_payload = read_people_profile_payload(storage, trusted, &profile, key_backend)?;
    if profile_payload.person_id != person.person_id
        || profile_payload.display_name != person.display_name
    {
        return Err("identity link does not match the current profile revision".to_string());
    }
    Ok(Some(person))
}

/// Commit one finalized source/revision pair and all its local projections as
/// one guarded Genesis transaction. No event is returned until the commit call
/// succeeds; callers must treat an uncertainty as a reconciliation boundary.
pub(crate) fn commit_meeting_transcript(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
) -> Result<CommittedMeetingEvent, String> {
    if request.attribution.identity_link_id.is_some() {
        return Err("explicit native local-owner unlock is required".to_string());
    }
    let key_backend = OsPeopleMetadataKeyBackend;
    commit_meeting_transcript_with_backend_and_guard(
        storage,
        attempt,
        request,
        None,
        &key_backend,
        None,
        None,
    )
}

pub(crate) fn commit_meeting_transcript_with_unlock(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
) -> Result<CommittedMeetingEvent, String> {
    if request.attribution.identity_link_id.is_none() {
        return commit_meeting_transcript(storage, attempt, request);
    }
    let native_capture = capture_native_identity_context(storage, session)?;
    let lifecycle_source = NativeLifecycleWitnessSource;
    let trusted = Some(&native_capture.context);
    let account_guard = native_capture.account_guard.as_ref();
    commit_meeting_transcript_with_backend_and_guard(
        storage,
        attempt,
        request,
        trusted,
        &OsPeopleMetadataKeyBackend,
        account_guard,
        Some((session, &lifecycle_source)),
    )
}

fn commit_meeting_transcript_with_backend(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
    trusted: Option<&TrustedIdentityContext>,
    key_backend: &dyn IdentityKeyBackend,
) -> Result<CommittedMeetingEvent, String> {
    commit_meeting_transcript_with_backend_and_guard(
        storage,
        attempt,
        request,
        trusted,
        key_backend,
        None,
        None,
    )
}

#[cfg(test)]
struct CommitTestHooks<'a> {
    before_broker_fence: Option<&'a dyn Fn()>,
    vault_fence: Option<&'a dyn Fn()>,
    broker_fence: Option<&'a dyn Fn()>,
}

#[cfg(not(test))]
struct CommitTestHooks<'a>(std::marker::PhantomData<&'a ()>);

impl<'a> CommitTestHooks<'a> {
    fn none() -> Self {
        #[cfg(test)]
        {
            Self {
                before_broker_fence: None,
                vault_fence: None,
                broker_fence: None,
            }
        }
        #[cfg(not(test))]
        {
            Self(std::marker::PhantomData)
        }
    }

    fn before_broker_fence(&self) {
        #[cfg(test)]
        if let Some(hook) = self.before_broker_fence {
            hook();
        }
    }

    fn vault_fence(&self) {
        #[cfg(test)]
        if let Some(hook) = self.vault_fence {
            hook();
        }
    }

    fn broker_fence(&self) {
        #[cfg(test)]
        if let Some(hook) = self.broker_fence {
            hook();
        }
    }
}

#[cfg(test)]
fn commit_meeting_transcript_with_test_unlock_and_backend(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
    key_backend: &dyn IdentityKeyBackend,
    lifecycle_source: &dyn LifecycleWitnessSource,
    before_broker_fence_hook: Option<&dyn Fn()>,
) -> Result<CommittedMeetingEvent, String> {
    commit_meeting_transcript_with_test_unlock_and_backend_options(
        storage,
        session,
        attempt,
        request,
        key_backend,
        lifecycle_source,
        None,
        before_broker_fence_hook,
        None,
        None,
    )
}

#[cfg(test)]
fn commit_meeting_transcript_with_test_unlock_and_backend_options(
    storage: &Storage,
    session: &NativeOwnerUnlockSession,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
    key_backend: &dyn IdentityKeyBackend,
    lifecycle_source: &dyn LifecycleWitnessSource,
    account_guard: Option<crate::auth_session::AccountOperationGuard>,
    before_broker_fence_hook: Option<&dyn Fn()>,
    vault_fence_hook: Option<&dyn Fn()>,
    broker_fence_hook: Option<&dyn Fn()>,
) -> Result<CommittedMeetingEvent, String> {
    let mut owned_broker = if account_guard.is_none() {
        let supplied_witness = lifecycle_source.read()?;
        if supplied_witness != session.account_witness
            || supplied_witness.state != "signed_out"
            || supplied_witness.native_user_id.is_some()
        {
            return Err(
                "test protected identity commit requires a same-broker account guard".to_string(),
            );
        }
        Some(crate::auth_session::tests::TestAccountOperationHarness::new())
    } else {
        None
    };
    let effective_guard = match account_guard {
        Some(guard) => Some(guard),
        None => owned_broker.as_mut().map(|harness| harness.take_guard()),
    };
    let native_capture = capture_native_identity_context_with_source(
        storage,
        session,
        lifecycle_source,
        effective_guard,
    )?;
    let trusted = Some(&native_capture.context);
    commit_meeting_transcript_with_backend_and_guard_impl(
        storage,
        attempt,
        request,
        trusted,
        key_backend,
        native_capture.account_guard.as_ref(),
        Some((session, lifecycle_source)),
        CommitTestHooks {
            before_broker_fence: before_broker_fence_hook,
            vault_fence: vault_fence_hook,
            broker_fence: broker_fence_hook,
        },
    )
}

fn commit_meeting_transcript_with_backend_and_guard(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
    trusted: Option<&TrustedIdentityContext>,
    key_backend: &dyn IdentityKeyBackend,
    account_guard: Option<&crate::auth_session::AccountOperationGuard>,
    unlock: Option<(&NativeOwnerUnlockSession, &dyn LifecycleWitnessSource)>,
) -> Result<CommittedMeetingEvent, String> {
    commit_meeting_transcript_with_backend_and_guard_impl(
        storage,
        attempt,
        request,
        trusted,
        key_backend,
        account_guard,
        unlock,
        CommitTestHooks::none(),
    )
}

fn meeting_commit_error(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.contains("expected frontier conflict") {
        format!("MEETING_COMMIT_REJECTED: {message}")
    } else {
        format!(
            "MEETING_COMMIT_UNCERTAIN: preserve transaction_id, expected_frontier, and committed_at for reopen/reconcile; {message}"
        )
    }
}

fn commit_meeting_transcript_with_backend_and_guard_impl(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &AtomicMeetingRequest,
    trusted: Option<&TrustedIdentityContext>,
    key_backend: &dyn IdentityKeyBackend,
    account_guard: Option<&crate::auth_session::AccountOperationGuard>,
    unlock: Option<(&NativeOwnerUnlockSession, &dyn LifecycleWitnessSource)>,
    test_hooks: CommitTestHooks<'_>,
) -> Result<CommittedMeetingEvent, String> {
    if unlock.is_some() && account_guard.is_none() {
        return Err("native account operation is unavailable".to_string());
    }
    request.validate()?;
    if let Some((session, lifecycle_source)) = unlock {
        if trusted != Some(session.context()) {
            return Err("trusted identity context is not backed by native unlock".to_string());
        }
        revalidate_native_local_owner_session(storage, session, lifecycle_source)?;
    } else if request.attribution.identity_link_id.is_some() {
        return Err("explicit native local-owner unlock is required".to_string());
    }
    require_meeting_scope(storage, &request.scope)?;
    let verified = request
        .sources
        .iter()
        .map(|source| verify_audio_coverage(storage, &request.scope, source))
        .collect::<Result<Vec<_>, _>>()?;
    check_coverage_conflicts(
        storage,
        &request.scope,
        &verified,
        request.revision.start_ms,
        request.revision.end_ms,
    )?;
    check_knowledge_scope(storage, request)?;
    let _authorized_person = check_identity_link_scope(storage, request, trusted, key_backend)?;

    let coverage = verified.iter().map(coverage_payload).collect::<Vec<_>>();
    let attribution = opaque_attribution_payload(&request.attribution)?;
    let knowledge = request
        .knowledge
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| error.to_string())?;
    let event_payload = json!({
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "event_type": "transcript_revision_committed",
        "event_id": request.event_id,
        "scope": request.scope,
        "source_cursor": request.source_cursor,
        "committed_cursor": request.committed_cursor,
        "coverage": coverage,
        "revision": request.revision,
        "attribution": attribution,
        "knowledge": knowledge,
    });
    let payload_hash = canonical_sha256(&event_payload)?;
    if let Some((existing_transaction_id, existing_hash, cursor, revision_id)) =
        query_existing_event(storage, &request.event_id)?
    {
        if existing_hash == payload_hash {
            return Ok(CommittedMeetingEvent {
                contract_version: meeting_intelligence_schema::CONTRACT_VERSION,
                event_id: request.event_id.clone(),
                transaction_id: existing_transaction_id,
                cursor,
                revision_id,
                payload_hash,
                commit_sequence: None,
                idempotent: true,
            });
        }
        return Err("EVENT_ID_CONFLICT: event identity reused with changed payload".to_string());
    }

    check_revision_conflicts(storage, request)?;
    let cursor_id = source_cursor_id(&request.scope);
    let prior_cursor = query(
        storage,
        "meeting_source_cursors",
        &["last_sequence", "last_event_cursor"],
        vec![eq("meeting_source_cursors", "id", json!(&cursor_id))],
        1,
    )?
    .into_iter()
    .next();
    let previous_sequence = prior_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_source_cursors.last_sequence"))
        .transpose()?
        .unwrap_or(-1);
    let previous_event_cursor = prior_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_source_cursors.last_event_cursor"))
        .transpose()?
        .unwrap_or(-1);
    let first_sequence = verified
        .iter()
        .map(|source| source.sequence_no)
        .min()
        .ok_or_else(|| "no verified source coverage".to_string())?;
    if first_sequence != previous_sequence + 1
        || request.source_cursor
            != verified
                .iter()
                .map(|source| source.sequence_no)
                .max()
                .unwrap()
        || request.committed_cursor != previous_event_cursor + 1
    {
        return Err("SOURCE_CURSOR_CONFLICT: source cursor is stale or has a gap".to_string());
    }

    let mut mutations = Vec::new();
    for source in &verified {
        let source_payload = coverage_payload(source);
        mutations.push(upsert(
            "meeting_source_coverage",
            json!({
                "id": source.id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": source.source_session_id,
                "track_id": source.track_id,
                "source_generation": source.source_generation,
                "sequence_no": source.sequence_no,
                "start_ms": source.start_ms,
                "end_ms": source.end_ms,
                "coverage_kind": source.kind.as_str(),
                "audio_chunk_id": source.audio_chunk_id,
                "file_path": source.file_path,
                "byte_size": source.byte_size,
                "checksum": source.checksum,
                "gap_reason": source.gap_reason,
                "payload_hash": canonical_sha256(&source_payload)?,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "finalized_at": attempt.committed_at,
                "created_at": attempt.committed_at,
            }),
        ));
    }
    let audio_refs = verified
        .iter()
        .filter(|source| source.kind == SourceCoverageKind::Audio)
        .map(|source| source.id.clone())
        .collect::<Vec<_>>();
    mutations.push(upsert(
        "transcript_revisions",
        json!({
            "id": request.revision.id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "source_session_id": request.scope.source_session_id,
            "utterance_id": request.revision.utterance_id,
            "revision": request.revision.revision,
            "supersedes_revision": request.revision.supersedes_revision,
            "expected_revision": request.revision.expected_revision,
            "state": "committed",
            "origin": request.revision.origin.as_str(),
            "raw_text": request.revision.raw_text,
            "effective_text": request.revision.effective_text,
            "language": request.revision.language,
            "confidence": request.revision.confidence,
            "start_ms": request.revision.start_ms,
            "end_ms": request.revision.end_ms,
            "audio_refs_json": audio_refs,
            "attribution_json": attribution,
            "model_run_id": request.revision.model_run_id,
            "review_state": request.revision.review_state,
            "payload_hash": payload_hash,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "created_at": attempt.committed_at,
        }),
    ));
    mutations.push(upsert(
        "transcript_projection",
        json!({
            "id": request.revision.utterance_id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "utterance_id": request.revision.utterance_id,
            "revision_id": request.revision.id,
            "revision": request.revision.revision,
            "state": "committed",
            "effective_text": request.revision.effective_text,
            "language": request.revision.language,
            "confidence": request.revision.confidence,
            "review_state": request.revision.review_state,
            "source_event_id": request.event_id,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "updated_at": attempt.committed_at,
        }),
    ));
    mutations.push(upsert(
        "transcript_event_log",
        json!({
            "id": request.event_id,
            "transaction_id": attempt.transaction_id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "source_session_id": request.scope.source_session_id,
            "source_generation": request.scope.source_generation,
            "cursor": request.committed_cursor,
            "event_type": "transcript_revision_committed",
            "revision_id": request.revision.id,
            "payload_json": event_payload,
            "payload_hash": payload_hash,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "committed_at": attempt.committed_at,
        }),
    ));
    mutations.push(upsert(
        "meeting_source_cursors",
        json!({
            "id": cursor_id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "source_session_id": request.scope.source_session_id,
            "track_id": request.scope.track_id,
            "source_generation": request.scope.source_generation,
            "last_sequence": request.source_cursor,
            "last_end_ms": verified.iter().map(|source| source.end_ms).max().unwrap(),
            "last_event_cursor": request.committed_cursor,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "updated_at": attempt.committed_at,
        }),
    ));
    if let Some(knowledge) = &request.knowledge {
        let evidence_bundle_id = knowledge
            .evidence_bundle_id
            .clone()
            .unwrap_or_else(|| format!("meeting-bundle::{}", request.event_id));
        mutations.push(upsert(
            "knowledge_evidence_bundles",
            json!({
                "id": evidence_bundle_id,
                "project_id": request.scope.project_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "query_ref": knowledge.id,
                "trigger_ref": request.event_id,
                "policy_snapshot_json": {
                    "read_grant_id": knowledge.read_grant_id,
                    "share_grant_id": knowledge.share_grant_id,
                    "audience_policy_revision": knowledge.audience_policy_revision,
                },
                "acl_snapshot_json": {
                    "read_state": knowledge.read_state,
                    "share_state": knowledge.share_state,
                },
                "selected_refs_json": [{
                    "collection_id": knowledge.collection_id,
                    "document_id": knowledge.document_id,
                    "document_version_id": knowledge.document_version_id,
                    "source_version": knowledge.source_version,
                    "citation": knowledge.citation,
                }],
                "output_hash": payload_hash,
                "state": "draft",
                "share_state": knowledge.share_state,
                "expires_at": null,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": attempt.committed_at,
            }),
        ));
    }

    let _operation_fence = if let Some((session, lifecycle_source)) = unlock {
        let operation_fence = session.begin_operation_fence()?;
        revalidate_native_local_owner_session_under_fence(storage, session, lifecycle_source)?;
        let _authorized_person =
            check_identity_link_scope(storage, request, Some(session.context()), key_backend)?;
        Some(operation_fence)
    } else {
        None
    };
    if let Some(account_guard) = account_guard {
        account_guard
            .check()
            .map_err(|_| "native account operation was invalidated".to_string())?;
    }

    // The VAULT read fence is held here, retaining the R2 lock/revoke control.
    // The broker-boundary hook is the last point before acquiring the
    // registered broker lifecycle lock.
    test_hooks.vault_fence();
    test_hooks.before_broker_fence();
    let mut transaction = Some(GenesisTransaction {
        transaction_id: attempt.transaction_id.clone(),
        expected_frontier: Some(attempt.expected_frontier),
        relational: vec![RelationalMutationGroup {
            namespace: NAMESPACE.to_string(),
            mutations,
        }],
        graph: BatchInput {
            nodes: vec![],
            edges: vec![],
        },
        vectors: vec![],
    });
    let commit = if let Some((session, _lifecycle_source)) = unlock {
        let expected_witness = session.account_witness.clone();
        let account_guard =
            account_guard.ok_or_else(|| "native account operation is unavailable".to_string())?;
        let mut commit_result = None;
        let commit_operation = || -> Result<(), String> {
            // No broker entry is made here. The callback only records the
            // test ordering signal and performs the already-prepared Genesis
            // transaction while the same broker lock remains held.
            test_hooks.broker_fence();
            match storage
                .commit_transaction(
                    transaction
                        .take()
                        .expect("account commit operation was invoked more than once"),
                )
                .map_err(meeting_commit_error)
            {
                Ok(commit) => {
                    commit_result = Some(Ok(commit));
                    Ok(())
                }
                Err(error) => {
                    commit_result = Some(Err(error.clone()));
                    Err(error)
                }
            }
        };
        let fence_result =
            account_guard.with_account_lifecycle_fence(&expected_witness, commit_operation);
        match fence_result {
            Ok(()) => commit_result
                .expect("account commit fence returned without executing the commit")?,
            Err(_fence_error) => match commit_result {
                Some(result) => result?,
                None => return Err("native account operation was invalidated".to_string()),
            },
        }
    } else {
        storage
            .commit_transaction(
                transaction
                    .take()
                    .expect("anonymous commit operation was not prepared"),
            )
            .map_err(meeting_commit_error)?
    };
    Ok(CommittedMeetingEvent {
        contract_version: meeting_intelligence_schema::CONTRACT_VERSION,
        event_id: request.event_id.clone(),
        transaction_id: attempt.transaction_id.clone(),
        cursor: request.committed_cursor,
        revision_id: request.revision.id.clone(),
        payload_hash,
        commit_sequence: Some(commit.commit_sequence),
        idempotent: false,
    })
}

/// Commits one finalized audio/gap batch, zero or more local ASR revisions,
/// recording-wide ordered events, source progress, and chunk completion in a
/// single Genesis transaction. The stable operation ID is the reconciliation
/// key after an uncertain commit.
pub(crate) fn commit_meeting_ingest_batch(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &MeetingIngestBatchRequest,
) -> Result<MeetingIngestBatchResult, String> {
    request.validate()?;
    let payload_hash = canonical_sha256(request)?;
    if let Some(existing) = query(
        storage,
        "meeting_input_batches",
        &["payload_hash", "transaction_id", "result_json"],
        vec![eq(
            "meeting_input_batches",
            "id",
            json!(&request.operation_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    {
        if string(&existing, "meeting_input_batches.payload_hash")? != payload_hash {
            return Err(
                "MEETING_BATCH_ID_CONFLICT: operation identity reused with changed payload"
                    .to_string(),
            );
        }
        let mut result: MeetingIngestBatchResult = serde_json::from_value(
            existing
                .get("meeting_input_batches.result_json")
                .cloned()
                .ok_or_else(|| "committed batch result is unavailable".to_string())?,
        )
        .map_err(|_| "committed batch result is invalid".to_string())?;
        result.transaction_id = string(&existing, "meeting_input_batches.transaction_id")?;
        result.idempotent = true;
        return Ok(result);
    }

    require_meeting_scope(storage, &request.scope)?;
    let mut new_verified = request
        .sources
        .iter()
        .map(|source| verify_audio_coverage(storage, &request.scope, source))
        .collect::<Result<Vec<_>, _>>()?;
    new_verified.sort_by_key(|source| source.sequence_no);
    check_new_coverage_conflicts(storage, &request.scope, &new_verified)?;
    let first_sequence = new_verified
        .first()
        .map(|source| source.sequence_no)
        .ok_or_else(|| "meeting input batch has no source coverage".to_string())?;
    let last_sequence = new_verified
        .last()
        .map(|source| source.sequence_no)
        .unwrap_or(-1);
    let source_id = source_cursor_id(&request.scope);
    let source_cursor = query(
        storage,
        "meeting_source_cursors",
        &["last_sequence", "last_end_ms"],
        vec![eq("meeting_source_cursors", "id", json!(&source_id))],
        1,
    )?
    .into_iter()
    .next();
    let previous_sequence = source_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_source_cursors.last_sequence"))
        .transpose()?
        .unwrap_or(-1);
    if first_sequence != previous_sequence + 1 {
        return Err("SOURCE_CURSOR_CONFLICT: batch sequence is stale or has a gap".to_string());
    }

    let recording_cursor_id = format!("recording::{}", request.scope.recording_id);
    let recording_cursor = query(
        storage,
        "meeting_recording_cursors",
        &[
            "last_cursor",
            "minimum_retained_cursor",
            "revision",
            "adoption_complete",
        ],
        vec![eq(
            "meeting_recording_cursors",
            "id",
            json!(&recording_cursor_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    if recording_cursor
        .as_ref()
        .and_then(|row| row.get("meeting_recording_cursors.adoption_complete"))
        .and_then(Value::as_bool)
        == Some(false)
    {
        return Err(
            "MEETING_STREAM_NOT_ADOPTED: reopen storage to finish v12 stream adoption".to_string(),
        );
    }
    let previous_event_cursor = recording_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.last_cursor"))
        .transpose()?
        .unwrap_or(-1);
    let first_cursor = previous_event_cursor
        .checked_add(1)
        .ok_or_else(|| "MEETING_CURSOR_OVERFLOW".to_string())?;
    let event_count = i64::try_from(request.revisions.len().max(1))
        .map_err(|_| "meeting event count overflow".to_string())?;
    let last_cursor = first_cursor
        .checked_add(event_count - 1)
        .ok_or_else(|| "MEETING_CURSOR_OVERFLOW".to_string())?;

    let mut verified_by_id = new_verified
        .iter()
        .cloned()
        .map(|source| (source.id.clone(), source))
        .collect::<HashMap<_, _>>();
    let mut revisions = request.revisions.clone();
    revisions.sort_by(|left, right| {
        left.start_ms
            .cmp(&right.start_ms)
            .then_with(|| left.utterance_id.cmp(&right.utterance_id))
    });
    let attribution = ParticipantAttribution {
        kind: "unknown_source".to_string(),
        participant_session_id: None,
        speaker_cluster_id: Some("unknown".to_string()),
        label_snapshot_ref: Some("unknown".to_string()),
        provider_ref_ciphertext: None,
        identity_link_id: None,
        identity_expected_revision: None,
        person_ref_ciphertext: None,
        evidence_revision: 0,
    };
    let opaque_attribution = opaque_attribution_payload(&attribution)?;
    let new_coverage_values = new_verified
        .iter()
        .map(coverage_payload)
        .collect::<Vec<_>>();
    let mut mutations = Vec::new();
    for source in &new_verified {
        let source_payload = coverage_payload(source);
        mutations.push(upsert(
            "meeting_source_coverage",
            json!({
                "id": source.id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": source.source_session_id,
                "track_id": source.track_id,
                "source_generation": source.source_generation,
                "sequence_no": source.sequence_no,
                "start_ms": source.start_ms,
                "end_ms": source.end_ms,
                "coverage_kind": source.kind.as_str(),
                "audio_chunk_id": source.audio_chunk_id,
                "file_path": source.file_path,
                "byte_size": source.byte_size,
                "checksum": source.checksum,
                "gap_reason": source.gap_reason,
                "payload_hash": canonical_sha256(&source_payload)?,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "finalized_at": attempt.committed_at,
                "created_at": attempt.committed_at,
            }),
        ));
    }

    let mut event_ids = Vec::new();
    let mut revision_ids = Vec::new();
    if revisions.is_empty() {
        let event_id = format!("mi-control::{}", request.operation_id);
        let event_type = if new_verified
            .iter()
            .any(|source| source.kind == SourceCoverageKind::Gap)
        {
            "source_gap"
        } else {
            "audio_processed_no_utterance"
        };
        let payload = json!({
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "event_type": event_type,
            "operation_id": request.operation_id,
            "scope": request.scope,
            "first_sequence": first_sequence,
            "last_sequence": last_sequence,
            "coverage": new_coverage_values,
            "outcome": if event_type == "source_gap" { "gap_recorded" } else { "processed_no_utterance" },
        });
        let hash = canonical_sha256(&payload)?;
        mutations.push(upsert(
            "meeting_control_events",
            json!({
                "id": event_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "source_generation": request.scope.source_generation,
                "cursor": first_cursor,
                "event_type": event_type,
                "batch_id": request.operation_id,
                "revision_id": null,
                "payload_json": payload,
                "payload_hash": hash,
                "transaction_id": attempt.transaction_id,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "committed_at": attempt.committed_at,
            }),
        ));
        event_ids.push(format!("mi-control::{}", request.operation_id));
    }

    for (index, revision) in revisions.iter().enumerate() {
        let binding = request
            .revision_coverage
            .iter()
            .find(|binding| binding.revision_id == revision.id)
            .ok_or_else(|| "transcript revision source binding is missing".to_string())?;
        let mut revision_coverage = Vec::new();
        for coverage_id in &binding.coverage_ids {
            let source = match verified_by_id.get(coverage_id) {
                Some(source) => source.clone(),
                None => {
                    let source = load_committed_coverage(storage, &request.scope, coverage_id)?;
                    verified_by_id.insert(coverage_id.clone(), source.clone());
                    source
                }
            };
            revision_coverage.push(source);
        }
        revision_coverage.sort_by_key(|source| source.sequence_no);
        let audio_spans = revision_coverage
            .iter()
            .filter(|source| source.kind == SourceCoverageKind::Audio)
            .map(|source| (source.start_ms, source.end_ms))
            .collect::<Vec<_>>();
        if !audio_range_is_covered(&audio_spans, revision.start_ms, revision.end_ms) {
            return Err(
                "transcript revision is not covered by its bound durable audio".to_string(),
            );
        }
        let compatibility_event_id = format!("meeting-revision-event::{}", revision.id);
        if query_existing_event(storage, &compatibility_event_id)?.is_some() {
            return Err(
                "MEETING_EVENT_ID_CONFLICT: transcript event identity already exists".to_string(),
            );
        }
        check_coverage_conflicts(
            storage,
            &request.scope,
            &revision_coverage,
            revision.start_ms,
            revision.end_ms,
        )?;
        let revision_request = AtomicMeetingRequest {
            event_id: compatibility_event_id.clone(),
            source_cursor: last_sequence,
            committed_cursor: first_cursor + index as i64,
            scope: request.scope.clone(),
            sources: request.sources.clone(),
            revision: revision.clone(),
            attribution: attribution.clone(),
            knowledge: None,
        };
        check_revision_conflicts(storage, &revision_request)?;

        let revision_payload = json!({
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "event_type": "transcript_revision_committed",
            "event_id": compatibility_event_id,
            "operation_id": request.operation_id,
            "scope": request.scope,
            "source_cursor": last_sequence,
            "committed_cursor": first_cursor + index as i64,
            "coverage": revision_coverage.iter().map(coverage_payload).collect::<Vec<_>>(),
            "revision": revision,
            "attribution": opaque_attribution,
        });
        let revision_hash = canonical_sha256(&revision_payload)?;
        let control_event_id = format!(
            "mi-control::{}",
            Sha256::digest(
                format!(
                    "{}:cursor:{}",
                    request.operation_id,
                    first_cursor + index as i64
                )
                .as_bytes()
            )[..16]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        mutations.push(upsert(
            "transcript_revisions",
            json!({
                "id": revision.id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "utterance_id": revision.utterance_id,
                "revision": revision.revision,
                "supersedes_revision": revision.supersedes_revision,
                "expected_revision": revision.expected_revision,
                "state": "committed",
                "origin": revision.origin.as_str(),
                "raw_text": revision.raw_text,
                "effective_text": revision.effective_text,
                "language": revision.language,
                "confidence": revision.confidence,
                "start_ms": revision.start_ms,
                "end_ms": revision.end_ms,
                "audio_refs_json": revision_coverage.iter()
                    .filter(|source| source.kind == SourceCoverageKind::Audio)
                    .map(|source| source.id.clone()).collect::<Vec<_>>(),
                "attribution_json": opaque_attribution,
                "model_run_id": revision.model_run_id,
                "review_state": "unreviewed",
                "payload_hash": revision_hash,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": attempt.committed_at,
            }),
        ));
        mutations.push(upsert(
            "transcript_projection",
            json!({
                "id": revision.utterance_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "utterance_id": revision.utterance_id,
                "revision_id": revision.id,
                "revision": revision.revision,
                "state": "committed",
                "effective_text": revision.effective_text,
                "language": revision.language,
                "confidence": revision.confidence,
                "review_state": "unreviewed",
                "source_event_id": compatibility_event_id,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "updated_at": attempt.committed_at,
            }),
        ));
        let existing_legacy_segment = query(
            storage,
            "transcript_segments",
            &["created_at"],
            vec![eq(
                "transcript_segments",
                "id",
                json!(&revision.utterance_id),
            )],
            1,
        )?
        .into_iter()
        .next();
        mutations.push(upsert(
            "transcript_segments",
            json!({
                "id": revision.utterance_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "speaker_id": null,
                "start_ms": revision.start_ms,
                "end_ms": revision.end_ms,
                "text": revision.effective_text,
                "confidence": revision.confidence,
                "created_at": existing_legacy_segment
                    .as_ref()
                    .and_then(|row| row.get("transcript_segments.created_at"))
                    .cloned()
                    .unwrap_or_else(|| json!(attempt.committed_at)),
                "updated_at": attempt.committed_at,
            }),
        ));
        mutations.push(upsert(
            "transcript_event_log",
            json!({
                "id": compatibility_event_id,
                "transaction_id": attempt.transaction_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "source_generation": request.scope.source_generation,
                "cursor": first_cursor + index as i64,
                "event_type": "transcript_revision_committed",
                "revision_id": revision.id,
                "payload_json": revision_payload,
                "payload_hash": revision_hash,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "committed_at": attempt.committed_at,
            }),
        ));
        mutations.push(upsert(
            "meeting_control_events",
            json!({
                "id": control_event_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "source_generation": request.scope.source_generation,
                "cursor": first_cursor + index as i64,
                "event_type": "transcript_revision_committed",
                "batch_id": request.operation_id,
                "revision_id": revision.id,
                "payload_json": revision_payload,
                "payload_hash": revision_hash,
                "transaction_id": attempt.transaction_id,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "committed_at": attempt.committed_at,
            }),
        ));
        event_ids.push(control_event_id);
        revision_ids.push(revision.id.clone());
    }

    let result = MeetingIngestBatchResult {
        contract_version: meeting_intelligence_schema::CONTRACT_VERSION,
        operation_id: request.operation_id.clone(),
        transaction_id: attempt.transaction_id.clone(),
        payload_hash: payload_hash.clone(),
        first_cursor,
        last_cursor,
        event_ids,
        revision_ids,
        idempotent: false,
    };
    let batch_mutation = upsert(
        "meeting_input_batches",
        json!({
            "id": request.operation_id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "source_session_id": request.scope.source_session_id,
            "source_generation": request.scope.source_generation,
            "first_sequence": first_sequence,
            "last_sequence": last_sequence,
            "operation_id": request.operation_id,
            "payload_hash": payload_hash,
            "first_cursor": first_cursor,
            "last_cursor": last_cursor,
            "result_json": serde_json::to_value(&result).map_err(|error| error.to_string())?,
            "transaction_id": attempt.transaction_id,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "committed_at": attempt.committed_at,
        }),
    );
    // Control events reference this row, so put the batch before those rows in
    // the Genesis mutation sequence even though its result is assembled last.
    mutations.insert(new_verified.len(), batch_mutation);
    let max_end_ms = new_verified
        .iter()
        .map(|source| source.end_ms)
        .max()
        .unwrap_or(0);
    mutations.push(upsert(
        "meeting_source_cursors",
        json!({
            "id": source_id,
            "project_id": request.scope.project_id,
            "recording_id": request.scope.recording_id,
            "meeting_session_id": request.scope.meeting_session_id,
            "source_session_id": request.scope.source_session_id,
            "track_id": request.scope.track_id,
            "source_generation": request.scope.source_generation,
            "last_sequence": last_sequence,
            "last_end_ms": max_end_ms,
            "last_event_cursor": last_cursor,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "updated_at": attempt.committed_at,
        }),
    ));
    let next_recording_revision = recording_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.revision"))
        .transpose()?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "meeting recording cursor revision overflow".to_string())?;
    let minimum_retained_cursor = recording_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.minimum_retained_cursor"))
        .transpose()?
        .unwrap_or(0);
    mutations.push(upsert(
        "meeting_recording_cursors",
        json!({
            "id": recording_cursor_id,
            "recording_id": request.scope.recording_id,
            "last_cursor": last_cursor,
            "minimum_retained_cursor": minimum_retained_cursor,
            "revision": next_recording_revision,
            "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
            "adoption_complete": true,
            "updated_at": attempt.committed_at,
        }),
    ));

    for source in new_verified
        .iter()
        .filter(|source| source.kind == SourceCoverageKind::Audio)
    {
        let chunk_id = source
            .audio_chunk_id
            .as_deref()
            .ok_or_else(|| "verified audio chunk id missing".to_string())?;
        let row = query(
            storage,
            "audio_chunks",
            &[
                "recording_id",
                "sequence_no",
                "file_path",
                "start_ms",
                "end_ms",
                "byte_size",
                "checksum",
                "created_at",
                "transcribed_at",
            ],
            vec![eq("audio_chunks", "id", json!(chunk_id))],
            1,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "audio chunk disappeared before batch commit".to_string())?;
        mutations.push(upsert("audio_chunks", json!({
            "id": chunk_id,
            "recording_id": row.get("audio_chunks.recording_id").cloned().unwrap_or(Value::Null),
            "sequence_no": row.get("audio_chunks.sequence_no").cloned().unwrap_or(Value::Null),
            "file_path": row.get("audio_chunks.file_path").cloned().unwrap_or(Value::Null),
            "start_ms": row.get("audio_chunks.start_ms").cloned().unwrap_or(Value::Null),
            "end_ms": row.get("audio_chunks.end_ms").cloned().unwrap_or(Value::Null),
            "byte_size": row.get("audio_chunks.byte_size").cloned().unwrap_or(Value::Null),
            "checksum": row.get("audio_chunks.checksum").cloned().unwrap_or(Value::Null),
            "created_at": row.get("audio_chunks.created_at").cloned().unwrap_or(Value::Null),
            "transcribed_at": attempt.committed_at,
        })));
    }

    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: attempt.transaction_id.clone(),
            expected_frontier: Some(attempt.expected_frontier),
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations,
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map_err(meeting_commit_error)?;
    Ok(result)
}

/// Appends a revision against previously committed audio coverage. The source
/// cursor is untouched; only the utterance revision and recording-wide event
/// cursor advance.
pub(crate) fn revise_meeting_transcript(
    storage: &Storage,
    attempt: &MeetingCommitAttempt,
    request: &MeetingRevisionRequest,
) -> Result<CommittedMeetingEvent, String> {
    let revision = &request.revision;
    request.scope.validate()?;
    if revision.id.is_empty()
        || revision.utterance_id.is_empty()
        || revision.effective_text.trim().is_empty()
        || revision.effective_text.len() > 12_000
        || revision.origin != TranscriptOrigin::Human
        || revision.review_state != "reviewed"
        || revision.revision < 2
        || revision.expected_revision.is_none()
        || revision.supersedes_revision != revision.expected_revision
    {
        return Err("manual revision request is invalid".to_string());
    }
    let event_id = format!("meeting-revision-event::{}", revision.id);
    if let Some(existing) = query(
        storage,
        "transcript_event_log",
        &[
            "transaction_id",
            "payload_hash",
            "cursor",
            "revision_id",
            "payload_json",
        ],
        vec![eq("transcript_event_log", "id", json!(&event_id))],
        1,
    )?
    .into_iter()
    .next()
    {
        let payload = existing
            .get("transcript_event_log.payload_json")
            .cloned()
            .unwrap_or(Value::Null);
        if payload.get("revision")
            == Some(&serde_json::to_value(revision).map_err(|error| error.to_string())?)
            && payload.get("scope")
                == Some(&serde_json::to_value(&request.scope).map_err(|error| error.to_string())?)
        {
            return Ok(CommittedMeetingEvent {
                contract_version: meeting_intelligence_schema::CONTRACT_VERSION,
                event_id,
                transaction_id: string(&existing, "transcript_event_log.transaction_id")?,
                cursor: integer(&existing, "transcript_event_log.cursor")?,
                revision_id: string(&existing, "transcript_event_log.revision_id")?,
                payload_hash: string(&existing, "transcript_event_log.payload_hash")?,
                commit_sequence: None,
                idempotent: true,
            });
        }
        return Err(
            "MEETING_EVENT_ID_CONFLICT: revision identity reused with changed payload".to_string(),
        );
    }

    require_meeting_scope(storage, &request.scope)?;
    let projection = query(
        storage,
        "transcript_projection",
        &[
            "revision_id",
            "revision",
            "state",
            "effective_text",
            "review_state",
        ],
        vec![
            eq("transcript_projection", "id", json!(&revision.utterance_id)),
            eq(
                "transcript_projection",
                "meeting_session_id",
                json!(&request.scope.meeting_session_id),
            ),
        ],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "utterance is not committed in this meeting session".to_string())?;
    if string(&projection, "transcript_projection.state")? != "committed" {
        return Err("only a committed utterance can be corrected".to_string());
    }
    let current_revision_id = string(&projection, "transcript_projection.revision_id")?;
    let current = query(
        storage,
        "transcript_revisions",
        &[
            "source_session_id",
            "revision",
            "raw_text",
            "language",
            "confidence",
            "start_ms",
            "end_ms",
            "model_run_id",
            "audio_refs_json",
            "attribution_json",
            "review_state",
        ],
        vec![eq(
            "transcript_revisions",
            "id",
            json!(&current_revision_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "current transcript revision is unavailable".to_string())?;
    let current_revision = integer(&current, "transcript_revisions.revision")?;
    if string(&current, "transcript_revisions.source_session_id")?
        != request.scope.source_session_id
        || revision.expected_revision != Some(current_revision)
        || revision.revision != current_revision + 1
        || revision.raw_text != string(&current, "transcript_revisions.raw_text")?
        || revision.start_ms != integer(&current, "transcript_revisions.start_ms")?
        || revision.end_ms != integer(&current, "transcript_revisions.end_ms")?
        || revision.language != optional_string(&current, "transcript_revisions.language")
        || revision.confidence
            != current
                .get("transcript_revisions.confidence")
                .and_then(Value::as_f64)
        || revision.model_run_id != optional_string(&current, "transcript_revisions.model_run_id")
    {
        return Err(
            "REVISION_CONFLICT: correction changed source provenance or expected revision"
                .to_string(),
        );
    }

    let coverage_ids: Vec<String> = serde_json::from_value(
        current
            .get("transcript_revisions.audio_refs_json")
            .cloned()
            .ok_or_else(|| "current revision source coverage is unavailable".to_string())?,
    )
    .map_err(|_| "current revision source coverage is invalid".to_string())?;
    if coverage_ids.is_empty() {
        return Err("current revision has no verified audio coverage".to_string());
    }
    let coverage = coverage_ids
        .iter()
        .map(|id| load_committed_coverage(storage, &request.scope, id))
        .collect::<Result<Vec<_>, _>>()?;
    let audio_spans = coverage
        .iter()
        .filter(|source| source.kind == SourceCoverageKind::Audio)
        .map(|source| (source.start_ms, source.end_ms))
        .collect::<Vec<_>>();
    if !audio_range_is_covered(&audio_spans, revision.start_ms, revision.end_ms) {
        return Err("correction audio provenance is no longer complete".to_string());
    }
    let check = AtomicMeetingRequest {
        event_id: event_id.clone(),
        source_cursor: coverage
            .iter()
            .map(|source| source.sequence_no)
            .max()
            .unwrap_or(-1),
        committed_cursor: -1,
        scope: request.scope.clone(),
        sources: vec![],
        revision: revision.clone(),
        attribution: ParticipantAttribution {
            kind: "unknown_source".to_string(),
            participant_session_id: None,
            speaker_cluster_id: Some("unknown".to_string()),
            label_snapshot_ref: Some("unknown".to_string()),
            provider_ref_ciphertext: None,
            identity_link_id: None,
            identity_expected_revision: None,
            person_ref_ciphertext: None,
            evidence_revision: 0,
        },
        knowledge: None,
    };
    check_revision_conflicts(storage, &check)?;

    let recording_cursor_id = format!("recording::{}", request.scope.recording_id);
    let recording_cursor = query(
        storage,
        "meeting_recording_cursors",
        &["last_cursor", "minimum_retained_cursor", "revision"],
        vec![eq(
            "meeting_recording_cursors",
            "id",
            json!(&recording_cursor_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    let prior_cursor = recording_cursor
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.last_cursor"))
        .transpose()?
        .unwrap_or(-1);
    let cursor = prior_cursor
        .checked_add(1)
        .ok_or_else(|| "MEETING_CURSOR_OVERFLOW".to_string())?;
    let attribution = current
        .get("transcript_revisions.attribution_json")
        .cloned()
        .unwrap_or_else(|| json!({"kind":"unknown_source"}));
    let event_payload = json!({
        "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
        "event_type": "transcript_revision_committed",
        "event_id": event_id,
        "scope": request.scope,
        "source_cursor": coverage.iter().map(|source| source.sequence_no).max().unwrap_or(-1),
        "committed_cursor": cursor,
        "coverage": coverage.iter().map(coverage_payload).collect::<Vec<_>>(),
        "revision": revision,
        "attribution": attribution,
    });
    let payload_hash = canonical_sha256(&event_payload)?;
    let control_event_id = format!(
        "mi-control::{}",
        Sha256::digest(format!("revision:{}", revision.id).as_bytes())[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let current_legacy_segment = query(
        storage,
        "transcript_segments",
        &["created_at"],
        vec![eq(
            "transcript_segments",
            "id",
            json!(&revision.utterance_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    let mut mutations = vec![
        upsert(
            "transcript_revisions",
            json!({
                "id": revision.id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "utterance_id": revision.utterance_id,
                "revision": revision.revision,
                "supersedes_revision": revision.supersedes_revision,
                "expected_revision": revision.expected_revision,
                "state": "committed",
                "origin": "human",
                "raw_text": revision.raw_text,
                "effective_text": revision.effective_text,
                "language": revision.language,
                "confidence": revision.confidence,
                "start_ms": revision.start_ms,
                "end_ms": revision.end_ms,
                "audio_refs_json": coverage_ids,
                "attribution_json": attribution,
                "model_run_id": revision.model_run_id,
                "review_state": "reviewed",
                "payload_hash": payload_hash,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": attempt.committed_at,
            }),
        ),
        upsert(
            "transcript_projection",
            json!({
                "id": revision.utterance_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "utterance_id": revision.utterance_id,
                "revision_id": revision.id,
                "revision": revision.revision,
                "state": "committed",
                "effective_text": revision.effective_text,
                "language": revision.language,
                "confidence": revision.confidence,
                "review_state": "reviewed",
                "source_event_id": event_id,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "updated_at": attempt.committed_at,
            }),
        ),
        upsert(
            "transcript_segments",
            json!({
                "id": revision.utterance_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "speaker_id": null,
                "start_ms": revision.start_ms,
                "end_ms": revision.end_ms,
                "text": revision.effective_text,
                "confidence": revision.confidence,
                "created_at": current_legacy_segment
                    .and_then(|row| row.get("transcript_segments.created_at").cloned())
                    .unwrap_or_else(|| json!(attempt.committed_at)),
                "updated_at": attempt.committed_at,
            }),
        ),
        upsert(
            "transcript_event_log",
            json!({
                "id": event_id,
                "transaction_id": attempt.transaction_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "source_generation": request.scope.source_generation,
                "cursor": cursor,
                "event_type": "transcript_revision_committed",
                "revision_id": revision.id,
                "payload_json": event_payload,
                "payload_hash": payload_hash,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "committed_at": attempt.committed_at,
            }),
        ),
        upsert(
            "meeting_control_events",
            json!({
                "id": control_event_id,
                "project_id": request.scope.project_id,
                "recording_id": request.scope.recording_id,
                "meeting_session_id": request.scope.meeting_session_id,
                "source_session_id": request.scope.source_session_id,
                "source_generation": request.scope.source_generation,
                "cursor": cursor,
                "event_type": "transcript_revision_committed",
                "batch_id": null,
                "revision_id": revision.id,
                "payload_json": event_payload,
                "payload_hash": payload_hash,
                "transaction_id": attempt.transaction_id,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "committed_at": attempt.committed_at,
            }),
        ),
        upsert(
            "meeting_recording_cursors",
            json!({
                "id": recording_cursor_id,
                "recording_id": request.scope.recording_id,
                "last_cursor": cursor,
                "minimum_retained_cursor": recording_cursor.as_ref()
                    .and_then(|row| row.get("meeting_recording_cursors.minimum_retained_cursor"))
                    .cloned().unwrap_or(json!(0)),
                "revision": recording_cursor.as_ref()
                    .and_then(|row| row.get("meeting_recording_cursors.revision"))
                    .and_then(Value::as_i64).unwrap_or(0) + 1,
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "adoption_complete": true,
                "updated_at": attempt.committed_at,
            }),
        ),
    ];
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: attempt.transaction_id.clone(),
            expected_frontier: Some(attempt.expected_frontier),
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations: std::mem::take(&mut mutations),
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map_err(meeting_commit_error)?;
    Ok(CommittedMeetingEvent {
        contract_version: meeting_intelligence_schema::CONTRACT_VERSION,
        event_id,
        transaction_id: attempt.transaction_id.clone(),
        cursor,
        revision_id: revision.id.clone(),
        payload_hash,
        commit_sequence: None,
        idempotent: false,
    })
}

pub(crate) fn replay_meeting_events(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
    cursor: &MeetingReplayCursor,
    limit: u32,
) -> Result<MeetingReplayPage, String> {
    require_recording_project_scope(storage, project_id, recording_id)?;
    if cursor.recording_id != recording_id {
        return Err("CURSOR_RECORDING_MISMATCH".to_string());
    }
    if cursor.after_cursor < -1 || limit == 0 || limit > 500 {
        return Err("CURSOR_INVALID_OR_LIMIT_EXCEEDED".to_string());
    }
    let recording_cursor_id = format!("recording::{recording_id}");
    let state = query(
        storage,
        "meeting_recording_cursors",
        &["last_cursor", "minimum_retained_cursor"],
        vec![eq(
            "meeting_recording_cursors",
            "id",
            json!(&recording_cursor_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    let high_watermark = state
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.last_cursor"))
        .transpose()?
        .unwrap_or(-1);
    let minimum = state
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.minimum_retained_cursor"))
        .transpose()?
        .unwrap_or(0);
    if cursor.after_cursor < minimum - 1 {
        return Err("CURSOR_EXPIRED".to_string());
    }
    if cursor.after_cursor > high_watermark {
        return Err("CURSOR_FUTURE".to_string());
    }
    let mut events = query_all(
        storage,
        "meeting_control_events",
        &[
            "id",
            "cursor",
            "event_type",
            "payload_json",
            "payload_hash",
            "transaction_id",
            "committed_at",
        ],
        vec![eq(
            "meeting_control_events",
            "recording_id",
            json!(recording_id),
        )],
    )?
    .into_iter()
    .filter(|row| {
        row.get("meeting_control_events.cursor")
            .and_then(Value::as_i64)
            .is_some_and(|value| value > cursor.after_cursor && value <= high_watermark)
    })
    .collect::<Vec<_>>();
    events.sort_by_key(|row| {
        row.get("meeting_control_events.cursor")
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX)
    });
    if let Some(first) = events
        .first()
        .and_then(|row| row.get("meeting_control_events.cursor"))
        .and_then(Value::as_i64)
    {
        if first != cursor.after_cursor.saturating_add(1) {
            return Err("CURSOR_GAP: request a fresh snapshot".to_string());
        }
    } else if cursor.after_cursor < high_watermark {
        return Err("CURSOR_GAP: request a fresh snapshot".to_string());
    }
    let has_more = events.len() > limit as usize;
    events.truncate(limit as usize);
    let mut next_cursor = cursor.after_cursor;
    let mut output = Vec::with_capacity(events.len());
    for row in events {
        let event_cursor = integer(&row, "meeting_control_events.cursor")?;
        if event_cursor != next_cursor.saturating_add(1) {
            return Err("CURSOR_GAP: request a fresh snapshot".to_string());
        }
        next_cursor = event_cursor;
        output.push(json!({
            "id": string(&row, "meeting_control_events.id")?,
            "cursor": event_cursor,
            "event_type": string(&row, "meeting_control_events.event_type")?,
            "payload": row.get("meeting_control_events.payload_json").cloned().unwrap_or(Value::Null),
            "payload_hash": string(&row, "meeting_control_events.payload_hash")?,
            "transaction_id": string(&row, "meeting_control_events.transaction_id")?,
            "committed_at": string(&row, "meeting_control_events.committed_at")?,
        }));
    }
    Ok(MeetingReplayPage {
        recording_id: recording_id.to_string(),
        high_watermark,
        next_cursor,
        has_more,
        events: output,
    })
}

pub(crate) fn meeting_transcript_snapshot(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<MeetingTranscriptSnapshot, String> {
    require_recording_project_scope(storage, project_id, recording_id)?;
    let recording_cursor_id = format!("recording::{recording_id}");
    let state = query(
        storage,
        "meeting_recording_cursors",
        &["last_cursor"],
        vec![eq(
            "meeting_recording_cursors",
            "id",
            json!(&recording_cursor_id),
        )],
        1,
    )?
    .into_iter()
    .next();
    let high_watermark = state
        .as_ref()
        .map(|row| integer(row, "meeting_recording_cursors.last_cursor"))
        .transpose()?
        .unwrap_or(-1);
    let events = query_all(
        storage,
        "meeting_control_events",
        &["cursor", "event_type", "revision_id", "payload_json"],
        vec![eq(
            "meeting_control_events",
            "recording_id",
            json!(recording_id),
        )],
    )?;
    let mut event_cursors = HashMap::new();
    let mut revision_scopes = HashMap::new();
    let mut legacy_snapshot = false;
    for event in events {
        let cursor = integer(&event, "meeting_control_events.cursor")?;
        if cursor > high_watermark {
            continue;
        }
        if string(&event, "meeting_control_events.event_type")? == "legacy_snapshot" {
            legacy_snapshot = true;
        }
        if let Some(revision_id) = event
            .get("meeting_control_events.revision_id")
            .and_then(Value::as_str)
        {
            event_cursors.insert(revision_id.to_string(), cursor);
            if let Some(scope) = event
                .get("meeting_control_events.payload_json")
                .and_then(|payload| payload.get("scope"))
            {
                revision_scopes.insert(revision_id.to_string(), scope.clone());
            }
        }
    }
    let mut latest = HashMap::<String, Value>::new();
    if !event_cursors.is_empty() {
        for revision in query_all(
            storage,
            "transcript_revisions",
            &[
                "id",
                "utterance_id",
                "revision",
                "state",
                "origin",
                "raw_text",
                "effective_text",
                "language",
                "confidence",
                "start_ms",
                "end_ms",
                "model_run_id",
                "review_state",
                "attribution_json",
            ],
            vec![eq(
                "transcript_revisions",
                "recording_id",
                json!(recording_id),
            )],
        )? {
            let revision_id = string(&revision, "transcript_revisions.id")?;
            let Some(cursor) = event_cursors.get(&revision_id).copied() else {
                continue;
            };
            let utterance_id = string(&revision, "transcript_revisions.utterance_id")?;
            let revision_no = integer(&revision, "transcript_revisions.revision")?;
            let attribution = revision
                .get("transcript_revisions.attribution_json")
                .cloned()
                .unwrap_or(Value::Null);
            let speaker_id = attribution
                .get("speaker_cluster_id")
                .or_else(|| attribution.get("speaker_id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            let candidate = json!({
                "utterance_id": utterance_id,
                "revision_id": revision_id,
                "scope": revision_scopes.get(&revision_id).cloned().unwrap_or(Value::Null),
                "revision": revision_no,
                "state": string(&revision, "transcript_revisions.state")?,
                "origin": string(&revision, "transcript_revisions.origin")?,
                "raw_text": string(&revision, "transcript_revisions.raw_text")?,
                "effective_text": string(&revision, "transcript_revisions.effective_text")?,
                "language": revision.get("transcript_revisions.language").cloned().unwrap_or(Value::Null),
                "confidence": revision.get("transcript_revisions.confidence").cloned().unwrap_or(Value::Null),
                "start_ms": integer(&revision, "transcript_revisions.start_ms")?,
                "end_ms": integer(&revision, "transcript_revisions.end_ms")?,
                "model_run_id": revision.get("transcript_revisions.model_run_id").cloned().unwrap_or(Value::Null),
                "speaker_id": speaker_id,
                "review_state": string(&revision, "transcript_revisions.review_state")?,
                "cursor": cursor,
            });
            let replace = latest
                .get(&utterance_id)
                .and_then(|value| value.get("revision"))
                .and_then(Value::as_i64)
                .is_none_or(|current| revision_no > current);
            if replace {
                latest.insert(utterance_id, candidate);
            }
        }
    }
    let mut utterances = latest.into_values().collect::<Vec<_>>();
    if utterances.is_empty() && legacy_snapshot {
        utterances = query_all(
            storage,
            "transcript_segments",
            &["id", "speaker_id", "start_ms", "end_ms", "text", "confidence"],
            vec![eq("transcript_segments", "recording_id", json!(recording_id))],
        )?
        .into_iter()
        .map(|row| {
            json!({
                "utterance_id": row.get("transcript_segments.id").cloned().unwrap_or(Value::Null),
                "revision_id": Value::Null,
                "revision": 0,
                "state": "legacy_read_only",
                "origin": "unknown",
                "raw_text": row.get("transcript_segments.text").cloned().unwrap_or(Value::Null),
                "effective_text": row.get("transcript_segments.text").cloned().unwrap_or(Value::Null),
                "language": Value::Null,
                "confidence": row.get("transcript_segments.confidence").cloned().unwrap_or(Value::Null),
                "speaker_id": row.get("transcript_segments.speaker_id").cloned().unwrap_or(Value::Null),
                "start_ms": row.get("transcript_segments.start_ms").cloned().unwrap_or(Value::Null),
                "end_ms": row.get("transcript_segments.end_ms").cloned().unwrap_or(Value::Null),
                "review_state": "unknown",
                "cursor": 0,
                "coverage_verified": false,
            })
        })
        .collect();
    }
    utterances.sort_by(|left, right| {
        left.get("start_ms")
            .and_then(Value::as_i64)
            .cmp(&right.get("start_ms").and_then(Value::as_i64))
            .then_with(|| {
                left.get("utterance_id")
                    .and_then(Value::as_str)
                    .cmp(&right.get("utterance_id").and_then(Value::as_str))
            })
    });
    if utterances.len() > 200 {
        utterances.drain(..utterances.len() - 200);
    }
    Ok(MeetingTranscriptSnapshot {
        recording_id: recording_id.to_string(),
        high_watermark,
        utterances,
        legacy_snapshot,
    })
}

pub(crate) fn query(
    storage: &Storage,
    table: &str,
    columns: &[&str],
    filters: Vec<RelationalFilter>,
    limit: u32,
) -> Result<Vec<Value>, String> {
    storage
        .query_relational(RelationalQuery {
            namespace: NAMESPACE.to_string(),
            table: table.to_string(),
            columns: columns
                .iter()
                .map(|column| format!("{table}.{column}"))
                .collect(),
            joins: vec![],
            filters,
            limit: Some(limit),
            offset: None,
        })
        .map_err(|error| error.to_string())
}

/// One page of [`query_all`]: `limit` rows starting `offset` rows in, ordered
/// by the base table's primary key on the engine side.
fn query_at(
    storage: &Storage,
    table: &str,
    columns: &[&str],
    filters: Vec<RelationalFilter>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Value>, String> {
    storage
        .query_relational(RelationalQuery {
            namespace: NAMESPACE.to_string(),
            table: table.to_string(),
            columns: columns
                .iter()
                .map(|column| format!("{table}.{column}"))
                .collect(),
            joins: vec![],
            filters,
            limit: Some(limit),
            offset: Some(offset),
        })
        .map_err(|error| error.to_string())
}

pub(crate) fn eq(table: &str, column: &str, value: Value) -> RelationalFilter {
    RelationalFilter::equal(&format!("{table}.{column}"), value)
}

pub(crate) fn upsert(table: &str, values: Value) -> RelationalRowMutation {
    RelationalRowMutation {
        table: table.to_string(),
        kind: RelationalMutationKind::Upsert,
        values,
        key: None,
    }
}

pub(crate) fn ensure_project_mutations(
    project_id: &str,
    storage_path: &str,
    timestamp: &str,
) -> Vec<RelationalRowMutation> {
    vec![
        upsert(
            "projects",
            json!({
                "id": project_id,
                "name": "FUNG Mobile",
                "storage_path": storage_path,
                "created_at": timestamp,
                "updated_at": timestamp
                ,"active_recording_id": null
            }),
        ),
        upsert(
            "graph_nodes",
            json!({
                "id": project_id,
                "project_id": project_id,
                "entity_type": "project",
                "entity_id": project_id,
                "label": "FUNG Mobile",
                "position_x": 50.0,
                "position_y": 17.0,
                "created_at": timestamp,
                "updated_at": timestamp
            }),
        ),
    ]
}

pub(crate) fn commit_note(
    storage: &Storage,
    note: &super::mobile::MobileNoteInput,
    storage_path: &str,
) -> Result<(), String> {
    let revision_id = Uuid::new_v4().to_string();
    let logical_clock = format!("{}:{}", note.updated_at, Uuid::new_v4());
    let mut mutations = ensure_project_mutations(&note.project_id, storage_path, &note.updated_at);
    mutations.extend([
        upsert(
            "notes",
            json!({
                "id": note.id,
                "project_id": note.project_id,
                "title": note.title,
                "current_revision_id": revision_id,
                "created_at": note.created_at,
                "updated_at": note.updated_at
            }),
        ),
        upsert(
            "note_revisions",
            json!({
                "id": revision_id,
                "note_id": note.id,
                "body": note.body,
                "evidence_label": note.evidence_label,
                "author_device_id": "mobile-local",
                "logical_clock": logical_clock,
                "created_at": note.updated_at
            }),
        ),
        upsert(
            "graph_nodes",
            json!({
                "id": note.id,
                "project_id": note.project_id,
                "entity_type": "note",
                "entity_id": note.id,
                "label": note.title,
                "position_x": 50.0,
                "position_y": 50.0,
                "created_at": note.created_at,
                "updated_at": note.updated_at
            }),
        ),
        upsert(
            "mutation_log",
            json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": note.project_id,
                "device_id": "mobile-local",
                "logical_clock": logical_clock,
                "entity_type": "note",
                "entity_id": note.id,
                "operation": "upsert",
                "payload_json": json!({"revisionId": revision_id}).to_string(),
                "created_at": note.updated_at
            }),
        ),
    ]);
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: Uuid::new_v4().to_string(),
            expected_frontier: None,
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations,
            }],
            graph: BatchInput {
                nodes: vec![NodeInput {
                    id: Some(note.id.clone()),
                    labels: vec!["Note".to_string()],
                    props: Some(json!({
                        "projectId": note.project_id,
                        "title": note.title,
                        "revisionId": revision_id
                    })),
                    embedding: None,
                    lang: None,
                    valid_from: Some(note.updated_at.clone()),
                    caused_by: None,
                    ttl: None,
                    collection: None,
                }],
                edges: vec![],
            },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) fn commit_relation(
    storage: &Storage,
    project_id: &str,
    edge: &super::mobile::GraphEdgeInput,
    timestamp: &str,
) -> Result<(), String> {
    storage
        .commit_transaction(GenesisTransaction {
            transaction_id: Uuid::new_v4().to_string(),
            expected_frontier: None,
            relational: vec![RelationalMutationGroup {
                namespace: NAMESPACE.to_string(),
                mutations: vec![upsert(
                    "graph_edges",
                    json!({
                        "id": edge.id,
                        "project_id": project_id,
                        "source_node_id": edge.source_id,
                        "target_node_id": edge.target_id,
                        "predicate": edge.predicate,
                        "epistemic_status": edge.status,
                        "provenance_json": "{\"actor\":\"user\"}",
                        "created_at": timestamp,
                        "updated_at": timestamp
                    }),
                )],
            }],
            graph: BatchInput {
                nodes: vec![],
                edges: vec![EdgeInput {
                    id: Some(edge.id.clone()),
                    from: edge.source_id.clone(),
                    to: edge.target_id.clone(),
                    rel: edge.predicate.clone(),
                    props: Some(json!({"epistemicStatus": edge.status})),
                    valid_from: Some(timestamp.to_string()),
                    supersede: None,
                    impact: None,
                    caused_by: None,
                }],
            },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[derive(Clone, Debug)]
pub(crate) struct CaptureRecord {
    pub(crate) recording_id: String,
    pub(crate) project_id: String,
    pub(crate) status: String,
    pub(crate) canonical_audio_path: String,
    pub(crate) duration_ms: i64,
    pub(crate) safe_offset_ms: i64,
    pub(crate) segment_count: i64,
    pub(crate) created_at: String,
    /// Fields below are carried purely so the row can be written back intact.
    ///
    /// `append_capture_chunk` and `finish_capture` rewrite the whole
    /// `recordings` row on every chunk, and an upsert with a column missing
    /// clears it. `source` and `input_path` were being hardcoded to
    /// `"microphone"` and `null` on each rewrite, so adopting orphaned audio
    /// into an imported recording silently relabelled it as a live capture
    /// and dropped the path it was imported from. `language` would have hit
    /// the same wall on the very first chunk.
    pub(crate) source: String,
    pub(crate) input_path: Option<String>,
    pub(crate) language: Option<String>,
}

pub(crate) fn string(row: &Value, key: &str) -> Result<String, String> {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing relational field {key}"))
}

pub(crate) fn integer(row: &Value, key: &str) -> Result<i64, String> {
    row.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing relational field {key}"))
}

pub(crate) fn capture(storage: &Storage, recording_id: &str) -> Result<CaptureRecord, String> {
    let recording = query(
        storage,
        "recordings",
        &[
            "id",
            "project_id",
            "status",
            "canonical_audio_path",
            "duration_ms",
            "created_at",
            "updated_at",
            "source",
            "input_path",
            "language",
        ],
        vec![eq("recordings", "id", json!(recording_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "recording was not found".to_string())?;
    let checkpoint = query(
        storage,
        "mobile_recording_checkpoints",
        &["safe_offset_ms", "segment_count", "last_checksum"],
        vec![eq(
            "mobile_recording_checkpoints",
            "recording_id",
            json!(recording_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "recording checkpoint was not found".to_string())?;
    Ok(CaptureRecord {
        recording_id: string(&recording, "recordings.id")?,
        project_id: string(&recording, "recordings.project_id")?,
        status: string(&recording, "recordings.status")?,
        canonical_audio_path: string(&recording, "recordings.canonical_audio_path")?,
        duration_ms: integer(&recording, "recordings.duration_ms")?,
        safe_offset_ms: integer(&checkpoint, "mobile_recording_checkpoints.safe_offset_ms")?,
        segment_count: integer(&checkpoint, "mobile_recording_checkpoints.segment_count")?,
        created_at: string(&recording, "recordings.created_at")?,
        // `source` predates this read and is required by the schema, but a
        // row imported from the retired SQLite database can still be missing
        // it; defaulting to "microphone" preserves the value the rewrites
        // used to hardcode rather than failing a capture over provenance.
        source: string(&recording, "recordings.source")
            .unwrap_or_else(|_| "microphone".to_string()),
        input_path: optional_string(&recording, "recordings.input_path"),
        language: optional_string(&recording, "recordings.language"),
    })
}

/// A nullable text column as `Option<String>`, with an absent column and a
/// JSON null treated the same: both mean "not set".
pub(crate) fn optional_string(row: &Value, key: &str) -> Option<String> {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

pub(crate) fn active_capture(
    storage: &Storage,
    project_id: &str,
) -> Result<Option<CaptureRecord>, String> {
    let rows = query(
        storage,
        "recordings",
        &["id", "status"],
        vec![eq("recordings", "project_id", json!(project_id))],
        100,
    )?;
    let id = rows.into_iter().find_map(|row| {
        matches!(
            row.get("recordings.status").and_then(Value::as_str),
            Some("recording" | "paused")
        )
        .then(|| {
            row.get("recordings.id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .flatten()
    });
    id.map(|id| capture(storage, &id)).transpose()
}

pub(crate) fn start_capture(
    storage: &Storage,
    project_id: &str,
    storage_path: &str,
    recording_id: &str,
    manifest_path: &str,
    timestamp: &str,
) -> Result<CaptureRecord, String> {
    let mut mutations = ensure_project_mutations(project_id, storage_path, timestamp);
    mutations.extend([
        upsert(
            "recordings",
            json!({
                "id": recording_id,
                "project_id": project_id,
                "source": "microphone",
                "input_path": null,
                "canonical_audio_path": manifest_path,
                "status": "recording",
                "duration_ms": 0,
                "created_at": timestamp,
                "updated_at": timestamp,
                // The mobile shell has no language selector, so its captures
                // are transcribed with whisper's own detection. Written
                // explicitly rather than omitted so the column's absence is
                // a statement, not an oversight.
                "language": null
            }),
        ),
        upsert(
            "mobile_recording_checkpoints",
            json!({
                "id": recording_id,
                "recording_id": recording_id,
                "safe_offset_ms": 0,
                "segment_count": 0,
                "last_checksum": null,
                "updated_at": timestamp
            }),
        ),
    ]);
    commit_rows(storage, mutations)?;
    capture(storage, recording_id)
}

/// The language a recording was captured in, or `None` when it was captured
/// without one and whisper detected per chunk.
///
/// Read from the ledger rather than threaded through call frames, so every
/// later pass over the same audio — the catch-up transcription, a recovery,
/// a re-run queued by the job engine — reaches the same answer the live
/// session used.
pub(crate) fn recording_language(storage: &Storage, recording_id: &str) -> Option<String> {
    query(
        storage,
        "recordings",
        &["language"],
        vec![eq("recordings", "id", json!(recording_id))],
        1,
    )
    .ok()?
    .first()
    .and_then(|row| optional_string(row, "recordings.language"))
}

/// Records that the transcriber has processed a chunk.
///
/// Distinct from "the chunk has words": a chunk of silence produces no
/// segments, and without this stamp it looked identical to a chunk that had
/// never been transcribed, so every catch-up pass offered it again forever.
///
/// Reads the row before writing because Genesis upserts replace the whole
/// row — the same reason `set_job_status` does. A chunk that has vanished is
/// not an error here: the caller has already produced its segments, and
/// failing the transcription over a bookkeeping write would lose them.
pub(crate) fn mark_chunk_transcribed(
    storage: &Storage,
    chunk_id: &str,
    timestamp: &str,
) -> Result<(), String> {
    let Some(row) = query(
        storage,
        "audio_chunks",
        &[
            "id",
            "recording_id",
            "sequence_no",
            "file_path",
            "start_ms",
            "end_ms",
            "byte_size",
            "checksum",
            "created_at",
        ],
        vec![eq("audio_chunks", "id", json!(chunk_id))],
        1,
    )?
    .into_iter()
    .next() else {
        return Ok(());
    };
    let field = |key: &str| row.get(key).cloned().unwrap_or(Value::Null);
    commit_rows(
        storage,
        vec![upsert(
            "audio_chunks",
            json!({
                "id": chunk_id,
                "recording_id": field("audio_chunks.recording_id"),
                "sequence_no": field("audio_chunks.sequence_no"),
                "file_path": field("audio_chunks.file_path"),
                "start_ms": field("audio_chunks.start_ms"),
                "end_ms": field("audio_chunks.end_ms"),
                "byte_size": field("audio_chunks.byte_size"),
                "checksum": field("audio_chunks.checksum"),
                "created_at": field("audio_chunks.created_at"),
                "transcribed_at": timestamp
            }),
        )],
    )
}

pub(crate) struct AudioChunk<'a> {
    pub(crate) id: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) byte_size: i64,
    pub(crate) checksum: &'a str,
    pub(crate) timestamp: &'a str,
}

pub(crate) fn append_capture_chunk(
    storage: &Storage,
    current: &CaptureRecord,
    chunk: AudioChunk<'_>,
) -> Result<CaptureRecord, String> {
    let sequence = current.segment_count + 1;
    commit_rows(
        storage,
        vec![
            upsert(
                "audio_chunks",
                json!({
                    "id": chunk.id,
                    "recording_id": current.recording_id,
                    "sequence_no": sequence,
                    "file_path": chunk.file_path,
                    "start_ms": chunk.start_ms,
                    "end_ms": chunk.end_ms,
                    "byte_size": chunk.byte_size,
                    "checksum": chunk.checksum,
                    "created_at": chunk.timestamp
                }),
            ),
            upsert(
                "mobile_recording_checkpoints",
                json!({
                    "id": current.recording_id,
                    "recording_id": current.recording_id,
                    "safe_offset_ms": chunk.end_ms,
                    "segment_count": sequence,
                    "last_checksum": chunk.checksum,
                    "updated_at": chunk.timestamp
                }),
            ),
            upsert(
                "recordings",
                json!({
                    "id": current.recording_id,
                    "project_id": current.project_id,
                    "source": current.source,
                    "input_path": current.input_path,
                    "canonical_audio_path": current.canonical_audio_path,
                    "status": current.status,
                    "duration_ms": chunk.end_ms,
                    "created_at": current.created_at,
                    "updated_at": chunk.timestamp,
                    "language": current.language
                }),
            ),
        ],
    )?;
    capture(storage, &current.recording_id)
}

pub(crate) fn finish_capture(
    storage: &Storage,
    current: &CaptureRecord,
    timestamp: &str,
) -> Result<CaptureRecord, String> {
    commit_rows(
        storage,
        vec![upsert(
            "recordings",
            json!({
                "id": current.recording_id,
                "project_id": current.project_id,
                "source": current.source,
                "input_path": current.input_path,
                "canonical_audio_path": current.canonical_audio_path,
                "status": "completed",
                "duration_ms": current.duration_ms,
                "created_at": current.created_at,
                "updated_at": timestamp,
                "language": current.language
            }),
        )],
    )?;
    capture(storage, &current.recording_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting_intelligence_schema::{
        open_person_identity, seal_person_identity, validate_private_identity_reference,
        AtomicMeetingRequest, IdentityAadContext, IdentityEnvelope, InMemoryIdentityKeyBackend,
        KnowledgeEvidenceInput, MeetingIngestBatchRequest, MeetingReplayCursor,
        MeetingRevisionRequest, MeetingScope, ParticipantAttribution, PersonIdentityPayload,
        PrivateIdentityReference, RevisionCoverageBinding, SourceCoverageInput, SourceCoverageKind,
        TranscriptOrigin, TranscriptRevisionInput,
    };
    use genesis_block_native::{BackupExportRequest, BackupRestoreRequest, OpenOptions};
    use serde::Serialize;
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    fn open() -> (std::path::PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-genesis-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&storage).unwrap();
        (path, storage)
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn private_asset_reader_stops_at_the_declared_byte_bound() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("asset.enc");
        std::fs::write(&path, b"12345").unwrap();

        let bounded = read_bounded_private_asset(&path, 4, "ASSET_SIZE_INVALID").unwrap_err();
        assert_eq!(bounded, "ASSET_SIZE_INVALID");
        let accepted = read_bounded_private_asset(&path, 5, "ASSET_SIZE_INVALID").unwrap();
        assert_eq!(accepted.as_slice(), b"12345");
    }

    fn open_at_v10() -> (PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-genesis-v10-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        for package in [
            schema_v1(),
            schema_v2(),
            schema_v3(),
            schema_v4(),
            schema_v5(),
            schema_v6(),
            schema_v7(),
            schema_v8(),
            schema_v9(),
            schema_v10(),
        ] {
            storage.register_relational_schema(package).unwrap();
        }
        (path, storage)
    }

    fn seed_meeting(storage: &Storage, root: &Path) -> (MeetingScope, PathBuf) {
        let timestamp = "2026-09-21T10:00:00Z";
        let scope = MeetingScope {
            project_id: "n3-project".to_string(),
            recording_id: "n3-recording".to_string(),
            meeting_session_id: "n3-session".to_string(),
            source_session_id: "n3-source".to_string(),
            track_id: "mic-1".to_string(),
            source_generation: 1,
        };
        let audio_path = root.join("audio-0.raw");
        let audio = b"n3-finalized-audio-0";
        std::fs::write(&audio_path, audio).unwrap();
        commit_rows(
            storage,
            vec![
                upsert(
                    "projects",
                    json!({
                        "id": scope.project_id,
                        "name": "N3 meeting",
                        "storage_path": root.display().to_string(),
                        "active_recording_id": null,
                        "created_at": timestamp,
                        "updated_at": timestamp
                    }),
                ),
                upsert(
                    "recordings",
                    json!({
                        "id": scope.recording_id,
                        "project_id": scope.project_id,
                        "source": "microphone",
                        "input_path": null,
                        "canonical_audio_path": root.display().to_string(),
                        "status": "completed",
                        "duration_ms": 1000,
                        "created_at": timestamp,
                        "updated_at": timestamp,
                        "language": null
                    }),
                ),
                upsert(
                    "audio_chunks",
                    json!({
                        "id": "n3-chunk-0",
                        "recording_id": scope.recording_id,
                        "sequence_no": 0,
                        "file_path": audio_path.display().to_string(),
                        "start_ms": 0,
                        "end_ms": 1000,
                        "byte_size": audio.len() as i64,
                        "checksum": sha256_bytes(audio),
                        "created_at": timestamp,
                        "transcribed_at": null
                    }),
                ),
                upsert(
                    "meeting_sessions",
                    json!({
                        "id": scope.meeting_session_id,
                        "project_id": scope.project_id,
                        "recording_id": scope.recording_id,
                        "session_generation": 1,
                        "source_mode": "local",
                        "state": "active",
                        "owner_scope": "project:n3-project",
                        "policy_version": "meeting-intelligence-v1",
                        "revision": 1,
                        "contract_version": 1,
                        "created_at": timestamp,
                        "updated_at": timestamp
                    }),
                ),
                upsert(
                    "meeting_source_sessions",
                    json!({
                        "id": scope.source_session_id,
                        "project_id": scope.project_id,
                        "recording_id": scope.recording_id,
                        "meeting_session_id": scope.meeting_session_id,
                        "source_kind": "finalized_audio",
                        "source_generation": scope.source_generation,
                        "state": "open",
                        "contract_version": 1,
                        "created_at": timestamp,
                        "ended_at": null
                    }),
                ),
            ],
        )
        .unwrap();
        (scope, audio_path)
    }

    fn add_audio_chunk(
        storage: &Storage,
        root: &Path,
        id: &str,
        sequence_no: i64,
        start_ms: i64,
        end_ms: i64,
        content: &[u8],
    ) -> PathBuf {
        let path = root.join(format!("{id}.raw"));
        std::fs::write(&path, content).unwrap();
        commit_rows(
            storage,
            vec![upsert(
                "audio_chunks",
                json!({
                    "id": id,
                    "recording_id": "n3-recording",
                    "sequence_no": sequence_no,
                    "file_path": path.display().to_string(),
                    "start_ms": start_ms,
                    "end_ms": end_ms,
                    "byte_size": content.len() as i64,
                    "checksum": sha256_bytes(content),
                    "created_at": "2026-09-21T10:01:00Z",
                    "transcribed_at": null
                }),
            )],
        )
        .unwrap();
        path
    }

    fn audio_source(
        scope: &MeetingScope,
        id: &str,
        chunk_id: &str,
        path: &Path,
        sequence_no: i64,
        start_ms: i64,
        end_ms: i64,
        checksum: Option<String>,
    ) -> SourceCoverageInput {
        SourceCoverageInput {
            id: id.to_string(),
            source_session_id: scope.source_session_id.clone(),
            track_id: scope.track_id.clone(),
            source_generation: scope.source_generation,
            sequence_no,
            start_ms,
            end_ms,
            kind: SourceCoverageKind::Audio,
            audio_chunk_id: Some(chunk_id.to_string()),
            file_path: Some(path.display().to_string()),
            byte_size: None,
            checksum,
            gap_reason: None,
        }
    }

    fn meeting_request(
        scope: &MeetingScope,
        event_id: &str,
        source: SourceCoverageInput,
        source_cursor: i64,
        committed_cursor: i64,
        revision_id: &str,
        utterance_id: &str,
        revision: i64,
        origin: TranscriptOrigin,
        expected_revision: Option<i64>,
        supersedes_revision: Option<i64>,
        text: &str,
        review_state: &str,
    ) -> AtomicMeetingRequest {
        let start_ms = source.start_ms;
        let end_ms = source.end_ms;
        AtomicMeetingRequest {
            event_id: event_id.to_string(),
            source_cursor,
            committed_cursor,
            scope: scope.clone(),
            sources: vec![source],
            revision: TranscriptRevisionInput {
                id: revision_id.to_string(),
                utterance_id: utterance_id.to_string(),
                revision,
                supersedes_revision,
                expected_revision,
                origin,
                raw_text: text.to_string(),
                effective_text: text.to_string(),
                language: Some("en-US".to_string()),
                confidence: None,
                start_ms,
                end_ms,
                model_run_id: None,
                review_state: review_state.to_string(),
            },
            attribution: ParticipantAttribution {
                kind: "anonymous".to_string(),
                participant_session_id: None,
                speaker_cluster_id: Some("speaker-0".to_string()),
                label_snapshot_ref: Some("speaker-0".to_string()),
                provider_ref_ciphertext: None,
                identity_link_id: None,
                identity_expected_revision: None,
                person_ref_ciphertext: None,
                evidence_revision: 0,
            },
            knowledge: None,
        }
    }

    fn ingest_batch_request(
        scope: &MeetingScope,
        operation_id: &str,
        source: SourceCoverageInput,
        revisions: Vec<TranscriptRevisionInput>,
    ) -> MeetingIngestBatchRequest {
        let revision_coverage = revisions
            .iter()
            .map(|revision| RevisionCoverageBinding {
                revision_id: revision.id.clone(),
                coverage_ids: vec![source.id.clone()],
            })
            .collect();
        MeetingIngestBatchRequest {
            operation_id: operation_id.to_string(),
            scope: scope.clone(),
            sources: vec![source],
            revisions,
            revision_coverage,
        }
    }

    struct IdentityFixture {
        path: PathBuf,
        storage: Storage,
        audio_root: tempfile::TempDir,
        audio_path: PathBuf,
        scope: MeetingScope,
        context: TrustedIdentityContext,
        backend: InMemoryIdentityKeyBackend,
        link_id: String,
        profile_id: String,
        profile_asset_id: String,
        profile_reference: PrivateIdentityReference,
        evidence_ref: String,
        evidence_sha256: String,
        key_ref: String,
        reference: PrivateIdentityReference,
        owner_source: TestOwnerIdentitySource,
        lifecycle_source: TestLifecycleWitnessSource,
    }

    #[derive(Clone)]
    struct TestLifecycleWitnessSource {
        witness: Arc<Mutex<crate::auth_session::LifecycleWitness>>,
    }

    impl TestLifecycleWitnessSource {
        fn new(witness: crate::auth_session::LifecycleWitness) -> Self {
            Self {
                witness: Arc::new(Mutex::new(witness)),
            }
        }

        fn set(&self, witness: crate::auth_session::LifecycleWitness) {
            *self.witness.lock().unwrap() = witness;
        }
    }

    impl LifecycleWitnessSource for TestLifecycleWitnessSource {
        fn read(&self) -> Result<crate::auth_session::LifecycleWitness, String> {
            self.witness
                .lock()
                .map(|witness| witness.clone())
                .map_err(|_| "test lifecycle witness is poisoned".to_string())
        }
    }

    struct HarnessLifecycleWitnessSource<'a> {
        harness: &'a crate::auth_session::tests::TestAccountOperationHarness,
    }

    impl LifecycleWitnessSource for HarnessLifecycleWitnessSource<'_> {
        fn read(&self) -> Result<crate::auth_session::LifecycleWitness, String> {
            self.harness.lifecycle_witness()
        }
    }

    struct TestOwnerIdentitySource {
        owner_principal_ref: String,
        captured_path: Arc<Mutex<Option<PathBuf>>>,
    }

    impl LocalOwnerIdentitySource for TestOwnerIdentitySource {
        fn owner_principal_ref(&self, data_root: &Path) -> Result<String, String> {
            *self.captured_path.lock().unwrap() = Some(data_root.to_path_buf());
            Ok(self.owner_principal_ref.clone())
        }
    }

    fn identity_unlock_with_lifecycle_source(
        fixture: &IdentityFixture,
        lifecycle_source: &dyn LifecycleWitnessSource,
    ) -> NativeOwnerUnlockSession {
        unlock_native_local_owner_with_sources(
            &fixture.storage,
            &fixture.owner_source,
            lifecycle_source,
        )
        .unwrap()
    }

    fn identity_unlock(fixture: &IdentityFixture) -> NativeOwnerUnlockSession {
        identity_unlock_with_lifecycle_source(fixture, &fixture.lifecycle_source)
    }

    fn identity_link_row(
        scope: &MeetingScope,
        context: &TrustedIdentityContext,
        link_id: &str,
        reference: &PrivateIdentityReference,
        evidence_ref: &str,
        evidence_sha256: &str,
        revision: i64,
        status: &str,
        locked_at: Option<&str>,
        revoked_at: Option<&str>,
    ) -> Value {
        identity_link_row_with_model(
            scope,
            context,
            link_id,
            reference,
            evidence_ref,
            evidence_sha256,
            revision,
            None,
            status,
            locked_at,
            revoked_at,
        )
    }

    fn identity_link_row_with_model(
        scope: &MeetingScope,
        context: &TrustedIdentityContext,
        link_id: &str,
        reference: &PrivateIdentityReference,
        evidence_ref: &str,
        evidence_sha256: &str,
        revision: i64,
        model_run_id: Option<&str>,
        status: &str,
        locked_at: Option<&str>,
        revoked_at: Option<&str>,
    ) -> Value {
        json!({
            "id": link_id,
            "project_id": scope.project_id,
            "recording_id": scope.recording_id,
            "meeting_session_id": scope.meeting_session_id,
            "speaker_id": "speaker-identity",
            "vault_id": context.vault_id,
            "person_ref_ciphertext_ref": reference.encrypted_blob_ref,
            "person_ref_ciphertext_sha256": reference.ciphertext_sha256,
            "person_ref_ciphertext_json": serde_json::to_string(&reference.envelope).unwrap(),
            "person_ref_key_ref": reference.envelope.key_ref,
            "match_source": if model_run_id.is_some() { "model_review" } else { "manual_review" },
            "status": status,
            "match_score": null,
            "threshold_policy_id": null,
            "model_run_id": model_run_id,
            "evidence_ciphertext_ref": evidence_ref,
            "evidence_ciphertext_sha256": evidence_sha256,
            "reviewer_ref_ciphertext_ref": null,
            "actor_ref_ciphertext_ref": null,
            "expected_revision": revision,
            "supersedes_id": null,
            "d8_policy_revision": 1,
            "revision": revision,
            "locked_at": locked_at,
            "revoked_at": revoked_at,
            "contract_version": 1,
            "created_at": "2026-09-21T10:00:00Z",
            "updated_at": "2026-09-21T10:00:00Z",
        })
    }

    fn set_fixture_speaker_evidence(storage: &Storage, revision_id: &str, speaker_id: &str) {
        let revision_columns = [
            "id",
            "project_id",
            "recording_id",
            "meeting_session_id",
            "source_session_id",
            "utterance_id",
            "revision",
            "supersedes_revision",
            "expected_revision",
            "state",
            "origin",
            "raw_text",
            "effective_text",
            "language",
            "confidence",
            "start_ms",
            "end_ms",
            "audio_refs_json",
            "attribution_json",
            "model_run_id",
            "review_state",
            "payload_hash",
            "contract_version",
            "created_at",
        ];
        let event_columns = [
            "id",
            "project_id",
            "recording_id",
            "meeting_session_id",
            "source_session_id",
            "source_generation",
            "cursor",
            "event_type",
            "batch_id",
            "revision_id",
            "payload_json",
            "payload_hash",
            "transaction_id",
            "contract_version",
            "committed_at",
        ];
        let mut revision = query(
            storage,
            "transcript_revisions",
            &revision_columns,
            vec![eq("transcript_revisions", "id", json!(revision_id))],
            1,
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let mut event = query(
            storage,
            "meeting_control_events",
            &event_columns,
            vec![eq(
                "meeting_control_events",
                "revision_id",
                json!(revision_id),
            )],
            1,
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let attribution = opaque_attribution_payload(&ParticipantAttribution {
            kind: "anonymous".to_string(),
            participant_session_id: None,
            speaker_cluster_id: Some(speaker_id.to_string()),
            label_snapshot_ref: Some(speaker_id.to_string()),
            provider_ref_ciphertext: None,
            identity_link_id: None,
            identity_expected_revision: None,
            person_ref_ciphertext: None,
            evidence_revision: 0,
        })
        .unwrap();
        let mut payload = event["meeting_control_events.payload_json"].clone();
        payload["attribution"] = attribution.clone();
        let payload_hash = canonical_sha256(&payload).unwrap();
        revision["transcript_revisions.attribution_json"] = attribution;
        revision["transcript_revisions.payload_hash"] = json!(payload_hash);
        event["meeting_control_events.payload_json"] = payload;
        event["meeting_control_events.payload_hash"] = json!(payload_hash);

        let flatten = |table_name: &str, columns: &[&str], row: &Value| {
            Value::Object(
                columns
                    .iter()
                    .map(|column| {
                        (
                            (*column).to_string(),
                            row.get(format!("{table_name}.{column}"))
                                .cloned()
                                .unwrap_or(Value::Null),
                        )
                    })
                    .collect(),
            )
        };
        commit_rows(
            storage,
            vec![
                upsert(
                    "transcript_revisions",
                    flatten("transcript_revisions", &revision_columns, &revision),
                ),
                upsert(
                    "meeting_control_events",
                    flatten("meeting_control_events", &event_columns, &event),
                ),
            ],
        )
        .unwrap();
    }

    fn identity_vault_row(
        vault_id: &str,
        owner_principal_ref: &str,
        account_ref: Option<&str>,
        state: &str,
    ) -> Value {
        json!({
            "id": vault_id,
            "owner_principal_ref": owner_principal_ref,
            "bound_account_ref": account_ref,
            "self_person_ref_ciphertext_ref": null,
            "self_person_ref_ciphertext_sha256": null,
            "self_person_ref_ciphertext_json": null,
            "state": state,
            "key_store_namespace": "people_metadata",
            "contract_version": 1,
            "created_at": "2026-09-21T10:00:00Z",
            "updated_at": "2026-09-21T10:00:00Z",
        })
    }

    fn identity_profile_row(
        profile_id: &str,
        asset_id: &str,
        vault_id: &str,
        owner_scope: &str,
        reference: &PrivateIdentityReference,
        key_ref: &str,
        status: &str,
        revision: i64,
    ) -> Value {
        json!({
            "id": profile_id,
            "vault_id": vault_id,
            "owner_scope": owner_scope,
            "profile_payload_ciphertext_ref": asset_id,
            "profile_ciphertext_sha256": reference.ciphertext_sha256,
            "key_ref": key_ref,
            "status": status,
            "revision": revision,
            "contract_version": 1,
            "created_at": "2026-09-21T10:00:00Z",
            "updated_at": "2026-09-21T10:00:00Z",
        })
    }

    fn identity_fixture() -> IdentityFixture {
        let (path, storage) = open();
        let audio_root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, audio_root.path());
        let vault_id = "00000000-0000-0000-0000-000000000001".to_string();
        let context = test_local_identity_context(&vault_id);
        let owner_source = TestOwnerIdentitySource {
            owner_principal_ref: context.owner_principal_ref.clone(),
            captured_path: Arc::new(Mutex::new(None)),
        };
        let lifecycle_source =
            TestLifecycleWitnessSource::new(crate::auth_session::LifecycleWitness {
                native_user_id: None,
                account_generation: 1,
                state: "signed_out",
            });
        let link_id = "identity-link:11111111111111111111111111111111".to_string();
        let profile_id = "profile:22222222222222222222222222222222".to_string();
        let key_ref = people_metadata_key_ref(&vault_id).unwrap();
        let mut backend = InMemoryIdentityKeyBackend::default();
        backend.insert(&key_ref, vec![7; 32]);
        commit_rows(
            &storage,
            vec![upsert(
                "speakers",
                json!({
                    "id": "speaker-identity",
                    "project_id": scope.project_id,
                    "key": "speaker-identity",
                    "display_name": "Speaker",
                    "confidence": null,
                    "created_at": "2026-09-21T10:00:00Z",
                    "updated_at": "2026-09-21T10:00:00Z",
                }),
            )],
        )
        .unwrap();
        let audio = std::fs::read(&audio_path).unwrap();
        let evidence_revision = TranscriptRevisionInput {
            id: "revision-identity-evidence-seed".to_string(),
            utterance_id: "utterance-identity-evidence-seed".to_string(),
            revision: 1,
            supersedes_revision: None,
            expected_revision: None,
            origin: TranscriptOrigin::LocalAsr,
            raw_text: "identity review evidence".to_string(),
            effective_text: "identity review evidence".to_string(),
            language: Some("en-US".to_string()),
            confidence: None,
            start_ms: 0,
            end_ms: 1000,
            model_run_id: None,
            review_state: "unreviewed".to_string(),
        };
        let evidence_batch = ingest_batch_request(
            &scope,
            "operation-identity-evidence-seed",
            audio_source(
                &scope,
                "coverage-identity-evidence-seed",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&audio)),
            ),
            vec![evidence_revision],
        );
        let evidence_attempt = begin_meeting_commit(
            &storage,
            "tx-identity-evidence-seed",
            "2026-09-21T10:00:00Z",
        )
        .unwrap();
        commit_meeting_ingest_batch(&storage, &evidence_attempt, &evidence_batch).unwrap();
        set_fixture_speaker_evidence(
            &storage,
            "revision-identity-evidence-seed",
            "speaker-identity",
        );
        let (evidence_cursor, evidence_sha256) =
            current_speaker_evidence(&storage, &scope, "speaker-identity").unwrap();
        let evidence_ref = evidence_reference(evidence_cursor, &evidence_sha256);
        let aad = IdentityAadContext {
            account_ref: None,
            scope: identity_scope_key(&scope),
            vault_id: vault_id.clone(),
            entity_id: link_id.clone(),
            revision: 1,
            model_context: manual_identity_model_context("speaker-identity", &evidence_sha256),
        };
        let payload = PersonIdentityPayload {
            link_id: link_id.clone(),
            profile_id: profile_id.clone(),
            person_id: "private-person-canary".to_string(),
            display_name: "private-label-canary".to_string(),
            account_ref: None,
            vault_id: vault_id.clone(),
            relationship_revision: 1,
            profile_revision: 1,
        };
        let reference = seal_person_identity(&payload, &aad, &key_ref, &backend).unwrap();
        let profile_payload = meeting_intelligence_schema::PersonProfilePayload {
            profile_id: profile_id.clone(),
            person_id: "private-person-canary".to_string(),
            display_name: "private-label-canary".to_string(),
            vault_id: vault_id.clone(),
            revision: 1,
        };
        let profile_context = people_profile_context(&context, &profile_id, 1);
        let profile_reference = meeting_intelligence_schema::seal_person_profile(
            &profile_payload,
            &profile_context,
            &key_ref,
            &backend,
        )
        .unwrap();
        let profile_asset_id = Uuid::new_v4().simple().to_string();
        let (profile_asset_relative, profile_asset_bytes, profile_asset_retention) =
            write_people_profile_asset(
                &canonical_data_root(&storage).unwrap(),
                &profile_asset_id,
                &profile_reference,
            )
            .unwrap();
        let followup_audio_path = add_audio_chunk(
            &storage,
            audio_root.path(),
            "identity-followup-chunk",
            1,
            1000,
            2000,
            b"identity-followup-audio",
        );
        commit_rows(
            &storage,
            vec![
                upsert(
                    "identity_vaults",
                    identity_vault_row(&vault_id, &context.owner_principal_ref, None, "active"),
                ),
                upsert(
                    "participant_profiles",
                    identity_profile_row(
                        &profile_id,
                        &profile_asset_id,
                        &context.vault_id,
                        &local_owner_collection_scope(&context.vault_id),
                        &profile_reference,
                        &key_ref,
                        "active",
                        1,
                    ),
                ),
                upsert(
                    "meeting_private_assets",
                    profile_asset_row(
                        &profile_asset_id,
                        &scope.project_id,
                        &context,
                        &profile_id,
                        1,
                        &key_ref,
                        &profile_asset_relative,
                        &profile_reference.ciphertext_sha256,
                        profile_asset_bytes,
                        profile_asset_retention,
                        "2026-09-21T10:00:00Z",
                    ),
                ),
                upsert(
                    "speaker_identity_links",
                    identity_link_row(
                        &scope,
                        &context,
                        &link_id,
                        &reference,
                        &evidence_ref,
                        &evidence_sha256,
                        1,
                        "confirmed",
                        None,
                        None,
                    ),
                ),
            ],
        )
        .unwrap();
        IdentityFixture {
            path,
            storage,
            audio_root,
            audio_path: followup_audio_path,
            scope,
            context,
            backend,
            link_id,
            profile_id,
            profile_asset_id,
            profile_reference,
            evidence_ref,
            evidence_sha256,
            key_ref,
            reference,
            owner_source,
            lifecycle_source,
        }
    }

    fn identity_request(fixture: &IdentityFixture, event_id: &str) -> AtomicMeetingRequest {
        let audio = std::fs::read(&fixture.audio_path).unwrap();
        let mut request = meeting_request(
            &fixture.scope,
            event_id,
            audio_source(
                &fixture.scope,
                &format!("coverage-{event_id}"),
                "identity-followup-chunk",
                &fixture.audio_path,
                1,
                1000,
                2000,
                Some(sha256_bytes(&audio)),
            ),
            1,
            1,
            &format!("revision-{event_id}"),
            &format!("utterance-{event_id}"),
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "ordinary spoken transcript",
            "unreviewed",
        );
        request.attribution.kind = "confirmed_person".to_string();
        request.attribution.speaker_cluster_id = Some("speaker-identity".to_string());
        request.attribution.label_snapshot_ref = Some("speaker-identity".to_string());
        request.attribution.identity_link_id = Some(fixture.link_id.clone());
        request.attribution.identity_expected_revision = Some(1);
        request.attribution.evidence_revision = 1;
        request
    }

    fn identity_aad_context(fixture: &IdentityFixture) -> IdentityAadContext {
        IdentityAadContext {
            account_ref: fixture.context.account_ref.clone(),
            scope: identity_scope_key(&fixture.scope),
            vault_id: fixture.context.vault_id.clone(),
            entity_id: fixture.link_id.clone(),
            revision: 1,
            model_context: manual_identity_model_context(
                "speaker-identity",
                &fixture.evidence_sha256,
            ),
        }
    }

    fn assert_private_canaries_absent<T: Serialize>(value: &T) {
        let encoded = serde_json::to_string(value).unwrap();
        assert!(
            !encoded.contains("private-person-canary"),
            "private person identity leaked into durable/event output: {encoded}"
        );
        assert!(
            !encoded.contains("private-label-canary"),
            "private display label leaked into durable/event output: {encoded}"
        );
    }

    fn identity_attempt(fixture: &IdentityFixture, transaction_id: &str) -> MeetingCommitAttempt {
        begin_meeting_commit(&fixture.storage, transaction_id, "2026-09-21T10:01:00Z").unwrap()
    }

    fn record_hook_error(slot: &Arc<Mutex<Option<String>>>, message: impl Into<String>) {
        let mut error = slot.lock().unwrap();
        if error.is_none() {
            *error = Some(message.into());
        }
    }

    fn finish_contender(
        contender_slot: &Arc<
            Mutex<
                Option<
                    std::thread::JoinHandle<Result<crate::auth_session::LifecycleOutcome, String>>,
                >,
            >,
        >,
        completed_rx: &std::sync::mpsc::Receiver<()>,
    ) -> Result<crate::auth_session::LifecycleOutcome, String> {
        let completion = completed_rx.recv_timeout(Duration::from_secs(1));
        let handle = contender_slot.lock().unwrap().take();
        let joined = if let Some(handle) = handle {
            let (joined_tx, joined_rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let result = handle
                    .join()
                    .map_err(|_| "account-switch contender panicked".to_string())
                    .and_then(|result| result);
                let _ = joined_tx.send(result);
            });
            joined_rx
                .recv_timeout(Duration::from_secs(1))
                .map_err(|_| "account-switch contender join did not complete".to_string())?
        } else {
            return Err("account-switch contender handle was not recorded".to_string());
        };
        completion
            .map_err(|_| "account-switch contender did not complete before join".to_string())?;
        joined
    }

    fn identity_commit_effects(
        storage: &Storage,
        event_id: &str,
    ) -> Result<[Vec<Value>; 5], String> {
        Ok([
            query(
                storage,
                "meeting_source_coverage",
                &["id"],
                vec![eq(
                    "meeting_source_coverage",
                    "id",
                    json!(format!("coverage-{event_id}")),
                )],
                1,
            )?,
            query(
                storage,
                "transcript_revisions",
                &["id"],
                vec![eq(
                    "transcript_revisions",
                    "id",
                    json!(format!("revision-{event_id}")),
                )],
                1,
            )?,
            query(
                storage,
                "transcript_projection",
                &["id"],
                vec![eq(
                    "transcript_projection",
                    "id",
                    json!(format!("utterance-{event_id}")),
                )],
                1,
            )?,
            query(
                storage,
                "transcript_event_log",
                &["id"],
                vec![eq("transcript_event_log", "id", json!(event_id))],
                1,
            )?,
            query(
                storage,
                "meeting_source_cursors",
                &["id", "last_sequence", "last_event_cursor"],
                vec![eq(
                    "meeting_source_cursors",
                    "id",
                    json!("n3-source::mic-1::1"),
                )],
                1,
            )?,
        ])
    }

    #[test]
    fn r2_native_identity_uses_app_data_parent_of_storage_root() {
        let fixture = identity_fixture();
        let _unlock = identity_unlock(&fixture);
        let storage_root = canonical_data_root(&fixture.storage).unwrap();
        let app_data_root = std::fs::canonicalize(fixture.storage.path.parent().unwrap()).unwrap();
        let captured = fixture
            .owner_source
            .captured_path
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(captured, app_data_root);
        assert_ne!(captured, storage_root);
        assert_eq!(
            storage_root,
            std::fs::canonicalize(&fixture.storage.path).unwrap()
        );
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn r2_native_unlock_is_bound_to_canonical_root_and_generation() {
        let first = identity_fixture();
        let first_session = identity_unlock(&first);
        let second = identity_fixture();
        let root_error = revalidate_native_local_owner_session(
            &second.storage,
            &first_session,
            &second.lifecycle_source,
        )
        .unwrap_err();
        assert!(root_error.contains("different data root"), "{root_error}");
        let first_path = first.path.clone();
        let second_path = second.path.clone();
        drop(first);
        drop(second);
        let _ = std::fs::remove_dir_all(first_path);
        let _ = std::fs::remove_dir_all(second_path);

        let generation_fixture = identity_fixture();
        let prior_generation = identity_unlock(&generation_fixture);
        let current_generation = identity_unlock(&generation_fixture);
        let generation_error = revalidate_native_local_owner_session(
            &generation_fixture.storage,
            &prior_generation,
            &generation_fixture.lifecycle_source,
        )
        .unwrap_err();
        assert!(generation_error.contains("stale"), "{generation_error}");
        current_generation.lock();
        let lock_error = revalidate_native_local_owner_session(
            &generation_fixture.storage,
            &current_generation,
            &generation_fixture.lifecycle_source,
        )
        .unwrap_err();
        assert!(
            lock_error.contains("stale") || lock_error.contains("locked"),
            "{lock_error}"
        );
        let path = generation_fixture.path.clone();
        drop(generation_fixture);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn r2_vault_operation_fence_serializes_lock_and_revoke() {
        let lock_fixture = identity_fixture();
        let lock_session = identity_unlock(&lock_fixture);
        let (lock_attempt_tx, lock_attempt_rx) = std::sync::mpsc::sync_channel(0);
        let (lock_completed_tx, lock_completed_rx_inner) = std::sync::mpsc::channel();
        let lock_completed_rx = Arc::new(Mutex::new(lock_completed_rx_inner));
        let lock_handle_slot = Arc::new(Mutex::new(None));
        let lock_thread_session = Arc::new(lock_session.clone());
        let lock_handle_slot_for_hook = lock_handle_slot.clone();
        let lock_attempt_tx_for_hook = lock_attempt_tx.clone();
        let lock_completed_tx_for_hook = lock_completed_tx.clone();
        let lock_completed_rx_for_hook = lock_completed_rx.clone();
        let lock_hook = move || {
            let session = lock_thread_session.clone();
            let attempt = lock_attempt_tx_for_hook.clone();
            let completed = lock_completed_tx_for_hook.clone();
            let handle = std::thread::spawn(move || {
                attempt.send(()).unwrap();
                session.lock();
                completed.send(()).unwrap();
            });
            *lock_handle_slot_for_hook.lock().unwrap() = Some(handle);
            lock_attempt_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            assert!(
                lock_completed_rx_for_hook
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(1))
                    .is_err(),
                "vault lock completed before the protected commit"
            );
        };
        let lock_request = identity_request(&lock_fixture, "event-r2-lock-fence");
        let lock_result = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &lock_fixture.storage,
            &lock_session,
            &identity_attempt(&lock_fixture, "tx-r2-lock-fence"),
            &lock_request,
            &lock_fixture.backend,
            &lock_fixture.lifecycle_source,
            None,
            None,
            Some(&lock_hook),
            None,
        )
        .unwrap();
        assert!(!lock_result.idempotent);
        lock_completed_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        lock_handle_slot
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .unwrap();
        let lock_retry = commit_meeting_transcript_with_test_unlock_and_backend(
            &lock_fixture.storage,
            &lock_session,
            &identity_attempt(&lock_fixture, "tx-r2-lock-retry"),
            &lock_request,
            &lock_fixture.backend,
            &lock_fixture.lifecycle_source,
            None,
        )
        .unwrap_err();
        assert!(
            lock_retry.contains("stale") || lock_retry.contains("locked"),
            "{lock_retry}"
        );
        let lock_path = lock_fixture.path.clone();
        drop(lock_fixture);
        let _ = std::fs::remove_dir_all(lock_path);

        let revoke_fixture = identity_fixture();
        let prior_session = identity_unlock(&revoke_fixture);
        let current_session = identity_unlock(&revoke_fixture);
        let prior_error = revalidate_native_local_owner_session(
            &revoke_fixture.storage,
            &prior_session,
            &revoke_fixture.lifecycle_source,
        )
        .unwrap_err();
        assert!(prior_error.contains("stale"), "{prior_error}");
        let (revoke_attempt_tx, revoke_attempt_rx) = std::sync::mpsc::sync_channel(0);
        let (revoke_completed_tx, revoke_completed_rx_inner) = std::sync::mpsc::channel();
        let revoke_completed_rx = Arc::new(Mutex::new(revoke_completed_rx_inner));
        let revoke_handle_slot = Arc::new(Mutex::new(None));
        let revoke_thread_session = Arc::new(prior_session.clone());
        let revoke_handle_slot_for_hook = revoke_handle_slot.clone();
        let revoke_attempt_tx_for_hook = revoke_attempt_tx.clone();
        let revoke_completed_tx_for_hook = revoke_completed_tx.clone();
        let revoke_completed_rx_for_hook = revoke_completed_rx.clone();
        let revoke_hook = move || {
            let session = revoke_thread_session.clone();
            let attempt = revoke_attempt_tx_for_hook.clone();
            let completed = revoke_completed_tx_for_hook.clone();
            let handle = std::thread::spawn(move || {
                attempt.send(()).unwrap();
                session.revoke();
                completed.send(()).unwrap();
            });
            *revoke_handle_slot_for_hook.lock().unwrap() = Some(handle);
            revoke_attempt_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            assert!(
                revoke_completed_rx_for_hook
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(1))
                    .is_err(),
                "vault revoke completed before the protected commit"
            );
        };
        let revoke_request = identity_request(&revoke_fixture, "event-r2-revoke-fence");
        let revoke_result = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &revoke_fixture.storage,
            &current_session,
            &identity_attempt(&revoke_fixture, "tx-r2-revoke-fence"),
            &revoke_request,
            &revoke_fixture.backend,
            &revoke_fixture.lifecycle_source,
            None,
            None,
            Some(&revoke_hook),
            None,
        )
        .unwrap();
        assert!(!revoke_result.idempotent);
        revoke_completed_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        revoke_handle_slot
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .unwrap();
        let revoke_retry = commit_meeting_transcript_with_test_unlock_and_backend(
            &revoke_fixture.storage,
            &current_session,
            &identity_attempt(&revoke_fixture, "tx-r2-revoke-retry"),
            &revoke_request,
            &revoke_fixture.backend,
            &revoke_fixture.lifecycle_source,
            None,
        )
        .unwrap_err();
        assert!(
            revoke_retry.contains("stale") || revoke_retry.contains("revoked"),
            "{revoke_retry}"
        );
        let revoke_path = revoke_fixture.path.clone();
        drop(revoke_fixture);
        let _ = std::fs::remove_dir_all(revoke_path);
    }

    #[test]
    fn r2_native_unlock_uses_registered_broker_short_guard_for_commit() {
        let fixture = identity_fixture();
        let mut broker = crate::auth_session::tests::TestAccountOperationHarness::new();
        broker.switch_account_for_test("account-a").unwrap();
        let account_guard = broker.take_guard();
        let lifecycle_source = HarnessLifecycleWitnessSource { harness: &broker };
        let unlock = identity_unlock_with_lifecycle_source(&fixture, &lifecycle_source);
        let (logout_attempt_tx, logout_attempt_rx) = std::sync::mpsc::sync_channel(0);
        let logout_completed = Arc::new(AtomicBool::new(false));
        let logout_handle_slot = Arc::new(Mutex::new(None));
        let logout_attempt_tx_for_hook = logout_attempt_tx.clone();
        let logout_completed_for_hook = logout_completed.clone();
        let logout_handle_slot_for_hook = logout_handle_slot.clone();
        let post_fence_hook = || {
            let handle = broker.spawn_logout(
                logout_attempt_tx_for_hook.clone(),
                logout_completed_for_hook.clone(),
            );
            *logout_handle_slot_for_hook.lock().unwrap() = Some(handle);
            logout_attempt_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            assert!(
                !logout_completed.load(Ordering::Acquire),
                "broker logout completed before the protected commit"
            );
        };
        let request = identity_request(&fixture, "event-r2-registered-broker");
        let committed = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &fixture.storage,
            &unlock,
            &identity_attempt(&fixture, "tx-r2-registered-broker"),
            &request,
            &fixture.backend,
            &lifecycle_source,
            Some(account_guard),
            None,
            None,
            Some(&post_fence_hook),
        )
        .unwrap();
        assert!(!committed.idempotent);
        let logout = logout_handle_slot
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert!(logout_completed.load(Ordering::Acquire));
        assert_eq!(logout.state, "signed_out");
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn r3_protected_commit_rejects_missing_account_guard_without_effects() {
        let fixture = identity_fixture();
        let unlock = identity_unlock(&fixture);
        let request = identity_request(&fixture, "event-r3-missing-account-guard");
        let attempt = identity_attempt(&fixture, "tx-r3-missing-account-guard");
        let effects_before = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let result = commit_meeting_transcript_with_backend_and_guard_impl(
            &fixture.storage,
            &attempt,
            &request,
            Some(&unlock.context),
            &fixture.backend,
            None,
            Some((&unlock, &fixture.lifecycle_source)),
            CommitTestHooks::none(),
        );
        let effects_after = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);

        assert_eq!(
            result,
            Err("native account operation is unavailable".to_string())
        );
        assert_eq!(effects_after, effects_before);
    }

    #[test]
    fn r3_contender_first_switch_before_broker_fence_rejects_without_partial_effects() {
        let fixture = identity_fixture();
        let mut broker = crate::auth_session::tests::TestAccountOperationHarness::new();
        broker.switch_account_for_test("account-a").unwrap();
        let account_guard = broker.take_guard();
        let lifecycle_source = HarnessLifecycleWitnessSource { harness: &broker };
        let unlock = identity_unlock_with_lifecycle_source(&fixture, &lifecycle_source);
        let (attempted_tx, attempted_rx) = std::sync::mpsc::sync_channel(1);
        let (lock_state_tx, lock_state_rx) = std::sync::mpsc::sync_channel(1);
        let (linearized_tx, linearized_rx) = std::sync::mpsc::sync_channel(1);
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let contender_slot = Arc::new(Mutex::new(
            None::<std::thread::JoinHandle<Result<crate::auth_session::LifecycleOutcome, String>>>,
        ));
        let hook_error = Arc::new(Mutex::new(None::<String>));
        let contender_slot_for_hook = contender_slot.clone();
        let hook_error_for_hook = hook_error.clone();
        let before_broker_fence = || {
            let handle = broker.spawn_account_switch_contender(
                "account-b",
                attempted_tx.clone(),
                lock_state_tx.clone(),
                linearized_tx.clone(),
                completed_tx.clone(),
            );
            *contender_slot_for_hook.lock().unwrap() = Some(handle);
            if attempted_rx.recv_timeout(Duration::from_secs(1)).is_err() {
                record_hook_error(&hook_error_for_hook, "contender did not announce attempt");
                return;
            }
            match lock_state_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(true) => record_hook_error(
                    &hook_error_for_hook,
                    "contender unexpectedly observed the broker fence as held",
                ),
                Ok(false) => {}
                Err(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender did not report try-lock state",
                ),
            }
            match linearized_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(witness) if witness.native_user_id.as_deref() == Some("account-b") => {}
                Ok(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender linearized with an unexpected lifecycle witness",
                ),
                Err(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender did not linearize before broker-fence acquisition",
                ),
            }
        };
        let request = identity_request(&fixture, "event-r3-contender-first");
        let effects_before = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let commit_result = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &fixture.storage,
            &unlock,
            &identity_attempt(&fixture, "tx-r3-contender-first"),
            &request,
            &fixture.backend,
            &lifecycle_source,
            Some(account_guard),
            Some(&before_broker_fence),
            None,
            None,
        );
        let contender_result = finish_contender(&contender_slot, &completed_rx);
        let after_switch = lifecycle_source.read();
        let effects_after = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let hook_error = hook_error.lock().unwrap().clone();
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);

        assert!(hook_error.is_none(), "{hook_error:?}");
        assert!(matches!(
            contender_result,
            Ok(outcome) if outcome.state == "authenticated"
        ));
        let commit_error = commit_result
            .err()
            .unwrap_or_else(|| "protected commit unexpectedly succeeded".to_string());
        assert!(
            commit_error.contains("invalidated"),
            "unexpected commit result: {commit_error}"
        );
        let after_switch = after_switch.unwrap();
        assert_eq!(after_switch.native_user_id.as_deref(), Some("account-b"));
        assert_eq!(after_switch.state, "authenticated");
        assert_eq!(effects_after, effects_before);
    }

    #[test]
    fn r3_commit_fence_first_blocks_switch_until_successful_commit() {
        let fixture = identity_fixture();
        let mut broker = crate::auth_session::tests::TestAccountOperationHarness::new();
        broker.switch_account_for_test("account-a").unwrap();
        let account_guard = broker.take_guard();
        let lifecycle_source = HarnessLifecycleWitnessSource { harness: &broker };
        let unlock = identity_unlock_with_lifecycle_source(&fixture, &lifecycle_source);
        let (attempted_tx, attempted_rx) = std::sync::mpsc::sync_channel(1);
        let (lock_state_tx, lock_state_rx) = std::sync::mpsc::sync_channel(1);
        let (linearized_tx, linearized_rx) = std::sync::mpsc::sync_channel(1);
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let contender_slot = Arc::new(Mutex::new(
            None::<std::thread::JoinHandle<Result<crate::auth_session::LifecycleOutcome, String>>>,
        ));
        let hook_error = Arc::new(Mutex::new(None::<String>));
        let contender_slot_for_hook = contender_slot.clone();
        let hook_error_for_hook = hook_error.clone();
        let broker_fence = || {
            let handle = broker.spawn_account_switch_contender(
                "account-b",
                attempted_tx.clone(),
                lock_state_tx.clone(),
                linearized_tx.clone(),
                completed_tx.clone(),
            );
            *contender_slot_for_hook.lock().unwrap() = Some(handle);
            if attempted_rx.recv_timeout(Duration::from_secs(1)).is_err() {
                record_hook_error(&hook_error_for_hook, "contender did not announce attempt");
                return;
            }
            match lock_state_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(true) => {}
                Ok(false) => record_hook_error(
                    &hook_error_for_hook,
                    "contender try-lock did not observe the held broker fence",
                ),
                Err(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender did not report try-lock state",
                ),
            }
            match linearized_rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Ok(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender linearized while the broker fence was held",
                ),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => record_hook_error(
                    &hook_error_for_hook,
                    "contender linearization channel disconnected",
                ),
            }
        };
        let request = identity_request(&fixture, "event-r3-fence-first-success");
        let effects_before = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let commit_result = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &fixture.storage,
            &unlock,
            &identity_attempt(&fixture, "tx-r3-fence-first-success"),
            &request,
            &fixture.backend,
            &lifecycle_source,
            Some(account_guard),
            None,
            None,
            Some(&broker_fence),
        );
        let contender_result = finish_contender(&contender_slot, &completed_rx);
        let linearized_witness = linearized_rx.recv_timeout(Duration::from_secs(1));
        let effects_after = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let hook_error = hook_error.lock().unwrap().clone();
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);

        assert!(hook_error.is_none(), "{hook_error:?}");
        assert!(commit_result.is_ok(), "{commit_result:?}");
        assert!(matches!(
            contender_result,
            Ok(outcome) if outcome.state == "authenticated"
        ));
        assert_eq!(
            linearized_witness.unwrap().native_user_id.as_deref(),
            Some("account-b")
        );
        assert!(effects_before[..4].iter().all(|rows| rows.is_empty()));
        assert_eq!(effects_before[4].len(), 1);
        assert!(effects_after[..4].iter().all(|rows| rows.len() == 1));
        assert_eq!(effects_after[4].len(), 1);
        assert_ne!(effects_before[4], effects_after[4]);
    }

    #[test]
    fn r3_commit_fence_error_releases_before_guard_teardown() {
        let fixture = identity_fixture();
        let mut broker = crate::auth_session::tests::TestAccountOperationHarness::new();
        broker.switch_account_for_test("account-a").unwrap();
        let account_guard = broker.take_guard();
        let lifecycle_source = HarnessLifecycleWitnessSource { harness: &broker };
        let unlock = identity_unlock_with_lifecycle_source(&fixture, &lifecycle_source);
        let (attempted_tx, attempted_rx) = std::sync::mpsc::sync_channel(1);
        let (lock_state_tx, lock_state_rx) = std::sync::mpsc::sync_channel(1);
        let (linearized_tx, linearized_rx) = std::sync::mpsc::sync_channel(1);
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let contender_slot = Arc::new(Mutex::new(
            None::<std::thread::JoinHandle<Result<crate::auth_session::LifecycleOutcome, String>>>,
        ));
        let hook_error = Arc::new(Mutex::new(None::<String>));
        let contender_slot_for_hook = contender_slot.clone();
        let hook_error_for_hook = hook_error.clone();
        let broker_fence = || {
            let handle = broker.spawn_account_switch_contender(
                "account-b",
                attempted_tx.clone(),
                lock_state_tx.clone(),
                linearized_tx.clone(),
                completed_tx.clone(),
            );
            *contender_slot_for_hook.lock().unwrap() = Some(handle);
            if attempted_rx.recv_timeout(Duration::from_secs(1)).is_err() {
                record_hook_error(&hook_error_for_hook, "contender did not announce attempt");
                return;
            }
            match lock_state_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(true) => {}
                Ok(false) => record_hook_error(
                    &hook_error_for_hook,
                    "contender try-lock did not observe the held broker fence",
                ),
                Err(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender did not report try-lock state",
                ),
            }
            match linearized_rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Ok(_) => record_hook_error(
                    &hook_error_for_hook,
                    "contender linearized while the broker fence was held",
                ),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => record_hook_error(
                    &hook_error_for_hook,
                    "contender linearization channel disconnected",
                ),
            }
        };
        let request = identity_request(&fixture, "event-r3-fence-first-error");
        let mut stale_attempt = identity_attempt(&fixture, "tx-r3-fence-first-error");
        stale_attempt.expected_frontier = stale_attempt.expected_frontier.saturating_add(1);
        let effects_before = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let stale_transaction_id = stale_attempt.transaction_id.clone();
        let commit_result = commit_meeting_transcript_with_test_unlock_and_backend_options(
            &fixture.storage,
            &unlock,
            &stale_attempt,
            &request,
            &fixture.backend,
            &lifecycle_source,
            Some(account_guard),
            None,
            None,
            Some(&broker_fence),
        );
        let contender_result = finish_contender(&contender_slot, &completed_rx);
        let linearized_witness = linearized_rx.recv_timeout(Duration::from_secs(1));
        let effects_after = identity_commit_effects(&fixture.storage, &request.event_id).unwrap();
        let hook_error = hook_error.lock().unwrap().clone();
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);

        assert!(hook_error.is_none(), "{hook_error:?}");
        assert!(matches!(
            commit_result,
            Err(ref error) if error.contains("MEETING_COMMIT_REJECTED")
        ));
        assert!(matches!(
            contender_result,
            Ok(outcome) if outcome.state == "authenticated"
        ));
        assert_eq!(
            linearized_witness.unwrap().native_user_id.as_deref(),
            Some("account-b")
        );
        assert_eq!(stale_attempt.transaction_id, stale_transaction_id);
        assert_eq!(effects_after, effects_before);
    }

    #[test]
    fn r1_positive_person_link_is_usable_but_durable_outputs_are_opaque() {
        let fixture = identity_fixture();
        let resolved_local = resolve_native_identity_context(
            &fixture.storage,
            &fixture.context.owner_principal_ref,
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved_local.vault_id, fixture.context.vault_id);
        assert_eq!(resolved_local.account_ref, None);
        let request = identity_request(&fixture, "event-identity-positive");
        let mut later_asr_request = request.clone();
        later_asr_request.revision.model_run_id = Some("asr-run-unrelated".to_string());
        let later_asr_authorized = check_identity_link_scope(
            &fixture.storage,
            &later_asr_request,
            Some(&fixture.context),
            &fixture.backend,
        )
        .unwrap()
        .unwrap();
        assert_eq!(later_asr_authorized.profile_id, fixture.profile_id);
        drop(later_asr_authorized);
        let authorized = check_identity_link_scope(
            &fixture.storage,
            &request,
            Some(&fixture.context),
            &fixture.backend,
        )
        .unwrap()
        .unwrap();
        assert_eq!(authorized.person_id, "private-person-canary");
        assert_eq!(authorized.display_name, "private-label-canary");
        assert_eq!(authorized.profile_id, fixture.profile_id);
        drop(authorized);

        let unlock = identity_unlock(&fixture);
        let attempt = identity_attempt(&fixture, "tx-identity-positive");
        let committed = commit_meeting_transcript_with_test_unlock_and_backend(
            &fixture.storage,
            &unlock,
            &attempt,
            &request,
            &fixture.backend,
            &fixture.lifecycle_source,
            None,
        )
        .unwrap();
        assert!(!committed.idempotent);
        assert_private_canaries_absent(&committed);

        for (table, id) in [
            (
                "meeting_source_coverage",
                "coverage-event-identity-positive",
            ),
            ("transcript_revisions", "revision-event-identity-positive"),
            ("transcript_projection", "utterance-event-identity-positive"),
            ("transcript_event_log", "event-identity-positive"),
            ("meeting_source_cursors", "n3-source::mic-1::1"),
        ] {
            assert_eq!(
                query(
                    &fixture.storage,
                    table,
                    &["id"],
                    vec![eq(table, "id", json!(id))],
                    1,
                )
                .unwrap()
                .len(),
                1,
                "identity commit must atomically write {table}"
            );
        }

        let revision = query(
            &fixture.storage,
            "transcript_revisions",
            &["raw_text", "attribution_json"],
            vec![eq(
                "transcript_revisions",
                "id",
                json!("revision-event-identity-positive"),
            )],
            1,
        )
        .unwrap();
        assert_eq!(
            revision[0]["transcript_revisions.raw_text"],
            "ordinary spoken transcript"
        );
        assert_private_canaries_absent(&revision);

        let projection = query(
            &fixture.storage,
            "transcript_projection",
            &["effective_text"],
            vec![eq(
                "transcript_projection",
                "id",
                json!("utterance-event-identity-positive"),
            )],
            1,
        )
        .unwrap();
        assert_private_canaries_absent(&projection);
        let event = query(
            &fixture.storage,
            "transcript_event_log",
            &["payload_json"],
            vec![eq(
                "transcript_event_log",
                "id",
                json!("event-identity-positive"),
            )],
            1,
        )
        .unwrap();
        assert_private_canaries_absent(&event);
        let link = query(
            &fixture.storage,
            "speaker_identity_links",
            &[
                "person_ref_ciphertext_ref",
                "person_ref_ciphertext_sha256",
                "person_ref_ciphertext_json",
                "person_ref_key_ref",
            ],
            vec![eq("speaker_identity_links", "id", json!(&fixture.link_id))],
            1,
        )
        .unwrap();
        assert_private_canaries_absent(&link);
        let profile = query(
            &fixture.storage,
            "participant_profiles",
            &[
                "id",
                "vault_id",
                "owner_scope",
                "profile_payload_ciphertext_ref",
                "profile_ciphertext_sha256",
                "key_ref",
                "status",
                "revision",
            ],
            vec![eq("participant_profiles", "id", json!(&fixture.profile_id))],
            1,
        )
        .unwrap();
        assert_private_canaries_absent(&profile);

        let replay = commit_meeting_transcript_with_test_unlock_and_backend(
            &fixture.storage,
            &unlock,
            &attempt,
            &request,
            &fixture.backend,
            &fixture.lifecycle_source,
            None,
        )
        .unwrap();
        assert!(replay.idempotent);
        let mut changed = request.clone();
        changed.revision.effective_text = "changed identity payload".to_string();
        let changed_error = commit_meeting_transcript_with_test_unlock_and_backend(
            &fixture.storage,
            &unlock,
            &identity_attempt(&fixture, "tx-identity-changed"),
            &changed,
            &fixture.backend,
            &fixture.lifecycle_source,
            None,
        )
        .unwrap_err();
        assert!(
            changed_error.contains("EVENT_ID_CONFLICT"),
            "{changed_error}"
        );
        assert_eq!(
            query(
                &fixture.storage,
                "transcript_revisions",
                &["id"],
                vec![eq(
                    "transcript_revisions",
                    "meeting_session_id",
                    json!(&fixture.scope.meeting_session_id),
                )],
                10,
            )
            .unwrap()
            .len(),
            2
        );

        let path = fixture.path.clone();
        drop(fixture);
        let reopened = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&reopened).unwrap();
        for (table, id, column) in [
            (
                "transcript_revisions",
                "revision-event-identity-positive",
                "attribution_json",
            ),
            (
                "transcript_event_log",
                "event-identity-positive",
                "payload_json",
            ),
            (
                "transcript_projection",
                "utterance-event-identity-positive",
                "effective_text",
            ),
        ] {
            let rows = query(
                &reopened,
                table,
                &[column],
                vec![eq(table, "id", json!(id))],
                1,
            )
            .unwrap();
            assert_private_canaries_absent(&rows);
        }
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn r1_identity_crypto_and_caller_claims_fail_closed_without_partial_commit() {
        let fixture = identity_fixture();
        let unlock = identity_unlock(&fixture);
        let request = identity_request(&fixture, "event-identity-crypto");
        let missing_key_backend = InMemoryIdentityKeyBackend::default();
        let missing_key_error = check_identity_link_scope(
            &fixture.storage,
            &request,
            Some(&fixture.context),
            &missing_key_backend,
        )
        .unwrap_err();
        assert!(missing_key_error.contains("key is unavailable"));

        let mut wrong_key_backend = InMemoryIdentityKeyBackend::default();
        wrong_key_backend.insert(&fixture.key_ref, vec![8; 32]);
        let wrong_key_error = check_identity_link_scope(
            &fixture.storage,
            &request,
            Some(&fixture.context),
            &wrong_key_backend,
        )
        .unwrap_err();
        assert!(wrong_key_error.contains("authentication failed"));

        let aad = identity_aad_context(&fixture);
        let mut tampered = fixture.reference.clone();
        tampered.envelope.ciphertext[0] ^= 1;
        tampered.ciphertext_sha256 = sha256_bytes(&tampered.envelope.ciphertext);
        let tamper_error =
            open_person_identity(&tampered, &aad, &fixture.link_id, 1, &fixture.backend)
                .unwrap_err();
        assert!(tamper_error.contains("authentication failed"));

        let mut swapped_scope = aad.clone();
        swapped_scope.scope.push_str("::cross-scope");
        let aad_error = open_person_identity(
            &fixture.reference,
            &swapped_scope,
            &fixture.link_id,
            1,
            &fixture.backend,
        )
        .unwrap_err();
        assert!(aad_error.contains("AAD context mismatch"));

        let mut swapped_vault = aad.clone();
        swapped_vault.vault_id = "vault:other".to_string();
        let vault_error = open_person_identity(
            &fixture.reference,
            &swapped_vault,
            &fixture.link_id,
            1,
            &fixture.backend,
        )
        .unwrap_err();
        assert!(vault_error.contains("AAD context mismatch"));

        let provenance = identity_fixture();
        commit_rows(
            &provenance.storage,
            vec![
                upsert(
                    "model_providers",
                    json!({
                        "id": "identity-provenance-provider",
                        "label": "identity provenance fixture",
                        "runtime_location": "local",
                        "kind": "identity_review",
                        "enabled": true,
                        "config_json": {},
                        "created_at": "2026-09-21T10:00:00Z",
                        "updated_at": "2026-09-21T10:00:00Z",
                    }),
                ),
                upsert(
                    "model_runs",
                    json!({
                        "id": "identity-model-provenance",
                        "recording_id": provenance.scope.recording_id,
                        "provider_id": "identity-provenance-provider",
                        "model_name": "identity-review-fixture",
                        "task_kind": "identity_review",
                        "runtime_location": "local",
                        "input_ref": "identity-review-input",
                        "output_ref": "identity-review-output",
                        "parameters_json": {},
                        "created_at": "2026-09-21T10:00:00Z",
                    }),
                ),
                upsert(
                    "speaker_identity_links",
                    identity_link_row_with_model(
                        &provenance.scope,
                        &provenance.context,
                        &provenance.link_id,
                        &provenance.reference,
                        &provenance.evidence_ref,
                        &provenance.evidence_sha256,
                        1,
                        Some("identity-model-provenance"),
                        "confirmed",
                        None,
                        None,
                    ),
                ),
            ],
        )
        .unwrap();
        let provenance_error = check_identity_link_scope(
            &provenance.storage,
            &identity_request(&provenance, "event-identity-tampered-provenance"),
            Some(&provenance.context),
            &provenance.backend,
        )
        .unwrap_err();
        assert!(provenance_error.contains("AAD context mismatch"));
        let provenance_path = provenance.path.clone();
        drop(provenance);
        let _ = std::fs::remove_dir_all(provenance_path);

        let semantic_error = open_person_identity(
            &fixture.reference,
            &aad,
            "identity-link:33333333333333333333333333333333",
            1,
            &fixture.backend,
        )
        .unwrap_err();
        assert!(semantic_error.contains("semantic relationship mismatch"));

        let mut placeholder = fixture.reference.clone();
        placeholder.encrypted_blob_ref =
            "local-ciphertext:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string();
        assert!(validate_private_identity_reference(&placeholder).is_err());

        let mut claimed_key = fixture.reference.clone();
        claimed_key.envelope.key_ref = format!("people_metadata:{}", "4".repeat(32));
        let mut injected = request.clone();
        injected.attribution.person_ref_ciphertext = Some(claimed_key);
        let injected_error = check_identity_link_scope(
            &fixture.storage,
            &injected,
            Some(&fixture.context),
            &fixture.backend,
        )
        .unwrap_err();
        assert!(injected_error.contains("does not match native custody"));

        let mut provider_only = request.clone();
        provider_only.attribution.kind = "confirmed_person".to_string();
        provider_only.attribution.identity_link_id = None;
        provider_only.attribution.identity_expected_revision = None;
        provider_only.attribution.person_ref_ciphertext = None;
        provider_only.attribution.evidence_revision = 0;
        assert!(provider_only.validate().is_err());

        let failed_commit_error = commit_meeting_transcript_with_test_unlock_and_backend(
            &fixture.storage,
            &unlock,
            &identity_attempt(&fixture, "tx-identity-missing-key"),
            &request,
            &missing_key_backend,
            &fixture.lifecycle_source,
            None,
        )
        .unwrap_err();
        assert!(failed_commit_error.contains("key is unavailable"));
        for (table, id) in [
            ("meeting_source_coverage", "coverage-event-identity-crypto"),
            ("transcript_revisions", "revision-event-identity-crypto"),
            ("transcript_projection", "utterance-event-identity-crypto"),
            ("transcript_event_log", "event-identity-crypto"),
        ] {
            assert!(
                query(
                    &fixture.storage,
                    table,
                    &["id"],
                    vec![eq(table, "id", json!(id))],
                    1,
                )
                .unwrap()
                .is_empty(),
                "failed identity authorization must not write {table}"
            );
        }
        let path = fixture.path.clone();
        drop(fixture);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn r1_identity_requires_current_profile_and_guarded_review_state() {
        let bound = identity_fixture();
        commit_rows(
            &bound.storage,
            vec![upsert(
                "identity_vaults",
                identity_vault_row(
                    &bound.context.vault_id,
                    &bound.context.owner_principal_ref,
                    Some("account-a"),
                    "active",
                ),
            )],
        )
        .unwrap();
        assert!(resolve_native_identity_context(
            &bound.storage,
            &bound.context.owner_principal_ref,
            None,
            None,
        )
        .is_err());
        let bound_context = resolve_native_identity_context(
            &bound.storage,
            &bound.context.owner_principal_ref,
            Some("account-a"),
            None,
        )
        .unwrap();
        assert_eq!(bound_context.account_ref.as_deref(), Some("account-a"));
        assert!(resolve_native_identity_context(
            &bound.storage,
            &bound.context.owner_principal_ref,
            Some("account-b"),
            None,
        )
        .is_err());
        let bound_path = bound.path.clone();
        drop(bound);
        let _ = std::fs::remove_dir_all(bound_path);

        let missing = identity_fixture();
        commit_rows(
            &missing.storage,
            vec![delete("participant_profiles", &missing.profile_id)],
        )
        .unwrap();
        let missing_error = check_identity_link_scope(
            &missing.storage,
            &identity_request(&missing, "event-identity-profile-missing"),
            Some(&missing.context),
            &missing.backend,
        )
        .unwrap_err();
        assert!(missing_error.contains("profile is missing"));
        let missing_path = missing.path.clone();
        drop(missing);
        let _ = std::fs::remove_dir_all(missing_path);

        let deleted = identity_fixture();
        commit_rows(
            &deleted.storage,
            vec![upsert(
                "participant_profiles",
                identity_profile_row(
                    &deleted.profile_id,
                    &deleted.profile_asset_id,
                    &deleted.context.vault_id,
                    &local_owner_collection_scope(&deleted.context.vault_id),
                    &deleted.profile_reference,
                    &deleted.key_ref,
                    "deleted",
                    1,
                ),
            )],
        )
        .unwrap();
        let deleted_error = check_identity_link_scope(
            &deleted.storage,
            &identity_request(&deleted, "event-identity-profile-deleted"),
            Some(&deleted.context),
            &deleted.backend,
        )
        .unwrap_err();
        assert!(deleted_error.contains("profile is not eligible"));
        let deleted_path = deleted.path.clone();
        drop(deleted);
        let _ = std::fs::remove_dir_all(deleted_path);

        let wrong_vault = identity_fixture();
        let wrong_vault_id = "vault:other".to_string();
        commit_rows(
            &wrong_vault.storage,
            vec![
                upsert(
                    "identity_vaults",
                    identity_vault_row(
                        &wrong_vault_id,
                        &wrong_vault.context.owner_principal_ref,
                        wrong_vault.context.account_ref.as_deref(),
                        "active",
                    ),
                ),
                upsert(
                    "participant_profiles",
                    identity_profile_row(
                        &wrong_vault.profile_id,
                        &wrong_vault.profile_asset_id,
                        &wrong_vault_id,
                        &local_owner_collection_scope(&wrong_vault_id),
                        &wrong_vault.profile_reference,
                        &wrong_vault.key_ref,
                        "active",
                        1,
                    ),
                ),
            ],
        )
        .unwrap();
        let wrong_vault_error = check_identity_link_scope(
            &wrong_vault.storage,
            &identity_request(&wrong_vault, "event-identity-profile-vault"),
            Some(&wrong_vault.context),
            &wrong_vault.backend,
        )
        .unwrap_err();
        assert!(wrong_vault_error.contains("PEOPLE_PROFILE_OWNER_MISMATCH"));
        let wrong_vault_path = wrong_vault.path.clone();
        drop(wrong_vault);
        let _ = std::fs::remove_dir_all(wrong_vault_path);

        let stale = identity_fixture();
        commit_rows(
            &stale.storage,
            vec![upsert(
                "speaker_identity_links",
                identity_link_row(
                    &stale.scope,
                    &stale.context,
                    &stale.link_id,
                    &stale.reference,
                    &stale.evidence_ref,
                    &stale.evidence_sha256,
                    2,
                    "confirmed",
                    None,
                    None,
                ),
            )],
        )
        .unwrap();
        let stale_error = check_identity_link_scope(
            &stale.storage,
            &identity_request(&stale, "event-identity-review-stale"),
            Some(&stale.context),
            &stale.backend,
        )
        .unwrap_err();
        assert!(stale_error.contains("review revision is stale"));
        let stale_path = stale.path.clone();
        drop(stale);
        let _ = std::fs::remove_dir_all(stale_path);

        for (suffix, locked_at, revoked_at, expected) in [
            (
                "locked",
                Some("2026-09-21T10:02:00Z"),
                None,
                "active review scope",
            ),
            (
                "revoked",
                None,
                Some("2026-09-21T10:03:00Z"),
                "active review scope",
            ),
        ] {
            let guarded = identity_fixture();
            commit_rows(
                &guarded.storage,
                vec![upsert(
                    "speaker_identity_links",
                    identity_link_row(
                        &guarded.scope,
                        &guarded.context,
                        &guarded.link_id,
                        &guarded.reference,
                        &guarded.evidence_ref,
                        &guarded.evidence_sha256,
                        1,
                        "confirmed",
                        locked_at,
                        revoked_at,
                    ),
                )],
            )
            .unwrap();
            let guarded_error = check_identity_link_scope(
                &guarded.storage,
                &identity_request(&guarded, &format!("event-identity-{suffix}")),
                Some(&guarded.context),
                &guarded.backend,
            )
            .unwrap_err();
            assert!(guarded_error.contains(expected));
            let guarded_path = guarded.path.clone();
            drop(guarded);
            let _ = std::fs::remove_dir_all(guarded_path);
        }

        let wrong_context = identity_fixture();
        let alternate_context = TrustedIdentityContext {
            owner_principal_ref: wrong_context.context.owner_principal_ref.clone(),
            account_ref: wrong_context.context.account_ref.clone(),
            vault_id: "vault:other".to_string(),
        };
        let wrong_context_error = check_identity_link_scope(
            &wrong_context.storage,
            &identity_request(&wrong_context, "event-identity-context-vault"),
            Some(&alternate_context),
            &wrong_context.backend,
        )
        .unwrap_err();
        assert!(wrong_context_error.contains("active review scope"));
        let wrong_context_path = wrong_context.path.clone();
        drop(wrong_context);
        let _ = std::fs::remove_dir_all(wrong_context_path);
    }

    /// Reproduces the startup crash-loop: legacy SQLite stores JSON columns
    /// as TEXT and booleans as INTEGER; before `coerce_legacy_value` the
    /// import hit REL_TYPE_MISMATCH on the first jobs/model_providers row,
    /// the completion marker was never written, and every launch re-crashed.
    #[test]
    fn legacy_import_coerces_text_json_and_integer_booleans() {
        let sqlite_path = std::env::temp_dir().join(format!("fung-legacy-{}.db", Uuid::new_v4()));
        let connection = Connection::open(&sqlite_path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE projects (id TEXT PRIMARY KEY, name TEXT NOT NULL, storage_path TEXT NOT NULL,
                    active_recording_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                CREATE TABLE jobs (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, type TEXT NOT NULL,
                    status TEXT NOT NULL, progress INTEGER NOT NULL, input_refs_json TEXT NOT NULL,
                    output_refs_json TEXT NOT NULL, provider_id TEXT, error_code TEXT, error_message TEXT,
                    attempt_no INTEGER NOT NULL, started_at TEXT, finished_at TEXT,
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                CREATE TABLE model_providers (id TEXT PRIMARY KEY, label TEXT NOT NULL,
                    runtime_location TEXT NOT NULL, kind TEXT NOT NULL, enabled INTEGER NOT NULL,
                    config_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                INSERT INTO projects VALUES ('p1', 'Legacy', 'C:/tmp', NULL, '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z');
                INSERT INTO jobs VALUES ('j1', 'p1', 'transcript.transcribe', 'queued', 0, '["C:/a.wav"]', '[]',
                    NULL, NULL, NULL, 1, NULL, NULL, '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z');
                INSERT INTO model_providers VALUES ('ollama-summary-intent', 'Ollama', 'local', 'summary_intent',
                    1, '{"endpoint":"http://127.0.0.1:11434"}', '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z');
                "#,
            )
            .unwrap();
        drop(connection);

        let (genesis_path, storage) = open();
        let imported = import_legacy_sqlite(&storage, &sqlite_path)
            .expect("legacy import must survive TEXT json and INTEGER booleans");
        assert!(
            imported >= 3,
            "expected all legacy rows to import, got {imported}"
        );

        let jobs = query(&storage, "jobs", &["id", "input_refs_json"], vec![], 10).unwrap();
        assert_eq!(jobs.len(), 1);
        let providers = query(
            &storage,
            "model_providers",
            &["id", "enabled", "config_json"],
            vec![],
            10,
        )
        .unwrap();
        assert_eq!(providers.len(), 1);

        let _ = std::fs::remove_file(&sqlite_path);
        drop(storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    #[test]
    fn note_and_relation_share_genesis_row_graph_commit_path() {
        let (path, storage) = open();
        // Relative, not absolute: since WP-1.3 the engine's frontier also
        // counts the schema registrations `open()` performs, so the invariant
        // under test is the delta — one durable transaction per commit.
        let base = storage.stable_frontier();
        let timestamp = "2026-07-20T00:00:00Z".to_string();
        for id in ["note-a", "note-b"] {
            commit_note(
                &storage,
                &crate::mobile::MobileNoteInput {
                    id: id.to_string(),
                    title: id.to_string(),
                    body: "body".to_string(),
                    project_id: "project-mobile".to_string(),
                    created_at: timestamp.clone(),
                    updated_at: timestamp.clone(),
                    evidence_label: None,
                },
                "projects/project-mobile",
            )
            .unwrap();
        }
        commit_relation(
            &storage,
            "project-mobile",
            &crate::mobile::GraphEdgeInput {
                id: "edge-a-b".to_string(),
                source_id: "note-a".to_string(),
                target_id: "note-b".to_string(),
                predicate: "supports".to_string(),
                status: "confirmed".to_string(),
            },
            &timestamp,
        )
        .unwrap();

        let rows = query(
            &storage,
            "graph_edges",
            &["id", "predicate"],
            vec![eq("graph_edges", "project_id", json!("project-mobile"))],
            10,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(storage.stable_frontier(), base + 3);
        assert!(storage.node_view("note-a").is_some());
        assert!(storage.node_view("note-b").is_some());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn u9_clean_restore_preserves_fung_notes_and_graph_relations() {
        let source_parent = tempfile::tempdir().unwrap();
        let source_path = source_parent.path().join("source-genesis");
        let storage = Storage::open(OpenOptions {
            path: source_path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&storage).unwrap();

        let timestamp = "2026-08-14T00:00:00Z".to_string();
        for id in ["backup-note-a", "backup-note-b"] {
            commit_note(
                &storage,
                &crate::mobile::MobileNoteInput {
                    id: id.to_string(),
                    title: id.to_string(),
                    body: "FUNG U9 fixture".to_string(),
                    project_id: "backup-project".to_string(),
                    created_at: timestamp.clone(),
                    updated_at: timestamp.clone(),
                    evidence_label: Some("fixture".to_string()),
                },
                "projects/backup-project",
            )
            .unwrap();
        }
        commit_relation(
            &storage,
            "backup-project",
            &crate::mobile::GraphEdgeInput {
                id: "backup-edge-a-b".to_string(),
                source_id: "backup-note-a".to_string(),
                target_id: "backup-note-b".to_string(),
                predicate: "supports".to_string(),
                status: "confirmed".to_string(),
            },
            &timestamp,
        )
        .unwrap();
        let recording = start_capture(
            &storage,
            "backup-project",
            "projects/backup-project",
            "backup-recording",
            "projects/backup-project/backup-recording/manifest.json",
            &timestamp,
        )
        .unwrap();
        append_capture_chunk(
            &storage,
            &recording,
            AudioChunk {
                id: "backup-audio-chunk",
                file_path: "projects/backup-project/backup-recording/segment-000001.m4a",
                start_ms: 0,
                end_ms: 1_000,
                byte_size: 128,
                checksum: "fixture-sha256",
                timestamp: &timestamp,
            },
        )
        .unwrap();

        let backup_parent = tempfile::tempdir().unwrap();
        let bundle_path = backup_parent.path().join("fung-u9-fixture.genesis");
        let source_frontier = storage.stable_frontier();
        let bundle = storage
            .export_backup(BackupExportRequest {
                destination: bundle_path.clone(),
            })
            .unwrap();
        assert_eq!(bundle.stable_frontier, source_frontier);

        let restore_parent = tempfile::tempdir().unwrap();
        let restore_path = restore_parent.path().join("restore-clean");
        let restored_bundle = Storage::restore_backup(BackupRestoreRequest {
            bundle_path,
            target_root: restore_path.clone(),
        })
        .unwrap();
        assert_eq!(restored_bundle.sha256, bundle.sha256);

        let restored = Storage::open(OpenOptions {
            path: restore_path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        let edges = query(
            &restored,
            "graph_edges",
            &["id", "predicate"],
            vec![eq("graph_edges", "project_id", json!("backup-project"))],
            10,
        )
        .unwrap();
        let chunks = query(
            &restored,
            "audio_chunks",
            &["id", "file_path", "checksum"],
            vec![eq(
                "audio_chunks",
                "recording_id",
                json!("backup-recording"),
            )],
            10,
        )
        .unwrap();
        assert_eq!(restored.stable_frontier(), source_frontier);
        assert!(restored.node_view("backup-note-a").is_some());
        assert!(restored.node_view("backup-note-b").is_some());
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["graph_edges.id"], "backup-edge-a-b");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["audio_chunks.id"], "backup-audio-chunk");
        assert_eq!(
            chunks[0]["audio_chunks.file_path"],
            "projects/backup-project/backup-recording/segment-000001.m4a"
        );
    }

    #[test]
    fn marking_a_chunk_transcribed_keeps_the_rest_of_its_row() {
        // The stamp is written by re-upserting the whole row, so a field left
        // out is a field erased — and `file_path` and `checksum` are how the
        // audio is found and verified.
        let (_path, storage) = open();
        let timestamp = "2026-08-19T11:00:00Z";
        commit_rows(&storage, vec![
            upsert("projects", json!({"id": "p", "name": "P", "storage_path": "C:/p", "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
            upsert("recordings", json!({"id": "r", "project_id": "p", "source": "microphone", "input_path": null, "canonical_audio_path": "C:/p/r", "status": "completed", "duration_ms": 0, "created_at": timestamp, "updated_at": timestamp})),
            upsert("audio_chunks", json!({"id": "c", "recording_id": "r", "sequence_no": 3, "file_path": "C:/p/r/mic-00003.wav", "start_ms": 100, "end_ms": 2100, "byte_size": 42, "checksum": "deadbeef", "created_at": timestamp})),
        ]).unwrap();

        mark_chunk_transcribed(&storage, "c", "2026-08-19T12:00:00Z").unwrap();

        let row = query(
            &storage,
            "audio_chunks",
            &[
                "id",
                "recording_id",
                "sequence_no",
                "file_path",
                "start_ms",
                "end_ms",
                "byte_size",
                "checksum",
                "created_at",
                "transcribed_at",
            ],
            vec![eq("audio_chunks", "id", json!("c"))],
            1,
        )
        .unwrap()
        .into_iter()
        .next()
        .expect("chunk must still exist");
        assert_eq!(
            optional_string(&row, "audio_chunks.transcribed_at").as_deref(),
            Some("2026-08-19T12:00:00Z")
        );
        assert_eq!(row["audio_chunks.file_path"], "C:/p/r/mic-00003.wav");
        assert_eq!(row["audio_chunks.checksum"], "deadbeef");
        assert_eq!(row["audio_chunks.sequence_no"], 3);
        assert_eq!(row["audio_chunks.byte_size"], 42);
    }

    #[test]
    fn stamping_a_chunk_that_is_gone_is_not_an_error() {
        // The caller has already committed the transcript by this point.
        // Failing here would report a successful transcription as a failure
        // and hand the chunk back to be transcribed a second time.
        let (_path, storage) = open();
        assert!(mark_chunk_transcribed(&storage, "no-such-chunk", "2026-08-19T12:00:00Z").is_ok());
    }

    #[test]
    fn adopting_audio_into_an_imported_recording_keeps_its_provenance() {
        // `append_capture_chunk` used to hardcode source="microphone" and
        // input_path=null on every rewrite, so recovering orphaned audio into
        // an imported recording relabelled it a live capture and dropped the
        // file it came from — derived state overwriting evidence.
        let (_path, storage) = open();
        let timestamp = "2026-08-19T11:00:00Z";
        commit_rows(&storage, vec![
            upsert("projects", json!({"id": "p", "name": "P", "storage_path": "C:/p", "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
            upsert("recordings", json!({"id": "r", "project_id": "p", "source": "import", "input_path": "D:/incoming/meeting.m4a", "canonical_audio_path": "C:/p/r", "status": "recording", "duration_ms": 0, "created_at": timestamp, "updated_at": timestamp, "language": "th"})),
            upsert("mobile_recording_checkpoints", json!({"id": "r", "recording_id": "r", "safe_offset_ms": 0, "segment_count": 0, "last_checksum": null, "updated_at": timestamp})),
        ]).unwrap();

        let record = capture(&storage, "r").unwrap();
        let record = append_capture_chunk(
            &storage,
            &record,
            AudioChunk {
                id: "c",
                file_path: "C:/p/r/mic-00001.wav",
                start_ms: 0,
                end_ms: 1000,
                byte_size: 8,
                checksum: "aa",
                timestamp: "2026-08-19T12:00:00Z",
            },
        )
        .unwrap();
        let finished = finish_capture(&storage, &record, "2026-08-19T12:00:00Z").unwrap();

        assert_eq!(finished.source, "import");
        assert_eq!(
            finished.input_path.as_deref(),
            Some("D:/incoming/meeting.m4a")
        );
        assert_eq!(finished.language.as_deref(), Some("th"));
    }

    #[test]
    fn an_absent_or_empty_nullable_column_reads_as_unset() {
        // A recording written before the column existed and one written with
        // an empty string must both mean "no language chosen"; treating the
        // empty string as a language would pass "" to the whisper worker.
        let row = json!({"recordings.language": "", "recordings.source": "import"});
        assert_eq!(optional_string(&row, "recordings.language"), None);
        assert_eq!(optional_string(&row, "recordings.missing"), None);
        assert_eq!(
            optional_string(&row, "recordings.source").as_deref(),
            Some("import")
        );
    }

    #[test]
    fn the_schema_chain_advances_one_version_at_a_time() {
        // Genesis requires a fresh database to start at version 1 and move up
        // one step at a time, and `install` skips a step that reports a
        // version conflict. A new package that forgets to bump its version,
        // or one left out of the list, therefore fails silently on an
        // existing database and only shows up as a missing column much later.
        let chain = [
            schema_v1(),
            schema_v2(),
            schema_v3(),
            schema_v4(),
            schema_v5(),
            schema_v6(),
            schema_v7(),
            schema_v8(),
            schema_v9(),
            schema_v10(),
            schema_v11(),
            schema_v12(),
            schema(),
        ];
        for (index, package) in chain.iter().enumerate() {
            let expected = index as u32 + 1;
            assert_eq!(
                package.schema_version, expected,
                "step {index} must register as version {expected}"
            );
            assert_eq!(
                package.previous_version,
                if index == 0 { None } else { Some(expected - 1) },
                "step {index} must follow the one before it"
            );
        }
    }

    #[test]
    fn the_current_schema_carries_the_transcript_columns() {
        // Both are read by name at runtime; a package that dropped one would
        // compile and then fail every capture with a missing-column error.
        let package = schema();
        let column_exists = |table_name: &str, column: &str| {
            package
                .tables
                .iter()
                .find(|candidate| candidate.name == table_name)
                .is_some_and(|table| table.columns.iter().any(|c| c.name == column))
        };
        assert!(column_exists("recordings", "language"));
        assert!(column_exists("audio_chunks", "transcribed_at"));
    }

    #[test]
    fn install_is_idempotent_after_a_prior_schema_upgrade() {
        let path =
            std::env::temp_dir().join(format!("fung-genesis-upgrade-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();

        storage.register_relational_schema(schema_v1()).unwrap();
        storage.register_relational_schema(schema_v2()).unwrap();
        storage.register_relational_schema(schema_v3()).unwrap();
        storage.register_relational_schema(schema_v4()).unwrap();
        storage.register_relational_schema(schema_v5()).unwrap();
        storage.register_relational_schema(schema_v6()).unwrap();
        install(&storage).unwrap();

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn capture_checkpoint_is_durable_in_genesis_transactions() {
        let (path, storage) = open();
        // Delta, not absolute: `open()`'s schema registrations advance the
        // frontier too since WP-1.3. One durable transaction per step is the
        // invariant.
        let base = storage.stable_frontier();
        let started = start_capture(
            &storage,
            "project-mobile",
            "projects/project-mobile",
            "recording-1",
            "projects/project-mobile/recording-1/manifest.json",
            "2026-07-20T00:00:00Z",
        )
        .unwrap();
        assert_eq!(started.safe_offset_ms, 0);

        let appended = append_capture_chunk(
            &storage,
            &started,
            AudioChunk {
                id: "chunk-1",
                file_path: "projects/project-mobile/recording-1/segment-000001.m4a",
                start_ms: 0,
                end_ms: 2_000,
                byte_size: 128,
                checksum: "abc",
                timestamp: "2026-07-20T00:00:02Z",
            },
        )
        .unwrap();
        assert_eq!(appended.safe_offset_ms, 2_000);
        assert_eq!(appended.segment_count, 1);

        let finished = finish_capture(&storage, &appended, "2026-07-20T00:00:03Z").unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(storage.stable_frontier(), base + 3);
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn legacy_sqlite_is_read_once_into_signed_genesis_rows() {
        let (path, storage) = open();
        // Delta, not absolute — see capture_checkpoint test.
        let base = storage.stable_frontier();
        let legacy = path.join("fung.db");
        let connection = Connection::open(&legacy).unwrap();
        connection.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT NOT NULL,storage_path TEXT NOT NULL,active_recording_id TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); INSERT INTO projects VALUES('legacy-project','Legacy','projects/legacy',NULL,'t','t');").unwrap();
        drop(connection);
        assert_eq!(import_legacy_sqlite(&storage, &legacy).unwrap(), 1);
        let rows = query(
            &storage,
            "projects",
            &["id", "name"],
            vec![eq("projects", "id", json!("legacy-project"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["projects.name"], "Legacy");
        assert_eq!(storage.stable_frontier(), base + 1);
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v4_adds_external_tables_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        // v4 tables accept rows through the normal adapter path.
        commit_rows(&storage, vec![
            upsert("external_connections", json!({"id": "zoom", "provider": "zoom", "account_label": "user@example.com", "status": "connected", "created_at": "t", "updated_at": "t"})),
        ]).unwrap();
        let rows = query(
            &storage,
            "external_connections",
            &["id", "status"],
            vec![eq("external_connections", "id", json!("zoom"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["external_connections.status"], "connected");
        // Re-install after a stepped upgrade must stay idempotent (mirrors existing v1->v3 test).
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v5_adds_tts_test_results_table_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        commit_rows(&storage, vec![
            upsert("model_providers", json!({"id": "tts-1", "label": "F5-TTS", "runtime_location": "local", "kind": "tts", "enabled": true, "config_json": "{}", "created_at": "t", "updated_at": "t"})),
            upsert("tts_test_results", json!({"id": "test-1", "provider_id": "tts-1", "status": "ok", "latency_ms": 812, "sample_audio_path": "/tmp/sample.wav", "error_message": null, "tested_at": "t"})),
        ]).unwrap();
        let rows = query(
            &storage,
            "tts_test_results",
            &["id", "status", "latency_ms"],
            vec![eq("tts_test_results", "id", json!("test-1"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["tts_test_results.status"], "ok");
        assert_eq!(rows[0]["tts_test_results.latency_ms"], 812);
        // Re-install after a stepped upgrade must stay idempotent.
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v6_adds_paired_devices_public_key_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        commit_rows(&storage, vec![
            upsert("paired_devices", json!({"id": "peer-1", "name": "FUNG Desktop", "endpoint": "192.168.1.20:8765", "trust_state": "paired", "pairing_proof_hash": "sess-uuid-1", "capabilities_json": [], "created_at": "t", "updated_at": "t", "public_key": "cGVlci1wdWJsaWMta2V5"})),
        ]).unwrap();
        let rows = query(
            &storage,
            "paired_devices",
            &["id", "public_key"],
            vec![eq("paired_devices", "id", json!("peer-1"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["paired_devices.public_key"], "cGVlci1wdWJsaWMta2V5");
        // Rows written without a public_key (nullable) must still be readable.
        commit_rows(&storage, vec![
            upsert("paired_devices", json!({"id": "peer-2", "name": "FUNG Desktop 2", "endpoint": "192.168.1.21:8765", "trust_state": "paired", "pairing_proof_hash": "sess-uuid-2", "capabilities_json": [], "created_at": "t", "updated_at": "t", "public_key": null})),
        ]).unwrap();
        let rows = query(
            &storage,
            "paired_devices",
            &["id", "public_key"],
            vec![eq("paired_devices", "id", json!("peer-2"))],
            1,
        )
        .unwrap();
        assert!(rows[0]["paired_devices.public_key"].is_null());
        // Re-install after a stepped upgrade must stay idempotent.
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v7_adds_delegated_jobs_executor_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        commit_rows(&storage, vec![
            upsert("projects", json!({"id": "proj-1", "name": "Test Project", "storage_path": "test-path", "created_at": "t", "updated_at": "t"})),
            upsert("delegated_jobs", json!({
                "id": "job-1", "project_id": "proj-1", "executor_device_id": null,
                "operation": "transcript.transcribe", "state": "queued", "progress": 0,
                "input_manifest_hash": "abc123", "checkpoint_json": null,
                "observed_at": "t", "created_at": "t", "updated_at": "t", "executor": "cloud"
            })),
        ]).unwrap();
        let rows = query(
            &storage,
            "delegated_jobs",
            &["id", "executor"],
            vec![eq("delegated_jobs", "id", json!("job-1"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["delegated_jobs.executor"], "cloud");
        // Rows written without executor (nullable) must still be readable.
        commit_rows(
            &storage,
            vec![upsert(
                "delegated_jobs",
                json!({
                    "id": "job-2", "project_id": "proj-1", "executor_device_id": null,
                    "operation": "transcript.transcribe", "state": "queued", "progress": 0,
                    "input_manifest_hash": "def456", "checkpoint_json": null,
                    "observed_at": "t", "created_at": "t", "updated_at": "t", "executor": null
                }),
            )],
        )
        .unwrap();
        let rows = query(
            &storage,
            "delegated_jobs",
            &["id", "executor"],
            vec![eq("delegated_jobs", "id", json!("job-2"))],
            1,
        )
        .unwrap();
        assert!(rows[0]["delegated_jobs.executor"].is_null());
        // Re-install after a stepped upgrade must stay idempotent.
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v9_adds_controlled_external_tool_tables_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        commit_rows(&storage, vec![
            upsert("projects", json!({
                "id": "proj-mcp", "name": "MCP Test", "storage_path": "test-path",
                "active_recording_id": null, "created_at": "t", "updated_at": "t"
            })),
            upsert("recordings", json!({
                "id": "rec-mcp", "project_id": "proj-mcp", "source": "microphone",
                "input_path": null, "canonical_audio_path": "recordings/rec-mcp.wav",
                "status": "recording", "duration_ms": 0, "created_at": "t", "updated_at": "t"
            })),
            upsert("external_connections", json!({
                "id": "connector-mcp", "provider": "local-mcp", "account_label": "Knowledge Base",
                "status": "configured", "transport": "stdio", "endpoint": "C:/approved/mcp.exe",
                "credential_ref": "fung/connector-mcp", "capabilities_json": ["documents.search"],
                "created_at": "t", "updated_at": "t"
            })),
            upsert("meeting_tool_grants", json!({
                "id": "grant-mcp", "project_id": "proj-mcp", "recording_id": "rec-mcp",
                "connector_id": "connector-mcp", "capabilities_json": ["documents.search"],
                "granted_at": "t", "expires_at": "t+5m", "revoked_at": null
            })),
            upsert("external_tool_previews", json!({
                "id": "preview-mcp", "project_id": "proj-mcp", "recording_id": "rec-mcp",
                "connector_id": "connector-mcp", "tool_name": "search_documents",
                "capability": "documents.search", "arguments_hash": "sha256:req",
                "approved_fields_json": ["query"], "evidence_refs_json": ["segment-7"],
                "state": "previewed", "expires_at": "t+1m", "created_at": "t"
            })),
            upsert("external_tool_runs", json!({
                "id": "run-mcp", "preview_id": "preview-mcp", "project_id": "proj-mcp",
                "recording_id": "rec-mcp", "connector_id": "connector-mcp",
                "tool_name": "search_documents", "capability": "documents.search",
                "request_hash": "sha256:req", "output_hash": "sha256:out",
                "status": "completed", "started_at": "t", "finished_at": "t+1s",
                "error_code": null, "result_ref": "result-mcp"
            })),
            upsert("external_tool_results", json!({
                "id": "result-mcp", "run_id": "run-mcp", "mime_type": "application/json",
                "sanitized_payload_json": {"title": "Approved document"},
                "source_refs_json": ["kb://document/42"], "byte_size": 29, "created_at": "t+1s"
            })),
        ]).unwrap();

        let rows = query(
            &storage,
            "external_tool_results",
            &["id", "run_id", "byte_size"],
            vec![eq("external_tool_results", "id", json!("result-mcp"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["external_tool_results.run_id"], "run-mcp");
        assert_eq!(rows[0]["external_tool_results.byte_size"], 29);

        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_migrates_v10_additively_and_keeps_canonical_lane_aggregates() {
        let (path, storage) = open_at_v10();
        commit_rows(
            &storage,
            vec![upsert(
                "projects",
                json!({
                    "id": "legacy-n3-project",
                    "name": "legacy",
                    "storage_path": "C:/legacy",
                    "active_recording_id": null,
                    "created_at": "t",
                    "updated_at": "t"
                }),
            )],
        )
        .unwrap();

        install(&storage).unwrap();
        let legacy = query(
            &storage,
            "projects",
            &["id", "name"],
            vec![eq("projects", "id", json!("legacy-n3-project"))],
            1,
        )
        .unwrap();
        assert_eq!(legacy[0]["projects.name"], "legacy");
        assert_eq!(schema_v10().schema_version, 10);
        assert_eq!(schema_v11().schema_version, 11);
        assert_eq!(schema_v12().schema_version, 12);
        assert_eq!(schema().schema_version, 13);
        let old_runs = schema_v12()
            .tables
            .into_iter()
            .find(|table| table.name == "meeting_agent_runs")
            .unwrap();
        assert!(!old_runs
            .columns
            .iter()
            .any(|column| column.name == "model_run_id"));
        let current_runs = schema()
            .tables
            .into_iter()
            .find(|table| table.name == "meeting_agent_runs")
            .unwrap();
        assert!(current_runs
            .columns
            .iter()
            .any(|column| column.name == "model_run_id" && column.nullable));
        for table_name in [
            "knowledge_collections",
            "knowledge_documents",
            "knowledge_document_versions",
            "knowledge_chunks",
            "knowledge_metric_observations",
            "knowledge_index_runs",
            "knowledge_evidence_bundles",
            "meeting_agent_grants",
            "meeting_agent_runs",
            "meeting_delivery_outbox",
            "meeting_delivery_receipts",
            "meeting_participant_sessions",
            "meeting_participant_evidence",
            "participant_profiles",
            "speaker_identity_links",
        ] {
            assert!(
                schema_v11()
                    .tables
                    .iter()
                    .any(|table| table.name == table_name),
                "v11 must freeze {table_name}"
            );
        }

        drop(storage);
        let reopened = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&reopened).unwrap();
        let preserved = query(
            &reopened,
            "projects",
            &["id"],
            vec![eq("projects", "id", json!("legacy-n3-project"))],
            1,
        )
        .unwrap();
        assert_eq!(preserved.len(), 1);
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn model_proposal_link_and_duplicate_request_survive_v13_migration() {
        let (path, storage) = open_at_v10();
        install(&storage).unwrap();
        let (scope, _) = seed_meeting(&storage, &path);
        let timestamp = "2026-09-25T10:00:00Z";
        assert!(ensure_meeting_agent_request_unused(&storage, "model-ask-a").is_ok());
        commit_rows(&storage, vec![
            upsert("model_providers", json!({
                "id": "ollama-summary-intent", "label": "Ollama", "runtime_location": "local",
                "kind": "summary_intent", "enabled": true, "config_json": {"endpoint":"http://127.0.0.1:11434"},
                "created_at": timestamp, "updated_at": timestamp
            })),
            upsert("model_runs", json!({
                "id": "agent-model-a", "recording_id": scope.recording_id,
                "provider_id": "ollama-summary-intent", "model_name": "llama3.1:8b",
                "task_kind": "meeting_agent_proposal", "runtime_location": "local",
                "input_ref": "sha256:input", "output_ref": "sha256:output",
                "parameters_json": {"maxOutputTokens": 768}, "created_at": timestamp
            })),
            upsert("meeting_agent_runs", json!({
                "id": "agent-run-model-ask-a", "project_id": scope.project_id,
                "recording_id": scope.recording_id, "meeting_session_id": scope.meeting_session_id,
                "grant_id": null, "model_run_id": "agent-model-a", "trigger_id": "model-ask-a",
                "transcript_cursor": 0, "transcript_revision_set_json": [], "evidence_ids_json": [],
                "policy_version": "meeting-local/1", "state": "draft_created",
                "contract_version": meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp
            })),
        ]).unwrap();
        let rows = query(
            &storage,
            "meeting_agent_runs",
            &["model_run_id"],
            vec![eq("meeting_agent_runs", "trigger_id", json!("model-ask-a"))],
            1,
        )
        .unwrap();
        assert_eq!(rows[0]["meeting_agent_runs.model_run_id"], "agent-model-a");
        assert_eq!(
            ensure_meeting_agent_request_unused(&storage, "model-ask-a").unwrap_err(),
            "MEETING_AGENT_REQUEST_DUPLICATE"
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn model_citations_must_be_drawn_from_all_supplied_evidence() {
        let model = MeetingAgentModelProvenance {
            id: "run-a".into(),
            model_name: "llama3.1:8b".into(),
            input_hash: "input".into(),
            output_hash: "output".into(),
            provider_config_hash: "config".into(),
        };
        let all = json!([{"id":"e0"}, {"id":"e1"}]);
        assert!(
            validate_model_reference_set(Some(&all), Some(&model), &json!([{"id":"e1"}])).is_ok()
        );
        assert_eq!(
            validate_model_reference_set(Some(&all), Some(&model), &json!([{"id":"e2"}]))
                .unwrap_err(),
            "MEETING_AGENT_MODEL_REFS_INVALID"
        );
        assert_eq!(
            validate_model_reference_set(None, Some(&model), &json!([{"id":"e1"}])).unwrap_err(),
            "MEETING_AGENT_MODEL_INPUT_INVALID"
        );
    }

    #[test]
    fn n3_atomic_commit_covers_custody_projection_event_and_cursor_after_reopen() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, root.path());
        let audio = std::fs::read(&audio_path).unwrap();
        let request = meeting_request(
            &scope,
            "event-atomic-0",
            audio_source(
                &scope,
                "coverage-0",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&audio)),
            ),
            0,
            0,
            "revision-0",
            "utterance-0",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "same words",
            "unreviewed",
        );
        let attempt =
            begin_meeting_commit(&storage, "n3-tx-atomic-0", "2026-09-21T10:02:00Z").unwrap();
        let committed = commit_meeting_transcript(&storage, &attempt, &request).unwrap();
        assert!(!committed.idempotent);
        assert_eq!(
            committed.commit_sequence,
            Some(committed.commit_sequence.unwrap())
        );

        for (table, id, column) in [
            ("meeting_source_coverage", "coverage-0", "id"),
            ("transcript_revisions", "revision-0", "id"),
            ("transcript_projection", "utterance-0", "id"),
            ("transcript_event_log", "event-atomic-0", "id"),
            ("meeting_source_cursors", "n3-source::mic-1::1", "id"),
        ] {
            assert_eq!(
                query(
                    &storage,
                    table,
                    &[column],
                    vec![eq(table, column, json!(id))],
                    1,
                )
                .unwrap()
                .len(),
                1,
                "atomic commit must write {table}"
            );
        }
        let revision = query(
            &storage,
            "transcript_revisions",
            &["confidence", "effective_text", "origin"],
            vec![eq("transcript_revisions", "id", json!("revision-0"))],
            1,
        )
        .unwrap();
        assert!(revision[0]["transcript_revisions.confidence"].is_null());
        assert_eq!(revision[0]["transcript_revisions.origin"], "local_asr");

        drop(storage);
        let reopened = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&reopened).unwrap();
        assert_eq!(
            query(
                &reopened,
                "transcript_event_log",
                &["transaction_id"],
                vec![eq("transcript_event_log", "id", json!("event-atomic-0"))],
                1,
            )
            .unwrap()
            .len(),
            1
        );
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_event_identity_replay_is_idempotent_and_changed_payload_conflicts() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, root.path());
        let audio = std::fs::read(&audio_path).unwrap();
        let request = meeting_request(
            &scope,
            "event-idempotent",
            audio_source(
                &scope,
                "coverage-idempotent",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&audio)),
            ),
            0,
            0,
            "revision-idempotent",
            "utterance-idempotent",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "repeatable text",
            "unreviewed",
        );
        let attempt =
            begin_meeting_commit(&storage, "n3-tx-idempotent", "2026-09-21T10:03:00Z").unwrap();
        commit_meeting_transcript(&storage, &attempt, &request).unwrap();
        let replay = commit_meeting_transcript(&storage, &attempt, &request).unwrap();
        assert!(replay.idempotent);
        assert_eq!(replay.transaction_id, attempt.transaction_id);
        let alternate_attempt = begin_meeting_commit(
            &storage,
            "n3-tx-idempotent-alternate",
            "2026-09-21T10:03:30Z",
        )
        .unwrap();
        let alternate_replay =
            commit_meeting_transcript(&storage, &alternate_attempt, &request).unwrap();
        assert!(alternate_replay.idempotent);
        assert_eq!(alternate_replay.transaction_id, "n3-tx-idempotent");

        let mut changed = request.clone();
        changed.revision.effective_text = "changed payload".to_string();
        let changed_attempt =
            begin_meeting_commit(&storage, "n3-tx-idempotent-changed", "2026-09-21T10:04:00Z")
                .unwrap();
        let error = commit_meeting_transcript(&storage, &changed_attempt, &changed).unwrap_err();
        assert!(error.contains("EVENT_ID_CONFLICT"), "{error}");
        assert_eq!(
            query(
                &storage,
                "transcript_revisions",
                &["id"],
                vec![eq(
                    "transcript_revisions",
                    "meeting_session_id",
                    json!(scope.meeting_session_id)
                )],
                10,
            )
            .unwrap()
            .len(),
            1
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_repeated_text_with_distinct_utterance_ids_is_preserved() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, first_path) = seed_meeting(&storage, root.path());
        let first_bytes = std::fs::read(&first_path).unwrap();
        let first = meeting_request(
            &scope,
            "event-repeat-0",
            audio_source(
                &scope,
                "coverage-repeat-0",
                "n3-chunk-0",
                &first_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&first_bytes)),
            ),
            0,
            0,
            "revision-repeat-0",
            "utterance-repeat-0",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "same words",
            "unreviewed",
        );
        let first_attempt =
            begin_meeting_commit(&storage, "n3-tx-repeat-0", "2026-09-21T10:05:00Z").unwrap();
        commit_meeting_transcript(&storage, &first_attempt, &first).unwrap();

        let second_path = add_audio_chunk(
            &storage,
            root.path(),
            "n3-chunk-1",
            1,
            1000,
            2000,
            b"n3-finalized-audio-1",
        );
        let second_bytes = std::fs::read(&second_path).unwrap();
        let second = meeting_request(
            &scope,
            "event-repeat-1",
            audio_source(
                &scope,
                "coverage-repeat-1",
                "n3-chunk-1",
                &second_path,
                1,
                1000,
                2000,
                Some(sha256_bytes(&second_bytes)),
            ),
            1,
            1,
            "revision-repeat-1",
            "utterance-repeat-1",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "same words",
            "unreviewed",
        );
        let second_attempt =
            begin_meeting_commit(&storage, "n3-tx-repeat-1", "2026-09-21T10:06:00Z").unwrap();
        commit_meeting_transcript(&storage, &second_attempt, &second).unwrap();
        let revisions = query_all(
            &storage,
            "transcript_revisions",
            &["id", "utterance_id", "effective_text"],
            vec![eq(
                "transcript_revisions",
                "meeting_session_id",
                json!(scope.meeting_session_id),
            )],
        )
        .unwrap();
        assert_eq!(revisions.len(), 2);
        assert_ne!(
            revisions[0]["transcript_revisions.utterance_id"],
            revisions[1]["transcript_revisions.utterance_id"]
        );
        assert_eq!(
            revisions[0]["transcript_revisions.effective_text"],
            revisions[1]["transcript_revisions.effective_text"]
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_rejects_bad_custody_gaps_and_stale_frontier_without_partial_effects() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, root.path());
        let audio = std::fs::read(&audio_path).unwrap();

        let bad_custody = meeting_request(
            &scope,
            "event-bad-custody",
            audio_source(
                &scope,
                "coverage-bad-custody",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some("0".repeat(64)),
            ),
            0,
            0,
            "revision-bad-custody",
            "utterance-bad-custody",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "not committed",
            "unreviewed",
        );
        let bad_attempt =
            begin_meeting_commit(&storage, "n3-tx-bad-custody", "2026-09-21T10:07:00Z").unwrap();
        let bad_error =
            commit_meeting_transcript(&storage, &bad_attempt, &bad_custody).unwrap_err();
        assert!(
            bad_error.contains("caller audio custody assertion"),
            "{bad_error}"
        );
        assert!(query(
            &storage,
            "transcript_event_log",
            &["id"],
            vec![eq("transcript_event_log", "id", json!("event-bad-custody"))],
            1
        )
        .unwrap()
        .is_empty());

        let gap = SourceCoverageInput {
            id: "coverage-gap".to_string(),
            source_session_id: scope.source_session_id.clone(),
            track_id: scope.track_id.clone(),
            source_generation: scope.source_generation,
            sequence_no: 0,
            start_ms: 0,
            end_ms: 1000,
            kind: SourceCoverageKind::Gap,
            audio_chunk_id: None,
            file_path: None,
            byte_size: None,
            checksum: None,
            gap_reason: Some("network_loss".to_string()),
        };
        let gap_request = meeting_request(
            &scope,
            "event-gap",
            gap,
            0,
            0,
            "revision-gap",
            "utterance-gap",
            1,
            TranscriptOrigin::ProviderAsr,
            None,
            None,
            "gap is not silence",
            "unreviewed",
        );
        let gap_attempt =
            begin_meeting_commit(&storage, "n3-tx-gap", "2026-09-21T10:08:00Z").unwrap();
        let gap_error =
            commit_meeting_transcript(&storage, &gap_attempt, &gap_request).unwrap_err();
        assert!(
            gap_error.contains("not covered by finalized audio"),
            "{gap_error}"
        );
        assert!(query(
            &storage,
            "transcript_event_log",
            &["id"],
            vec![eq("transcript_event_log", "id", json!("event-gap"))],
            1
        )
        .unwrap()
        .is_empty());

        let valid = meeting_request(
            &scope,
            "event-stale-frontier",
            audio_source(
                &scope,
                "coverage-stale-frontier",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&audio)),
            ),
            0,
            0,
            "revision-stale-frontier",
            "utterance-stale-frontier",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "stale frontier",
            "unreviewed",
        );
        let stale_attempt =
            begin_meeting_commit(&storage, "n3-tx-stale-frontier", "2026-09-21T10:09:00Z").unwrap();
        commit_rows(
            &storage,
            vec![upsert(
                "projects",
                json!({
                    "id": scope.project_id,
                    "name": "N3 meeting updated",
                    "storage_path": root.path().display().to_string(),
                    "active_recording_id": null,
                    "created_at": "2026-09-21T10:00:00Z",
                    "updated_at": "2026-09-21T10:09:01Z"
                }),
            )],
        )
        .unwrap();
        let stale_error = commit_meeting_transcript(&storage, &stale_attempt, &valid).unwrap_err();
        assert!(
            stale_error.contains("MEETING_COMMIT_REJECTED"),
            "{stale_error}"
        );
        assert!(query(
            &storage,
            "transcript_event_log",
            &["id"],
            vec![eq(
                "transcript_event_log",
                "id",
                json!("event-stale-frontier")
            )],
            1
        )
        .unwrap()
        .is_empty());
        assert!(query(
            &storage,
            "meeting_source_coverage",
            &["id"],
            vec![eq(
                "meeting_source_coverage",
                "id",
                json!("coverage-stale-frontier")
            )],
            1
        )
        .unwrap()
        .is_empty());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_manual_correction_blocks_late_asr_after_expected_revision() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, first_path) = seed_meeting(&storage, root.path());
        let first_bytes = std::fs::read(&first_path).unwrap();
        let first = meeting_request(
            &scope,
            "event-review-0",
            audio_source(
                &scope,
                "coverage-review-0",
                "n3-chunk-0",
                &first_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&first_bytes)),
            ),
            0,
            0,
            "revision-review-0",
            "utterance-review",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "draft text",
            "unreviewed",
        );
        let first_attempt =
            begin_meeting_commit(&storage, "n3-tx-review-0", "2026-09-21T10:10:00Z").unwrap();
        commit_meeting_transcript(&storage, &first_attempt, &first).unwrap();

        let manual_path = add_audio_chunk(
            &storage,
            root.path(),
            "n3-chunk-review-1",
            1,
            1000,
            2000,
            b"n3-review-audio-1",
        );
        let manual_bytes = std::fs::read(&manual_path).unwrap();
        let manual = meeting_request(
            &scope,
            "event-review-1",
            audio_source(
                &scope,
                "coverage-review-1",
                "n3-chunk-review-1",
                &manual_path,
                1,
                1000,
                2000,
                Some(sha256_bytes(&manual_bytes)),
            ),
            1,
            1,
            "revision-review-1",
            "utterance-review",
            2,
            TranscriptOrigin::Human,
            Some(1),
            Some(1),
            "reviewed text",
            "reviewed",
        );
        let manual_attempt =
            begin_meeting_commit(&storage, "n3-tx-review-1", "2026-09-21T10:11:00Z").unwrap();
        commit_meeting_transcript(&storage, &manual_attempt, &manual).unwrap();

        let late_path = add_audio_chunk(
            &storage,
            root.path(),
            "n3-chunk-review-2",
            2,
            2000,
            3000,
            b"n3-review-audio-2",
        );
        let late_bytes = std::fs::read(&late_path).unwrap();
        let late = meeting_request(
            &scope,
            "event-review-late",
            audio_source(
                &scope,
                "coverage-review-late",
                "n3-chunk-review-2",
                &late_path,
                2,
                2000,
                3000,
                Some(sha256_bytes(&late_bytes)),
            ),
            2,
            2,
            "revision-review-late",
            "utterance-review",
            3,
            TranscriptOrigin::LocalAsr,
            Some(2),
            Some(2),
            "late worker text",
            "unreviewed",
        );
        let late_attempt =
            begin_meeting_commit(&storage, "n3-tx-review-late", "2026-09-21T10:12:00Z").unwrap();
        let late_error = commit_meeting_transcript(&storage, &late_attempt, &late).unwrap_err();
        assert!(late_error.contains("late ASR"), "{late_error}");
        let projection = query(
            &storage,
            "transcript_projection",
            &["revision", "effective_text", "review_state"],
            vec![eq("transcript_projection", "id", json!("utterance-review"))],
            1,
        )
        .unwrap();
        assert_eq!(projection[0]["transcript_projection.revision"], 2);
        assert_eq!(
            projection[0]["transcript_projection.effective_text"],
            "reviewed text"
        );
        assert_eq!(
            projection[0]["transcript_projection.review_state"],
            "reviewed"
        );
        assert!(query(
            &storage,
            "transcript_event_log",
            &["id"],
            vec![eq("transcript_event_log", "id", json!("event-review-late"))],
            1
        )
        .unwrap()
        .is_empty());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn n3_ke_sharing_and_si_private_relationships_remain_fail_closed() {
        let denied_share = KnowledgeEvidenceInput {
            id: "evidence-1".to_string(),
            collection_id: "collection-1".to_string(),
            document_id: "document-1".to_string(),
            document_version_id: "version-1".to_string(),
            source_version: "v1".to_string(),
            evidence_bundle_id: None,
            citation: json!({"page": 1}),
            read_grant_id: None,
            share_grant_id: Some("share-1".to_string()),
            read_state: "denied".to_string(),
            share_state: "granted".to_string(),
            audience_policy_revision: Some(1),
        };
        assert!(denied_share.validate().is_err());
        let sensitive = KnowledgeEvidenceInput {
            citation: json!({"secret_token": "must-not-persist"}),
            read_state: "granted".to_string(),
            read_grant_id: Some("read-1".to_string()),
            share_state: "denied".to_string(),
            share_grant_id: None,
            ..denied_share.clone()
        };
        assert!(sensitive.validate().is_err());

        let invalid_private = PrivateIdentityReference {
            encrypted_blob_ref: "https://provider.example/private".to_string(),
            ciphertext_sha256: "a".repeat(64),
            envelope: IdentityEnvelope {
                version: 1,
                key_ref: format!("people_metadata:{}", "a".repeat(32)),
                nonce: vec![0; 24],
                aad_sha256: "b".repeat(64),
                ciphertext: vec![0; 16],
            },
        };
        assert!(validate_private_identity_reference(&invalid_private).is_err());
        let key_ref = format!("people_metadata:{}", "c".repeat(32));
        let mut backend = InMemoryIdentityKeyBackend::default();
        backend.insert(&key_ref, vec![7; 32]);
        let context = IdentityAadContext {
            account_ref: Some("native-account".to_string()),
            scope: "p::r::m::s::t::1".to_string(),
            vault_id: "vault-native".to_string(),
            entity_id: "identity-link:11111111111111111111111111111111".to_string(),
            revision: 1,
            model_context: "model-local".to_string(),
        };
        let payload = PersonIdentityPayload {
            link_id: context.entity_id.clone(),
            profile_id: "profile:22222222222222222222222222222222".to_string(),
            person_id: "private-person-canary".to_string(),
            display_name: "private-label-canary".to_string(),
            account_ref: context.account_ref.clone(),
            vault_id: context.vault_id.clone(),
            relationship_revision: 1,
            profile_revision: 1,
        };
        let valid_private = seal_person_identity(&payload, &context, &key_ref, &backend).unwrap();
        assert!(validate_private_identity_reference(&valid_private).is_ok());
        let opened =
            open_person_identity(&valid_private, &context, &payload.link_id, 1, &backend).unwrap();
        assert_eq!(opened.person_id, "private-person-canary");
        assert_eq!(opened.display_name, "private-label-canary");
        let identity = ParticipantAttribution {
            kind: "confirmed_person".to_string(),
            participant_session_id: Some("participant-session-1".to_string()),
            speaker_cluster_id: Some("speaker-1".to_string()),
            label_snapshot_ref: Some("label-1".to_string()),
            provider_ref_ciphertext: None,
            identity_link_id: Some(payload.link_id),
            identity_expected_revision: Some(1),
            person_ref_ciphertext: Some(valid_private),
            evidence_revision: 2,
        };
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn n3_reopens_and_replays_durable_transaction_identity() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, root.path());
        let audio = std::fs::read(&audio_path).unwrap();
        let request = meeting_request(
            &scope,
            "event-reopen-replay",
            audio_source(
                &scope,
                "coverage-reopen-replay",
                "n3-chunk-0",
                &audio_path,
                0,
                0,
                1000,
                Some(sha256_bytes(&audio)),
            ),
            0,
            0,
            "revision-reopen-replay",
            "utterance-reopen-replay",
            1,
            TranscriptOrigin::LocalAsr,
            None,
            None,
            "reopen exact attempt",
            "unreviewed",
        );
        let attempt =
            begin_meeting_commit(&storage, "n3-tx-reopen-replay", "2026-09-21T10:13:00Z").unwrap();
        commit_meeting_transcript(&storage, &attempt, &request).unwrap();
        drop(storage);

        let reopened = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        install(&reopened).unwrap();
        let replay = commit_meeting_transcript(&reopened, &attempt, &request).unwrap();
        assert!(replay.idempotent);
        assert_eq!(replay.transaction_id, "n3-tx-reopen-replay");
        let alternate_attempt = begin_meeting_commit(
            &reopened,
            "n3-tx-reopen-replay-alternate",
            "2026-09-21T10:13:30Z",
        )
        .unwrap();
        let alternate_replay =
            commit_meeting_transcript(&reopened, &alternate_attempt, &request).unwrap();
        assert!(alternate_replay.idempotent);
        assert_eq!(alternate_replay.transaction_id, "n3-tx-reopen-replay");
        drop(reopened);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn v12_batch_is_atomic_multi_source_ordered_and_revision_only_corrections_replay() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, audio_path) = seed_meeting(&storage, root.path());
        let audio = std::fs::read(&audio_path).unwrap();
        let source = audio_source(
            &scope,
            "coverage-v12-mic-0",
            "n3-chunk-0",
            &audio_path,
            0,
            0,
            1000,
            Some(sha256_bytes(&audio)),
        );
        let first = TranscriptRevisionInput {
            id: "revision-v12-one".to_string(),
            utterance_id: "utterance-v12-one".to_string(),
            revision: 1,
            supersedes_revision: None,
            expected_revision: None,
            origin: TranscriptOrigin::LocalAsr,
            raw_text: "สวัสดี".to_string(),
            effective_text: "สวัสดี".to_string(),
            language: Some("th-TH".to_string()),
            confidence: Some(0.91),
            start_ms: 100,
            end_ms: 400,
            model_run_id: None,
            review_state: "unreviewed".to_string(),
        };
        let second = TranscriptRevisionInput {
            id: "revision-v12-two".to_string(),
            utterance_id: "utterance-v12-two".to_string(),
            revision: 1,
            supersedes_revision: None,
            expected_revision: None,
            origin: TranscriptOrigin::LocalAsr,
            raw_text: "เริ่มประชุม".to_string(),
            effective_text: "เริ่มประชุม".to_string(),
            language: Some("th-TH".to_string()),
            confidence: Some(0.84),
            start_ms: 500,
            end_ms: 900,
            model_run_id: None,
            review_state: "unreviewed".to_string(),
        };
        let first_batch = ingest_batch_request(
            &scope,
            "operation-v12-mic-0",
            source,
            vec![first.clone(), second],
        );
        let attempt =
            begin_meeting_commit(&storage, "n3-tx-v12-mic-0", "2026-09-24T10:00:00Z").unwrap();
        let committed = commit_meeting_ingest_batch(&storage, &attempt, &first_batch).unwrap();
        assert_eq!((committed.first_cursor, committed.last_cursor), (0, 1));
        assert_eq!(committed.revision_ids.len(), 2);
        let replayed_batch = commit_meeting_ingest_batch(&storage, &attempt, &first_batch).unwrap();
        assert!(replayed_batch.idempotent);
        assert_eq!(replayed_batch.event_ids, committed.event_ids);

        let legacy_projection = query(
            &storage,
            "transcript_segments",
            &["text"],
            vec![eq("transcript_segments", "id", json!("utterance-v12-one"))],
            1,
        )
        .unwrap();
        assert_eq!(legacy_projection[0]["transcript_segments.text"], "สวัสดี");
        let chunk = query(
            &storage,
            "audio_chunks",
            &["transcribed_at"],
            vec![eq("audio_chunks", "id", json!("n3-chunk-0"))],
            1,
        )
        .unwrap();
        assert!(optional_string(&chunk[0], "audio_chunks.transcribed_at").is_some());

        let quiet_path = add_audio_chunk(
            &storage,
            root.path(),
            "n3-chunk-v12-quiet",
            2,
            1000,
            2000,
            b"n3-finalized-silence-fixture",
        );
        let quiet_source = audio_source(
            &scope,
            "coverage-v12-mic-1",
            "n3-chunk-v12-quiet",
            &quiet_path,
            1,
            1000,
            2000,
            None,
        );
        let quiet_batch = ingest_batch_request(&scope, "operation-v12-mic-1", quiet_source, vec![]);
        let quiet_attempt =
            begin_meeting_commit(&storage, "n3-tx-v12-mic-1", "2026-09-24T10:00:01Z").unwrap();
        let quiet = commit_meeting_ingest_batch(&storage, &quiet_attempt, &quiet_batch).unwrap();
        assert_eq!((quiet.first_cursor, quiet.last_cursor), (2, 2));
        assert!(query(
            &storage,
            "meeting_control_events",
            &["event_type"],
            vec![eq(
                "meeting_control_events",
                "id",
                json!(quiet.event_ids[0])
            )],
            1,
        )
        .unwrap()
        .iter()
        .any(|row| row["meeting_control_events.event_type"] == "audio_processed_no_utterance"));

        let system_session = "n3-source-system";
        commit_rows(
            &storage,
            vec![upsert(
                "meeting_source_sessions",
                json!({
                    "id": system_session,
                    "project_id": scope.project_id,
                    "recording_id": scope.recording_id,
                    "meeting_session_id": scope.meeting_session_id,
                    "source_kind": "system_audio",
                    "source_generation": 2,
                    "state": "open",
                    "contract_version": 1,
                    "created_at": "2026-09-24T10:00:02Z",
                    "ended_at": null
                }),
            )],
        )
        .unwrap();
        let mut system_scope = scope.clone();
        system_scope.source_session_id = system_session.to_string();
        system_scope.track_id = "system-1".to_string();
        system_scope.source_generation = 2;
        let system_path = add_audio_chunk(
            &storage,
            root.path(),
            "n3-chunk-v12-system",
            3,
            0,
            1000,
            b"n3-finalized-system-fixture",
        );
        let system_source = audio_source(
            &system_scope,
            "coverage-v12-system-0",
            "n3-chunk-v12-system",
            &system_path,
            0,
            0,
            1000,
            None,
        );
        let system_batch = ingest_batch_request(
            &system_scope,
            "operation-v12-system-0",
            system_source,
            vec![],
        );
        let system_attempt =
            begin_meeting_commit(&storage, "n3-tx-v12-system-0", "2026-09-24T10:00:03Z").unwrap();
        let system = commit_meeting_ingest_batch(&storage, &system_attempt, &system_batch).unwrap();
        assert_eq!((system.first_cursor, system.last_cursor), (3, 3));

        let correction = TranscriptRevisionInput {
            id: "revision-v12-one-human".to_string(),
            utterance_id: first.utterance_id.clone(),
            revision: 2,
            supersedes_revision: Some(1),
            expected_revision: Some(1),
            origin: TranscriptOrigin::Human,
            raw_text: first.raw_text.clone(),
            effective_text: "สวัสดีครับ".to_string(),
            language: first.language.clone(),
            confidence: first.confidence,
            start_ms: first.start_ms,
            end_ms: first.end_ms,
            model_run_id: first.model_run_id.clone(),
            review_state: "reviewed".to_string(),
        };
        let correction_attempt =
            begin_meeting_commit(&storage, "n3-tx-v12-correction", "2026-09-24T10:00:04Z").unwrap();
        let corrected = revise_meeting_transcript(
            &storage,
            &correction_attempt,
            &MeetingRevisionRequest {
                scope: scope.clone(),
                revision: correction,
            },
        )
        .unwrap();
        assert_eq!(corrected.cursor, 4);
        let projection = query(
            &storage,
            "transcript_segments",
            &["text"],
            vec![eq("transcript_segments", "id", json!("utterance-v12-one"))],
            1,
        )
        .unwrap();
        assert_eq!(projection[0]["transcript_segments.text"], "สวัสดีครับ");

        let snapshot =
            meeting_transcript_snapshot(&storage, &scope.project_id, &scope.recording_id).unwrap();
        assert_eq!(snapshot.high_watermark, 4);
        assert_eq!(snapshot.utterances.len(), 2);
        assert_eq!(snapshot.utterances[0]["effective_text"], "สวัสดีครับ");
        require_current_meeting_transcript_cursor(
            &storage,
            &scope.project_id,
            &scope.recording_id,
            snapshot.high_watermark,
        )
        .unwrap();
        assert_eq!(
            require_current_meeting_transcript_cursor(
                &storage,
                &scope.project_id,
                &scope.recording_id,
                snapshot.high_watermark - 1,
            )
            .unwrap_err(),
            "MEETING_TRANSCRIPT_CURSOR_STALE"
        );
        assert_eq!(
            meeting_transcript_snapshot(&storage, "another-project", &scope.recording_id)
                .unwrap_err(),
            "MEETING_RECORDING_SCOPE_DENIED"
        );
        let first_page = replay_meeting_events(
            &storage,
            &scope.project_id,
            &scope.recording_id,
            &MeetingReplayCursor {
                recording_id: scope.recording_id.clone(),
                after_cursor: -1,
            },
            2,
        )
        .unwrap();
        assert!(first_page.has_more);
        assert_eq!(first_page.next_cursor, 1);
        assert_eq!(
            replay_meeting_events(
                &storage,
                "another-project",
                &scope.recording_id,
                &MeetingReplayCursor {
                    recording_id: scope.recording_id.clone(),
                    after_cursor: -1,
                },
                10,
            )
            .unwrap_err(),
            "MEETING_RECORDING_SCOPE_DENIED"
        );
        let foreign_cursor = MeetingReplayCursor {
            recording_id: "another-recording".to_string(),
            after_cursor: 1,
        };
        assert_eq!(
            replay_meeting_events(
                &storage,
                &scope.project_id,
                &scope.recording_id,
                &foreign_cursor,
                10,
            )
            .unwrap_err(),
            "CURSOR_RECORDING_MISMATCH"
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn meeting_agent_draft_rejects_evidence_after_acl_revision_changes() {
        let (path, storage) = open();
        let root = tempfile::tempdir().unwrap();
        let (scope, _) = seed_meeting(&storage, root.path());
        let vault_id = "evidence-vault";
        let collection_id = "evidence-collection";
        let document_id = "evidence-document";
        let version_id = "evidence-version";
        let asset_id = "evidence-asset";
        let timestamp = "2026-09-25T00:00:00Z";
        let owner_scope = local_owner_collection_scope(vault_id);
        let locator_data = json!({
            "pageIndex": null,
            "startChar": 0,
            "endChar": 16,
            "startLine": 1,
            "endLine": 1,
        });
        let content_hash = sha256_bytes(b"meeting evidence");
        commit_rows(
            &storage,
            vec![
                upsert(
                    "identity_vaults",
                    json!({
                        "id": vault_id,
                        "owner_principal_ref": "principal:device:test",
                        "bound_account_ref": null,
                        "self_person_ref_ciphertext_ref": null,
                        "self_person_ref_ciphertext_sha256": null,
                        "self_person_ref_ciphertext_json": null,
                        "state": "active",
                        "key_store_namespace": "FUNG",
                        "contract_version": 1,
                        "created_at": timestamp,
                        "updated_at": timestamp,
                    }),
                ),
                upsert(
                    "knowledge_collections",
                    json!({
                        "id": collection_id,
                        "project_id": scope.project_id,
                        "owner_scope": owner_scope,
                        "classification": "internal",
                        "read_policy_ref": "local-read:test",
                        "share_policy_ref": "local-share-deny:test",
                        "revision": 1,
                        "status": "active",
                        "contract_version": 1,
                        "created_at": timestamp,
                        "updated_at": timestamp,
                    }),
                ),
                upsert(
                    "knowledge_documents",
                    json!({
                        "id": document_id,
                        "collection_id": collection_id,
                        "source_kind": "text",
                        "source_ref": asset_id,
                        "title_ref": "evidence-title",
                        "current_version_id": version_id,
                        "acl_revision": 1,
                        "status": "active",
                        "contract_version": 1,
                        "created_at": timestamp,
                        "updated_at": timestamp,
                    }),
                ),
                upsert(
                    "knowledge_document_versions",
                    json!({
                        "id": version_id,
                        "document_id": document_id,
                        "version_no": 1,
                        "version_label": "v1",
                        "content_hash": content_hash,
                        "custody_ref": "source/evidence.txt",
                        "mime_type": "text/plain",
                        "source_modified_at": null,
                        "ingested_at": timestamp,
                        "parser_version": "fung-knowledge-extractor/0.1.0",
                        "validity_json": {},
                        "state": "active",
                        "contract_version": 1,
                        "created_at": timestamp,
                    }),
                ),
                upsert(
                    "knowledge_chunks",
                    json!({
                        "id": "evidence-chunk",
                        "version_id": version_id,
                        "text_ref": asset_id,
                        "locator_json": locator_data,
                        "token_start": null,
                        "token_end": null,
                        "byte_start": null,
                        "byte_end": null,
                        "extraction_quality_json": {},
                        "index_generation": 1,
                        "content_hash": content_hash,
                        "contract_version": 1,
                        "created_at": timestamp,
                    }),
                ),
                upsert(
                    "meeting_private_assets",
                    json!({
                        "id": asset_id,
                        "project_id": scope.project_id,
                        "vault_id": vault_id,
                        "owner_scope": owner_scope,
                        "purpose": "knowledge_document",
                        "entity_id": asset_id,
                        "entity_revision": 1,
                        "custody_ref": "meeting-assets/knowledge/evidence-asset.enc",
                        "key_ref": format!("knowledge:{vault_id}"),
                        "ciphertext_sha256": "b".repeat(64),
                        "ciphertext_bytes": 1,
                        "state": "active",
                        "retention_json": {},
                        "contract_version": 1,
                        "created_at": timestamp,
                        "updated_at": timestamp,
                    }),
                ),
            ],
        )
        .unwrap();

        let selected = [collection_id.to_string()].into_iter().collect();
        let evidence = json!([{
            "collectionId": collection_id,
            "documentId": document_id,
            "versionId": version_id,
            "documentVersionNumber": 1,
            "sourceVersion": "v1",
            "contentSha256": content_hash,
            "aclRevision": 1,
            "readGrantId": "read:12345678",
            "locator": "line 1-1 · char 0-16",
            "locatorData": locator_data,
        }]);
        let data_root = root.path().canonicalize().unwrap().join("owner-data");
        validate_current_meeting_agent_evidence(
            &storage,
            &scope.project_id,
            vault_id,
            &owner_scope,
            &selected,
            &evidence,
            &data_root,
        )
        .unwrap();

        commit_rows(
            &storage,
            vec![upsert(
                "knowledge_documents",
                json!({
                    "id": document_id,
                    "collection_id": collection_id,
                    "source_kind": "text",
                    "source_ref": asset_id,
                    "title_ref": "evidence-title",
                    "current_version_id": version_id,
                    "acl_revision": 2,
                    "status": "active",
                    "contract_version": 1,
                    "created_at": timestamp,
                    "updated_at": timestamp,
                }),
            )],
        )
        .unwrap();
        assert_eq!(
            validate_current_meeting_agent_evidence(
                &storage,
                &scope.project_id,
                vault_id,
                &owner_scope,
                &selected,
                &evidence,
                &data_root,
            )
            .unwrap_err(),
            "MEETING_AGENT_EVIDENCE_STALE"
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}
