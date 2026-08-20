//! Knowledge-graph builder: deterministic structural layer plus best-effort
//! LLM extraction (Topic/Decision/ActionItem/Mention) with evidence links to
//! transcript segments, persisted via genesis_adapter.

use crate::{genesis_adapter, now};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The storage engine hard-caps every relational query at 1000 rows and
/// offers no offset/cursor, so a project or recording whose rows exceed this
/// ceiling loses visibility past the first page. Documented at each call site
/// below rather than silently working around it.
const QUERY_ROW_CEILING: u32 = crate::genesis_adapter::ROW_CAP;

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ExtractedItem {
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) owner: Option<String>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) evidence: Vec<usize>,
    #[serde(default)]
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Extraction {
    #[serde(default)]
    pub(crate) topics: Vec<ExtractedItem>,
    #[serde(default)]
    pub(crate) decisions: Vec<ExtractedItem>,
    #[serde(default)]
    pub(crate) action_items: Vec<ExtractedItem>,
    #[serde(default)]
    pub(crate) mentions: Vec<ExtractedItem>,
}

pub(crate) fn parse_extraction(raw: &str) -> Result<Extraction, String> {
    serde_json::from_str(raw).map_err(|e| format!("extraction parse failed: {e}"))
}

pub(crate) fn det_node_id(recording_id: &str, kind: &str, label: &str) -> String {
    let digest = Sha256::digest(format!("{kind}\u{1}{}", label.trim().to_lowercase()).as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("gx:{recording_id}:{hex}")
}

/// Ids of previously-extracted rows for this recording (prefix match done in
/// Rust because genesis filters are equality-only). `column` is e.g.
/// "graph_nodes.id"; `prefix` is `gx:{recording_id}:` or `gxe:{recording_id}:`.
pub(crate) fn stale_extraction_ids(
    rows: &[serde_json::Value],
    column: &str,
    prefix: &str,
) -> Vec<String> {
    rows.iter()
        .filter_map(|row| row.get(column).and_then(serde_json::Value::as_str))
        .filter(|id| id.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The project's own graph node backs the FK that `part_of` edges target.
/// Projects created outside `create_project` may not have one, so make sure it
/// exists before committing any structural edge.
fn ensure_project_node(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    timestamp: &str,
) -> Result<(), String> {
    let existing = genesis_adapter::query(
        storage,
        "graph_nodes",
        &["id"],
        vec![genesis_adapter::eq(
            "graph_nodes",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )?;
    if !existing.is_empty() {
        return Ok(());
    }
    let label = genesis_adapter::query(
        storage,
        "projects",
        &["name"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .map(|row| genesis_adapter::string(&row, "projects.name"))
    .transpose()?
    .unwrap_or_else(|| "Project".to_string());
    genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "graph_nodes",
            serde_json::json!({
                "id": project_id,
                "project_id": project_id,
                "entity_type": "project",
                "entity_id": project_id,
                "label": label,
                "position_x": 50.0,
                "position_y": 17.0,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        )],
    )
}

/// Structural layer: meeting node, speaker nodes, spoke_in + part_of edges.
pub(crate) fn structural_mutations(
    project_id: &str,
    recording_id: &str,
    meeting_label: &str,
    speakers: &[(String, String)], // (speaker_id, display_name)
    timestamp: &str,
) -> Vec<genesis_block_native::RelationalRowMutation> {
    let meeting_node = format!("meeting:{recording_id}");
    let system_provenance = "{\"actor\":\"system\"}";
    let mut mutations = vec![
        genesis_adapter::upsert(
            "graph_nodes",
            serde_json::json!({"id": meeting_node, "project_id": project_id, "entity_type": "meeting", "entity_id": recording_id, "label": meeting_label, "position_x": 50.0, "position_y": 50.0, "created_at": timestamp, "updated_at": timestamp}),
        ),
        genesis_adapter::upsert(
            "graph_edges",
            serde_json::json!({"id": format!("edge:{meeting_node}:part_of"), "project_id": project_id, "source_node_id": meeting_node, "target_node_id": project_id, "predicate": "part_of", "epistemic_status": "confirmed", "provenance_json": system_provenance, "created_at": timestamp, "updated_at": timestamp}),
        ),
    ];
    for (speaker_id, display_name) in speakers {
        let speaker_node = format!("speaker:{speaker_id}");
        mutations.push(genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": speaker_node, "project_id": project_id, "entity_type": "speaker", "entity_id": speaker_id, "label": display_name, "position_x": 30.0, "position_y": 70.0, "created_at": timestamp, "updated_at": timestamp})));
        mutations.push(genesis_adapter::upsert("graph_edges", serde_json::json!({"id": format!("edge:{speaker_node}:spoke_in:{recording_id}"), "project_id": project_id, "source_node_id": speaker_node, "target_node_id": meeting_node, "predicate": "spoke_in", "epistemic_status": "confirmed", "provenance_json": system_provenance, "created_at": timestamp, "updated_at": timestamp})));
    }
    mutations
}

/// LLM-extraction layer. Each entity becomes one `graph_nodes` row plus one
/// `graph_edges` row linking it back to the meeting node; the edge's
/// `provenance_json` carries the evidence segment ids, confidence and model
/// run id, and `epistemic_status` is `"ai_proposed"` so it is never
/// indistinguishable from structural (`"confirmed"`) truth. The edge id
/// reuses the 16 hex characters `det_node_id` appends as its final 16 bytes
/// (pure ASCII, so the slice always lands on a char boundary regardless of
/// what `recording_id` contains) — that suffix is the entity's content hash,
/// so it is already unique per entity and reusing it keeps the edge id
/// deterministic too.
pub(crate) fn extraction_mutations(
    project_id: &str,
    recording_id: &str,
    model_run_id: &str,
    extraction: &Extraction,
    segment_ids: &[String],
    timestamp: &str,
) -> Vec<genesis_block_native::RelationalRowMutation> {
    let meeting_node = format!("meeting:{recording_id}");
    let mut mutations = Vec::new();
    let groups: [(&str, &Vec<ExtractedItem>); 4] = [
        ("topic", &extraction.topics),
        ("decision", &extraction.decisions),
        ("action_item", &extraction.action_items),
        ("mention", &extraction.mentions),
    ];
    for (kind, items) in groups {
        for item in items {
            if item.label.trim().is_empty() {
                continue;
            }
            let node_id = det_node_id(recording_id, kind, &item.label);
            let evidence: Vec<&str> = item
                .evidence
                .iter()
                .filter_map(|index| segment_ids.get(*index).map(String::as_str))
                .collect();
            let provenance = serde_json::json!({
                "actor": "ai",
                "modelRunId": model_run_id,
                "evidenceSegmentIds": evidence,
                "confidence": item.confidence,
                "owner": item.owner,
                "kind": item.kind,
            })
            .to_string();
            mutations.push(genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": node_id, "project_id": project_id, "entity_type": kind, "entity_id": node_id, "label": item.label, "position_x": 70.0, "position_y": 30.0, "created_at": timestamp, "updated_at": timestamp})));
            mutations.push(genesis_adapter::upsert("graph_edges", serde_json::json!({"id": format!("gxe:{recording_id}:{}", &node_id[node_id.len() - 16..]), "project_id": project_id, "source_node_id": node_id, "target_node_id": meeting_node, "predicate": "extracted_from", "epistemic_status": "ai_proposed", "provenance_json": provenance, "created_at": timestamp, "updated_at": timestamp})));
        }
    }
    mutations
}

const EXTRACTION_PROMPT_HEADER: &str = r#"You are a meeting-analysis assistant. From the numbered transcript below, extract entities as STRICT JSON with this exact shape (no prose, no markdown):
{"topics":[{"label":"...","evidence":[segment numbers],"confidence":0.0}],
 "decisions":[{"label":"...","evidence":[...],"confidence":0.0}],
 "actionItems":[{"label":"who does what by when","owner":"speaker name or null","evidence":[...],"confidence":0.0}],
 "mentions":[{"label":"...","kind":"person|project|organization|other","evidence":[...],"confidence":0.0}]}
Labels must be in the transcript's language (Thai stays Thai). Evidence lists the segment numbers that support each item. Use [] when a category has nothing.
Transcript:
"#;

pub(crate) fn llm_provider_config(
    storage: &genesis_block_native::Storage,
) -> Result<(String, String), String> {
    let row = genesis_adapter::query(
        storage,
        "model_providers",
        &["config_json", "enabled"],
        vec![genesis_adapter::eq(
            "model_providers",
            "id",
            serde_json::json!("ollama-summary-intent"),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "summary/intent model provider is not configured".to_string())?;
    let enabled = row
        .get("model_providers.enabled")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_i64().map(|number| number != 0))
        })
        .unwrap_or(false);
    if !enabled {
        return Err("summary/intent model provider is disabled".to_string());
    }
    let config = row
        .get("model_providers.config_json")
        .and_then(serde_json::Value::as_str)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or(serde_json::Value::Null);
    let endpoint = config
        .get("endpoint")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("http://127.0.0.1:11434")
        .to_string();
    let model = config
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("llama3.1:8b")
        .to_string();
    let model = resolve_available_model(&endpoint, model);
    Ok((endpoint, model))
}

/// The configured model name is a preference, not a guarantee — users install
/// whatever they like into Ollama. If the endpoint is reachable and does NOT
/// have the configured model, fall back to the first installed model instead
/// of letting every downstream call 404. Unreachable endpoint: keep the
/// configured name and let the caller surface the connection error.
fn resolve_available_model(endpoint: &str, configured: String) -> String {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return configured;
    };
    let Ok(response) = client.get(format!("{endpoint}/api/tags")).send() else {
        return configured;
    };
    let Ok(tags) = response.json::<serde_json::Value>() else {
        return configured;
    };
    let names: Vec<String> = tags
        .get("models")
        .and_then(serde_json::Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() || names.iter().any(|name| name == &configured) {
        return configured;
    }
    names.into_iter().next().expect("non-empty checked above")
}

pub(crate) fn call_llm(endpoint: &str, model: &str, prompt: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ChatMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct ChatResponse {
        message: ChatMessage,
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{endpoint}/api/chat"))
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
            "format": "json",
        }))
        .send()
        .map_err(|e| send_error_message(endpoint, &e))?;
    if !response.status().is_success() {
        return Err(format!("LLM endpoint returned {}", response.status()));
    }
    response
        .json::<ChatResponse>()
        .map(|r| r.message.content)
        .map_err(|e| e.to_string())
}

/// Error text for a failed `send()` in [`call_llm`].
///
/// A timeout and a connection failure are NOT the same condition and must not
/// share wording: `cloud_executor::is_connection_error` matches on
/// "LLM endpoint unreachable" to decide whether to retry the prompt in the
/// cloud, and a slow-but-working Ollama is a *reachable* endpoint — shipping
/// that user's transcript to a cloud provider because their machine was busy
/// would violate the local-first contract. Mirrors the `e.is_timeout()` idiom
/// `cloud_executor::custom_llm` already uses for the same distinction.
///
/// Factored out of `call_llm` so the mapping is testable against real
/// `reqwest::Error`s without waiting out the 600s production timeout above;
/// see `a_slow_but_reachable_endpoint_is_a_timeout_not_a_connection_failure`.
fn send_error_message(endpoint: &str, e: &reqwest::Error) -> String {
    if e.is_timeout() {
        format!("LLM endpoint timed out at {endpoint}: {e}")
    } else {
        format!("LLM endpoint unreachable at {endpoint}: {e}")
    }
}

/// The label the meeting node carries in the graph.
///
/// Resolved from the ledger rather than passed in by the caller, because a
/// queued job has to be able to run itself: the Zoom importer used to hand
/// the meeting topic down through four call frames, which meant the same
/// build triggered from the retry button silently produced a
/// differently-labelled meeting node. Zoom's topic is recovered from the
/// `external_meeting_links` row it already writes; anything else falls back
/// to the project name.
pub(crate) fn meeting_label(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> String {
    let topic = genesis_adapter::query(
        storage,
        "external_meeting_links",
        &["payload_json"],
        vec![genesis_adapter::eq(
            "external_meeting_links",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        1,
    )
    .ok()
    .and_then(|rows| rows.into_iter().next())
    .and_then(|row| {
        let raw = row.get("external_meeting_links.payload_json")?.clone();
        let parsed = raw
            .as_str()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .unwrap_or(raw);
        parsed
            .get("topic")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    })
    .filter(|topic| !topic.trim().is_empty());
    if let Some(topic) = topic {
        return topic;
    }
    genesis_adapter::query(
        storage,
        "projects",
        &["name"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .ok()
    .and_then(|rows| rows.into_iter().next())
    .and_then(|row| {
        row.get("projects.name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    })
    .filter(|name| !name.trim().is_empty())
    .unwrap_or_else(|| "Meeting".to_string())
}

/// Queues a graph build for a recording whose transcript is ready.
///
/// This used to seed its own job row and spawn a detached thread, so a build
/// that died with the process left a `running` row nothing would ever
/// finish, and an LLM outage lost the extraction layer outright. The engine
/// owns both now. `trigger_job_id`, when given, is the job that caused this
/// one (usually `zoom.import`) — a queueing failure is reported against it
/// because that job may already have said `completed`.
pub(crate) fn queue_graph_build(
    engine: &crate::job_engine::JobEngine,
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    trigger_job_id: Option<&str>,
) {
    let Err(message) = engine.enqueue(
        crate::job_engine::JobKind::GraphBuild,
        project_id,
        Some(recording_id),
    ) else {
        return;
    };
    let Some(trigger_job_id) = trigger_job_id else {
        eprintln!("[jobs] graph build could not be queued: {message}");
        return;
    };
    let _ = genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "job_events",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "job_id": trigger_job_id,
                "status": "failed",
                "message": format!("graph build could not be queued: {message}"),
                "created_at": now(),
            }),
        )],
    );
}

pub(crate) fn run_graph_build(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    meeting_label: &str,
    job_id: &str,
) -> Result<(), String> {
    // 1) Structural layer (always succeeds independently of the LLM).
    // NOTE (query ceiling): capped at 1000 transcript segments — the engine
    // rejects any limit above that. A recording with more than 1000 segments
    // silently loses its tail here: those segments are absent from both the
    // evidence-segment-id list used below and the LLM prompt. There is no
    // offset/cursor to page around this for a read that must return every
    // segment in one shot the way this prompt needs it.
    let mut segment_rows = genesis_adapter::query(
        storage,
        "transcript_segments",
        &["id", "start_ms", "text", "speaker_id"],
        vec![genesis_adapter::eq(
            "transcript_segments",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        QUERY_ROW_CEILING,
    )?;
    segment_rows.sort_by_key(|row| {
        row.get("transcript_segments.start_ms")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
    });
    if segment_rows.len() >= QUERY_ROW_CEILING as usize {
        let timestamp = now();
        let _ = genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "job_events",
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "job_id": job_id,
                    "status": "running",
                    "message": "transcript exceeds the 1000-row query ceiling; graph extraction covers only the first 1000 segments",
                    "created_at": timestamp,
                }),
            )],
        );
    }

    // Speakers for the `spoke_in` edges must be scoped to *this recording*,
    // not the whole project: a project can hold multiple recordings, and
    // asserting a "confirmed" spoke_in edge for a speaker who spoke in a
    // different recording of the same project would violate the
    // epistemic-status rule. genesis_adapter filters are equality-only, so
    // there's no `id IN (...)` query — collect this recording's speaker ids
    // from its transcript segments and speaker turns, then resolve display
    // names by pulling the project's speakers (capped, like every other
    // query here) and keeping only the ids that are actually in scope.
    let mut recording_speaker_ids: std::collections::HashSet<String> = segment_rows
        .iter()
        .filter_map(|row| {
            row.get("transcript_segments.speaker_id")
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_owned)
        .collect();
    let turn_rows = genesis_adapter::query(
        storage,
        "speaker_turns",
        &["speaker_id"],
        vec![genesis_adapter::eq(
            "speaker_turns",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        QUERY_ROW_CEILING,
    )?;
    if turn_rows.len() >= QUERY_ROW_CEILING as usize {
        let timestamp = now();
        let _ = genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "job_events",
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "job_id": job_id,
                    "status": "running",
                    "message": "recording's speaker turns exceed the 1000-row query ceiling; some turn-only speakers may be missing from the graph",
                    "created_at": timestamp,
                }),
            )],
        );
    }
    recording_speaker_ids.extend(turn_rows.iter().filter_map(|row| {
        row.get("speaker_turns.speaker_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    }));

    let speaker_rows = genesis_adapter::query(
        storage,
        "speakers",
        &["id", "display_name"],
        vec![genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        QUERY_ROW_CEILING,
    )?;
    if speaker_rows.len() >= QUERY_ROW_CEILING as usize {
        let timestamp = now();
        let _ = genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "job_events",
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "job_id": job_id,
                    "status": "running",
                    "message": "project speakers exceed the 1000-row query ceiling; some of this recording's speakers may be missing from the graph",
                    "created_at": timestamp,
                }),
            )],
        );
    }
    let speakers: Vec<(String, String)> = speaker_rows
        .iter()
        .filter_map(|row| {
            let id = row.get("speakers.id")?.as_str()?.to_string();
            if !recording_speaker_ids.contains(&id) {
                return None;
            }
            Some((id, row.get("speakers.display_name")?.as_str()?.to_string()))
        })
        .collect();
    let timestamp = now();
    ensure_project_node(storage, project_id, &timestamp)?;
    genesis_adapter::commit_rows(
        storage,
        structural_mutations(
            project_id,
            recording_id,
            meeting_label,
            &speakers,
            &timestamp,
        ),
    )?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(20), None);

    // 2) Find (but do not yet delete) old extraction rows for this recording,
    //    so a re-run replaces rather than duplicates them.
    // NOTE (query ceiling): these two queries are filtered by project_id (the
    // only equality filter available — prefix filtering happens in Rust), so
    // they return this recording's stale gx:/gxe: rows mixed in with every
    // other graph node/edge in the project, still capped at 1000 rows total.
    // If a project's cumulative graph_nodes/graph_edges exceed 1000, some of
    // this recording's prior extraction rows can fall outside the page and
    // survive the cleanup below — a re-run would then leave orphaned rows
    // from the previous run alongside the freshly-inserted ones instead of
    // replacing them. There is no offset to page further into an
    // equality-only, non-deletable remainder, so this is a known limitation
    // rather than a silently-patched one.
    let node_rows = genesis_adapter::query(
        storage,
        "graph_nodes",
        &["id"],
        vec![genesis_adapter::eq(
            "graph_nodes",
            "project_id",
            serde_json::json!(project_id),
        )],
        QUERY_ROW_CEILING,
    )?;
    let edge_rows = genesis_adapter::query(
        storage,
        "graph_edges",
        &["id"],
        vec![genesis_adapter::eq(
            "graph_edges",
            "project_id",
            serde_json::json!(project_id),
        )],
        QUERY_ROW_CEILING,
    )?;
    if node_rows.len() >= QUERY_ROW_CEILING as usize
        || edge_rows.len() >= QUERY_ROW_CEILING as usize
    {
        let timestamp = now();
        let _ = genesis_adapter::commit_rows(
            storage,
            vec![genesis_adapter::upsert(
                "job_events",
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "job_id": job_id,
                    "status": "running",
                    "message": "project graph exceeds the 1000-row query ceiling; some superseded extraction rows may remain",
                    "created_at": timestamp,
                }),
            )],
        );
    }
    // Computed here (queries happen where they always did) but NOT committed
    // yet: deleting the prior extraction before the LLM call means an
    // ordinary failure (Ollama not running) destroys the previous good
    // extraction and produces nothing. The delete is folded into the same
    // commit as the fresh insert below, once parse_extraction has actually
    // succeeded — extraction ids are deterministic, so delete+insert in one
    // batch is still idempotent.
    let mut cleanup = Vec::new();
    for id in stale_extraction_ids(
        &edge_rows,
        "graph_edges.id",
        &format!("gxe:{recording_id}:"),
    ) {
        cleanup.push(genesis_adapter::delete("graph_edges", &id));
    }
    for id in stale_extraction_ids(&node_rows, "graph_nodes.id", &format!("gx:{recording_id}:")) {
        cleanup.push(genesis_adapter::delete("graph_nodes", &id));
    }

    // 3) LLM extraction (best-effort by design, but a failure fails the JOB so
    //    the user can retry — structural graph above is already committed,
    //    and the prior extraction (if any) is untouched since its cleanup
    //    hasn't been committed yet).
    let segment_ids: Vec<String> = segment_rows
        .iter()
        .filter_map(|row| {
            row.get("transcript_segments.id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    let mut prompt = String::from(EXTRACTION_PROMPT_HEADER);
    for (index, row) in segment_rows.iter().enumerate() {
        let text = row
            .get("transcript_segments.text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        prompt.push_str(&format!("[{index}] {text}\n"));
    }
    let (endpoint, model) = llm_provider_config(storage)?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(40), None);
    // Tier-3 cloud fallback inputs. The policy DB lives in the app data
    // directory, which is the parent of the GenesisDB directory this Storage
    // was opened at (lib.rs::app_state opens it at `<app data dir>/genesisdb`).
    // Deriving it from the Storage already in hand keeps the path off
    // start_graph_build's signature, which runs on a detached worker thread
    // with no AppState in scope.
    let data_root = storage.path.parent().ok_or_else(|| {
        "cannot locate the app data directory from the graph storage path".to_string()
    })?;
    let policy_conn = crate::paired_devices_connection_at(data_root).map_err(|e| e.to_string())?;
    let policy = crate::policy::load_policy(&policy_conn)?;
    let calls_today =
        crate::policy::calls_today(&policy_conn, crate::cloud_config::CloudTaskKind::Llm)?;
    // The fallback itself lives in cloud_executor.rs, which is the module
    // allowed to handle key-bearing cloud configs; see the guard note there.
    let cloud = crate::cloud_executor::first_configured_llm_provider();
    // `runtime_location` is reported back by the fallback because only it
    // knows which transport actually produced the text; recording a hardcoded
    // "local" here would make the model_runs audit row false for every
    // cloud-fallback build.
    let (raw, runtime_location) = crate::cloud_executor::call_llm_with_fallback(
        || call_llm(&endpoint, &model, &prompt),
        &prompt,
        cloud.as_ref(),
        &policy,
        calls_today,
        &policy_conn,
    )?;
    let extraction = parse_extraction(&raw)?;
    let _ = crate::set_job_status(storage, job_id, "running", Some(80), None);

    let model_run_id = Uuid::new_v4().to_string();
    let timestamp = now();
    // The prior extraction's cleanup deletes and the fresh insert land in one
    // commit — only now that the new extraction has actually parsed.
    let mut mutations = cleanup;
    mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": "ollama-summary-intent", "model_name": model, "task_kind": "graph_extraction", "runtime_location": runtime_location, "input_ref": recording_id, "output_ref": format!("graph:{recording_id}"), "parameters_json": {"endpoint": endpoint}, "created_at": timestamp})));
    mutations.extend(extraction_mutations(
        project_id,
        recording_id,
        &model_run_id,
        &extraction,
        &segment_ids,
        &timestamp,
    ));
    genesis_adapter::commit_rows(storage, mutations)
}

/// Manual retry surface for a failed/never-run graph build.
///
/// Enqueues rather than spawning, so pressing it twice queues one build and
/// a failed build is retried by policy instead of by the user noticing.
#[tauri::command]
pub(crate) fn graph_build_start(
    project_id: String,
    recording_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> crate::AppResult<()> {
    state
        .jobs
        .enqueue(
            crate::job_engine::JobKind::GraphBuild,
            &project_id,
            Some(&recording_id),
        )
        .map(|_| ())
        .map_err(crate::AppError::Genesis)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTRACTION_FIXTURE: &str = r#"{
      "topics": [{"label": "Q3 roadmap", "evidence": [0, 2], "confidence": 0.8}],
      "decisions": [{"label": "Ship zoom import in August", "evidence": [2], "confidence": 0.7}],
      "actionItems": [{"label": "Boss drafts the release note", "owner": "p:boss", "evidence": [3], "confidence": 0.9}],
      "mentions": [{"label": "GenesisBlockDB", "kind": "project", "evidence": [1], "confidence": 0.6}]
    }"#;

    #[test]
    fn extraction_parses_with_tolerant_defaults() {
        let extraction = parse_extraction(EXTRACTION_FIXTURE).unwrap();
        assert_eq!(extraction.topics.len(), 1);
        assert_eq!(extraction.action_items[0].owner.as_deref(), Some("p:boss"));
        // Missing arrays default to empty instead of failing.
        let sparse = parse_extraction(r#"{"topics": []}"#).unwrap();
        assert!(sparse.decisions.is_empty());
    }

    #[test]
    fn det_node_ids_are_stable_and_recording_scoped() {
        let a = det_node_id("rec-1", "topic", "Q3 roadmap");
        assert_eq!(a, det_node_id("rec-1", "topic", "Q3 roadmap"));
        assert_ne!(a, det_node_id("rec-2", "topic", "Q3 roadmap"));
        assert!(a.starts_with("gx:rec-1:"));
    }

    #[test]
    fn stale_ids_filter_matches_only_this_recordings_extractions() {
        let rows = vec![
            serde_json::json!({"graph_nodes.id": "gx:rec-1:aaaa"}),
            serde_json::json!({"graph_nodes.id": "gx:rec-2:bbbb"}),
            serde_json::json!({"graph_nodes.id": "meeting:rec-1"}),
            serde_json::json!({"graph_nodes.id": "some-note"}),
        ];
        assert_eq!(
            stale_extraction_ids(&rows, "graph_nodes.id", "gx:rec-1:"),
            vec!["gx:rec-1:aaaa".to_string()]
        );
    }

    #[test]
    fn extraction_mutations_carry_evidence_in_edge_provenance() {
        let extraction = parse_extraction(EXTRACTION_FIXTURE).unwrap();
        let segment_ids = vec![
            "s0".to_string(),
            "s1".to_string(),
            "s2".to_string(),
            "s3".to_string(),
        ];
        let mutations =
            extraction_mutations("p1", "rec-1", "run-1", &extraction, &segment_ids, "t");
        // 4 entities → 4 nodes + 4 edges.
        assert_eq!(mutations.len(), 8);
        let edge = mutations
            .iter()
            .find_map(|m| (m.table == "graph_edges").then(|| m.values.clone()))
            .unwrap();
        let provenance: serde_json::Value =
            serde_json::from_str(edge["provenance_json"].as_str().unwrap()).unwrap();
        assert_eq!(provenance["actor"], "ai");
        assert!(provenance["evidenceSegmentIds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v.as_str().unwrap().starts_with('s')));
        assert_eq!(edge["epistemic_status"], "ai_proposed");
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-graph-test-{}", Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    #[test]
    fn structural_commit_succeeds_for_a_project_seeded_without_a_graph_node() {
        let (path, storage) = open_storage();
        // Mirrors the Zoom import seed: a project row with no graph_nodes row.
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"Weekly sync","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"completed","duration_ms":10,"created_at":"t","updated_at":"t"})),
        ]).unwrap();

        ensure_project_node(&storage, "p1", "t").unwrap();
        crate::genesis_adapter::commit_rows(
            &storage,
            structural_mutations(
                "p1",
                "r1",
                "Weekly sync",
                &[("s1".to_string(), "Boss".to_string())],
                "t",
            ),
        )
        .expect("structural commit must not violate the graph_edges foreign key");

        let nodes = crate::genesis_adapter::query(
            &storage,
            "graph_nodes",
            &["id"],
            vec![crate::genesis_adapter::eq(
                "graph_nodes",
                "project_id",
                serde_json::json!("p1"),
            )],
            100,
        )
        .unwrap();
        // project + meeting + speaker
        assert_eq!(nodes.len(), 3);
        let edges = crate::genesis_adapter::query(
            &storage,
            "graph_edges",
            &["id"],
            vec![crate::genesis_adapter::eq(
                "graph_edges",
                "project_id",
                serde_json::json!("p1"),
            )],
            100,
        )
        .unwrap();
        // part_of + spoke_in
        assert_eq!(edges.len(), 2);

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn ensure_project_node_is_idempotent_and_keeps_the_existing_label() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"Weekly sync","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("graph_nodes", serde_json::json!({"id":"p1","project_id":"p1","entity_type":"project","entity_id":"p1","label":"Renamed by user","position_x":50.0,"position_y":17.0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();

        ensure_project_node(&storage, "p1", "t2").unwrap();

        let rows = crate::genesis_adapter::query(
            &storage,
            "graph_nodes",
            &["id", "label"],
            vec![crate::genesis_adapter::eq(
                "graph_nodes",
                "id",
                serde_json::json!("p1"),
            )],
            10,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0]["graph_nodes.label"], "Renamed by user",
            "must not overwrite a user-edited label"
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_failed_llm_call_leaves_the_prior_extraction_intact() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"Weekly sync","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"completed","duration_ms":10,"created_at":"t","updated_at":"t"})),
        ]).unwrap();
        ensure_project_node(&storage, "p1", "t").unwrap();

        // Seed a prior extraction node, as a previous successful run would
        // have left behind.
        let prior_node_id = det_node_id("r1", "topic", "Prior topic");
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("graph_nodes", serde_json::json!({"id": prior_node_id, "project_id":"p1","entity_type":"topic","entity_id":prior_node_id,"label":"Prior topic","position_x":70.0,"position_y":30.0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();

        // No model_providers row exists (e.g. Ollama not configured/running),
        // so llm_provider_config fails before any network call.
        let outcome = run_graph_build(&storage, "p1", "r1", "Weekly sync", "job-does-not-exist");
        assert!(
            outcome.is_err(),
            "a missing LLM provider must fail the build"
        );

        let nodes = crate::genesis_adapter::query(
            &storage,
            "graph_nodes",
            &["id"],
            vec![crate::genesis_adapter::eq(
                "graph_nodes",
                "id",
                serde_json::json!(prior_node_id),
            )],
            1,
        )
        .unwrap();
        assert_eq!(
            nodes.len(),
            1,
            "a failed LLM call must not delete the prior extraction"
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    /// The tier-3 cloud fallback decides whether to retry in the cloud by
    /// matching on `call_llm`'s error *text*, so the two must not drift
    /// apart. Pins both directions from the producing side: an unreachable
    /// Ollama has to be recognised as a connection failure, and a reachable
    /// Ollama that answers badly must NOT be — otherwise a genuine bug would
    /// get silently masked by a cloud retry.
    #[test]
    fn call_llm_error_text_matches_what_the_cloud_fallback_keys_on() {
        // Nothing listens on TCP port 1 — a deterministic, immediate
        // connection refusal rather than a real network round-trip.
        let unreachable = call_llm("http://127.0.0.1:1", "llama3.1:8b", "prompt")
            .expect_err("port 1 must refuse the connection");
        assert!(
            crate::cloud_executor::is_connection_error(&unreachable),
            "an unreachable Ollama must be recognised as a connection failure: {unreachable}",
        );

        // A reachable endpoint that answers 500: a real bug, not a missing
        // Ollama.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let _ = std::io::Write::write_all(
                    &mut stream,
                    b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n",
                );
            }
        });
        let bad_response = call_llm(&format!("http://{addr}"), "llama3.1:8b", "prompt")
            .expect_err("a 500 must fail the call");
        assert!(
            !crate::cloud_executor::is_connection_error(&bad_response),
            "a bad response must NOT be treated as a connection failure: {bad_response}",
        );
    }

    /// Third case of the same contract, split out because `call_llm`'s
    /// production timeout is 600s and no test may wait that out.
    ///
    /// A slow-but-reachable Ollama is NOT an unreachable one. Before the fix
    /// both conditions produced the same "LLM endpoint unreachable" text, so a
    /// busy local model silently shipped the user's transcript to a cloud
    /// provider. This drives the *production* mapping helper
    /// (`send_error_message`, which `call_llm` uses verbatim) with two real
    /// `reqwest::Error`s — one refused connection, one genuine client timeout
    /// against a server that accepts and then says nothing — and asserts they
    /// classify differently. Nothing here is a restatement of the mapping: the
    /// errors and the mapping are both the real ones, only the client's 1s
    /// timeout is test-local.
    #[test]
    fn a_slow_but_reachable_endpoint_is_a_timeout_not_a_connection_failure() {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()
            .unwrap();

        // Accepts the connection, reads the request, then holds the socket
        // open without answering until well past the client's 1s deadline.
        // Holding it (rather than dropping it) is what makes this a timeout
        // instead of a connection reset.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        let endpoint = format!("http://{addr}");
        let timeout_error = client
            .post(format!("{endpoint}/api/chat"))
            .json(&serde_json::json!({"model": "llama3.1:8b", "messages": []}))
            .send()
            .expect_err("a server that never answers must time the client out");
        assert!(
            timeout_error.is_timeout(),
            "sanity: reqwest must actually report this as a timeout, else the \
             distinction the fix relies on does not exist: {timeout_error}",
        );
        let timed_out = send_error_message(&endpoint, &timeout_error);
        assert!(
            timed_out.contains("LLM endpoint timed out"),
            "a timeout must say so: {timed_out}",
        );
        assert!(
            !crate::cloud_executor::is_connection_error(&timed_out),
            "a slow but reachable Ollama must NOT trigger the cloud fallback: {timed_out}",
        );

        // Same helper, same call site, genuinely refused connection: still
        // classified as unreachable. Proves the split did not break the case
        // the fallback exists for.
        // Deliberately a much longer deadline than the timeout half above.
        // On Windows a loopback connect to a closed port does not surface the
        // refusal instantly (the stack retransmits the SYN first), so a 1s
        // budget races it and reqwest reports the client deadline instead of
        // the refusal — measured on this host. 30s is far past that race and
        // still nowhere near call_llm's 600s; if a host ever exceeded it the
        // test fails loudly rather than quietly asserting the wrong thing.
        let refusing_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let refused_error = refusing_client
            .post("http://127.0.0.1:1/api/chat")
            .json(&serde_json::json!({"model": "llama3.1:8b", "messages": []}))
            .send()
            .expect_err("port 1 must refuse the connection");
        assert!(
            !refused_error.is_timeout(),
            "sanity: a refused connection is not a timeout: {refused_error}",
        );
        let unreachable = send_error_message("http://127.0.0.1:1", &refused_error);
        assert!(
            crate::cloud_executor::is_connection_error(&unreachable),
            "a genuinely unreachable Ollama must still be recognised: {unreachable}",
        );
    }
}
