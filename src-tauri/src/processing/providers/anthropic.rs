//! Anthropic Messages API — Claude.
//!
//! Differs from OpenAI in three ways:
//! - `system` is a top-level field, not a `messages[0]`
//! - `max_tokens` is REQUIRED (we always pass one)
//! - Auth is `x-api-key`, not `Authorization: Bearer`
//! - Needs `anthropic-version: 2023-06-01` header

use super::super::GptConfig;
use anyhow::Result;
use reqwest::RequestBuilder;
use serde::Deserialize;

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text { text: String },
    /// Future-proof against tool_use / thinking blocks — ignore in our use case.
    #[serde(other)]
    Other,
}

pub fn build_request(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> RequestBuilder {
    // max_tokens is required; default to a generous cap if call site didn't set one.
    let max_tokens = cfg.max_tokens.unwrap_or(4096);

    let mut body = serde_json::json!({
        "model": cfg.model,
        "system": system,
        "messages": [
            { "role": "user", "content": user }
        ],
    });

    if is_adaptive_only(&cfg.model) {
        // Claude 5-generation models reject non-default sampling params and
        // think adaptively by default. Low effort keeps a cleanup pass fast;
        // max_tokens also covers thinking tokens, so add headroom.
        body["max_tokens"] = serde_json::json!(max_tokens + 2048);
        body["output_config"] = serde_json::json!({ "effort": "low" });
    } else {
        body["max_tokens"] = serde_json::json!(max_tokens);
        body["temperature"] = serde_json::json!(cfg.temperature);
    }

    let mut req = http.post(&cfg.endpoint)
        .header("anthropic-version", "2023-06-01")
        .json(&body);

    if let Some(key) = &cfg.api_key {
        req = req.header("x-api-key", key);
    }

    req
}

/// Claude models from the 5 generation on (plus Opus 4.7/4.8) reject a
/// custom `temperature` and support `output_config.effort`.
fn is_adaptive_only(model: &str) -> bool {
    let m = model.trim_start_matches("anthropic/");
    ["claude-haiku-5", "claude-sonnet-5", "claude-opus-5", "claude-fable-5",
     "claude-opus-4-7", "claude-opus-4-8"]
        .iter()
        .any(|p| m.starts_with(p))
}

pub fn parse_response(raw_body: &str) -> Result<String> {
    let result: AnthropicResponse = serde_json::from_str(raw_body)
        .map_err(|e| anyhow::anyhow!("Failed to parse Anthropic response: {}. Raw: {:.200}", e, raw_body))?;

    if let Some(reason) = &result.stop_reason {
        if reason == "max_tokens" {
            eprintln!("[VoiceInk] Anthropic: hit max_tokens — output may be truncated");
        }
    }

    // Concatenate text blocks (Claude usually returns one). Skip tool-use / thinking.
    let text: String = result.content.iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Other => None,
        })
        .collect::<Vec<_>>()
        .join("");

    Ok(text)
}
