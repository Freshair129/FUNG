use genesis_block_native::{
    BatchInput, EdgeInput, GenesisTransaction, NodeInput, RelationalColumn, RelationalColumnType,
    RelationalFilter, RelationalForeignKey, RelationalIndex, RelationalMutationGroup,
    RelationalMutationKind, RelationalQuery, RelationalRowMutation, RelationalSchemaPackage,
    RelationalTable, Storage,
};
use serde_json::{json, Value};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use std::path::Path;
use uuid::Uuid;

pub(crate) const NAMESPACE: &str = "fung_mobile";

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
        table("paired_devices", vec![required("id", Text), required("name", Text), required("endpoint", Text), required("trust_state", Text), required("pairing_proof_hash", Text), required("capabilities_json", Json), required("created_at", Text), required("updated_at", Text)], vec![], vec![]),
        table("capability_grants", vec![required("id", Text), required("device_id", Text), required("project_id", Text), required("capabilities_json", Json), nullable("expires_at", Text), nullable("revoked_at", Text), required("created_at", Text)], vec![fk("device_id", "paired_devices"), fk("project_id", "projects")], vec![]),
        table("delegated_jobs", vec![required("id", Text), required("project_id", Text), nullable("executor_device_id", Text), required("operation", Text), required("state", Text), required("progress", Integer), required("input_manifest_hash", Text), nullable("checkpoint_json", Json), required("observed_at", Text), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("executor_device_id", "paired_devices")], vec![]),
        table("speakers", vec![required("id", Text), required("project_id", Text), required("key", Text), required("display_name", Text), nullable("confidence", Real), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects")], vec![]),
        table("model_providers", vec![required("id", Text), required("label", Text), required("runtime_location", Text), required("kind", Text), required("enabled", Boolean), required("config_json", Json), required("created_at", Text), required("updated_at", Text)], vec![], vec![]),
        table("model_runs", vec![required("id", Text), required("recording_id", Text), required("provider_id", Text), required("model_name", Text), required("task_kind", Text), required("runtime_location", Text), required("input_ref", Text), required("output_ref", Text), required("parameters_json", Json), required("created_at", Text)], vec![fk("recording_id", "recordings"), fk("provider_id", "model_providers")], vec![]),
        table("speaker_turns", vec![required("id", Text), required("project_id", Text), required("recording_id", Text), required("speaker_id", Text), required("start_ms", Integer), required("end_ms", Integer), nullable("confidence", Real), required("status", Text), nullable("model_run_id", Text), required("overlap", Boolean), required("revision", Integer), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("recording_id", "recordings"), fk("speaker_id", "speakers"), fk("model_run_id", "model_runs")], vec![]),
        table("speaker_timeline_revisions", vec![required("id", Text), required("project_id", Text), required("operation", Text), required("payload_json", Json), required("created_at", Text)], vec![fk("project_id", "projects")], vec![]),
        table("waveform_tiles", vec![required("id", Text), required("recording_id", Text), required("zoom_level", Integer), required("tile_index", Integer), required("start_ms", Integer), required("end_ms", Integer), required("peaks_json", Json), required("checksum", Text), required("created_at", Text)], vec![fk("recording_id", "recordings")], vec![]),
        table("story_sequences", vec![required("id", Text), required("project_id", Text), required("title", Text), required("duration_ms", Integer), required("current_revision", Integer), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects")], vec![]),
        table("story_clips", vec![required("id", Text), required("sequence_id", Text), nullable("source_turn_id", Text), required("source_recording_id", Text), required("source_start_ms", Integer), required("source_end_ms", Integer), required("timeline_start_ms", Integer), required("speaker_id", Text), nullable("effect_chain_id", Text), required("revision", Integer), required("created_at", Text), required("updated_at", Text)], vec![fk("sequence_id", "story_sequences"), fk("source_turn_id", "speaker_turns"), fk("source_recording_id", "recordings"), fk("speaker_id", "speakers")], vec![]),
        table("story_revisions", vec![required("id", Text), required("sequence_id", Text), required("operation", Text), required("before_json", Json), required("after_json", Json), required("applied", Boolean), required("author_device_id", Text), required("created_at", Text)], vec![fk("sequence_id", "story_sequences")], vec![]),
        table("voice_profiles", vec![required("id", Text), required("project_id", Text), required("display_name", Text), required("rights_basis", Text), required("rights_evidence_ref", Text), required("rights_state", Text), nullable("provider_id", Text), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("provider_id", "model_providers")], vec![]),
        table("effect_chains", vec![required("id", Text), required("project_id", Text), required("owner_kind", Text), required("owner_id", Text), required("label", Text), required("bypassed", Boolean), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects")], vec![]),
        table("effect_nodes", vec![required("id", Text), required("chain_id", Text), required("position", Integer), required("kind", Text), required("parameters_json", Json), required("bypassed", Boolean), required("created_at", Text), required("updated_at", Text)], vec![fk("chain_id", "effect_chains")], vec![]),
        table("model_packages", vec![required("id", Text), required("label", Text), required("provider_kind", Text), required("model_version", Text), required("size_bytes", Integer), nullable("checksum", Text), required("runtime_location", Text), required("install_state", Text), required("compatibility_json", Json), required("languages_json", Json), nullable("license_ref", Text), required("observed_at", Text)], vec![], vec![]),
        table("transcript_segments", vec![required("id", Text), required("project_id", Text), required("recording_id", Text), nullable("speaker_id", Text), required("start_ms", Integer), required("end_ms", Integer), required("text", Text), nullable("confidence", Real), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("recording_id", "recordings"), fk("speaker_id", "speakers")], vec![]),
        table("transcript_refinement_proposals", vec![required("id", Text), required("project_id", Text), nullable("transcript_segment_id", Text), required("original_text", Text), required("proposed_text", Text), required("policy", Text), nullable("model_run_id", Text), required("status", Text), nullable("reviewed_at", Text), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("transcript_segment_id", "transcript_segments"), fk("model_run_id", "model_runs")], vec![]),
        table("agent_voice_grants", vec![required("id", Text), required("project_id", Text), required("mcp_client_id", Text), required("voice_profile_id", Text), required("capability", Text), required("granted_at", Text), nullable("expires_at", Text), nullable("revoked_at", Text)], vec![fk("project_id", "projects"), fk("voice_profile_id", "voice_profiles")], vec![]),
        table("agent_voice_sessions", vec![required("id", Text), required("project_id", Text), required("mcp_client_id", Text), required("voice_profile_id", Text), required("grant_id", Text), required("requested_text_hash", Text), required("state", Text), required("retain_output", Boolean), nullable("stop_actor", Text), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("voice_profile_id", "voice_profiles"), fk("grant_id", "agent_voice_grants")], vec![]),
        table("jobs", vec![required("id", Text), required("project_id", Text), required("type", Text), required("status", Text), required("progress", Integer), required("input_refs_json", Json), required("output_refs_json", Json), nullable("provider_id", Text), nullable("error_code", Text), nullable("error_message", Text), required("attempt_no", Integer), nullable("started_at", Text), nullable("finished_at", Text), required("created_at", Text), required("updated_at", Text)], vec![fk("project_id", "projects"), fk("provider_id", "model_providers")], vec![]),
        table("job_events", vec![required("id", Text), required("job_id", Text), required("status", Text), required("message", Text), required("created_at", Text)], vec![fk("job_id", "jobs")], vec![]),
        table("audit_events", vec![required("id", Text), required("project_id", Text), required("event_type", Text), required("actor", Text), required("payload_json", Json), required("created_at", Text)], vec![fk("project_id", "projects")], vec![]),
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
            vec![fk("project_id", "projects"), fk("recording_id", "recordings")],
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
pub(crate) fn schema() -> RelationalSchemaPackage {
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
    Ok(())
}

/// One-way compatibility import. The retired SQLite file is opened read-only;
/// every imported row becomes a normal signed Genesis transaction.
pub(crate) fn import_legacy_sqlite(storage: &Storage, path: &Path) -> Result<usize, String> {
    if !path.is_file() { return Ok(0); }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let package = schema();
    let priority = |name: &str| match name {
        "projects" => 0, "recordings" => 1, "paired_devices" | "model_providers" | "speakers" | "external_connections" => 2,
        "notes" | "mobile_recording_checkpoints" | "audio_chunks" | "delegated_jobs" | "jobs" => 3,
        "note_revisions" | "graph_nodes" | "model_runs" | "transcript_segments" | "story_sequences" | "voice_profiles" | "effect_chains" | "job_events" | "audit_events" => 4,
        "graph_edges" | "speaker_turns" | "waveform_tiles" | "story_clips" | "transcript_refinement_proposals" | "agent_voice_grants" | "effect_nodes" | "external_imports" => 5,
        "story_revisions" | "agent_voice_sessions" | "capability_grants" | "mutation_log" => 6,
        _ => 7,
    };
    let mut tables = package.tables.clone();
    tables.sort_by_key(|table| priority(&table.name));
    let mut mutations = Vec::new();
    for table in tables {
        let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)", [&table.name], |row| row.get(0)).unwrap_or(false);
        if !exists { continue; }
        let mut column_statement = connection.prepare(&format!("PRAGMA table_info(\"{}\")", table.name)).map_err(|error| error.to_string())?;
        let existing_columns = column_statement.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?.filter_map(Result::ok).collect::<std::collections::HashSet<_>>();
        let columns = table.columns.iter().filter(|column| existing_columns.contains(&column.name)).collect::<Vec<_>>();
        if columns.is_empty() { continue; }
        let sql = format!("SELECT {} FROM \"{}\"", columns.iter().map(|column| format!("\"{}\"", column.name)).collect::<Vec<_>>().join(","), table.name);
        let mut statement = connection.prepare(&sql).map_err(|error| error.to_string())?;
        let rows = statement.query_map([], |row| {
            let mut object = serde_json::Map::new();
            for (index, column) in columns.iter().enumerate() {
                let value = match row.get_ref(index)? { ValueRef::Null => Value::Null, ValueRef::Integer(value) => json!(value), ValueRef::Real(value) => json!(value), ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()), ValueRef::Blob(value) => Value::Array(value.iter().copied().map(Value::from).collect()) };
                object.insert(column.name.clone(), value);
            }
            if table.name == "mobile_recording_checkpoints" && !object.contains_key("id") { if let Some(value) = object.get("recording_id").cloned() { object.insert("id".to_string(), value); } }
            Ok(Value::Object(object))
        }).map_err(|error| error.to_string())?;
        for row in rows { mutations.push(upsert(&table.name, row.map_err(|error| error.to_string())?)); }
    }
    let count = mutations.len();
    if count > 0 { commit_rows(storage, mutations)?; }
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
            graph: BatchInput { nodes: vec![], edges: vec![] },
            vectors: vec![],
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
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
    })
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
                "updated_at": timestamp
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
                    "source": "microphone",
                    "input_path": null,
                    "canonical_audio_path": current.canonical_audio_path,
                    "status": current.status,
                    "duration_ms": chunk.end_ms,
                    "created_at": current.created_at,
                    "updated_at": chunk.timestamp
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
                "source": "microphone",
                "input_path": null,
                "canonical_audio_path": current.canonical_audio_path,
                "status": "completed",
                "duration_ms": current.duration_ms,
                "created_at": current.created_at,
                "updated_at": timestamp
            }),
        )],
    )?;
    capture(storage, &current.recording_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::OpenOptions;

    fn open() -> (std::path::PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-genesis-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .unwrap();
        install(&storage).unwrap();
        (path, storage)
    }

    #[test]
    fn note_and_relation_share_genesis_row_graph_commit_path() {
        let (path, storage) = open();
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
        assert_eq!(storage.stable_frontier(), 3);
        assert!(storage.node_view("note-a").is_some());
        assert!(storage.node_view("note-b").is_some());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn install_is_idempotent_after_a_prior_schema_upgrade() {
        let path = std::env::temp_dir().join(format!("fung-genesis-upgrade-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
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
        assert_eq!(storage.stable_frontier(), 3);
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn legacy_sqlite_is_read_once_into_signed_genesis_rows() {
        let (path, storage) = open();
        let legacy = path.join("fung.db");
        let connection = Connection::open(&legacy).unwrap();
        connection.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT NOT NULL,storage_path TEXT NOT NULL,active_recording_id TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); INSERT INTO projects VALUES('legacy-project','Legacy','projects/legacy',NULL,'t','t');").unwrap();
        drop(connection);
        assert_eq!(import_legacy_sqlite(&storage, &legacy).unwrap(), 1);
        let rows = query(&storage, "projects", &["id", "name"], vec![eq("projects", "id", json!("legacy-project"))], 1).unwrap();
        assert_eq!(rows[0]["projects.name"], "Legacy");
        assert_eq!(storage.stable_frontier(), 1);
        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn schema_v4_adds_external_tables_and_upgrade_is_idempotent() {
        let (path, storage) = open();
        // v4 tables accept rows through the normal adapter path.
        commit_rows(&storage, vec![
            upsert("external_connections", json!({"id": "zoom", "provider": "zoom", "account_label": "user@example.com", "status": "connected", "created_at": "t", "updated_at": "t"})),
        ]).unwrap();
        let rows = query(&storage, "external_connections", &["id", "status"], vec![eq("external_connections", "id", json!("zoom"))], 1).unwrap();
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
        let rows = query(&storage, "tts_test_results", &["id", "status", "latency_ms"], vec![eq("tts_test_results", "id", json!("test-1"))], 1).unwrap();
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
        let rows = query(&storage, "paired_devices", &["id", "public_key"], vec![eq("paired_devices", "id", json!("peer-1"))], 1).unwrap();
        assert_eq!(rows[0]["paired_devices.public_key"], "cGVlci1wdWJsaWMta2V5");
        // Rows written without a public_key (nullable) must still be readable.
        commit_rows(&storage, vec![
            upsert("paired_devices", json!({"id": "peer-2", "name": "FUNG Desktop 2", "endpoint": "192.168.1.21:8765", "trust_state": "paired", "pairing_proof_hash": "sess-uuid-2", "capabilities_json": [], "created_at": "t", "updated_at": "t", "public_key": null})),
        ]).unwrap();
        let rows = query(&storage, "paired_devices", &["id", "public_key"], vec![eq("paired_devices", "id", json!("peer-2"))], 1).unwrap();
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
        let rows = query(&storage, "delegated_jobs", &["id", "executor"], vec![eq("delegated_jobs", "id", json!("job-1"))], 1).unwrap();
        assert_eq!(rows[0]["delegated_jobs.executor"], "cloud");
        // Rows written without executor (nullable) must still be readable.
        commit_rows(&storage, vec![
            upsert("delegated_jobs", json!({
                "id": "job-2", "project_id": "proj-1", "executor_device_id": null,
                "operation": "transcript.transcribe", "state": "queued", "progress": 0,
                "input_manifest_hash": "def456", "checkpoint_json": null,
                "observed_at": "t", "created_at": "t", "updated_at": "t", "executor": null
            })),
        ]).unwrap();
        let rows = query(&storage, "delegated_jobs", &["id", "executor"], vec![eq("delegated_jobs", "id", json!("job-2"))], 1).unwrap();
        assert!(rows[0]["delegated_jobs.executor"].is_null());
        // Re-install after a stepped upgrade must stay idempotent.
        storage.register_relational_schema(schema()).unwrap();
        install(&storage).unwrap();
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}
