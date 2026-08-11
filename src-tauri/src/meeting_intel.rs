//! Live-meeting intelligence: topic tracking, local-KB question answering,
//! and the post-meeting summary/export pipeline.
//!
//! @req FR-104, FR-105, FR-115, NFR-101, NFR-104
//!
//! Every model output here runs on the local BYOM provider
//! (`ollama-summary-intent`) and every *persisted* artifact carries
//! provenance: summaries reference a `model_runs` row, and evidence refs
//! point at real `transcript_segments` ids. Mid-meeting topic ticks are
//! ephemeral UI events by design — they are never stored as facts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::graph_build::{call_llm, llm_provider_config};
use crate::live_meeting::{RecentSegment, SharedRecent};
use crate::{genesis_adapter, now, AppError, AppResult, AppState};

const TOPIC_INTERVAL: Duration = Duration::from_secs(45);
const TOPIC_WINDOW_SEGMENTS: usize = 40;
/// The relational engine caps every query at 1000 rows with no cursor
/// (see graph_build.rs); searches below inherit that cap and say so.
const ENGINE_ROW_CAP: u32 = 1000;

// ---------------------------------------------------------------------------
// Topic tracker (ephemeral, event-only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveTopicEvent {
    recording_id: String,
    topic: String,
    open_points: Vec<String>,
    action_items: Vec<String>,
    model: String,
    window_start_ms: i64,
    window_end_ms: i64,
}

pub(crate) fn spawn_topic_tracker(
    app: tauri::AppHandle,
    storage: Arc<genesis_block_native::Storage>,
    recent: SharedRecent,
    stop: Arc<AtomicBool>,
    recording_id: String,
) {
    std::thread::spawn(move || {
        let mut last_run = Instant::now();
        let mut last_seen_end_ms: i64 = -1;
        loop {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
            if last_run.elapsed() < TOPIC_INTERVAL {
                continue;
            }
            let window: Vec<RecentSegment> = {
                let guard = recent.lock().expect("recent buffer mutex poisoned");
                guard
                    .iter()
                    .rev()
                    .take(TOPIC_WINDOW_SEGMENTS)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            };
            let Some(newest) = window.last() else { continue };
            if newest.end_ms <= last_seen_end_ms {
                continue; // nothing new was said since the last tick
            }
            last_run = Instant::now();
            last_seen_end_ms = newest.end_ms;

            let (endpoint, model) = match llm_provider_config(&storage) {
                Ok(pair) => pair,
                Err(_) => continue, // no local LLM — live view simply has no topic card
            };
            let transcript = render_window(&window);
            let prompt = format!(
                "คุณเป็นผู้ช่วยจดประชุมที่เป็นกลาง อ่านบทสนทนาล่าสุดแล้วตอบเป็น JSON เท่านั้น รูปแบบ: \
                 {{\"topic\": \"หัวข้อที่กำลังคุยตอนนี้ (สั้น 1 ประโยค)\", \"openPoints\": [\"ประเด็นที่ยังค้าง/คำถามที่ยังไม่ได้คำตอบ\"], \"actionItems\": [\"งานที่มีคนพูดว่าจะทำ\"]}} \
                 ห้ามเดาข้อมูลนอกบทสนทนา ห้ามประเมินบุคลิกหรืออารมณ์ของผู้พูด ตอบภาษาไทย\n\nบทสนทนา:\n{transcript}"
            );
            match call_llm(&endpoint, &model, &prompt) {
                Ok(raw) => {
                    let value = tolerant_json(&raw);
                    let topic = value
                        .get("topic")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if topic.is_empty() {
                        continue;
                    }
                    let _ = app.emit(
                        "live-topic",
                        LiveTopicEvent {
                            recording_id: recording_id.clone(),
                            topic,
                            open_points: string_list(&value, "openPoints"),
                            action_items: string_list(&value, "actionItems"),
                            model: model.clone(),
                            window_start_ms: window.first().map(|s| s.start_ms).unwrap_or(0),
                            window_end_ms: newest.end_ms,
                        },
                    );
                }
                Err(error) => eprintln!("[topic-tracker] skipped tick: {error}"),
            }
        }
    });
}

fn render_window(window: &[RecentSegment]) -> String {
    window
        .iter()
        .map(|segment| {
            format!(
                "[{}:{:02}] {}: {}",
                segment.start_ms / 60_000,
                (segment.start_ms / 1000) % 60,
                segment.speaker,
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tolerant_json(raw: &str) -> serde_json::Value {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        return value;
    }
    if let (Some(open), Some(close)) = (raw.find('{'), raw.rfind('}')) {
        if close > open {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw[open..=close]) {
                return value;
            }
        }
    }
    serde_json::Value::Null
}

fn string_list(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .filter(|item| !item.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// "ถาม FUNG" — local knowledge-base answer with cited sources
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskSource {
    n: usize,
    kind: String,
    project_name: Option<String>,
    text: String,
    start_ms: Option<i64>,
    recording_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskAnswer {
    answer: String,
    sources: Vec<AskSource>,
    model: String,
    searched_rows_capped: bool,
}

fn keyword_candidates(question: &str, endpoint: &str, model: &str) -> Vec<String> {
    let mut keywords: Vec<String> = question
        .split_whitespace()
        .filter(|token| token.chars().count() >= 2)
        .map(str::to_string)
        .collect();
    // Thai text usually arrives as one unsegmented token — ask the local LLM
    // to split search keys; on any failure the raw tokens still work.
    let prompt = format!(
        "แตกคำค้นหลักจากคำถามนี้เป็น JSON เท่านั้น: {{\"keywords\": [\"คำ1\", \"คำ2\"]}} ใช้ 2-6 คำ สั้น กระชับ ทั้งไทย/อังกฤษตามคำถาม ห้ามอธิบายเพิ่ม\nคำถาม: {question}"
    );
    if let Ok(raw) = call_llm(endpoint, model, &prompt) {
        keywords.extend(string_list(&tolerant_json(&raw), "keywords"));
    }
    keywords.retain(|keyword| keyword.chars().count() >= 2);
    keywords.sort();
    keywords.dedup();
    keywords
}

fn contains_any(text: &str, keywords: &[String]) -> usize {
    let lowered = text.to_lowercase();
    keywords
        .iter()
        .filter(|keyword| lowered.contains(&keyword.to_lowercase()))
        .count()
}

#[tauri::command]
pub(crate) fn meeting_ask(
    question: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<AskAnswer> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err(AppError::InvalidInput("คำถามว่างเปล่า".to_string()));
    }
    let (endpoint, model) = llm_provider_config(&state.genesis).map_err(AppError::Genesis)?;
    let keywords = keyword_candidates(&question, &endpoint, &model);

    let project_names: std::collections::HashMap<String, String> =
        genesis_adapter::query(&state.genesis, "projects", &["id", "name"], vec![], ENGINE_ROW_CAP)
            .map_err(AppError::Genesis)?
            .into_iter()
            .filter_map(|row| {
                Some((
                    row.get("projects.id")?.as_str()?.to_string(),
                    row.get("projects.name")?.as_str()?.to_string(),
                ))
            })
            .collect();

    // Past transcripts (optionally narrowed to one project).
    let segment_filter = match &project_id {
        Some(id) => vec![genesis_adapter::eq("transcript_segments", "project_id", serde_json::json!(id))],
        None => vec![],
    };
    let segment_rows = genesis_adapter::query(
        &state.genesis,
        "transcript_segments",
        &["id", "project_id", "recording_id", "start_ms", "text", "created_at"],
        segment_filter,
        ENGINE_ROW_CAP,
    )
    .map_err(AppError::Genesis)?;
    let searched_rows_capped = segment_rows.len() >= ENGINE_ROW_CAP as usize;

    let mut scored: Vec<(usize, String, serde_json::Value)> = segment_rows
        .into_iter()
        .filter_map(|row| {
            let text = row.get("transcript_segments.text")?.as_str()?.to_string();
            let hits = contains_any(&text, &keywords);
            if hits == 0 {
                return None;
            }
            let created = row
                .get("transcript_segments.created_at")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            Some((hits, created, row))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    scored.truncate(12);

    // Knowledge-graph nodes (topics/decisions/actions extracted earlier).
    let graph_rows = genesis_adapter::query(
        &state.genesis,
        "graph_nodes",
        &["id", "project_id", "entity_type", "label"],
        vec![],
        ENGINE_ROW_CAP,
    )
    .unwrap_or_default();
    let mut graph_hits: Vec<serde_json::Value> = graph_rows
        .into_iter()
        .filter(|row| {
            row.get("graph_nodes.label")
                .and_then(serde_json::Value::as_str)
                .map(|label| contains_any(label, &keywords) > 0)
                .unwrap_or(false)
        })
        .collect();
    graph_hits.truncate(6);

    let mut sources: Vec<AskSource> = Vec::new();
    for (_, _, row) in &scored {
        let pid = row
            .get("transcript_segments.project_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        sources.push(AskSource {
            n: sources.len() + 1,
            kind: "transcript".to_string(),
            project_name: project_names.get(pid).cloned(),
            text: row
                .get("transcript_segments.text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            start_ms: row
                .get("transcript_segments.start_ms")
                .and_then(serde_json::Value::as_i64),
            recording_id: row
                .get("transcript_segments.recording_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        });
    }
    for row in &graph_hits {
        let pid = row
            .get("graph_nodes.project_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        sources.push(AskSource {
            n: sources.len() + 1,
            kind: format!(
                "graph:{}",
                row.get("graph_nodes.entity_type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("node")
            ),
            project_name: project_names.get(pid).cloned(),
            text: row
                .get("graph_nodes.label")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            start_ms: None,
            recording_id: None,
        });
    }

    // Current-session tail gives the model live context (not cited).
    let live_tail: Vec<RecentSegment> = {
        let live = state.live.lock().expect("live session mutex poisoned");
        live.as_ref()
            .map(|session| {
                let guard = session.recent.lock().expect("recent buffer mutex poisoned");
                guard.iter().rev().take(10).cloned().collect::<Vec<_>>().into_iter().rev().collect()
            })
            .unwrap_or_default()
    };

    if sources.is_empty() {
        return Ok(AskAnswer {
            answer: "ไม่พบข้อมูลที่เกี่ยวข้องใน Knowledge Base ในเครื่อง (ค้นจาก transcript เก่าและ knowledge graph แล้ว)".to_string(),
            sources,
            model,
            searched_rows_capped,
        });
    }

    let evidence_block = sources
        .iter()
        .map(|source| {
            format!(
                "[{}] ({}{}) {}",
                source.n,
                source.kind,
                source
                    .project_name
                    .as_ref()
                    .map(|name| format!(" — โปรเจกต์ {name}"))
                    .unwrap_or_default(),
                source.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let live_block = if live_tail.is_empty() {
        String::new()
    } else {
        format!("\n\nบริบทที่กำลังคุยกันตอนนี้ (อ้างอิงไม่ได้ ใช้เข้าใจคำถามเท่านั้น):\n{}", render_window(&live_tail))
    };
    let prompt = format!(
        "คุณเป็นผู้ช่วยค้นข้อมูลระหว่างประชุม ตอบคำถามโดยใช้ข้อมูลจากหลักฐานที่ให้เท่านั้น \
         ถ้าหลักฐานไม่พอให้บอกตรง ๆ ว่าไม่พบ ห้ามเดา ตอบสั้น กระชับ ภาษาไทย และตอบเป็น JSON เท่านั้น: \
         {{\"answer\": \"คำตอบ\", \"refs\": [1, 2]}} โดย refs คือหมายเลขหลักฐานที่ใช้จริง\n\nคำถาม: {question}\n\nหลักฐาน:\n{evidence_block}{live_block}"
    );
    let raw = call_llm(&endpoint, &model, &prompt).map_err(AppError::Genesis)?;
    let value = tolerant_json(&raw);
    let answer = value
        .get("answer")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| raw.trim().to_string());
    let refs: Vec<usize> = value
        .get("refs")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(|item| item.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    let cited: Vec<AskSource> = if refs.is_empty() {
        sources.into_iter().take(5).collect()
    } else {
        sources.into_iter().filter(|source| refs.contains(&source.n)).collect()
    };

    Ok(AskAnswer {
        answer,
        sources: cited,
        model,
        searched_rows_capped,
    })
}

// ---------------------------------------------------------------------------
// Post-meeting pipeline: summary (3 kinds) → markdown export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveSummaryEvent {
    recording_id: String,
    state: String,
    detail: Option<String>,
    export_path: Option<String>,
}

struct SegmentView {
    id: String,
    speaker: String,
    start_ms: i64,
    text: String,
}

fn load_segments(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<SegmentView>, String> {
    let speaker_names: std::collections::HashMap<String, String> = genesis_adapter::query(
        storage,
        "speakers",
        &["id", "display_name"],
        vec![genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id))],
        ENGINE_ROW_CAP,
    )?
    .into_iter()
    .filter_map(|row| {
        Some((
            row.get("speakers.id")?.as_str()?.to_string(),
            row.get("speakers.display_name")?.as_str()?.to_string(),
        ))
    })
    .collect();

    let mut segments: Vec<SegmentView> = genesis_adapter::query(
        storage,
        "transcript_segments",
        &["id", "recording_id", "speaker_id", "start_ms", "text"],
        vec![genesis_adapter::eq("transcript_segments", "recording_id", serde_json::json!(recording_id))],
        ENGINE_ROW_CAP,
    )?
    .into_iter()
    .filter_map(|row| {
        Some(SegmentView {
            id: row.get("transcript_segments.id")?.as_str()?.to_string(),
            speaker: row
                .get("transcript_segments.speaker_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|id| speaker_names.get(id))
                .cloned()
                .unwrap_or_else(|| "ไม่ระบุ".to_string()),
            start_ms: row.get("transcript_segments.start_ms").and_then(serde_json::Value::as_i64)?,
            text: row.get("transcript_segments.text")?.as_str()?.to_string(),
        })
    })
    .collect();
    segments.sort_by_key(|segment| segment.start_ms);
    Ok(segments)
}

fn refs_to_segment_ids(value: &serde_json::Value, key: &str, segments: &[SegmentView]) -> Vec<String> {
    let mut ids: Vec<String> = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64())
                .filter_map(|index| segments.get(index as usize).map(|segment| segment.id.clone()))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    ids
}

fn collect_item_refs(items: &serde_json::Value, segments: &[SegmentView]) -> Vec<String> {
    let mut ids: Vec<String> = items
        .as_array()
        .map(|list| {
            list.iter()
                .flat_map(|item| refs_to_segment_ids(item, "refs", segments))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    ids
}

/// Generates the three summary kinds, persists them with model provenance,
/// writes the Markdown export, and reports progress over `live-summary`
/// events. Called by the live coordinator after capture ends; also reachable
/// by a UI retry via `generate_meeting_summary`.
pub(crate) fn run_post_meeting(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) {
    let emit = |state: &str, detail: Option<String>, export_path: Option<String>| {
        let _ = app.emit(
            "live-summary",
            LiveSummaryEvent {
                recording_id: recording_id.to_string(),
                state: state.to_string(),
                detail,
                export_path,
            },
        );
    };
    emit("running", Some("กำลังสรุปการประชุม...".to_string()), None);
    match generate_summaries_and_export(storage, project_id, recording_id) {
        Ok(export_path) => emit("ready", None, Some(export_path)),
        Err(error) => emit("failed", Some(error), None),
    }
}

pub(crate) fn generate_summaries_and_export(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<String, String> {
    let segments = load_segments(storage, project_id, recording_id)?;
    if segments.is_empty() {
        return Err("ยังไม่มี transcript ของเซสชันนี้ — ถ้าถอดสดล้มเหลว ให้ถอดจากไฟล์ chunk ย้อนหลังก่อน".to_string());
    }
    let (endpoint, model) = llm_provider_config(storage)?;

    let job_id = Uuid::new_v4().to_string();
    let created = now();
    genesis_adapter::commit_rows(storage, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "summary.generate", "status": "running", "progress": 0, "input_refs_json": [recording_id], "output_refs_json": [], "provider_id": "ollama-summary-intent", "error_code": null, "error_message": null, "attempt_no": 1, "started_at": created, "finished_at": null, "created_at": created, "updated_at": created})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": "summary.generate started", "created_at": created})),
    ])?;

    let transcript_block = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            format!(
                "[{index}] ({}:{:02}) {}: {}",
                segment.start_ms / 60_000,
                (segment.start_ms / 1000) % 60,
                segment.speaker,
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let shared_rules = "ใช้ข้อมูลจาก transcript เท่านั้น ห้ามแต่งเติม ห้ามตัดสินบุคลิกผู้พูด อ้างอิงด้วยหมายเลข segment ใน refs เสมอ ตอบเป็น JSON เท่านั้น ภาษาไทย";
    let result = (|| -> Result<String, String> {
        // 1) whole_story — narrative paragraph.
        let raw = call_llm(&endpoint, &model, &format!(
            "{shared_rules} รูปแบบ: {{\"story\": \"ย่อหน้าสรุปเรื่องราวการประชุมทั้งหมด 4-8 ประโยค\", \"refs\": [0,1]}}\n\nTranscript:\n{transcript_block}"
        ))?;
        let story_value = tolerant_json(&raw);
        let story = story_value.get("story").and_then(serde_json::Value::as_str).unwrap_or(raw.trim()).to_string();
        let story_refs = refs_to_segment_ids(&story_value, "refs", &segments);

        // 2) timeline — key points in order.
        let raw = call_llm(&endpoint, &model, &format!(
            "{shared_rules} รูปแบบ: {{\"points\": [{{\"point\": \"ประเด็นสำคัญ\", \"refs\": [3]}}]}} เรียงตามลำดับเวลา 4-10 ข้อ\n\nTranscript:\n{transcript_block}"
        ))?;
        let points_value = tolerant_json(&raw);
        let points = points_value.get("points").cloned().unwrap_or(serde_json::json!([]));
        let points_refs = collect_item_refs(&points, &segments);

        // 3) decisions_actions — decisions made + tasks owned.
        let raw = call_llm(&endpoint, &model, &format!(
            "{shared_rules} รูปแบบ: {{\"items\": [{{\"item\": \"งานหรือข้อตัดสินใจ\", \"owner\": \"ชื่อผู้รับผิดชอบถ้าระบุใน transcript ไม่งั้น null\", \"refs\": [5]}}]}}\n\nTranscript:\n{transcript_block}"
        ))?;
        let actions_value = tolerant_json(&raw);
        let actions = actions_value.get("items").cloned().unwrap_or(serde_json::json!([]));
        let actions_refs = collect_item_refs(&actions, &segments);

        // Persist: one model_run + one summary row per kind.
        let mut mutations = Vec::new();
        let mut persisted: Vec<(String, String)> = Vec::new(); // (kind, content)
        for (kind, content, refs) in [
            ("whole_story", story.clone(), story_refs),
            ("timeline", serde_json::to_string(&points).unwrap_or_else(|_| "[]".into()), points_refs),
            ("decisions_actions", serde_json::to_string(&actions).unwrap_or_else(|_| "[]".into()), actions_refs),
        ] {
            let timestamp = now();
            let model_run_id = Uuid::new_v4().to_string();
            let summary_id = Uuid::new_v4().to_string();
            mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({
                "id": model_run_id, "recording_id": recording_id, "provider_id": "ollama-summary-intent",
                "model_name": model, "task_kind": format!("summary.generate:{kind}"), "runtime_location": "local",
                "input_ref": recording_id, "output_ref": format!("summary:{summary_id}"),
                "parameters_json": {"endpoint": endpoint}, "created_at": timestamp,
            })));
            mutations.push(genesis_adapter::upsert("summaries", serde_json::json!({
                "id": summary_id, "project_id": project_id, "kind": kind, "content": content,
                "evidence_refs_json": refs, "model_run_id": model_run_id,
                "created_at": timestamp, "updated_at": timestamp,
            })));
            persisted.push((kind.to_string(), content));
        }
        genesis_adapter::commit_rows(storage, mutations)?;

        // Markdown export closes the loop.
        let export_path = write_markdown_export(storage, project_id, recording_id, &persisted, &segments)?;
        Ok(export_path)
    })();

    match &result {
        Ok(_) => {
            let _ = crate::set_job_status(storage, &job_id, "completed", Some(100), None);
        }
        Err(error) => {
            let _ = crate::set_job_status(storage, &job_id, "failed", None, Some(error));
        }
    }
    result
}

fn format_clock(ms: i64) -> String {
    format!("{}:{:02}:{:02}", ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60)
}

fn write_markdown_export(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    summaries: &[(String, String)],
    segments: &[SegmentView],
) -> Result<String, String> {
    let project = genesis_adapter::query(
        storage,
        "projects",
        &["name", "storage_path"],
        vec![genesis_adapter::eq("projects", "id", serde_json::json!(project_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or("project not found")?;
    let project_name = genesis_adapter::string(&project, "projects.name")?;
    let storage_path = genesis_adapter::string(&project, "projects.storage_path")?;

    let mut body = String::new();
    body.push_str(&format!("# บันทึกการประชุม — {project_name}\n\n"));
    body.push_str(&format!("- สร้างเมื่อ: {}\n- recording: `{recording_id}`\n- เครื่องมือ: FUNG Live Meeting (ประมวลผลในเครื่องทั้งหมด)\n\n", now()));

    for (kind, content) in summaries {
        match kind.as_str() {
            "whole_story" => {
                body.push_str("## สรุปภาพรวม\n\n");
                body.push_str(content);
                body.push_str("\n\n");
            }
            "timeline" => {
                body.push_str("## ประเด็นสำคัญ\n\n");
                if let Ok(points) = serde_json::from_str::<serde_json::Value>(content) {
                    for point in points.as_array().cloned().unwrap_or_default() {
                        if let Some(text) = point.get("point").and_then(serde_json::Value::as_str) {
                            body.push_str(&format!("- {text}\n"));
                        }
                    }
                }
                body.push('\n');
            }
            "decisions_actions" => {
                body.push_str("## การตัดสินใจและงานที่ต้องทำ\n\n");
                if let Ok(items) = serde_json::from_str::<serde_json::Value>(content) {
                    for item in items.as_array().cloned().unwrap_or_default() {
                        let text = item.get("item").and_then(serde_json::Value::as_str).unwrap_or("");
                        let owner = item.get("owner").and_then(serde_json::Value::as_str);
                        match owner {
                            Some(owner) if !owner.is_empty() => body.push_str(&format!("- [ ] {text} — **{owner}**\n")),
                            _ => body.push_str(&format!("- [ ] {text}\n")),
                        }
                    }
                }
                body.push('\n');
            }
            _ => {}
        }
    }

    body.push_str("## Transcript ฉบับเต็ม\n\n");
    for segment in segments {
        body.push_str(&format!("- `{}` **{}**: {}\n", format_clock(segment.start_ms), segment.speaker, segment.text));
    }

    let exports_dir = std::path::PathBuf::from(&storage_path).join("exports");
    std::fs::create_dir_all(&exports_dir).map_err(|error| format!("create exports dir failed: {error}"))?;
    let file_path = exports_dir.join(format!("meeting-{}.md", &recording_id[..8.min(recording_id.len())]));
    std::fs::write(&file_path, body).map_err(|error| format!("write export failed: {error}"))?;

    let timestamp = now();
    genesis_adapter::commit_rows(storage, vec![genesis_adapter::upsert(
        "export_artifacts",
        // kind 'txt': the contract enum has no dedicated markdown kind yet.
        serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "kind": "txt", "file_path": file_path.display().to_string(), "source_layer_id": null, "created_at": timestamp}),
    )])?;

    Ok(file_path.display().to_string())
}

// ---------------------------------------------------------------------------
// UI-facing commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryRow {
    id: String,
    kind: String,
    content: String,
    evidence_count: usize,
    created_at: String,
}

#[tauri::command]
pub(crate) fn meeting_summaries(
    project_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SummaryRow>> {
    let mut rows = genesis_adapter::query(
        &state.genesis,
        "summaries",
        &["id", "kind", "content", "evidence_refs_json", "created_at"],
        vec![genesis_adapter::eq("summaries", "project_id", serde_json::json!(project_id))],
        200,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .map(|row| {
        let evidence_count = row
            .get("summaries.evidence_refs_json")
            .and_then(serde_json::Value::as_str)
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
            .map(|refs| refs.len())
            .or_else(|| {
                row.get("summaries.evidence_refs_json")
                    .and_then(serde_json::Value::as_array)
                    .map(|refs| refs.len())
            })
            .unwrap_or(0);
        Ok(SummaryRow {
            id: genesis_adapter::string(&row, "summaries.id").map_err(AppError::Genesis)?,
            kind: genesis_adapter::string(&row, "summaries.kind").map_err(AppError::Genesis)?,
            content: genesis_adapter::string(&row, "summaries.content").map_err(AppError::Genesis)?,
            evidence_count,
            created_at: genesis_adapter::string(&row, "summaries.created_at").map_err(AppError::Genesis)?,
        })
    })
    .collect::<AppResult<Vec<_>>>()?;
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(rows)
}

/// Manual retry surface for the post-meeting pipeline (e.g. Ollama was down
/// when the session ended).
#[tauri::command]
pub(crate) fn generate_meeting_summary(
    project_id: String,
    recording_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let storage = state.genesis.clone();
    std::thread::spawn(move || {
        run_post_meeting(&app, &storage, &project_id, &recording_id);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerant_json_recovers_object_wrapped_in_prose() {
        let value = tolerant_json("แน่นอนครับ นี่คือผลลัพธ์ {\"topic\": \"งบประมาณ\", \"openPoints\": []} หวังว่าช่วยได้");
        assert_eq!(value.get("topic").and_then(serde_json::Value::as_str), Some("งบประมาณ"));
    }

    #[test]
    fn contains_any_is_case_insensitive_and_counts_hits() {
        let keywords = vec!["Genesis".to_string(), "สัญญา".to_string()];
        assert_eq!(contains_any("คุยเรื่องสัญญาและ genesis กัน", &keywords), 2);
        assert_eq!(contains_any("ไม่มีอะไรเกี่ยวข้อง", &keywords), 0);
    }

    #[test]
    fn refs_map_to_segment_ids_and_dedupe() {
        let segments = vec![
            SegmentView { id: "seg-a".into(), speaker: "เรา".into(), start_ms: 0, text: "หนึ่ง".into() },
            SegmentView { id: "seg-b".into(), speaker: "อีกฝ่าย".into(), start_ms: 1000, text: "สอง".into() },
        ];
        let value = serde_json::json!({"refs": [0, 1, 1, 9]});
        let ids = refs_to_segment_ids(&value, "refs", &segments);
        assert_eq!(ids, vec!["seg-a".to_string(), "seg-b".to_string()]);
    }
}
