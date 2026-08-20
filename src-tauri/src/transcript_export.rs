//! Subtitle export: turning stored transcript segments into `.srt` and
//! `.vtt` files beside the recording they describe.
//!
//! `export_artifacts.kind` has accepted `'srt'` and `'vtt'` since the schema
//! was written and the UI has offered an export button for as long, but
//! `export.render` was in the job engine's *inert* list — a button that filed
//! a row nothing would ever run. This module is what makes it real.
//!
//! # Why the formatting is not trivial
//!
//! Both formats are line-oriented and neither escapes anything by default,
//! so transcript text can terminate or reinterpret the structure around it:
//!
//! * **A blank line ends a cue in SRT.** Text containing one splits a cue in
//!   two, and every cue after it in the file shifts — the numbering no longer
//!   matches, and most players stop there. Internal blank lines are collapsed.
//! * **`<` starts markup in VTT.** A transcript reading `a < b` becomes an
//!   unterminated tag. `&`, `<` and `>` are escaped; SRT, which has no markup,
//!   is left alone.
//! * **`-->` inside cue text** is how both formats recognise a timing line.
//!   It is rewritten in the text so a parser cannot mistake it for one.
//! * **Zero-length cues do not display.** faster-whisper can emit `end ==
//!   start` on a very short utterance, and a cue no player shows is a line of
//!   the meeting silently missing from the file.
//!
//! Each of these has a test, because each produces a file that opens fine and
//! is quietly wrong — the failure mode is a viewer who never learns that the
//! last forty minutes are not there.

use std::path::PathBuf;

use serde::Serialize;
use uuid::Uuid;

use crate::genesis_adapter;

/// Rows one query can return. GenesisBlockDB rejects any limit outside
/// `1..1000` and its relational filters are equality-only with no offset, so
/// this is a hard ceiling on a single read rather than a page size — see
/// [`load_cues`], which refuses rather than truncating.
const SEGMENT_READ_CAP: u32 = crate::genesis_adapter::ROW_CAP;

/// Shortest cue this will write. A zero-length cue is not displayed by any
/// player, so an utterance whose start and end coincide would vanish from a
/// file that otherwise looks complete.
const MIN_CUE_MS: i64 = 40;

/// One subtitle cue, already resolved and ordered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cue {
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    /// `None` for a recording whose segments carry no speaker — every file
    /// import, until diarization runs.
    pub(crate) speaker: Option<String>,
    pub(crate) text: String,
}

/// What a completed render produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubtitleExport {
    pub(crate) srt_path: String,
    pub(crate) vtt_path: String,
    pub(crate) cue_count: usize,
}

/// `HH:MM:SS` plus a separated millisecond field. SRT uses a comma, WebVTT a
/// full stop; nothing else differs, so the split is the only parameter.
///
/// Hours are not clamped to two digits: a player that mis-reads `100:00:00`
/// is a better outcome than silently exporting a 100-hour recording as if it
/// were four.
fn timecode(ms: i64, millis_separator: char) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        millis_separator,
        ms % 1000
    )
}

/// Collapses everything that could terminate or restructure a cue.
///
/// Applied to both formats: line handling is identical, and only the escaping
/// that follows differs.
fn flatten_cue_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        // A literal arrow in the text is how a parser recognises a timing
        // line. Replaced rather than escaped, since neither format defines an
        // escape for it.
        .map(|line| line.replace("-->", "→"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// WebVTT cue text is markup: `<` opens a tag and `&` opens an entity.
fn escape_vtt(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Normalises order and durations so both writers see the same cues.
///
/// Sorting is not assumed of the storage engine: `query_relational` makes no
/// ordering guarantee, and a subtitle file whose cues are out of order is
/// rejected outright by some players and silently mis-rendered by others.
fn prepare(mut cues: Vec<Cue>) -> Vec<Cue> {
    cues.retain(|cue| !flatten_cue_text(&cue.text).is_empty());
    cues.sort_by_key(|cue| (cue.start_ms, cue.end_ms));
    for cue in &mut cues {
        if cue.end_ms < cue.start_ms + MIN_CUE_MS {
            cue.end_ms = cue.start_ms + MIN_CUE_MS;
        }
    }
    cues
}

/// SubRip. Cues numbered from 1, blank line between each, CRLF-free.
///
/// The speaker, when there is one, is prefixed into the text: SRT has no
/// speaker field, and `Name: text` is the convention players and humans both
/// already read.
pub(crate) fn to_srt(cues: &[Cue]) -> String {
    let cues = prepare(cues.to_vec());
    let mut out = String::new();
    for (index, cue) in cues.iter().enumerate() {
        let text = flatten_cue_text(&cue.text);
        let line = match &cue.speaker {
            Some(speaker) if !speaker.trim().is_empty() => {
                format!("{}: {}", speaker.trim(), text)
            }
            _ => text,
        };
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            timecode(cue.start_ms, ','),
            timecode(cue.end_ms, ','),
            line
        ));
    }
    out
}

/// WebVTT. Speakers become voice spans, which is the format's own mechanism
/// and lets a player style or filter by speaker instead of treating the name
/// as part of what was said.
pub(crate) fn to_vtt(cues: &[Cue]) -> String {
    let cues = prepare(cues.to_vec());
    let mut out = String::from("WEBVTT\n\n");
    for cue in cues.iter() {
        let text = escape_vtt(&flatten_cue_text(&cue.text));
        let line = match &cue.speaker {
            Some(speaker) if !speaker.trim().is_empty() => {
                format!("<v {}>{}</v>", escape_vtt(speaker.trim()), text)
            }
            _ => text,
        };
        out.push_str(&format!(
            "{} --> {}\n{}\n\n",
            timecode(cue.start_ms, '.'),
            timecode(cue.end_ms, '.'),
            line
        ));
    }
    out
}

/// Reads one recording's segments, with speaker names resolved.
///
/// Refuses rather than truncates when the read hits [`SEGMENT_READ_CAP`]. A
/// subtitle file is judged complete by whether it plays to the end, so one
/// that stops at cue 1000 does not look broken — it looks like the recording
/// ended there. Failing is the only outcome that tells the truth.
pub(crate) fn load_cues(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<Cue>, String> {
    let speaker_names: std::collections::HashMap<String, String> = genesis_adapter::query(
        storage,
        "speakers",
        &["id", "display_name"],
        vec![genesis_adapter::eq(
            "speakers",
            "project_id",
            serde_json::json!(project_id),
        )],
        SEGMENT_READ_CAP,
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
        &["id", "speaker_id", "start_ms", "end_ms", "text"],
        vec![genesis_adapter::eq(
            "transcript_segments",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;

    if page.capped {
        return Err(format!(
            "ถอดเสียงได้ {SEGMENT_READ_CAP} ท่อนขึ้นไป ซึ่งเกินเพดานการอ่านครั้งเดียวของ storage engine \
             — ไฟล์ที่ได้จะขาดท้ายโดยไม่มีอะไรบอก จึงไม่เขียนไฟล์ (ต้องแก้ที่ engine ให้อ่านเป็นหน้าได้ก่อน)"
        ));
    }

    Ok(page
        .rows
        .into_iter()
        .filter_map(|row| {
            Some(Cue {
                start_ms: row.get("transcript_segments.start_ms")?.as_i64()?,
                end_ms: row.get("transcript_segments.end_ms")?.as_i64()?,
                speaker: row
                    .get("transcript_segments.speaker_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|id| speaker_names.get(id))
                    .cloned(),
                text: row
                    .get("transcript_segments.text")?
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect())
}

/// Writes `.srt` and `.vtt` for one recording and records both in
/// `export_artifacts`.
///
/// Idempotent: the filenames derive from the recording id, so a second run
/// overwrites the first rather than accumulating `-1`, `-2` copies. That
/// matters because the job engine retries.
pub(crate) fn render_subtitles(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<SubtitleExport, String> {
    let cues = load_cues(storage, project_id, recording_id)?;
    if cues.is_empty() {
        return Err("ยังไม่มี transcript ของการบันทึกนี้ — ถอดเสียงก่อนจึงจะส่งออกซับไตเติลได้".to_string());
    }

    let storage_path = project_storage_dir(storage, project_id)?;
    let exports_dir = storage_path.join("exports");
    std::fs::create_dir_all(&exports_dir)
        .map_err(|error| format!("create exports dir failed: {error}"))?;

    let stem = format!(
        "transcript-{}",
        &recording_id[..8.min(recording_id.len())]
    );
    let srt_path = exports_dir.join(format!("{stem}.srt"));
    let vtt_path = exports_dir.join(format!("{stem}.vtt"));

    // Written before either artifact row, so a failed write never leaves a
    // ledger entry pointing at a file that is not there.
    std::fs::write(&srt_path, to_srt(&cues))
        .map_err(|error| format!("write srt failed: {error}"))?;
    std::fs::write(&vtt_path, to_vtt(&cues))
        .map_err(|error| format!("write vtt failed: {error}"))?;

    let timestamp = crate::now();
    genesis_adapter::commit_rows(
        storage,
        vec![
            genesis_adapter::upsert(
                "export_artifacts",
                serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "kind": "srt", "file_path": srt_path.display().to_string(), "source_layer_id": null, "created_at": timestamp}),
            ),
            genesis_adapter::upsert(
                "export_artifacts",
                serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "kind": "vtt", "file_path": vtt_path.display().to_string(), "source_layer_id": null, "created_at": timestamp}),
            ),
        ],
    )?;

    Ok(SubtitleExport {
        srt_path: srt_path.display().to_string(),
        vtt_path: vtt_path.display().to_string(),
        cue_count: cues.len(),
    })
}

/// One row of `export_artifacts`, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportArtifact {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) file_path: String,
    pub(crate) created_at: String,
}

/// Lists a project's exports, newest first.
///
/// Without this the feature is only half-delivered: the job completes, the
/// files exist, and nothing in the app can say where. `export_artifacts` was
/// already the table for it — it just had no reader.
#[tauri::command]
pub(crate) fn list_export_artifacts(
    project_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> crate::AppResult<Vec<ExportArtifact>> {
    let mut artifacts: Vec<ExportArtifact> = genesis_adapter::query(
        &state.genesis,
        "export_artifacts",
        &["id", "kind", "file_path", "created_at"],
        vec![genesis_adapter::eq(
            "export_artifacts",
            "project_id",
            serde_json::json!(project_id),
        )],
        SEGMENT_READ_CAP,
    )
    .map_err(crate::AppError::Genesis)?
    .into_iter()
    .filter_map(|row| {
        Some(ExportArtifact {
            id: row.get("export_artifacts.id")?.as_str()?.to_string(),
            kind: row.get("export_artifacts.kind")?.as_str()?.to_string(),
            file_path: row.get("export_artifacts.file_path")?.as_str()?.to_string(),
            created_at: row.get("export_artifacts.created_at")?.as_str()?.to_string(),
        })
    })
    .collect();
    // Newest first: the query engine guarantees no order, and the only thing
    // a user wants after an export is the one that just ran.
    artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(artifacts)
}

/// The project's own storage root. Duplicated from `lib.rs` rather than
/// shared because that one returns `AppResult`, and every other function here
/// speaks the handler's `Result<_, String>`.
fn project_storage_dir(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> Result<PathBuf, String> {
    genesis_adapter::query(
        storage,
        "projects",
        &["storage_path"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )?
    .first()
    .and_then(|row| row.get("projects.storage_path"))
    .and_then(serde_json::Value::as_str)
    .map(PathBuf::from)
    .ok_or_else(|| format!("project {project_id} has no storage path"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start_ms: i64, end_ms: i64, text: &str) -> Cue {
        Cue {
            start_ms,
            end_ms,
            speaker: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn srt_and_vtt_differ_only_where_the_formats_do() {
        let cues = vec![cue(0, 1500, "สวัสดีครับ"), cue(1500, 3200, "เริ่มประชุม")];

        // SRT: numbered, comma before milliseconds, no header.
        assert_eq!(
            to_srt(&cues),
            "1\n00:00:00,000 --> 00:00:01,500\nสวัสดีครับ\n\n\
             2\n00:00:01,500 --> 00:00:03,200\nเริ่มประชุม\n\n"
        );
        // VTT: header, full stop before milliseconds, no cue numbers.
        assert_eq!(
            to_vtt(&cues),
            "WEBVTT\n\n00:00:00.000 --> 00:00:01.500\nสวัสดีครับ\n\n\
             00:00:01.500 --> 00:00:03.200\nเริ่มประชุม\n\n"
        );
    }

    #[test]
    fn hours_are_carried_not_wrapped() {
        // 3h 04m 05.006s. A recording past the hour mark is exactly the case
        // FUNG exists for, so the hour field has to be real.
        let rendered = to_srt(&[cue(11_045_006, 11_046_000, "ท้ายประชุม")]);
        assert!(rendered.contains("03:04:05,006 --> 03:04:06,000"), "{rendered}");
    }

    #[test]
    fn a_blank_line_inside_a_cue_cannot_split_it() {
        // The defect this guards: in SRT a blank line ends the cue, so the
        // rest of this text would become cue "2"'s body, the real cue 2 would
        // be misread, and most players stop at the malformed entry. The file
        // still opens.
        let rendered = to_srt(&[cue(0, 1000, "บรรทัดแรก\n\nบรรทัดสอง"), cue(1000, 2000, "ต่อไป")]);
        assert_eq!(
            rendered,
            "1\n00:00:00,000 --> 00:00:01,000\nบรรทัดแรก บรรทัดสอง\n\n\
             2\n00:00:01,000 --> 00:00:02,000\nต่อไป\n\n"
        );
        // Exactly two cues, so the numbering still describes the file.
        assert_eq!(rendered.matches(" --> ").count(), 2);
    }

    #[test]
    fn markup_characters_are_escaped_in_vtt_and_left_alone_in_srt() {
        let cues = vec![cue(0, 1000, "ถ้า a < b && c > d")];
        assert!(to_vtt(&cues).contains("a &lt; b &amp;&amp; c &gt; d"));
        // SRT has no markup; escaping there would put entities on screen.
        assert!(to_srt(&cues).contains("a < b && c > d"));
    }

    #[test]
    fn an_arrow_in_the_text_cannot_be_read_as_a_timing_line() {
        for rendered in [
            to_srt(&[cue(0, 1000, "ขั้นตอน A --> B")]),
            to_vtt(&[cue(0, 1000, "ขั้นตอน A --> B")]),
        ] {
            // One arrow only: the real timing line.
            assert_eq!(rendered.matches("-->").count(), 1, "{rendered}");
            assert!(rendered.contains("ขั้นตอน A → B"), "{rendered}");
        }
    }

    #[test]
    fn a_zero_length_segment_still_displays() {
        // faster-whisper emits these on very short utterances. A cue with no
        // duration is shown by nothing, so the line would be missing from a
        // file that otherwise looks complete.
        let rendered = to_srt(&[cue(5000, 5000, "ครับ")]);
        assert!(rendered.contains("00:00:05,000 --> 00:00:05,040"), "{rendered}");
    }

    #[test]
    fn cues_are_ordered_by_time_not_by_whatever_the_query_returned() {
        // `query_relational` makes no ordering guarantee.
        let rendered = to_srt(&[cue(9000, 9500, "ท้าย"), cue(1000, 1500, "ต้น")]);
        assert!(
            rendered.find("ต้น").unwrap() < rendered.find("ท้าย").unwrap(),
            "{rendered}"
        );
        assert!(rendered.starts_with("1\n00:00:01,000"), "{rendered}");
    }

    #[test]
    fn empty_segments_are_dropped_without_disturbing_the_numbering() {
        let rendered = to_srt(&[
            cue(0, 1000, "หนึ่ง"),
            cue(1000, 2000, "   \n  "),
            cue(2000, 3000, "สอง"),
        ]);
        assert!(rendered.contains("1\n00:00:00,000"));
        assert!(rendered.contains("2\n00:00:02,000"));
        assert_eq!(rendered.matches(" --> ").count(), 2);
    }

    #[test]
    fn speakers_use_each_formats_own_mechanism() {
        let cues = vec![Cue {
            start_ms: 0,
            end_ms: 1000,
            speaker: Some("เรา".to_string()),
            text: "ตกลงครับ".to_string(),
        }];
        // SRT has no speaker field; the convention is a prefix.
        assert!(to_srt(&cues).contains("เรา: ตกลงครับ"));
        // VTT does have one, so the name is not part of what was said.
        assert!(to_vtt(&cues).contains("<v เรา>ตกลงครับ</v>"));
    }

    #[test]
    fn a_speakerless_recording_gets_no_stray_separator() {
        // Every file import has `speaker_id: null` until diarization runs,
        // so this is the common case, not the edge one.
        let rendered = to_srt(&[cue(0, 1000, "ข้อความ")]);
        assert!(rendered.contains("\nข้อความ\n"), "{rendered}");
        assert!(!rendered.contains(": ข้อความ"), "{rendered}");
    }

    fn open_storage() -> (std::path::PathBuf, genesis_block_native::Storage) {
        let path = std::env::temp_dir().join(format!("fung-export-test-{}", Uuid::new_v4()));
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

    /// A project with `segment_count` segments on recording `r1`, stored under
    /// a real temp directory so the writer has somewhere to write.
    fn seed(
        storage: &genesis_block_native::Storage,
        storage_path: &std::path::Path,
        segment_count: i64,
    ) {
        let mut rows = vec![
            genesis_adapter::upsert("projects", serde_json::json!({"id":"p1","name":"m","storage_path":storage_path.display().to_string(),"active_recording_id":null,"created_at":"t","updated_at":"t"})),
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
    fn rendering_writes_both_files_and_records_both_artifacts() {
        let (path, storage) = open_storage();
        let project_dir = path.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        seed(&storage, &project_dir, 3);

        let export = render_subtitles(&storage, "p1", "r1").unwrap();
        assert_eq!(export.cue_count, 3);

        let srt = std::fs::read_to_string(&export.srt_path).unwrap();
        let vtt = std::fs::read_to_string(&export.vtt_path).unwrap();
        assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:00,900\nบรรทัด 0"), "{srt}");
        assert!(vtt.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:00.900\n"), "{vtt}");

        let artifacts = genesis_adapter::query(
            &storage,
            "export_artifacts",
            &["kind", "file_path"],
            vec![genesis_adapter::eq(
                "export_artifacts",
                "project_id",
                serde_json::json!("p1"),
            )],
            10,
        )
        .unwrap();
        let mut kinds: Vec<String> = artifacts
            .iter()
            .filter_map(|row| Some(row.get("export_artifacts.kind")?.as_str()?.to_string()))
            .collect();
        kinds.sort();
        assert_eq!(kinds, vec!["srt".to_string(), "vtt".to_string()]);

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_retry_overwrites_its_own_output_instead_of_accumulating() {
        // The job engine retries. Two runs must leave two files, not four.
        let (path, storage) = open_storage();
        let project_dir = path.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        seed(&storage, &project_dir, 2);

        let first = render_subtitles(&storage, "p1", "r1").unwrap();
        let second = render_subtitles(&storage, "p1", "r1").unwrap();
        assert_eq!(first.srt_path, second.srt_path);

        let written: Vec<_> = std::fs::read_dir(project_dir.join("exports"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(written.len(), 2, "{written:?}");

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_recording_past_the_read_ceiling_is_refused_not_truncated() {
        // The defect this exists to prevent: a subtitle file that stops at
        // cue 1000 does not look broken to a viewer — it looks like the
        // recording ended there. No file is better than a plausible one.
        let (path, storage) = open_storage();
        let project_dir = path.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        seed(&storage, &project_dir, SEGMENT_READ_CAP as i64 + 200);

        let error = render_subtitles(&storage, "p1", "r1").unwrap_err();
        assert!(error.contains(&SEGMENT_READ_CAP.to_string()), "{error}");
        assert!(
            !project_dir.join("exports").exists(),
            "a refused export must not leave a partial file behind"
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_recording_with_no_transcript_is_refused_with_a_next_step() {
        let (path, storage) = open_storage();
        let project_dir = path.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        seed(&storage, &project_dir, 0);

        let error = render_subtitles(&storage, "p1", "r1").unwrap_err();
        assert!(error.contains("ถอดเสียงก่อน"), "{error}");

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn an_empty_transcript_produces_a_valid_but_empty_vtt() {
        // The header alone is a valid WebVTT file. `render_subtitles` refuses
        // before reaching here, but the formatter must not produce garbage
        // for any caller.
        assert_eq!(to_vtt(&[]), "WEBVTT\n\n");
        assert_eq!(to_srt(&[]), "");
    }
}
