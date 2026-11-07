//! Read-only Tauri route inspection; blocking native enumeration stays off UI.
use crate::capture_routing::{
    self, CaptureRoutePlan, NativeInventory, Rejection, RouteError, ValidatedRoutePlan,
};

fn worker_error(error: impl std::fmt::Display) -> RouteError {
    RouteError {
        code: Rejection::InventoryError,
        message: format!("native inventory worker failed: {error}"),
    }
}

#[tauri::command]
pub(crate) async fn capture_route_inventory(
    process_ids: Option<Vec<u32>>,
) -> Result<NativeInventory, RouteError> {
    let process_ids = process_ids.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        capture_routing::read_native_inventory(&process_ids)
    })
    .await
    .map_err(worker_error)?
}

#[tauri::command]
pub(crate) async fn capture_route_validate(
    plan: CaptureRoutePlan,
) -> Result<ValidatedRoutePlan, RouteError> {
    // A client supplies intent only. It cannot deserialize an inventory or a
    // private validated token to bypass a fresh native admission observation.
    tauri::async_runtime::spawn_blocking(move || capture_routing::admit_route_plan(&plan))
        .await
        .map_err(worker_error)?
}

#[cfg(any(windows, test))]
fn publish_started_session<T, R>(
    slot: &mut Option<T>,
    session: T,
    shutting_down: impl FnOnce(&Option<T>) -> bool,
    request_stop: impl FnOnce(&T),
    response: impl FnOnce(&Option<T>) -> Result<R, String>,
) -> Result<R, String> {
    // Publish the control handle before checking shutdown or building a response;
    // either post-start path may fail while native work still needs an owner.
    *slot = Some(session);
    if shutting_down(slot) {
        request_stop(slot.as_ref().expect("started session was just published"));
        return Err("desktop shutdown interrupted routed start".into());
    }
    response(slot)
}

/// Explicit native execution. The caller provides intent; the production
/// coordinator performs fresh admission and creates all recording/source rows.
#[tauri::command]
pub(crate) async fn capture_route_start(
    project_id: String,
    plan: CaptureRoutePlan,
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        if state
            .shutdown_started
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err("desktop is shutting down".into());
        }
        let storage = std::sync::Arc::clone(&state.genesis);
        let guard = std::sync::Arc::clone(&state.native_capture);
        let sessions = std::sync::Arc::clone(&state.routed_live);
        let shutting_down = std::sync::Arc::clone(&state.shutdown_started);
        tauri::async_runtime::spawn_blocking(move || {
            let mut slot = sessions
                .lock()
                .map_err(|_| "routed session mutex poisoned")?;
            if shutting_down.load(std::sync::atomic::Ordering::Acquire) {
                return Err("desktop is shutting down".into());
            }
            if slot.as_ref().is_some_and(|session| !session.is_finished()) {
                return Err("a routed recording is already active".into());
            }
            // Dropping a finished handle cannot release or stop a new capture.
            slot.take();
            let session = crate::routed_capture::start(storage, project_id, plan, guard)?;
            publish_started_session(
                &mut slot,
                session,
                |_| shutting_down.load(std::sync::atomic::Ordering::Acquire),
                crate::routed_capture::RoutedCaptureHandle::request_stop,
                |slot| {
                    serde_json::to_value(
                        slot.as_ref()
                            .expect("started session was just published")
                            .status(),
                    )
                    .map_err(|e| e.to_string())
                },
            )
        })
        .await
        .map_err(|e| format!("routed capture worker: {e}"))?
    }
    #[cfg(not(windows))]
    {
        let _ = (project_id, plan, state);
        Err("routed endpoint capture requires Windows".into())
    }
}

#[tauri::command]
pub(crate) fn capture_route_status(
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        let slot = state
            .routed_live
            .try_lock()
            .map_err(|_| "routed operation in progress; retry status")?;
        slot.as_ref()
            .map_or(Ok(serde_json::Value::Null), |session| {
                serde_json::to_value(session.status()).map_err(|e| e.to_string())
            })
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Err("routed endpoint capture requires Windows".into())
    }
}

#[tauri::command]
pub(crate) async fn capture_route_stop(
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        let sessions = std::sync::Arc::clone(&state.routed_live);
        tauri::async_runtime::spawn_blocking(move || {
            let mut slot = sessions
                .lock()
                .map_err(|_| "routed session mutex poisoned")?;
            let session = slot.as_mut().ok_or("no routed recording is active")?;
            serde_json::to_value(session.stop_and_join()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("routed stop worker: {e}"))?
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Err("routed endpoint capture requires Windows".into())
    }
}

#[cfg(windows)]
pub(crate) fn shutdown_routed_capture(state: &crate::AppState) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let session = loop {
        match state.routed_live.try_lock() {
            Ok(mut slot) => break slot.take(),
            Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner().take(),
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                eprintln!("[capture] routed control operation still draining during shutdown");
                return false;
            }
        }
    };
    if let Some(mut session) = session {
        session.request_stop();
        while !session.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if session.is_finished() {
            session.join();
        } else {
            // Drop transfers unfinished work to its reaper; the native capture
            // reservation remains owned until the real clients have terminated.
            eprintln!("[capture] routed shutdown still draining; completion is unproven");
            return false;
        }
    }
    true
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "read-only native host endpoint inventory; explicit isolated verification only"]
    fn native_command_inventory_smoke() {
        let inventory =
            tauri::async_runtime::block_on(capture_route_inventory(Some(vec![std::process::id()])))
                .expect("read-only native inventory command");
        assert!(!inventory.endpoints.is_empty());
        assert!(inventory.errors.is_empty(), "{:?}", inventory.errors);
        assert!(matches!(
            inventory.defaults,
            capture_routing::Observation::Known(_)
        ));
        if let Ok(path) = std::env::var("FUNG_ROUTE_INVENTORY_RECEIPT") {
            let path = std::path::PathBuf::from(path);
            assert!(path.is_absolute() && !path.exists());
            std::fs::write(path, serde_json::to_vec_pretty(&inventory).unwrap()).unwrap();
        }
        eprintln!(
            "READ_ONLY_ROUTE_INVENTORY endpoints={}",
            inventory.endpoints.len()
        );
    }
}

#[cfg(test)]
mod started_session_ownership_tests {
    use super::publish_started_session;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Default)]
    struct Probe {
        stop_requested: AtomicBool,
        quiescent: AtomicBool,
        dropped: AtomicUsize,
        dropped_while_active: AtomicBool,
    }

    struct CaptureOwner(Arc<Probe>);

    impl CaptureOwner {
        fn request_stop(&self) {
            self.0.stop_requested.store(true, Ordering::SeqCst);
        }

        fn finish_shutdown(&mut self) {
            assert!(self.0.stop_requested.load(Ordering::SeqCst));
            self.0.quiescent.store(true, Ordering::SeqCst);
        }
    }

    impl Drop for CaptureOwner {
        fn drop(&mut self) {
            if !self.0.quiescent.load(Ordering::SeqCst) {
                self.0.dropped_while_active.store(true, Ordering::SeqCst);
            }
            self.0.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn new_capture() -> (CaptureOwner, Arc<Probe>) {
        let probe = Arc::new(Probe::default());
        (CaptureOwner(Arc::clone(&probe)), probe)
    }

    fn shutdown_to_quiescence(slot: &mut Option<CaptureOwner>, probe: &Probe) {
        let mut owner = slot
            .take()
            .expect("shutdown must retain access to the started capture");
        owner.request_stop();
        owner.finish_shutdown();
        drop(owner);
        assert!(probe.quiescent.load(Ordering::SeqCst));
        assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
        assert!(!probe.dropped_while_active.load(Ordering::SeqCst));
    }

    #[test]
    fn shutdown_interruption_keeps_control_until_quiescence() {
        let (capture, probe) = new_capture();
        let mut slot = None;

        let result = publish_started_session(
            &mut slot,
            capture,
            |slot| {
                assert!(
                    slot.is_some(),
                    "shutdown check must observe the published handle"
                );
                true
            },
            CaptureOwner::request_stop,
            |slot| {
                assert!(slot.is_some(), "response must observe the published handle");
                Ok(())
            },
        );

        assert_eq!(
            result,
            Err("desktop shutdown interrupted routed start".into())
        );
        assert!(
            slot.is_some(),
            "interrupted capture handle must stay published"
        );
        assert!(probe.stop_requested.load(Ordering::SeqCst));
        assert_eq!(probe.dropped.load(Ordering::SeqCst), 0);
        shutdown_to_quiescence(&mut slot, &probe);
    }

    #[test]
    fn failed_post_start_response_keeps_control_until_quiescence() {
        let (capture, probe) = new_capture();
        let mut slot = None;

        let result = publish_started_session(
            &mut slot,
            capture,
            |slot| {
                assert!(
                    slot.is_some(),
                    "shutdown check must observe the published handle"
                );
                false
            },
            CaptureOwner::request_stop,
            |slot| {
                assert!(slot.is_some(), "serialization must run after publication");
                Err::<(), String>("response serialization failed".into())
            },
        );

        assert_eq!(result, Err("response serialization failed".into()));
        assert!(
            slot.is_some(),
            "failed response must not drop the capture handle"
        );
        assert_eq!(probe.dropped.load(Ordering::SeqCst), 0);
        shutdown_to_quiescence(&mut slot, &probe);
    }
}
