//! Speaker diarization for a locally captured meeting.
//!
//! Until now diarization was reachable only from Zoom mixed-audio import,
//! which hands the worker one file whose timeline *is* the recording's. A
//! local capture is not that: it is a directory of per-channel chunks, and
//! turning those into something a diarizer can read forces two decisions
//! that change what the answer means.
//!
//! # Which audio
//!
//! The system channel alone, never a mix of both.
//!
//! FUNG already knows, from capture provenance rather than inference, that
//! the microphone is us. Mixing that channel back in and asking a model to
//! re-derive it would replace a fact with a guess, and the guess is the one
//! that can be wrong. What is genuinely unknown is which of the *remote*
//! participants is speaking, and that is entirely in the system channel.
//!
//! So microphone segments keep the attribution they already have and are not
//! touched. Only segments attributed to the far side are refined, from one
//! "อีกฝ่าย" into the individual speakers behind it.
//!
//! The cost is stated rather than hidden: two people sharing one microphone
//! in the same room stay a single speaker. Splitting them needs a true
//! mixdown, and `transcribe.py --concat-only` concatenates rather than mixes,
//! so that is a different feature and not this one.
//!
//! # Which timeline
//!
//! Concatenating chunks produces a file whose offsets are *not* the
//! recording's timestamps. Chunk three begins in the file at the sum of the
//! two before it, but begins in the recording at whatever `start_ms` says —
//! and those diverge the moment a chunk is dropped, which is exactly what
//! happens in the degraded captures this feature is most useful for.
//!
//! Every turn the worker returns is therefore projected back through
//! [`ChunkSpan`]s before it is believed. A turn that straddles a gap is split
//! at the boundary rather than stretched across it, because a turn stretched
//! over missing audio asserts that someone was speaking during a period no
//! recording exists for.

use crate::speaker_merge::SpeakerTurn;
use crate::zoom_sync::DiarizeTurn;

/// Where one chunk sits in both timelines.
///
/// `file_*` is the offset inside the concatenated WAV, measured from the
/// chunks' real decoded durations rather than their ledger `end_ms - start_ms`
/// — the file contains audio, so only the audio can say how long it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChunkSpan {
    pub(crate) file_start_ms: i64,
    pub(crate) file_end_ms: i64,
    pub(crate) recording_start_ms: i64,
}

impl ChunkSpan {
    fn to_recording(self, file_ms: i64) -> i64 {
        self.recording_start_ms + (file_ms - self.file_start_ms)
    }
}

/// Lays chunks end to end, recording where each lands in the concatenated
/// file and where it belongs in the recording.
///
/// `chunks` is `(recording_start_ms, decoded_duration_ms)` in playback order.
/// A chunk of zero or negative duration contributes nothing and is skipped
/// rather than producing an empty span that later maps every turn onto it.
pub(crate) fn plan_spans(chunks: &[(i64, i64)]) -> Vec<ChunkSpan> {
    let mut spans = Vec::with_capacity(chunks.len());
    let mut cursor = 0i64;
    for &(recording_start_ms, duration_ms) in chunks {
        if duration_ms <= 0 {
            continue;
        }
        spans.push(ChunkSpan {
            file_start_ms: cursor,
            file_end_ms: cursor + duration_ms,
            recording_start_ms,
        });
        cursor += duration_ms;
    }
    spans
}

/// Total audio the spans expect the concatenated file to hold.
pub(crate) fn concatenated_duration_ms(spans: &[ChunkSpan]) -> i64 {
    spans.last().map(|span| span.file_end_ms).unwrap_or(0)
}

/// How far the produced file may differ from the planned length before the
/// projection stops being trustworthy.
///
/// Decoding and resampling round by well under a millisecond per chunk, so
/// the allowance is generous for that. What it is really guarding against is
/// a chunk that failed to decode and was dropped, which shortens the file by
/// that chunk's entire duration — hundreds or thousands of milliseconds —
/// and shifts every turn after it. Labels that are confidently in the wrong
/// place are worse than no labels, so a mismatch declines rather than
/// proceeds.
pub(crate) fn duration_tolerance_ms(chunk_count: usize) -> i64 {
    100 + 2 * chunk_count as i64
}

/// Translates diarization turns from file time into recording time.
///
/// A turn is clipped to the spans it actually covers. Anything outside them
/// is dropped — the worker occasionally reports a turn running a few
/// milliseconds past the end of the audio, and honouring that would place
/// speech after the recording stopped.
///
/// Pieces of one turn that meet exactly in recording time are rejoined, so a
/// speaker talking across two contiguous chunks stays one turn rather than
/// being fragmented by an implementation detail of how the file was built.
pub(crate) fn project_turns(turns: &[DiarizeTurn], spans: &[ChunkSpan]) -> Vec<SpeakerTurn> {
    let mut projected: Vec<SpeakerTurn> = Vec::new();

    for turn in turns {
        // Guard against a worker that reports end before start; a negative
        // range would silently match nothing and lose the turn.
        let (turn_start, turn_end) = if turn.start_ms <= turn.end_ms {
            (turn.start_ms, turn.end_ms)
        } else {
            (turn.end_ms, turn.start_ms)
        };

        for span in spans {
            let overlap_start = turn_start.max(span.file_start_ms);
            let overlap_end = turn_end.min(span.file_end_ms);
            if overlap_start >= overlap_end {
                continue;
            }
            let start_ms = span.to_recording(overlap_start);
            let end_ms = span.to_recording(overlap_end);

            // Rejoin only with the immediately preceding piece, and only when
            // it is the same speaker meeting this one exactly. Merging across
            // a gap would re-assert the continuity the split just removed.
            match projected.last_mut() {
                Some(previous)
                    if previous.speaker_key == turn.speaker_key && previous.end_ms == start_ms =>
                {
                    previous.end_ms = end_ms;
                }
                _ => projected.push(SpeakerTurn {
                    speaker_key: turn.speaker_key.clone(),
                    display_name: turn.display_name.clone(),
                    start_ms,
                    end_ms,
                    confidence: turn.confidence,
                    // Computed across the whole set by `compute_overlaps`
                    // once projection is done; a per-piece guess here would
                    // be wrong for any turn that was split.
                    overlap: false,
                }),
            }
        }
    }

    projected.sort_by_key(|turn| (turn.start_ms, turn.end_ms));
    projected
}

/// What a local diarization run achieved.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalDiarizationOutcome {
    /// Distinct speakers the model found on the far side.
    pub(crate) speakers_found: usize,
    /// Turns after projection back onto the recording's timeline.
    pub(crate) turns: usize,
    /// Segments whose speaker changed.
    pub(crate) segments_reattributed: usize,
    /// Far-side segments no turn covered, which keep the label they had.
    pub(crate) segments_unchanged: usize,
    /// Set when the pass declined to run, rather than reporting zero work as
    /// success.
    pub(crate) skipped_reason: Option<String>,
}

/// The channel diarization is run over. The microphone is us by capture
/// provenance; only the far side is unknown. See the module docs.
const SUBJECT_CHANNEL: &str = crate::live_meeting::CHANNEL_SYSTEM;

/// The storage engine caps one query at 1000 rows and offers no cursor, so a
/// recording past this many chunks or segments loses its tail. Stated at the
/// call sites below rather than silently truncated.
const QUERY_LIMIT: u32 = crate::genesis_adapter::ROW_CAP;

/// Diarizes a locally captured recording's far-side audio and re-labels the
/// segments it covers.
///
/// Steps, each of which can decline: gather the system channel's chunks,
/// concatenate them into one file, diarize that file, project the turns back
/// onto the recording's timeline, then rewrite only the affected segments'
/// speaker — never their ids, because summaries and graph edges cite those.
pub(crate) fn diarize_recording(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    data_root: &std::path::Path,
    project_id: &str,
    recording_id: &str,
    on_progress: impl Fn(i64) + Send + Sync + 'static,
) -> LocalDiarizationOutcome {
    let declined = |reason: String| LocalDiarizationOutcome {
        skipped_reason: Some(reason),
        ..LocalDiarizationOutcome::default()
    };

    let readiness = crate::diarization::probe(runtime, data_root);
    if let Some(blocker) = readiness.blocker {
        return declined(format!("{}: {}", blocker.code(), blocker.detail()));
    }

    let chunks = match system_chunks(storage, recording_id) {
        Ok(chunks) => chunks,
        Err(reason) => return declined(reason),
    };
    if chunks.is_empty() {
        return declined(
            "ไม่พบไฟล์เสียงฝั่งอีกฝ่ายของการบันทึกนี้ — แยกเสียงผู้พูดได้เฉพาะเสียงจากระบบ".to_string(),
        );
    }

    let spans = plan_spans(
        &chunks
            .iter()
            .map(|chunk| (chunk.start_ms, chunk.duration_ms))
            .collect::<Vec<_>>(),
    );
    if spans.is_empty() {
        return declined("ไฟล์เสียงฝั่งอีกฝ่ายอ่านความยาวไม่ได้".to_string());
    }

    // A directory FUNG owns, beside the audio it is built from, so a crash
    // leaves the temporary file somewhere the user can find and delete.
    let work_dir = data_root.join("diarization").join(recording_id);
    if let Err(error) = std::fs::create_dir_all(&work_dir) {
        return declined(format!("สร้างโฟลเดอร์ทำงานไม่ได้: {error}"));
    }
    let manifest_path = work_dir.join("chunks.txt");
    let concat_path = work_dir.join("far-side.wav");
    let manifest = chunks
        .iter()
        .map(|chunk| chunk.file_path.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(error) = std::fs::write(&manifest_path, manifest) {
        return declined(format!("เขียน manifest ไม่ได้: {error}"));
    }

    let concat = crate::run_python_worker(
        runtime,
        &runtime.script,
        &[
            "--manifest",
            &manifest_path.display().to_string(),
            "--concat-only",
            &concat_path.display().to_string(),
        ],
        None,
        None,
        move |pct| on_progress(pct * 30 / 100),
    );
    if let Err(error) = concat {
        cleanup(&work_dir);
        return declined(format!("รวมไฟล์เสียงไม่สำเร็จ: {error}"));
    }

    // The projection is only as good as the assumption that the file holds
    // exactly the audio the spans describe. Check it rather than trust it: a
    // chunk that silently failed to decode would shift every turn after it.
    let planned_ms = concatenated_duration_ms(&spans);
    let Some(actual_ms) = crate::recovery::wav_duration_ms(&concat_path) else {
        cleanup(&work_dir);
        return declined("อ่านความยาวไฟล์เสียงที่รวมแล้วไม่ได้".to_string());
    };
    let drift_ms = (actual_ms - planned_ms).abs();
    if drift_ms > duration_tolerance_ms(spans.len()) {
        cleanup(&work_dir);
        return declined(format!(
            "ความยาวไฟล์เสียงที่รวมแล้วไม่ตรงกับที่คำนวณไว้ ({actual_ms} ms เทียบกับ {planned_ms} ms)              — เวลาของผู้พูดจะคลาดเคลื่อน จึงไม่แยกเสียงผู้พูด"
        ));
    }

    let diarized = crate::run_diarization(
        runtime,
        data_root,
        &concat_path.display().to_string(),
        |_| {},
    );
    // The concatenated audio is a derivative of chunks that are still on
    // disk, so it is removed either way rather than left to accumulate a
    // second copy of every meeting.
    cleanup(&work_dir);
    let diarized = match diarized {
        Ok(diarized) => diarized,
        Err(error) => return declined(error),
    };

    let mut turns = project_turns(&diarized.turns, &spans);
    crate::speaker_merge::compute_overlaps(&mut turns);
    let speakers_found = turns
        .iter()
        .map(|turn| turn.speaker_key.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();

    match apply_turns(storage, project_id, recording_id, &turns) {
        Ok((reattributed, unchanged)) => {
            let outcome = LocalDiarizationOutcome {
                speakers_found,
                turns: turns.len(),
                segments_reattributed: reattributed,
                segments_unchanged: unchanged,
                skipped_reason: None,
            };
            let _ = tauri::Emitter::emit(app, "local-diarization", &outcome);
            outcome
        }
        Err(reason) => declined(reason),
    }
}

fn cleanup(work_dir: &std::path::Path) {
    if let Err(error) = std::fs::remove_dir_all(work_dir) {
        // Losing the temporary audio matters less than losing the result, so
        // this is reported and not raised.
        eprintln!("[diarize] could not remove {}: {error}", work_dir.display());
    }
}

/// One far-side chunk with the duration its audio actually has.
struct SystemChunk {
    file_path: String,
    start_ms: i64,
    duration_ms: i64,
}

/// The recording's system-channel chunks, in playback order, with unreadable
/// files left out.
fn system_chunks(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
) -> Result<Vec<SystemChunk>, String> {
    let rows = crate::genesis_adapter::query(
        storage,
        "audio_chunks",
        &["file_path", "start_ms", "sequence_no"],
        vec![crate::genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        QUERY_LIMIT,
    )?;
    if rows.len() as u32 >= QUERY_LIMIT {
        return Err(format!(
            "การบันทึกนี้มีมากกว่า {QUERY_LIMIT} ช่วงเสียง — เกินกว่าที่จะอ่านได้ในคำสั่งเดียว"
        ));
    }

    let mut chunks: Vec<(i64, SystemChunk)> = Vec::new();
    for row in &rows {
        let file_path = row
            .get("audio_chunks.file_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let name = std::path::Path::new(&file_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if crate::live_meeting::channel_for_file_name(&name) != Some(SUBJECT_CHANNEL) {
            continue;
        }
        // Duration comes from the WAV header, not `end_ms - start_ms`: the
        // concatenated file holds audio, so only the audio can say how long
        // each piece of it is, and a mismatch here shifts every later turn.
        let Some(duration_ms) = crate::recovery::wav_duration_ms(std::path::Path::new(&file_path))
        else {
            continue;
        };
        let sequence_no = row
            .get("audio_chunks.sequence_no")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        chunks.push((
            sequence_no,
            SystemChunk {
                file_path,
                start_ms: row
                    .get("audio_chunks.start_ms")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                duration_ms,
            },
        ));
    }
    // Ordered by recorded time rather than sequence number: the two agree in
    // a healthy capture, and where they disagree the timeline is what the
    // projection depends on.
    chunks.sort_by_key(|(sequence_no, chunk)| (chunk.start_ms, *sequence_no));
    Ok(chunks.into_iter().map(|(_, chunk)| chunk).collect())
}

/// Rewrites the speaker of far-side segments the turns cover, and records the
/// turns with their provenance.
///
/// Segment rows are updated in place — same id, same text, same
/// `created_at`. `persist_attribution` deletes and re-inserts, which mints
/// new ids; doing that here would orphan every summary evidence ref and
/// graph edge that cites a segment, and those exist by the time anyone asks
/// for diarization after the fact.
fn apply_turns(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    turns: &[crate::speaker_merge::SpeakerTurn],
) -> Result<(usize, usize), String> {
    let subject_speaker_id = crate::live_meeting::speaker_id_for(project_id, "them");
    let rows = crate::genesis_adapter::query(
        storage,
        "transcript_segments",
        &[
            "id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "text",
            "confidence",
            "created_at",
        ],
        vec![crate::genesis_adapter::eq(
            "transcript_segments",
            "recording_id",
            serde_json::json!(recording_id),
        )],
        QUERY_LIMIT,
    )?;
    if rows.len() as u32 >= QUERY_LIMIT {
        return Err(format!(
            "การบันทึกนี้มีมากกว่า {QUERY_LIMIT} ช่วงข้อความ — เกินกว่าที่จะอ่านได้ในคำสั่งเดียว"
        ));
    }

    let existing: Vec<crate::speaker_merge::ExistingSegment> = rows
        .iter()
        .map(|row| crate::speaker_merge::ExistingSegment {
            id: row
                .get("transcript_segments.id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            speaker_id: row
                .get("transcript_segments.speaker_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            start_ms: row
                .get("transcript_segments.start_ms")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            end_ms: row
                .get("transcript_segments.end_ms")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
        })
        .collect();
    let far_side = existing
        .iter()
        .filter(|segment| segment.speaker_id.as_deref() == Some(subject_speaker_id.as_str()))
        .count();
    let plan = crate::speaker_merge::plan_reattribution(&existing, &subject_speaker_id, turns);

    // Diarization is a proposal, and re-running it produces a new one. The
    // previous proposal is removed first so a second run replaces rather than
    // doubles it; turns a person has confirmed are not `proposed` and are
    // left alone.
    crate::speaker_merge::delete_recording_rows(
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

    let timestamp = crate::now();
    let mut mutations = Vec::new();
    let provider_id = "fung-desktop-attribution";
    let model_run_id = uuid::Uuid::new_v4().to_string();
    mutations.push(crate::genesis_adapter::upsert("model_providers", serde_json::json!({"id": provider_id, "label": "FUNG Desktop attribution", "runtime_location": "local", "kind": "diarization", "enabled": true, "config_json": {}, "created_at": timestamp, "updated_at": timestamp})));
    mutations.push(crate::genesis_adapter::upsert("model_runs", serde_json::json!({"id": model_run_id, "recording_id": recording_id, "provider_id": provider_id, "model_name": crate::diarization::DIARIZATION_MODEL, "task_kind": "diarization", "runtime_location": "local", "input_ref": format!("channel:{SUBJECT_CHANNEL}"), "output_ref": format!("speaker-turns:{recording_id}"), "parameters_json": {"channel": SUBJECT_CHANNEL}, "created_at": timestamp})));

    // One speaker row per distinct key, reused across runs so a renamed
    // "Speaker 2" keeps its name when diarization is run again.
    let mut key_to_id: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for turn in turns {
        if key_to_id.contains_key(&turn.speaker_key) {
            continue;
        }
        let existing_id = crate::genesis_adapter::query(
            storage,
            "speakers",
            &["id"],
            vec![
                crate::genesis_adapter::eq("speakers", "project_id", serde_json::json!(project_id)),
                crate::genesis_adapter::eq("speakers", "key", serde_json::json!(turn.speaker_key)),
            ],
            1,
        )?
        .into_iter()
        .next()
        .and_then(|row| {
            row.get("speakers.id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
        let id = match existing_id {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                mutations.push(crate::genesis_adapter::upsert(
                    "speakers",
                    serde_json::json!({
                        "id": id, "project_id": project_id, "key": turn.speaker_key,
                        "display_name": turn.display_name, "confidence": turn.confidence,
                        "created_at": timestamp, "updated_at": timestamp,
                    }),
                ));
                id
            }
        };
        key_to_id.insert(turn.speaker_key.clone(), id);
    }

    let by_id: std::collections::HashMap<&str, &serde_json::Value> = rows
        .iter()
        .filter_map(|row| Some((row.get("transcript_segments.id")?.as_str()?, row)))
        .collect();
    for (segment_id, speaker_key) in &plan {
        let Some(row) = by_id.get(segment_id.as_str()) else {
            continue;
        };
        let speaker_id = key_to_id.get(speaker_key).expect("resolved above");
        let field = |key: &str| row.get(key).cloned().unwrap_or(serde_json::Value::Null);
        mutations.push(crate::genesis_adapter::upsert(
            "transcript_segments",
            serde_json::json!({
                "id": segment_id,
                "project_id": project_id,
                "recording_id": recording_id,
                "speaker_id": speaker_id,
                "start_ms": field("transcript_segments.start_ms"),
                "end_ms": field("transcript_segments.end_ms"),
                "text": field("transcript_segments.text"),
                "confidence": field("transcript_segments.confidence"),
                "created_at": field("transcript_segments.created_at"),
                "updated_at": timestamp,
            }),
        ));
    }

    for turn in turns {
        let speaker_id = key_to_id.get(&turn.speaker_key).expect("resolved above");
        mutations.push(crate::genesis_adapter::upsert("speaker_turns", serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id, "start_ms": turn.start_ms, "end_ms": turn.end_ms, "confidence": turn.confidence, "status": "proposed", "model_run_id": model_run_id, "overlap": turn.overlap, "revision": 1, "created_at": timestamp, "updated_at": timestamp})));
    }

    crate::genesis_adapter::commit_rows(storage, mutations)?;
    Ok((plan.len(), far_side.saturating_sub(plan.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(speaker: &str, start_ms: i64, end_ms: i64) -> DiarizeTurn {
        DiarizeTurn {
            speaker_key: speaker.to_string(),
            display_name: format!("Speaker {speaker}"),
            start_ms,
            end_ms,
            confidence: None,
        }
    }

    #[test]
    fn contiguous_chunks_lay_end_to_end() {
        let spans = plan_spans(&[(0, 2_000), (2_000, 2_000), (4_000, 1_500)]);
        assert_eq!(
            spans,
            vec![
                ChunkSpan {
                    file_start_ms: 0,
                    file_end_ms: 2_000,
                    recording_start_ms: 0
                },
                ChunkSpan {
                    file_start_ms: 2_000,
                    file_end_ms: 4_000,
                    recording_start_ms: 2_000
                },
                ChunkSpan {
                    file_start_ms: 4_000,
                    file_end_ms: 5_500,
                    recording_start_ms: 4_000
                },
            ]
        );
        assert_eq!(concatenated_duration_ms(&spans), 5_500);
    }

    #[test]
    fn a_dropped_chunk_moves_file_time_away_from_recording_time() {
        // The whole reason projection exists. Chunk three is recorded at
        // 8s but sits at 4s in the file, because the chunk between them was
        // lost. Reading file offsets as timestamps would place its speech
        // four seconds early.
        let spans = plan_spans(&[(0, 2_000), (2_000, 2_000), (8_000, 2_000)]);
        assert_eq!(spans[2].file_start_ms, 4_000);
        assert_eq!(spans[2].recording_start_ms, 8_000);
    }

    #[test]
    fn a_turn_inside_one_chunk_shifts_by_that_chunks_offset() {
        let spans = plan_spans(&[(0, 2_000), (10_000, 2_000)]);
        let projected = project_turns(&[turn("s:0", 2_500, 3_000)], &spans);
        assert_eq!(projected.len(), 1);
        assert_eq!(
            (projected[0].start_ms, projected[0].end_ms),
            (10_500, 11_000)
        );
    }

    #[test]
    fn a_turn_crossing_a_gap_is_split_not_stretched() {
        // Stretching would claim someone spoke through a period for which no
        // audio exists. Two pieces is the honest answer.
        let spans = plan_spans(&[(0, 2_000), (10_000, 2_000)]);
        let projected = project_turns(&[turn("s:0", 1_000, 3_000)], &spans);
        assert_eq!(
            projected
                .iter()
                .map(|t| (t.start_ms, t.end_ms))
                .collect::<Vec<_>>(),
            vec![(1_000, 2_000), (10_000, 11_000)]
        );
    }

    #[test]
    fn a_turn_crossing_contiguous_chunks_stays_one_turn() {
        // Chunking is an implementation detail of how the file was built;
        // it must not show up as fragmentation in the result.
        let spans = plan_spans(&[(0, 2_000), (2_000, 2_000)]);
        let projected = project_turns(&[turn("s:0", 1_000, 3_000)], &spans);
        assert_eq!(projected.len(), 1);
        assert_eq!((projected[0].start_ms, projected[0].end_ms), (1_000, 3_000));
    }

    #[test]
    fn two_speakers_meeting_at_a_boundary_are_not_merged() {
        // Same instant, different speaker: rejoining on time alone would
        // hand one speaker's words to another.
        let spans = plan_spans(&[(0, 2_000), (2_000, 2_000)]);
        let projected = project_turns(&[turn("s:0", 0, 2_000), turn("s:1", 2_000, 4_000)], &spans);
        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].speaker_key, "s:0");
        assert_eq!(projected[1].speaker_key, "s:1");
    }

    #[test]
    fn a_turn_running_past_the_audio_is_clipped() {
        // Workers report a turn a few milliseconds past the end often
        // enough; honouring it would place speech after the recording ended.
        let spans = plan_spans(&[(0, 2_000)]);
        let projected = project_turns(&[turn("s:0", 1_500, 9_000)], &spans);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].end_ms, 2_000);
    }

    #[test]
    fn a_turn_covering_no_audio_is_dropped_rather_than_placed() {
        let spans = plan_spans(&[(0, 2_000)]);
        assert!(project_turns(&[turn("s:0", 5_000, 6_000)], &spans).is_empty());
    }

    #[test]
    fn an_empty_chunk_contributes_nothing() {
        // A zero-length span would sit at one file offset and swallow every
        // turn that touched it.
        let spans = plan_spans(&[(0, 2_000), (2_000, 0), (4_000, 1_000)]);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].recording_start_ms, 4_000);
    }

    #[test]
    fn a_reversed_turn_is_read_as_its_range_not_discarded() {
        let spans = plan_spans(&[(0, 4_000)]);
        let projected = project_turns(&[turn("s:0", 3_000, 1_000)], &spans);
        assert_eq!(projected.len(), 1);
        assert_eq!((projected[0].start_ms, projected[0].end_ms), (1_000, 3_000));
    }

    #[test]
    fn the_drift_allowance_catches_a_dropped_chunk_but_not_rounding() {
        // Resampling rounds by well under a millisecond per chunk; a chunk
        // that failed to decode costs its whole duration. The allowance has
        // to sit between those, or it either rejects healthy files or lets
        // every later turn land in the wrong place.
        assert!(
            duration_tolerance_ms(20) < 1_000,
            "a lost 1s chunk must be caught"
        );
        assert!(
            duration_tolerance_ms(20) > 20,
            "a millisecond of rounding per chunk must not be rejected"
        );
        // Grows with the file, so a long meeting is not held to a budget
        // sized for a short one.
        assert!(duration_tolerance_ms(200) > duration_tolerance_ms(20));
    }

    #[test]
    fn nothing_can_be_projected_without_audio() {
        assert!(project_turns(&[turn("s:0", 0, 1_000)], &[]).is_empty());
        assert_eq!(concatenated_duration_ms(&[]), 0);
    }
}
