//! Provider protocol dispatch.
//!
//! Each provider speaks one of three wire protocols: OpenAI Chat Completions,
//! Anthropic Messages, or Google Gemini generateContent. Per-protocol code
//! lives in this module's submodules; the shared retry + error mapping +
//! markdown-fence stripping lives in `complete()` below.

pub mod openai;
pub mod anthropic;
pub mod gemini;
pub mod catalog;
pub mod dynamic_models;

use super::GptConfig;
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI `/v1/chat/completions` shape. Used by OpenAI itself plus every
    /// "OpenAI-compatible" host (Grok, Groq, DeepSeek, Mistral, OpenRouter,
    /// Perplexity, Together, Ollama, LM Studio).
    OpenAiChat,
    /// Anthropic `/v1/messages` shape (Claude).
    Anthropic,
    /// Google Generative Language API (`v1beta/models/.../:generateContent`).
    Gemini,
}

/// Strip a leading/trailing ```...``` markdown fence if the LLM wrapped its
/// answer in one. We instruct against this in the system prompts but smaller
/// models occasionally do it anyway.
fn strip_code_fence(s: &str) -> String {
    let trimmed = s.trim();
    if !trimmed.starts_with("```") {
        return s.to_string();
    }
    let after_open = match trimmed.find('\n') {
        Some(idx) => &trimmed[idx + 1..],
        None => return s.to_string(),
    };
    let stripped = match after_open.rfind("```") {
        Some(idx) => after_open[..idx].trim_end(),
        None => after_open,
    };
    stripped.trim().to_string()
}

/// Single-shot LLM completion with single retry on transient errors.
/// Per-protocol request building + response parsing is delegated to
/// `openai`, `anthropic`, `gemini` submodules.
pub async fn complete(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    eprintln!("[VoiceInk] LLM request → {} (model: {}, protocol: {:?}, temp: {})",
        cfg.endpoint, cfg.model, cfg.protocol, cfg.temperature);

    let mut attempts = 0u8;
    loop {
        attempts += 1;

        let req = match cfg.protocol {
            Protocol::OpenAiChat => openai::build_request(http, cfg, system, user),
            Protocol::Anthropic  => anthropic::build_request(http, cfg, system, user),
            Protocol::Gemini     => gemini::build_request(http, cfg, system, user),
        };

        let send_result = req
            .timeout(std::time::Duration::from_secs(60))
            .send().await;

        match send_result {
            Ok(response) => {
                let status = response.status();
                let is_retryable = status.as_u16() == 429 || status.is_server_error();
                if is_retryable && attempts < 2 {
                    eprintln!("[VoiceInk] LLM transient error {} — retrying in 2s", status.as_u16());
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    eprintln!("[VoiceInk] LLM error {}: {}", status.as_u16(), body);
                    return Err(anyhow::anyhow!("LLM API error {}: {}", status.as_u16(), body));
                }
                let raw_body = response.text().await?;
                let text = match cfg.protocol {
                    Protocol::OpenAiChat => openai::parse_response(&raw_body)?,
                    Protocol::Anthropic  => anthropic::parse_response(&raw_body)?,
                    Protocol::Gemini     => gemini::parse_response(&raw_body)?,
                };
                let cleaned = strip_code_fence(&text);
                eprintln!("[VoiceInk] LLM response: {} chars", cleaned.len());
                return Ok(cleaned);
            }
            Err(e) => {
                if attempts < 2 {
                    eprintln!("[VoiceInk] LLM connection error: {} — retrying in 2s", e);
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                eprintln!("[VoiceInk] LLM connection failed: {}", e);
                return Err(anyhow::anyhow!("Cannot connect to LLM: {}. Check that the service is running.", e));
            }
        }
    }
}
