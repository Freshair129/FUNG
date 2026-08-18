//! Non-secret backup status boundary.
//!
//! The Genesis export/encrypt/filesystem job is intentionally not wired yet.
//! Until that controller exists, this command reports `unavailable` rather
//! than creating a mock or partial backup.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupTerminalState {
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupArchiveMetadata {
    archive_id: String,
    digest: String,
    byte_count: u64,
    timestamp: String,
    selected_root_id: String,
    relative_archive_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupStatus {
    terminal_state: BackupTerminalState,
    archive: Option<BackupArchiveMetadata>,
}

#[tauri::command]
pub(crate) fn backup_status() -> BackupStatus {
    BackupStatus {
        terminal_state: BackupTerminalState::Unavailable,
        archive: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backup_status_is_fail_closed_and_serializes_no_secret_material() {
        let status = backup_status();

        assert!(matches!(
            status.terminal_state,
            BackupTerminalState::Unavailable
        ));
        assert!(status.archive.is_none());
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!({
                "terminalState": "unavailable",
                "archive": null,
            })
        );
    }

    #[test]
    fn backup_status_boundary_has_no_secret_response_fields() {
        let source =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/backup.rs")).unwrap();

        for prohibited_field in [
            ["recovery", "Secret"].concat(),
            ["data", "Key"].concat(),
            ["provider", "Token"].concat(),
        ] {
            assert!(
                !source.contains(&prohibited_field),
                "backup status must not add {prohibited_field}"
            );
        }
    }
}
