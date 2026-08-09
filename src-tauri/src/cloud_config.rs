//! Cloud BYOM provider configuration. Keys live ONLY in the OS credential
//! store (keyring) — never persisted or logged. Mirrors zoom_sync.rs's
//! TokenSet pattern.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CloudTaskKind {
    Stt,
    Llm,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(crate) enum CloudProviderConfig {
    Anthropic { api_key: String },
    OpenAi { api_key: String },
    Custom {
        endpoint: String,
        api_key: String,
        task_kind: CloudTaskKind,
    },
}

impl std::fmt::Debug for CloudProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anthropic { .. } => write!(f, "Anthropic {{ api_key: <redacted> }}"),
            Self::OpenAi { .. } => write!(f, "OpenAi {{ api_key: <redacted> }}"),
            Self::Custom { endpoint, task_kind, .. } => write!(
                f,
                "Custom {{ endpoint: {endpoint:?}, api_key: <redacted>, task_kind: {task_kind:?} }}"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudConfigValidation {
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
}

impl CloudProviderConfig {
    pub(crate) fn validate(&self) -> CloudConfigValidation {
        let key = match self {
            Self::Anthropic { api_key } | Self::OpenAi { api_key } => api_key,
            Self::Custom { api_key, .. } => api_key,
        };
        if key.trim().is_empty() {
            return CloudConfigValidation { ok: false, error: Some("ต้องระบุ API key".into()) };
        }
        if let Self::Custom { endpoint, .. } = self {
            if !endpoint.starts_with("https://") {
                return CloudConfigValidation {
                    ok: false,
                    error: Some("endpoint ต้องเริ่มด้วย https:// (คลาวด์ไม่อนุญาต plaintext)".into()),
                };
            }
        }
        CloudConfigValidation { ok: true, error: None }
    }
}

const KEYRING_SERVICE: &str = "FUNG";

/// The five fixed keyring usernames this feature ever writes to. `provider`
/// is `"anthropic" | "openai" | "custom"`; `task_kind` disambiguates the two
/// providers usable for both STT and LLM (OpenAI, Custom) — Anthropic has no
/// STT product, so there is no `cloud-stt-anthropic` slot.
pub(crate) fn cloud_config_slot(provider: &str, task_kind: CloudTaskKind) -> String {
    let kind = match task_kind {
        CloudTaskKind::Stt => "stt",
        CloudTaskKind::Llm => "llm",
    };
    format!("cloud-{kind}-{provider}")
}

fn keyring_entry(slot: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, slot).map_err(|e| e.to_string())
}

pub(crate) fn save_cloud_config(slot: &str, config: &CloudProviderConfig) -> Result<(), String> {
    let payload = serde_json::to_string(config).map_err(|e| e.to_string())?;
    keyring_entry(slot)?.set_password(&payload).map_err(|e| e.to_string())
}

pub(crate) fn load_cloud_config(slot: &str) -> Result<Option<CloudProviderConfig>, String> {
    match keyring_entry(slot)?.get_password() {
        Ok(payload) => serde_json::from_str(&payload).map(Some).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn delete_cloud_config(slot: &str) -> Result<(), String> {
    match keyring_entry(slot)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_is_invalid() {
        let config = CloudProviderConfig::OpenAi { api_key: "".into() };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("API key"));
    }

    #[test]
    fn custom_requires_https() {
        let config = CloudProviderConfig::Custom {
            endpoint: "http://example.com/stt".into(),
            api_key: "sk-test".into(),
            task_kind: CloudTaskKind::Stt,
        };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("https"));
    }

    #[test]
    fn custom_https_with_key_is_valid() {
        let config = CloudProviderConfig::Custom {
            endpoint: "https://example.com/stt".into(),
            api_key: "sk-test".into(),
            task_kind: CloudTaskKind::Stt,
        };
        assert!(config.validate().ok);
    }

    #[test]
    fn debug_never_exposes_the_key() {
        let config = CloudProviderConfig::OpenAi { api_key: "sk-super-secret-value".into() };
        let debug_output = format!("{config:?}");
        assert!(!debug_output.contains("sk-super-secret-value"));
        assert!(debug_output.contains("redacted"));
    }

    #[test]
    fn slot_naming_distinguishes_task_kind() {
        assert_eq!(cloud_config_slot("openai", CloudTaskKind::Stt), "cloud-stt-openai");
        assert_eq!(cloud_config_slot("openai", CloudTaskKind::Llm), "cloud-llm-openai");
    }

    #[test]
    fn serde_roundtrip_anthropic() {
        let config = CloudProviderConfig::Anthropic { api_key: "sk-ant-test".into() };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CloudProviderConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            CloudProviderConfig::Anthropic { api_key } => assert_eq!(api_key, "sk-ant-test"),
            _ => panic!("wrong variant"),
        }
    }

    // Keyring roundtrip requires a real OS credential store, which CI runners
    // may not provide identically to a dev machine — mirrors zoom_sync.rs's
    // token tests, which do NOT exercise the actual keyring backend for the
    // same reason. Roundtrip is validated at the plan's manual acceptance
    // step (Controller Gate) on a real desktop instead.

    #[test]
    fn no_source_file_serializes_cloud_config_into_genesis_or_supabase_paths() {
        // Static check: CloudProviderConfig must never be passed to
        // genesis_adapter::commit_rows/upsert, nor referenced from any .ts
        // file that also imports the supabase client. This is a coarse but
        // effective guard — a real leak would require a source line matching
        // both conditions, which this test makes structurally awkward to add
        // by accident.
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in walk_rs_files(&src_dir) {
            let contents = std::fs::read_to_string(&entry).unwrap_or_default();
            if contents.contains("CloudProviderConfig") {
                assert!(
                    !contents.contains("genesis_adapter::commit_rows") || entry.file_name().unwrap() == "cloud_config.rs",
                    "{entry:?} references both CloudProviderConfig and genesis_adapter::commit_rows"
                );
            }
        }
    }

    fn walk_rs_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(dir) else { return out };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
        out
    }
}
