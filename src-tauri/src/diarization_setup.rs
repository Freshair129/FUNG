//! Tauri commands behind the "enable speaker separation" flow.
//!
//! Every network action here starts from a button the user pressed: nothing
//! is downloaded, and no token is stored, on app start or from a probe.
//! The add-on install is long (≈240 MB) so it runs on its own thread and
//! reports through events; the model fetch is short enough to await.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::diarization_addon::{self, InstallProgress};
use crate::AppState;

pub(crate) const PROGRESS_EVENT: &str = "diarization-addon-progress";
pub(crate) const FINISHED_EVENT: &str = "diarization-addon-finished";

/// The cancel flag of the install in flight, if any. One install at a time:
/// two would race on the same staging directory.
static INSTALL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallFinished {
    pub(crate) ok: bool,
    pub(crate) code: Option<String>,
    pub(crate) detail: Option<String>,
}

fn install_running() -> bool {
    INSTALL.lock().map(|slot| slot.is_some()).unwrap_or(true)
}

/// Starts installing the add-on in the background and returns at once.
/// Progress arrives as `diarization-addon-progress`, the outcome as one
/// `diarization-addon-finished`.
#[tauri::command]
pub(crate) fn diarization_addon_install(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = INSTALL
            .lock()
            .map_err(|_| "install state is unavailable".to_string())?;
        if slot.is_some() {
            return Err("กำลังติดตั้งอยู่แล้ว".to_string());
        }
        *slot = Some(Arc::clone(&cancel));
    }

    let data_root = state.data_root.clone();
    let python = state.whisper_runtime_clone().python;
    std::thread::spawn(move || {
        let progress_app = app.clone();
        let on_progress = move |progress: InstallProgress| {
            let _ = progress_app.emit(PROGRESS_EVENT, progress);
        };
        let result = diarization_addon::install(&data_root, &python, &cancel, &on_progress);
        if let Ok(mut slot) = INSTALL.lock() {
            *slot = None;
        }
        let finished = match result {
            Ok(()) => InstallFinished {
                ok: true,
                code: None,
                detail: None,
            },
            Err(error) => {
                eprintln!("[diarization-addon] install stopped: {}", error.code());
                InstallFinished {
                    ok: false,
                    code: Some(error.code().to_string()),
                    detail: Some(error.detail()),
                }
            }
        };
        let _ = app.emit(FINISHED_EVENT, finished);
    });
    Ok(())
}

/// Asks the running install to stop. It finishes the current read, deletes
/// its partial file and reports `cancelled`.
#[tauri::command]
pub(crate) fn diarization_addon_cancel() -> Result<(), String> {
    if let Some(cancel) = INSTALL
        .lock()
        .map_err(|_| "install state is unavailable".to_string())?
        .as_ref()
    {
        cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Deletes the add-on, and the downloaded model too when asked. The token is
/// removed with the model, since it exists only to fetch it.
#[tauri::command]
pub(crate) fn diarization_addon_remove(
    include_model: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if install_running() {
        return Err("กำลังติดตั้งอยู่ ยกเลิกก่อนแล้วค่อยลบ".to_string());
    }
    diarization_addon::remove(&state.data_root).map_err(|error| error.to_string())?;
    if include_model {
        let cache = crate::diarization::hf_home(&state.data_root);
        if cache.exists() {
            std::fs::remove_dir_all(&cache).map_err(|error| error.to_string())?;
        }
        crate::diarization::delete_token()?;
    }
    Ok(())
}

/// Saves the Hugging Face token the user pasted. The value goes straight to
/// the OS credential store and is never returned to the UI.
#[tauri::command]
pub(crate) fn diarization_set_token(token: String) -> Result<(), String> {
    crate::diarization::store_token(&token)
}

#[tauri::command]
pub(crate) fn diarization_clear_token() -> Result<(), String> {
    crate::diarization::delete_token()
}

/// The pages the user must visit, by fixed key, so the UI cannot be made to
/// open an arbitrary URL. Both gated repos must be accepted: the pipeline
/// loads `segmentation-3.0` under the same account.
pub(crate) fn model_page_url(page: &str) -> Option<&'static str> {
    match page {
        "pipeline" => Some("https://huggingface.co/pyannote/speaker-diarization-3.1"),
        "segmentation" => Some("https://huggingface.co/pyannote/segmentation-3.0"),
        "tokens" => Some("https://huggingface.co/settings/tokens"),
        _ => None,
    }
}

/// Opens a Hugging Face page in the system browser, where the user signs in
/// and accepts the terms themselves.
#[tauri::command]
pub(crate) fn diarization_open_page(app: AppHandle, page: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = model_page_url(&page).ok_or_else(|| format!("unknown page {page}"))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FetchOutcome {
    pub(crate) ok: bool,
    /// `token_invalid`, `terms_not_accepted`, `network`, `not_ready` or
    /// `unknown`.
    pub(crate) code: Option<String>,
    pub(crate) detail: Option<String>,
}

/// The `MODEL_ACCESS <code>` line `diarize.py --fetch-only` prints, if any.
pub(crate) fn fetch_error_code(worker_error: &str) -> &'static str {
    for code in ["token_invalid", "terms_not_accepted", "network"] {
        if worker_error.contains(&format!("MODEL_ACCESS {code}")) {
            return code;
        }
    }
    "unknown"
}

/// Failures that prove the stored token cannot fetch the model. A network
/// failure proves nothing about the token, so it is kept.
pub(crate) fn token_is_unusable(code: &str) -> bool {
    matches!(code, "token_invalid" | "terms_not_accepted")
}

pub(crate) fn fetch_error_detail(code: &str) -> &'static str {
    match code {
        "token_invalid" => "token ไม่ถูกต้อง — ตรวจว่าคัดลอกมาครบ แล้วลองใหม่",
        "terms_not_accepted" => {
            "บัญชีนี้ยังไม่ได้ยอมรับเงื่อนไขของโมเดล — กดยอมรับในหน้า Hugging Face ทั้งสองหน้า แล้วลองใหม่"
        }
        "network" => "เชื่อมต่อ Hugging Face ไม่ได้ — ตรวจอินเทอร์เน็ตแล้วลองใหม่",
        "not_ready" => "ต้องติดตั้งส่วนเสริมก่อน",
        _ => "ดาวน์โหลดโมเดลไม่สำเร็จ — ลองใหม่อีกครั้ง",
    }
}

/// Downloads the gated model now, so a wrong token or unaccepted licence is
/// reported on the setup screen instead of failing the first real job.
#[tauri::command]
pub(crate) async fn diarization_fetch_model(
    state: State<'_, AppState>,
) -> Result<FetchOutcome, String> {
    let runtime = state.whisper_runtime_clone();
    let data_root = state.data_root.clone();
    let readiness = crate::diarization::probe(&runtime, &data_root);
    if !(readiness.runtime_present && readiness.worker_present && readiness.dependencies_present) {
        return Ok(FetchOutcome {
            ok: false,
            code: Some("not_ready".into()),
            detail: Some(fetch_error_detail("not_ready").into()),
        });
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::fetch_diarization_model(&runtime, &data_root)
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(match result {
        Ok(()) => FetchOutcome {
            ok: true,
            code: None,
            detail: None,
        },
        Err(error) => {
            let code = fetch_error_code(&error);
            eprintln!("[diarization-setup] model fetch failed: {code}");
            if token_is_unusable(code) {
                // A stored token counts as "ready" (the first job would
                // fetch the model), so a token Hugging Face just refused
                // must not stay behind and turn the status green.
                crate::diarization::delete_token()?;
            }
            FetchOutcome {
                ok: false,
                code: Some(code.into()),
                detail: Some(fetch_error_detail(code).into()),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_codes_map_to_their_own_message() {
        assert_eq!(
            fetch_error_code("worker exited with 3: MODEL_ACCESS token_invalid"),
            "token_invalid"
        );
        assert_eq!(
            fetch_error_code("MODEL_ACCESS terms_not_accepted"),
            "terms_not_accepted"
        );
        assert_eq!(fetch_error_code("MODEL_ACCESS network"), "network");
        assert_eq!(
            fetch_error_code("worker exited with 1: Traceback ..."),
            "unknown"
        );
        let messages: std::collections::HashSet<_> = [
            "token_invalid",
            "terms_not_accepted",
            "network",
            "not_ready",
            "unknown",
        ]
        .iter()
        .map(|code| fetch_error_detail(code))
        .collect();
        assert_eq!(messages.len(), 5, "each failure needs its own instruction");
    }

    #[test]
    fn a_refused_token_is_forgotten_but_a_network_failure_keeps_it() {
        assert!(token_is_unusable("token_invalid"));
        assert!(token_is_unusable("terms_not_accepted"));
        assert!(!token_is_unusable("network"));
        assert!(!token_is_unusable("unknown"));
        assert!(!token_is_unusable("not_ready"));
    }

    #[test]
    fn only_fixed_hugging_face_pages_can_be_opened() {
        for page in ["pipeline", "segmentation", "tokens"] {
            assert!(model_page_url(page)
                .unwrap()
                .starts_with("https://huggingface.co/"));
        }
        assert_eq!(model_page_url("https://evil.example"), None);
        assert!(model_page_url("pipeline")
            .unwrap()
            .ends_with(crate::diarization::DIARIZATION_MODEL));
    }
}
