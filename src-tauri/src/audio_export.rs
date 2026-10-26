//! Source-audio export for the existing durable export queue.
//!
//! Live WAV chunks are stitched directly, imported MP3 files are copied, and
//! other project-owned audio files are converted by the bundled PyAV worker.
//! Missing runtime or codec support is a terminal export error rather than a
//! fabricated WAV/MP3 extension.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{genesis_adapter, local_api};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioExport {
    pub(crate) kind: &'static str,
    pub(crate) file_path: String,
}

pub(crate) fn render_source_audio_channels(
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<AudioExport>, String> {
    render_source_audio_channels_with_commit(
        storage,
        runtime,
        project_id,
        recording_id,
        genesis_adapter::commit_rows,
    )
}

fn render_source_audio_channels_with_commit<F>(
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    project_id: &str,
    recording_id: &str,
    commit: F,
) -> Result<Vec<AudioExport>, String>
where
    F: FnOnce(
        &genesis_block_native::Storage,
        Vec<genesis_block_native::RelationalRowMutation>,
    ) -> Result<(), String>,
{
    render_source_audio_channels_with_hooks(
        storage,
        runtime,
        project_id,
        recording_id,
        commit,
        || Ok(()),
    )
}

fn render_source_audio_channels_with_hooks<F, B>(
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    project_id: &str,
    recording_id: &str,
    commit: F,
    before_prepare: B,
) -> Result<Vec<AudioExport>, String>
where
    F: FnOnce(
        &genesis_block_native::Storage,
        Vec<genesis_block_native::RelationalRowMutation>,
    ) -> Result<(), String>,
    B: FnOnce() -> Result<(), String>,
{
    let snapshot = source_channels(storage, project_id, recording_id)?;
    before_prepare()?;
    let exports_dir = snapshot.root.join("exports");
    std::fs::create_dir_all(&exports_dir)
        .map_err(|error| format!("create exports dir failed: {error}"))?;
    let mut staging = ExportStage(
        exports_dir.join(format!(".audio-stage-{}", Uuid::new_v4())),
        false,
    );
    std::fs::create_dir(&staging.0)
        .map_err(|error| format!("create audio stage failed: {error}"))?;
    let multichannel = snapshot.channels.len() > 1;
    let stem: String = recording_id
        .chars()
        .take(8)
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    let mut prepared = Vec::new();
    let mut output_bytes = 0u64;
    for channel in &snapshot.channels {
        // Retain only one renderer result at a time, not a Vec of channel PCM.
        let audio = build_snapshot_audio(&snapshot, channel)?;
        if audio.missing_chunks > 0 {
            return Err(format!(
                "ไฟล์เสียงช่อง {channel} ยังไม่ครบ — หาย {} chunk",
                audio.missing_chunks
            ));
        }
        let kind = match audio.mime {
            "audio/wav" => "wav",
            "audio/mpeg" => "mp3",
            _ => "mp3",
        };
        if multichannel && audio.mime != "audio/wav" {
            return Err(
                "multichannel source export ต้องเป็น WAV แยกช่อง; ไม่รวม/แปลงเสียงโดยปริยาย".into(),
            );
        }
        let filename = if multichannel {
            format!("audio-{stem}-{channel}.{kind}")
        } else {
            format!("audio-{stem}.{kind}")
        };
        let candidate = staging.0.join(&filename);
        if matches!(audio.mime, "audio/wav" | "audio/mpeg") {
            std::fs::write(&candidate, &audio.bytes)
                .map_err(|error| format!("write audio stage failed: {error}"))?;
        } else {
            if snapshot.source == "desktop" || snapshot.source == "microphone" {
                return Err("ไม่สามารถ transcode live capture ที่ไม่มี source file เดียวได้".into());
            }
            let source = snapshot_import_path(&snapshot)?;
            transcode_source(
                runtime,
                &source,
                &candidate,
                kind,
                MAX_AUDIO_EXPORT_BYTES - output_bytes,
            )?;
        }
        drop(audio);
        let (size, checksum) = digest_file(&candidate)?;
        output_bytes = output_bytes
            .checked_add(size)
            .ok_or("audio export byte count overflow")?;
        if output_bytes > MAX_AUDIO_EXPORT_BYTES {
            return Err("audio export exceeds 512 MiB byte ceiling".into());
        }
        prepared.push(PreparedAudio {
            channel: channel.clone(),
            kind,
            filename,
            size,
            checksum,
        });
    }
    // Repair/recovery may legitimately alter even a completed recording. Never
    // publish a set prepared from one ledger version as if it were the new one.
    if source_channels(storage, project_id, recording_id)? != snapshot {
        return Err(
            "audio source snapshot changed during export; retry after repair/recovery completes"
                .into(),
        );
    }
    let mut legacy_rollback = None;
    let published_dir = if multichannel {
        let mut hash = Sha256::new();
        hash.update((project_id.len() as u64).to_le_bytes());
        hash.update(project_id.as_bytes());
        hash.update((recording_id.len() as u64).to_le_bytes());
        hash.update(recording_id.as_bytes());
        for audio in &prepared {
            hash.update([0]);
            hash.update(audio.channel.as_bytes());
            hash.update([0]);
            hash.update(audio.kind.as_bytes());
            hash.update(audio.size.to_le_bytes());
            hash.update(audio.checksum.as_bytes());
        }
        let published = exports_dir.join(format!("audio-set-{:x}", hash.finalize()));
        if published.exists() {
            // An interrupted DB commit may leave the complete immutable set.
            // Reuse only verified bytes, never silently trust a damaged set.
            for audio in &prepared {
                let existing = published.join(&audio.filename);
                if std::fs::symlink_metadata(&existing)
                    .map_err(|error| error.to_string())?
                    .file_type()
                    .is_symlink()
                    || digest_file(&existing)? != (audio.size, audio.checksum.clone())
                {
                    return Err(
                        "existing audio export set does not match its content identity".into(),
                    );
                }
            }
        } else {
            std::fs::rename(&staging.0, &published)
                .map_err(|error| format!("publish audio set failed: {error}"))?;
        }
        published
    } else {
        let audio = prepared.first().ok_or("no source audio channels")?;
        let target = exports_dir.join(&audio.filename);
        let expected_id = format!("audio-export:{project_id}:{recording_id}:{}", audio.kind);
        let canonical_target = target.canonicalize().ok();
        for row in genesis_adapter::query_all(
            storage,
            "export_artifacts",
            &["id", "file_path"],
            vec![genesis_adapter::eq(
                "export_artifacts",
                "project_id",
                serde_json::json!(project_id),
            )],
        )? {
            let other_id = genesis_adapter::string(&row, "export_artifacts.id")?;
            let other_path =
                PathBuf::from(genesis_adapter::string(&row, "export_artifacts.file_path")?);
            let same_path = other_path == target
                || canonical_target
                    .as_ref()
                    .is_some_and(|path| other_path.canonicalize().ok().as_ref() == Some(path));
            if other_id != expected_id && same_path {
                return Err(
                    "legacy audio filename belongs to another export artifact; refusing overwrite"
                        .into(),
                );
            }
        }
        let previous = if target.exists() {
            if std::fs::symlink_metadata(&target)
                .map_err(|error| error.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("legacy audio export is a symbolic link".into());
            }
            let backup = staging.0.join("previous-export");
            std::fs::copy(&target, &backup)
                .map_err(|error| format!("backup previous export failed: {error}"))?;
            Some(backup)
        } else {
            None
        };
        std::fs::rename(staging.0.join(&audio.filename), &target)
            .map_err(|error| format!("publish audio export failed: {error}"))?;
        legacy_rollback = Some((target, previous));
        exports_dir
    };
    let metadata_result = (|| {
        let timestamp = crate::now();
        let mut mutations = Vec::new();
        let mut exports = Vec::new();
        for audio in prepared {
            let path = published_dir.join(audio.filename);
            let id = if multichannel {
                format!(
                    "audio-export:{project_id}:{recording_id}:{}:{}",
                    audio.channel, audio.kind
                )
            } else {
                format!("audio-export:{project_id}:{recording_id}:{}", audio.kind)
            };
            mutations.push(genesis_adapter::upsert("export_artifacts", serde_json::json!({
            "id":id, "project_id":project_id, "kind":audio.kind, "file_path":path.display().to_string(),
            "source_layer_id":null, "created_at":timestamp,
        })));
            exports.push(AudioExport {
                kind: audio.kind,
                file_path: path.display().to_string(),
            });
        }
        if multichannel {
            for kind in ["wav", "mp3"] {
                let legacy_id = format!("audio-export:{project_id}:{recording_id}:{kind}");
                if !genesis_adapter::query(
                    storage,
                    "export_artifacts",
                    &["id"],
                    vec![genesis_adapter::eq(
                        "export_artifacts",
                        "id",
                        serde_json::json!(legacy_id),
                    )],
                    1,
                )?
                .is_empty()
                {
                    mutations.push(genesis_adapter::delete("export_artifacts", &legacy_id));
                }
            }
        }
        commit(storage, mutations)?;
        Ok(exports)
    })();
    if let Err(error) = metadata_result {
        if let Some((target, previous)) = legacy_rollback {
            let rollback = if let Some(previous) = previous {
                std::fs::rename(previous, &target)
            } else {
                std::fs::remove_file(&target)
            };
            if let Err(rollback_error) = rollback {
                staging.1 = true;
                return Err(format!(
                    "{error}; audio rollback failed: {rollback_error}; recovery stage: {}",
                    staging.0.display()
                ));
            }
        }
        return Err(error);
    }
    metadata_result
}

const MAX_AUDIO_EXPORT_BYTES: u64 = 512 * 1024 * 1024;

struct ExportStage(PathBuf, bool);
impl Drop for ExportStage {
    fn drop(&mut self) {
        if !self.1 {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

struct PreparedAudio {
    channel: String,
    kind: &'static str,
    filename: String,
    size: u64,
    checksum: String,
}

#[derive(Debug, PartialEq, Eq)]
struct SourceSnapshot {
    root: PathBuf,
    source: String,
    channels: Vec<String>,
    chunks: Vec<ValidatedChunk>,
}

#[derive(Debug, PartialEq, Eq)]
struct ValidatedChunk {
    id: String,
    ordinal: u32,
    path: String,
    start: i64,
    end: i64,
    bytes: u64,
    checksum: String,
}

fn snapshot_import_path(snapshot: &SourceSnapshot) -> Result<PathBuf, String> {
    let chunk = snapshot
        .chunks
        .iter()
        .find(|chunk| local_api::channel_of(&chunk.path) == "file")
        .ok_or("snapshot has no imported source")?;
    crate::audio_custody::resolve_chunk_path(&snapshot.root, &chunk.path)
        .ok_or_else(|| "validated source audio is no longer in project custody".into())
}

fn build_snapshot_audio(
    snapshot: &SourceSnapshot,
    channel: &str,
) -> Result<local_api::Audio, String> {
    if channel == "file" {
        let path = snapshot_import_path(snapshot)?;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .map_err(|error| error.to_string())?
            .take(MAX_AUDIO_EXPORT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_AUDIO_EXPORT_BYTES {
            return Err("audio source exceeds 512 MiB byte ceiling".into());
        }
        return Ok(local_api::Audio {
            bytes,
            mime: local_api::mime_for(&path),
            missing_chunks: 0,
        });
    }
    let chunks = snapshot
        .chunks
        .iter()
        .filter(|chunk| local_api::channel_of(&chunk.path) == channel)
        .map(|chunk| (chunk.ordinal, chunk.path.clone()))
        .collect::<Vec<_>>();
    local_api::stitch_wav_with_limit(
        &snapshot.root,
        &chunks,
        Some(MAX_AUDIO_EXPORT_BYTES as usize),
    )
    .map_err(|error| format!("เตรียมไฟล์เสียงช่อง {channel} ส่งออกไม่สำเร็จ: {error:?}"))
}

fn digest_file(path: &Path) -> Result<(u64, String), String> {
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("read export file failed: {error}"))?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    if size > MAX_AUDIO_EXPORT_BYTES {
        return Err("audio export exceeds 512 MiB byte ceiling".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

fn source_channels(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<SourceSnapshot, String> {
    let recording = crate::routed_media_access::require_legacy_media_source(storage, recording_id)?;
    if genesis_adapter::string(&recording, "recordings.project_id")? != project_id {
        return Err("การบันทึกไม่ได้อยู่ในโปรเจกต์นี้".into());
    }
    if genesis_adapter::string(&recording, "recordings.status")? != "completed" {
        return Err(
            "audio source is still changing; stop and finish the recording before exporting".into(),
        );
    }
    let source = genesis_adapter::string(&recording, "recordings.source")?;
    let project = project_row(storage, project_id)?;
    let root = PathBuf::from(genesis_adapter::string(&project, "projects.storage_path")?);
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &[
            "id",
            "file_path",
            "sequence_no",
            "start_ms",
            "end_ms",
            "checksum",
        ],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let mut paths = Vec::new();
    let mut bytes = 0u64;
    let mut chunks = Vec::new();
    let mut timelines = std::collections::BTreeMap::<&str, Vec<(u32, i64, i64)>>::new();
    for row in rows {
        let path = genesis_adapter::string(&row, "audio_chunks.file_path")?;
        let resolved = crate::audio_custody::resolve_chunk_path(&root, &path).ok_or_else(|| {
            format!("source audio chunk is missing or outside project custody: {path}")
        })?;
        let size = std::fs::metadata(&resolved)
            .map_err(|error| error.to_string())?
            .len();
        bytes = bytes
            .checked_add(size)
            .ok_or("audio source byte count overflow")?;
        if bytes > MAX_AUDIO_EXPORT_BYTES {
            return Err("audio source exceeds 512 MiB byte ceiling".into());
        }
        let channel = local_api::channel_of(&path);
        let start = genesis_adapter::integer(&row, "audio_chunks.start_ms")?;
        let end = genesis_adapter::integer(&row, "audio_chunks.end_ms")?;
        let ordinal = local_api::chunk_ordinal(&path)
            .unwrap_or(genesis_adapter::integer(&row, "audio_chunks.sequence_no")?.max(0) as u32);
        if channel != "file" {
            if start < 0 || end <= start {
                return Err(format!(
                    "audio timeline {channel} has invalid interval {start}..{end}"
                ));
            }
            let wav = hound::WavReader::open(&resolved)
                .map_err(|error| format!("read source WAV failed: {error}"))?;
            let rate = u128::from(wav.spec().sample_rate);
            let actual = u128::from(wav.duration()) * 1000;
            let declared = (end - start) as u128 * rate;
            if rate == 0 || actual.abs_diff(declared) > rate {
                return Err(format!("audio timeline {channel} duration does not match source PCM (allowing 1 ms ledger rounding)"));
            }
            timelines
                .entry(channel)
                .or_default()
                .push((ordinal, start, end));
        }
        chunks.push(ValidatedChunk {
            id: genesis_adapter::string(&row, "audio_chunks.id")?,
            ordinal,
            path: path.clone(),
            start,
            end,
            bytes: size,
            checksum: genesis_adapter::string(&row, "audio_chunks.checksum")?,
        });
        paths.push(path);
    }
    for (channel, mut intervals) in timelines {
        intervals.sort_by_key(|interval| interval.0);
        let mut expected = 0;
        let mut previous_ordinal = None;
        for (ordinal, start, end) in intervals {
            if start != expected || end <= start || previous_ordinal == Some(ordinal) {
                return Err(format!("audio timeline {channel} is noncontiguous or malformed at {start}..{end}; source gaps must not be compressed"));
            }
            expected = end;
            previous_ordinal = Some(ordinal);
        }
    }
    let channels = local_api::channels_of(&paths);
    if channels.is_empty() {
        return Err("recording has no audio channels".into());
    }
    chunks.sort_by(|left, right| {
        (left.ordinal, &left.path, &left.id).cmp(&(right.ordinal, &right.path, &right.id))
    });
    Ok(SourceSnapshot {
        root,
        source,
        channels,
        chunks,
    })
}

/// Exports the selected recording's locally playable source audio when it is
/// already WAV or MP3. The existing `export.render` job owns this call, so
/// audio and subtitle artifacts share the same durable retry boundary.
#[cfg(test)]
pub(crate) fn render_source_audio(
    storage: &genesis_block_native::Storage,
    runtime: &crate::WhisperRuntime,
    project_id: &str,
    recording_id: &str,
) -> Result<Option<AudioExport>, String> {
    if source_channels(storage, project_id, recording_id)?
        .channels
        .len()
        != 1
    {
        return Err("use the all-channel renderer for a multichannel recording".into());
    }
    render_source_audio_channels(storage, runtime, project_id, recording_id)
        .map(|exports| exports.into_iter().next())
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

fn transcode_source(
    runtime: &crate::WhisperRuntime,
    source_path: &std::path::Path,
    output_path: &std::path::Path,
    output_format: &str,
    output_limit: u64,
) -> Result<(), String> {
    let script = runtime
        .script
        .parent()
        .map(|parent| parent.join("transcode_audio.py"))
        .ok_or_else(|| "ไม่พบโฟลเดอร์ของ FUNG Python worker".to_string())?;
    let input = source_path.to_string_lossy().into_owned();
    let output = output_path.to_string_lossy().into_owned();
    let output_limit = output_limit.to_string();
    let args = [
        "--input",
        input.as_str(),
        "--output",
        output.as_str(),
        "--format",
        output_format,
        "--max-output-bytes",
        output_limit.as_str(),
    ];
    crate::run_python_worker(runtime, &script, &args, None, None, |_| {}).map(|_| ())
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
        write_wav_values(path, &[0, 100, -100, 50]);
    }

    fn write_wav_values(path: &Path, values: &[i16]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in values {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn update_fixture_interval(storage: &Storage, source: &Path, start: i64, end: i64) {
        genesis_adapter::commit_rows(storage, vec![genesis_adapter::upsert("audio_chunks", serde_json::json!({
            "id":"c1", "recording_id":"r1", "sequence_no":1, "file_path":source.display().to_string(),
            "start_ms":start, "end_ms":end, "byte_size":std::fs::metadata(source).unwrap().len(),
            "checksum":"fixture", "created_at":"created", "transcribed_at":"transcribed",
        }))]).unwrap();
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
    fn completed_capture_exports_while_asr_job_remains_running_and_decode_is_bounded() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let source = seed_wav(&storage, &project_path);
        genesis_adapter::commit_rows(&storage, vec![genesis_adapter::upsert("jobs", serde_json::json!({
            "id":"asr-job","project_id":"p1","type":"recording.capture","status":"running","progress":0,
            "input_refs_json":["r1"],"output_refs_json":[],"provider_id":null,"error_code":null,"error_message":null,
            "attempt_no":1,"started_at":"t","finished_at":null,"created_at":"t","updated_at":"t"
        }))]).unwrap();
        let bounded = local_api::stitch_wav_with_limit(
            &project_path,
            &[(1, source.display().to_string())],
            Some(6),
        );
        assert!(
            matches!(bounded, Err(local_api::AudioError::Unsupported(message)) if message.contains("byte ceiling"))
        );
        let exports = render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap();
        assert_eq!(exports.len(), 1);
        assert!(Path::new(&exports[0].file_path).is_file());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn active_capture_and_between_phase_source_mutation_cannot_publish_partial_exports() {
        for mode in 0..3 {
            let (path, storage) = open_storage();
            let project_path = path.join("project");
            let source = seed_wav(&storage, &project_path);
            if mode == 0 {
                genesis_adapter::commit_rows(&storage, vec![genesis_adapter::upsert("recordings", serde_json::json!({"id":"r1","project_id":"p1","source":"desktop","input_path":null,"canonical_audio_path":source.display().to_string(),"status":"recording","duration_ms":1,"created_at":"created","updated_at":"updated"}))]).unwrap();
            }
            let called = std::cell::Cell::new(false);
            let result = render_source_audio_channels_with_hooks(
                &storage,
                &test_runtime(),
                "p1",
                "r1",
                genesis_adapter::commit_rows,
                || {
                    called.set(true);
                    let new_path = source.with_file_name(if mode == 2 {
                        "mic-00001-repaired.wav"
                    } else {
                        "system-00001.wav"
                    });
                    write_wav_values(&new_path, &[2000, -2000, 3000, -3000]);
                    genesis_adapter::commit_rows(
                        &storage,
                        vec![genesis_adapter::upsert(
                            "audio_chunks",
                            serde_json::json!({"id":if mode==2 {"c1"}else{"c2"},"recording_id":"r1","sequence_no":if mode==2 {1}else{2},"file_path":new_path.display().to_string(),"start_ms":0,"end_ms":1,"byte_size":52,"checksum":"fixture","created_at":"created","transcribed_at":null}),
                        )],
                    )
                },
            );
            assert!(
                result.is_err(),
                "active or changed source must not publish a misleading set"
            );
            if mode == 0 {
                assert!(!called.get(), "active capture must fail before preparation");
            }
            assert!(
                genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                    .unwrap()
                    .is_empty()
            );
            drop(storage);
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn multichannel_metadata_failure_reuses_complete_set_and_preserves_legacy_until_commit() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        seed_wav(&storage, &project_path);
        let legacy = render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap();
        let legacy_bytes = std::fs::read(&legacy[0].file_path).unwrap();
        let system = project_path.join("live/r1/chunks/system-00001.wav");
        write_wav_values(&system, &[1234, -1234, 2345, -2345]);
        genesis_adapter::commit_rows(&storage, vec![
            genesis_adapter::upsert("audio_chunks", serde_json::json!({"id":"c2","recording_id":"r1","sequence_no":2,"file_path":system.display().to_string(),"start_ms":0,"end_ms":1,"byte_size":52,"checksum":"fixture","created_at":"created","transcribed_at":null})),
            genesis_adapter::upsert("export_artifacts", serde_json::json!({"id":"audio-export:p1:other:wav","project_id":"p1","kind":"wav","file_path":"unrelated.wav","source_layer_id":null,"created_at":"created"})),
        ]).unwrap();
        let failed = render_source_audio_channels_with_commit(
            &storage,
            &test_runtime(),
            "p1",
            "r1",
            |_, _| Err("injected metadata commit failure".into()),
        );
        assert!(failed.unwrap_err().contains("injected"));
        let rows =
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![]).unwrap();
        assert_eq!(rows.len(), 2); // previous legacy + unrelated, no partial new set
        assert!(rows
            .iter()
            .any(|row| row["export_artifacts.id"] == "audio-export:p1:r1:wav"));
        let entries_before = std::fs::read_dir(project_path.join("exports"))
            .unwrap()
            .count();
        let exports = render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap();
        assert_eq!(exports.len(), 2);
        assert_eq!(
            std::fs::read_dir(project_path.join("exports"))
                .unwrap()
                .count(),
            entries_before
        );
        assert_eq!(std::fs::read(&legacy[0].file_path).unwrap(), legacy_bytes);
        let rows =
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![]).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(!rows
            .iter()
            .any(|row| row["export_artifacts.id"] == "audio-export:p1:r1:wav"));
        assert!(rows
            .iter()
            .any(|row| row["export_artifacts.id"] == "audio-export:p1:other:wav"));
        let intact = std::fs::read(&exports[1].file_path).unwrap();
        std::fs::write(&exports[0].file_path, b"tampered").unwrap();
        assert!(
            render_source_audio_channels(&storage, &test_runtime(), "p1", "r1")
                .unwrap_err()
                .contains("content identity")
        );
        assert_eq!(std::fs::read(&exports[1].file_path).unwrap(), intact);
        assert_eq!(
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                .unwrap()
                .len(),
            3
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn source_budget_and_publication_failure_are_honest_before_artifact_commit() {
        for oversized in [false, true] {
            let (path, storage) = open_storage();
            let project_path = path.join("project");
            let source = seed_wav(&storage, &project_path);
            if oversized {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(source)
                    .unwrap()
                    .set_len(MAX_AUDIO_EXPORT_BYTES + 1)
                    .unwrap();
            } else {
                std::fs::write(project_path.join("exports"), b"unrelated existing file").unwrap();
            }
            let error =
                render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap_err();
            if oversized {
                assert!(error.contains("byte ceiling"));
            } else {
                assert_eq!(
                    std::fs::read(project_path.join("exports")).unwrap(),
                    b"unrelated existing file"
                );
            }
            assert!(
                genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                    .unwrap()
                    .is_empty()
            );
            drop(storage);
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn normal_fractional_millisecond_pcm_duration_is_not_rejected() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let source = seed_wav(&storage, &project_path);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav = hound::WavWriter::create(&source, spec).unwrap();
        for _ in 0..256_772 {
            wav.write_sample(50i16).unwrap();
        }
        wav.finalize().unwrap();
        update_fixture_interval(&storage, &source, 0, 5349);
        let exports = render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap();
        assert_eq!(
            hound::WavReader::open(&exports[0].file_path)
                .unwrap()
                .duration(),
            256_772
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn export_rejects_live_timeline_gaps_and_initial_offsets() {
        for mode in 0..5 {
            let (path, storage) = open_storage();
            let project_path = path.join("project");
            let source = seed_wav(&storage, &project_path);
            if mode != 0 && mode != 4 {
                let (start, end) = match mode {
                    1 => (2000, 2001),
                    2 => (0, 0),
                    _ => (0, 1000),
                };
                update_fixture_interval(&storage, &source, start, end);
            } else {
                let second = source.with_file_name("mic-00002.wav");
                write_wav(&second);
                let start = if mode == 0 { 3000 } else { 0 };
                genesis_adapter::commit_rows(&storage, vec![genesis_adapter::upsert("audio_chunks", serde_json::json!({"id":"c2", "recording_id":"r1", "sequence_no":2, "file_path":second.display().to_string(), "start_ms":start, "end_ms":start+1, "byte_size":52,"checksum":"fixture","created_at":"created","transcribed_at":null}))]).unwrap();
            }
            let error =
                render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap_err();
            assert!(error.contains("timeline"), "{error}");
            assert!(
                genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                    .unwrap()
                    .is_empty()
            );
            drop(storage);
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn failed_legacy_metadata_commit_preserves_previous_file_and_identity() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        let source = seed_wav(&storage, &project_path);
        let previous = render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").unwrap();
        let bytes = std::fs::read(&previous[0].file_path).unwrap();
        write_wav_values(&source, &[1234, -1234, 2345, -2345]);
        let failed = render_source_audio_channels_with_commit(
            &storage,
            &test_runtime(),
            "p1",
            "r1",
            |_, _| Err("injected metadata commit failure".into()),
        );
        assert!(failed.is_err());
        assert_eq!(std::fs::read(&previous[0].file_path).unwrap(), bytes);
        assert_eq!(
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                .unwrap()
                .len(),
            1
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn multichannel_export_preserves_both_sources_and_retries_without_duplicate_artifacts() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        seed_wav(&storage, &project_path);
        let system = project_path.join("live/r1/chunks/system-00001.wav");
        write_wav_values(&system, &[0, 2000, -2000, 1000]);
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "audio_chunks",
                serde_json::json!({"id":"c2", "recording_id":"r1", "sequence_no":2,
                "file_path":system.display().to_string(), "start_ms":0, "end_ms":1,
                "byte_size":std::fs::metadata(&system).unwrap().len(), "checksum":"fixture",
                "created_at":"chunk-created", "transcribed_at":"chunk-transcribed"}),
            )],
        )
        .unwrap();
        let runtime = test_runtime();
        let exports = render_source_audio_channels(&storage, &runtime, "p1", "r1").unwrap();
        assert_eq!(
            exports.len(),
            2,
            "both microphone and system audio must survive export"
        );
        assert!(exports
            .iter()
            .any(|export| export.file_path.ends_with("-mic.wav")));
        assert!(exports
            .iter()
            .any(|export| export.file_path.ends_with("-system.wav")));
        for export in &exports {
            let source = if export.file_path.ends_with("-system.wav") {
                system.clone()
            } else {
                system.with_file_name("mic-00001.wav")
            };
            assert_eq!(
                std::fs::read(&export.file_path).unwrap(),
                std::fs::read(source).unwrap()
            );
        }
        assert_eq!(
            render_source_audio_channels(&storage, &runtime, "p1", "r1").unwrap(),
            exports
        );
        let artifacts =
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![]).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts
            .iter()
            .any(|row| row["export_artifacts.id"] == "audio-export:p1:r1:mic:wav"));
        assert!(artifacts
            .iter()
            .any(|row| row["export_artifacts.id"] == "audio-export:p1:r1:system:wav"));
        assert_eq!(
            std::fs::read_dir(project_path.join("exports"))
                .unwrap()
                .count(),
            1
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn missing_second_channel_does_not_publish_the_first_channel() {
        let (path, storage) = open_storage();
        let project_path = path.join("project");
        seed_wav(&storage, &project_path);
        genesis_adapter::commit_rows(&storage, vec![genesis_adapter::upsert("audio_chunks",
            serde_json::json!({"id":"c2", "recording_id":"r1", "sequence_no":2,
                "file_path":project_path.join("live/r1/chunks/system-00001.wav").display().to_string(),
                "start_ms":0, "end_ms":1, "byte_size":52, "checksum":"fixture",
                "created_at":"chunk-created", "transcribed_at":null}))]).unwrap();
        assert!(render_source_audio_channels(&storage, &test_runtime(), "p1", "r1").is_err());
        assert!(
            genesis_adapter::query_all(&storage, "export_artifacts", &["id"], vec![])
                .unwrap()
                .is_empty()
        );
        assert!(!project_path.join("exports").exists());
        drop(storage);
        let _ = std::fs::remove_dir_all(path);
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
