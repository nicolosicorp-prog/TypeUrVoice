use anyhow::{Context, Result};
use reqwest::multipart;
use serde::Deserialize;
use crate::storage::dictionary;
use crate::state::CloudTranscriptionProvider;

#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
}

/// Per-provider transcription endpoint + default model id. Both speak the
/// same multipart /audio/transcriptions OpenAI shape, so we just swap URL +
/// model + key.
///
/// Defaults (Oct 2026): OpenAI `gpt-transcribe` replaced whisper-1 as the
/// recommended file-transcription model (lower error rate, cheaper per
/// minute); Groq `whisper-large-v3-turbo` is the fast multilingual option.
fn whisper_endpoint(provider: &CloudTranscriptionProvider) -> (&'static str, &'static str, &'static str) {
    match provider {
        CloudTranscriptionProvider::OpenAi => (
            "https://api.openai.com/v1/audio/transcriptions",
            "gpt-transcribe",
            "OpenAI",
        ),
        CloudTranscriptionProvider::Groq => (
            "https://api.groq.com/openai/v1/audio/transcriptions",
            "whisper-large-v3-turbo",
            "Groq",
        ),
    }
}

/// Selectable transcription models per provider, default first.
pub fn transcription_models(provider: &CloudTranscriptionProvider) -> &'static [&'static str] {
    match provider {
        CloudTranscriptionProvider::OpenAi => &["gpt-transcribe", "gpt-4o-mini-transcribe", "whisper-1"],
        CloudTranscriptionProvider::Groq   => &["whisper-large-v3-turbo", "whisper-large-v3"],
    }
}

/// Which key slot to read from `keys.json` for cloud Whisper transcription.
pub fn key_slot_for(provider: &CloudTranscriptionProvider) -> &'static str {
    match provider {
        CloudTranscriptionProvider::OpenAi => "openai",
        CloudTranscriptionProvider::Groq   => "compat:groq",
    }
}

pub async fn transcribe(
    audio_wav: Vec<u8>,
    api_key: &str,
    source_language: &str,
    dict_words: &[String],
    provider: &CloudTranscriptionProvider,
    model_override: Option<&str>,
    http: &reqwest::Client,
) -> anyhow::Result<super::TranscriptionResult> {
    let (url, default_model, vendor) = whisper_endpoint(provider);
    // Only honour an override that belongs to this provider — a stale value
    // left over from switching OpenAI ↔ Groq would otherwise 400.
    let model = model_override
        .filter(|m| transcription_models(provider).contains(m))
        .unwrap_or(default_model);

    let file_part = multipart::Part::bytes(audio_wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("response_format", "json");

    if source_language != "auto" && !source_language.is_empty() {
        form = form.text("language", source_language.to_string());
    }

    if let Some(prompt) = dictionary::format_for_whisper(dict_words) {
        form = form.text("prompt", prompt);
    }

    let response = http
        .post(url)
        .bearer_auth(api_key)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(60))  // per-call budget
        .send()
        .await
        .with_context(|| format!("Failed to reach {} Whisper API", vendor))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{} API error {}: {}",
            vendor,
            status.as_u16(),
            body
        ));
    }

    let result: WhisperResponse = response.json().await.context("Failed to parse Whisper response")?;

    Ok(super::TranscriptionResult {
        text: result.text.trim().to_string(),
        language: result.language.unwrap_or_else(|| "en".to_string()),
    })
}

/// Probe the lightest available endpoint for the given key slot. Returns true
/// on HTTP 2xx, false otherwise. `slot` matches `keys.json` slot names.
///
/// Per-provider probes:
///   - openai / compat:* → GET /models with Bearer key (OpenAI standard)
///   - anthropic         → tiny POST /v1/messages (Anthropic has no GET /models we can rely on)
///   - gemini            → GET /v1beta/models with Bearer key
pub async fn test_provider_key(slot: &str, api_key: &str, http: &reqwest::Client) -> Result<bool> {
    use std::time::Duration;
    const TIMEOUT: Duration = Duration::from_secs(10);

    if slot == "anthropic" {
        // Smallest possible probe — 1 token of output.
        let body = serde_json::json!({
            "model": "claude-haiku-5-5",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}]
        });
        let response = http.post("https://api.anthropic.com/v1/messages")
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", api_key)
            .timeout(TIMEOUT)
            .json(&body)
            .send().await?;
        // 401/403 ⇒ bad key. 200 / 400 (validation) / 429 (rate limit) ⇒ key is valid.
        return Ok(response.status().is_success() || response.status().as_u16() == 429);
    }

    if slot == "gemini" {
        let response = http.get("https://generativelanguage.googleapis.com/v1beta/models")
            .bearer_auth(api_key)
            .timeout(TIMEOUT)
            .send().await?;
        return Ok(response.status().is_success());
    }

    // OpenAI + every OpenAI-compatible provider.
    let url = match slot {
        "openai"            => "https://api.openai.com/v1/models",
        "compat:grok"       => "https://api.x.ai/v1/models",
        "compat:groq"       => "https://api.groq.com/openai/v1/models",
        "compat:deepseek"   => "https://api.deepseek.com/v1/models",
        "compat:mistral"    => "https://api.mistral.ai/v1/models",
        "compat:openrouter" => "https://openrouter.ai/api/v1/models",
        "compat:perplexity" => "https://api.perplexity.ai/models",
        "compat:together"   => "https://api.together.xyz/v1/models",
        // Custom endpoint can't be probed without knowing the URL — caller
        // should test by attempting an actual completion.
        _ => return Ok(false),
    };

    let response = http.get(url)
        .bearer_auth(api_key)
        .timeout(TIMEOUT)
        .send().await?;
    Ok(response.status().is_success())
}
