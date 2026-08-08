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
