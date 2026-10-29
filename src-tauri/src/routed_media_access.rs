//! Temporary admission boundary for media consumers that only understand the
//! legacy mic/system/file layout. This does not implement routed playback.

use crate::genesis_adapter as ga;
use genesis_block_native::Storage;
use serde_json::{json, Value};

/// Read recording ownership from the SDK before a legacy consumer examines any
/// paths or PCM. Completed routed sources still need a source-aware consumer;
/// the custody completed-access guard alone cannot make filename guessing safe.
/// The returned row is metadata, not a source access lease.
pub(crate) fn require_legacy_media_source(
    storage: &Storage,
    recording_id: &str,
) -> Result<Value, String> {
    let row = ga::query(
        storage,
        "recordings",
        &["id", "project_id", "source", "status"],
        vec![ga::eq("recordings", "id", json!(recording_id))],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "recording not found for legacy media access".to_string())?;
    let project = ga::string(&row, "recordings.project_id")?;
    let source = ga::string(&row, "recordings.source")?;
    if project.is_empty() || source.is_empty() {
        return Err("missing recording media ownership".into());
    }
    if matches!(
        source.as_str(),
        "routed_endpoint" | "routed_endpoint_pending"
    ) {
        return Err(format!(
            "{source} media requires a source-aware consumer; legacy media access is unsupported"
        ));
    }
    // A damaged/repaired legacy marker cannot bypass the routed source set.
    // Query failures remain errors; they must never be interpreted as no sources.
    if !ga::query(
        storage,
        "capture_sources",
        &["id"],
        vec![ga::eq(
            "capture_sources",
            "recording_id",
            json!(recording_id),
        )],
        1,
    )?
    .is_empty()
    {
        return Err("routed_endpoint source bindings conflict with the legacy media marker".into());
    }
    Ok(row)
}

#[cfg(test)]
mod tests {
    use crate::{capture_source_custody as custody, genesis_adapter as ga};
    use genesis_block_native::{OpenOptions, Storage};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    const AT: &str = "2026-10-27T00:00:00Z";

    struct Fixture {
        _temp: tempfile::TempDir,
        storage: Storage,
        project: String,
        recording: String,
        root: PathBuf,
        source: custody::CaptureSourceRegistration,
        wav: PathBuf,
    }

    impl Fixture {
        fn attempt(&self) -> custody::SourceCommitAttempt {
            custody::begin_source_commit(&self.storage, &Uuid::new_v4().to_string(), AT).unwrap()
        }

        fn routed(state: &str) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("projects").join("project");
            fs::create_dir_all(&root).unwrap();
            let storage = Storage::open(OpenOptions {
                path: temp.path().join("db").display().to_string(),
                page_cache_mb: Some(16),
                read_only: Some(false),
                vector_dim: Some(4),
                retention: None,
            })
            .unwrap();
            ga::install(&storage).unwrap();
            let project = Uuid::new_v4().to_string();
            let recording = Uuid::new_v4().to_string();
            let source = custody::CaptureSourceRegistration {
                id: Uuid::new_v4().to_string(),
                epoch: 1,
                kind: "endpoint".into(),
                endpoint_id: Some("fixture-endpoint".into()),
                label: "Private routed source".into(),
                sample_rate: 48000,
                clock_origin_qpc_hns: "1234".into(),
                metadata: json!({"policyVersion": 1}),
            };
            ga::commit_rows(&storage, vec![
                ga::upsert("projects", json!({"id":project,"name":"test","storage_path":root,"created_at":"t","updated_at":"t"})),
                ga::upsert("recordings", json!({"id":recording,"project_id":project,"source":"microphone","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":AT,"updated_at":AT})),
                ga::upsert("mobile_recording_checkpoints", json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":"t"})),
            ]).unwrap();
            let wav = root
                .join("live")
                .join(&recording)
                .join("sources")
                .join(&source.id)
                .join("epoch-1")
                .join("chunk-1.wav");
            let f = Self {
                _temp: temp,
                storage,
                project,
                recording,
                root,
                source,
                wav,
            };
            let other_source = custody::CaptureSourceRegistration {
                id: Uuid::new_v4().to_string(),
                endpoint_id: Some("fixture-other-endpoint".into()),
                label: "Other private routed source".into(),
                ..f.source.clone()
            };
            let sources = [f.source.clone(), other_source];
            custody::register_capture_sources(
                &f.storage,
                &f.attempt(),
                &f.project,
                &f.recording,
                &sources,
            )
            .unwrap();
            for (ordinal, source) in sources.iter().enumerate() {
                let wav = f
                    .root
                    .join("live")
                    .join(&f.recording)
                    .join("sources")
                    .join(&source.id)
                    .join("epoch-1")
                    .join("chunk-1.wav");
                fs::create_dir_all(wav.parent().unwrap()).unwrap();
                let mut writer = hound::WavWriter::create(
                    &wav,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: 48000,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for frame in 0..4800 {
                    writer
                        .write_sample(((frame + ordinal * 100) % 1000) as i16)
                        .unwrap();
                }
                writer.finalize().unwrap();
                let chunk = custody::CaptureSourceChunk {
                    id: Uuid::new_v4().to_string(),
                    source_id: source.id.clone(),
                    epoch: 1,
                    sequence: 1,
                    path: wav.clone(),
                    start_frame: 0,
                    end_frame: 4800,
                    start_ms: 0,
                    end_ms: 100,
                    checksum: format!("{:x}", Sha256::digest(fs::read(&wav).unwrap())),
                };
                custody::append_capture_source_chunk(
                    &f.storage,
                    &f.attempt(),
                    &f.project,
                    &f.recording,
                    &chunk,
                )
                .unwrap();
            }
            match state {
                "completed" => {
                    custody::complete_capture_sources(&f.storage, &f.attempt(), &f.recording)
                        .unwrap();
                    custody::require_capture_source_access(&f.storage, &f.recording).unwrap();
                }
                "quarantined" => {
                    custody::quarantine_capture_source(
                        &f.storage,
                        &f.attempt(),
                        &f.source.id,
                        "fixture quarantine",
                    )
                    .unwrap();
                    custody::complete_capture_sources(&f.storage, &f.attempt(), &f.recording)
                        .unwrap();
                }
                "recording" => (),
                other => panic!("unknown fixture state {other}"),
            }
            assert!(
                fs::read(&f.wav).unwrap().len() > 44,
                "a readable PCM file is present"
            );
            f
        }
    }

    fn runtime() -> crate::WhisperRuntime {
        crate::WhisperRuntime {
            python: PathBuf::from("must-not-run-python"),
            script: PathBuf::from("must-not-run-worker"),
            cuda_bin: PathBuf::new(),
        }
    }

    #[test]
    fn desktop_playback_rejects_routed_sources_before_path_or_pcm_inspection() {
        for state in ["recording", "quarantined", "completed"] {
            let f = Fixture::routed(state);
            let mut stale_legacy_row = crate::recording_review::validate_recording_pair(
                &f.storage,
                &f.project,
                &f.recording,
            )
            .unwrap();
            // The fresh SDK owner must win over the caller's stale legacy row.
            stale_legacy_row.source = "import".into();
            for data_root in [
                f._temp.path().to_path_buf(),
                f._temp.path().join("absent-root"),
            ] {
                let result = crate::desktop_playback::tests::prepare_source_without_output(
                    &f.storage,
                    &data_root,
                    &stale_legacy_row,
                );
                assert_eq!(
                    result.unwrap_err().code,
                    "PLAYBACK_SOURCE_UNSUPPORTED",
                    "state={state}, root={data_root:?}"
                );
            }
        }
    }

    #[test]
    fn pending_routed_recording_never_enters_legacy_media_before_registration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        fs::create_dir(&root).unwrap();
        let storage = Storage::open(OpenOptions {
            path: temp.path().join("db").display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        ga::install(&storage).unwrap();
        let project = Uuid::new_v4().to_string();
        let recording = Uuid::new_v4().to_string();
        ga::commit_rows(&storage, vec![
            ga::upsert("projects", json!({"id":project,"name":"test","storage_path":root,"created_at":AT,"updated_at":AT})),
            ga::upsert("recordings", json!({"id":recording,"project_id":project,"source":"routed_endpoint_pending","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":AT,"updated_at":AT})),
            ga::upsert("mobile_recording_checkpoints", json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":AT})),
        ]).unwrap();
        assert!(ga::query_all(&storage, "capture_sources", &["id"], vec![])
            .unwrap()
            .is_empty());
        let mut stale_legacy_row =
            crate::recording_review::validate_recording_pair(&storage, &project, &recording)
                .unwrap();
        stale_legacy_row.source = "import".into();
        let error = crate::desktop_playback::tests::prepare_source_without_output(
            &storage,
            &temp.path().join("absent-root"),
            &stale_legacy_row,
        )
        .unwrap_err();
        assert_eq!(error.code, "PLAYBACK_SOURCE_UNSUPPORTED");
        assert!(
            matches!(crate::local_api::build_audio(&storage, &recording, None), Err(crate::local_api::AudioError::Unsupported(ref reason)) if reason.contains("routed_endpoint_pending"))
        );
        assert!(
            matches!(crate::audio_export::render_source_audio_channels(&storage, &runtime(), &project, &recording), Err(ref reason) if reason.contains("routed_endpoint_pending"))
        );
        assert!(
            matches!(crate::mobile::playback_manifest(&storage, &recording), Err(ref reason) if reason.to_string().contains("routed_endpoint_pending"))
        );
        assert!(
            matches!(crate::mobile::playback_segment(&storage, temp.path(), &recording, 1), Err(ref reason) if reason.to_string().contains("routed_endpoint_pending"))
        );
        assert!(ga::query_all(&storage, "export_artifacts", &["id"], vec![])
            .unwrap()
            .is_empty());
        assert!(!root.join("exports").exists());
        let metadata = crate::local_api::list_recordings(&storage).unwrap();
        assert_eq!(
            metadata["recordings"][0]["source"],
            "routed_endpoint_pending"
        );
        assert_eq!(metadata["recordings"][0]["channels"], json!([]));
    }

    #[test]
    fn routed_audio_never_reaches_legacy_local_pcm_even_when_completed() {
        for state in ["recording", "quarantined", "completed"] {
            let f = Fixture::routed(state);
            for channel in [None, Some("file"), Some("mic"), Some("system")] {
                let result = crate::local_api::build_audio(&f.storage, &f.recording, channel);
                assert!(
                    matches!(result, Err(crate::local_api::AudioError::Unsupported(ref reason)) if reason.contains("routed_endpoint")),
                    "state={state}, channel={channel:?}, returned PCM={}",
                    result.is_ok()
                );
            }
        }
    }

    #[test]
    fn routed_audio_never_publishes_legacy_export_even_when_completed() {
        for state in ["completed", "recording", "quarantined"] {
            let f = Fixture::routed(state);
            let result = crate::audio_export::render_source_audio_channels(
                &f.storage,
                &runtime(),
                &f.project,
                &f.recording,
            );
            assert!(
                matches!(result, Err(ref reason) if reason.contains("routed_endpoint")),
                "state={state}: {result:?}"
            );
            assert!(
                ga::query_all(&f.storage, "export_artifacts", &["id"], vec![])
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !f.root.join("exports").exists(),
                "no private staging or public export is created"
            );
        }
    }

    #[test]
    fn routed_audio_never_becomes_a_legacy_mobile_manifest() {
        for state in ["recording", "quarantined", "completed"] {
            let f = Fixture::routed(state);
            let result = crate::mobile::playback_manifest(&f.storage, &f.recording);
            assert!(
                matches!(result, Err(ref reason) if reason.to_string().contains("routed_endpoint")),
                "state={state}: {result:?}"
            );
        }
    }

    #[test]
    fn routed_audio_never_reaches_legacy_mobile_segment_reader() {
        for state in ["recording", "quarantined", "completed"] {
            let f = Fixture::routed(state);
            let result =
                crate::mobile::playback_segment(&f.storage, f._temp.path(), &f.recording, 1);
            assert!(
                matches!(result, Err(ref reason) if reason.to_string().contains("routed_endpoint")),
                "state={state}: {result:?}"
            );
        }
    }

    #[test]
    fn routed_legacy_media_rejection_survives_storage_reopen() {
        for state in ["recording", "quarantined", "completed"] {
            let Fixture {
                _temp: temp,
                storage,
                project,
                recording,
                ..
            } = Fixture::routed(state);
            drop(storage);
            let storage = Storage::open(OpenOptions {
                path: temp.path().join("db").display().to_string(),
                page_cache_mb: Some(16),
                read_only: Some(false),
                vector_dim: Some(4),
                retention: None,
            })
            .unwrap();
            assert!(crate::local_api::build_audio(&storage, &recording, None).is_err());
            assert!(crate::audio_export::render_source_audio_channels(
                &storage,
                &runtime(),
                &project,
                &recording
            )
            .is_err());
            assert!(crate::mobile::playback_manifest(&storage, &recording).is_err());
            assert!(crate::mobile::playback_segment(&storage, temp.path(), &recording, 1).is_err());
        }
    }

    #[test]
    fn legacy_marker_cannot_hide_routed_source_bindings() {
        let f = Fixture::routed("completed");
        ga::commit_rows(&f.storage, vec![ga::upsert("recordings", json!({
            "id":f.recording,"project_id":f.project,"source":"import","canonical_audio_path":f.wav,
            "status":"completed","duration_ms":100,"created_at":AT,"updated_at":AT
        }))]).unwrap();
        let result = super::require_legacy_media_source(&f.storage, &f.recording);
        assert!(matches!(result, Err(ref reason) if reason.contains("bindings conflict")));
        let row =
            crate::recording_review::validate_recording_pair(&f.storage, &f.project, &f.recording)
                .unwrap();
        for data_root in [
            f._temp.path().to_path_buf(),
            f._temp.path().join("absent-root"),
        ] {
            let error = crate::desktop_playback::tests::prepare_source_without_output(
                &f.storage, &data_root, &row,
            )
            .unwrap_err();
            assert_eq!(error.code, "PLAYBACK_SOURCE_UNSUPPORTED");
        }
        assert!(crate::local_api::build_audio(&f.storage, &f.recording, None).is_err());
        assert!(crate::audio_export::render_source_audio_channels(
            &f.storage,
            &runtime(),
            &f.project,
            &f.recording
        )
        .is_err());
    }

    #[test]
    fn missing_recording_ownership_never_falls_back_to_legacy_media() {
        let f = Fixture::routed("completed");
        assert!(super::require_legacy_media_source(&f.storage, "missing").is_err());
        ga::commit_rows(
            &f.storage,
            vec![ga::upsert(
                "recordings",
                json!({
                    "id":"missing-source","project_id":f.project,"source":"","canonical_audio_path":"unused",
                    "status":"completed","duration_ms":0,"created_at":"t","updated_at":"t"
                }),
            )],
        )
        .unwrap();
        assert!(super::require_legacy_media_source(&f.storage, "missing-source").is_err());
    }

    #[test]
    fn routed_metadata_remains_visible_without_inventing_a_file_channel() {
        let f = Fixture::routed("quarantined");
        let list = crate::local_api::list_recordings(&f.storage).unwrap();
        let row = list["recordings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == f.recording)
            .unwrap();
        assert_eq!(row["source"], "routed_endpoint");
        assert_eq!(row["status"], "partial");
        assert_eq!(row["chunkCount"], 2);
        assert_eq!(row["channels"], json!([]));
        assert_eq!(
            crate::recording_review::channel_for_path(
                &f.wav.display().to_string(),
                "routed_endpoint"
            ),
            None
        );
        crate::recording_review::validate_recording_pair(&f.storage, &f.project, &f.recording)
            .unwrap();
    }
}
