//! Native source catalog. No media paths or endpoint identifiers cross IPC.

use crate::{
    completed_source_media,
    recording_review::{self, ReviewError},
    AppState,
};
use genesis_block_native::Storage;
use serde::Serialize;
use tauri::{State, WebviewWindow};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackSourceCatalogItem {
    source_id: String,
    capture_epoch: u64,
    label: String,
    kind: String,
    sample_rate: u32,
    duration_frames: u64,
}

fn safe_wire_integer(value: i64) -> Result<u64, ReviewError> {
    if !(0..=MAX_SAFE_INTEGER).contains(&value) {
        return Err(ReviewError::new(
            "PLAYBACK_SOURCE_INVALID",
            "Source metadata is unavailable.",
            false,
        ));
    }
    Ok(value as u64)
}

fn project_catalog_error(code: &str) -> ReviewError {
    let (code, retryable) = match code {
        "RESOURCE_LIMIT" => ("RESOURCE_LIMIT", false),
        "PLAYBACK_OPEN_TIMEOUT" => ("PLAYBACK_OPEN_TIMEOUT", true),
        "PLAYBACK_SOURCE_CHANGED" => ("PLAYBACK_SOURCE_CHANGED", true),
        _ => ("PLAYBACK_SOURCE_INVALID", false),
    };
    ReviewError::new(code, "Source catalog is unavailable.", retryable)
}

fn catalog_for_recording(
    storage: &Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<PlaybackSourceCatalogItem>, ReviewError> {
    recording_review::validate_recording_pair(storage, project_id, recording_id)?;
    let manifest = completed_source_media::completed_manifest(storage, project_id, recording_id)
        .map_err(|code| project_catalog_error(&code))?;
    if manifest.project_id != project_id || manifest.recording_id != recording_id {
        return Err(ReviewError::new(
            "PLAYBACK_SOURCE_INVALID",
            "Source identity is inconsistent.",
            false,
        ));
    }
    manifest
        .sources
        .into_iter()
        .map(|source| {
            let duration = source
                .chunks
                .last()
                .ok_or_else(|| {
                    ReviewError::new(
                        "PLAYBACK_SOURCE_INVALID",
                        "Source metadata is unavailable.",
                        false,
                    )
                })?
                .chunk
                .end_frame;
            if source.epoch <= 0 || duration <= 0 {
                return Err(ReviewError::new(
                    "PLAYBACK_SOURCE_INVALID",
                    "Source metadata is unavailable.",
                    false,
                ));
            }
            Ok(PlaybackSourceCatalogItem {
                source_id: source.id,
                capture_epoch: safe_wire_integer(source.epoch)?,
                label: source.label,
                kind: source.kind,
                sample_rate: source.sample_rate,
                duration_frames: safe_wire_integer(duration)?,
            })
        })
        .collect()
}

#[tauri::command]
pub(crate) async fn desktop_playback_sources(
    window: WebviewWindow,
    state: State<'_, AppState>,
    project_id: String,
    recording_id: String,
) -> Result<Vec<PlaybackSourceCatalogItem>, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    let storage = std::sync::Arc::clone(&state.genesis);
    let sources = tauri::async_runtime::spawn_blocking(move || {
        catalog_for_recording(&storage, &project_id, &recording_id)
    })
    .await
    .map_err(|_| {
        ReviewError::new("NATIVE_UNAVAILABLE", "Source catalog is unavailable.", true)
    })??;
    if recording_review::trusted_main_owner(&window, &state)? != owner {
        return Err(ReviewError::new(
            "NATIVE_UNAVAILABLE",
            "The native desktop window is unavailable.",
            false,
        ));
    }
    Ok(sources)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::completed_source_media::tests::Fixture;

    #[test]
    fn catalog_projects_only_authoritative_source_identity_and_frame_metadata() {
        let f = Fixture::new(true);
        let sources = catalog_for_recording(&f.storage, &f.project, &f.recording).unwrap();
        assert_eq!(sources.len(), 2);
        for registration in &f.regs {
            let source = sources
                .iter()
                .find(|source| source.source_id == registration.id)
                .unwrap();
            assert_eq!(source.capture_epoch, registration.epoch as u64);
            assert_eq!(source.label, registration.label);
            assert_eq!(source.kind, registration.kind);
            assert_eq!(source.sample_rate, registration.sample_rate as u32);
            assert_eq!(
                source.duration_frames, 1,
                "sub-millisecond tails remain exact"
            );
        }
        let json = serde_json::to_value(&sources).unwrap();
        for source in json.as_array().unwrap() {
            let object = source.as_object().unwrap();
            assert_eq!(object.len(), 6);
            for key in [
                "sourceId",
                "captureEpoch",
                "label",
                "kind",
                "sampleRate",
                "durationFrames",
            ] {
                assert!(object.contains_key(key));
            }
        }
    }

    #[test]
    fn catalog_rejects_wrong_project_and_incomplete_recording() {
        let f = Fixture::new(true);
        assert!(
            catalog_for_recording(&f.storage, &uuid::Uuid::new_v4().to_string(), &f.recording)
                .is_err()
        );
        let unfinished = Fixture::new(false);
        assert!(catalog_for_recording(
            &unfinished.storage,
            &unfinished.project,
            &unfinished.recording
        )
        .is_err());
    }

    #[test]
    fn catalog_rejects_aggregate_after_one_source_is_quarantined() {
        let f = Fixture::new(true);
        let attempt = crate::capture_source_custody::begin_source_commit(
            &f.storage,
            &uuid::Uuid::new_v4().to_string(),
            "2026-10-31T00:00:00Z",
        )
        .unwrap();
        crate::capture_source_custody::quarantine_capture_source(
            &f.storage,
            &attempt,
            &f.regs[0].id,
            "test-quarantine",
        )
        .unwrap();
        assert!(catalog_for_recording(&f.storage, &f.project, &f.recording).is_err());
    }

    #[test]
    fn catalog_rejects_values_outside_javascript_safe_integer_range() {
        assert!(safe_wire_integer(0).is_ok());
        assert!(safe_wire_integer(9_007_199_254_740_991).is_ok());
        assert!(safe_wire_integer(-1).is_err());
        assert!(safe_wire_integer(9_007_199_254_740_992).is_err());
    }

    #[test]
    fn catalog_error_projection_retains_safe_bounded_codes() {
        for code in [
            "RESOURCE_LIMIT",
            "PLAYBACK_OPEN_TIMEOUT",
            "PLAYBACK_SOURCE_CHANGED",
        ] {
            assert_eq!(project_catalog_error(code).code, code);
        }
        let error = project_catalog_error("private path must not be exposed");
        assert_eq!(error.code, "PLAYBACK_SOURCE_INVALID");
        assert!(!error.message.contains("private path"));
    }
}
