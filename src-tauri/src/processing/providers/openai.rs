//! OpenAI Chat Completions protocol — also used verbatim by Grok, Groq,
//! DeepSeek, Mistral, OpenRouter, Perplexity, Together, Ollama, LM Studio.

use super::super::GptConfig;
use anyhow::Result;
use reqwest::RequestBuilder;
use serde::Deserialize;

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    /// May be empty for reasoning models that put output in reasoning_content.
    #[serde(default)]
    content: Option<String>,
    /// Reasoning/thinking models (Gemma, Grok 4, etc.) put their real output here.
    #[serde(default)]
    reasoning_content: Option<String>,
}

pub fn build_request(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> RequestBuilder {
    let mut body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ],
    });

    if let Some(effort) = openai_reasoning_effort(&cfg.endpoint, &cfg.model) {
        // OpenAI reasoning models (gpt-5*, gpt-6*, o-series) reject a custom
        // `temperature` and the legacy `max_tokens` field. Their output budget
        // also covers hidden reasoning tokens, so add headroom on top of the
        // visible-output estimate.
        body["reasoning_effort"] = serde_json::json!(effort);
        if let Some(max) = cfg.max_tokens {
            body["max_completion_tokens"] = serde_json::json!(max + 2048);
        }
    } else {
        body["temperature"] = serde_json::json!(cfg.temperature);
        if let Some(max) = cfg.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }
    }

    // Built-in llama-server: Qwen3-style chat templates think by default,
    // which only adds latency for a cleanup pass.
    if cfg.disable_thinking {
        body["chat_template_kwargs"] = serde_json::json!({ "enable_thinking": false });
    }

    let mut req = http.post(&cfg.endpoint).json(&body);

    if let Some(key) = &cfg.api_key {
        req = req.bearer_auth(key);
    }

    // OpenRouter requests an HTTP-Referer + X-Title header so its dashboard can
    // attribute traffic. Harmless on every other OpenAI-compatible host.
    if cfg.endpoint.contains("openrouter.ai") {
        req = req
            .header("HTTP-Referer", "https://typeurvoice.app")
            .header("X-Title", "TypeUrVoice");
    }

    req
}

/// Lowest-latency `reasoning_effort` for OpenAI's own reasoning models, or
/// `None` for classic chat models and every non-OpenAI host (which keep the
/// temperature + max_tokens shape).
///
/// The original GPT-5 family (gpt-5, gpt-5-mini, gpt-5-nano) supports
/// `minimal` but not `none`; GPT-5.x point releases and GPT-6 support `none`;
/// the o-series bottoms out at `low`.
pub(crate) fn openai_reasoning_effort(endpoint: &str, model: &str) -> Option<&'static str> {
    if !endpoint.contains("api.openai.com") {
        return None;
    }
    let m = model.trim();
    if m == "gpt-5" || m.starts_with("gpt-5-") {
        Some("minimal")
    } else if m.starts_with("gpt-5.") || m.starts_with("gpt-6") {
        Some("none")
    } else if m.starts_with('o') && m.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
        Some("low")
    } else {
        None
    }
}

pub fn parse_response(raw_body: &str) -> Result<String> {
    let result: ChatResponse = serde_json::from_str(raw_body)
        .map_err(|e| anyhow::anyhow!("Failed to parse OpenAI-style response: {}. Raw: {:.200}", e, raw_body))?;

    let choice = result.choices.first()
        .ok_or_else(|| anyhow::anyhow!("LLM returned empty response (no choices)"))?;

    let content = choice.message.content.as_deref().unwrap_or("").trim();
    let reasoning = choice.message.reasoning_content.as_deref().unwrap_or("").trim();

    if !content.is_empty() {
        Ok(content.to_string())
    } else if !reasoning.is_empty() {
        eprintln!("[TypeUrVoice] LLM content was empty — using reasoning_content as fallback");
        Ok(reasoning.to_string())
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::openai_reasoning_effort;
    const OAI: &str = "https://api.openai.com/v1/chat/completions";

    #[test]
    fn reasoning_effort_by_family() {
        assert_eq!(openai_reasoning_effort(OAI, "gpt-5-nano"), Some("minimal"));
        assert_eq!(openai_reasoning_effort(OAI, "gpt-5"), Some("minimal"));
        assert_eq!(openai_reasoning_effort(OAI, "gpt-5.6-luna"), Some("none"));
        assert_eq!(openai_reasoning_effort(OAI, "gpt-6-luna"), Some("none"));
        assert_eq!(openai_reasoning_effort(OAI, "o3"), Some("low"));
        assert_eq!(openai_reasoning_effort(OAI, "gpt-4o-mini"), None);
        assert_eq!(openai_reasoning_effort(OAI, "gpt-4.1"), None);
    }

    #[test]
    fn other_hosts_keep_classic_shape() {
        assert_eq!(openai_reasoning_effort("https://api.groq.com/openai/v1/chat/completions", "openai/gpt-oss-20b"), None);
        assert_eq!(openai_reasoning_effort("http://127.0.0.1:8080/v1/chat/completions", "gpt-5"), None);
    }
}
