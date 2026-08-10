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
        // Static check enforcing: CloudProviderConfig (Rust) and cloud config slots
        // (TS) must never coexist with Supabase, GenesisBlockDB, or localStorage.
        //
        // Checks:
        // 1. .rs files containing CloudProviderConfig must NOT use it in a
        //    persistence path:
        //    - genesis_adapter::commit_rows or genesis_adapter::upsert
        //    - "supabase" (any case)
        //    - "localStorage"
        //    Exception: this file (cloud_config.rs) is exempt.
        // 2. .ts/.tsx files importing supabase client must NOT contain:
        //    - "cloud_config" or "apiKey" in supabase insert/upsert/update chains
        //    This is a coarse but effective guard: a real leak would require both
        //    conditions, which this test makes structurally awkward to add by accident.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().expect("CARGO_MANIFEST_DIR is src-tauri");

        // Walk .rs files in src-tauri/src
        let rs_src_dir = manifest_dir.join("src");
        for entry in walk_source_files(&rs_src_dir, &["rs"]) {
            let contents = std::fs::read_to_string(&entry).unwrap_or_default();
            let file_name = entry.file_name().unwrap_or_default().to_string_lossy();

            if contents.contains("CloudProviderConfig") && file_name != "cloud_config.rs" {
                // Check for genesis_adapter violations
                assert!(
                    !contents.contains("genesis_adapter::commit_rows"),
                    "{entry:?} references both CloudProviderConfig and genesis_adapter::commit_rows"
                );
                assert!(
                    !contents.contains("genesis_adapter::upsert"),
                    "{entry:?} references both CloudProviderConfig and genesis_adapter::upsert"
                );

                // Check for supabase violations (case-insensitive)
                let lower = contents.to_lowercase();
                assert!(
                    !lower.contains("supabase"),
                    "{entry:?} references both CloudProviderConfig and supabase"
                );

                // Check for localStorage violations
                assert!(
                    !contents.contains("localStorage"),
                    "{entry:?} references both CloudProviderConfig and localStorage"
                );
            }
        }

        // Walk .ts/.tsx files in src/
        let ts_src_dir = repo_root.join("src");
        for entry in walk_source_files(&ts_src_dir, &["ts", "tsx"]) {
            let contents = std::fs::read_to_string(&entry).unwrap_or_default();

            // Check if file imports supabase client (common patterns)
            let has_supabase_import = contents.contains("from \"../lib/supabase\"") ||
                                      contents.contains("from '../lib/supabase'") ||
                                      contents.contains("from \"@supabase/supabase-js\"") ||
                                      contents.contains("from '@supabase/supabase-js'");

            if has_supabase_import {
                // If it imports supabase, it must not contain cloud_config or apiKey
                // in the context of data insertion (supabase.from(...).insert/upsert/update)
                assert!(
                    !contents.contains("cloud_config") && !contents.contains("apiKey"),
                    "{entry:?} imports supabase client but also references cloud_config or apiKey"
                );
            }
        }
    }

    fn walk_source_files(
        dir: &std::path::Path,
        extensions: &[&str],
    ) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(dir) else { return out };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk_source_files(&path, extensions));
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    out.push(path);
                }
            }
        }
        out
    }
}
