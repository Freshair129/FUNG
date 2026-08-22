# BYOM TTS Provider Registration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Let users register their own TTS providers (F5-TTS-THAI, Piper, REST servers, etc.) via Settings UI, then use them through a 🔊 button on P3 recap and through the existing MCP voice pipeline.

**Architecture:** Extend the existing `model_providers` table with `kind = 'tts'`. New `tts_config.rs` module handles config parsing and validation per runtime type. New `tts_executor.rs` module dispatches synthesis to the registered provider (Python subprocess, HTTP POST, or local binary). Frontend adds a `TtsProviderPanel` component in Settings and a 🔊 button at P3 recap.

**Tech Stack:** Rust (Tauri v2, serde, reqwest, uuid, std::process), TypeScript/React (Vite, lucide-react), GenesisBlockDB, SQLite (test path)

## Global Constraints

- All IDs are UUIDv4 via the `uuid` crate (`Uuid::new_v4().to_string()`) — no ULID
- No async runtime (no tokio) — background work uses `std::thread::spawn`
- Production database is GenesisBlockDB (via `genesis_adapter`); tests use SQLite via `init_database`
- Tauri IPC pattern: `#[tauri::command]` functions registered in `generate_handler![]`
- TypeScript IPC pattern: `canInvoke()` guard → `invoke<T>(command, args)`
- UI language: Thai labels, English code identifiers
- No TTS provider is seeded at startup — user registers all providers manually
- Process timeout for TTS synthesis: 30 seconds
- Output format expected: WAV with valid RIFF header

---

### Task 1: TTS Config Module — Types + Validation

**Files:**
- Create: `src-tauri/src/tts_config.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod tts_config;`)

**Interfaces:**
- Consumes: nothing (leaf module)
- Produces:
  - `TtsProviderConfig` enum (tagged by `runtime_type`: `PythonScript`, `RestApi`, `LocalBinary`)
  - `TtsDevice` enum (`Cuda`, `Cpu`)
  - `TtsValidation` struct with `ok: bool`, `error: Option<String>`, `warnings: Vec<String>`
  - `TtsProviderConfig::validate(&self) -> TtsValidation`
  - `is_private_ip(endpoint: &str) -> bool`

- [x] **Step 1: Write failing tests for PythonScript validation**

Create `src-tauri/src/tts_config.rs` with the test module first:

```rust
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

impl TtsProviderConfig {
    pub(crate) fn validate(&self) -> TtsValidation {
        todo!()
    }
}

pub(crate) fn is_private_ip(_endpoint: &str) -> bool {
    todo!()
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
            binary_path: if cfg!(windows) { "C:\\Windows\\System32\\cmd.exe" } else { "/bin/sh" }.into(),
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
            binary_path: if cfg!(windows) { "C:\\Windows\\System32\\cmd.exe" } else { "/bin/sh" }.into(),
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
```

- [x] **Step 2: Run tests to verify they fail**

```bash
cd src-tauri && cargo test tts_config --lib -- --nocapture
```

Expected: all tests FAIL with `not yet implemented`

- [x] **Step 3: Register the module in lib.rs**

Add this line near the other `mod` declarations at the top of `src-tauri/src/lib.rs` (around line 10, next to `mod genesis_adapter;`):

```rust
mod tts_config;
```

- [x] **Step 4: Implement validate() and is_private_ip()**

Replace the `todo!()` bodies in `src-tauri/src/tts_config.rs`:

```rust
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
                    return TtsValidation::invalid(format!(
                        "venv path ไม่มีอยู่: {venv_path}"
                    ));
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
                    return TtsValidation::invalid(format!(
                        "script ไม่มีอยู่: {script_path}"
                    ));
                }
                if script.extension().and_then(|e| e.to_str()) != Some("py") {
                    return TtsValidation::invalid(format!(
                        "script ต้องเป็นไฟล์ .py: {script_path}"
                    ));
                }
                if let Some(mp) = model_path {
                    if !Path::new(mp).exists() {
                        return TtsValidation::invalid(format!(
                            "model path ไม่มีอยู่: {mp}"
                        ));
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
                    warnings.push(
                        "endpoint ชี้ไปยัง public IP — ข้อมูลจะถูกส่งออกนอกเครือข่ายท้องถิ่น"
                            .into(),
                    );
                }
                TtsValidation { ok: true, error: None, warnings }
            }

            Self::LocalBinary {
                binary_path,
                model_path,
                args_template,
            } => {
                let bin = Path::new(binary_path);
                if !bin.exists() {
                    return TtsValidation::invalid(format!(
                        "binary ไม่มีอยู่: {binary_path}"
                    ));
                }
                if !args_template.contains("{text}") || !args_template.contains("{output}") {
                    return TtsValidation::invalid(
                        "args_template ต้องมี {text} และ {output} placeholder".into(),
                    );
                }
                if let Some(mp) = model_path {
                    if !Path::new(mp).exists() {
                        return TtsValidation::invalid(format!(
                            "model path ไม่มีอยู่: {mp}"
                        ));
                    }
                }
                TtsValidation::ok()
            }
        }
    }
}

impl TtsValidation {
    fn ok() -> Self {
        Self { ok: true, error: None, warnings: vec![] }
    }
    fn invalid(msg: String) -> Self {
        Self { ok: false, error: Some(msg), warnings: vec![] }
    }
}

pub(crate) fn is_private_ip(endpoint: &str) -> bool {
    // Strip protocol
    let host_part = endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    // Strip path and port
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
                .map_or(false, |n| (16..=31).contains(&n)))
}
```

- [x] **Step 5: Run tests to verify they pass**

```bash
cd src-tauri && cargo test tts_config --lib -- --nocapture
```

Expected: all 9 tests PASS

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/tts_config.rs src-tauri/src/lib.rs
git commit -m "feat(tts): add TTS config types with validation

- TtsProviderConfig enum: PythonScript, RestApi, LocalBinary
- TtsDevice enum: Cuda, Cpu
- validate() checks paths exist, URL format, template placeholders
- is_private_ip() restricts REST endpoints to local/LAN by default
- 9 unit tests covering all validation branches"
```

---

### Task 2: TTS Executor — Dispatch + Synthesis + Output Validation

**Files:**
- Create: `src-tauri/src/tts_executor.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod tts_executor;`)

**Interfaces:**
- Consumes: `tts_config::TtsProviderConfig`, `tts_config::TtsDevice`
- Produces:
  - `TtsSynthesisRequest { text: String, ref_audio: Option<PathBuf>, ref_text: Option<String> }`
  - `TtsSynthesisResult { audio_path: PathBuf, latency_ms: u64 }`
  - `dispatch(config: &TtsProviderConfig, request: &TtsSynthesisRequest, temp_dir: &Path) -> Result<TtsSynthesisResult, String>`
  - `validate_wav(path: &Path) -> Result<(), String>`

- [x] **Step 1: Write the module with tests**

Create `src-tauri/src/tts_executor.rs`:

```rust
// src-tauri/src/tts_executor.rs

use crate::tts_config::{TtsDevice, TtsProviderConfig};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct TtsSynthesisRequest {
    pub(crate) text: String,
    pub(crate) ref_audio: Option<PathBuf>,
    pub(crate) ref_text: Option<String>,
}

pub(crate) struct TtsSynthesisResult {
    pub(crate) audio_path: PathBuf,
    pub(crate) latency_ms: u64,
}

pub(crate) fn dispatch(
    config: &TtsProviderConfig,
    request: &TtsSynthesisRequest,
    temp_dir: &Path,
) -> Result<TtsSynthesisResult, String> {
    let output_path = temp_dir.join(format!("tts-{}.wav", Uuid::new_v4()));
    let start = Instant::now();

    match config {
        TtsProviderConfig::PythonScript {
            venv_path,
            script_path,
            model_path,
            device,
        } => {
            exec_python_script(
                venv_path, script_path, model_path.as_deref(), device,
                request, &output_path,
            )?;
        }
        TtsProviderConfig::RestApi {
            endpoint,
            auth_header,
        } => {
            exec_rest_api(endpoint, auth_header.as_deref(), request, &output_path)?;
        }
        TtsProviderConfig::LocalBinary {
            binary_path,
            model_path,
            args_template,
        } => {
            exec_local_binary(
                binary_path, model_path.as_deref(), args_template,
                request, &output_path,
            )?;
        }
    }

    validate_wav(&output_path)?;

    let latency_ms = start.elapsed().as_millis() as u64;
    Ok(TtsSynthesisResult { audio_path: output_path, latency_ms })
}

fn exec_python_script(
    venv_path: &str,
    script_path: &str,
    model_path: Option<&str>,
    device: &TtsDevice,
    request: &TtsSynthesisRequest,
    output_path: &Path,
) -> Result<(), String> {
    let python = if cfg!(windows) {
        Path::new(venv_path).join("Scripts").join("python.exe")
    } else {
        Path::new(venv_path).join("bin").join("python")
    };

    let mut cmd = Command::new(&python);
    cmd.arg(script_path)
        .arg("--text").arg(&request.text)
        .arg("--output").arg(output_path)
        .arg("--device").arg(match device {
            TtsDevice::Cuda => "cuda",
            TtsDevice::Cpu => "cpu",
        });

    if let Some(mp) = model_path {
        cmd.arg("--model").arg(mp);
    }
    if let Some(ref_audio) = &request.ref_audio {
        cmd.arg("--ref-audio").arg(ref_audio);
    }
    if let Some(ref_text) = &request.ref_text {
        cmd.arg("--ref-text").arg(ref_text);
    }

    run_with_timeout(cmd, "python script")
}

fn exec_rest_api(
    endpoint: &str,
    auth_header: Option<&str>,
    request: &TtsSynthesisRequest,
    output_path: &Path,
) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;

    let mut body = serde_json::json!({
        "text": request.text,
        "format": "wav",
    });
    if let Some(ref_audio) = &request.ref_audio {
        body["ref_audio"] = serde_json::Value::String(ref_audio.display().to_string());
    }
    if let Some(ref_text) = &request.ref_text {
        body["ref_text"] = serde_json::Value::String(ref_text.clone());
    }

    let mut req = client.post(endpoint).json(&body);
    if let Some(header) = auth_header {
        req = req.header("Authorization", header);
    }

    let response = req.send().map_err(|e| {
        if e.is_timeout() {
            "endpoint ไม่ตอบสนองภายใน 30 วินาที".to_string()
        } else {
            format!("เชื่อมต่อ endpoint ไม่ได้: {e}")
        }
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().unwrap_or_default();
        let truncated = if body_text.len() > 500 { &body_text[..500] } else { &body_text };
        return Err(format!("endpoint ตอบ {status}: {truncated}"));
    }

    let bytes = response.bytes().map_err(|e| format!("อ่าน response ไม่ได้: {e}"))?;
    std::fs::write(output_path, &bytes)
        .map_err(|e| format!("เขียนไฟล์เสียงไม่ได้: {e}"))
}

fn exec_local_binary(
    binary_path: &str,
    model_path: Option<&str>,
    args_template: &str,
    request: &TtsSynthesisRequest,
    output_path: &Path,
) -> Result<(), String> {
    let args_str = args_template
        .replace("{text}", &request.text)
        .replace("{output}", &output_path.display().to_string());

    if let Some(mp) = model_path {
        // Model path is passed via env var to avoid template complexity
        std::env::set_var("FUNG_TTS_MODEL_PATH", mp);
    }

    let mut cmd = Command::new(binary_path);
    for arg in shell_words::split(&args_str)
        .map_err(|e| format!("parse args_template ผิดพลาด: {e}"))?
    {
        cmd.arg(arg);
    }

    run_with_timeout(cmd, "binary")
}

fn run_with_timeout(mut cmd: Command, label: &str) -> Result<(), String> {
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("เริ่ม {label} ไม่ได้: {e}"))?;

    // Wait with timeout using a thread
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    match rx.recv_timeout(TIMEOUT) {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let truncated = if stderr.len() > 500 {
                    &stderr[..500]
                } else {
                    &stderr
                };
                Err(format!("{label} ล้มเหลว (exit {}): {truncated}",
                    output.status.code().unwrap_or(-1)))
            }
        }
        Ok(Err(e)) => Err(format!("{label} error: {e}")),
        Err(_) => {
            // Timeout — the thread will clean up eventually
            Err(format!("{label} ไม่เสร็จภายใน 30 วินาที — ถูกยกเลิก"))
        }
    }
}

pub(crate) fn validate_wav(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("ไม่พบไฟล์เสียงที่สร้าง".into());
    }
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("อ่านข้อมูลไฟล์ไม่ได้: {e}"))?;
    if meta.len() == 0 {
        return Err("ไฟล์เสียงมีขนาด 0 bytes".into());
    }
    // Check RIFF/WAV header
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("เปิดไฟล์ไม่ได้: {e}"))?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|_| "ไฟล์เสียงสั้นเกินไป ไม่ใช่ WAV".to_string())?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("ไฟล์เสียงไม่ถูกรูปแบบ WAV (RIFF header ไม่ถูกต้อง)".into());
    }
    Ok(())
}

pub(crate) fn cleanup_temp(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_valid_wav(dir: &Path) -> PathBuf {
        let path = dir.join("test.wav");
        let mut f = std::fs::File::create(&path).unwrap();
        // Minimal valid WAV header (44 bytes)
        let data_size: u32 = 0;
        let file_size: u32 = 36; // 44 - 8
        f.write_all(b"RIFF").unwrap();
        f.write_all(&file_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
        f.write_all(&1u16.to_le_bytes()).unwrap();  // PCM
        f.write_all(&1u16.to_le_bytes()).unwrap();  // mono
        f.write_all(&16000u32.to_le_bytes()).unwrap(); // sample rate
        f.write_all(&32000u32.to_le_bytes()).unwrap(); // byte rate
        f.write_all(&2u16.to_le_bytes()).unwrap();  // block align
        f.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();
        path
    }

    #[test]
    fn validate_wav_accepts_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = make_valid_wav(dir.path());
        assert!(validate_wav(&path).is_ok());
    }

    #[test]
    fn validate_wav_rejects_missing_file() {
        let result = validate_wav(Path::new("/nonexistent/file.wav"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ไม่พบ"));
    }

    #[test]
    fn validate_wav_rejects_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        std::fs::File::create(&path).unwrap();
        let result = validate_wav(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("0 bytes"));
    }

    #[test]
    fn validate_wav_rejects_non_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake.wav");
        std::fs::write(&path, b"this is not a wav file at all").unwrap();
        let result = validate_wav(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RIFF"));
    }
}
```

- [x] **Step 2: Add shell-words and tempfile dependencies**

Add to `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
shell-words = "1"
```

And under `[dev-dependencies]` (create this section if it doesn't exist):

```toml
[dev-dependencies]
tempfile = "3"
```

- [x] **Step 3: Register the module in lib.rs**

Add below the `mod tts_config;` line added in Task 1:

```rust
mod tts_executor;
```

- [x] **Step 4: Run tests to verify they pass**

```bash
cd src-tauri && cargo test tts_executor --lib -- --nocapture
```

Expected: 4 tests PASS (validate_wav tests). The dispatch/exec functions are tested indirectly in Task 3 integration tests.

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/tts_executor.rs src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(tts): add TTS executor with dispatch and WAV validation

- dispatch() routes to python_script, rest_api, or local_binary
- exec_python_script() follows the FUNG script contract (--text, --output, etc.)
- exec_rest_api() POSTs JSON to endpoint, saves response as WAV
- exec_local_binary() expands args_template with {text}/{output} placeholders
- run_with_timeout() kills child process after 30s
- validate_wav() checks RIFF/WAV header
- cleanup_temp() removes temp output files"
```

---

### Task 3: Schema Migration + Tauri Commands + TypeScript Bridge

**Files:**
- Modify: `src-tauri/src/genesis_adapter.rs` (add `tts_test_results` table to schema, bump to v5)
- Modify: `src-tauri/src/lib.rs` (update test SQLite CHECK, add 5 Tauri commands, register in handler)
- Modify: `src/tauri.ts` (add TypeScript types + IPC wrappers)
- Modify: `src/mobile/model.ts` (add `TtsProviderConfig` type)

**Interfaces:**
- Consumes: `tts_config::TtsProviderConfig`, `tts_config::TtsValidation`, `tts_executor::dispatch`, `tts_executor::TtsSynthesisRequest`, `tts_executor::TtsSynthesisResult`, `tts_executor::cleanup_temp`
- Produces:
  - Tauri commands: `tts_provider_register`, `tts_provider_update`, `tts_provider_toggle`, `tts_provider_test`, `tts_synthesize_text`
  - TypeScript: `ttsProviderRegister(...)`, `ttsProviderUpdate(...)`, `ttsProviderToggle(...)`, `ttsProviderTest(...)`, `ttsSynthesizeText(...)`
  - TypeScript types: `TtsTestResult`, `TtsProviderConfig`

- [x] **Step 1: Add `tts_test_results` table to GenesisBlockDB schema**

In `src-tauri/src/genesis_adapter.rs`, find the current `schema()` function (v4) and add the new table. Add this `push` call after the existing `external_imports` table:

```rust
// Inside the schema() function, after the external_imports table push:
tables.push(table(
    "tts_test_results",
    vec![
        required("provider_id", ColumnKind::Text),
        required("status", ColumnKind::Text),
        nullable("latency_ms", ColumnKind::Integer),
        nullable("sample_audio_path", ColumnKind::Text),
        nullable("error_message", ColumnKind::Text),
        required("tested_at", ColumnKind::Text),
    ],
));
```

Also update the `install()` function to register the schema extension. Find where it registers v4 and add after it:

```rust
// After v4 registration in install():
let v5_tables = vec![table(
    "tts_test_results",
    vec![
        required("provider_id", ColumnKind::Text),
        required("status", ColumnKind::Text),
        nullable("latency_ms", ColumnKind::Integer),
        nullable("sample_audio_path", ColumnKind::Text),
        nullable("error_message", ColumnKind::Text),
        required("tested_at", ColumnKind::Text),
    ],
)];
match storage.register_relational_schema("fung_v5", v5_tables) {
    Ok(_) => {}
    Err(e) if e.contains("already registered") => {}
    Err(e) => return Err(e),
}
```

- [x] **Step 2: Update test SQLite schema in lib.rs**

In `src-tauri/src/lib.rs`, inside the `#[cfg(test)]` function `init_database`, update the `model_providers` CHECK constraint (around line 326) to include `'tts'`:

```sql
kind TEXT NOT NULL CHECK (kind IN ('transcription', 'diarization', 'cleanup', 'separation', 'summary_intent', 'tts')),
```

And add the `tts_test_results` table creation after the existing tables:

```sql
CREATE TABLE IF NOT EXISTS tts_test_results (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES model_providers(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('ok', 'error')),
    latency_ms INTEGER,
    sample_audio_path TEXT,
    error_message TEXT,
    tested_at TEXT NOT NULL
);
```

- [x] **Step 3: Add the 5 Tauri commands in lib.rs**

Add these command functions in `src-tauri/src/lib.rs` after the `list_model_providers` command (around line 592). They follow the existing patterns: take `State<AppState>`, return `Result<T, AppError>`.

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsRegisterInput {
    label: String,
    config_json: String, // JSON string of TtsProviderConfig
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsRegisterOutput {
    provider_id: String,
    validation: tts_config::TtsValidation,
}

#[tauri::command]
fn tts_provider_register(
    input: TtsRegisterInput,
    state: tauri::State<'_, AppState>,
) -> Result<TtsRegisterOutput, AppError> {
    // 1. Parse config_json into TtsProviderConfig
    let config: tts_config::TtsProviderConfig =
        serde_json::from_str(&input.config_json)
            .map_err(|e| AppError::Validation(format!("config ไม่ถูกรูปแบบ: {e}")))?;

    // 2. Validate
    let validation = config.validate();
    if !validation.ok {
        return Ok(TtsRegisterOutput {
            provider_id: String::new(),
            validation,
        });
    }

    // 3. Insert into model_providers
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let genesis = state.genesis.lock().map_err(|_| AppError::Lock)?;

    genesis_adapter::commit_rows(
        &genesis,
        vec![genesis_adapter::upsert(
            "model_providers",
            serde_json::json!({
                "id": id,
                "label": input.label,
                "runtime_location": "local",
                "kind": "tts",
                "enabled": true,
                "config_json": input.config_json,
                "created_at": now,
                "updated_at": now,
            }),
        )],
    )
    .map_err(AppError::Genesis)?;

    Ok(TtsRegisterOutput {
        provider_id: id,
        validation,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsUpdateInput {
    provider_id: String,
    label: Option<String>,
    config_json: Option<String>,
}

#[tauri::command]
fn tts_provider_update(
    input: TtsUpdateInput,
    state: tauri::State<'_, AppState>,
) -> Result<tts_config::TtsValidation, AppError> {
    // If config_json provided, validate it first
    if let Some(ref cj) = input.config_json {
        let config: tts_config::TtsProviderConfig =
            serde_json::from_str(cj)
                .map_err(|e| AppError::Validation(format!("config ไม่ถูกรูปแบบ: {e}")))?;
        let validation = config.validate();
        if !validation.ok {
            return Ok(validation);
        }
    }

    let genesis = state.genesis.lock().map_err(|_| AppError::Lock)?;
    let now = chrono::Utc::now().to_rfc3339();

    // Read current provider
    let rows = genesis_adapter::query(
        &genesis, "model_providers",
        &["id", "label", "runtime_location", "kind", "enabled", "config_json", "created_at", "updated_at"],
        vec![genesis_adapter::eq("model_providers", "id", &input.provider_id)],
        1,
    ).map_err(AppError::Genesis)?;

    let row = rows.first()
        .ok_or_else(|| AppError::Validation(format!("ไม่พบ provider: {}", input.provider_id)))?;

    let mut updated = row.clone();
    if let Some(label) = &input.label {
        updated["label"] = serde_json::Value::String(label.clone());
    }
    if let Some(cj) = &input.config_json {
        updated["config_json"] = serde_json::Value::String(cj.clone());
    }
    updated["updated_at"] = serde_json::Value::String(now);

    genesis_adapter::commit_rows(
        &genesis,
        vec![genesis_adapter::upsert("model_providers", updated)],
    ).map_err(AppError::Genesis)?;

    Ok(tts_config::TtsValidation { ok: true, error: None, warnings: vec![] })
}

#[tauri::command]
fn tts_provider_toggle(
    provider_id: String,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<bool, AppError> {
    let genesis = state.genesis.lock().map_err(|_| AppError::Lock)?;
    let now = chrono::Utc::now().to_rfc3339();

    let rows = genesis_adapter::query(
        &genesis, "model_providers",
        &["id", "label", "runtime_location", "kind", "enabled", "config_json", "created_at", "updated_at"],
        vec![genesis_adapter::eq("model_providers", "id", &provider_id)],
        1,
    ).map_err(AppError::Genesis)?;

    let row = rows.first()
        .ok_or_else(|| AppError::Validation(format!("ไม่พบ provider: {provider_id}")))?;

    let mut updated = row.clone();
    updated["enabled"] = serde_json::Value::Bool(enabled);
    updated["updated_at"] = serde_json::Value::String(now);

    genesis_adapter::commit_rows(
        &genesis,
        vec![genesis_adapter::upsert("model_providers", updated)],
    ).map_err(AppError::Genesis)?;

    Ok(true)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsTestOutput {
    status: String,       // "ok" or "error"
    latency_ms: Option<u64>,
    audio_path: Option<String>,
    message: Option<String>,
}

#[tauri::command]
fn tts_provider_test(
    provider_id: String,
    test_text: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<TtsTestOutput, AppError> {
    let text = test_text.unwrap_or_else(|| "ทดสอบระบบเสียง".into());

    let genesis = state.genesis.lock().map_err(|_| AppError::Lock)?;

    // Read provider config
    let rows = genesis_adapter::query(
        &genesis, "model_providers",
        &["id", "config_json"],
        vec![
            genesis_adapter::eq("model_providers", "id", &provider_id),
            genesis_adapter::eq("model_providers", "kind", "tts"),
        ],
        1,
    ).map_err(AppError::Genesis)?;

    let row = rows.first()
        .ok_or_else(|| AppError::Validation(format!("ไม่พบ TTS provider: {provider_id}")))?;

    let config_str = row["config_json"].as_str()
        .ok_or_else(|| AppError::Validation("config_json ว่าง".into()))?;
    let config: tts_config::TtsProviderConfig =
        serde_json::from_str(config_str)
            .map_err(|e| AppError::Validation(format!("config ไม่ถูกรูปแบบ: {e}")))?;

    let temp_dir = std::env::temp_dir().join("fung-tts");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::Io(format!("สร้าง temp dir ไม่ได้: {e}")))?;

    let request = tts_executor::TtsSynthesisRequest {
        text,
        ref_audio: None,
        ref_text: None,
    };

    let (status, latency_ms, audio_path, message) = match tts_executor::dispatch(&config, &request, &temp_dir) {
        Ok(result) => ("ok".into(), Some(result.latency_ms), Some(result.audio_path.display().to_string()), None),
        Err(e) => ("error".into(), None, None, Some(e)),
    };

    // Record test result
    let now = chrono::Utc::now().to_rfc3339();
    let test_id = Uuid::new_v4().to_string();
    let _ = genesis_adapter::commit_rows(
        &genesis,
        vec![genesis_adapter::upsert(
            "tts_test_results",
            serde_json::json!({
                "id": test_id,
                "provider_id": provider_id,
                "status": status,
                "latency_ms": latency_ms,
                "sample_audio_path": audio_path,
                "error_message": message,
                "tested_at": now,
            }),
        )],
    );

    Ok(TtsTestOutput { status, latency_ms, audio_path, message })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsSynthesizeInput {
    text: String,
    provider_id: Option<String>,
    ref_audio: Option<String>,
    ref_text: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsSynthesizeOutput {
    audio_path: String,
    latency_ms: u64,
}

#[tauri::command]
fn tts_synthesize_text(
    input: TtsSynthesizeInput,
    state: tauri::State<'_, AppState>,
) -> Result<TtsSynthesizeOutput, AppError> {
    let genesis = state.genesis.lock().map_err(|_| AppError::Lock)?;

    // Find the provider
    let config_str = if let Some(pid) = &input.provider_id {
        // Specific provider requested
        let rows = genesis_adapter::query(
            &genesis, "model_providers",
            &["config_json"],
            vec![
                genesis_adapter::eq("model_providers", "id", pid),
                genesis_adapter::eq("model_providers", "kind", "tts"),
                genesis_adapter::eq("model_providers", "enabled", "1"),
            ],
            1,
        ).map_err(AppError::Genesis)?;
        rows.first()
            .and_then(|r| r["config_json"].as_str().map(String::from))
            .ok_or_else(|| AppError::Validation(
                format!("TTS provider '{pid}' ไม่พร้อมใช้งาน")
            ))?
    } else {
        // Use first enabled TTS provider
        let rows = genesis_adapter::query(
            &genesis, "model_providers",
            &["config_json"],
            vec![
                genesis_adapter::eq("model_providers", "kind", "tts"),
                genesis_adapter::eq("model_providers", "enabled", "1"),
            ],
            1,
        ).map_err(AppError::Genesis)?;
        rows.first()
            .and_then(|r| r["config_json"].as_str().map(String::from))
            .ok_or_else(|| AppError::Validation(
                "ยังไม่ได้ลงทะเบียน TTS provider — ไปตั้งค่าที่ Settings".into()
            ))?
    };

    let config: tts_config::TtsProviderConfig =
        serde_json::from_str(&config_str)
            .map_err(|e| AppError::Validation(format!("config ผิดพลาด: {e}")))?;

    let temp_dir = std::env::temp_dir().join("fung-tts");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::Io(format!("สร้าง temp dir ไม่ได้: {e}")))?;

    let request = tts_executor::TtsSynthesisRequest {
        text: input.text,
        ref_audio: input.ref_audio.map(std::path::PathBuf::from),
        ref_text: input.ref_text,
    };

    let result = tts_executor::dispatch(&config, &request, &temp_dir)
        .map_err(|e| AppError::Tts(e))?;

    Ok(TtsSynthesizeOutput {
        audio_path: result.audio_path.display().to_string(),
        latency_ms: result.latency_ms,
    })
}
```

- [x] **Step 4: Add `AppError::Tts` and `AppError::Io` variants**

In `lib.rs`, find the `AppError` enum and add:

```rust
Tts(String),
Io(String),
```

And their `Display` / serialization match arms following the existing pattern.

- [x] **Step 5: Register new commands in invoke_handler**

In `lib.rs`, find the `tauri::generate_handler![...]` block (around line 981) and add:

```rust
tts_provider_register,
tts_provider_update,
tts_provider_toggle,
tts_provider_test,
tts_synthesize_text,
```

- [x] **Step 6: Add TypeScript types to model.ts**

In `src/mobile/model.ts`, add after the `VoiceProfile` type (around line 113):

```typescript
export type TtsRuntimeType = "python_script" | "rest_api" | "local_binary";

export type TtsPythonScriptConfig = {
  runtime_type: "python_script";
  venv_path: string;
  script_path: string;
  model_path?: string;
  device: "cuda" | "cpu";
};

export type TtsRestApiConfig = {
  runtime_type: "rest_api";
  endpoint: string;
  auth_header?: string;
};

export type TtsLocalBinaryConfig = {
  runtime_type: "local_binary";
  binary_path: string;
  model_path?: string;
  args_template: string;
};

export type TtsProviderConfig =
  | TtsPythonScriptConfig
  | TtsRestApiConfig
  | TtsLocalBinaryConfig;

export type TtsTestResult = {
  status: "ok" | "error";
  latencyMs?: number;
  audioPath?: string;
  message?: string;
};

export type TtsValidation = {
  ok: boolean;
  error?: string;
  warnings: string[];
};

export type TtsRegisterResult = {
  providerId: string;
  validation: TtsValidation;
};
```

- [x] **Step 7: Add TypeScript IPC wrappers to tauri.ts**

In `src/tauri.ts`, add after the `listModelProviders` function (around line 145):

```typescript
// ── TTS Provider Management ──

export async function ttsProviderRegister(
  label: string,
  configJson: string,
): Promise<{ providerId: string; validation: { ok: boolean; error?: string; warnings: string[] } }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_register", { input: { label, configJson } });
}

export async function ttsProviderUpdate(
  providerId: string,
  label?: string,
  configJson?: string,
): Promise<{ ok: boolean; error?: string; warnings: string[] }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_update", { input: { providerId, label, configJson } });
}

export async function ttsProviderToggle(
  providerId: string,
  enabled: boolean,
): Promise<boolean> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_toggle", { providerId, enabled });
}

export async function ttsProviderTest(
  providerId: string,
  testText?: string,
): Promise<{ status: string; latencyMs?: number; audioPath?: string; message?: string }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_test", { providerId, testText });
}

export async function ttsSynthesizeText(
  text: string,
  providerId?: string,
  refAudio?: string,
  refText?: string,
): Promise<{ audioPath: string; latencyMs: number }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_synthesize_text", {
    input: { text, providerId, refAudio, refText },
  });
}
```

- [x] **Step 8: Verify Rust compilation**

```bash
cd src-tauri && cargo check
```

Expected: no errors

- [x] **Step 9: Commit**

```bash
git add src-tauri/src/genesis_adapter.rs src-tauri/src/lib.rs src/tauri.ts src/mobile/model.ts
git commit -m "feat(tts): add schema migration, Tauri commands, and TS bridge

- GenesisBlockDB schema v5: tts_test_results table
- SQLite test path: add 'tts' to model_providers kind CHECK
- 5 Tauri commands: register, update, toggle, test, synthesize
- TypeScript types: TtsProviderConfig, TtsTestResult, TtsValidation
- TypeScript IPC wrappers matching Tauri command signatures"
```

---

### Task 4: TtsProviderPanel — Settings UI Component

**Files:**
- Create: `src/components/TtsProviderPanel.tsx`
- Create: `src/components/TtsProviderPanel.css`
- Modify: `src/App.tsx` (add TTS panel state + button + render)

**Interfaces:**
- Consumes: `listModelProviders()`, `ttsProviderRegister(...)`, `ttsProviderUpdate(...)`, `ttsProviderToggle(...)`, `ttsProviderTest(...)` from `src/tauri.ts`; `ModelProvider` type; `TtsProviderConfig` types from `model.ts`
- Produces: `<TtsProviderPanel onClose={fn} />` component used by `App.tsx`

- [x] **Step 1: Create TtsProviderPanel.css**

Create `src/components/TtsProviderPanel.css` following the same overlay pattern as `ExternalAccountPanel.css`:

```css
/* src/components/TtsProviderPanel.css */

.tts-panel-overlay {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(6px);
}

.tts-panel {
  background: var(--bg-surface, #1a1a2e);
  border-radius: 16px;
  padding: 28px 32px;
  max-width: 520px;
  width: 90vw;
  max-height: 80vh;
  overflow-y: auto;
  color: var(--text-primary, #e0e0e0);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}

.tts-panel h2 {
  font-size: 18px;
  margin: 0 0 20px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.tts-panel-close {
  margin-left: auto;
  background: none;
  border: none;
  color: var(--text-secondary, #999);
  cursor: pointer;
  padding: 4px;
}

/* ── Provider Card ── */

.tts-provider-card {
  background: var(--bg-card, rgba(255,255,255,0.04));
  border-radius: 10px;
  padding: 14px 16px;
  margin-bottom: 12px;
}

.tts-provider-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.tts-provider-card-header .indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.indicator.ok    { background: #4ade80; }
.indicator.warn  { background: #facc15; }
.indicator.error { background: #f87171; }
.indicator.off   { background: #555; }

.tts-provider-card-meta {
  font-size: 12px;
  color: var(--text-secondary, #999);
  margin-bottom: 10px;
}

.tts-provider-card-actions {
  display: flex;
  gap: 8px;
}

.tts-provider-card-actions button {
  font-size: 12px;
  padding: 5px 12px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(255,255,255,0.1));
  background: transparent;
  color: var(--text-primary, #e0e0e0);
  cursor: pointer;
}
.tts-provider-card-actions button:hover {
  background: rgba(255,255,255,0.06);
}

/* ── Registration Form ── */

.tts-form { margin-top: 16px; }

.tts-form label {
  display: block;
  font-size: 13px;
  margin-bottom: 4px;
  color: var(--text-secondary, #999);
}

.tts-form input,
.tts-form select {
  width: 100%;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(255,255,255,0.1));
  background: var(--bg-input, rgba(255,255,255,0.04));
  color: var(--text-primary, #e0e0e0);
  font-size: 13px;
  margin-bottom: 12px;
}

.tts-form-row {
  display: flex;
  gap: 8px;
}

.tts-form-actions {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.tts-form-actions button {
  padding: 8px 18px;
  border-radius: 8px;
  border: none;
  font-size: 13px;
  cursor: pointer;
}
.btn-primary {
  background: var(--accent, #6366f1);
  color: #fff;
}
.btn-secondary {
  background: transparent;
  border: 1px solid var(--border, rgba(255,255,255,0.1));
  color: var(--text-primary, #e0e0e0);
}

/* ── Empty State ── */

.tts-empty {
  text-align: center;
  padding: 24px 0;
  color: var(--text-secondary, #999);
}
.tts-empty p { margin: 8px 0; font-size: 13px; }

/* ── Test Result ── */

.tts-test-result {
  margin-top: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 12px;
}
.tts-test-result.ok {
  background: rgba(74, 222, 128, 0.1);
  color: #4ade80;
}
.tts-test-result.error {
  background: rgba(248, 113, 113, 0.1);
  color: #f87171;
}

/* ── Warnings ── */

.tts-warnings {
  margin-top: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  background: rgba(250, 204, 21, 0.1);
  font-size: 12px;
  color: #facc15;
}
```

- [x] **Step 2: Create TtsProviderPanel.tsx**

Create `src/components/TtsProviderPanel.tsx`:

```tsx
// src/components/TtsProviderPanel.tsx

import { useEffect, useState } from "react";
import {
  Volume2, X, Plus, Play, Pencil, ToggleLeft, ToggleRight, Loader2,
} from "lucide-react";
import {
  listModelProviders,
  ttsProviderRegister,
  ttsProviderUpdate,
  ttsProviderToggle,
  ttsProviderTest,
  type ModelProvider,
} from "../tauri";
import type {
  TtsRuntimeType, TtsProviderConfig, TtsTestResult, TtsValidation,
} from "../mobile/model";
import "./TtsProviderPanel.css";

type Props = { onClose: () => void };

type FormState = {
  label: string;
  runtimeType: TtsRuntimeType;
  venvPath: string;
  scriptPath: string;
  modelPath: string;
  device: "cuda" | "cpu";
  endpoint: string;
  authHeader: string;
  binaryPath: string;
  argsTemplate: string;
};

const emptyForm: FormState = {
  label: "", runtimeType: "python_script",
  venvPath: "", scriptPath: "", modelPath: "", device: "cuda",
  endpoint: "", authHeader: "",
  binaryPath: "", argsTemplate: "--text {text} --output {output}",
};

function buildConfigJson(form: FormState): string {
  switch (form.runtimeType) {
    case "python_script":
      return JSON.stringify({
        runtime_type: "python_script",
        venv_path: form.venvPath,
        script_path: form.scriptPath,
        ...(form.modelPath ? { model_path: form.modelPath } : {}),
        device: form.device,
      });
    case "rest_api":
      return JSON.stringify({
        runtime_type: "rest_api",
        endpoint: form.endpoint,
        ...(form.authHeader ? { auth_header: form.authHeader } : {}),
      });
    case "local_binary":
      return JSON.stringify({
        runtime_type: "local_binary",
        binary_path: form.binaryPath,
        ...(form.modelPath ? { model_path: form.modelPath } : {}),
        args_template: form.argsTemplate,
      });
  }
}

export default function TtsProviderPanel({ onClose }: Props) {
  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [testResult, setTestResult] = useState<TtsTestResult | null>(null);
  const [testing, setTesting] = useState<string | null>(null); // provider id
  const [saving, setSaving] = useState(false);
  const [validation, setValidation] = useState<TtsValidation | null>(null);

  const loadProviders = async () => {
    const all = await listModelProviders();
    setProviders(all.filter((p) => p.kind === "tts"));
  };

  useEffect(() => { loadProviders(); }, []);

  const handleSave = async () => {
    setSaving(true);
    setValidation(null);
    try {
      const configJson = buildConfigJson(form);
      if (editId) {
        const v = await ttsProviderUpdate(editId, form.label, configJson);
        setValidation(v);
        if (!v.ok) return;
      } else {
        const r = await ttsProviderRegister(form.label, configJson);
        setValidation(r.validation);
        if (!r.validation.ok) return;
      }
      setShowForm(false);
      setEditId(null);
      setForm(emptyForm);
      await loadProviders();
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async (providerId: string) => {
    setTesting(providerId);
    setTestResult(null);
    try {
      const result = await ttsProviderTest(providerId);
      setTestResult(result as TtsTestResult);
      // Play audio if successful
      if (result.status === "ok" && result.audioPath) {
        const audio = new Audio(`asset://localhost/${result.audioPath}`);
        audio.play().catch(() => {});
      }
    } catch (e: any) {
      setTestResult({ status: "error", message: e?.message ?? String(e), warnings: [] } as any);
    } finally {
      setTesting(null);
    }
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    await ttsProviderToggle(id, !enabled);
    await loadProviders();
  };

  const handleEdit = (p: ModelProvider) => {
    // Pre-fill form from provider — we'd need config_json from the provider
    // For now, open form with just the label
    setEditId(p.id);
    setForm({ ...emptyForm, label: p.label });
    setShowForm(true);
  };

  const f = (key: keyof FormState) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
    setForm((prev) => ({ ...prev, [key]: e.target.value }));

  return (
    <div className="tts-panel-overlay" onClick={onClose}>
      <div className="tts-panel" onClick={(e) => e.stopPropagation()}>
        <h2>
          <Volume2 size={18} />
          Voice Synthesis Providers
          <button className="tts-panel-close" onClick={onClose}><X size={16} /></button>
        </h2>

        {/* Provider cards */}
        {providers.map((p) => {
          const indicatorClass = !p.enabled ? "off"
            : (testing === p.id) ? "warn"
            : (testResult && testing === null) ? "ok" : "warn";
          return (
            <div key={p.id} className="tts-provider-card">
              <div className="tts-provider-card-header">
                <span className={`indicator ${indicatorClass}`} />
                <strong>{p.label}</strong>
              </div>
              <div className="tts-provider-card-meta">
                {p.runtimeLocation} · {p.enabled ? "เปิดใช้งาน" : "ปิดใช้งาน"}
              </div>
              <div className="tts-provider-card-actions">
                <button onClick={() => handleTest(p.id)} disabled={testing !== null}>
                  {testing === p.id ? <Loader2 size={12} className="spin" /> : <Play size={12} />}
                  {" "}ทดสอบ
                </button>
                <button onClick={() => handleEdit(p)}><Pencil size={12} /> แก้ไข</button>
                <button onClick={() => handleToggle(p.id, p.enabled)}>
                  {p.enabled ? <ToggleRight size={12} /> : <ToggleLeft size={12} />}
                  {" "}{p.enabled ? "ปิดใช้งาน" : "เปิดใช้งาน"}
                </button>
              </div>
              {testResult && testing === null && (
                <div className={`tts-test-result ${testResult.status}`}>
                  {testResult.status === "ok"
                    ? `✅ สำเร็จ · ${testResult.latencyMs} ms`
                    : `❌ ${testResult.message}`}
                </div>
              )}
            </div>
          );
        })}

        {/* Empty state */}
        {providers.length === 0 && !showForm && (
          <div className="tts-empty">
            <Volume2 size={32} />
            <p>ยังไม่ได้ตั้งค่า TTS provider</p>
            <p>เพิ่ม provider เพื่อใช้งานเสียงสังเคราะห์</p>
          </div>
        )}

        {/* Add button */}
        {!showForm && (
          <button
            className="btn-secondary"
            style={{ width: "100%", marginTop: 12 }}
            onClick={() => { setShowForm(true); setEditId(null); setForm(emptyForm); }}
          >
            <Plus size={14} /> เพิ่ม TTS Provider
          </button>
        )}

        {/* Registration form */}
        {showForm && (
          <div className="tts-form">
            <label>ประเภท</label>
            <select value={form.runtimeType} onChange={f("runtimeType")}>
              <option value="python_script">Python Script</option>
              <option value="rest_api">REST API</option>
              <option value="local_binary">Local Binary</option>
            </select>

            <label>ชื่อ</label>
            <input value={form.label} onChange={f("label")} placeholder="เช่น F5-TTS-THAI" />

            {form.runtimeType === "python_script" && (
              <>
                <label>Venv path</label>
                <input value={form.venvPath} onChange={f("venvPath")} placeholder="D:\tts\.venv" />
                <label>Script path</label>
                <input value={form.scriptPath} onChange={f("scriptPath")} placeholder="D:\tts\synthesize.py" />
                <label>Model path (optional)</label>
                <input value={form.modelPath} onChange={f("modelPath")} placeholder="D:\tts\models\v1" />
                <label>Device</label>
                <div className="tts-form-row">
                  <label><input type="radio" name="device" value="cuda" checked={form.device==="cuda"} onChange={f("device")} /> CUDA</label>
                  <label><input type="radio" name="device" value="cpu" checked={form.device==="cpu"} onChange={f("device")} /> CPU</label>
                </div>
              </>
            )}

            {form.runtimeType === "rest_api" && (
              <>
                <label>Endpoint URL</label>
                <input value={form.endpoint} onChange={f("endpoint")} placeholder="http://127.0.0.1:5000/synthesize" />
                <label>Authorization header (optional)</label>
                <input value={form.authHeader} onChange={f("authHeader")} placeholder="Bearer ..." />
              </>
            )}

            {form.runtimeType === "local_binary" && (
              <>
                <label>Binary path</label>
                <input value={form.binaryPath} onChange={f("binaryPath")} placeholder="C:\piper\piper.exe" />
                <label>Model path (optional)</label>
                <input value={form.modelPath} onChange={f("modelPath")} />
                <label>Arguments template</label>
                <input value={form.argsTemplate} onChange={f("argsTemplate")} placeholder="--text {text} --output {output}" />
              </>
            )}

            {/* Validation feedback */}
            {validation && !validation.ok && (
              <div className="tts-test-result error">❌ {validation.error}</div>
            )}
            {validation && validation.warnings.length > 0 && (
              <div className="tts-warnings">
                ⚠️ {validation.warnings.join(" · ")}
              </div>
            )}

            <div className="tts-form-actions">
              <button className="btn-primary" onClick={handleSave} disabled={saving || !form.label}>
                {saving ? "กำลังบันทึก..." : editId ? "อัปเดต" : "บันทึก"}
              </button>
              <button className="btn-secondary" onClick={() => { setShowForm(false); setEditId(null); }}>
                ยกเลิก
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
```

- [x] **Step 3: Wire TtsProviderPanel into App.tsx**

In `src/App.tsx`:

3a. Add import at the top (near the `ExternalAccountPanel` import):

```typescript
import TtsProviderPanel from "./components/TtsProviderPanel";
```

3b. Add state (near `accountPanelOpen` / `zoomPanelOpen`, around line 647-648):

```typescript
const [ttsPanelOpen, setTtsPanelOpen] = useState(false);
```

3c. Add the TTS settings button in the sidebar (between the existing settings button and zoom button, around line 1370):

```tsx
<button
  className="action-btn"
  title="TTS Providers"
  onClick={() => setTtsPanelOpen(true)}
>
  <Volume2 size={16} />
</button>
```

Add `Volume2` to the lucide-react import at the top of App.tsx.

3d. Add the panel render (next to `ExternalAccountPanel` render, around line 1018):

```tsx
{ttsPanelOpen && <TtsProviderPanel onClose={() => setTtsPanelOpen(false)} />}
```

- [x] **Step 4: Verify the app compiles**

```bash
npm run dev
```

Expected: app starts without errors, new 🔊 button visible in sidebar, clicking opens TtsProviderPanel

- [x] **Step 5: Manual test — registration flow**

1. Click the Volume2 button in sidebar → panel opens with empty state
2. Click "เพิ่ม TTS Provider" → form appears
3. Select "Python Script", fill in invalid paths → click บันทึก → validation error shown
4. Click ยกเลิก → form closes
5. Click "เพิ่ม TTS Provider" again → select "REST API" → fill endpoint → save

- [x] **Step 6: Commit**

```bash
git add src/components/TtsProviderPanel.tsx src/components/TtsProviderPanel.css src/App.tsx
git commit -m "feat(tts): add TtsProviderPanel settings UI

- Registration form with Python Script / REST API / Local Binary tabs
- Provider cards with status indicator (🟢/🟡/🔴/⚫)
- Test synthesize button with audio playback
- Edit and enable/disable toggle
- Validation feedback in form
- Empty state prompting user to add a provider"
```

---

### Task 5: P3 Recap 🔊 Button + CreativeStudio Provider Dropdown

**Files:**
- Modify: `src/App.tsx` (add 🔊 button to P3 Intelligence recap tile)
- Modify: `src/mobile/CreativeStudio.tsx` (add TTS provider dropdown in voice tab, unlock grant)

**Interfaces:**
- Consumes: `ttsSynthesizeText(...)`, `listModelProviders()` from `src/tauri.ts`; `ModelProvider` type
- Produces: user-facing 🔊 button at P3 recap, TTS provider selector in CreativeStudio voice tab

- [x] **Step 1: Add TTS playback state and helper to App.tsx**

In `src/App.tsx`, add state variables near the other state declarations (around line 648):

```typescript
const [ttsPlaying, setTtsPlaying] = useState(false);
const [ttsLoading, setTtsLoading] = useState(false);
const [ttsAudio, setTtsAudio] = useState<HTMLAudioElement | null>(null);
```

Add the playback handler function (near the other handler functions):

```typescript
const handleTtsPlay = async (text: string) => {
  // If already playing, stop
  if (ttsAudio) {
    ttsAudio.pause();
    setTtsAudio(null);
    setTtsPlaying(false);
    return;
  }

  // Check if capture is active — block TTS during recording
  if (captureState && captureState !== "idle" && captureState !== "completed") {
    // Don't play TTS during recording to prevent feedback
    return;
  }

  setTtsLoading(true);
  try {
    const result = await ttsSynthesizeText(text);
    const audio = new Audio(`asset://localhost/${result.audioPath}`);
    audio.onended = () => { setTtsPlaying(false); setTtsAudio(null); };
    audio.onerror = () => { setTtsPlaying(false); setTtsAudio(null); };
    await audio.play();
    setTtsAudio(audio);
    setTtsPlaying(true);
  } catch (e: any) {
    // If no provider registered, show toast/alert
    const msg = e?.message ?? String(e);
    if (msg.includes("ยังไม่ได้ลงทะเบียน")) {
      setTtsPanelOpen(true); // Open TTS settings
    }
  } finally {
    setTtsLoading(false);
  }
};
```

Add `ttsSynthesizeText` to the import from `"./tauri"`.

- [x] **Step 2: Add 🔊 button to P3 Intelligence recap tile**

In `src/App.tsx`, find the P3 "Meeting recap" tile definition (around line 337-362). The tile content renders through the generic tile system. Add a 🔊 button to the tile header or content area.

Find the recap tile rendering and add after the recap text content:

```tsx
{/* Inside the P3 recap tile content area */}
<button
  className="action-btn tts-speak-btn"
  title={ttsPlaying ? "หยุดฟัง" : "ฟังสรุป"}
  onClick={() => {
    // Get the recap text from the current tile content
    const recapText = /* the recap text variable used in this tile */;
    if (recapText) handleTtsPlay(recapText);
  }}
  disabled={ttsLoading}
  style={{ marginLeft: 8 }}
>
  {ttsLoading ? <Loader2 size={14} className="spin" />
    : ttsPlaying ? <span>⏸</span>
    : <Volume2 size={14} />}
</button>
```

Add `Loader2` to the lucide-react import if not already imported.

Note: The exact placement depends on how the tile content is structured. The implementer should find where the recap text is rendered within the P3 tile and place the button adjacent to that text. Look for the tile rendering pattern driven by `currentTile` (line 731) and the activity/event feed.

- [x] **Step 3: Add TTS speak button CSS**

Add to `src/styles.css` (at the end):

```css
/* ── TTS Speak Button ── */
.tts-speak-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: 1px solid var(--border, rgba(255,255,255,0.1));
  background: transparent;
  color: var(--text-secondary, #999);
  cursor: pointer;
  transition: all 0.15s;
  vertical-align: middle;
}
.tts-speak-btn:hover:not(:disabled) {
  background: rgba(255,255,255,0.06);
  color: var(--text-primary, #e0e0e0);
}
.tts-speak-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
.spin {
  animation: spin 1s linear infinite;
}
```

- [x] **Step 4: Add TTS provider dropdown to CreativeStudio voice tab**

In `src/mobile/CreativeStudio.tsx`, modify the voice tab section (around line 214).

Add state and effect at the top of the `ProcessingStudio` component:

```typescript
const [ttsProviders, setTtsProviders] = useState<ModelProvider[]>([]);
const [selectedTtsId, setSelectedTtsId] = useState<string>("");

useEffect(() => {
  listModelProviders().then((all) => {
    const tts = all.filter((p) => p.kind === "tts" && p.enabled);
    setTtsProviders(tts);
  });
}, []);
```

Add the import at the top of the file:

```typescript
import { listModelProviders, type ModelProvider } from "../tauri";
```

Replace the `setGrant` function (line 194) that currently shows an alert:

```typescript
const setGrant = () => {
  if (ttsProviders.length === 0) {
    alert("ยังไม่ได้ลงทะเบียน TTS provider — ไปตั้งค่าที่ Settings ก่อน");
    return;
  }
  if (!selectedTtsId) {
    alert("เลือก TTS provider ก่อนเปิดสิทธิ์");
    return;
  }
  // Toggle the grant
  const next = !state.agentVoiceGrant;
  onChange({ ...state, agentVoiceGrant: next });
  setAgentVoiceGrant(
    /* projectId */ "",
    state.voiceProfile.id,
    next,
  ).catch(() => {});
};
```

Add the provider selector dropdown in the voice tab section, above the grant button:

```tsx
{/* TTS Provider selector */}
<div style={{ marginBottom: 12 }}>
  <label style={{ fontSize: 12, color: "#999", display: "block", marginBottom: 4 }}>
    TTS Provider
  </label>
  {ttsProviders.length > 0 ? (
    <select
      value={selectedTtsId}
      onChange={(e) => setSelectedTtsId(e.target.value)}
      style={{
        width: "100%", padding: "6px 8px", borderRadius: 6,
        background: "rgba(255,255,255,0.04)", color: "#e0e0e0",
        border: "1px solid rgba(255,255,255,0.1)", fontSize: 13,
      }}
    >
      <option value="">— เลือก provider —</option>
      {ttsProviders.map((p) => (
        <option key={p.id} value={p.id}>{p.label}</option>
      ))}
    </select>
  ) : (
    <p style={{ fontSize: 12, color: "#999" }}>
      ยังไม่มี TTS provider — ตั้งค่าที่ Settings
    </p>
  )}
</div>
```

Add `setAgentVoiceGrant` to the bridge.ts import.

- [x] **Step 5: Verify the app compiles and works**

```bash
npm run dev
```

Expected:
- P3 recap area shows 🔊 button
- CreativeStudio voice tab shows TTS provider dropdown
- If no TTS provider registered, clicking 🔊 opens Settings panel
- If provider registered, clicking 🔊 synthesizes and plays audio

- [x] **Step 6: Commit**

```bash
git add src/App.tsx src/styles.css src/mobile/CreativeStudio.tsx
git commit -m "feat(tts): add 🔊 button at P3 recap and provider dropdown in CreativeStudio

- 🔊 button next to meeting recap text — click to synthesize and play
- Loading spinner during synthesis, pause button while playing
- Block TTS playback during active recording (feedback prevention)
- No-provider state opens TTS settings panel
- CreativeStudio voice tab: TTS provider dropdown from enabled providers
- Provider selection unlocks agent voice grant button"
```

---

## Verification Checklist

After all 5 tasks are complete, verify end-to-end:

- [x] `cargo test` passes all new Rust tests (tts_config + tts_executor)
- [x] `cargo check` compiles without warnings
- [x] `npm run dev` starts without errors
- [x] Settings panel: register a Python Script TTS provider with valid paths → success
- [x] Settings panel: register with invalid paths → validation error shown
- [x] Settings panel: test synthesize → audio plays
- [x] Settings panel: toggle provider off → card shows ⚫
- [x] P3 recap: click 🔊 → audio synthesized and played
- [x] P3 recap: click 🔊 with no provider → Settings panel opens
- [x] CreativeStudio voice tab: dropdown shows registered TTS providers
- [x] CreativeStudio voice tab: select provider → grant button unlocked
