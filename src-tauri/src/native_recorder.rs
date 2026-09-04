use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime, State, Wry,
};

use crate::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeSegment {
    pub(crate) sequence: i64,
    pub(crate) relative_path: String,
    pub(crate) duration_ms: i64,
    pub(crate) byte_size: i64,
    pub(crate) checksum: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeRecorderStatus {
    pub(crate) available: bool,
    pub(crate) recording_id: Option<String>,
    pub(crate) state: String,
    /// Live input amplitude 0-100 from the Android recorder; defaulted so
    /// older plugin payloads (and the desktop stub) deserialize as silent.
    #[serde(default)]
    pub(crate) level_percent: i64,
    pub(crate) safe_offset_ms: i64,
    pub(crate) segment_count: i64,
    pub(crate) segments: Vec<NativeSegment>,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingRequest<'a> {
    recording_id: &'a str,
}

pub(crate) struct NativeRecorder<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    marker: std::marker::PhantomData<fn() -> R>,
}

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mobile-recorder")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let recorder: NativeRecorder<R> = NativeRecorder {
                handle: _api
                    .register_android_plugin("dev.fung.local.recorder", "RecorderPlugin")?,
            };
            #[cfg(not(target_os = "android"))]
            let recorder: NativeRecorder<R> = NativeRecorder {
                marker: std::marker::PhantomData,
            };
            app.manage(recorder);
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
fn run(
    recorder: &NativeRecorder<Wry>,
    command: &str,
    recording_id: &str,
) -> AppResult<NativeRecorderStatus> {
    recorder
        .handle
        .run_mobile_plugin(command, RecordingRequest { recording_id })
        .map_err(|error| AppError::InvalidInput(format!("native recorder: {error}")))
}

#[cfg(not(target_os = "android"))]
fn run(
    _recorder: &NativeRecorder<Wry>,
    _command: &str,
    _recording_id: &str,
) -> AppResult<NativeRecorderStatus> {
    Ok(NativeRecorderStatus {
        available: false,
        recording_id: None,
        state: "unavailable".to_string(),
        level_percent: 0,
        safe_offset_ms: 0,
        segment_count: 0,
        segments: Vec::new(),
    })
}

#[tauri::command]
pub(crate) fn mobile_native_recorder_start(
    recording_id: String,
    recorder: State<'_, NativeRecorder<Wry>>,
) -> AppResult<NativeRecorderStatus> {
    run(&recorder, "start", &recording_id)
}

#[tauri::command]
pub(crate) fn mobile_native_recorder_status(
    recording_id: String,
    recorder: State<'_, NativeRecorder<Wry>>,
) -> AppResult<NativeRecorderStatus> {
    run(&recorder, "status", &recording_id)
}

#[tauri::command]
pub(crate) fn mobile_native_recorder_control(
    recording_id: String,
    action: String,
    recorder: State<'_, NativeRecorder<Wry>>,
) -> AppResult<NativeRecorderStatus> {
    let command = match action.as_str() {
        "pause" | "resume" | "stop" => action.as_str(),
        _ => {
            return Err(AppError::InvalidInput(
                "native recorder action must be pause, resume, or stop".to_string(),
            ))
        }
    };
    run(&recorder, command, &recording_id)
}
