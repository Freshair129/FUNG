// src-tauri/src/cloud_executor.rs
//! Cloud STT/LLM dispatch. Mirrors tts_executor.rs's HTTP-call conventions:
//! bounded timeout, truncated+redacted errors, never logs the request (which
//! is where the key lives).

use crate::cloud_config::CloudProviderConfig;
use crate::fungwire::Segment;
use std::path::Path;
use std::time::Duration;

const STT_TIMEOUT: Duration = Duration::from_secs(120);
const LLM_TIMEOUT: Duration = Duration::from_secs(60);

fn truncated(body: &str) -> &str {
    if body.len() <= 500 {
        return body;
    }
    let end = body.char_indices().take_while(|(i, _)| *i < 500).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
    &body[..end]
}

pub(crate) fn dispatch_stt(config: &CloudProviderConfig, audio_path: &Path) -> Result<Vec<Segment>, String> {
    match config {
        CloudProviderConfig::OpenAi { api_key } => openai_stt(api_key, audio_path),
        CloudProviderConfig::Custom { endpoint, api_key, .. } => custom_stt(endpoint, api_key, audio_path),
        CloudProviderConfig::Anthropic { .. } => Err("Anthropic ไม่มีบริการ STT".into()),
    }
}

pub(crate) fn dispatch_llm(config: &CloudProviderConfig, prompt: &str) -> Result<String, String> {
    match config {
        CloudProviderConfig::Anthropic { api_key } => anthropic_llm(api_key, prompt),
        CloudProviderConfig::OpenAi { api_key } => openai_llm(api_key, prompt),
        CloudProviderConfig::Custom { endpoint, api_key, .. } => custom_llm(endpoint, api_key, prompt),
    }
}

fn openai_stt(api_key: &str, audio_path: &Path) -> Result<Vec<Segment>, String> {
    #[derive(serde::Deserialize)]
    struct OpenAiSttSegment { start: f64, end: f64, text: String }
    #[derive(serde::Deserialize)]
    struct OpenAiSttResponse { segments: Vec<OpenAiSttSegment> }

    let file_name = audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();
    let bytes = std::fs::read(audio_path).map_err(|e| format!("อ่านไฟล์เสียงไม่ได้: {e}"))?;
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("response_format", "verbose_json");

    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .map_err(|e| if e.is_timeout() { "OpenAI STT ไม่ตอบสนองภายใน 120 วินาที".to_string() } else { format!("เชื่อมต่อ OpenAI STT ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("OpenAI STT ตอบ {status}: {}", truncated(&body)));
    }

    let parsed: OpenAiSttResponse = response.json().map_err(|e| format!("อ่าน response OpenAI STT ไม่ได้: {e}"))?;
    Ok(parsed
        .segments
        .into_iter()
        .map(|s| Segment {
            start_ms: (s.start * 1000.0).round() as i64,
            end_ms: (s.end * 1000.0).round() as i64,
            text: s.text,
            confidence: Some(1.0), // OpenAI's verbose_json has no per-segment confidence (spec §16, resolved)
        })
        .collect())
}

fn custom_stt(endpoint: &str, api_key: &str, audio_path: &Path) -> Result<Vec<Segment>, String> {
    let bytes = std::fs::read(audio_path).map_err(|e| format!("อ่านไฟล์เสียงไม่ได้: {e}"))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "audio/wav")
        .body(bytes)
        .send()
        .map_err(|e| if e.is_timeout() { "custom STT endpoint ไม่ตอบสนองภายใน 120 วินาที".to_string() } else { format!("เชื่อมต่อ custom STT endpoint ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("custom STT endpoint ตอบ {status}: {}", truncated(&body)));
    }
    response.json::<Vec<Segment>>().map_err(|e| format!("อ่าน response custom STT ไม่ได้: {e}"))
}

fn anthropic_llm(api_key: &str, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct ContentBlock { text: String }
    #[derive(serde::Deserialize)]
    struct MessagesResponse { content: Vec<ContentBlock> }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 2048,
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "Anthropic ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ Anthropic ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Anthropic ตอบ {status}: {}", truncated(&body)));
    }
    let parsed: MessagesResponse = response.json().map_err(|e| format!("อ่าน response Anthropic ไม่ได้: {e}"))?;
    parsed.content.into_iter().next().map(|c| c.text).ok_or_else(|| "Anthropic ตอบกลับไม่มีเนื้อหา".to_string())
}

fn openai_llm(api_key: &str, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Choice { message: ChoiceMessage }
    #[derive(serde::Deserialize)]
    struct ChoiceMessage { content: String }
    #[derive(serde::Deserialize)]
    struct ChatResponse { choices: Vec<Choice> }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "OpenAI ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ OpenAI ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("OpenAI ตอบ {status}: {}", truncated(&body)));
    }
    let parsed: ChatResponse = response.json().map_err(|e| format!("อ่าน response OpenAI ไม่ได้: {e}"))?;
    parsed.choices.into_iter().next().map(|c| c.message.content).ok_or_else(|| "OpenAI ตอบกลับไม่มีเนื้อหา".to_string())
}

fn custom_llm(endpoint: &str, api_key: &str, prompt: &str) -> Result<String, String> {
    // Same {endpoint}/api/chat Ollama-shaped contract graph_build.rs::call_llm
    // already speaks — a "custom" LLM endpoint needs no new wire format.
    #[derive(serde::Deserialize)]
    struct ChatMessage { content: String }
    #[derive(serde::Deserialize)]
    struct ChatResponse { message: ChatMessage }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| format!("สร้าง HTTP client ไม่ได้: {e}"))?;
    let response = client
        .post(format!("{endpoint}/api/chat"))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
        }))
        .send()
        .map_err(|e| if e.is_timeout() { "custom LLM endpoint ไม่ตอบสนองภายใน 60 วินาที".to_string() } else { format!("เชื่อมต่อ custom LLM endpoint ไม่ได้: {e}") })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("custom LLM endpoint ตอบ {status}: {}", truncated(&body)));
    }
    response.json::<ChatResponse>().map(|r| r.message.content).map_err(|e| format!("อ่าน response custom LLM ไม่ได้: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Spawns a one-shot HTTP server on 127.0.0.1 that reads one request and
    /// replies with `status_line` + `body`, then exits. Returns the bound
    /// "127.0.0.1:<port>" address.
    fn one_shot_server(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf); // drain the request, ignore contents
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        addr
    }

    #[test]
    fn custom_stt_parses_segment_array() {
        let addr = one_shot_server("HTTP/1.1 200 OK", r#"[{"start_ms":0,"end_ms":1200,"text":"hello","confidence":0.9}]"#);
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"fake-wav-bytes").unwrap();
        let segments = custom_stt(&format!("http://{addr}/stt"), "test-key", &audio_path).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "hello");
    }

    #[test]
    fn custom_stt_error_status_is_truncated_and_labeled() {
        let addr = one_shot_server("HTTP/1.1 401 Unauthorized", "invalid api key");
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"fake-wav-bytes").unwrap();
        let result = custom_stt(&format!("http://{addr}/stt"), "bad-key", &audio_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("401"));
    }

    #[test]
    fn custom_llm_parses_ollama_shaped_response() {
        let addr = one_shot_server("HTTP/1.1 200 OK", r#"{"message":{"content":"the answer"}}"#);
        let result = custom_llm(&format!("http://{addr}/x"), "test-key", "prompt").unwrap();
        assert_eq!(result, "the answer");
    }

    #[test]
    fn error_body_over_500_chars_is_truncated() {
        let long_body_owned = "x".repeat(1000);
        let long_body: &'static str = Box::leak(long_body_owned.into_boxed_str());
        let addr = one_shot_server("HTTP/1.1 500 Internal Server Error", long_body);
        let result = custom_llm(&format!("http://{addr}/x"), "test-key", "prompt");
        let message = result.unwrap_err();
        // "custom LLM endpoint ตอบ 500: " prefix + <=500 chars of body
        assert!(message.len() < 600);
    }

    #[test]
    fn anthropic_dispatch_stt_is_rejected_with_a_clear_message() {
        let config = CloudProviderConfig::Anthropic { api_key: "sk-ant-test".into() };
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"x").unwrap();
        let result = dispatch_stt(&config, &audio_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("STT"));
    }
}
