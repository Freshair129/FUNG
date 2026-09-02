//! Tauri command wrappers for cloud_config.rs/policy.rs. Lives in its own
//! module — never lib.rs — because it references CloudProviderConfig, and
//! lib.rs already contains calls into the genesis_adapter row-commit API for
//! unrelated commands; Task 2's leak-detection guard (cloud_config.rs)
//! forbids the two from co-existing in one file. See cloud_executor.rs's
//! identical rationale.

use crate::{cloud_config, paired_devices_connection, policy, AppError, AppResult, AppState};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudConfigInput {
    provider: String, // "anthropic" | "openai" | "custom"
    task_kind: cloud_config::CloudTaskKind,
    api_key: String,
    endpoint: Option<String>, // required when provider == "custom"
    /// Optional model override for the "anthropic"/"openai" providers (e.g.
    /// "claude-3-5-sonnet-20241022", "gpt-4o-mini", "whisper-1"). Ignored for
    /// "custom", which has no model concept of its own. `None`/absent falls
    /// back to cloud_executor.rs's hardcoded defaults.
    #[serde(default)]
    model: Option<String>,
}

#[tauri::command]
pub(crate) fn cloud_config_set(
    input: CloudConfigInput,
) -> AppResult<cloud_config::CloudConfigValidation> {
    let config = match input.provider.as_str() {
        "anthropic" => cloud_config::CloudProviderConfig::Anthropic {
            api_key: input.api_key,
            model: input.model,
        },
        "openai" => cloud_config::CloudProviderConfig::OpenAi {
            api_key: input.api_key,
            model: input.model,
        },
        "custom" => cloud_config::CloudProviderConfig::Custom {
            endpoint: input.endpoint.ok_or_else(|| {
                AppError::InvalidInput("endpoint required for custom provider".into())
            })?,
            api_key: input.api_key,
            task_kind: input.task_kind,
        },
        other => return Err(AppError::InvalidInput(format!("unknown provider: {other}"))),
    };
    let validation = config.validate();
    if !validation.ok {
        return Ok(validation);
    }
    let slot = cloud_config::cloud_config_slot(&input.provider, input.task_kind);
    cloud_config::save_cloud_config(&slot, &config).map_err(AppError::Cloud)?;
    Ok(validation)
}

#[tauri::command]
pub(crate) fn cloud_config_clear(
    provider: String,
    task_kind: cloud_config::CloudTaskKind,
) -> AppResult<()> {
    let slot = cloud_config::cloud_config_slot(&provider, task_kind);
    cloud_config::delete_cloud_config(&slot).map_err(AppError::Cloud)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudConfigStatus {
    provider: String,
    task_kind: cloud_config::CloudTaskKind,
    configured: bool,
}

#[tauri::command]
pub(crate) fn cloud_config_status() -> AppResult<Vec<CloudConfigStatus>> {
    let slots: &[(&str, cloud_config::CloudTaskKind)] = &[
        ("anthropic", cloud_config::CloudTaskKind::Llm),
        ("openai", cloud_config::CloudTaskKind::Stt),
        ("openai", cloud_config::CloudTaskKind::Llm),
        ("custom", cloud_config::CloudTaskKind::Stt),
        ("custom", cloud_config::CloudTaskKind::Llm),
    ];
    let mut out = Vec::with_capacity(slots.len());
    for (provider, task_kind) in slots {
        let slot = cloud_config::cloud_config_slot(provider, *task_kind);
        let configured = cloud_config::load_cloud_config(&slot)
            .map_err(AppError::Cloud)?
            .is_some();
        out.push(CloudConfigStatus {
            provider: provider.to_string(),
            task_kind: *task_kind,
            configured,
        });
    }
    Ok(out)
}

#[tauri::command]
pub(crate) fn tier_policy_get(state: State<'_, AppState>) -> AppResult<policy::TierPolicy> {
    let conn = paired_devices_connection(&state)?;
    policy::load_policy(&conn).map_err(AppError::Cloud)
}

#[tauri::command]
pub(crate) fn tier_policy_set(
    state: State<'_, AppState>,
    policy: policy::TierPolicy,
) -> AppResult<policy::TierPolicy> {
    let conn = paired_devices_connection(&state)?;
    policy::save_policy(&conn, &policy).map_err(AppError::Cloud)?;
    Ok(policy)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudCallCounts {
    stt: u32,
    llm: u32,
}

#[tauri::command]
pub(crate) fn cloud_call_counts_today(state: State<'_, AppState>) -> AppResult<CloudCallCounts> {
    let conn = paired_devices_connection(&state)?;
    Ok(CloudCallCounts {
        stt: policy::calls_today(&conn, cloud_config::CloudTaskKind::Stt)
            .map_err(AppError::Cloud)?,
        llm: policy::calls_today(&conn, cloud_config::CloudTaskKind::Llm)
            .map_err(AppError::Cloud)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_config_set_rejects_empty_key_without_touching_keyring() {
        let input = CloudConfigInput {
            provider: "openai".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Stt,
            api_key: "".into(),
            endpoint: None,
            model: None,
        };
        let result = cloud_config_set(input);
        assert!(result.is_ok()); // command succeeds; validation.ok is false
        assert!(!result.unwrap().ok);
    }

    #[test]
    fn cloud_config_set_custom_without_endpoint_is_rejected() {
        let input = CloudConfigInput {
            provider: "custom".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Stt,
            api_key: "key".into(),
            endpoint: None,
            model: None,
        };
        let result = cloud_config_set(input);
        assert!(result.is_err());
    }
}
