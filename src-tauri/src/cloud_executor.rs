// src-tauri/src/cloud_executor.rs
//! Cloud STT/LLM dispatch. Mirrors tts_executor.rs's HTTP-call conventions:
//! bounded timeout, truncated+redacted errors, never logs the request (which
//! is where the key lives).

use crate::cloud_config::CloudProviderConfig;
use crate::fungwire::Segment;
use std::path::Path;
use std::time::Duration;
use url::Url;

const STT_TIMEOUT: Duration = Duration::from_secs(120);
const LLM_TIMEOUT: Duration = Duration::from_secs(60);

/// Fallback OpenAI STT model, used when the user's `CloudProviderConfig` has
/// no `model` override configured.
const DEFAULT_OPENAI_STT_MODEL: &str = "whisper-1";
/// Fallback Anthropic chat model, used when the user's `CloudProviderConfig`
/// has no `model` override configured.
const DEFAULT_ANTHROPIC_MODEL: &str = "claude-3-5-sonnet-20241022";
/// Fallback OpenAI chat model, used when the user's `CloudProviderConfig` has
/// no `model` override configured.
const DEFAULT_OPENAI_LLM_MODEL: &str = "gpt-4o-mini";

/// Stable prefix for an LLM cloud-admission failure. `job_engine::classify`
/// checks this before the local transport markers because admission failures
/// must not retry the graph build or reserve the same cloud slot again.
pub(crate) const CLOUD_ADMISSION_NON_RETRYABLE_PREFIX: &str = "cloud_admission_non_retryable:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveLlmExecution {
    pub(crate) text: String,
    pub(crate) provider_id: String,
    pub(crate) provider_label: String,
    pub(crate) model_name: String,
    pub(crate) endpoint: String,
    pub(crate) runtime_location: &'static str,
}

fn redact_case_insensitive(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut output = text.to_string();
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(start) = lower.find(&needle.to_ascii_lowercase()) else {
            return output;
        };
        let end = start + needle.len();
        output.replace_range(start..end, replacement);
    }
}

fn redact_sensitive_text(text: &str, secrets: &[&str]) -> String {
    let mut output = text.to_string();
    for secret in secrets {
        if !secret.trim().is_empty() {
            output = redact_case_insensitive(&output, secret, "<redacted>");
        }
    }
    let output = redact_sensitive_header_values(&output);
    redact_bearer_value(&output)
}

fn truncated_redacted(body: &str, secrets: &[&str]) -> String {
    let redacted = redact_sensitive_text(body, secrets);
    truncated(&redacted).to_string()
}

const SENSITIVE_HEADER_NAMES: &[&str] = &[
    "proxy-authorization",
    "authorization",
    "x-api-key",
    "x-api_key",
    "xapikey",
    "api-key",
    "api_key",
    "apikey",
    "authentication",
    "x-auth-token",
    "auth-token",
    "access-token",
    "refresh-token",
    "id-token",
    "set-cookie",
    "cookie",
    "client-secret",
    "client_secret",
    "proxy-auth",
    "token",
    "session",
    "secret",
    "password",
];

fn starts_with_ascii_case_insensitive_at(text: &str, start: usize, needle: &str) -> bool {
    let bytes = text.as_bytes();
    let needle = needle.as_bytes();
    start + needle.len() <= bytes.len()
        && bytes[start..start + needle.len()]
            .iter()
            .zip(needle)
            .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
}

fn is_header_name_boundary(text: &str, start: usize) -> bool {
    start == 0 || !text.as_bytes()[start - 1].is_ascii_alphanumeric()
}

fn sensitive_header_value_end(text: &str, start: usize, name: &str, quoted: bool) -> usize {
    // Cookie values contain semicolon-separated name/value pairs, so redact
    // the entire line rather than risking a later cookie fragment escaping.
    // Do the same for quoted/JSON-shaped values: stopping at punctuation
    // inside a quoted value could leave a secret suffix behind.
    if quoted || matches!(name, "cookie" | "set-cookie") {
        return text[start..]
            .find(['\r', '\n'])
            .map(|offset| start + offset)
            .unwrap_or(text.len());
    }
    text[start..]
        .find([';', ',', '\r', '\n'])
        .map(|offset| start + offset)
        .unwrap_or(text.len())
}

/// Removes values attached to sensitive header-like fields, including values
/// the caller did not configure as its own API key. The prefix before the
/// field (which contains the provider/status classification) is retained.
fn redact_sensitive_header_values(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut copied_until = 0;
    let mut index = 0;
    while index < text.len() {
        if is_header_name_boundary(text, index) {
            let mut match_end = None;
            for name in SENSITIVE_HEADER_NAMES {
                if !starts_with_ascii_case_insensitive_at(text, index, name) {
                    continue;
                }
                let mut separator = index + name.len();
                while text
                    .as_bytes()
                    .get(separator)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    separator += 1;
                }
                let mut quoted = false;
                if matches!(text.as_bytes().get(separator), Some(b'"' | b'\'')) {
                    quoted = true;
                    separator += 1;
                    while text
                        .as_bytes()
                        .get(separator)
                        .is_some_and(u8::is_ascii_whitespace)
                    {
                        separator += 1;
                    }
                }
                if !matches!(text.as_bytes().get(separator), Some(b':' | b'=')) {
                    continue;
                }
                let mut value_start = separator + 1;
                while text
                    .as_bytes()
                    .get(value_start)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    value_start += 1;
                }
                if matches!(text.as_bytes().get(value_start), Some(b'"' | b'\'')) {
                    quoted = true;
                    value_start += 1;
                }
                match_end = Some(sensitive_header_value_end(text, value_start, name, quoted));
                break;
            }
            if let Some(end) = match_end {
                output.push_str(&text[copied_until..index]);
                output.push_str("<redacted-header>");
                copied_until = end;
                index = end;
                continue;
            }
        }
        index += text[index..].chars().next().unwrap().len_utf8();
    }
    output.push_str(&text[copied_until..]);
    output
}

fn redact_bearer_value(text: &str) -> String {
    const BEARER: &str = "bearer";
    let mut output = String::with_capacity(text.len());
    let mut copied_until = 0;
    let mut index = 0;
    while index < text.len() {
        if is_header_name_boundary(text, index)
            && starts_with_ascii_case_insensitive_at(text, index, BEARER)
        {
            let mut value_start = index + BEARER.len();
            while text
                .as_bytes()
                .get(value_start)
                .is_some_and(u8::is_ascii_whitespace)
            {
                value_start += 1;
            }
            if value_start > index + BEARER.len() && value_start < text.len() {
                let end = text[value_start..]
                    .find([' ', '\t', ';', ',', '\r', '\n', '"', '\'', '}', ']'])
                    .map(|offset| value_start + offset)
                    .unwrap_or(text.len());
                output.push_str(&text[copied_until..index]);
                output.push_str("<redacted-bearer>");
                copied_until = end;
                index = end;
                continue;
            }
        }
        index += text[index..].chars().next().unwrap().len_utf8();
    }
    output.push_str(&text[copied_until..]);
    output
}

/// Returns the configured custom endpoint without userinfo, query, or
/// fragment. Production cloud configuration is HTTPS-only; unit tests may
/// use a loopback HTTP listener as their fake provider.
fn sanitize_custom_endpoint(endpoint: &str) -> Result<String, String> {
    let mut parsed =
        Url::parse(endpoint.trim()).map_err(|_| "custom cloud endpoint is invalid".to_string())?;
    let loopback_http = cfg!(test)
        && parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"));
    if parsed.scheme() != "https" && !loopback_http {
        return Err("custom cloud endpoint must use https".into());
    }
    if parsed.host_str().is_none() {
        return Err("custom cloud endpoint is invalid".into());
    }
    parsed
        .set_username("")
        .map_err(|_| "custom cloud endpoint is invalid".to_string())?;
    parsed.set_password(None).ok();
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn effective_cloud_provenance(
    config: &CloudProviderConfig,
) -> Result<(String, String, String, String), String> {
    match config {
        CloudProviderConfig::Anthropic { model, .. } => Ok((
            "cloud-anthropic-summary-intent".into(),
            "Anthropic".into(),
            model
                .as_deref()
                .unwrap_or(DEFAULT_ANTHROPIC_MODEL)
                .to_string(),
            "https://api.anthropic.com/v1/messages".into(),
        )),
        CloudProviderConfig::OpenAi { model, .. } => Ok((
            "cloud-openai-summary-intent".into(),
            "OpenAI".into(),
            model
                .as_deref()
                .unwrap_or(DEFAULT_OPENAI_LLM_MODEL)
                .to_string(),
            "https://api.openai.com/v1/chat/completions".into(),
        )),
        CloudProviderConfig::Custom { endpoint, .. } => Ok((
            "cloud-custom-summary-intent".into(),
            "Custom".into(),
            "custom-unspecified".into(),
            sanitize_custom_endpoint(endpoint)?,
        )),
    }
}

fn truncated(body: &str) -> &str {
    if body.len() <= 500 {
        return body;
    }
    let end = body
        .char_indices()
        .take_while(|(i, _)| *i < 500)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
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

pub(crate) fn dispatch_stt(
    config: &CloudProviderConfig,
    audio_path: &Path,
) -> Result<Vec<Segment>, String> {
    if !config.has_configured_api_key() {
        return Err("cloud provider API key is not configured".into());
    }
    match config {
        CloudProviderConfig::OpenAi { api_key, model } => {
            openai_stt(api_key, model.as_deref(), audio_path)
        }
        CloudProviderConfig::Custom {
            endpoint, api_key, ..
        } => custom_stt(endpoint, api_key, audio_path),
        CloudProviderConfig::Anthropic { .. } => Err("Anthropic ไม่มีบริการ STT".into()),
    }
}

pub(crate) fn dispatch_llm(config: &CloudProviderConfig, prompt: &str) -> Result<String, String> {
    if !config.has_configured_api_key() {
        return Err("cloud provider API key is not configured".into());
    }
    match config {
        CloudProviderConfig::Anthropic { api_key, model } => {
            anthropic_llm(api_key, model.as_deref(), prompt)
        }
        CloudProviderConfig::OpenAi { api_key, model } => {
            openai_llm(api_key, model.as_deref(), prompt)
        }
        CloudProviderConfig::Custom {
            endpoint, api_key, ..
        } => custom_llm(endpoint, api_key, prompt),
    }
}

fn openai_stt(
    api_key: &str,
    model: Option<&str>,
    audio_path: &Path,
) -> Result<Vec<Segment>, String> {
    #[derive(serde::Deserialize)]
    struct OpenAiSttSegment {
        start: f64,
        end: f64,
        text: String,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiSttResponse {
        segments: Vec<OpenAiSttSegment>,
    }

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
        .text(
            "model",
            model.unwrap_or(DEFAULT_OPENAI_STT_MODEL).to_string(),
        )
        .text("response_format", "verbose_json");

    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| redact_sensitive_text(&format!("สร้าง HTTP client ไม่ได้: {e}"), &[api_key]))?;
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "OpenAI STT ไม่ตอบสนองภายใน 120 วินาที".to_string()
            } else {
                redact_sensitive_text(&format!("เชื่อมต่อ OpenAI STT ไม่ได้: {e}"), &[api_key])
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "OpenAI STT ตอบ {status}: {}",
            truncated_redacted(&body, &[api_key])
        ));
    }

    let parsed: OpenAiSttResponse = response.json().map_err(|e| {
        redact_sensitive_text(&format!("อ่าน response OpenAI STT ไม่ได้: {e}"), &[api_key])
    })?;
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
    let endpoint = sanitize_custom_endpoint(endpoint)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(STT_TIMEOUT)
        .build()
        .map_err(|e| redact_sensitive_text(&format!("สร้าง HTTP client ไม่ได้: {e}"), &[api_key]))?;
    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", mime_for_audio_path(audio_path))
        .body(bytes)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "custom STT endpoint ไม่ตอบสนองภายใน 120 วินาที".to_string()
            } else {
                redact_sensitive_text(&format!("เชื่อมต่อ custom STT endpoint ไม่ได้: {e}"), &[api_key])
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "custom STT endpoint ตอบ {status}: {}",
            truncated_redacted(&body, &[api_key])
        ));
    }
    response.json::<Vec<Segment>>().map_err(|e| {
        redact_sensitive_text(&format!("อ่าน response custom STT ไม่ได้: {e}"), &[api_key])
    })
}

fn anthropic_llm(api_key: &str, model: Option<&str>, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct ContentBlock {
        text: String,
    }
    #[derive(serde::Deserialize)]
    struct MessagesResponse {
        content: Vec<ContentBlock>,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| redact_sensitive_text(&format!("สร้าง HTTP client ไม่ได้: {e}"), &[api_key]))?;
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": model.unwrap_or(DEFAULT_ANTHROPIC_MODEL),
            "max_tokens": 2048,
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!("Anthropic ไม่ตอบสนองภายใน {} วินาที", LLM_TIMEOUT.as_secs())
            } else {
                redact_sensitive_text(&format!("เชื่อมต่อ Anthropic ไม่ได้: {e}"), &[api_key])
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "Anthropic ตอบ {status}: {}",
            truncated_redacted(&body, &[api_key])
        ));
    }
    let parsed: MessagesResponse = response.json().map_err(|e| {
        redact_sensitive_text(&format!("อ่าน response Anthropic ไม่ได้: {e}"), &[api_key])
    })?;
    parsed
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .ok_or_else(|| "Anthropic ตอบกลับไม่มีเนื้อหา".to_string())
}

fn openai_llm(api_key: &str, model: Option<&str>, prompt: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }
    #[derive(serde::Deserialize)]
    struct ChoiceMessage {
        content: String,
    }
    #[derive(serde::Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| redact_sensitive_text(&format!("สร้าง HTTP client ไม่ได้: {e}"), &[api_key]))?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "model": model.unwrap_or(DEFAULT_OPENAI_LLM_MODEL),
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!("OpenAI ไม่ตอบสนองภายใน {} วินาที", LLM_TIMEOUT.as_secs())
            } else {
                redact_sensitive_text(&format!("เชื่อมต่อ OpenAI ไม่ได้: {e}"), &[api_key])
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "OpenAI ตอบ {status}: {}",
            truncated_redacted(&body, &[api_key])
        ));
    }
    let parsed: ChatResponse = response.json().map_err(|e| {
        redact_sensitive_text(&format!("อ่าน response OpenAI ไม่ได้: {e}"), &[api_key])
    })?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "OpenAI ตอบกลับไม่มีเนื้อหา".to_string())
}

fn custom_llm(endpoint: &str, api_key: &str, prompt: &str) -> Result<String, String> {
    // Same {endpoint}/api/chat Ollama-shaped contract graph_build.rs::call_llm
    // already speaks — a "custom" LLM endpoint needs no new wire format.
    #[derive(serde::Deserialize)]
    struct ChatMessage {
        content: String,
    }
    #[derive(serde::Deserialize)]
    struct ChatResponse {
        message: ChatMessage,
    }

    let endpoint = sanitize_custom_endpoint(endpoint)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(LLM_TIMEOUT)
        .build()
        .map_err(|e| redact_sensitive_text(&format!("สร้าง HTTP client ไม่ได้: {e}"), &[api_key]))?;
    let response = client
        .post(format!("{endpoint}/api/chat"))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
        }))
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "custom LLM endpoint ไม่ตอบสนองภายใน {} วินาที",
                    LLM_TIMEOUT.as_secs()
                )
            } else {
                redact_sensitive_text(&format!("เชื่อมต่อ custom LLM endpoint ไม่ได้: {e}"), &[api_key])
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "custom LLM endpoint ตอบ {status}: {}",
            truncated_redacted(&body, &[api_key])
        ));
    }
    response
        .json::<ChatResponse>()
        .map(|r| r.message.content)
        .map_err(|e| {
            redact_sensitive_text(&format!("อ่าน response custom LLM ไม่ได้: {e}"), &[api_key])
        })
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
/// not running / not reachable. Two other local failures must NOT be masked by
/// a silent cloud retry, and both surface unchanged instead:
/// - a malformed response (bad status, unparseable body) — a real bug,
///   matching `graph_build::call_llm`'s pre-existing behavior;
/// - a **timeout** ("LLM endpoint timed out at …") — the endpoint answered the
///   connection and was simply slow, so it is reachable by definition.
///   Sending that user's transcript to a cloud provider because their own
///   machine was busy would break the local-first promise, which is exactly
///   why `call_llm` words the two `send()` failures differently.
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
    #[cfg(test)]
    {
        if let Some(config) = TEST_LLM_CLOUD_CONFIG
            .lock()
            .expect("test cloud config mutex poisoned")
            .clone()
        {
            return config.has_configured_api_key().then_some(config);
        }
    }

    use crate::cloud_config::{cloud_config_slot, load_cloud_config, CloudTaskKind};
    for provider in ["anthropic", "openai", "custom"] {
        let slot = cloud_config_slot(provider, CloudTaskKind::Llm);
        if let Ok(Some(config)) = load_cloud_config(&slot) {
            return Some(config);
        }
    }
    None
}

#[cfg(test)]
static TEST_LLM_CLOUD_CONFIG: std::sync::Mutex<Option<CloudProviderConfig>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_test_llm_cloud_config(config: Option<CloudProviderConfig>) {
    *TEST_LLM_CLOUD_CONFIG
        .lock()
        .expect("test cloud config mutex poisoned") = config;
}

#[cfg(test)]
pub(crate) fn set_test_custom_llm_provider(endpoint: String, api_key: String) {
    set_test_llm_cloud_config(Some(CloudProviderConfig::Custom {
        endpoint,
        api_key,
        task_kind: crate::cloud_config::CloudTaskKind::Llm,
    }));
}

/// `runtime_location` value recorded on the `model_runs` audit row when the
/// extraction came from the user's own machine.
pub(crate) const RUNTIME_LOCAL: &str = "local";
/// `runtime_location` value recorded when the cloud fallback actually ran.
pub(crate) const RUNTIME_CLOUD: &str = "cloud";

fn cloud_admission_failure(code: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) => format!(
            "{CLOUD_ADMISSION_NON_RETRYABLE_PREFIX}{code}: Ollama fallback not admitted ({detail})"
        ),
        None => {
            format!("{CLOUD_ADMISSION_NON_RETRYABLE_PREFIX}{code}: Ollama fallback not admitted")
        }
    }
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
/// `policy_conn` is used only after the local connection failure has been
/// classified. The reservation commits before `dispatch_llm` can contact a
/// provider.
pub(crate) fn call_llm_with_fallback(
    local_call: impl FnOnce() -> Result<String, String>,
    local_endpoint: &str,
    local_model: &str,
    prompt: &str,
    cloud: Option<&CloudProviderConfig>,
    policy_conn: &rusqlite::Connection,
) -> Result<EffectiveLlmExecution, String> {
    match local_call() {
        Ok(text) => Ok(EffectiveLlmExecution {
            text,
            provider_id: "ollama-summary-intent".into(),
            provider_label: "Ollama / llama.cpp".into(),
            model_name: local_model.to_string(),
            endpoint: local_endpoint.to_string(),
            runtime_location: RUNTIME_LOCAL,
        }),
        Err(e) if is_connection_error(&e) => {
            let Some(config) = cloud else {
                return match crate::policy::reserve_cloud_call(
                    policy_conn,
                    crate::cloud_config::CloudTaskKind::Llm,
                    false,
                ) {
                    Err(crate::policy::CloudAdmissionError::Blocked { reason }) => {
                        Err(cloud_admission_failure(reason, None))
                    }
                    Err(crate::policy::CloudAdmissionError::Persistence(message)) => {
                        Err(cloud_admission_failure("policy_error", Some(&message)))
                    }
                    Ok(()) => Err(cloud_admission_failure("no_key_configured", None)),
                };
            };
            let (provider_id, provider_label, model_name, endpoint) =
                effective_cloud_provenance(config)?;
            match crate::policy::reserve_cloud_call(
                policy_conn,
                crate::cloud_config::CloudTaskKind::Llm,
                true,
            ) {
                Ok(()) => {
                    let text = dispatch_llm(config, prompt)?;
                    Ok(EffectiveLlmExecution {
                        text,
                        provider_id,
                        provider_label,
                        model_name,
                        endpoint,
                        runtime_location: RUNTIME_CLOUD,
                    })
                }
                Err(crate::policy::CloudAdmissionError::Blocked { reason }) => {
                    Err(cloud_admission_failure(reason, None))
                }
                Err(crate::policy::CloudAdmissionError::Persistence(message)) => {
                    Err(cloud_admission_failure("policy_error", Some(&message)))
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
        assert_eq!(
            mime_for_audio_path(Path::new("/tmp/concat.wav")),
            "audio/wav"
        );
        // Extension casing comes from whoever named the file, not from us.
        assert_eq!(
            mime_for_audio_path(Path::new("/tmp/CONCAT.WAV")),
            "audio/wav"
        );
    }

    /// The single-segment FUNGWIRE cloud job uploads the phone's raw
    /// `segment-0.m4a` with no concat step, so this is the case the old
    /// hard-coded `audio/wav` mislabelled.
    #[test]
    fn mime_for_audio_path_labels_m4a_as_audio_m4a() {
        assert_eq!(
            mime_for_audio_path(Path::new("/tmp/segment-0.m4a")),
            "audio/m4a"
        );
    }

    #[test]
    fn mime_for_audio_path_falls_back_to_wav_for_unknown_or_missing_extension() {
        assert_eq!(
            mime_for_audio_path(Path::new("/tmp/audio.ogg")),
            "audio/wav"
        );
        assert_eq!(mime_for_audio_path(Path::new("/tmp/audio")), "audio/wav");
    }

    #[test]
    fn custom_stt_parses_segment_array() {
        let addr = one_shot_server(
            "HTTP/1.1 200 OK",
            r#"[{"start_ms":0,"end_ms":1200,"text":"hello","confidence":0.9}]"#,
        );
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
        let config = CloudProviderConfig::Anthropic {
            api_key: "sk-ant-test".into(),
            model: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("test.wav");
        std::fs::write(&audio_path, b"x").unwrap();
        let result = dispatch_stt(&config, &audio_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("STT"));
    }

    #[test]
    fn empty_llm_api_key_is_rejected_before_dispatch() {
        let config = CloudProviderConfig::Custom {
            endpoint: "http://127.0.0.1:1".into(),
            api_key: " \t".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let error = dispatch_llm(&config, "prompt").unwrap_err();
        assert_eq!(error, "cloud provider API key is not configured");
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
    const LOCAL_UNREACHABLE: &str =
        "LLM endpoint unreachable at http://127.0.0.1:11434: connection refused";
    /// The message `call_llm` produces when Ollama answers, badly.
    const LOCAL_BAD_STATUS: &str = "LLM endpoint returned 500 Internal Server Error";
    /// The message `call_llm` produces when Ollama is listening but too slow.
    /// Reachable — so this must NOT be treated as a connection failure.
    const LOCAL_TIMEOUT: &str =
        "LLM endpoint timed out at http://127.0.0.1:11434: operation timed out";

    fn llm_cloud_policy(enabled: bool, daily_cap: u32) -> crate::policy::TierPolicy {
        crate::policy::TierPolicy {
            stt_cloud_enabled: false,
            llm_cloud_enabled: enabled,
            daily_cap,
        }
    }

    fn configure_llm_policy(conn: &rusqlite::Connection, enabled: bool, daily_cap: u32) {
        crate::policy::save_policy(conn, &llm_cloud_policy(enabled, daily_cap)).unwrap();
    }

    fn llm_calls_today(conn: &rusqlite::Connection) -> u32 {
        crate::policy::calls_today(conn, crate::cloud_config::CloudTaskKind::Llm).unwrap()
    }

    /// A stub cloud LLM endpoint speaking the Ollama-shaped custom contract.
    fn stub_cloud_provider() -> CloudProviderConfig {
        let addr = one_shot_server(
            "HTTP/1.1 200 OK",
            r#"{"message":{"content":"cloud extraction result"}}"#,
        );
        CloudProviderConfig::Custom {
            endpoint: format!("http://{addr}"),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        }
    }

    /// `is_connection_error` is a substring match, so the three messages
    /// `call_llm` can produce must be classified deliberately, not by
    /// accident. The timeout case is the one that matters: "timed out" and
    /// "unreachable" are different conditions and only the latter may reach
    /// the cloud.
    #[test]
    fn only_a_genuine_connection_failure_is_classified_as_one() {
        assert!(is_connection_error(LOCAL_UNREACHABLE));
        assert!(
            !is_connection_error(LOCAL_TIMEOUT),
            "a reachable-but-slow endpoint is not an unreachable one",
        );
        assert!(!is_connection_error(LOCAL_BAD_STATUS));
    }

    /// The behavioural half of the same fix: a timed-out local call must fail
    /// the build with its own error, never silently ship the prompt to a cloud
    /// provider — even with cloud enabled, configured, and a working stub
    /// endpoint standing by.
    #[test]
    fn a_local_timeout_is_never_retried_in_the_cloud() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_TIMEOUT.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        assert_eq!(
            result.unwrap_err(),
            LOCAL_TIMEOUT,
            "a slow local Ollama must surface as a timeout, not as a cloud result",
        );
        assert_eq!(
            llm_calls_today(&policy_conn),
            0,
            "no cloud dispatch happened, so the counter must not move",
        );
    }

    #[test]
    fn ollama_connection_failure_falls_back_to_cloud_when_enabled_and_configured() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, true, 20);

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        let execution = result.unwrap();
        assert_eq!(execution.text, "cloud extraction result");
        assert_eq!(execution.provider_id, "cloud-custom-summary-intent");
        assert_eq!(execution.model_name, "custom-unspecified");
        assert_eq!(execution.runtime_location, RUNTIME_CLOUD);
        assert!(execution.endpoint.starts_with("http://127.0.0.1:"));
        assert_eq!(
            llm_calls_today(&policy_conn),
            1,
            "a successful cloud dispatch must consume one of the day's budgeted calls",
        );
    }

    #[test]
    fn ollama_connection_failure_with_cloud_disabled_returns_non_retryable_admission_error() {
        let cloud_config = CloudProviderConfig::Custom {
            endpoint: "http://127.0.0.1:9".into(),
            api_key: "k".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, false, 20);

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        let error = result.unwrap_err();
        assert!(
            error.contains("Ollama"),
            "the blocked-fallback error must name Ollama: {error}"
        );
        assert!(
            error.contains("cloud_disabled"),
            "the block reason must be surfaced: {error}"
        );
        assert!(error.starts_with(CLOUD_ADMISSION_NON_RETRYABLE_PREFIX));
        assert!(
            !error.contains("LLM endpoint unreachable"),
            "admission failure must not retain the local retry marker: {error}"
        );
        let failure = crate::job_engine::classify(&error);
        assert_eq!(failure.code, "cloud_admission_blocked");
        assert!(!failure.retryable);
        assert!(matches!(
            crate::job_engine::next_step(Err(failure), 1, false),
            crate::job_engine::NextStep::Fail(_)
        ));
        assert!(
            crate::job_engine::classify(LOCAL_UNREACHABLE).retryable,
            "the genuine local transport failure must remain retryable"
        );
        assert_eq!(
            llm_calls_today(&policy_conn),
            0,
            "a blocked fallback dispatches nothing, so it must not consume a call",
        );
    }

    /// Cloud enabled but no provider ever configured must surface as a
    /// `no_key_configured` block, mirroring how
    /// `fungwire_server::dispatch_cloud_stt` reports the same situation on
    /// the STT side, instead of silently dropping back to the original local
    /// error with no hint that cloud fallback was skipped and why.
    #[test]
    fn ollama_connection_failure_without_a_configured_provider_surfaces_no_key_configured() {
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, true, 20);

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            None,
            &policy_conn,
        );

        let error = result.unwrap_err();
        assert!(
            error.contains("Ollama"),
            "the blocked-fallback error must name Ollama: {error}"
        );
        assert!(
            error.contains("no_key_configured"),
            "the block reason must be surfaced: {error}",
        );
        assert!(error.starts_with(CLOUD_ADMISSION_NON_RETRYABLE_PREFIX));
        assert!(!error.contains("LLM endpoint unreachable"));
        assert_eq!(llm_calls_today(&policy_conn), 0);
    }

    #[test]
    fn a_non_connection_ollama_error_is_not_masked_by_the_cloud_fallback() {
        // A local Ollama that IS reachable and answers badly. That is a real
        // bug, not an "Ollama isn't running" condition, so it must surface
        // unchanged instead of being silently retried in the cloud.
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, true, 20);

        let result = call_llm_with_fallback(
            || Err(LOCAL_BAD_STATUS.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        assert_eq!(
            result.unwrap_err(),
            LOCAL_BAD_STATUS,
            "a bad local response must surface as-is, not as a cloud result",
        );
        assert_eq!(
            llm_calls_today(&policy_conn),
            0,
            "no cloud dispatch happened, so the counter must not move",
        );
    }

    #[test]
    fn a_successful_local_call_never_reaches_the_cloud_or_the_counter() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();

        let result = call_llm_with_fallback(
            || Ok("local extraction result".to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        let execution = result.unwrap();
        assert_eq!(execution.text, "local extraction result");
        assert_eq!(execution.provider_id, "ollama-summary-intent");
        assert_eq!(execution.model_name, "llama3.1:8b");
        assert_eq!(execution.endpoint, "http://127.0.0.1:11434");
        assert_eq!(execution.runtime_location, RUNTIME_LOCAL);
        assert_eq!(llm_calls_today(&policy_conn), 0);
    }

    #[test]
    fn a_failed_cloud_dispatch_retains_its_reserved_call() {
        // Cloud is allowed and configured, but the provider is unreachable.
        // Admission is committed before egress, so the failed provider round
        // trip retains its reservation.
        let cloud_config = CloudProviderConfig::Custom {
            // Nothing listens on TCP port 1 — deterministic, immediate refusal.
            endpoint: "http://127.0.0.1:1".into(),
            api_key: "test-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, true, 20);

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        assert!(result.is_err());
        assert_eq!(
            llm_calls_today(&policy_conn),
            1,
            "a committed reservation must remain charged when the provider fails",
        );
    }

    #[test]
    fn a_reached_daily_cap_blocks_the_fallback() {
        let cloud_config = stub_cloud_provider();
        let policy_conn = rusqlite::Connection::open_in_memory().unwrap();
        configure_llm_policy(&policy_conn, true, 1);
        crate::policy::reserve_cloud_call(
            &policy_conn,
            crate::cloud_config::CloudTaskKind::Llm,
            true,
        )
        .unwrap();

        let result = call_llm_with_fallback(
            || Err(LOCAL_UNREACHABLE.to_string()),
            "http://127.0.0.1:11434",
            "llama3.1:8b",
            "prompt",
            Some(&cloud_config),
            &policy_conn,
        );

        let error = result.unwrap_err();
        assert!(
            error.contains("cap_reached"),
            "the block reason must be surfaced: {error}"
        );
        assert!(error.starts_with(CLOUD_ADMISSION_NON_RETRYABLE_PREFIX));
        assert!(!error.contains("LLM endpoint unreachable"));
        assert_eq!(llm_calls_today(&policy_conn), 1);
    }

    #[test]
    fn cloud_errors_redact_api_keys_and_sensitive_headers() {
        let key = "sentinel-api-key-should-never-escape";
        let arbitrary_authorization = "another-authorization-secret";
        let arbitrary_api_key = "another-api-key-secret";
        let arbitrary_cookie = "session=another-cookie-secret";
        let arbitrary_quoted_api_key = "another-quoted-api-key-secret";
        let body: &'static str = Box::leak(
            format!(
                "error=unauthorized; authorization: Bearer {arbitrary_authorization}; \
                 x-api-key: {arbitrary_api_key}; api-key={key}; cookie: {arbitrary_cookie}\n\
                 \"x-api-key\": \"{arbitrary_quoted_api_key}\""
            )
            .into_boxed_str(),
        );
        let addr = one_shot_server("HTTP/1.1 401 Unauthorized", body);
        let result = custom_llm(&format!("http://{addr}"), key, "prompt");
        let error = result.unwrap_err();

        for secret in [
            key,
            arbitrary_authorization,
            arbitrary_api_key,
            arbitrary_cookie,
            arbitrary_quoted_api_key,
        ] {
            assert!(
                !error.contains(secret),
                "sensitive value leaked in error: {error}"
            );
        }
        assert!(
            error.contains("401"),
            "status classification was lost: {error}"
        );
        assert!(
            error.contains("error=unauthorized"),
            "error detail was lost: {error}"
        );
        assert!(!error.to_ascii_lowercase().contains("authorization"));
        assert!(!error.to_ascii_lowercase().contains("x-api-key"));
        assert!(!error.to_ascii_lowercase().contains("api-key"));
        assert!(!error.to_ascii_lowercase().contains("cookie"));
    }

    #[test]
    fn custom_provenance_strips_userinfo_query_and_fragment() {
        let config = CloudProviderConfig::Custom {
            endpoint: "http://user:password@127.0.0.1:8080/private?api_key=sentinel#fragment"
                .into(),
            api_key: "sentinel-api-key".into(),
            task_kind: crate::cloud_config::CloudTaskKind::Llm,
        };
        let (_, _, model, endpoint) = effective_cloud_provenance(&config).unwrap();
        assert_eq!(model, "custom-unspecified");
        assert_eq!(endpoint, "http://127.0.0.1:8080/private");
        assert!(!endpoint.contains("sentinel"));
        assert!(!endpoint.contains("password"));
    }

    #[test]
    fn built_in_cloud_provenance_uses_provider_specific_defaults_and_overrides() {
        let anthropic = CloudProviderConfig::Anthropic {
            api_key: "sentinel-anthropic-key".into(),
            model: None,
        };
        assert_eq!(
            effective_cloud_provenance(&anthropic).unwrap(),
            (
                "cloud-anthropic-summary-intent".into(),
                "Anthropic".into(),
                DEFAULT_ANTHROPIC_MODEL.into(),
                "https://api.anthropic.com/v1/messages".into(),
            )
        );

        let openai = CloudProviderConfig::OpenAi {
            api_key: "sentinel-openai-key".into(),
            model: Some("gpt-test-model".into()),
        };
        assert_eq!(
            effective_cloud_provenance(&openai).unwrap(),
            (
                "cloud-openai-summary-intent".into(),
                "OpenAI".into(),
                "gpt-test-model".into(),
                "https://api.openai.com/v1/chat/completions".into(),
            )
        );
    }
}
