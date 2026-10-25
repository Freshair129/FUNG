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
