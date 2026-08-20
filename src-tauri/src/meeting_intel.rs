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

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::graph_build::{call_llm, llm_provider_config};
use crate::live_meeting::{RecentSegment, SharedRecent};
use crate::{genesis_adapter, now, AppError, AppResult, AppState};

/// GenesisBlockDB rejects any relational query with a limit above 1000 and
/// offers no cursor, so a project past this many summaries or a ledger past
/// this many model runs loses the tail. The previous read used 200, which
/// was below the engine's own ceiling for no stated reason.
const SUMMARY_QUERY_LIMIT: u32 = crate::genesis_adapter::ROW_CAP;

const TOPIC_INTERVAL: Duration = Duration::from_secs(45);
const TOPIC_WINDOW_SEGMENTS: usize = 40;
/// The relational engine caps every query at 1000 rows with no cursor
/// (see graph_build.rs); searches below inherit that cap and say so.
const ENGINE_ROW_CAP: u32 = crate::genesis_adapter::ROW_CAP;

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
            let Some(newest) = window.last() else {
                continue;
            };
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

    let project_names: std::collections::HashMap<String, String> = genesis_adapter::query(
        &state.genesis,
        "projects",
        &["id", "name"],
        vec![],
        ENGINE_ROW_CAP,
    )
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
        Some(id) => vec![genesis_adapter::eq(
            "transcript_segments",
            "project_id",
            serde_json::json!(id),
        )],
        None => vec![],
    };
    let segment_rows = genesis_adapter::query(
        &state.genesis,
        "transcript_segments",
        &[
            "id",
            "project_id",
            "recording_id",
            "start_ms",
            "text",
            "created_at",
        ],
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
                guard
                    .iter()
                    .rev()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
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
        format!(
            "\n\nบริบทที่กำลังคุยกันตอนนี้ (อ้างอิงไม่ได้ ใช้เข้าใจคำถามเท่านั้น):\n{}",
            render_window(&live_tail)
        )
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
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let cited: Vec<AskSource> = if refs.is_empty() {
        sources.into_iter().take(5).collect()
    } else {
        sources
            .into_iter()
            .filter(|source| refs.contains(&source.n))
            .collect()
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

#[derive(Debug)]
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
        vec![genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
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

    let page = genesis_adapter::query_capped(
        storage,
        "transcript_segments",
        &["id", "recording_id", "speaker_id", "start_ms", "text"],
        vec![genesis_adapter::eq(
            "transcript_segments",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    // Refused, not truncated. Everything downstream of this read goes into an
    // LLM prompt and comes back as "the meeting" — a narrative, a timeline, a
    // decisions list. A summary built on a transcript missing its last hour
    // does not look partial; it looks like a meeting that ended early, and it
    // is written into `summaries` with the same provenance as a complete one.
    // Of every consequence of this ceiling, that is the one nobody can spot
    // afterwards.
    if page.capped {
        return Err(format!(
            "การบันทึกนี้มีอย่างน้อย {} ท่อน ซึ่งเกินเพดานการอ่านครั้งเดียวของ storage engine              — สรุปที่ได้จะขาดช่วงท้ายโดยอ่านเหมือนสรุปครบ จึงไม่สรุปให้",
            genesis_adapter::ROW_CAP
        ));
    }
    let mut segments: Vec<SegmentView> = page
        .rows
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
            start_ms: row
                .get("transcript_segments.start_ms")
                .and_then(serde_json::Value::as_i64)?,
            text: row.get("transcript_segments.text")?.as_str()?.to_string(),
        })
    })
    .collect();
    segments.sort_by_key(|segment| segment.start_ms);
    Ok(segments)
}

fn refs_to_segment_ids(
    value: &serde_json::Value,
    key: &str,
    segments: &[SegmentView],
) -> Vec<String> {
    let mut ids: Vec<String> = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64())
                .filter_map(|index| {
                    segments
                        .get(index as usize)
                        .map(|segment| segment.id.clone())
                })
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

/// Queues post-meeting summarisation for a recording.
///
/// Previously this ran the whole pipeline inline on the caller's thread and
/// emitted the result directly, which meant a meeting that ended while
/// Ollama was down lost its summary with no retry and no record — the only
/// way back was the manual button. It now hands the work to the job engine,
/// which owns attempts, backoff, and survival across a restart, and which
/// emits the same `live-summary` states from the worker so the live panel's
/// contract is unchanged.
///
/// Emitting `running` here rather than waiting for the worker keeps the
/// panel honest about queued work: the user pressed stop and something is
/// pending, even if the worker is still finishing a previous job.
pub(crate) fn queue_post_meeting(
    app: &tauri::AppHandle,
    engine: &crate::job_engine::JobEngine,
    project_id: &str,
    recording_id: &str,
) {
    let emit = |state: &str, detail: Option<String>| {
        let _ = app.emit(
            "live-summary",
            LiveSummaryEvent {
                recording_id: recording_id.to_string(),
                state: state.to_string(),
                detail,
                export_path: None,
            },
        );
    };
    match engine.enqueue(
        crate::job_engine::JobKind::SummaryGenerate,
        project_id,
        Some(recording_id),
    ) {
        Ok(_) => emit("running", Some("กำลังสรุปการประชุม...".to_string())),
        // A queue that cannot accept work is a failure the user must see;
        // silently dropping it is how the summary went missing before.
        Err(error) => emit("failed", Some(format!("เข้าคิวสรุปไม่สำเร็จ: {error}"))),
    }
}

/// Deterministic id for one summary of one recording.
///
/// The `summaries` table has no `recording_id` column — the link runs through
/// `model_run_id` — so before the job engine every run minted a fresh UUID
/// and a retry *appended* a second recap beside the first rather than
/// replacing it. Pressing the manual "generate summary" button twice already
/// produced two, and a retrying engine would have made that routine.
///
/// Deriving the id from (project, recording, kind) makes the write an upsert
/// in fact as well as in name, which is what lets `summary.generate` be
/// registered as a retryable job at all. Mirrors `graph_build::det_node_id`.
///
/// This does not fix the *read* side: `meeting_summaries` is project-scoped,
/// so two recordings in one project still return both meetings' summaries
/// interleaved. That needs a schema column and is left alone here.
pub(crate) fn summary_row_id(project_id: &str, recording_id: &str, kind: &str) -> String {
    let digest = Sha256::digest(format!("{project_id}\u{1}{recording_id}\u{1}{kind}").as_bytes());
    let hex: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    format!("sum:{hex}")
}

/// Generates the three summary kinds, persists them with model provenance,
/// and writes the Markdown export. Returns the export path.
///
/// Owns no job row: its caller is the job engine, which owns status,
/// attempts, and retry. Also called directly by the `__debug_live_smoke`
/// harness, which reports through its own smoke log.
pub(crate) fn summarize_and_export(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<String, String> {
    let segments = load_segments(storage, project_id, recording_id)?;
    if segments.is_empty() {
        return Err(
            "ยังไม่มี transcript ของเซสชันนี้ — ถ้าถอดสดล้มเหลว ให้ถอดจากไฟล์ chunk ย้อนหลังก่อน".to_string(),
        );
    }
    let (endpoint, model) = llm_provider_config(storage)?;

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
        let story = story_value
            .get("story")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(raw.trim())
            .to_string();
        let story_refs = refs_to_segment_ids(&story_value, "refs", &segments);

        // 2) timeline — key points in order.
        let raw = call_llm(&endpoint, &model, &format!(
            "{shared_rules} รูปแบบ: {{\"points\": [{{\"point\": \"ประเด็นสำคัญ\", \"refs\": [3]}}]}} เรียงตามลำดับเวลา 4-10 ข้อ\n\nTranscript:\n{transcript_block}"
        ))?;
        let points_value = tolerant_json(&raw);
        let points = points_value
            .get("points")
            .cloned()
            .unwrap_or(serde_json::json!([]));
        let points_refs = collect_item_refs(&points, &segments);

        // 3) decisions_actions — decisions made + tasks owned.
        let raw = call_llm(&endpoint, &model, &format!(
            "{shared_rules} รูปแบบ: {{\"items\": [{{\"item\": \"งานหรือข้อตัดสินใจ\", \"owner\": \"ชื่อผู้รับผิดชอบถ้าระบุใน transcript ไม่งั้น null\", \"refs\": [5]}}]}}\n\nTranscript:\n{transcript_block}"
        ))?;
        let actions_value = tolerant_json(&raw);
        let actions = actions_value
            .get("items")
            .cloned()
            .unwrap_or(serde_json::json!([]));
        let actions_refs = collect_item_refs(&actions, &segments);

        // Persist: one model_run + one summary row per kind.
        let mut mutations = Vec::new();
        let mut persisted: Vec<(String, String)> = Vec::new(); // (kind, content)
        for (kind, content, refs) in [
            ("whole_story", story.clone(), story_refs),
            (
                "timeline",
                serde_json::to_string(&points).unwrap_or_else(|_| "[]".into()),
                points_refs,
            ),
            (
                "decisions_actions",
                serde_json::to_string(&actions).unwrap_or_else(|_| "[]".into()),
                actions_refs,
            ),
        ] {
            let timestamp = now();
            // Both ids are derived, not random, so a second attempt rewrites
            // the same two rows instead of leaving the first attempt's output
            // orphaned beside the second's.
            let summary_id = summary_row_id(project_id, recording_id, kind);
            let model_run_id = format!("run:{summary_id}");
            mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({
                "id": model_run_id, "recording_id": recording_id, "provider_id": "ollama-summary-intent",
                "model_name": model, "task_kind": format!("summary.generate:{kind}"), "runtime_location": "local",
                "input_ref": recording_id, "output_ref": format!("summary:{summary_id}"),
                "parameters_json": {"endpoint": endpoint}, "created_at": timestamp,
            })));
            mutations.push(genesis_adapter::upsert(
                "summaries",
                serde_json::json!({
                    "id": summary_id, "project_id": project_id, "kind": kind, "content": content,
                    "evidence_refs_json": refs, "model_run_id": model_run_id,
                    "created_at": timestamp, "updated_at": timestamp,
                }),
            ));
            persisted.push((kind.to_string(), content));
        }
        genesis_adapter::commit_rows(storage, mutations)?;

        // Markdown export closes the loop.
        let export_path =
            write_markdown_export(storage, project_id, recording_id, &persisted, &segments)?;
        Ok(export_path)
    })();

    result
}

fn format_clock(ms: i64) -> String {
    format!(
        "{}:{:02}:{:02}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60
    )
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
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
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
                        let text = item
                            .get("item")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        let owner = item.get("owner").and_then(serde_json::Value::as_str);
                        match owner {
                            Some(owner) if !owner.is_empty() => {
                                body.push_str(&format!("- [ ] {text} — **{owner}**\n"))
                            }
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
        body.push_str(&format!(
            "- `{}` **{}**: {}\n",
            format_clock(segment.start_ms),
            segment.speaker,
            segment.text
        ));
    }

    let exports_dir = std::path::PathBuf::from(&storage_path).join("exports");
    std::fs::create_dir_all(&exports_dir)
        .map_err(|error| format!("create exports dir failed: {error}"))?;
    let file_path = exports_dir.join(format!(
        "meeting-{}.md",
        &recording_id[..8.min(recording_id.len())]
    ));
    std::fs::write(&file_path, body).map_err(|error| format!("write export failed: {error}"))?;

    let timestamp = now();
    genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "export_artifacts",
            // kind 'txt': the contract enum has no dedicated markdown kind yet.
            serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "kind": "txt", "file_path": file_path.display().to_string(), "source_layer_id": null, "created_at": timestamp}),
        )],
    )?;

    Ok(file_path.display().to_string())
}

// ---------------------------------------------------------------------------
// UI-facing commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryRow {
    id: String,
    kind: String,
    content: String,
    evidence_count: usize,
    created_at: String,
    /// The recording this summary describes, resolved through its
    /// `model_runs` row. The `summaries` table has no such column, which is
    /// why the read used to be unable to say which meeting a recap came
    /// from.
    recording_id: String,
    /// True when a newer summary of the same kind exists for this recording.
    ///
    /// Ledgers written before summary ids became deterministic can hold
    /// several recaps of one meeting — the manual retry button appended one
    /// per press. They are reported rather than hidden or deleted: the user
    /// can see that an older version exists and which one is current.
    superseded: bool,
}

/// One recording's summaries, plus what the query deliberately left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingSummaries {
    /// Newest first; within a kind, only the first is not superseded.
    pub(crate) rows: Vec<SummaryRow>,
    /// Summaries in the same project attributed to a *different* recording.
    ///
    /// Reported rather than silently dropped: the panel previously showed
    /// every summary in the project as though they all belonged to the
    /// session on screen, so a user who now sees fewer rows is owed the
    /// reason.
    pub(crate) other_recordings: usize,
    /// Summaries in this project whose `model_runs` row could not be found,
    /// so no recording can be established for them. A partially-committed
    /// write leaves these behind, and they are invisible to every
    /// recording-scoped read — including this one — so the count is the
    /// only trace they have.
    ///
    /// Only meaningful when `attribution_complete`; see below.
    pub(crate) unattributable: usize,
    /// False when the `model_runs` read hit the engine's row ceiling.
    ///
    /// The ceiling means some runs were not read, so a summary counted as
    /// `unattributable` may in fact belong to a recording whose run simply
    /// fell off the end. Reporting the doubt is the only honest option:
    /// folding those rows into `other_recordings` would assert a recording
    /// this query never saw, and dropping the count would hide real orphans.
    pub(crate) attribution_complete: bool,
}

/// Number of evidence segment ids a summary cites.
///
/// The column round-trips as either a JSON array or a string holding one,
/// depending on how it was stored, so both shapes are read.
fn evidence_count(row: &serde_json::Value) -> usize {
    let Some(value) = row.get("summaries.evidence_refs_json") else {
        return 0;
    };
    if let Some(array) = value.as_array() {
        return array.len();
    }
    value
        .as_str()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .map(|refs| refs.len())
        .unwrap_or(0)
}

/// Splits a project's summaries into the ones belonging to one recording and
/// the ones that do not.
///
/// `runs_for_recording` holds the `model_runs` ids of the recording being
/// asked about; `known_runs` holds every `model_runs` id in the ledger,
/// which is what lets a genuinely orphaned summary be told apart from one
/// that simply belongs to another meeting. Collapsing those two into "not
/// mine" would hide a broken write behind a normal-looking count.
///
/// Pure, so the attribution rule — the part that decides whose recap the
/// user is shown — is testable without a ledger.
pub(crate) fn attribute_summaries(
    summary_rows: &[serde_json::Value],
    recording_id: &str,
    runs_for_recording: &HashSet<String>,
    known_runs: &HashSet<String>,
    attribution_complete: bool,
) -> MeetingSummaries {
    let mut rows = Vec::new();
    let mut other_recordings = 0usize;
    let mut unattributable = 0usize;

    for row in summary_rows {
        let text = |key: &str| {
            row.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let model_run_id = text("summaries.model_run_id");
        if !runs_for_recording.contains(&model_run_id) {
            if known_runs.contains(&model_run_id) {
                other_recordings += 1;
            } else {
                unattributable += 1;
            }
            continue;
        }
        rows.push(SummaryRow {
            id: text("summaries.id"),
            kind: text("summaries.kind"),
            content: text("summaries.content"),
            evidence_count: evidence_count(row),
            created_at: text("summaries.created_at"),
            recording_id: recording_id.to_string(),
            superseded: false,
        });
    }

    // Newest first overall, which also puts the current summary of each kind
    // ahead of its leftovers. Ties break on id so two writes landing in the
    // same second keep a stable order across calls instead of swapping.
    rows.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    let mut seen_kinds = HashSet::new();
    for row in &mut rows {
        row.superseded = !seen_kinds.insert(row.kind.clone());
    }

    MeetingSummaries {
        rows,
        other_recordings,
        unattributable,
        attribution_complete,
    }
}

/// Every summary of one recording, and nothing from any other.
///
/// This used to take only a project id and return every summary in it. With
/// one recording per project that was indistinguishable from correct; with
/// two, the live panel showed the previous meeting's recap beside the
/// current one and nothing in the response said which was which, because
/// `summaries` has no `recording_id` column to filter on.
///
/// Attribution runs through `model_runs`, which does carry the recording, in
/// a fixed three queries rather than one lookup per summary. Going through
/// the run rows also covers summaries written before ids became
/// deterministic, whose ids say nothing about which meeting they came from.
#[tauri::command]
pub(crate) fn meeting_summaries(
    project_id: String,
    recording_id: String,
    state: State<'_, AppState>,
) -> AppResult<MeetingSummaries> {
    let run_ids = |filters: Vec<_>| -> AppResult<HashSet<String>> {
        Ok(genesis_adapter::query(
            &state.genesis,
            "model_runs",
            &["id"],
            filters,
            SUMMARY_QUERY_LIMIT,
        )
        .map_err(AppError::Genesis)?
        .iter()
        .filter_map(|row| row.get("model_runs.id").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect())
    };

    let runs_for_recording = run_ids(vec![genesis_adapter::eq(
        "model_runs",
        "recording_id",
        serde_json::json!(recording_id),
    )])?;
    // Genesis filters are equality-only with no `IN` and no join, so the
    // second set is fetched whole and membership is decided in Rust. A
    // ledger with more model runs than the ceiling returns a partial set,
    // which is reported rather than quietly turning unread runs into
    // orphans.
    let known_runs = run_ids(vec![])?;
    let attribution_complete = (known_runs.len() as u32) < SUMMARY_QUERY_LIMIT;

    let summary_rows = genesis_adapter::query(
        &state.genesis,
        "summaries",
        &[
            "id",
            "kind",
            "content",
            "evidence_refs_json",
            "model_run_id",
            "created_at",
        ],
        vec![genesis_adapter::eq(
            "summaries",
            "project_id",
            serde_json::json!(project_id),
        )],
        SUMMARY_QUERY_LIMIT,
    )
    .map_err(AppError::Genesis)?;

    Ok(attribute_summaries(
        &summary_rows,
        &recording_id,
        &runs_for_recording,
        &known_runs,
        attribution_complete,
    ))
}

/// Manual retry surface for the post-meeting pipeline (e.g. Ollama was down
/// when the session ended).
///
/// Enqueues rather than spawning: the engine deduplicates, so pressing this
/// three times queues one summarisation instead of three racing writes to
/// the same rows.
#[tauri::command]
pub(crate) fn generate_meeting_summary(
    project_id: String,
    recording_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    queue_post_meeting(&app, &state.jobs, &project_id, &recording_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A summary row as the ledger returns it.
    fn summary(id: &str, kind: &str, run: &str, created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "summaries.id": id,
            "summaries.kind": kind,
            "summaries.content": format!("content of {id}"),
            "summaries.evidence_refs_json": ["seg-1", "seg-2"],
            "summaries.model_run_id": run,
            "summaries.created_at": created_at,
        })
    }

    fn ids(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn another_meeting_in_the_same_project_is_not_shown_as_this_one() {
        // The whole defect: `summaries` is keyed by project, so before this
        // the panel rendered the previous session's recap under the current
        // session's heading with nothing to tell them apart.
        let rows = [
            summary("s-mine", "whole_story", "run-mine", "2026-08-19T10:00:00Z"),
            summary(
                "s-theirs",
                "whole_story",
                "run-theirs",
                "2026-08-19T09:00:00Z",
            ),
        ];
        let result = attribute_summaries(
            &rows,
            "rec-mine",
            &ids(&["run-mine"]),
            &ids(&["run-mine", "run-theirs"]),
            true,
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].id, "s-mine");
        assert_eq!(result.rows[0].recording_id, "rec-mine");
        // Silently returning one row would look like the project only ever
        // had one summary. The count is what makes the filtering visible.
        assert_eq!(result.other_recordings, 1);
        assert_eq!(result.unattributable, 0);
    }

    #[test]
    fn a_summary_whose_model_run_is_gone_is_counted_not_ignored() {
        // A partially-committed write leaves a summary no recording-scoped
        // read can ever return. Folding it into `other_recordings` would
        // dress a broken write up as a normal one.
        let rows = [summary(
            "s-orphan",
            "timeline",
            "run-vanished",
            "2026-08-19T10:00:00Z",
        )];
        let result = attribute_summaries(
            &rows,
            "rec-mine",
            &ids(&["run-mine"]),
            &ids(&["run-mine"]),
            true,
        );
        assert!(result.rows.is_empty());
        assert_eq!(result.other_recordings, 0);
        assert_eq!(result.unattributable, 1);
    }

    #[test]
    fn older_duplicates_of_one_kind_are_marked_rather_than_dropped() {
        // Ledgers written before summary ids became deterministic hold one
        // recap per press of the retry button. The newest is current; the
        // rest stay visible so nothing is deleted behind the user's back.
        let rows = [
            summary("s-old", "whole_story", "run-a", "2026-08-19T09:00:00Z"),
            summary("s-new", "whole_story", "run-b", "2026-08-19T11:00:00Z"),
            summary("s-only", "timeline", "run-a", "2026-08-19T10:00:00Z"),
        ];
        let result = attribute_summaries(
            &rows,
            "rec-mine",
            &ids(&["run-a", "run-b"]),
            &ids(&["run-a", "run-b"]),
            true,
        );
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["s-new", "s-only", "s-old"],
            "newest first, regardless of kind"
        );
        let superseded = |id: &str| {
            result
                .rows
                .iter()
                .find(|row| row.id == id)
                .expect("row must be present")
                .superseded
        };
        assert!(!superseded("s-new"), "the newest recap is current");
        assert!(superseded("s-old"), "the older recap is a leftover");
        assert!(
            !superseded("s-only"),
            "the only summary of its kind is never superseded"
        );
    }

    #[test]
    fn two_summaries_written_in_the_same_second_keep_a_stable_order() {
        // Without the id tiebreak the two could swap between calls, which
        // would make `superseded` point at a different row each refresh.
        let rows = [
            summary("s-a", "whole_story", "run-a", "2026-08-19T10:00:00Z"),
            summary("s-b", "whole_story", "run-a", "2026-08-19T10:00:00Z"),
        ];
        let runs = ids(&["run-a"]);
        let first = attribute_summaries(&rows, "rec", &runs, &runs, true);
        let reversed: Vec<_> = rows.iter().rev().cloned().collect();
        let second = attribute_summaries(&reversed, "rec", &runs, &runs, true);
        assert_eq!(first.rows, second.rows);
    }

    #[test]
    fn a_truncated_model_run_read_says_so_instead_of_inventing_orphans() {
        // With the run table truncated, a summary belonging to a recording
        // whose run fell off the end is indistinguishable from a genuine
        // orphan. The count still reports it, but the flag says not to trust
        // it as evidence of a broken write.
        let rows = [summary(
            "s-other",
            "timeline",
            "run-unread",
            "2026-08-19T10:00:00Z",
        )];
        let result = attribute_summaries(
            &rows,
            "rec-mine",
            &ids(&["run-mine"]),
            &ids(&["run-mine"]),
            false,
        );
        assert_eq!(result.unattributable, 1);
        assert!(!result.attribution_complete);
    }

    #[test]
    fn evidence_counts_survive_both_shapes_the_column_takes() {
        let as_array = serde_json::json!({"summaries.evidence_refs_json": ["a", "b", "c"]});
        let as_string = serde_json::json!({"summaries.evidence_refs_json": "[\"a\",\"b\"]"});
        assert_eq!(evidence_count(&as_array), 3);
        assert_eq!(evidence_count(&as_string), 2);
        assert_eq!(evidence_count(&serde_json::json!({})), 0);
    }

    #[test]
    fn a_second_summary_run_rewrites_the_first_instead_of_adding_one() {
        // This is what makes summary.generate safe to retry. Before the ids
        // were derived, a retry appended a second recap and the project
        // showed both.
        let first = summary_row_id("proj", "rec", "whole_story");
        assert_eq!(first, summary_row_id("proj", "rec", "whole_story"));

        // Different recording, different project, and different kind must
        // never collide — a collision would overwrite another meeting's
        // summary with this one's.
        assert_ne!(first, summary_row_id("proj", "rec-2", "whole_story"));
        assert_ne!(first, summary_row_id("proj-2", "rec", "whole_story"));
        assert_ne!(first, summary_row_id("proj", "rec", "timeline"));
    }

    #[test]
    fn summary_ids_are_not_confusable_across_a_shifted_separator() {
        // "a" + "bc" and "ab" + "c" must not hash alike; the unit separator
        // is what prevents it, and a refactor that drops it would silently
        // start overwriting the wrong rows.
        assert_ne!(
            summary_row_id("a", "bc", "timeline"),
            summary_row_id("ab", "c", "timeline")
        );
    }

    #[test]
    fn tolerant_json_recovers_object_wrapped_in_prose() {
        let value = tolerant_json(
            "แน่นอนครับ นี่คือผลลัพธ์ {\"topic\": \"งบประมาณ\", \"openPoints\": []} หวังว่าช่วยได้",
        );
        assert_eq!(
            value.get("topic").and_then(serde_json::Value::as_str),
            Some("งบประมาณ")
        );
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
            SegmentView {
                id: "seg-a".into(),
                speaker: "เรา".into(),
                start_ms: 0,
                text: "หนึ่ง".into(),
            },
            SegmentView {
                id: "seg-b".into(),
                speaker: "อีกฝ่าย".into(),
                start_ms: 1000,
                text: "สอง".into(),
            },
        ];
        let value = serde_json::json!({"refs": [0, 1, 1, 9]});
        let ids = refs_to_segment_ids(&value, "refs", &segments);
        assert_eq!(ids, vec!["seg-a".to_string(), "seg-b".to_string()]);
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path =
            std::env::temp_dir().join(format!("fung-summary-cap-test-{}", uuid::Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    fn seed(storage: &genesis_block_native::Storage, segment_count: i64) {
        let mut rows = vec![
            genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"completed","duration_ms":0,"created_at":"t","updated_at":"t"})),
        ];
        for index in 0..segment_count {
            rows.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({
                "id": format!("s{index}"), "project_id": "p1", "recording_id": "r1",
                "speaker_id": null, "start_ms": index * 1000, "end_ms": index * 1000 + 900,
                "text": format!("บรรทัด {index}"), "confidence": 0.9,
                "created_at": "t", "updated_at": "t",
            })));
        }
        genesis_adapter::commit_rows(storage, rows).unwrap();
    }

    #[test]
    fn a_transcript_past_the_read_ceiling_is_refused_not_summarised() {
        // The defect: everything downstream of `load_segments` goes into an
        // LLM prompt and comes back as "the meeting". A summary built on a
        // transcript missing its last hour does not read as partial — it
        // reads as a meeting that ended early, and lands in `summaries` with
        // the same provenance as a complete one. A 3-hour session is roughly
        // 1500-2500 segments, so this was the normal case, not the edge one.
        let (path, storage) = open_storage();
        seed(&storage, genesis_adapter::ROW_CAP as i64 + 300);

        let error = load_segments(&storage, "p1", "r1").unwrap_err();
        assert!(
            error.contains(&genesis_adapter::ROW_CAP.to_string()),
            "the refusal must name the ceiling that caused it: {error}"
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_transcript_under_the_ceiling_still_loads_whole() {
        // The refusal must be the exception, not a new floor: the common
        // recording has to keep summarising exactly as before.
        let (path, storage) = open_storage();
        seed(&storage, 250);

        let segments = load_segments(&storage, "p1", "r1").unwrap();
        assert_eq!(segments.len(), 250);
        assert_eq!(segments.first().unwrap().start_ms, 0);
        assert_eq!(segments.last().unwrap().start_ms, 249_000);

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}
