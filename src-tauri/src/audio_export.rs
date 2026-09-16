//! Source-audio export for the existing durable export queue.
//!
//! Live WAV chunks are stitched directly, imported MP3 files are copied, and
//! other project-owned audio files are converted by the bundled PyAV worker.
//! Missing runtime or codec support is a terminal export error rather than a
//! fabricated WAV/MP3 extension.

use std::path::PathBuf;

use crate::{genesis_adapter, local_api};
#[cfg(test)]
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioExport {
    pub(crate) kind: &'static str,
    pub(crate) file_path: String,
}

/// Exports the selected recording's locally playable source audio when it is
/// already WAV or MP3. The existing `export.render` job owns this call, so
/// audio and subtitle artifacts share the same durable retry boundary.
pub(crate) fn render_source_audio(
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    project_id: &str,
    recording_id: &str,
) -> Result<Option<AudioExport>, String> {
    let recording = genesis_adapter::query(
        storage,
        "recordings",
        &["project_id"],
        vec![genesis_adapter::eq(
            "recordings",
            "id",
            serde_json::json!(recording_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| format!("ไม่พบการบันทึก {recording_id}"))?;
    let owner = genesis_adapter::string(&recording, "recordings.project_id")?;
    if owner != project_id {
        return Err(format!("การบันทึก {recording_id} ไม่ได้อยู่ในโปรเจกต์นี้"));
    }

    let audio = local_api::build_audio(storage, recording_id, None)
        .map_err(|error| format!("เตรียมไฟล์เสียงส่งออกไม่สำเร็จ: {error:?}"))?;
    if audio.missing_chunks > 0 {
        return Err(format!(
            "ไฟล์เสียงยังไม่ครบ — หาย {} chunk จึงไม่สร้าง export ที่ดูเหมือนสมบูรณ์",
            audio.missing_chunks
        ));
    }

    let (kind, extension) = match audio.mime {
        "audio/wav" => ("wav", "wav"),
        "audio/mpeg" => ("mp3", "mp3"),
        _ => {
            let extension = "mp3";
            let source_path = imported_source_path(storage, project_id, recording_id)?;
            let project = project_row(storage, project_id)?;
            let storage_path =
                PathBuf::from(genesis_adapter::string(&project, "projects.storage_path")?);
            let exports_dir = storage_path.join("exports");
            std::fs::create_dir_all(&exports_dir)
                .map_err(|error| format!("create exports dir failed: {error}"))?;
            let stem = format!("audio-{}", recording_id.chars().take(8).collect::<String>());
            let path = exports_dir.join(format!("{stem}.{extension}"));
            transcode_source(runtime, &source_path, &path, extension)?;
            return record_audio_export(storage, project_id, recording_id, "mp3", &path);
        }
    };

    let project = project_row(storage, project_id)?;
    let storage_path = PathBuf::from(genesis_adapter::string(&project, "projects.storage_path")?);
    let exports_dir = storage_path.join("exports");
    std::fs::create_dir_all(&exports_dir)
        .map_err(|error| format!("create exports dir failed: {error}"))?;

    let stem = format!("audio-{}", recording_id.chars().take(8).collect::<String>());
    let path = exports_dir.join(format!("{stem}.{extension}"));
    std::fs::write(&path, audio.bytes)
        .map_err(|error| format!("write audio export failed: {error}"))?;

    record_audio_export(storage, project_id, recording_id, kind, &path)
}

fn project_row(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> Result<serde_json::Value, String> {
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
    .into_iter()
    .next()
    .ok_or_else(|| format!("ไม่พบโปรเจกต์ {project_id}"))
}

fn imported_source_path(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<PathBuf, String> {
    let recording = genesis_adapter::query(
        storage,
        "recordings",
        &["source", "canonical_audio_path"],
        vec![
            genesis_adapter::eq("recordings", "id", serde_json::json!(recording_id)),
            genesis_adapter::eq("recordings", "project_id", serde_json::json!(project_id)),
        ],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| format!("ไม่พบการบันทึก {recording_id}"))?;
    let source = genesis_adapter::string(&recording, "recordings.source")?;
    if source == "desktop" || source == "microphone" {
        return Err("ไม่สามารถ transcode live capture ที่ไม่มี source file เดียวได้".to_string());
    }
    let recorded = genesis_adapter::string(&recording, "recordings.canonical_audio_path")?;
    let project = project_row(storage, project_id)?;
    let root = PathBuf::from(genesis_adapter::string(&project, "projects.storage_path")?);
    crate::audio_custody::resolve_chunk_path(&root, &recorded)
        .ok_or_else(|| "ไม่พบ source audio file ใน project custody".to_string())
}

fn transcode_source(
    runtime: &crate::WhisperRuntime,
    source_path: &std::path::Path,
    output_path: &std::path::Path,
    output_format: &str,
) -> Result<(), String> {
    let script = runtime
        .script
        .parent()
        .map(|parent| parent.join("transcode_audio.py"))
        .ok_or_else(|| "ไม่พบโฟลเดอร์ของ FUNG Python worker".to_string())?;
    let input = source_path.to_string_lossy().into_owned();
    let output = output_path.to_string_lossy().into_owned();
    let args = [
        "--input",
        input.as_str(),
        "--output",
        output.as_str(),
        "--format",
        output_format,
    ];
    crate::run_python_worker(runtime, &script, &args, None, None, |_| {}).map(|_| ())
}

fn record_audio_export(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    kind: &'static str,
    path: &std::path::Path,
) -> Result<Option<AudioExport>, String> {
    let timestamp = crate::now();
    genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "export_artifacts",
            serde_json::json!({
                "id": format!("audio-export:{project_id}:{recording_id}:{kind}"),
                "project_id": project_id,
                "kind": kind,
                "file_path": path.display().to_string(),
                "source_layer_id": null,
                "created_at": timestamp,
            }),
        )],
    )?;

    Ok(Some(AudioExport {
        kind,
        file_path: path.display().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};
    use std::path::{Path, PathBuf};

    fn open_storage() -> (PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-audio-export-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (path, storage)
    }

    fn test_runtime() -> crate::WhisperRuntime {
        crate::WhisperRuntime {
            python: PathBuf::from("python-that-does-not-exist"),
            script: PathBuf::from("scripts/transcribe.py"),
            cuda_bin: PathBuf::new(),
        }
    }

    fn write_wav(path: &Path) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in [0i16, 100, -100, 50] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn seed_wav(storage: &Storage, project_path: &Path) -> PathBuf {
        let source = project_path
            .join("live")
            .join("r1")
            .join("chunks")
            .join("mic-00001.wav");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        write_wav(&source);
        genesis_adapter::commit_rows(
            storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "p1",
                        "name": "Meeting",
                        "storage_path": project_path.display().to_string(),
                        "active_recording_id": "r1",
                        "created_at": "project-created",
                        "updated_at": "project-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "r1",
                        "project_id": "p1",
                        "source": "desktop",
                        "input_path": null,
                        "canonical_audio_path": source.display().to_string(),
                        "status": "completed",
                        "duration_ms": 1,
                        "created_at": "recording-created",
                        "updated_at": "recording-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({
                        "id": "c1",
                        "recording_id": "r1",
                        "sequence_no": 1,
                        "file_path": source.display().to_string(),
                        "start_ms": 0,
                        "end_ms": 1,
                        "byte_size": std::fs::metadata(&source).unwrap().len(),
                        "checksum": "fixture",
                        "created_at": "chunk-created",
                        "transcribed_at": "chunk-transcribed",
                    }),
                ),
            ],
        )
        .unwrap();
        source
    }

    fn seed_import(
        storage: &Storage,
        project_path: &Path,
        file_name: &str,
        bytes: &[u8],
    ) -> PathBuf {
        let source = project_path.join("imports").join("r1").join(file_name);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, bytes).unwrap();
        genesis_adapter::commit_rows(
            storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({
                        "id": "p1",
                        "name": "Meeting",
                        "storage_path": project_path.display().to_string(),
                        "active_recording_id": "r1",
                        "created_at": "project-created",
                        "updated_at": "project-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({
                        "id": "r1",
                        "project_id": "p1",
                        "source": "import",
                        "input_path": null,
                        "canonical_audio_path": source.display().to_string(),
                        "status": "completed",
                        "duration_ms": 1,
                        "created_at": "recording-created",
                        "updated_at": "recording-updated",
                    }),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({
                        "id": "c1",
                        "recording_id": "r1",
                        "sequence_no": 1,
                        "file_path": source.display().to_string(),
                        "start_ms": 0,
                        "end_ms": 1,
                        "byte_size": bytes.len(),
                        "checksum": "fixture",
                        "created_at": "chunk-created",
                        "transcribed_at": "chunk-transcribed",
                    }),
                ),
            ],
        )
        .unwrap();
        source
    }

    #[test]
    fn live_wav_chunks_export_once_and_record_a_wav_artifact() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let source = seed_wav(&storage, &project_path);

        let runtime = test_runtime();
        let export = render_source_audio(&storage, &runtime, "p1", "r1")
            .unwrap()
            .unwrap();
        assert_eq!(export.kind, "wav");
        assert_eq!(
            std::fs::read(&export.file_path).unwrap(),
            std::fs::read(source).unwrap()
        );

        let artifact = genesis_adapter::query(
            &storage,
            "export_artifacts",
            &["kind", "file_path"],
            vec![genesis_adapter::eq(
                "export_artifacts",
                "id",
                serde_json::json!("audio-export:p1:r1:wav"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(artifact["export_artifacts.kind"], "wav");
        assert_eq!(artifact["export_artifacts.file_path"], export.file_path);

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn unsupported_source_format_fails_without_runtime_and_does_not_fabricate_an_extension() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let _source = seed_import(&storage, &project_path, "meeting.m4a", b"not-decoded-audio");

        let runtime = test_runtime();
        let error = render_source_audio(&storage, &runtime, "p1", "r1").unwrap_err();
        assert!(error.contains("FUNG Python runtime is missing"));
        assert!(
            genesis_adapter::query(&storage, "export_artifacts", &["id"], vec![], 10,)
                .unwrap()
                .is_empty()
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn imported_mp3_is_copied_and_recorded_as_an_mp3_artifact() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let source = seed_import(&storage, &project_path, "meeting.mp3", b"fixture-mp3");

        let runtime = test_runtime();
        let export = render_source_audio(&storage, &runtime, "p1", "r1")
            .unwrap()
            .unwrap();
        assert_eq!(export.kind, "mp3");
        assert_eq!(
            std::fs::read(&export.file_path).unwrap(),
            std::fs::read(source).unwrap()
        );

        let artifact = genesis_adapter::query(
            &storage,
            "export_artifacts",
            &["kind", "file_path"],
            vec![genesis_adapter::eq(
                "export_artifacts",
                "id",
                serde_json::json!("audio-export:p1:r1:mp3"),
            )],
            1,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(artifact["export_artifacts.kind"], "mp3");
        assert_eq!(artifact["export_artifacts.file_path"], export.file_path);

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }
}
