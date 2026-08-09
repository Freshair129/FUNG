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

/// The MIME type to label an uploaded audio file with, derived from the file's
/// own extension rather than hard-coded.
///
/// FUNGWIRE's cloud path does not always upload a `.wav`: a *single*-segment
/// job skips the concat step entirely and uploads the raw `.m4a` segment it
/// received from the phone (`fungwire_server::dispatch_cloud_stt`). Labelling
/// those bytes `audio/wav` is a lie the OpenAI endpoint happens to forgive
/// (it sniffs the filename instead), but a strict "custom" endpoint is
/// entitled to reject it.
///
/// Anything unrecognised falls back to `audio/wav`, which is both the previous
/// hard-coded behaviour and what the concat step always produces.
pub(crate) fn mime_for_audio_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("m4a") => "audio/m4a",
        Some("wav") => "audio/wav",
        _ => "audio/wav",
    }
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
        .mime_str(mime_for_audio_path(audio_path))
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
        .header("Content-Type", mime_for_audio_path(audio_path))
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

// ---- Tier-3 LLM cloud fallback (spec §8) ---------------------------------
// The graph builder's LLM call is local-first: it talks to the user's own
// Ollama. When that machine simply isn't running Ollama, and the user has
// both enabled cloud LLM and configured a key, the same prompt is retried
// against their cloud provider instead of failing the build.
//
// This lives here rather than in graph_build.rs on purpose. cloud_config.rs's
// static leak guard (`no_source_file_serializes_cloud_config_into_*_paths`)
// forbids any source file from naming `CloudProviderConfig` alongside
// `genesis_adapter` writes, so that a key-bearing config can never be
// persisted into GenesisBlockDB by accident. graph_build.rs is a
// genesis-writing file (it commits `model_runs` rows carrying the LLM
// endpoint), so it must stay out of the key-bearing type's business — it
// calls [`call_llm_with_fallback`] and never sees a `CloudProviderConfig`.

/// True only for the specific failure this fallback exists to catch — Ollama
/// not running / not reachable. A malformed-response error (bad status,
/// unparseable body) is NOT masked by a silent cloud retry; it surfaces as a
/// real bug, matching `graph_build::call_llm`'s pre-existing behavior for
/// those cases.
///
/// This keys off the message `call_llm` builds for a failed `send()`, so the
/// two must not drift apart; `graph_build`'s
/// `call_llm_error_text_matches_what_the_cloud_fallback_keys_on` pins that
/// contract from the producing side (which is why this is `pub(crate)`).
pub(crate) fn is_connection_error(message: &str) -> bool {
    message.contains("LLM endpoint unreachable")
}

/// First-configured wins: Anthropic, then OpenAI, then Custom (documented
/// priority order, surfaced in CloudProvidersPanel per spec §16, resolved).
/// `None` means the user has configured no LLM cloud provider at all, which
/// makes the fallback a no-op and leaves the local error untouched.
///
/// The STT counterpart is `fungwire_server::resolve_stt_cloud_config`, which
/// consults only two slots because Anthropic has no STT product.
pub(crate) fn first_configured_llm_provider() -> Option<CloudProviderConfig> {
    use crate::cloud_config::{cloud_config_slot, load_cloud_config, CloudTaskKind};
    for provider in ["anthropic", "openai", "custom"] {
        let slot = cloud_config_slot(provider, CloudTaskKind::Llm);
        if let Ok(Some(config)) = load_cloud_config(&slot) {
            return Some(config);
        }
    }
    None
}

/// Wraps a local LLM call with the tier-3 cloud fallback.
///
/// `local_call` is the caller's own local-first attempt — in practice
/// `graph_build::call_llm` bound to its endpoint/model/prompt. Taking it as a
/// closure keeps the dependency pointing one way (graph_build → here) instead
/// of this module reaching back into the graph builder for its HTTP helper.
///
/// `cloud` is the first-configured LLM provider (see
/// [`first_configured_llm_provider`]); `None` if nothing is configured.
/// `calls_today` is read by the caller before this function, mirroring
/// `fungwire_server::dispatch_cloud_stt`'s convention, and `policy_conn` is
/// the same connection it was read from so that a successful dispatch can
/// charge the day's budget here — the only place that knows the cloud path
/// actually ran.
pub(crate) fn call_llm_with_fallback(
    local_call: impl FnOnce() -> Result<String, String>,
    prompt: &str,
    cloud: Option<&CloudProviderConfig>,
    policy: &crate::policy::TierPolicy,
    calls_today: u32,
    policy_conn: &rusqlite::Connection,
) -> Result<String, String> {
    match local_call() {
        Ok(text) => Ok(text),
        Err(e) if is_connection_error(&e) => {
            let Some(config) = cloud else { return Err(e) };
            match crate::policy::decide_cloud_tier(policy, crate::cloud_config::CloudTaskKind::Llm, calls_today, true) {
                crate::policy::TierDecision::Allow => {
                    let result = dispatch_llm(config, prompt);
                    // Charged only on success: a failed cloud round trip
                    // produced nothing, so it must not eat the daily cap.
                    if result.is_ok() {
                        let _ = crate::policy::increment_calls_today(policy_conn, crate::cloud_config::CloudTaskKind::Llm);
                    }
                    result
                }
                crate::policy::TierDecision::Blocked { reason } => {
                    Err(format!("Ollama unreachable and cloud fallback blocked ({reason}): {e}"))
                }
            }
        }
        Err(e) => Err(e),
    }
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
    fn mime_for_audio_path_labels_wav_as_audio_wav() {
        assert_eq!(mime_for_audio_path(Path::new("/tmp/concat.wav")), "audio/wav");
        // Extension casing comes from whoever named the file, not from us.
        assert_eq!(mime_for_audio_path(Path::new("/tmp/CONCAT.WAV")), "audio/wav");
    }

    /// The single-segment FUNGWIRE cloud job uploads the phone's raw
    /// `segment-0.m4a` with no concat step, so this is the case the old
    /// hard-coded `audio/wav` mislabelled.
    #[test]
    fn mime_for_audio_path_labels_m4a_as_audio_m4a() {
        assert_eq!(mime_for_audio_path(Path::new("/tmp/segment-0.m4a")), "audio/m4a");
    }

    #[test]
    fn mime_for_audio_path_falls_back_to_wav_for_unknown_or_missing_extension() {
        assert_eq!(mime_for_audio_path(Path::new("/tmp/audio.ogg")), "audio/wav");
        assert_eq!(mime_for_audio_path(Path::new("/tmp/audio")), "audio/wav");
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

    // ---- Tier-3 LLM cloud fallback --------------------------------------
    // The fallback's whole contract is "which of the two transports ran, and
    // did the daily counter move". The local side is injected as a closure
    // returning the exact error text `graph_build::call_llm` produces (that
    // text is pinned against the real function by graph_build's
    // `call_llm_error_text_matches_what_the_cloud_fallback_keys_on`), while
    // the cloud side goes through a real `dispatch_llm` HTTP round trip.
    // Each test gets its own in-memory policy DB so counter assertions stay
    // independent.

    /// The message `call_llm` produces when Ollama is not listening.
    const LOCAL_UNREACHABLE: &str = "LLM endpoint unreachable at http://127.0.0.1:11434: connection refused";
    /// The message `call_llm` produces when Ollama answers, badly.
    const LOCAL_BAD_STATUS: &str = "LLM endpoint returned 500 Internal Server Error";

    fn llm_cloud_policy(enabled: bool) -> crate::policy::TierPolicy {
        crate::policy::TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: enabled, daily_cap: 20 }
    }

    fn llm_calls_today(conn: &rusqlite::Connection) -> u32 {
        crate::policy::calls_today(conn, crate::cloud_config::CloudTaskKind::Llm).unwrap()
    }

    /// A stub cloud LLM endpoint speaking the Ollama-shaped custom contract.
    fn stub_cloud_provider() -> CloudProviderConfig {
        let addr = one_shot_server("HTTP/1.1 200 OK", r#"{"message":{"content":"cloud extraction result"}}"#);
        CloudProviderConfig::Custom {
            endpoint: format!("http://{addr}"),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        }
    }

    #[test]
    fn ollama_connection_failure_falls_back_to_cloud_when_enabled_and_configured() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "prompt", Some(&cloud_config), &llm_cloud_policy(true), 0, &policy_conn,
        );

        assert_eq!(result.unwrap(), "cloud extraction result");
        assert_eq!(
            llm_calls_today(&policy_conn), 1,
            "a successful cloud dispatch must consume one of the day's budgeted calls",
        );
    }

    #[test]
    fn ollama_connection_failure_with_cloud_disabled_returns_original_error() {
        let cloud_config = CloudProviderConfig::Custom {
            endpoint: "http://127.0.0.1:9".into(),
            api_key: "k".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "prompt", Some(&cloud_config), &llm_cloud_policy(false), 0, &policy_conn,
        );

        let error = result.unwrap_err();
        assert!(error.contains("Ollama"), "the blocked-fallback error must name Ollama: {error}");
        assert!(error.contains("cloud_disabled"), "the block reason must be surfaced: {error}");
        assert_eq!(
            llm_calls_today(&policy_conn), 0,
            "a blocked fallback dispatches nothing, so it must not consume a call",
        );
    }

    #[test]
    fn ollama_connection_failure_without_a_configured_provider_returns_original_error() {
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "prompt", None, &llm_cloud_policy(true), 0, &policy_conn,
        );

        assert_eq!(
            result.unwrap_err(), LOCAL_UNREACHABLE,
            "with no provider configured the local error must pass through verbatim",
        );
        assert_eq!(llm_calls_today(&policy_conn), 0);
    }

    #[test]
    fn a_non_connection_ollama_error_is_not_masked_by_the_cloud_fallback() {
        // A local Ollama that IS reachable and answers badly. That is a real
        // bug, not an "Ollama isn't running" condition, so it must surface
        // unchanged instead of being silently retried in the cloud.
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_BAD_STATUS.to_string()),
            "prompt", Some(&cloud_config), &llm_cloud_policy(true), 0, &policy_conn,
        );

        assert_eq!(
            result.unwrap_err(), LOCAL_BAD_STATUS,
            "a bad local response must surface as-is, not as a cloud result",
        );
        assert_eq!(
            llm_calls_today(&policy_conn), 0,
            "no cloud dispatch happened, so the counter must not move",
        );
    }

    #[test]
    fn a_successful_local_call_never_reaches_the_cloud_or_the_counter() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Ok("local extraction result".to_string()),
            "prompt", Some(&cloud_config), &llm_cloud_policy(true), 0, &policy_conn,
        );

        assert_eq!(result.unwrap(), "local extraction result");
        assert_eq!(llm_calls_today(&policy_conn), 0);
    }

    #[test]
    fn a_failed_cloud_dispatch_does_not_consume_a_call() {
        // Cloud is allowed and configured, but the provider is unreachable.
        let cloud_config = CloudProviderConfig::Custom {
            // Nothing listens on TCP port 1 — deterministic, immediate refusal.
            endpoint: "http://127.0.0.1:1".into(),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "prompt", Some(&cloud_config), &llm_cloud_policy(true), 0, &policy_conn,
        );

        assert!(result.is_err());
        assert_eq!(
            llm_calls_today(&policy_conn), 0,
            "a cloud round trip that produced nothing must not eat the daily cap",
        );
    }

    #[test]
    fn a_reached_daily_cap_blocks_the_fallback() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        let policy = llm_cloud_policy(true);

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "prompt", Some(&cloud_config), &policy, policy.daily_cap, &policy_conn,
        );

        let error = result.unwrap_err();
        assert!(error.contains("cap_reached"), "the block reason must be surfaced: {error}");
        assert_eq!(llm_calls_today(&policy_conn), 0);
    }
}
