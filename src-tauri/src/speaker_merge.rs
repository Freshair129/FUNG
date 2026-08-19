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

/// `outputs` is `(file index, display name, transcription)`. The file index
/// is folded into the speaker key so two participants who share a Zoom
/// display name (e.g. two devices both named "iPhone") stay distinct
/// speakers instead of silently merging; the display name is still used
/// as-is for the label.
pub(crate) fn merge_participant_outputs(
    outputs: Vec<(usize, String, WhisperOutput)>,
) -> Vec<AttributedSegment> {
    let mut merged: Vec<AttributedSegment> = outputs
        .into_iter()
        .flat_map(|(index, display_name, output)| {
            let key = format!("p:{index}:{}", display_name.trim().to_lowercase());
            output
                .segments
                .into_iter()
                .map(move |segment| AttributedSegment {
                    speaker_key: Some(key.clone()),
                    display_name: Some(display_name.clone()),
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                    text: segment.text,
                    confidence: segment.confidence,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    merged.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
    merged
}

/// Marks each turn's `overlap` flag: true when its time range intersects a
/// different speaker's turn. Shared by Path A's grouped turns and Path B's
/// diarization turns so both routes compute overlap identically instead of
/// one of them hardcoding `false`.
pub(crate) fn compute_overlaps(turns: &mut [SpeakerTurn]) {
    let snapshot = turns.to_vec();
    for turn in turns.iter_mut() {
        turn.overlap = snapshot.iter().any(|other| {
            other.speaker_key != turn.speaker_key
                && other.start_ms < turn.end_ms
                && turn.start_ms < other.end_ms
        });
    }
}

pub(crate) fn group_turns(segments: &[AttributedSegment], gap_ms: i64) -> Vec<SpeakerTurn> {
    let mut turns: Vec<SpeakerTurn> = Vec::new();
    for segment in segments {
        let (Some(key), Some(name)) = (&segment.speaker_key, &segment.display_name) else {
            continue;
        };
        let extend = turns
            .iter_mut()
            .rev()
            .find(|turn| &turn.speaker_key == key)
            .filter(|turn| {
                segment.start_ms - turn.end_ms <= gap_ms && segment.start_ms >= turn.start_ms
            });
        match extend {
            Some(turn) => {
                turn.end_ms = turn.end_ms.max(segment.end_ms);
                turn.confidence = match (turn.confidence, segment.confidence) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            None => turns.push(SpeakerTurn {
                speaker_key: key.clone(),
                display_name: name.clone(),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                confidence: segment.confidence,
                overlap: false,
            }),
        }
    }
    compute_overlaps(&mut turns);
    turns.sort_by_key(|turn| (turn.start_ms, turn.end_ms));
    turns
}

/// Assigns each transcript segment the diarization turn with the largest
/// time overlap; segments overlapping nothing stay unassigned (speaker null).
pub(crate) fn assign_by_overlap(
    segments: &[AttributedSegment],
    turns: &[crate::zoom_sync::DiarizeTurn],
) -> Vec<AttributedSegment> {
    segments
        .iter()
        .map(|segment| {
            let best = turns
                .iter()
                .map(|turn| {
                    (
                        turn,
                        (segment.end_ms.min(turn.end_ms) - segment.start_ms.max(turn.start_ms))
                            .max(0),
                    )
                })
                .filter(|(_, overlap)| *overlap > 0)
                .max_by_key(|(_, overlap)| *overlap)
                .map(|(turn, _)| turn);
            AttributedSegment {
                speaker_key: best.map(|turn| turn.speaker_key.clone()),
                display_name: best.map(|turn| turn.display_name.clone()),
                ..segment.clone()
            }
        })
        .collect()
}

/// Deletes rows of `table` belonging to `recording_id`, paging around the
/// storage engine's 1000-row query ceiling by committing each page before
/// querying again. `deletable` decides which rows of a page to remove; a page
/// with nothing deletable ends the sweep.
fn delete_recording_rows(
    storage: &genesis_block_native::Storage,
    table: &str,
    recording_id: &str,
    columns: &[&str],
    deletable: impl Fn(&serde_json::Value) -> bool,
) -> Result<(), String> {
    let id_column = format!("{table}.id");
    loop {
        let rows = genesis_adapter::query(
            storage,
            table,
            columns,
            vec![genesis_adapter::eq(
                table,
                "recording_id",
                serde_json::json!(recording_id),
            )],
            1000,
        )?;
        if rows.is_empty() {
            return Ok(());
        }
        let mutations: Vec<_> = rows
            .iter()
            .filter(|row| deletable(row))
            .map(|row| {
                Ok(genesis_adapter::delete(
                    table,
                    &genesis_adapter::string(row, &id_column)?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // Every row on this page is retained, so no further page can be reached
        // by deleting — the sweep is done.
        if mutations.is_empty() {
            return Ok(());
        }
        genesis_adapter::commit_rows(storage, mutations)?;
    }
}

/// Persists speakers (reused by key), transcript segments, proposed speaker
/// turns and diarization provenance, then marks the recording completed.
/// Deletes this recording's previously-persisted segments/proposed turns
/// first so a re-run replaces rather than duplicates. The cleanup is
/// committed ahead of the insert batch — the engine has no way to delete an
/// unbounded set inside one transaction, so paging the deletes must commit
/// each page as it goes rather than joining the final `mutations` batch.
#[allow(clippy::too_many_arguments)]
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
    delete_recording_rows(
        storage,
        "transcript_segments",
        recording_id,
        &["id"],
        |_| true,
    )?;
    delete_recording_rows(
        storage,
        "speaker_turns",
        recording_id,
        &["id", "status"],
        |row| {
            row.get("speaker_turns.status")
                .and_then(serde_json::Value::as_str)
                == Some("proposed")
        },
    )?;

    let mut mutations = Vec::new();

    let provider_id = "fung-desktop-attribution";
    let model_run_id = Uuid::new_v4().to_string();
    mutations.push(genesis_adapter::upsert("model_providers", serde_json::json!({"id": provider_id, "label": "FUNG Desktop attribution", "runtime_location": runtime_location, "kind": "diarization", "enabled": true, "config_json": {}, "created_at": timestamp, "updated_at": timestamp})));
    mutations.push(genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": provider_id, "model_name": model_name, "task_kind": "diarization", "runtime_location": runtime_location, "input_ref": recording_id, "output_ref": format!("speaker-turns:{recording_id}"), "parameters_json": {}, "created_at": timestamp})));

    // Resolve one speaker per distinct key. Querying per key avoids listing a
    // whole project's speakers, which the engine would cap.
    let mut wanted: Vec<(String, String, Option<f64>)> = Vec::new();
    for segment in segments {
        if let (Some(key), Some(name)) = (&segment.speaker_key, &segment.display_name) {
            if !wanted.iter().any(|(existing, _, _)| existing == key) {
                wanted.push((key.clone(), name.clone(), segment.confidence));
            }
        }
    }
    for turn in turns {
        if !wanted
            .iter()
            .any(|(existing, _, _)| existing == &turn.speaker_key)
        {
            wanted.push((
                turn.speaker_key.clone(),
                turn.display_name.clone(),
                turn.confidence,
            ));
        }
    }

    let mut key_to_id: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (key, display_name, confidence) in &wanted {
        let existing = genesis_adapter::query(
            storage,
            "speakers",
            &["id"],
            vec![
                genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id)),
                genesis_adapter::eq("speakers", "key", serde_json::json!(key)),
            ],
            1,
        )?
        .into_iter()
        .next()
        .map(|row| genesis_adapter::string(&row, "speakers.id"))
        .transpose()?;
        match existing {
            Some(id) => {
                key_to_id.insert(key.clone(), id);
            }
            None => {
                let id = Uuid::new_v4().to_string();
                mutations.push(genesis_adapter::upsert(
                    "speakers",
                    serde_json::json!({
                        "id": id, "project_id": project_id, "key": key,
                        "display_name": display_name, "confidence": confidence,
                        "created_at": timestamp, "updated_at": timestamp,
                    }),
                ));
                key_to_id.insert(key.clone(), id);
            }
        }
    }

    for segment in segments {
        let speaker_id = match &segment.speaker_key {
            Some(key) => serde_json::json!(key_to_id.get(key).expect("resolved above")),
            None => serde_json::Value::Null,
        };
        mutations.push(genesis_adapter::upsert("transcript_segments", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": segment.start_ms, "end_ms": segment.end_ms, "text": segment.text, "confidence": segment.confidence, "created_at": timestamp, "updated_at": timestamp})));
    }
    for turn in turns {
        let speaker_id = key_to_id.get(&turn.speaker_key).expect("resolved above");
        mutations.push(genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": turn.start_ms, "end_ms": turn.end_ms, "confidence": turn.confidence, "status": "proposed", "model_run_id": model_run_id, "overlap": turn.overlap, "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }
    let recording = genesis_adapter::query(
        storage,
        "recordings",
        &["source", "input_path", "canonical_audio_path", "created_at"],
        vec![genesis_adapter::eq(
            "recordings",
            "id",
            serde_json::json!(recording_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "recording not found".to_string())?;
    mutations.push(genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": genesis_adapter::string(&recording, "recordings.source")?, "input_path": recording.get("recordings.input_path").cloned().unwrap_or(serde_json::Value::Null), "canonical_audio_path": genesis_adapter::string(&recording, "recordings.canonical_audio_path")?, "status": "completed", "duration_ms": duration_ms, "created_at": genesis_adapter::string(&recording, "recordings.created_at")?, "updated_at": timestamp})));
    genesis_adapter::commit_rows(storage, mutations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WhisperOutput, WhisperSegment};

    fn seg(start: i64, end: i64, text: &str) -> WhisperSegment {
        WhisperSegment {
            start_ms: start,
            end_ms: end,
            text: text.to_string(),
            confidence: Some(0.9),
        }
    }

    #[test]
    fn merge_interleaves_participants_by_time() {
        let merged = merge_participant_outputs(vec![
            (
                0,
                "Boss".to_string(),
                WhisperOutput {
                    duration_ms: 10_000,
                    segments: vec![seg(0, 2_000, "hello"), seg(6_000, 8_000, "bye")],
                },
            ),
            (
                1,
                "ATHER".to_string(),
                WhisperOutput {
                    duration_ms: 10_000,
                    segments: vec![seg(2_500, 5_000, "hi")],
                },
            ),
        ]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].display_name.as_deref(), Some("Boss"));
        assert_eq!(merged[1].display_name.as_deref(), Some("ATHER"));
        assert_eq!(merged[1].speaker_key.as_deref(), Some("p:1:ather"));
        assert_eq!(merged[2].text, "bye");
    }

    #[test]
    fn merge_keeps_same_display_name_distinct_by_file_index() {
        // Two Zoom participants can share a display name (e.g. two devices
        // both named "iPhone"); the file index must keep them distinct.
        let merged = merge_participant_outputs(vec![
            (
                0,
                "iPhone".to_string(),
                WhisperOutput {
                    duration_ms: 2_000,
                    segments: vec![seg(0, 1_000, "a")],
                },
            ),
            (
                1,
                "iPhone".to_string(),
                WhisperOutput {
                    duration_ms: 2_000,
                    segments: vec![seg(0, 1_000, "b")],
                },
            ),
        ]);
        assert_ne!(merged[0].speaker_key, merged[1].speaker_key);
        assert_eq!(merged[0].display_name.as_deref(), Some("iPhone"));
        assert_eq!(merged[1].display_name.as_deref(), Some("iPhone"));
    }

    #[test]
    fn group_turns_merges_within_gap_and_flags_overlap() {
        let merged = merge_participant_outputs(vec![
            (
                0,
                "Boss".to_string(),
                WhisperOutput {
                    duration_ms: 10_000,
                    segments: vec![
                        seg(0, 2_000, "a"),
                        seg(2_800, 4_000, "b"),
                        seg(9_000, 9_500, "c"),
                    ],
                },
            ),
            (
                1,
                "ATHER".to_string(),
                WhisperOutput {
                    duration_ms: 10_000,
                    segments: vec![seg(3_500, 6_000, "x")],
                },
            ),
        ]);
        let turns = group_turns(&merged, 1_500);
        // Boss: [0..4000] (gap 800 <= 1500 merges) and [9000..9500]; ATHER: [3500..6000].
        assert_eq!(turns.len(), 3);
        let boss_first = turns
            .iter()
            .find(|t| t.speaker_key == "p:0:boss" && t.start_ms == 0)
            .unwrap();
        assert_eq!(boss_first.end_ms, 4_000);
        assert!(boss_first.overlap, "intersects ATHER 3500..6000");
        let boss_second = turns
            .iter()
            .find(|t| t.speaker_key == "p:0:boss" && t.start_ms == 9_000)
            .unwrap();
        assert!(!boss_second.overlap);
    }

    #[test]
    fn assign_by_overlap_picks_dominant_turn_and_leaves_gaps_unassigned() {
        let segments = vec![
            AttributedSegment {
                speaker_key: None,
                display_name: None,
                start_ms: 0,
                end_ms: 2_000,
                text: "a".into(),
                confidence: None,
            },
            AttributedSegment {
                speaker_key: None,
                display_name: None,
                start_ms: 2_000,
                end_ms: 4_000,
                text: "b".into(),
                confidence: None,
            },
            AttributedSegment {
                speaker_key: None,
                display_name: None,
                start_ms: 8_000,
                end_ms: 9_000,
                text: "c".into(),
                confidence: None,
            },
        ];
        let turns = vec![
            crate::zoom_sync::DiarizeTurn {
                speaker_key: "s:0".into(),
                display_name: "Speaker 1".into(),
                start_ms: 0,
                end_ms: 2_500,
                confidence: None,
            },
            crate::zoom_sync::DiarizeTurn {
                speaker_key: "s:1".into(),
                display_name: "Speaker 2".into(),
                start_ms: 2_500,
                end_ms: 5_000,
                confidence: None,
            },
        ];
        let assigned = assign_by_overlap(&segments, &turns);
        assert_eq!(assigned[0].speaker_key.as_deref(), Some("s:0"));
        assert_eq!(assigned[1].speaker_key.as_deref(), Some("s:1")); // 1500ms overlap beats 500ms
        assert_eq!(assigned[2].speaker_key, None); // no overlap → unassigned
    }

    #[test]
    fn persist_attribution_reuses_speaker_keys_and_links_segments() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();
        let merged = merge_participant_outputs(vec![(
            0,
            "Boss".to_string(),
            WhisperOutput {
                duration_ms: 4_000,
                segments: vec![seg(0, 2_000, "a")],
            },
        )]);
        let turns = group_turns(&merged, 1_500);
        persist_attribution(
            &storage,
            "p1",
            "r1",
            "local",
            "faster-whisper per-participant",
            &merged,
            &turns,
            4_000,
        )
        .unwrap();
        // Run twice: speaker row must be reused, not duplicated.
        persist_attribution(
            &storage,
            "p1",
            "r1",
            "local",
            "faster-whisper per-participant",
            &merged,
            &turns,
            4_000,
        )
        .unwrap();
        let speakers = crate::genesis_adapter::query(
            &storage,
            "speakers",
            &["id", "key"],
            vec![crate::genesis_adapter::eq(
                "speakers",
                "project_id",
                serde_json::json!("p1"),
            )],
            10,
        )
        .unwrap();
        assert_eq!(speakers.len(), 1);
        let segments = crate::genesis_adapter::query(
            &storage,
            "transcript_segments",
            &["id", "speaker_id"],
            vec![crate::genesis_adapter::eq(
                "transcript_segments",
                "project_id",
                serde_json::json!("p1"),
            )],
            10,
        )
        .unwrap();
        assert!(segments.iter().all(|row| row
            .get("transcript_segments.speaker_id")
            .and_then(serde_json::Value::as_str)
            .is_some()));
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn persist_attribution_replaces_more_segments_than_one_query_page() {
        let (path, storage) = open_storage();
        crate::genesis_adapter::commit_rows(&storage, vec![
            crate::genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":"s","active_recording_id":null,"created_at":"t","updated_at":"t"})),
            crate::genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":"c","status":"pending","duration_ms":0,"created_at":"t","updated_at":"t"})),
        ]).unwrap();

        // 1200 segments — past the engine's 1000-row query ceiling.
        let mut output = WhisperOutput {
            duration_ms: 1_200_000,
            segments: Vec::new(),
        };
        for index in 0..1200i64 {
            output.segments.push(WhisperSegment {
                start_ms: index * 1000,
                end_ms: index * 1000 + 900,
                text: format!("line {index}"),
                confidence: Some(0.9),
            });
        }
        let merged = merge_participant_outputs(vec![(0, "Boss".to_string(), output)]);
        let turns = group_turns(&merged, 1_500);
        persist_attribution(
            &storage, "p1", "r1", "local", "test", &merged, &turns, 1_200_000,
        )
        .unwrap();
        persist_attribution(
            &storage, "p1", "r1", "local", "test", &merged, &turns, 1_200_000,
        )
        .unwrap();

        // A second run must replace, not append. A single 1000-row query cannot
        // see the whole table when the bug duplicates rows (1200 kept + 1200
        // fresh = 2400), so count the exact total by repeatedly counting and
        // deleting a page until the table is empty.
        let mut total = 0usize;
        loop {
            let page = crate::genesis_adapter::query(
                &storage,
                "transcript_segments",
                &["id"],
                vec![crate::genesis_adapter::eq(
                    "transcript_segments",
                    "recording_id",
                    serde_json::json!("r1"),
                )],
                1000,
            )
            .unwrap();
            if page.is_empty() {
                break;
            }
            total += page.len();
            let deletes: Vec<_> = page
                .iter()
                .map(|row| {
                    crate::genesis_adapter::delete(
                        "transcript_segments",
                        &crate::genesis_adapter::string(row, "transcript_segments.id").unwrap(),
                    )
                })
                .collect();
            crate::genesis_adapter::commit_rows(&storage, deletes).unwrap();
        }
        assert_eq!(
            total, 1200,
            "second run must replace the 1200 segments exactly, not accumulate duplicates"
        );

        let speakers = crate::genesis_adapter::query(
            &storage,
            "speakers",
            &["id"],
            vec![crate::genesis_adapter::eq(
                "speakers",
                "project_id",
                serde_json::json!("p1"),
            )],
            100,
        )
        .unwrap();
        assert_eq!(speakers.len(), 1, "speaker rows must be reused across runs");

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-merge-test-{}", uuid::Uuid::new_v4()));
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
}
