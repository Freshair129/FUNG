use crate::{native_recorder::NativeSegment, now, AppError, AppResult, AppState};
#[cfg(test)]
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tauri::State;
use uuid::Uuid;

fn native_segment_file_name(relative_path: &str) -> AppResult<std::ffi::OsString> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err(AppError::InvalidInput(
            "native segment path must stay inside app data".to_string(),
        ));
    }
    relative
        .file_name()
        .map(|name| name.to_os_string())
        .ok_or_else(|| AppError::InvalidInput("native segment filename is missing".to_string()))
}

pub(crate) struct MobileGatewayControl {
    pub(crate) bind: String,
    pub(crate) stop: Arc<AtomicBool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureSession {
    recording_id: String,
    state: String,
    safe_offset_ms: i64,
    segment_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackSegment {
    sequence: i64,
    duration_ms: i64,
    mime_type: String,
    bytes: Vec<u8>,
    has_next: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MobileNoteInput {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) project_id: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) evidence_label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphEdgeInput {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) target_id: String,
    pub(crate) predicate: String,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNodeOutput {
    id: String,
    label: String,
    kind: String,
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphEdgeOutput {
    id: String,
    source_id: String,
    target_id: String,
    predicate: String,
    status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct GraphOutput {
    nodes: Vec<GraphNodeOutput>,
    edges: Vec<GraphEdgeOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpStateOutput {
    enabled: bool,
    bind: Option<String>,
    access_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceIntent {
    intent: String,
    destructive: bool,
    requires_confirmation: bool,
    confidence: f64,
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(crate) fn init_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_recording_checkpoints (
            recording_id TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
            safe_offset_ms INTEGER NOT NULL DEFAULT 0,
            segment_count INTEGER NOT NULL DEFAULT 0,
            last_checksum TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            current_revision_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS note_revisions (
            id TEXT PRIMARY KEY,
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            body TEXT NOT NULL,
            evidence_label TEXT,
            author_device_id TEXT NOT NULL,
            logical_clock TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS graph_nodes (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            -- 'meeting'..'mention' are written by graph_build.rs (structural
            -- meeting/speaker layer plus the LLM extraction layer); this mirror
            -- must accept every entity_type the primary store can hold.
            entity_type TEXT NOT NULL CHECK (entity_type IN ('note', 'project', 'recording', 'person', 'meeting', 'speaker', 'topic', 'decision', 'action_item', 'mention')),
            entity_id TEXT NOT NULL,
            label TEXT NOT NULL,
            position_x REAL NOT NULL DEFAULT 50,
            position_y REAL NOT NULL DEFAULT 50,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, entity_type, entity_id)
        );

        CREATE TABLE IF NOT EXISTS graph_edges (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            source_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
            target_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
            predicate TEXT NOT NULL,
            -- 'ai_proposed' marks LLM-extracted edges from graph_build.rs. It is
            -- storable here but deliberately not writable through
            -- mobile_relation_upsert — see the ALLOWED list on that command.
            epistemic_status TEXT NOT NULL CHECK (epistemic_status IN ('confirmed', 'inferred', 'evidence', 'superseded', 'disputed', 'ai_proposed')),
            provenance_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS mutation_log (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            device_id TEXT NOT NULL,
            logical_clock TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS graph_conflicts (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            entity_id TEXT NOT NULL,
            competing_revision_ids_json TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('open', 'resolved')),
            created_at TEXT NOT NULL,
            resolved_at TEXT
        );

        CREATE TABLE IF NOT EXISTS paired_devices (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            endpoint TEXT NOT NULL UNIQUE,
            trust_state TEXT NOT NULL CHECK (trust_state IN ('paired', 'revoked', 'unreachable')),
            pairing_proof_hash TEXT NOT NULL,
            capabilities_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS capability_grants (
            id TEXT PRIMARY KEY,
            device_id TEXT NOT NULL REFERENCES paired_devices(id) ON DELETE CASCADE,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            capabilities_json TEXT NOT NULL,
            expires_at TEXT,
            revoked_at TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS delegated_jobs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            executor_device_id TEXT REFERENCES paired_devices(id),
            operation TEXT NOT NULL,
            state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'paused', 'completed', 'failed', 'cancelled')),
            progress INTEGER NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
            input_manifest_hash TEXT NOT NULL,
            checkpoint_json TEXT,
            observed_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS speaker_turns (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE RESTRICT,
            start_ms INTEGER NOT NULL CHECK (start_ms >= 0),
            end_ms INTEGER NOT NULL CHECK (end_ms > start_ms),
            confidence REAL CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
            status TEXT NOT NULL CHECK (status IN ('proposed', 'confirmed')),
            model_run_id TEXT REFERENCES model_runs(id) ON DELETE SET NULL,
            overlap INTEGER NOT NULL DEFAULT 0 CHECK (overlap IN (0, 1)),
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS speaker_timeline_revisions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            operation TEXT NOT NULL CHECK (operation IN ('rename', 'split', 'merge', 'confirm')),
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS waveform_tiles (
            id TEXT PRIMARY KEY,
            recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
            zoom_level INTEGER NOT NULL,
            tile_index INTEGER NOT NULL,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            peaks_json TEXT NOT NULL,
            checksum TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(recording_id, zoom_level, tile_index)
        );

        CREATE TABLE IF NOT EXISTS story_sequences (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
            current_revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS story_clips (
            id TEXT PRIMARY KEY,
            sequence_id TEXT NOT NULL REFERENCES story_sequences(id) ON DELETE CASCADE,
            source_turn_id TEXT REFERENCES speaker_turns(id) ON DELETE SET NULL,
            source_recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE RESTRICT,
            source_start_ms INTEGER NOT NULL CHECK (source_start_ms >= 0),
            source_end_ms INTEGER NOT NULL CHECK (source_end_ms > source_start_ms),
            timeline_start_ms INTEGER NOT NULL CHECK (timeline_start_ms >= 0),
            speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE RESTRICT,
            effect_chain_id TEXT REFERENCES effect_chains(id) ON DELETE SET NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS story_revisions (
            id TEXT PRIMARY KEY,
            sequence_id TEXT NOT NULL REFERENCES story_sequences(id) ON DELETE CASCADE,
            operation TEXT NOT NULL,
            before_json TEXT NOT NULL,
            after_json TEXT NOT NULL,
            applied INTEGER NOT NULL DEFAULT 1 CHECK (applied IN (0, 1)),
            author_device_id TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS voice_profiles (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            display_name TEXT NOT NULL,
            rights_basis TEXT NOT NULL CHECK (rights_basis IN ('owned_recording', 'licensed_pack', 'explicit_consent')),
            rights_evidence_ref TEXT NOT NULL,
            rights_state TEXT NOT NULL CHECK (rights_state IN ('valid', 'revoked', 'expired')),
            provider_id TEXT REFERENCES model_providers(id) ON DELETE SET NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS effect_chains (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            owner_kind TEXT NOT NULL CHECK (owner_kind IN ('project', 'story_clip', 'voice_profile')),
            owner_id TEXT NOT NULL,
            label TEXT NOT NULL,
            bypassed INTEGER NOT NULL DEFAULT 0 CHECK (bypassed IN (0, 1)),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS effect_nodes (
            id TEXT PRIMARY KEY,
            chain_id TEXT NOT NULL REFERENCES effect_chains(id) ON DELETE CASCADE,
            position INTEGER NOT NULL CHECK (position >= 0),
            kind TEXT NOT NULL CHECK (kind IN ('pitch_shift', 'reverb', 'delay', 'compressor', 'low_pass')),
            parameters_json TEXT NOT NULL,
            bypassed INTEGER NOT NULL DEFAULT 0 CHECK (bypassed IN (0, 1)),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(chain_id, position)
        );

        CREATE TABLE IF NOT EXISTS model_packages (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            provider_kind TEXT NOT NULL,
            model_version TEXT NOT NULL,
            size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
            checksum TEXT,
            runtime_location TEXT NOT NULL CHECK (runtime_location IN ('mobile', 'desktop', 'cloud')),
            install_state TEXT NOT NULL CHECK (install_state IN ('installed', 'available', 'incompatible', 'unknown')),
            compatibility_json TEXT NOT NULL DEFAULT '{}',
            languages_json TEXT NOT NULL DEFAULT '[]',
            license_ref TEXT,
            observed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS transcript_refinement_proposals (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            transcript_segment_id TEXT REFERENCES transcript_segments(id) ON DELETE SET NULL,
            original_text TEXT NOT NULL,
            proposed_text TEXT NOT NULL,
            policy TEXT NOT NULL,
            model_run_id TEXT REFERENCES model_runs(id) ON DELETE SET NULL,
            status TEXT NOT NULL CHECK (status IN ('proposed', 'accepted', 'rejected', 'partially_accepted')),
            reviewed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_voice_grants (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            mcp_client_id TEXT NOT NULL,
            voice_profile_id TEXT NOT NULL REFERENCES voice_profiles(id) ON DELETE CASCADE,
            capability TEXT NOT NULL CHECK (capability = 'voice.speak'),
            granted_at TEXT NOT NULL,
            expires_at TEXT,
            revoked_at TEXT,
            UNIQUE(project_id, mcp_client_id, voice_profile_id, capability)
        );

        CREATE TABLE IF NOT EXISTS agent_voice_sessions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            mcp_client_id TEXT NOT NULL,
            voice_profile_id TEXT NOT NULL REFERENCES voice_profiles(id) ON DELETE RESTRICT,
            grant_id TEXT NOT NULL REFERENCES agent_voice_grants(id) ON DELETE RESTRICT,
            requested_text_hash TEXT NOT NULL,
            state TEXT NOT NULL CHECK (state IN ('queued', 'speaking', 'muted', 'stopped', 'completed', 'failed')),
            retain_output INTEGER NOT NULL DEFAULT 0 CHECK (retain_output IN (0, 1)),
            stop_actor TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_notes_project_updated ON notes(project_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_graph_nodes_project ON graph_nodes(project_id, entity_type);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(project_id, source_node_id);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(project_id, target_node_id);
        CREATE INDEX IF NOT EXISTS idx_mutation_project_clock ON mutation_log(project_id, logical_clock);
        CREATE INDEX IF NOT EXISTS idx_delegated_jobs_project ON delegated_jobs(project_id, state);
        CREATE INDEX IF NOT EXISTS idx_speaker_turns_viewport ON speaker_turns(recording_id, start_ms, end_ms);
        CREATE INDEX IF NOT EXISTS idx_speaker_turns_speaker ON speaker_turns(project_id, speaker_id);
        CREATE INDEX IF NOT EXISTS idx_waveform_tiles_viewport ON waveform_tiles(recording_id, zoom_level, tile_index);
        CREATE INDEX IF NOT EXISTS idx_story_sequences_project ON story_sequences(project_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_story_clips_sequence ON story_clips(sequence_id, timeline_start_ms);
        CREATE INDEX IF NOT EXISTS idx_story_revisions_sequence ON story_revisions(sequence_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_effect_nodes_chain ON effect_nodes(chain_id, position);
        CREATE INDEX IF NOT EXISTS idx_refinement_project_status ON transcript_refinement_proposals(project_id, status, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_agent_voice_sessions_active ON agent_voice_sessions(project_id, state, updated_at DESC);
        "#,
    )?;
    Ok(())
}

fn genesis_capture_session(record: crate::genesis_adapter::CaptureRecord) -> CaptureSession {
    CaptureSession {
        recording_id: record.recording_id,
        state: record.status,
        safe_offset_ms: record.safe_offset_ms,
        segment_count: record.segment_count,
    }
}

#[tauri::command]
pub(crate) fn mobile_capture_start(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<CaptureSession> {
    let root = state.data_root.to_path_buf();
    if let Some(active) = crate::genesis_adapter::active_capture(&state.genesis, &project_id)
        .map_err(AppError::Genesis)?
    {
        return Ok(genesis_capture_session(active));
    }

    let recording_id = Uuid::new_v4().to_string();
    let recording_dir = root
        .join("projects")
        .join(&project_id)
        .join("recordings")
        .join(&recording_id);
    fs::create_dir_all(&recording_dir)?;
    let canonical_path = recording_dir.join("manifest.json").display().to_string();
    let timestamp = now();
    let storage_path = root
        .join("projects")
        .join(&project_id)
        .display()
        .to_string();
    crate::genesis_adapter::start_capture(
        &state.genesis,
        &project_id,
        &storage_path,
        &recording_id,
        &canonical_path,
        &timestamp,
    )
    .map(genesis_capture_session)
    .map_err(AppError::Genesis)
}

#[tauri::command]
pub(crate) fn mobile_capture_append_segment(
    recording_id: String,
    bytes: Vec<u8>,
    duration_ms: i64,
    state: State<'_, AppState>,
) -> AppResult<CaptureSession> {
    if bytes.is_empty() || duration_ms <= 0 {
        return Err(AppError::InvalidInput(
            "audio segment and positive duration are required".to_string(),
        ));
    }
    let current = crate::genesis_adapter::capture(&state.genesis, &recording_id)
        .map_err(AppError::Genesis)?;
    if current.status != "recording" {
        return Err(AppError::InvalidInput(
            "recording is not active".to_string(),
        ));
    }
    let sequence = current.segment_count + 1;
    let start_ms = current.safe_offset_ms;
    let end_ms = start_ms + duration_ms;
    let root = &state.data_root;
    let directory = root
        .join("projects")
        .join(&current.project_id)
        .join("recordings")
        .join(&recording_id);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("segment-{sequence:06}.webm"));
    fs::write(&path, &bytes)?;
    let checksum = sha256(&bytes);
    let timestamp = now();
    crate::genesis_adapter::append_capture_chunk(
        &state.genesis,
        &current,
        crate::genesis_adapter::AudioChunk {
            id: &Uuid::new_v4().to_string(),
            file_path: &path.display().to_string(),
            start_ms,
            end_ms,
            byte_size: bytes.len() as i64,
            checksum: &checksum,
            timestamp: &timestamp,
        },
    )
    .map(genesis_capture_session)
    .map_err(AppError::Genesis)
}

#[tauri::command]
pub(crate) fn mobile_capture_reconcile_native(
    recording_id: String,
    mut segments: Vec<NativeSegment>,
    state: State<'_, AppState>,
) -> AppResult<CaptureSession> {
    let mut current = crate::genesis_adapter::capture(&state.genesis, &recording_id)
        .map_err(AppError::Genesis)?;
    if current.status != "recording" && current.status != "paused" {
        return Ok(genesis_capture_session(current));
    }

    segments.sort_by_key(|segment| segment.sequence);
    let root = &state.data_root;
    let native_root = root.join("native-recordings").join(&recording_id);
    let canonical_root = root
        .join("projects")
        .join(&current.project_id)
        .join("recordings")
        .join(&recording_id);
    fs::create_dir_all(&canonical_root)?;

    for segment in segments {
        if segment.sequence <= 0 || segment.duration_ms <= 0 || segment.byte_size <= 0 {
            return Err(AppError::InvalidInput(
                "native segment metadata must be positive".to_string(),
            ));
        }
        let file_name = native_segment_file_name(&segment.relative_path)?;
        let source = native_root.join(file_name);
        if !source.starts_with(&native_root) || !source.is_file() {
            return Err(AppError::InvalidInput(
                "native segment is missing from the recording directory".to_string(),
            ));
        }
        let existing = crate::genesis_adapter::query(
            &state.genesis,
            "audio_chunks",
            &["id"],
            vec![
                crate::genesis_adapter::eq(
                    "audio_chunks",
                    "recording_id",
                    serde_json::json!(recording_id),
                ),
                crate::genesis_adapter::eq(
                    "audio_chunks",
                    "sequence_no",
                    serde_json::json!(segment.sequence),
                ),
            ],
            1,
        )
        .map_err(AppError::Genesis)?;
        if !existing.is_empty() {
            continue;
        }

        if segment.sequence != current.segment_count + 1 {
            break;
        }
        let bytes = fs::read(&source)?;
        if bytes.len() as i64 != segment.byte_size || sha256(&bytes) != segment.checksum {
            return Err(AppError::InvalidInput(
                "native segment checksum or size does not match its journal".to_string(),
            ));
        }
        let destination = canonical_root.join(format!("segment-{:06}.m4a", segment.sequence));
        let temporary = destination.with_extension("m4a.tmp");
        fs::write(&temporary, &bytes)?;
        fs::rename(&temporary, &destination)?;
        let end_ms = current.safe_offset_ms + segment.duration_ms;
        let timestamp = now();
        current = crate::genesis_adapter::append_capture_chunk(
            &state.genesis,
            &current,
            crate::genesis_adapter::AudioChunk {
                id: &Uuid::new_v4().to_string(),
                file_path: &destination.display().to_string(),
                start_ms: current.safe_offset_ms,
                end_ms,
                byte_size: segment.byte_size,
                checksum: &segment.checksum,
                timestamp: &timestamp,
            },
        )
        .map_err(AppError::Genesis)?;
    }
    Ok(genesis_capture_session(current))
}

#[tauri::command]
pub(crate) fn mobile_capture_finish(
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<CaptureSession> {
    let current = crate::genesis_adapter::capture(&state.genesis, &recording_id)
        .map_err(AppError::Genesis)?;
    let timestamp = now();
    crate::genesis_adapter::finish_capture(&state.genesis, &current, &timestamp)
        .map(genesis_capture_session)
        .map_err(AppError::Genesis)
}

#[tauri::command]
pub(crate) fn mobile_capture_playback_segment(
    recording_id: String,
    sequence: i64,
    state: State<'_, AppState>,
) -> AppResult<PlaybackSegment> {
    if sequence <= 0 {
        return Err(AppError::InvalidInput(
            "playback sequence must be positive".to_string(),
        ));
    }
    let capture = crate::genesis_adapter::capture(&state.genesis, &recording_id)
        .map_err(AppError::Genesis)?;
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "audio_chunks",
        &["sequence_no", "file_path", "start_ms", "end_ms", "checksum"],
        vec![
            crate::genesis_adapter::eq(
                "audio_chunks",
                "recording_id",
                serde_json::json!(recording_id),
            ),
            crate::genesis_adapter::eq("audio_chunks", "sequence_no", serde_json::json!(sequence)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("playback segment not found".to_string()))?;
    let path = std::path::PathBuf::from(text_value(&row, "audio_chunks.file_path")?);
    let root = state
        .data_root
        .join("projects")
        .join(&capture.project_id)
        .join("recordings")
        .join(&recording_id);
    if !path.starts_with(&root) || !path.is_file() {
        return Err(AppError::InvalidInput(
            "playback segment escaped the recording sandbox or is missing".to_string(),
        ));
    }
    let bytes = fs::read(&path)?;
    if sha256(&bytes) != text_value(&row, "audio_chunks.checksum")? {
        return Err(AppError::InvalidInput(
            "playback segment checksum does not match Genesis metadata".to_string(),
        ));
    }
    Ok(PlaybackSegment {
        sequence,
        duration_ms: int_value(&row, "audio_chunks.end_ms")?
            - int_value(&row, "audio_chunks.start_ms")?,
        mime_type: if path.extension().and_then(|value| value.to_str()) == Some("m4a") {
            "audio/mp4"
        } else {
            "audio/webm"
        }
        .to_string(),
        bytes,
        has_next: sequence < capture.segment_count,
    })
}

#[tauri::command]
pub(crate) fn mobile_note_upsert(
    note: MobileNoteInput,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if note.title.trim().is_empty() {
        return Err(AppError::InvalidInput("note title is required".to_string()));
    }
    let storage_path = state
        .genesis_path
        .parent()
        .unwrap_or(&state.genesis_path)
        .join("projects")
        .join(&note.project_id)
        .display()
        .to_string();
    crate::genesis_adapter::commit_note(&state.genesis, &note, &storage_path)
        .map_err(AppError::Genesis)
}

/// Epistemic statuses a mobile client may assert through
/// `mobile_relation_upsert`. Deliberately narrower than the `graph_edges`
/// CHECK constraint: `ai_proposed` is storable (graph_build.rs writes it) but
/// certifies that an edge came from the local extraction pipeline, so a client
/// must not be able to mint it. Reviewing an ai_proposed edge means upserting
/// one of the statuses below over it, which is the intended transition.
const CLIENT_WRITABLE_EPISTEMIC_STATUS: [&str; 5] = [
    "confirmed",
    "inferred",
    "evidence",
    "superseded",
    "disputed",
];

#[tauri::command]
pub(crate) fn mobile_relation_upsert(
    edge: GraphEdgeInput,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if !CLIENT_WRITABLE_EPISTEMIC_STATUS.contains(&edge.status.as_str()) {
        return Err(AppError::InvalidInput(
            "invalid epistemic status".to_string(),
        ));
    }
    let source = crate::genesis_adapter::query(
        &state.genesis,
        "graph_nodes",
        &["project_id"],
        vec![crate::genesis_adapter::eq(
            "graph_nodes",
            "id",
            serde_json::json!(edge.source_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?;
    let project_id = source
        .first()
        .and_then(|row| row.get("graph_nodes.project_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::InvalidInput("source graph node was not found".to_string()))?
        .to_string();
    let timestamp = now();
    crate::genesis_adapter::commit_relation(&state.genesis, &project_id, &edge, &timestamp)
        .map_err(AppError::Genesis)
}

#[tauri::command]
pub(crate) fn mobile_graph_query(
    project_id: String,
    depth: i64,
    state: State<'_, AppState>,
) -> AppResult<GraphOutput> {
    if !(1..=2).contains(&depth) {
        return Err(AppError::InvalidInput(
            "graph depth must be 1 or 2".to_string(),
        ));
    }
    // This is the only graph read surface, and one meeting's structural plus
    // extracted nodes now realistically exceeds the old 200-row cap. Raised
    // to the engine's hard 1000-row query ceiling — a project whose
    // cumulative graph_nodes exceed that still loses its tail here, same
    // limitation as every other query against this table.
    let node_rows = crate::genesis_adapter::query(
        &state.genesis,
        "graph_nodes",
        &["id", "label", "entity_type", "position_x", "position_y"],
        vec![crate::genesis_adapter::eq(
            "graph_nodes",
            "project_id",
            serde_json::json!(project_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?;
    let nodes = node_rows
        .into_iter()
        .filter_map(|row| {
            Some(GraphNodeOutput {
                id: row.get("graph_nodes.id")?.as_str()?.to_string(),
                label: row.get("graph_nodes.label")?.as_str()?.to_string(),
                kind: row.get("graph_nodes.entity_type")?.as_str()?.to_string(),
                x: row.get("graph_nodes.position_x")?.as_f64()?,
                y: row.get("graph_nodes.position_y")?.as_f64()?,
            })
        })
        .collect();
    let edge_rows = crate::genesis_adapter::query(
        &state.genesis,
        "graph_edges",
        &[
            "id",
            "source_node_id",
            "target_node_id",
            "predicate",
            "epistemic_status",
        ],
        vec![crate::genesis_adapter::eq(
            "graph_edges",
            "project_id",
            serde_json::json!(project_id),
        )],
        400,
    )
    .map_err(AppError::Genesis)?;
    let edges = edge_rows
        .into_iter()
        .filter_map(|row| {
            Some(GraphEdgeOutput {
                id: row.get("graph_edges.id")?.as_str()?.to_string(),
                source_id: row.get("graph_edges.source_node_id")?.as_str()?.to_string(),
                target_id: row.get("graph_edges.target_node_id")?.as_str()?.to_string(),
                predicate: row.get("graph_edges.predicate")?.as_str()?.to_string(),
                status: row
                    .get("graph_edges.epistemic_status")?
                    .as_str()?
                    .to_string(),
            })
        })
        .collect();
    Ok(GraphOutput { nodes, edges })
}

/// Upserts the Genesis `paired_devices` row for a mobile/desktop pairing that
/// has already been cryptographically verified server-side via the Supabase
/// `confirm_pairing` RPC. `peer_device_id` is the cloud `devices.id` for the
/// counterpart device, and `pairing_session_id` is carried through as the
/// `pairing_proof_hash` — a reference to the verified session, not a locally
/// computed proof (unlike the fake path this replaces). `public_key` is the
/// desktop peer's base64 ed25519 verifying key (Supabase `devices.public_key`,
/// Task 5), cached locally so the Noise KK handshake has both static keys
/// without a network round-trip; `None` when the caller could not fetch it
/// (e.g. the peer registered before Task 5 shipped).
fn pairing_complete_inner(
    storage: &genesis_block_native::Storage,
    peer_device_id: &str,
    name: &str,
    endpoint: &str,
    pairing_session_id: &str,
    public_key: Option<&str>,
) -> AppResult<()> {
    let peer_device_id = peer_device_id.trim();
    let pairing_session_id = pairing_session_id.trim();
    let name = name.trim();
    if peer_device_id.is_empty() {
        return Err(AppError::InvalidInput(
            "peer device id is required".to_string(),
        ));
    }
    if pairing_session_id.is_empty() {
        return Err(AppError::InvalidInput(
            "pairing session id is required".to_string(),
        ));
    }
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::InvalidInput(
            "device name must be between 1 and 120 characters".to_string(),
        ));
    }
    let timestamp = now();
    let existing = crate::genesis_adapter::query(
        storage,
        "paired_devices",
        &["created_at"],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "id",
            serde_json::json!(peer_device_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next();
    let created_at = existing
        .as_ref()
        .and_then(|row| row.get("paired_devices.created_at"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&timestamp)
        .to_string();
    crate::genesis_adapter::commit_rows(
        storage,
        vec![crate::genesis_adapter::upsert(
            "paired_devices",
            serde_json::json!({
                "id": peer_device_id, "name": name, "endpoint": endpoint, "trust_state": "paired",
                "pairing_proof_hash": pairing_session_id, "capabilities_json": [],
                "created_at": created_at, "updated_at": timestamp, "public_key": public_key
            }),
        )],
    )
    .map_err(AppError::Genesis)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn mobile_pairing_complete(
    peer_device_id: String,
    name: String,
    endpoint: String,
    pairing_session_id: String,
    public_key: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    pairing_complete_inner(
        &state.genesis,
        &peer_device_id,
        &name,
        &endpoint,
        &pairing_session_id,
        public_key.as_deref(),
    )
}

#[tauri::command]
pub(crate) fn mobile_voice_parse(transcript: String) -> VoiceIntent {
    parse_voice_intent(&transcript)
}

fn parse_voice_intent(transcript: &str) -> VoiceIntent {
    let normalized = transcript.trim().to_lowercase();
    let (intent, destructive, confidence) =
        if normalized.contains("เริ่ม") && normalized.contains("บันทึก") {
            ("capture.start", false, 0.98)
        } else if normalized.contains("หยุด") && normalized.contains("บันทึก") {
            ("capture.stop", false, 0.96)
        } else if normalized.contains("สร้าง") && normalized.contains("โน้ต") {
            ("note.create", false, 0.94)
        } else if normalized.contains("ลบ") {
            ("entity.delete", true, 0.88)
        } else if normalized.contains("กราฟ") {
            ("graph.open", false, 0.91)
        } else {
            ("unknown", false, 0.25)
        };
    VoiceIntent {
        intent: intent.to_string(),
        destructive,
        requires_confirmation: destructive || confidence < 0.8,
        confidence,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimelineSpeaker {
    id: String,
    project_id: String,
    key: String,
    display_name: String,
    confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimelineTurn {
    id: String,
    project_id: String,
    recording_id: String,
    speaker_id: String,
    start_ms: i64,
    end_ms: i64,
    confidence: Option<f64>,
    status: String,
    overlap: bool,
    revision: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimelineOutput {
    recording_id: Option<String>,
    duration_ms: i64,
    speakers: Vec<TimelineSpeaker>,
    turns: Vec<TimelineTurn>,
    source: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DiarizationJobOutput {
    id: String,
    state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiarizationTurnInput {
    speaker_key: String,
    display_name: Option<String>,
    start_ms: i64,
    end_ms: i64,
    confidence: Option<f64>,
    overlap: Option<bool>,
}

fn text_value(row: &serde_json::Value, key: &str) -> AppResult<String> {
    row.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::Genesis(format!("missing text column {key}")))
}

fn int_value(row: &serde_json::Value, key: &str) -> AppResult<i64> {
    row.get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| AppError::Genesis(format!("missing integer column {key}")))
}

fn bool_value(row: &serde_json::Value, key: &str) -> bool {
    row.get(key)
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_i64().map(|number| number != 0))
        })
        .unwrap_or(false)
}

fn json_value(row: &serde_json::Value, key: &str) -> serde_json::Value {
    row.get(key)
        .and_then(serde_json::Value::as_str)
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn timeline_output(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> AppResult<TimelineOutput> {
    let mut recordings = crate::genesis_adapter::query(
        storage,
        "recordings",
        &["id", "duration_ms", "updated_at"],
        vec![crate::genesis_adapter::eq(
            "recordings",
            "project_id",
            serde_json::json!(project_id),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    recordings.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("recordings.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let Some(recording) = recordings.first() else {
        return Ok(TimelineOutput {
            recording_id: None,
            duration_ms: 0,
            speakers: Vec::new(),
            turns: Vec::new(),
            source: "local".to_string(),
        });
    };
    let recording_id = text_value(recording, "recordings.id")?;
    let duration_ms = int_value(recording, "recordings.duration_ms")?;
    let mut turn_rows = crate::genesis_adapter::query(
        storage,
        "speaker_turns",
        &[
            "id",
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "confidence",
            "status",
            "overlap",
            "revision",
        ],
        vec![crate::genesis_adapter::eq(
            "speaker_turns",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?;
    turn_rows.sort_by_key(|row| {
        (
            row.get("speaker_turns.start_ms")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default(),
            row.get("speaker_turns.end_ms")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default(),
        )
    });
    let turns = turn_rows
        .into_iter()
        .map(|row| {
            Ok(TimelineTurn {
                id: text_value(&row, "speaker_turns.id")?,
                project_id: text_value(&row, "speaker_turns.project_id")?,
                recording_id: text_value(&row, "speaker_turns.recording_id")?,
                speaker_id: text_value(&row, "speaker_turns.speaker_id")?,
                start_ms: int_value(&row, "speaker_turns.start_ms")?,
                end_ms: int_value(&row, "speaker_turns.end_ms")?,
                confidence: row
                    .get("speaker_turns.confidence")
                    .and_then(serde_json::Value::as_f64),
                status: text_value(&row, "speaker_turns.status")?,
                overlap: bool_value(&row, "speaker_turns.overlap"),
                revision: int_value(&row, "speaker_turns.revision")?,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    let speaker_ids: std::collections::HashSet<&str> =
        turns.iter().map(|turn| turn.speaker_id.as_str()).collect();
    let mut speakers = crate::genesis_adapter::query(
        storage,
        "speakers",
        &["id", "project_id", "key", "display_name", "confidence"],
        vec![crate::genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        500,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .filter(|row| {
        row.get("speakers.id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| speaker_ids.contains(id))
    })
    .map(|row| {
        Ok(TimelineSpeaker {
            id: text_value(&row, "speakers.id")?,
            project_id: text_value(&row, "speakers.project_id")?,
            key: text_value(&row, "speakers.key")?,
            display_name: text_value(&row, "speakers.display_name")?,
            confidence: row
                .get("speakers.confidence")
                .and_then(serde_json::Value::as_f64),
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    speakers.sort_by(|a, b| a.key.cmp(&b.key));
    let mut runs = crate::genesis_adapter::query(
        storage,
        "model_runs",
        &["runtime_location", "task_kind", "created_at"],
        vec![crate::genesis_adapter::eq(
            "model_runs",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    runs.retain(|row| {
        row.get("model_runs.task_kind")
            .and_then(serde_json::Value::as_str)
            == Some("diarization")
    });
    runs.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("model_runs.created_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let source = runs
        .first()
        .and_then(|row| row.get("model_runs.runtime_location"))
        .and_then(serde_json::Value::as_str);
    Ok(TimelineOutput {
        recording_id: Some(recording_id),
        duration_ms,
        speakers,
        turns,
        source: if source == Some("lan") {
            "desktop"
        } else {
            "local"
        }
        .to_string(),
    })
}

fn timeline_revision_mutation(
    project_id: &str,
    operation: &str,
    payload: serde_json::Value,
) -> genesis_block_native::RelationalRowMutation {
    crate::genesis_adapter::upsert(
        "speaker_timeline_revisions",
        serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "operation": operation, "payload_json": payload, "created_at": now()}),
    )
}

#[tauri::command]
pub(crate) fn mobile_timeline_query(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<TimelineOutput> {
    timeline_output(&state.genesis, &project_id)
}

#[tauri::command]
pub(crate) fn mobile_diarization_start(
    project_id: String,
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<DiarizationJobOutput> {
    let mut devices = crate::genesis_adapter::query(
        &state.genesis,
        "paired_devices",
        &["id", "updated_at"],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "trust_state",
            serde_json::json!("paired"),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    devices.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("paired_devices.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let executor = devices
        .first()
        .and_then(|row| row.get("paired_devices.id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let Some(executor_device_id) = executor else {
        return Err(AppError::InvalidInput(
            "a paired FUNG Desktop is required for diarization".to_string(),
        ));
    };
    let valid = crate::genesis_adapter::query(
        &state.genesis,
        "recordings",
        &["id"],
        vec![
            crate::genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id)),
            crate::genesis_adapter::eq("recordings", "project_id", serde_json::json!(project_id)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?;
    if valid.is_empty() {
        return Err(AppError::InvalidInput(
            "recording does not belong to this project".to_string(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("delegated_jobs", serde_json::json!({"id": id, "project_id": project_id, "executor_device_id": executor_device_id, "operation": "transcript.diarize", "state": "queued", "progress": 0, "input_manifest_hash": sha256(recording_id.as_bytes()), "checkpoint_json": null, "observed_at": timestamp, "created_at": timestamp, "updated_at": timestamp}))]).map_err(AppError::Genesis)?;
    Ok(DiarizationJobOutput {
        id,
        state: "queued".to_string(),
    })
}

#[tauri::command]
pub(crate) fn mobile_processing_job_start(
    project_id: String,
    operation: String,
    input_ref: String,
    state: State<'_, AppState>,
) -> AppResult<DiarizationJobOutput> {
    const ALLOWED: [&str; 4] = [
        "transcript.refine",
        "audio.effect_preview",
        "story.export",
        "model.install",
    ];
    if !ALLOWED.contains(&operation.as_str())
        || input_ref.trim().is_empty()
        || input_ref.chars().count() > 512
    {
        return Err(AppError::InvalidInput(
            "unsupported processing operation or input reference".to_string(),
        ));
    }
    let mut devices = crate::genesis_adapter::query(
        &state.genesis,
        "paired_devices",
        &["id", "updated_at"],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "trust_state",
            serde_json::json!("paired"),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    devices.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("paired_devices.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let executor = devices
        .first()
        .and_then(|row| row.get("paired_devices.id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let Some(executor_device_id) = executor else {
        return Err(AppError::InvalidInput(
            "a paired FUNG Desktop is required for this processing job".to_string(),
        ));
    };
    let project_exists = crate::genesis_adapter::query(
        &state.genesis,
        "projects",
        &["id"],
        vec![crate::genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?;
    if project_exists.is_empty() {
        return Err(AppError::InvalidInput("project does not exist".to_string()));
    }
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("delegated_jobs", serde_json::json!({"id": id, "project_id": project_id, "executor_device_id": executor_device_id, "operation": operation, "state": "queued", "progress": 0, "input_manifest_hash": sha256(input_ref.as_bytes()), "checkpoint_json": null, "observed_at": timestamp, "created_at": timestamp, "updated_at": timestamp}))]).map_err(AppError::Genesis)?;
    Ok(DiarizationJobOutput {
        id,
        state: "queued".to_string(),
    })
}

#[tauri::command]
pub(crate) fn mobile_diarization_import(
    project_id: String,
    recording_id: String,
    provider_id: String,
    turns: Vec<DiarizationTurnInput>,
    state: State<'_, AppState>,
) -> AppResult<TimelineOutput> {
    if turns.iter().any(|turn| {
        turn.speaker_key.trim().is_empty()
            || turn.start_ms < 0
            || turn.end_ms <= turn.start_ms
            || turn
                .confidence
                .is_some_and(|value| !(0.0..=1.0).contains(&value))
    }) {
        return Err(AppError::InvalidInput(
            "invalid diarization turn".to_string(),
        ));
    }
    let timestamp = now();
    let model_run_id = Uuid::new_v4().to_string();
    let mut mutations = Vec::new();
    for row in crate::genesis_adapter::query(
        &state.genesis,
        "speaker_turns",
        &["id", "status"],
        vec![crate::genesis_adapter::eq(
            "speaker_turns",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?
    {
        if row
            .get("speaker_turns.status")
            .and_then(serde_json::Value::as_str)
            == Some("proposed")
        {
            mutations.push(crate::genesis_adapter::delete(
                "speaker_turns",
                &text_value(&row, "speaker_turns.id")?,
            ));
        }
    }
    mutations.push(crate::genesis_adapter::upsert("model_providers", serde_json::json!({"id": provider_id, "label": "FUNG Desktop", "runtime_location": "lan", "kind": "diarization", "enabled": true, "config_json": {}, "created_at": timestamp, "updated_at": timestamp})));
    mutations.push(crate::genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": provider_id, "model_name": "FUNG Desktop diarization", "task_kind": "diarization", "runtime_location": "lan", "input_ref": recording_id, "output_ref": format!("speaker-turns:{recording_id}"), "parameters_json": {}, "created_at": timestamp})));
    let existing_speakers = crate::genesis_adapter::query(
        &state.genesis,
        "speakers",
        &["id", "key", "created_at"],
        vec![crate::genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        500,
    )
    .map_err(AppError::Genesis)?;
    for turn in turns {
        let existing = existing_speakers.iter().find(|row| {
            row.get("speakers.key").and_then(serde_json::Value::as_str)
                == Some(turn.speaker_key.as_str())
        });
        let speaker_id = existing
            .and_then(|row| row.get("speakers.id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let created_at = existing
            .and_then(|row| row.get("speakers.created_at"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&timestamp)
            .to_string();
        mutations.push(crate::genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id, "project_id": project_id, "key": turn.speaker_key, "display_name": turn.display_name.unwrap_or_else(|| "ผู้พูด".to_string()), "confidence": turn.confidence, "created_at": created_at, "updated_at": timestamp})));
        mutations.push(crate::genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": turn.start_ms, "end_ms": turn.end_ms, "confidence": turn.confidence, "status": "proposed", "model_run_id": model_run_id, "overlap": turn.overlap.unwrap_or(false), "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }
    crate::genesis_adapter::commit_rows(&state.genesis, mutations).map_err(AppError::Genesis)?;
    timeline_output(&state.genesis, &project_id)
}

#[tauri::command]
pub(crate) fn mobile_speaker_rename(
    speaker_id: String,
    display_name: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let name = display_name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::InvalidInput(
            "speaker name must contain 1 to 80 characters".to_string(),
        ));
    }
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "speakers",
        &[
            "id",
            "project_id",
            "key",
            "display_name",
            "confidence",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "speakers",
            "id",
            serde_json::json!(speaker_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("speaker not found".to_string()))?;
    let project_id = text_value(&row, "speakers.project_id")?;
    let old_name = text_value(&row, "speakers.display_name")?;
    let timestamp = now();
    crate::genesis_adapter::commit_rows(&state.genesis, vec![
        crate::genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id, "project_id": project_id, "key": text_value(&row, "speakers.key")?, "display_name": name, "confidence": row.get("speakers.confidence").cloned().unwrap_or(serde_json::Value::Null), "created_at": text_value(&row, "speakers.created_at")?, "updated_at": timestamp})),
        timeline_revision_mutation(&project_id, "rename", serde_json::json!({"speakerId": speaker_id, "from": old_name, "to": name})),
    ]).map_err(AppError::Genesis)
}

#[tauri::command]
pub(crate) fn mobile_speaker_turn_split(
    turn_id: String,
    split_ms: i64,
    state: State<'_, AppState>,
) -> AppResult<TimelineOutput> {
    let turn = crate::genesis_adapter::query(
        &state.genesis,
        "speaker_turns",
        &[
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "confidence",
            "status",
            "model_run_id",
            "overlap",
            "revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "speaker_turns",
            "id",
            serde_json::json!(turn_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("speaker turn not found".to_string()))?;
    let start_ms = int_value(&turn, "speaker_turns.start_ms")?;
    let end_ms = int_value(&turn, "speaker_turns.end_ms")?;
    if split_ms <= start_ms || split_ms >= end_ms {
        return Err(AppError::InvalidInput(
            "split point must be inside the selected turn".to_string(),
        ));
    }
    let timestamp = now();
    let new_id = Uuid::new_v4().to_string();
    let project_id = text_value(&turn, "speaker_turns.project_id")?;
    let recording_id = text_value(&turn, "speaker_turns.recording_id")?;
    let speaker_id = text_value(&turn, "speaker_turns.speaker_id")?;
    let revision = int_value(&turn, "speaker_turns.revision")? + 1;
    let common = |id: &str, start: i64, end: i64, created: String| {
        crate::genesis_adapter::upsert(
            "speaker_turns",
            serde_json::json!({"id": id, "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": start, "end_ms": end, "confidence": turn.get("speaker_turns.confidence").cloned().unwrap_or(serde_json::Value::Null), "status": text_value(&turn, "speaker_turns.status").unwrap_or_else(|_| "proposed".to_string()), "model_run_id": turn.get("speaker_turns.model_run_id").cloned().unwrap_or(serde_json::Value::Null), "overlap": turn.get("speaker_turns.overlap").cloned().unwrap_or(serde_json::Value::Bool(false)), "revision": revision, "created_at": created, "updated_at": timestamp}),
        )
    };
    crate::genesis_adapter::commit_rows(&state.genesis, vec![common(&turn_id, start_ms, split_ms, text_value(&turn, "speaker_turns.created_at")?), common(&new_id, split_ms, end_ms, timestamp.clone()), timeline_revision_mutation(&project_id, "split", serde_json::json!({"turnId": turn_id, "newTurnId": new_id, "splitMs": split_ms, "originalEndMs": end_ms}))]).map_err(AppError::Genesis)?;
    timeline_output(&state.genesis, &project_id)
}

#[tauri::command]
pub(crate) fn mobile_speaker_merge(
    project_id: String,
    source_speaker_id: String,
    target_speaker_id: String,
    state: State<'_, AppState>,
) -> AppResult<TimelineOutput> {
    if source_speaker_id == target_speaker_id {
        return Err(AppError::InvalidInput(
            "source and target speakers must differ".to_string(),
        ));
    }
    let speakers = crate::genesis_adapter::query(
        &state.genesis,
        "speakers",
        &["id"],
        vec![crate::genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        500,
    )
    .map_err(AppError::Genesis)?;
    if ![source_speaker_id.as_str(), target_speaker_id.as_str()]
        .iter()
        .all(|id| {
            speakers
                .iter()
                .any(|row| row.get("speakers.id").and_then(serde_json::Value::as_str) == Some(*id))
        })
    {
        return Err(AppError::InvalidInput(
            "both speakers must belong to this project".to_string(),
        ));
    }
    let timestamp = now();
    let rows = crate::genesis_adapter::query(
        &state.genesis,
        "speaker_turns",
        &[
            "id",
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "confidence",
            "status",
            "model_run_id",
            "overlap",
            "revision",
            "created_at",
        ],
        vec![
            crate::genesis_adapter::eq(
                "speaker_turns",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "speaker_turns",
                "speaker_id",
                serde_json::json!(source_speaker_id),
            ),
        ],
        1000,
    )
    .map_err(AppError::Genesis)?;
    let mut mutations = rows.into_iter().map(|row| Ok(crate::genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": text_value(&row, "speaker_turns.id")?, "project_id": project_id, "recording_id": text_value(&row, "speaker_turns.recording_id")?, "speaker_id": target_speaker_id, "start_ms": int_value(&row, "speaker_turns.start_ms")?, "end_ms": int_value(&row, "speaker_turns.end_ms")?, "confidence": row.get("speaker_turns.confidence").cloned().unwrap_or(serde_json::Value::Null), "status": text_value(&row, "speaker_turns.status")?, "model_run_id": row.get("speaker_turns.model_run_id").cloned().unwrap_or(serde_json::Value::Null), "overlap": row.get("speaker_turns.overlap").cloned().unwrap_or(serde_json::Value::Bool(false)), "revision": int_value(&row, "speaker_turns.revision")? + 1, "created_at": text_value(&row, "speaker_turns.created_at")?, "updated_at": timestamp})))).collect::<AppResult<Vec<_>>>()?;
    mutations.push(timeline_revision_mutation(&project_id, "merge", serde_json::json!({"sourceSpeakerId": source_speaker_id, "targetSpeakerId": target_speaker_id})));
    crate::genesis_adapter::commit_rows(&state.genesis, mutations).map_err(AppError::Genesis)?;
    timeline_output(&state.genesis, &project_id)
}

#[tauri::command]
pub(crate) fn mobile_speaker_turn_confirm(
    turn_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "speaker_turns",
        &[
            "project_id",
            "recording_id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "confidence",
            "status",
            "model_run_id",
            "overlap",
            "revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "speaker_turns",
            "id",
            serde_json::json!(turn_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("speaker turn not found".to_string()))?;
    let project_id = text_value(&row, "speaker_turns.project_id")?;
    crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": turn_id, "project_id": project_id, "recording_id": text_value(&row, "speaker_turns.recording_id")?, "speaker_id": text_value(&row, "speaker_turns.speaker_id")?, "start_ms": int_value(&row, "speaker_turns.start_ms")?, "end_ms": int_value(&row, "speaker_turns.end_ms")?, "confidence": row.get("speaker_turns.confidence").cloned().unwrap_or(serde_json::Value::Null), "status": "confirmed", "model_run_id": row.get("speaker_turns.model_run_id").cloned().unwrap_or(serde_json::Value::Null), "overlap": row.get("speaker_turns.overlap").cloned().unwrap_or(serde_json::Value::Bool(false)), "revision": int_value(&row, "speaker_turns.revision")? + 1, "created_at": text_value(&row, "speaker_turns.created_at")?, "updated_at": now()})), timeline_revision_mutation(&project_id, "confirm", serde_json::json!({"turnId": turn_id}))]).map_err(AppError::Genesis)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoryClipOutput {
    id: String,
    source_turn_id: Option<String>,
    source_recording_id: String,
    source_start_ms: i64,
    source_end_ms: i64,
    timeline_start_ms: i64,
    speaker_id: String,
    effect_chain_id: Option<String>,
    revision: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoryOutput {
    id: String,
    project_id: String,
    title: String,
    duration_ms: i64,
    revision: i64,
    clips: Vec<StoryClipOutput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectNodeInput {
    id: String,
    kind: String,
    position: i64,
    parameters: serde_json::Value,
    bypassed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelPackageOutput {
    id: String,
    label: String,
    model_version: String,
    size_bytes: i64,
    runtime_location: String,
    install_state: String,
    compatibility: serde_json::Value,
    languages: serde_json::Value,
    observed_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceProfileOutput {
    id: String,
    display_name: String,
    rights_basis: String,
    rights_state: String,
    provider_available: bool,
}

fn story_clips(
    storage: &genesis_block_native::Storage,
    sequence_id: &str,
) -> AppResult<Vec<StoryClipOutput>> {
    let mut rows = crate::genesis_adapter::query(
        storage,
        "story_clips",
        &[
            "id",
            "source_turn_id",
            "source_recording_id",
            "source_start_ms",
            "source_end_ms",
            "timeline_start_ms",
            "speaker_id",
            "effect_chain_id",
            "revision",
        ],
        vec![crate::genesis_adapter::eq(
            "story_clips",
            "sequence_id",
            serde_json::json!(sequence_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?;
    rows.sort_by_key(|row| {
        (
            row.get("story_clips.timeline_start_ms")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default(),
            row.get("story_clips.id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    rows.into_iter()
        .map(|row| {
            Ok(StoryClipOutput {
                id: text_value(&row, "story_clips.id")?,
                source_turn_id: row
                    .get("story_clips.source_turn_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                source_recording_id: text_value(&row, "story_clips.source_recording_id")?,
                source_start_ms: int_value(&row, "story_clips.source_start_ms")?,
                source_end_ms: int_value(&row, "story_clips.source_end_ms")?,
                timeline_start_ms: int_value(&row, "story_clips.timeline_start_ms")?,
                speaker_id: text_value(&row, "story_clips.speaker_id")?,
                effect_chain_id: row
                    .get("story_clips.effect_chain_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                revision: int_value(&row, "story_clips.revision")?,
            })
        })
        .collect()
}

fn story_output(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> AppResult<Option<StoryOutput>> {
    let mut rows = crate::genesis_adapter::query(
        storage,
        "story_sequences",
        &[
            "id",
            "title",
            "duration_ms",
            "current_revision",
            "updated_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_sequences",
            "project_id",
            serde_json::json!(project_id),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    rows.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("story_sequences.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let Some(sequence) = rows.first() else {
        return Ok(None);
    };
    let id = text_value(sequence, "story_sequences.id")?;
    Ok(Some(StoryOutput {
        clips: story_clips(storage, &id)?,
        id,
        project_id: project_id.to_string(),
        title: text_value(sequence, "story_sequences.title")?,
        duration_ms: int_value(sequence, "story_sequences.duration_ms")?,
        revision: int_value(sequence, "story_sequences.current_revision")?,
    }))
}

fn story_revision_mutation(
    sequence_id: &str,
    operation: &str,
    before: &str,
    after: &str,
) -> genesis_block_native::RelationalRowMutation {
    crate::genesis_adapter::upsert(
        "story_revisions",
        serde_json::json!({"id": Uuid::new_v4().to_string(), "sequence_id": sequence_id, "operation": operation, "before_json": before, "after_json": after, "applied": true, "author_device_id": "mobile-local", "created_at": now()}),
    )
}

fn replacement_story_mutations(
    storage: &genesis_block_native::Storage,
    sequence_id: &str,
    snapshot: &str,
) -> AppResult<Vec<genesis_block_native::RelationalRowMutation>> {
    let clips: Vec<StoryClipOutput> = serde_json::from_str(snapshot)
        .map_err(|error| AppError::InvalidInput(format!("story snapshot is invalid: {error}")))?;
    let timestamp = now();
    let mut mutations = story_clips(storage, sequence_id)?
        .into_iter()
        .map(|clip| crate::genesis_adapter::delete("story_clips", &clip.id))
        .collect::<Vec<_>>();
    for clip in clips {
        mutations.push(crate::genesis_adapter::upsert("story_clips", serde_json::json!({"id": clip.id, "sequence_id": sequence_id, "source_turn_id": clip.source_turn_id, "source_recording_id": clip.source_recording_id, "source_start_ms": clip.source_start_ms, "source_end_ms": clip.source_end_ms, "timeline_start_ms": clip.timeline_start_ms, "speaker_id": clip.speaker_id, "effect_chain_id": clip.effect_chain_id, "revision": clip.revision, "created_at": timestamp, "updated_at": timestamp})));
    }
    Ok(mutations)
}

#[tauri::command]
pub(crate) fn mobile_story_create(
    project_id: String,
    title: String,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    if title.trim().is_empty() || title.chars().count() > 120 {
        return Err(AppError::InvalidInput(
            "story title must contain 1 to 120 characters".to_string(),
        ));
    }
    if let Some(existing) = story_output(&state.genesis, &project_id)? {
        return Ok(existing);
    }
    let mut recordings = crate::genesis_adapter::query(
        &state.genesis,
        "recordings",
        &["id", "duration_ms", "updated_at"],
        vec![crate::genesis_adapter::eq(
            "recordings",
            "project_id",
            serde_json::json!(project_id),
        )],
        100,
    )
    .map_err(AppError::Genesis)?;
    recordings.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("recordings.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let recording = recordings.first().ok_or_else(|| {
        AppError::InvalidInput("recording is required before creating a story".to_string())
    })?;
    let recording_id = text_value(recording, "recordings.id")?;
    let duration_ms = int_value(recording, "recordings.duration_ms")?;
    let sequence_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let mut mutations = vec![crate::genesis_adapter::upsert(
        "story_sequences",
        serde_json::json!({"id": sequence_id, "project_id": project_id, "title": title.trim(), "duration_ms": duration_ms, "current_revision": 1, "created_at": timestamp, "updated_at": timestamp}),
    )];
    let mut turns = crate::genesis_adapter::query(
        &state.genesis,
        "speaker_turns",
        &["id", "speaker_id", "start_ms", "end_ms"],
        vec![crate::genesis_adapter::eq(
            "speaker_turns",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?;
    turns.sort_by_key(|row| {
        row.get("speaker_turns.start_ms")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default()
    });
    for turn in turns {
        let start_ms = int_value(&turn, "speaker_turns.start_ms")?;
        mutations.push(crate::genesis_adapter::upsert("story_clips", serde_json::json!({"id": Uuid::new_v4().to_string(), "sequence_id": sequence_id, "source_turn_id": text_value(&turn, "speaker_turns.id")?, "source_recording_id": recording_id, "source_start_ms": start_ms, "source_end_ms": int_value(&turn, "speaker_turns.end_ms")?, "timeline_start_ms": start_ms, "speaker_id": text_value(&turn, "speaker_turns.speaker_id")?, "effect_chain_id": null, "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }
    crate::genesis_adapter::commit_rows(&state.genesis, mutations).map_err(AppError::Genesis)?;
    story_output(&state.genesis, &project_id)?
        .ok_or_else(|| AppError::InvalidInput("story could not be created".to_string()))
}

#[tauri::command]
pub(crate) fn mobile_story_query(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<Option<StoryOutput>> {
    story_output(&state.genesis, &project_id)
}

fn mutate_story_clip<F>(
    storage: &genesis_block_native::Storage,
    clip_id: &str,
    operation: &str,
    mutate: F,
) -> AppResult<StoryOutput>
where
    F: FnOnce(&mut StoryClipOutput) -> AppResult<()>,
{
    let clip_row = crate::genesis_adapter::query(
        storage,
        "story_clips",
        &["sequence_id", "created_at"],
        vec![crate::genesis_adapter::eq(
            "story_clips",
            "id",
            serde_json::json!(clip_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("story clip not found".to_string()))?;
    let sequence_id = text_value(&clip_row, "story_clips.sequence_id")?;
    let sequence = crate::genesis_adapter::query(
        storage,
        "story_sequences",
        &[
            "project_id",
            "title",
            "duration_ms",
            "current_revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_sequences",
            "id",
            serde_json::json!(sequence_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))?;
    let project_id = text_value(&sequence, "story_sequences.project_id")?;
    let mut clips = story_clips(storage, &sequence_id)?;
    let before =
        serde_json::to_string(&clips).map_err(|error| AppError::InvalidInput(error.to_string()))?;
    let clip = clips
        .iter_mut()
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| AppError::InvalidInput("story clip not found".to_string()))?;
    mutate(clip)?;
    clip.revision += 1;
    let changed = clip.clone();
    let after =
        serde_json::to_string(&clips).map_err(|error| AppError::InvalidInput(error.to_string()))?;
    let timestamp = now();
    let mut mutations = crate::genesis_adapter::query(
        storage,
        "story_revisions",
        &["id", "applied"],
        vec![crate::genesis_adapter::eq(
            "story_revisions",
            "sequence_id",
            serde_json::json!(sequence_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .filter(|row| {
        row.get("story_revisions.applied")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
    })
    .filter_map(|row| {
        row.get("story_revisions.id")
            .and_then(serde_json::Value::as_str)
            .map(|id| crate::genesis_adapter::delete("story_revisions", id))
    })
    .collect::<Vec<_>>();
    mutations.extend([
        crate::genesis_adapter::upsert("story_clips", serde_json::json!({"id": changed.id, "sequence_id": sequence_id, "source_turn_id": changed.source_turn_id, "source_recording_id": changed.source_recording_id, "source_start_ms": changed.source_start_ms, "source_end_ms": changed.source_end_ms, "timeline_start_ms": changed.timeline_start_ms, "speaker_id": changed.speaker_id, "effect_chain_id": changed.effect_chain_id, "revision": changed.revision, "created_at": text_value(&clip_row, "story_clips.created_at")?, "updated_at": timestamp})),
        crate::genesis_adapter::upsert("story_sequences", serde_json::json!({"id": sequence_id, "project_id": project_id, "title": text_value(&sequence, "story_sequences.title")?, "duration_ms": int_value(&sequence, "story_sequences.duration_ms")?, "current_revision": int_value(&sequence, "story_sequences.current_revision")? + 1, "created_at": text_value(&sequence, "story_sequences.created_at")?, "updated_at": timestamp})),
        story_revision_mutation(&sequence_id, operation, &before, &after),
    ]);
    crate::genesis_adapter::commit_rows(storage, mutations).map_err(AppError::Genesis)?;
    story_output(storage, &project_id)?
        .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))
}

#[tauri::command]
pub(crate) fn mobile_story_clip_move(
    clip_id: String,
    timeline_start_ms: i64,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    if timeline_start_ms < 0 {
        return Err(AppError::InvalidInput(
            "timeline start must be non-negative".to_string(),
        ));
    }
    mutate_story_clip(&state.genesis, &clip_id, "move", |clip| {
        clip.timeline_start_ms = timeline_start_ms;
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn mobile_story_clip_trim(
    clip_id: String,
    source_start_ms: i64,
    source_end_ms: i64,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    if source_start_ms < 0 || source_end_ms <= source_start_ms {
        return Err(AppError::InvalidInput("trim range is invalid".to_string()));
    }
    mutate_story_clip(&state.genesis, &clip_id, "trim", |clip| {
        let Some(turn_id) = clip.source_turn_id.as_ref() else {
            return Err(AppError::InvalidInput(
                "clip has no immutable source turn".to_string(),
            ));
        };
        let turn = crate::genesis_adapter::query(
            &state.genesis,
            "speaker_turns",
            &["start_ms", "end_ms"],
            vec![crate::genesis_adapter::eq(
                "speaker_turns",
                "id",
                serde_json::json!(turn_id),
            )],
            1,
        )
        .map_err(AppError::Genesis)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::InvalidInput("source turn not found".to_string()))?;
        if source_start_ms < int_value(&turn, "speaker_turns.start_ms")?
            || source_end_ms > int_value(&turn, "speaker_turns.end_ms")?
        {
            return Err(AppError::InvalidInput(
                "trim range must remain inside the immutable source turn".to_string(),
            ));
        }
        clip.source_start_ms = source_start_ms;
        clip.source_end_ms = source_end_ms;
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn mobile_story_clip_split(
    clip_id: String,
    split_source_ms: i64,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "story_clips",
        &[
            "sequence_id",
            "source_turn_id",
            "source_recording_id",
            "source_start_ms",
            "source_end_ms",
            "timeline_start_ms",
            "speaker_id",
            "effect_chain_id",
            "revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_clips",
            "id",
            serde_json::json!(clip_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("story clip not found".to_string()))?;
    let start_ms = int_value(&row, "story_clips.source_start_ms")?;
    let end_ms = int_value(&row, "story_clips.source_end_ms")?;
    if split_source_ms <= start_ms || split_source_ms >= end_ms {
        return Err(AppError::InvalidInput(
            "split point must be inside the clip".to_string(),
        ));
    }
    let sequence_id = text_value(&row, "story_clips.sequence_id")?;
    let sequence = crate::genesis_adapter::query(
        &state.genesis,
        "story_sequences",
        &[
            "project_id",
            "title",
            "duration_ms",
            "current_revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_sequences",
            "id",
            serde_json::json!(sequence_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))?;
    let project_id = text_value(&sequence, "story_sequences.project_id")?;
    let mut clips = story_clips(&state.genesis, &sequence_id)?;
    let before =
        serde_json::to_string(&clips).map_err(|error| AppError::InvalidInput(error.to_string()))?;
    let original = clips
        .iter_mut()
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| AppError::InvalidInput("story clip not found".to_string()))?;
    original.source_end_ms = split_source_ms;
    original.revision += 1;
    let changed = original.clone();
    let new_clip = StoryClipOutput {
        id: Uuid::new_v4().to_string(),
        source_turn_id: changed.source_turn_id.clone(),
        source_recording_id: changed.source_recording_id.clone(),
        source_start_ms: split_source_ms,
        source_end_ms: end_ms,
        timeline_start_ms: changed.timeline_start_ms + (split_source_ms - start_ms),
        speaker_id: changed.speaker_id.clone(),
        effect_chain_id: changed.effect_chain_id.clone(),
        revision: changed.revision,
    };
    clips.push(new_clip.clone());
    let after =
        serde_json::to_string(&clips).map_err(|error| AppError::InvalidInput(error.to_string()))?;
    let timestamp = now();
    crate::genesis_adapter::commit_rows(&state.genesis, vec![
        crate::genesis_adapter::upsert("story_clips", serde_json::json!({"id": changed.id, "sequence_id": sequence_id, "source_turn_id": changed.source_turn_id, "source_recording_id": changed.source_recording_id, "source_start_ms": changed.source_start_ms, "source_end_ms": changed.source_end_ms, "timeline_start_ms": changed.timeline_start_ms, "speaker_id": changed.speaker_id, "effect_chain_id": changed.effect_chain_id, "revision": changed.revision, "created_at": text_value(&row, "story_clips.created_at")?, "updated_at": timestamp})),
        crate::genesis_adapter::upsert("story_clips", serde_json::json!({"id": new_clip.id, "sequence_id": sequence_id, "source_turn_id": new_clip.source_turn_id, "source_recording_id": new_clip.source_recording_id, "source_start_ms": new_clip.source_start_ms, "source_end_ms": new_clip.source_end_ms, "timeline_start_ms": new_clip.timeline_start_ms, "speaker_id": new_clip.speaker_id, "effect_chain_id": new_clip.effect_chain_id, "revision": new_clip.revision, "created_at": timestamp, "updated_at": timestamp})),
        crate::genesis_adapter::upsert("story_sequences", serde_json::json!({"id": sequence_id, "project_id": project_id, "title": text_value(&sequence, "story_sequences.title")?, "duration_ms": int_value(&sequence, "story_sequences.duration_ms")?, "current_revision": int_value(&sequence, "story_sequences.current_revision")? + 1, "created_at": text_value(&sequence, "story_sequences.created_at")?, "updated_at": timestamp})),
        story_revision_mutation(&sequence_id, "split", &before, &after),
    ]).map_err(AppError::Genesis)?;
    story_output(&state.genesis, &project_id)?
        .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))
}

fn story_history(
    sequence_id: String,
    undo: bool,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    let sequence = crate::genesis_adapter::query(
        &state.genesis,
        "story_sequences",
        &[
            "project_id",
            "title",
            "duration_ms",
            "current_revision",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_sequences",
            "id",
            serde_json::json!(sequence_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))?;
    let project_id = text_value(&sequence, "story_sequences.project_id")?;
    let mut revisions = crate::genesis_adapter::query(
        &state.genesis,
        "story_revisions",
        &[
            "id",
            "before_json",
            "after_json",
            "applied",
            "operation",
            "author_device_id",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "story_revisions",
            "sequence_id",
            serde_json::json!(sequence_id),
        )],
        1000,
    )
    .map_err(AppError::Genesis)?;
    revisions.retain(|row| bool_value(row, "story_revisions.applied") == undo);
    revisions.sort_by_key(|row| {
        row.get("story_revisions.created_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let revision = if undo {
        revisions.last()
    } else {
        revisions.first()
    };
    let Some(revision) = revision else {
        return story_output(&state.genesis, &project_id)?
            .ok_or_else(|| AppError::InvalidInput("story not found".to_string()));
    };
    let snapshot = text_value(
        revision,
        if undo {
            "story_revisions.before_json"
        } else {
            "story_revisions.after_json"
        },
    )?;
    let mut mutations = replacement_story_mutations(&state.genesis, &sequence_id, &snapshot)?;
    let timestamp = now();
    mutations.push(crate::genesis_adapter::upsert("story_revisions", serde_json::json!({"id": text_value(revision, "story_revisions.id")?, "sequence_id": sequence_id, "operation": text_value(revision, "story_revisions.operation")?, "before_json": text_value(revision, "story_revisions.before_json")?, "after_json": text_value(revision, "story_revisions.after_json")?, "applied": !undo, "author_device_id": text_value(revision, "story_revisions.author_device_id")?, "created_at": text_value(revision, "story_revisions.created_at")?})));
    mutations.push(crate::genesis_adapter::upsert("story_sequences", serde_json::json!({"id": sequence_id, "project_id": project_id, "title": text_value(&sequence, "story_sequences.title")?, "duration_ms": int_value(&sequence, "story_sequences.duration_ms")?, "current_revision": std::cmp::max(1, int_value(&sequence, "story_sequences.current_revision")? + if undo { -1 } else { 1 }), "created_at": text_value(&sequence, "story_sequences.created_at")?, "updated_at": timestamp})));
    crate::genesis_adapter::commit_rows(&state.genesis, mutations).map_err(AppError::Genesis)?;
    story_output(&state.genesis, &project_id)?
        .ok_or_else(|| AppError::InvalidInput("story not found".to_string()))
}

#[tauri::command]
pub(crate) fn mobile_story_undo(
    sequence_id: String,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    story_history(sequence_id, true, state)
}

#[tauri::command]
pub(crate) fn mobile_story_redo(
    sequence_id: String,
    state: State<'_, AppState>,
) -> AppResult<StoryOutput> {
    story_history(sequence_id, false, state)
}

#[tauri::command]
pub(crate) fn mobile_model_packages_query(
    state: State<'_, AppState>,
) -> AppResult<Vec<ModelPackageOutput>> {
    let mut rows = crate::genesis_adapter::query(
        &state.genesis,
        "model_packages",
        &[
            "id",
            "label",
            "model_version",
            "size_bytes",
            "runtime_location",
            "install_state",
            "compatibility_json",
            "languages_json",
            "observed_at",
        ],
        vec![],
        500,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        Ok(ModelPackageOutput {
            id: text_value(&row, "model_packages.id")?,
            label: text_value(&row, "model_packages.label")?,
            model_version: text_value(&row, "model_packages.model_version")?,
            size_bytes: int_value(&row, "model_packages.size_bytes")?,
            runtime_location: text_value(&row, "model_packages.runtime_location")?,
            install_state: text_value(&row, "model_packages.install_state")?,
            compatibility: json_value(&row, "model_packages.compatibility_json"),
            languages: json_value(&row, "model_packages.languages_json"),
            observed_at: text_value(&row, "model_packages.observed_at")?,
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    rows.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(rows)
}

#[tauri::command]
pub(crate) fn mobile_refinement_review(
    proposal_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if decision != "accepted" && decision != "rejected" {
        return Err(AppError::InvalidInput(
            "decision must be accepted or rejected".to_string(),
        ));
    }
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "transcript_refinement_proposals",
        &[
            "project_id",
            "transcript_segment_id",
            "original_text",
            "proposed_text",
            "policy",
            "model_run_id",
            "status",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "transcript_refinement_proposals",
            "id",
            serde_json::json!(proposal_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("refinement proposal not found".to_string()))?;
    if text_value(&row, "transcript_refinement_proposals.status")? != "proposed" {
        return Err(AppError::InvalidInput(
            "refinement proposal was already reviewed".to_string(),
        ));
    }
    let timestamp = now();
    crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("transcript_refinement_proposals", serde_json::json!({"id": proposal_id, "project_id": text_value(&row, "transcript_refinement_proposals.project_id")?, "transcript_segment_id": row.get("transcript_refinement_proposals.transcript_segment_id").cloned().unwrap_or(serde_json::Value::Null), "original_text": text_value(&row, "transcript_refinement_proposals.original_text")?, "proposed_text": text_value(&row, "transcript_refinement_proposals.proposed_text")?, "policy": text_value(&row, "transcript_refinement_proposals.policy")?, "model_run_id": row.get("transcript_refinement_proposals.model_run_id").cloned().unwrap_or(serde_json::Value::Null), "status": decision, "reviewed_at": timestamp, "created_at": text_value(&row, "transcript_refinement_proposals.created_at")?, "updated_at": timestamp}))]).map_err(AppError::Genesis)?;
    Ok(())
}

fn effect_parameter_valid(kind: &str, parameters: &serde_json::Value) -> bool {
    let number = |key: &str| parameters.get(key).and_then(|value| value.as_f64());
    match kind {
        "pitch_shift" => number("semitones").is_some_and(|value| (-12.0..=12.0).contains(&value)),
        "reverb" => number("room").is_some_and(|value| (0.0..=1.0).contains(&value)),
        "delay" => number("milliseconds").is_some_and(|value| (0.0..=2000.0).contains(&value)),
        "compressor" => number("threshold").is_some_and(|value| (-60.0..=0.0).contains(&value)),
        "low_pass" => number("hertz").is_some_and(|value| (80.0..=20000.0).contains(&value)),
        _ => false,
    }
}

#[tauri::command]
pub(crate) fn mobile_effect_chain_update(
    project_id: String,
    owner_kind: String,
    owner_id: String,
    label: String,
    nodes: Vec<EffectNodeInput>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    if !matches!(
        owner_kind.as_str(),
        "project" | "story_clip" | "voice_profile"
    ) || nodes
        .iter()
        .any(|node| !effect_parameter_valid(&node.kind, &node.parameters))
    {
        return Err(AppError::InvalidInput(
            "effect chain contains an invalid owner or parameter".to_string(),
        ));
    }
    let existing = crate::genesis_adapter::query(
        &state.genesis,
        "effect_chains",
        &["id", "created_at"],
        vec![
            crate::genesis_adapter::eq(
                "effect_chains",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "effect_chains",
                "owner_kind",
                serde_json::json!(owner_kind),
            ),
            crate::genesis_adapter::eq("effect_chains", "owner_id", serde_json::json!(owner_id)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next();
    let chain_id = existing
        .as_ref()
        .and_then(|row| row.get("effect_chains.id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let created_at = existing
        .as_ref()
        .and_then(|row| row.get("effect_chains.created_at"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&timestamp)
        .to_string();
    let mut mutations = crate::genesis_adapter::query(
        &state.genesis,
        "effect_nodes",
        &["id"],
        vec![crate::genesis_adapter::eq(
            "effect_nodes",
            "chain_id",
            serde_json::json!(chain_id),
        )],
        100,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .filter_map(|row| {
        row.get("effect_nodes.id")
            .and_then(serde_json::Value::as_str)
            .map(|id| crate::genesis_adapter::delete("effect_nodes", id))
    })
    .collect::<Vec<_>>();
    mutations.push(crate::genesis_adapter::upsert("effect_chains", serde_json::json!({"id": chain_id, "project_id": project_id, "owner_kind": owner_kind, "owner_id": owner_id, "label": label, "bypassed": false, "created_at": created_at, "updated_at": timestamp})));
    for node in nodes {
        mutations.push(crate::genesis_adapter::upsert("effect_nodes", serde_json::json!({"id": node.id, "chain_id": chain_id, "position": node.position, "kind": node.kind, "parameters_json": node.parameters, "bypassed": node.bypassed, "created_at": timestamp, "updated_at": timestamp})));
    }
    crate::genesis_adapter::commit_rows(&state.genesis, mutations).map_err(AppError::Genesis)?;
    Ok(chain_id)
}

#[tauri::command]
pub(crate) fn mobile_voice_profiles_query(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<VoiceProfileOutput>> {
    let mut rows = crate::genesis_adapter::query(
        &state.genesis,
        "voice_profiles",
        &[
            "id",
            "display_name",
            "rights_basis",
            "rights_state",
            "provider_id",
        ],
        vec![crate::genesis_adapter::eq(
            "voice_profiles",
            "project_id",
            serde_json::json!(project_id),
        )],
        500,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        Ok(VoiceProfileOutput {
            id: text_value(&row, "voice_profiles.id")?,
            display_name: text_value(&row, "voice_profiles.display_name")?,
            rights_basis: text_value(&row, "voice_profiles.rights_basis")?,
            rights_state: text_value(&row, "voice_profiles.rights_state")?,
            provider_available: row
                .get("voice_profiles.provider_id")
                .is_some_and(|value| !value.is_null()),
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    rows.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(rows)
}

#[tauri::command]
pub(crate) fn mobile_agent_voice_grant_set(
    project_id: String,
    mcp_client_id: String,
    voice_profile_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<bool> {
    let profile = crate::genesis_adapter::query(
        &state.genesis,
        "voice_profiles",
        &["rights_state"],
        vec![
            crate::genesis_adapter::eq("voice_profiles", "id", serde_json::json!(voice_profile_id)),
            crate::genesis_adapter::eq(
                "voice_profiles",
                "project_id",
                serde_json::json!(project_id),
            ),
        ],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("voice profile not found".to_string()))?;
    let rights = text_value(&profile, "voice_profiles.rights_state")?;
    if enabled && rights != "valid" {
        return Err(AppError::InvalidInput(
            "voice rights are not valid".to_string(),
        ));
    }
    let timestamp = now();
    let existing = crate::genesis_adapter::query(
        &state.genesis,
        "agent_voice_grants",
        &["id", "granted_at", "expires_at"],
        vec![
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "mcp_client_id",
                serde_json::json!(mcp_client_id),
            ),
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "voice_profile_id",
                serde_json::json!(voice_profile_id),
            ),
        ],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next();
    let id = existing
        .as_ref()
        .and_then(|row| row.get("agent_voice_grants.id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("agent_voice_grants", serde_json::json!({"id": id, "project_id": project_id, "mcp_client_id": mcp_client_id, "voice_profile_id": voice_profile_id, "capability": "voice.speak", "granted_at": if enabled { timestamp.clone() } else { existing.as_ref().and_then(|row| row.get("agent_voice_grants.granted_at")).and_then(serde_json::Value::as_str).unwrap_or(&timestamp).to_string() }, "expires_at": existing.as_ref().and_then(|row| row.get("agent_voice_grants.expires_at")).cloned().unwrap_or(serde_json::Value::Null), "revoked_at": if enabled { serde_json::Value::Null } else { serde_json::Value::String(timestamp) }}))]).map_err(AppError::Genesis)?;
    Ok(enabled)
}

#[tauri::command]
pub(crate) fn mobile_agent_voice_stop(
    session_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let row = crate::genesis_adapter::query(
        &state.genesis,
        "agent_voice_sessions",
        &[
            "project_id",
            "mcp_client_id",
            "voice_profile_id",
            "grant_id",
            "requested_text_hash",
            "state",
            "retain_output",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "agent_voice_sessions",
            "id",
            serde_json::json!(session_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("voice session not found".to_string()))?;
    let current = text_value(&row, "agent_voice_sessions.state")?;
    if ["queued", "speaking", "muted"].contains(&current.as_str()) {
        crate::genesis_adapter::commit_rows(&state.genesis, vec![crate::genesis_adapter::upsert("agent_voice_sessions", serde_json::json!({"id": session_id, "project_id": text_value(&row, "agent_voice_sessions.project_id")?, "mcp_client_id": text_value(&row, "agent_voice_sessions.mcp_client_id")?, "voice_profile_id": text_value(&row, "agent_voice_sessions.voice_profile_id")?, "grant_id": text_value(&row, "agent_voice_sessions.grant_id")?, "requested_text_hash": text_value(&row, "agent_voice_sessions.requested_text_hash")?, "state": "stopped", "retain_output": bool_value(&row, "agent_voice_sessions.retain_output"), "stop_actor": "mobile-user", "created_at": text_value(&row, "agent_voice_sessions.created_at")?, "updated_at": now()}))]).map_err(AppError::Genesis)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn mobile_mcp_set_enabled(
    enabled: bool,
    expose_lan: bool,
    state: State<'_, AppState>,
) -> AppResult<McpStateOutput> {
    let mut gateway = state
        .mobile_gateway
        .lock()
        .expect("mobile gateway mutex poisoned");
    if !enabled {
        if let Some(control) = gateway.take() {
            control.stop.store(true, Ordering::SeqCst);
        }
        return Ok(McpStateOutput {
            enabled: false,
            bind: None,
            access_token: None,
        });
    }
    if let Some(control) = gateway.as_ref() {
        return Ok(McpStateOutput {
            enabled: true,
            bind: Some(control.bind.clone()),
            access_token: None,
        });
    }
    let listener = TcpListener::bind(if expose_lan {
        "0.0.0.0:0"
    } else {
        "127.0.0.1:0"
    })?;
    listener.set_nonblocking(true)?;
    let bind = listener.local_addr()?.to_string();
    let access_token = Uuid::new_v4().to_string();
    let worker_token = access_token.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let storage = state.genesis.clone();
    thread::spawn(move || {
        while !worker_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => handle_gateway_stream(stream, &storage, &worker_token),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(40))
                }
                Err(_) => break,
            }
        }
    });
    *gateway = Some(MobileGatewayControl {
        bind: bind.clone(),
        stop,
    });
    Ok(McpStateOutput {
        enabled: true,
        bind: Some(bind),
        access_token: Some(access_token),
    })
}

fn handle_gateway_stream(
    mut stream: TcpStream,
    storage: &genesis_block_native::Storage,
    access_token: &str,
) {
    let mut buffer = vec![0_u8; 64 * 1024];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let raw = String::from_utf8_lossy(&buffer[..read]);
    let authorized = raw.lines().any(|line| {
        line.trim()
            .eq_ignore_ascii_case(&format!("Authorization: Bearer {access_token}"))
    });
    if !authorized {
        write_json(
            &mut stream,
            "401 Unauthorized",
            &serde_json::json!({"error":"AUTH_REQUIRED"}),
        );
        return;
    }
    let first = raw.lines().next().unwrap_or_default();
    if first.starts_with("GET /health ") {
        write_json(
            &mut stream,
            "200 OK",
            &serde_json::json!({"status":"ok","runtime":"mobile","mcp":true}),
        );
        return;
    }
    if first.starts_with("POST /mcp ") {
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("{}");
        let request: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
        let method = request
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let result = match method {
            "initialize" => {
                serde_json::json!({"protocolVersion":"2025-06-18","serverInfo":{"name":"fung-mobile","version":env!("CARGO_PKG_VERSION")},"capabilities":{"tools":{}}})
            }
            "tools/list" => serde_json::json!({"tools":[
                {"name":"fung.projects.list","description":"List local projects","inputSchema":{"type":"object","properties":{}}},
                {"name":"fung.notes.search","description":"Search local note titles","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}},
                {"name":"fung.capture.status","description":"Read active capture state","inputSchema":{"type":"object","properties":{}}},
                {"name":"fung.voice.speak","description":"Queue speech with an explicitly granted, rights-valid voice profile","inputSchema":{"type":"object","properties":{"projectId":{"type":"string"},"voiceProfileId":{"type":"string"},"text":{"type":"string","maxLength":2000},"retainOutput":{"type":"boolean","default":false}},"required":["projectId","voiceProfileId","text"]}}
            ]}),
            "tools/call" => {
                gateway_tool_call(storage, request.get("params").cloned().unwrap_or_default())
            }
            _ => {
                serde_json::json!({"isError":true,"content":[{"type":"text","text":"Method not found"}]})
            }
        };
        write_json(
            &mut stream,
            "200 OK",
            &serde_json::json!({"jsonrpc":"2.0","id":id,"result":result}),
        );
        return;
    }
    write_json(
        &mut stream,
        "404 Not Found",
        &serde_json::json!({"error":"NOT_FOUND"}),
    );
}

fn gateway_tool_call(
    storage: &genesis_block_native::Storage,
    params_value: serde_json::Value,
) -> serde_json::Value {
    let name = params_value
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let arguments = params_value.get("arguments").cloned().unwrap_or_default();
    let payload = match name {
        "fung.projects.list" => {
            let mut rows = match crate::genesis_adapter::query(
                storage,
                "projects",
                &["id", "name", "updated_at"],
                vec![],
                100,
            ) {
                Ok(rows) => rows,
                Err(error) => return tool_error(error),
            };
            rows.sort_by_key(|row| {
                std::cmp::Reverse(
                    row.get("projects.updated_at")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                )
            });
            serde_json::Value::Array(rows.into_iter().filter_map(|row| Some(serde_json::json!({"id":row.get("projects.id")?.as_str()?,"name":row.get("projects.name")?.as_str()?,"updatedAt":row.get("projects.updated_at")?.as_str()?}))).collect())
        }
        "fung.notes.search" => {
            let query = arguments
                .get("query")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let mut rows = match crate::genesis_adapter::query(
                storage,
                "notes",
                &["id", "title", "updated_at"],
                vec![],
                1000,
            ) {
                Ok(rows) => rows,
                Err(error) => return tool_error(error),
            };
            rows.retain(|row| {
                row.get("notes.title")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|title| title.to_lowercase().contains(&query.to_lowercase()))
            });
            rows.sort_by_key(|row| {
                std::cmp::Reverse(
                    row.get("notes.updated_at")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                )
            });
            rows.truncate(50);
            serde_json::Value::Array(rows.into_iter().filter_map(|row| Some(serde_json::json!({"id":row.get("notes.id")?.as_str()?,"title":row.get("notes.title")?.as_str()?,"updatedAt":row.get("notes.updated_at")?.as_str()?}))).collect())
        }
        "fung.capture.status" => {
            let mut rows = match crate::genesis_adapter::query(
                storage,
                "recordings",
                &["id", "project_id", "status", "duration_ms", "updated_at"],
                vec![],
                1000,
            ) {
                Ok(rows) => rows,
                Err(error) => return tool_error(error),
            };
            rows.retain(|row| {
                matches!(
                    row.get("recordings.status")
                        .and_then(serde_json::Value::as_str),
                    Some("recording" | "paused")
                )
            });
            rows.sort_by_key(|row| {
                std::cmp::Reverse(
                    row.get("recordings.updated_at")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                )
            });
            rows.first().map(|row| serde_json::json!({"recordingId":row.get("recordings.id"),"projectId":row.get("recordings.project_id"),"state":row.get("recordings.status"),"safeOffsetMs":row.get("recordings.duration_ms")})).unwrap_or(serde_json::Value::Null)
        }
        "fung.voice.speak" => match queue_agent_voice(storage, &arguments) {
            Ok(value) => value,
            Err(error) => return tool_error(error),
        },
        _ => return tool_error("Tool is not granted or does not exist".to_string()),
    };
    serde_json::json!({"content":[{"type":"text","text":payload.to_string()}],"structuredContent":payload})
}

fn queue_agent_voice(
    storage: &genesis_block_native::Storage,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    const MCP_CLIENT_ID: &str = "fung-mobile-mcp";
    let project_id = arguments
        .get("projectId")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    let profile_id = arguments
        .get("voiceProfileId")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    let text = arguments
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    let retain_output = arguments
        .get("retainOutput")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if project_id.is_empty()
        || profile_id.is_empty()
        || text.is_empty()
        || text.chars().count() > 2000
    {
        return Err(
            "projectId, voiceProfileId, and 1..2000 characters of text are required".to_string(),
        );
    }
    let timestamp = now();
    let grant = crate::genesis_adapter::query(
        storage,
        "agent_voice_grants",
        &["id", "capability", "expires_at", "revoked_at"],
        vec![
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "project_id",
                serde_json::json!(project_id),
            ),
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "mcp_client_id",
                serde_json::json!(MCP_CLIENT_ID),
            ),
            crate::genesis_adapter::eq(
                "agent_voice_grants",
                "voice_profile_id",
                serde_json::json!(profile_id),
            ),
        ],
        1,
    )?
    .into_iter()
    .next();
    let profile = crate::genesis_adapter::query(
        storage,
        "voice_profiles",
        &["rights_state", "provider_id"],
        vec![
            crate::genesis_adapter::eq("voice_profiles", "id", serde_json::json!(profile_id)),
            crate::genesis_adapter::eq(
                "voice_profiles",
                "project_id",
                serde_json::json!(project_id),
            ),
        ],
        1,
    )?
    .into_iter()
    .next();
    let valid_grant = grant.as_ref().is_some_and(|row| {
        row.get("agent_voice_grants.capability")
            .and_then(serde_json::Value::as_str)
            == Some("voice.speak")
            && row
                .get("agent_voice_grants.revoked_at")
                .is_none_or(serde_json::Value::is_null)
            && row
                .get("agent_voice_grants.expires_at")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|expiry| expiry > timestamp.as_str())
    });
    let valid_profile = profile.as_ref().is_some_and(|row| {
        row.get("voice_profiles.rights_state")
            .and_then(serde_json::Value::as_str)
            == Some("valid")
            && row
                .get("voice_profiles.provider_id")
                .is_some_and(|value| !value.is_null())
    });
    if !valid_grant || !valid_profile {
        return Err(
            "VOICE_NOT_GRANTED: a valid rights record, provider, and active MCP grant are required"
                .to_string(),
        );
    }
    let grant_id = grant
        .as_ref()
        .and_then(|row| row.get("agent_voice_grants.id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut devices = crate::genesis_adapter::query(
        storage,
        "paired_devices",
        &["id", "updated_at"],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "trust_state",
            serde_json::json!("paired"),
        )],
        100,
    )?;
    devices.sort_by_key(|row| {
        std::cmp::Reverse(
            row.get("paired_devices.updated_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    let executor = devices
        .first()
        .and_then(|row| row.get("paired_devices.id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let Some(executor_id) = executor else {
        return Err("VOICE_PROVIDER_UNAVAILABLE: pair FUNG Desktop first".to_string());
    };
    let session_id = Uuid::new_v4().to_string();
    let job_id = Uuid::new_v4().to_string();
    let text_hash = sha256(text.as_bytes());
    crate::genesis_adapter::commit_rows(
        storage,
        vec![
            crate::genesis_adapter::upsert(
                "agent_voice_sessions",
                serde_json::json!({"id": session_id, "project_id": project_id, "mcp_client_id": MCP_CLIENT_ID, "voice_profile_id": profile_id, "grant_id": grant_id, "requested_text_hash": text_hash, "state": "queued", "retain_output": retain_output, "stop_actor": null, "created_at": timestamp, "updated_at": timestamp}),
            ),
            crate::genesis_adapter::upsert(
                "delegated_jobs",
                serde_json::json!({"id": job_id, "project_id": project_id, "executor_device_id": executor_id, "operation": "voice.synthesize", "state": "queued", "progress": 0, "input_manifest_hash": text_hash, "checkpoint_json": {"sessionId": session_id, "voiceProfileId": profile_id, "retainOutput": retain_output}, "observed_at": timestamp, "created_at": timestamp, "updated_at": timestamp}),
            ),
        ],
    )?;
    Ok(
        serde_json::json!({"sessionId":session_id,"jobId":job_id,"state":"queued","retainOutput":retain_output}),
    )
}

fn tool_error(message: String) -> serde_json::Value {
    serde_json::json!({"isError":true,"content":[{"type":"text","text":message}]})
}

fn write_json(stream: &mut TcpStream, status: &str, value: &serde_json::Value) {
    let body = value.to_string();
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};

    fn open_genesis() -> (std::path::PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-mobile-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .expect("open GenesisBlockDB");
        crate::genesis_adapter::install(&storage).expect("install schema");
        (path, storage)
    }

    #[test]
    fn voice_parser_requires_confirmation_for_delete() {
        let intent = parse_voice_intent("ลบโน้ตนี้");
        assert_eq!(intent.intent, "entity.delete");
        assert!(intent.destructive);
        assert!(intent.requires_confirmation);
    }

    /// Mirror database seeded with the parent tables `init_schema` references.
    fn mirror_db() -> Connection {
        let db = Connection::open_in_memory().expect("memory database");
        db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE projects(id TEXT PRIMARY KEY, name TEXT, storage_path TEXT, active_recording_id TEXT, archived_at TEXT, created_at TEXT, updated_at TEXT); CREATE TABLE recordings(id TEXT PRIMARY KEY, project_id TEXT, status TEXT, duration_ms INTEGER);").expect("parent schema");
        init_schema(&db).expect("mobile schema");
        db.execute(
            "INSERT INTO projects(id, name, created_at, updated_at) VALUES ('p', 'Project', 't', 't')",
            [],
        )
        .expect("seed project");
        db
    }

    fn insert_node(db: &Connection, id: &str, entity_type: &str) -> rusqlite::Result<usize> {
        db.execute(
            "INSERT INTO graph_nodes(id, project_id, entity_type, entity_id, label, created_at, updated_at) VALUES (?1, 'p', ?2, ?1, 'label', 't', 't')",
            rusqlite::params![id, entity_type],
        )
    }

    fn insert_edge(db: &Connection, id: &str, status: &str) -> rusqlite::Result<usize> {
        db.execute(
            "INSERT INTO graph_edges(id, project_id, source_node_id, target_node_id, predicate, epistemic_status, created_at, updated_at) VALUES (?1, 'p', 'src', 'dst', 'extracted_from', ?2, 't', 't')",
            rusqlite::params![id, status],
        )
    }

    /// The mirror must accept every `epistemic_status` the primary store can
    /// hold, including `ai_proposed` from graph_build.rs — a stricter CHECK
    /// here would make those rows unsyncable.
    #[test]
    fn graph_schema_preserves_epistemic_status() {
        let db = mirror_db();
        insert_node(&db, "src", "meeting").expect("source node");
        insert_node(&db, "dst", "project").expect("target node");
        for (index, status) in [
            "confirmed",
            "inferred",
            "evidence",
            "superseded",
            "disputed",
            "ai_proposed",
        ]
        .into_iter()
        .enumerate()
        {
            insert_edge(&db, &format!("e{index}"), status)
                .unwrap_or_else(|error| panic!("{status} must be storable: {error}"));
        }
        assert!(
            insert_edge(&db, "bogus", "hallucinated").is_err(),
            "the CHECK must still reject unknown statuses"
        );
    }

    /// Same contract for `entity_type`: graph_build.rs writes the structural
    /// (meeting/speaker) and extracted (topic/decision/action_item/mention)
    /// kinds alongside the original four.
    #[test]
    fn graph_schema_preserves_entity_types() {
        let db = mirror_db();
        for (index, entity_type) in [
            "note",
            "project",
            "recording",
            "person",
            "meeting",
            "speaker",
            "topic",
            "decision",
            "action_item",
            "mention",
        ]
        .into_iter()
        .enumerate()
        {
            insert_node(&db, &format!("n{index}"), entity_type)
                .unwrap_or_else(|error| panic!("{entity_type} must be storable: {error}"));
        }
        assert!(
            insert_node(&db, "bogus", "wormhole").is_err(),
            "the CHECK must still reject unknown entity types"
        );
    }

    /// The mirror stores `ai_proposed`, but the client-facing write command
    /// must not let a mobile device forge AI provenance. Widening the schema
    /// CHECK must not silently widen this.
    #[test]
    fn relation_upsert_rejects_ai_proposed_from_clients() {
        assert!(!CLIENT_WRITABLE_EPISTEMIC_STATUS.contains(&"ai_proposed"));
        assert!(CLIENT_WRITABLE_EPISTEMIC_STATUS.contains(&"confirmed"));
        assert!(CLIENT_WRITABLE_EPISTEMIC_STATUS.contains(&"disputed"));
    }

    #[test]
    fn timeline_query_keeps_anonymous_turns_independent_from_transcript() {
        let (path, storage) = open_genesis();
        let mut mutations =
            crate::genesis_adapter::ensure_project_mutations("p", "projects/p", "t");
        mutations.extend([
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r","project_id":"p","source":"microphone","input_path":null,"canonical_audio_path":"r/manifest.json","status":"completed","duration_ms":90000,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("speakers", serde_json::json!({"id":"s","project_id":"p","key":"speaker-1","display_name":"ผู้พูด 1","confidence":0.86,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("speaker_turns", serde_json::json!({"id":"turn","project_id":"p","recording_id":"r","speaker_id":"s","start_ms":1000,"end_ms":5000,"confidence":0.86,"status":"proposed","model_run_id":null,"overlap":false,"revision":1,"created_at":"t","updated_at":"t"})),
        ]);
        crate::genesis_adapter::commit_rows(&storage, mutations).expect("seed timeline");
        let output = timeline_output(&storage, "p").expect("timeline output");
        assert_eq!(output.recording_id.as_deref(), Some("r"));
        assert_eq!(output.speakers.len(), 1);
        assert_eq!(output.turns.len(), 1);
        assert_eq!(output.turns[0].status, "proposed");
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_upserts_verified_row() {
        let (path, storage) = open_genesis();
        pairing_complete_inner(
            &storage,
            "cloud-dev-1",
            "FUNG Desktop",
            "192.168.1.20:8765",
            "sess-uuid-1",
            None,
        )
        .expect("pairing complete");
        let rows = crate::genesis_adapter::query(
            &storage,
            "paired_devices",
            &[
                "id",
                "name",
                "endpoint",
                "trust_state",
                "pairing_proof_hash",
                "capabilities_json",
            ],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                serde_json::json!("cloud-dev-1"),
            )],
            1,
        )
        .expect("query paired_devices");
        let row = rows.first().expect("paired_devices row must exist");
        assert_eq!(
            row.get("paired_devices.id")
                .and_then(serde_json::Value::as_str),
            Some("cloud-dev-1")
        );
        assert_eq!(
            row.get("paired_devices.name")
                .and_then(serde_json::Value::as_str),
            Some("FUNG Desktop")
        );
        assert_eq!(
            row.get("paired_devices.endpoint")
                .and_then(serde_json::Value::as_str),
            Some("192.168.1.20:8765")
        );
        assert_eq!(
            row.get("paired_devices.trust_state")
                .and_then(serde_json::Value::as_str),
            Some("paired")
        );
        assert_eq!(
            row.get("paired_devices.pairing_proof_hash")
                .and_then(serde_json::Value::as_str),
            Some("sess-uuid-1")
        );
        assert_eq!(
            row.get("paired_devices.capabilities_json").cloned(),
            Some(serde_json::json!([]))
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_stores_peer_public_key() {
        let (path, storage) = open_genesis();
        pairing_complete_inner(
            &storage,
            "cloud-dev-1b",
            "FUNG Desktop",
            "192.168.1.20:8765",
            "sess-uuid-1b",
            Some("cGVlci1wdWJsaWMta2V5LWJhc2U2NA=="),
        )
        .expect("pairing complete with public key");
        let rows = crate::genesis_adapter::query(
            &storage,
            "paired_devices",
            &["id", "public_key"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                serde_json::json!("cloud-dev-1b"),
            )],
            1,
        )
        .expect("query paired_devices");
        let row = rows.first().expect("paired_devices row must exist");
        assert_eq!(
            row.get("paired_devices.public_key")
                .and_then(serde_json::Value::as_str),
            Some("cGVlci1wdWJsaWMta2V5LWJhc2U2NA==")
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_allows_empty_endpoint() {
        let (path, storage) = open_genesis();
        pairing_complete_inner(
            &storage,
            "cloud-dev-2",
            "FUNG Desktop 2",
            "",
            "sess-uuid-2",
            None,
        )
        .expect("pairing complete with empty endpoint");
        let rows = crate::genesis_adapter::query(
            &storage,
            "paired_devices",
            &["id", "endpoint"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                serde_json::json!("cloud-dev-2"),
            )],
            1,
        )
        .expect("query paired_devices");
        let row = rows.first().expect("paired_devices row must exist");
        assert_eq!(
            row.get("paired_devices.endpoint")
                .and_then(serde_json::Value::as_str),
            Some("")
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_rejects_blank_peer_device_id() {
        let (path, storage) = open_genesis();
        let error = pairing_complete_inner(
            &storage,
            "  ",
            "FUNG Desktop",
            "192.168.1.20:8765",
            "sess-uuid-3",
            None,
        )
        .expect_err("blank peer device id must be rejected");
        assert!(matches!(error, AppError::InvalidInput(_)));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_rejects_blank_pairing_session_id() {
        let (path, storage) = open_genesis();
        let error = pairing_complete_inner(
            &storage,
            "cloud-dev-3",
            "FUNG Desktop",
            "192.168.1.20:8765",
            "",
            None,
        )
        .expect_err("blank pairing session id must be rejected");
        assert!(matches!(error, AppError::InvalidInput(_)));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_rejects_invalid_name_length() {
        let (path, storage) = open_genesis();
        let too_long = "x".repeat(121);
        let empty_name_error = pairing_complete_inner(
            &storage,
            "cloud-dev-4",
            "  ",
            "192.168.1.20:8765",
            "sess-uuid-4",
            None,
        )
        .expect_err("empty name must be rejected");
        assert!(matches!(empty_name_error, AppError::InvalidInput(_)));
        let too_long_error = pairing_complete_inner(
            &storage,
            "cloud-dev-4",
            &too_long,
            "192.168.1.20:8765",
            "sess-uuid-4",
            None,
        )
        .expect_err("121-char name must be rejected");
        assert!(matches!(too_long_error, AppError::InvalidInput(_)));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn pairing_complete_preserves_created_at_on_repair() {
        let (path, storage) = open_genesis();
        pairing_complete_inner(
            &storage,
            "cloud-dev-5",
            "FUNG Desktop",
            "192.168.1.20:8765",
            "sess-uuid-5a",
            None,
        )
        .expect("first pairing");
        let first = crate::genesis_adapter::query(
            &storage,
            "paired_devices",
            &["created_at"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                serde_json::json!("cloud-dev-5"),
            )],
            1,
        )
        .expect("query first")
        .into_iter()
        .next()
        .and_then(|row| {
            row.get("paired_devices.created_at")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .expect("first created_at");
        pairing_complete_inner(
            &storage,
            "cloud-dev-5",
            "FUNG Desktop",
            "192.168.1.20:9999",
            "sess-uuid-5b",
            None,
        )
        .expect("re-pairing");
        let rows = crate::genesis_adapter::query(
            &storage,
            "paired_devices",
            &["created_at", "endpoint", "pairing_proof_hash"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                serde_json::json!("cloud-dev-5"),
            )],
            1,
        )
        .expect("query second");
        let row = rows.first().expect("row still present");
        assert_eq!(
            row.get("paired_devices.created_at")
                .and_then(serde_json::Value::as_str),
            Some(first.as_str())
        );
        assert_eq!(
            row.get("paired_devices.endpoint")
                .and_then(serde_json::Value::as_str),
            Some("192.168.1.20:9999")
        );
        assert_eq!(
            row.get("paired_devices.pairing_proof_hash")
                .and_then(serde_json::Value::as_str),
            Some("sess-uuid-5b")
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn native_segment_path_rejects_parent_escape() {
        assert!(native_segment_file_name("native-recordings/id/segment-000001.m4a").is_ok());
        assert!(native_segment_file_name("../fung.db").is_err());
    }

    #[test]
    fn native_segment_contract_uses_camel_case() {
        let segment: NativeSegment = serde_json::from_value(serde_json::json!({
            "sequence": 1,
            "relativePath": "native-recordings/id/segment-000001.m4a",
            "durationMs": 5000,
            "byteSize": 42,
            "checksum": "abc"
        }))
        .expect("native segment contract");
        assert_eq!(segment.sequence, 1);
        assert_eq!(segment.duration_ms, 5000);
    }

    #[test]
    fn effect_parameters_are_bounded() {
        assert!(effect_parameter_valid(
            "reverb",
            &serde_json::json!({"room": 0.7})
        ));
        assert!(!effect_parameter_valid(
            "reverb",
            &serde_json::json!({"room": 1.2})
        ));
        assert!(effect_parameter_valid(
            "pitch_shift",
            &serde_json::json!({"semitones": -12})
        ));
        assert!(!effect_parameter_valid(
            "low_pass",
            &serde_json::json!({"hertz": 40})
        ));
        assert!(!effect_parameter_valid("unknown", &serde_json::json!({})));
    }

    #[test]
    fn agent_voice_tool_is_default_deny() {
        let (path, storage) = open_genesis();
        let result = queue_agent_voice(
            &storage,
            &serde_json::json!({
                "projectId": "project-1", "voiceProfileId": "voice-1", "text": "hello"
            }),
        )
        .expect_err("an absent grant must be denied");
        assert!(result.contains("VOICE_NOT_GRANTED"));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}
