//! Speaker attribution: merge per-participant whisper outputs (Path A) or
//! align diarization turns with a mixed transcript (Path B), then persist
//! speakers/segments/turns through genesis_adapter.

use crate::{genesis_adapter, now, WhisperOutput};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct AttributedSegment {
    pub(crate) speaker_key: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct SpeakerTurn {
    pub(crate) speaker_key: String,
    pub(crate) display_name: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) confidence: Option<f64>,
    pub(crate) overlap: bool,
}

pub(crate) fn merge_participant_outputs(outputs: Vec<(String, WhisperOutput)>) -> Vec<AttributedSegment> {
    let mut merged: Vec<AttributedSegment> = outputs.into_iter().flat_map(|(display_name, output)| {
        let key = format!("p:{}", display_name.trim().to_lowercase());
        output.segments.into_iter().map(move |segment| AttributedSegment {
            speaker_key: Some(key.clone()),
            display_name: Some(display_name.clone()),
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            text: segment.text,
            confidence: segment.confidence,
        }).collect::<Vec<_>>()
    }).collect();
    merged.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
    merged
}

pub(crate) fn group_turns(segments: &[AttributedSegment], gap_ms: i64) -> Vec<SpeakerTurn> {
    let mut turns: Vec<SpeakerTurn> = Vec::new();
    for segment in segments {
        let (Some(key), Some(name)) = (&segment.speaker_key, &segment.display_name) else { continue };
        let extend = turns.iter_mut().rev()
            .find(|turn| &turn.speaker_key == key)
            .filter(|turn| segment.start_ms - turn.end_ms <= gap_ms && segment.start_ms >= turn.start_ms);
        match extend {
            Some(turn) => {
                turn.end_ms = turn.end_ms.max(segment.end_ms);
                turn.confidence = match (turn.confidence, segment.confidence) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            None => turns.push(SpeakerTurn {
                speaker_key: key.clone(), display_name: name.clone(),
                start_ms: segment.start_ms, end_ms: segment.end_ms,
                confidence: segment.confidence, overlap: false,
            }),
        }
    }
    // Overlap pass: a turn overlaps when it intersects a different speaker's turn.
    let snapshot = turns.clone();
    for turn in &mut turns {
        turn.overlap = snapshot.iter().any(|other|
            other.speaker_key != turn.speaker_key
                && other.start_ms < turn.end_ms
                && turn.start_ms < other.end_ms);
    }
    turns.sort_by_key(|turn| (turn.start_ms, turn.end_ms));
    turns
}

/// Persists speakers (reused by key), transcript segments, proposed speaker
/// turns and diarization provenance, then marks the recording completed.
/// Deletes this recording's previously-persisted segments/proposed turns
/// first so a re-run replaces rather than duplicates.
pub(crate) fn persist_attribution(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    runtime_location: &str,
    model_name: &str,
    segments: &[AttributedSegment],
    turns: &[SpeakerTurn],
    duration_ms: i64,
) -> Result<(), String> {
    let timestamp = now();
    let mut mutations = Vec::new();
    for row in genesis_adapter::query(storage, "transcript_segments", &["id"],
        vec![genesis_adapter::eq("transcript_segments", "recording_id", serde_json::json!(recording_id))], 1000)? {
        mutations.push(genesis_adapter::delete("transcript_segments", &genesis_adapter::string(&row, "transcript_segments.id")?));
    }
    for row in genesis_adapter::query(storage, "speaker_turns", &["id", "status"],
        vec![genesis_adapter::eq("speaker_turns", "recording_id", serde_json::json!(recording_id))], 1000)? {
        if row.get("speaker_turns.status").and_then(serde_json::Value::as_str) == Some("proposed") {
            mutations.push(genesis_adapter::delete("speaker_turns", &genesis_adapter::string(&row, "speaker_turns.id")?));
        }
    }

    let provider_id = "fung-desktop-attribution";
    let model_run_id = Uuid::new_v4().to_string();
    mutations.push(genesis_adapter::upsert("model_providers", serde_json::json!({"id": provider_id, "label": "FUNG Desktop attribution", "runtime_location": runtime_location, "kind": "diarization", "enabled": true, "config_json": {}, "created_at": timestamp, "updated_at": timestamp})));
    mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": provider_id, "model_name": model_name, "task_kind": "diarization", "runtime_location": runtime_location, "input_ref": recording_id, "output_ref": format!("speaker-turns:{recording_id}"), "parameters_json": {}, "created_at": timestamp})));

    // Reuse speakers by key (same contract as mobile_diarization_import).
    let existing = genesis_adapter::query(storage, "speakers", &["id", "key", "created_at"],
        vec![genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id))], 500)?;
    let mut key_to_id = std::collections::HashMap::new();
    for row in &existing {
        if let (Some(key), Some(id)) = (
            row.get("speakers.key").and_then(serde_json::Value::as_str),
            row.get("speakers.id").and_then(serde_json::Value::as_str),
        ) { key_to_id.insert(key.to_string(), id.to_string()); }
    }
    let mut ensure_speaker = |key: &str, display_name: &str, confidence: Option<f64>, mutations: &mut Vec<_>| -> String {
        if let Some(id) = key_to_id.get(key) { return id.clone(); }
        let id = Uuid::new_v4().to_string();
        mutations.push(genesis_adapter::upsert("speakers", serde_json::json!({"id": id, "project_id": project_id, "key": key, "display_name": display_name, "confidence": confidence, "created_at": timestamp, "updated_at": timestamp})));
        key_to_id.insert(key.to_string(), id.clone());
        id
    };

    for segment in segments {
        let speaker_id = match (&segment.speaker_key, &segment.display_name) {
            (Some(key), Some(name)) => serde_json::json!(ensure_speaker(key, name, segment.confidence, &mut mutations)),
            _ => serde_json::Value::Null,
        };
        mutations.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": segment.start_ms, "end_ms": segment.end_ms, "text": segment.text, "confidence": segment.confidence, "created_at": timestamp, "updated_at": timestamp})));
    }
    for turn in turns {
        let speaker_id = ensure_speaker(&turn.speaker_key, &turn.display_name, turn.confidence, &mut mutations);
        mutations.push(genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": turn.start_ms, "end_ms": turn.end_ms, "confidence": turn.confidence, "status": "proposed", "model_run_id": model_run_id, "overlap": turn.overlap, "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }
    let recording = genesis_adapter::query(storage, "recordings", &["source", "input_path", "canonical_audio_path", "created_at"],
        vec![genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id))], 1)?
        .into_iter().next().ok_or_else(|| "recording not found".to_string())?;
    mutations.push(genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": genesis_adapter::string(&recording, "recordings.source")?, "input_path": recording.get("recordings.input_path").cloned().unwrap_or(serde_json::Value::Null), "canonical_audio_path": genesis_adapter::string(&recording, "recordings.canonical_audio_path")?, "status": "completed", "duration_ms": duration_ms, "created_at": genesis_adapter::string(&recording, "recordings.created_at")?, "updated_at": timestamp})));
    genesis_adapter::commit_rows(storage, mutations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WhisperOutput, WhisperSegment};

    fn seg(start: i64, end: i64, text: &str) -> WhisperSegment {
        WhisperSegment { start_ms: start, end_ms: end, text: text.to_string(), confidence: Some(0.9) }
    }

    #[test]
    fn merge_interleaves_participants_by_time() {
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(0, 2_000, "hello"), seg(6_000, 8_000, "bye")] }),
            ("ATHER".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(2_500, 5_000, "hi")] }),
        ]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].display_name.as_deref(), Some("Boss"));
        assert_eq!(merged[1].display_name.as_deref(), Some("ATHER"));
        assert_eq!(merged[1].speaker_key.as_deref(), Some("p:ather"));
        assert_eq!(merged[2].text, "bye");
    }

    #[test]
    fn group_turns_merges_within_gap_and_flags_overlap() {
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(0, 2_000, "a"), seg(2_800, 4_000, "b"), seg(9_000, 9_500, "c")] }),
            ("ATHER".to_string(), WhisperOutput { duration_ms: 10_000, segments: vec![seg(3_500, 6_000, "x")] }),
        ]);
        let turns = group_turns(&merged, 1_500);
        // Boss: [0..4000] (gap 800 <= 1500 merges) and [9000..9500]; ATHER: [3500..6000].
        assert_eq!(turns.len(), 3);
        let boss_first = turns.iter().find(|t| t.speaker_key == "p:boss" && t.start_ms == 0).unwrap();
        assert_eq!(boss_first.end_ms, 4_000);
        assert!(boss_first.overlap, "intersects ATHER 3500..6000");
        let boss_second = turns.iter().find(|t| t.speaker_key == "p:boss" && t.start_ms == 9_000).unwrap();
        assert!(!boss_second.overlap);
    }

    #[test]
    fn persist_attribution_reuses_speaker_keys_and_links_segments() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();
        let merged = merge_participant_outputs(vec![
            ("Boss".to_string(), WhisperOutput { duration_ms: 4_000, segments: vec![seg(0, 2_000, "a")] }),
        ]);
        let turns = group_turns(&merged, 1_500);
        persist_attribution(&storage, "p1", "r1", "local", "faster-whisper per-participant", &merged, &turns, 4_000).unwrap();
        // Run twice: speaker row must be reused, not duplicated.
        persist_attribution(&storage, "p1", "r1", "local", "faster-whisper per-participant", &merged, &turns, 4_000).unwrap();
        let speakers = crate::genesis_adapter::query(&storage, "speakers", &["id", "key"],
            vec![crate::genesis_adapter::eq("speakers", "project_id", serde_json::json!("p1"))], 10).unwrap();
        assert_eq!(speakers.len(), 1);
        let segments = crate::genesis_adapter::query(&storage, "transcript_segments", &["id", "speaker_id"],
            vec![crate::genesis_adapter::eq("transcript_segments", "project_id", serde_json::json!("p1"))], 10).unwrap();
        assert!(segments.iter().all(|row| row.get("transcript_segments.speaker_id").and_then(serde_json::Value::as_str).is_some()));
        drop(storage); let _ = std::fs::remove_dir_all(path);
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-merge-test-{}", uuid::Uuid::new_v4()));
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: path.display().to_string(), page_cache_mb: Some(16), read_only: Some(false), vector_dim: Some(4),
        }).unwrap();
        crate::genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }
}
