// src-tauri/src/tts_config.rs

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub(crate) enum TtsProviderConfig {
    PythonScript {
        venv_path: String,
        script_path: String,
        model_path: Option<String>,
        device: TtsDevice,
    },
    RestApi {
        endpoint: String,
        auth_header: Option<String>,
    },
    LocalBinary {
        binary_path: String,
        model_path: Option<String>,
        args_template: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TtsDevice {
    Cuda,
    Cpu,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TtsValidation {
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
    pub(crate) warnings: Vec<String>,
}

impl TtsValidation {
    fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            warnings: vec![],
        }
    }
    fn invalid(msg: String) -> Self {
        Self {
            ok: false,
            error: Some(msg),
            warnings: vec![],
        }
    }
}

impl TtsProviderConfig {
    pub(crate) fn validate(&self) -> TtsValidation {
        match self {
            Self::PythonScript {
                venv_path,
                script_path,
                model_path,
                ..
            } => {
                let venv = Path::new(venv_path);
                if !venv.exists() {
                    return TtsValidation::invalid(format!("venv path ไม่มีอยู่: {venv_path}"));
                }
                let python = if cfg!(windows) {
                    venv.join("Scripts").join("python.exe")
                } else {
                    venv.join("bin").join("python")
                };
                if !python.exists() {
                    return TtsValidation::invalid(format!(
                        "ไม่พบ python ใน venv: {}",
                        python.display()
                    ));
                }
                let script = Path::new(script_path);
                if !script.exists() {
                    return TtsValidation::invalid(format!("script ไม่มีอยู่: {script_path}"));
                }
                if script.extension().and_then(|e| e.to_str()) != Some("py") {
                    return TtsValidation::invalid(format!("script ต้องเป็นไฟล์ .py: {script_path}"));
                }
                if let Some(mp) = model_path {
                    if !Path::new(mp).exists() {
                        return TtsValidation::invalid(format!("model path ไม่มีอยู่: {mp}"));
                    }
                }
                TtsValidation::ok()
            }

            Self::RestApi { endpoint, .. } => {
                if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                    return TtsValidation::invalid(
                        "endpoint ต้องเริ่มด้วย http:// หรือ https://".into(),
                    );
                }
                let mut warnings = Vec::new();
                if !is_private_ip(endpoint) {
                    warnings.push("endpoint ชี้ไปยัง public IP — ข้อมูลจะถูกส่งออกนอกเครือข่ายท้องถิ่น".into());
                }
                TtsValidation {
                    ok: true,
                    error: None,
                    warnings,
                }
            }

            Self::LocalBinary {
                binary_path,
                model_path,
                args_template,
            } => {
                let bin = Path::new(binary_path);
                if !bin.exists() {
                    return TtsValidation::invalid(format!("binary ไม่มีอยู่: {binary_path}"));
                }
                if !args_template.contains("{text}") || !args_template.contains("{output}") {
                    return TtsValidation::invalid(
                        "args_template ต้องมี {text} และ {output} placeholder".into(),
                    );
                }
                if let Some(mp) = model_path {
                    if !Path::new(mp).exists() {
                        return TtsValidation::invalid(format!("model path ไม่มีอยู่: {mp}"));
                    }
                }
                TtsValidation::ok()
            }
        }
    }
}

pub(crate) fn is_private_ip(endpoint: &str) -> bool {
    let host_part = endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = host_part.split('/').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");

    host == "localhost"
        || host == "127.0.0.1"
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|s| s.parse::<u8>().ok())
                .is_some_and(|n| (16..=31).contains(&n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_script_missing_venv_is_invalid() {
        let config = TtsProviderConfig::PythonScript {
            venv_path: "/nonexistent/path/.venv".into(),
            script_path: file!().into(), // this file exists
            model_path: None,
            device: TtsDevice::Cpu,
        };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("venv"));
    }

    #[test]
    fn rest_api_valid_localhost() {
        let config = TtsProviderConfig::RestApi {
            endpoint: "http://127.0.0.1:5000/synthesize".into(),
            auth_header: None,
        };
        let result = config.validate();
        assert!(result.ok);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn rest_api_public_url_warns() {
        let config = TtsProviderConfig::RestApi {
            endpoint: "https://api.example.com/tts".into(),
            auth_header: None,
        };
        let result = config.validate();
        assert!(result.ok); // not blocked, just warned
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn local_binary_template_missing_placeholders_is_invalid() {
        let config = TtsProviderConfig::LocalBinary {
            binary_path: if cfg!(windows) {
                "C:\\Windows\\System32\\cmd.exe"
            } else {
                "/bin/sh"
            }
            .into(),
            model_path: None,
            args_template: "--speak something".into(), // missing {text} and {output}
        };
        let result = config.validate();
        assert!(!result.ok);
        assert!(result.error.as_deref().unwrap().contains("{text}"));
    }

    #[test]
    fn local_binary_valid_template() {
        let config = TtsProviderConfig::LocalBinary {
            binary_path: if cfg!(windows) {
                "C:\\Windows\\System32\\cmd.exe"
            } else {
                "/bin/sh"
            }
            .into(),
            model_path: None,
            args_template: "--text {text} --output {output}".into(),
        };
        let result = config.validate();
        assert!(result.ok);
    }

    #[test]
    fn is_private_ip_localhost() {
        assert!(is_private_ip("http://127.0.0.1:5000/api"));
        assert!(is_private_ip("http://localhost:8080"));
    }

    #[test]
    fn is_private_ip_lan() {
        assert!(is_private_ip("http://192.168.1.100:5000"));
        assert!(is_private_ip("http://10.0.0.5:5000"));
    }

    #[test]
    fn is_private_ip_public() {
        assert!(!is_private_ip("https://api.example.com/tts"));
        assert!(!is_private_ip("http://8.8.8.8:5000"));
    }

    #[test]
    fn serde_roundtrip_python_script() {
        let config = TtsProviderConfig::PythonScript {
            venv_path: "/path/to/venv".into(),
            script_path: "/path/to/synth.py".into(),
            model_path: Some("/path/to/model".into()),
            device: TtsDevice::Cuda,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TtsProviderConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            TtsProviderConfig::PythonScript { device, .. } => {
                assert!(matches!(device, TtsDevice::Cuda));
            }
            _ => panic!("wrong variant"),
        }
    }
}
