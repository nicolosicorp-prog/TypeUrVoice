//! Google Gemini — Generative Language API (`generateContent`).
//!
//! Differs from OpenAI in:
//! - `system_instruction` is a top-level object, not part of `messages`
//! - Roles use `user` and `model` (not `user`/`assistant`)
//! - `safetySettings` block must be permissive — without it, perfectly
//!   innocuous transcripts get blocked as "harassment" / "dangerous_content".
//!   This is a transcription tool: the user is dictating their own words, so
//!   we set every category to BLOCK_NONE.
//! - Auth: `Authorization: Bearer <key>` works (cleaner than `?key=...` in URL,
//!   keeps the key out of logs).
//! - Endpoint takes the model name in the URL path:
//!   https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent

use super::super::GptConfig;
use anyhow::Result;
use reqwest::RequestBuilder;
use serde::Deserialize;

#[derive(Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default)]
    #[allow(dead_code)]
    prompt_feedback: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(default)]
    text: Option<String>,
}

/// Build the per-request URL — Gemini requires the model name in the path.
/// `cfg.endpoint` is treated as the base (e.g. "https://generativelanguage.googleapis.com/v1beta")
/// and the model + ":generateContent" suffix are appended.
fn build_url(cfg: &GptConfig) -> String {
    let base = cfg.endpoint.trim_end_matches('/');
    format!("{}/models/{}:generateContent", base, cfg.model)
}

pub fn build_request(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> RequestBuilder {
    let mut generation_config = serde_json::json!({});
    if cfg.model.starts_with("gemini-3") {
        // Gemini 3 is tuned for the default temperature (1.0) and thinks by
        // default. Flash-Lite already defaults to minimal thinking; ask the
        // others for "low" so a cleanup pass doesn't sit in reasoning.
        if !cfg.model.contains("flash-lite") {
            generation_config["thinkingConfig"] = serde_json::json!({ "thinkingLevel": "low" });
        }
    } else {
        generation_config["temperature"] = serde_json::json!(cfg.temperature);
    }
    if let Some(max) = cfg.max_tokens {
        generation_config["maxOutputTokens"] = serde_json::json!(max);
    }

    let body = serde_json::json!({
        "system_instruction": {
            "parts": [{ "text": system }]
        },
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": user }]
            }
        ],
        "generationConfig": generation_config,
        // Permissive safety — this is a dictation tool, the user is transcribing
        // their own speech. Default thresholds reject mundane content.
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT",        "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_HATE_SPEECH",       "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" },
        ]
    });

    let url = build_url(cfg);
    let mut req = http.post(&url).json(&body);

    if let Some(key) = &cfg.api_key {
        req = req.bearer_auth(key);
    }

    req
}

pub fn parse_response(raw_body: &str) -> Result<String> {
    let result: GeminiResponse = serde_json::from_str(raw_body)
        .map_err(|e| anyhow::anyhow!("Failed to parse Gemini response: {}. Raw: {:.200}", e, raw_body))?;

    let candidate = result.candidates.first()
        .ok_or_else(|| anyhow::anyhow!(
            "Gemini returned no candidates — content may have been blocked by safety filters"
        ))?;

    if let Some(reason) = &candidate.finish_reason {
        if reason != "STOP" && reason != "MAX_TOKENS" {
            eprintln!("[TypeUrVoice] Gemini finish_reason: {} — output may be incomplete", reason);
        }
    }

    let content = candidate.content.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Gemini candidate had no content"))?;

    let text: String = content.parts.iter()
        .filter_map(|p| p.text.as_deref())
        .collect::<Vec<_>>()
        .join("");

    Ok(text)
}
