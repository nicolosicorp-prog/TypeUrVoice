//! Static catalogue of providers + their default endpoints / model lists.
//!
//! Hardcoded for v1; Phase E8 will add a /models fetch with 24h cache so
//! these lists stay fresh against API changes. The defaults below are the
//! recommended/cheapest model for each provider (good fit for cleanup tasks).

use crate::state::{Settings, TextProcessor, OpenAiCompatPreset};
use super::Protocol;

/// Per-provider metadata: endpoint, default model, suggested model list,
/// wire protocol, and the keys.json slot where the API key is stored.
///
/// Owned strings throughout — built per-call from settings. The cost
/// (a handful of small allocations per recording) is dwarfed by the
/// HTTP round-trip that follows.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub display_name:    String,
    pub endpoint:        String,
    pub default_model:   String,
    pub recommended:     Vec<String>,
    pub protocol:        Protocol,
    /// Empty ⇒ no key required (Ollama, LM Studio).
    pub key_slot:        String,
    /// True ⇒ user must enter an API key.
    pub requires_key:    bool,
    /// True ⇒ provider has a stable model catalogue (fetched lazily in E8);
    /// false ⇒ free-form model entry (OpenRouter, Together, Custom).
    pub stable_catalog:  bool,
}

fn s(arr: &[&str]) -> Vec<String> {
    arr.iter().map(|s| (*s).to_string()).collect()
}

pub fn openai() -> ProviderInfo {
    ProviderInfo {
        display_name:   "OpenAI".to_string(),
        endpoint:       "https://api.openai.com/v1/chat/completions".to_string(),
        default_model:  "gpt-6-luna".to_string(),
        recommended:    s(&["gpt-6-luna", "gpt-5.6-luna", "gpt-5-nano", "gpt-5-mini", "gpt-6.1-sol"]),
        protocol:       Protocol::OpenAiChat,
        key_slot:       "openai".to_string(),
        requires_key:   true,
        stable_catalog: true,
    }
}

pub fn anthropic() -> ProviderInfo {
    ProviderInfo {
        display_name:   "Anthropic".to_string(),
        endpoint:       "https://api.anthropic.com/v1/messages".to_string(),
        default_model:  "claude-haiku-5-5".to_string(),
        recommended:    s(&["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"]),
        protocol:       Protocol::Anthropic,
        key_slot:       "anthropic".to_string(),
        requires_key:   true,
        stable_catalog: true,
    }
}

pub fn gemini() -> ProviderInfo {
    ProviderInfo {
        display_name:   "Google Gemini".to_string(),
        // Base URL — gemini handler appends /models/{model}:generateContent
        endpoint:       "https://generativelanguage.googleapis.com/v1beta".to_string(),
        default_model:  "gemini-3.5-flash-lite".to_string(),
        recommended:    s(&["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-3.5-flash", "gemini-3.1-pro-preview"]),
        protocol:       Protocol::Gemini,
        key_slot:       "gemini".to_string(),
        requires_key:   true,
        stable_catalog: true,
    }
}

/// OpenAI-compatible presets. Each maps to a base URL + recommended models.
/// All speak `Protocol::OpenAiChat`.
pub fn openai_compat(preset: &OpenAiCompatPreset, custom_url: &str) -> ProviderInfo {
    match preset {
        OpenAiCompatPreset::Grok => ProviderInfo {
            display_name:   "xAI Grok".to_string(),
            endpoint:       "https://api.x.ai/v1/chat/completions".to_string(),
            default_model:  "grok-4.1-fast".to_string(),
            recommended:    s(&["grok-4.1-fast", "grok-4.3"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:grok".to_string(),
            requires_key:   true,
            stable_catalog: true,
        },
        OpenAiCompatPreset::Groq => ProviderInfo {
            display_name:   "Groq".to_string(),
            endpoint:       "https://api.groq.com/openai/v1/chat/completions".to_string(),
            default_model:  "llama-3.3-70b-versatile".to_string(),
            recommended:    s(&["llama-3.3-70b-versatile", "openai/gpt-oss-20b", "openai/gpt-oss-120b", "llama-3.1-8b-instant"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:groq".to_string(),
            requires_key:   true,
            stable_catalog: true,
        },
        OpenAiCompatPreset::DeepSeek => ProviderInfo {
            display_name:   "DeepSeek".to_string(),
            endpoint:       "https://api.deepseek.com/v1/chat/completions".to_string(),
            default_model:  "deepseek-v4-flash".to_string(),
            recommended:    s(&["deepseek-v4-flash", "deepseek-v4-pro"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:deepseek".to_string(),
            requires_key:   true,
            stable_catalog: true,
        },
        OpenAiCompatPreset::Mistral => ProviderInfo {
            display_name:   "Mistral".to_string(),
            endpoint:       "https://api.mistral.ai/v1/chat/completions".to_string(),
            default_model:  "mistral-small-latest".to_string(),
            recommended:    s(&["mistral-small-latest", "mistral-medium-latest", "mistral-large-latest"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:mistral".to_string(),
            requires_key:   true,
            stable_catalog: true,
        },
        OpenAiCompatPreset::OpenRouter => ProviderInfo {
            display_name:   "OpenRouter".to_string(),
            endpoint:       "https://openrouter.ai/api/v1/chat/completions".to_string(),
            default_model:  "openai/gpt-6-luna".to_string(),
            recommended:    s(&["openai/gpt-6-luna", "anthropic/claude-haiku-5-5", "google/gemini-3.5-flash-lite", "meta-llama/llama-3.3-70b-instruct"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:openrouter".to_string(),
            requires_key:   true,
            stable_catalog: false,
        },
        OpenAiCompatPreset::Perplexity => ProviderInfo {
            display_name:   "Perplexity".to_string(),
            endpoint:       "https://api.perplexity.ai/chat/completions".to_string(),
            default_model:  "sonar".to_string(),
            recommended:    s(&["sonar", "sonar-pro"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:perplexity".to_string(),
            requires_key:   true,
            stable_catalog: true,
        },
        OpenAiCompatPreset::Together => ProviderInfo {
            display_name:   "Together AI".to_string(),
            endpoint:       "https://api.together.xyz/v1/chat/completions".to_string(),
            default_model:  "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string(),
            recommended:    s(&[
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
                "openai/gpt-oss-20b",
            ]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       "compat:together".to_string(),
            requires_key:   true,
            stable_catalog: false,
        },
        OpenAiCompatPreset::Custom => {
            let endpoint = if custom_url.trim().is_empty() {
                "http://localhost:8080/v1/chat/completions".to_string()
            } else {
                let base = custom_url.trim_end_matches('/');
                if base.ends_with("/chat/completions") {
                    base.to_string()
                } else if base.ends_with("/v1") {
                    format!("{}/chat/completions", base)
                } else {
                    format!("{}/v1/chat/completions", base)
                }
            };
            ProviderInfo {
                display_name:   "Custom".to_string(),
                endpoint,
                default_model:  String::new(),
                recommended:    Vec::new(),
                protocol:       Protocol::OpenAiChat,
                key_slot:       "compat:custom".to_string(),
                requires_key:   true,
                stable_catalog: false,
            }
        }
    }
}

/// Resolve provider info from current settings.
pub fn for_settings(settings: &Settings) -> ProviderInfo {
    match settings.text_processor {
        TextProcessor::OpenAi    => openai(),
        TextProcessor::Anthropic => anthropic(),
        TextProcessor::Gemini    => gemini(),
        TextProcessor::OpenAiCompat => openai_compat(
            &settings.openai_compat_preset,
            &settings.openai_compat_url,
        ),
        TextProcessor::Ollama => ProviderInfo {
            display_name:   "Ollama".to_string(),
            endpoint:       String::new(),  // built from text_processor_url at runtime
            default_model:  "gemma4".to_string(),
            recommended:    s(&["gemma4", "qwen3", "llama3.2"]),
            protocol:       Protocol::OpenAiChat,
            key_slot:       String::new(),
            requires_key:   false,
            stable_catalog: false,
        },
        TextProcessor::BuiltIn => ProviderInfo {
            display_name:   "Built-in (on this Mac)".to_string(),
            endpoint:       String::new(),  // filled from the running llama-server
            default_model:  crate::local_llm::models::DEFAULT_MODEL.to_string(),
            recommended:    crate::local_llm::models::MODELS.iter().map(|m| m.key.to_string()).collect(),
            protocol:       Protocol::OpenAiChat,
            key_slot:       String::new(),
            requires_key:   false,
            stable_catalog: false,
        },
        TextProcessor::LmStudio => ProviderInfo {
            display_name:   "LM Studio".to_string(),
            endpoint:       String::new(),
            default_model:  String::new(),
            recommended:    Vec::new(),
            protocol:       Protocol::OpenAiChat,
            key_slot:       String::new(),
            requires_key:   false,
            stable_catalog: false,
        },
    }
}
