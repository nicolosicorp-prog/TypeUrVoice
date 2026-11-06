pub mod cleanup;
pub mod fillers;
pub mod guard;
pub mod translation;
pub mod summary;
pub mod providers;
pub mod occupation_profiles;
pub mod profile_dictionaries;

use providers::Protocol;

/// Unified LLM config — works across OpenAI, Anthropic, Gemini, every
/// OpenAI-compatible host (Grok, Groq, DeepSeek, Mistral, OpenRouter,
/// Perplexity, Together) and local backends (Ollama, LM Studio).
///
/// `protocol` selects the wire format; per-protocol code lives in
/// `processing::providers::{openai,anthropic,gemini}`.
#[derive(Clone)]
pub(super) struct GptConfig {
    pub endpoint:    String,
    pub model:       String,
    pub api_key:     Option<String>,
    pub protocol:    Protocol,
    /// Sampling temperature. 0.0 for mechanical edits (cleanup, translation),
    /// 0.3 for creative tasks (polish, summary).
    pub temperature: f32,
    /// Output cap. `None` lets the API decide (when the protocol allows it —
    /// Anthropic always requires one and the handler supplies a default).
    pub max_tokens:  Option<u32>,
    /// Ask OpenAI-compatible local servers to skip chat-template thinking
    /// (`chat_template_kwargs.enable_thinking = false`). Only set for the
    /// built-in llama-server, where unknown fields are safe.
    pub disable_thinking: bool,
}

impl GptConfig {
    /// Override the temperature for one call.
    pub fn with_temperature(&self, temperature: f32) -> Self {
        Self { temperature, ..self.clone() }
    }

    /// Override max_tokens for one call.
    pub fn with_max_tokens(&self, max_tokens: Option<u32>) -> Self {
        Self { max_tokens, ..self.clone() }
    }
}

/// Send one chat-completion request via the configured protocol. Retries
/// on 429 / 5xx, strips markdown code-fences from the response.
pub(super) async fn gpt_complete(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> anyhow::Result<String> {
    providers::complete(http, cfg, system, user).await
}

use anyhow::Result;
use crate::state::{Settings, OutputMode, TextProcessor};
use crate::storage::dictionary;

#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub processed_text: String,
    pub translated_text: Option<String>,
    pub summary: Option<String>,
    pub final_text: String, // What gets pasted
}

/// Build LLM config from settings. Pulls the API key from the multi-provider
/// keys.json store (legacy `_openai_key` arg ignored — kept for call-site
/// compatibility during the transition).
fn build_gpt_config(settings: &Settings, _openai_key_legacy: Option<&str>) -> GptConfig {
    let info = providers::catalog::for_settings(settings);

    // Local providers (Ollama, LM Studio) build endpoint from text_processor_url
    // at runtime since the user owns it. Hosted providers use the catalog endpoint.
    // BuiltIn: run_pipeline / test_text_processor put the running llama-server's
    // base URL into `text_processor_url` on their per-call settings clone.
    let is_local = matches!(
        settings.text_processor,
        TextProcessor::Ollama | TextProcessor::LmStudio | TextProcessor::BuiltIn
    );
    let endpoint = if is_local {
        let base = settings.text_processor_url.trim_end_matches('/');
        format!("{}/v1/chat/completions", base)
    } else {
        info.endpoint.clone()
    };

    // Model: hosted providers use processing_model (or fall back to catalog default);
    // local providers use text_processor_model.
    let model = if settings.text_processor == TextProcessor::BuiltIn {
        settings.builtin_llm_model.clone()  // informational — llama-server serves one model
    } else if is_local {
        settings.text_processor_model.clone()
    } else if !settings.processing_model.trim().is_empty() {
        settings.processing_model.clone()
    } else {
        info.default_model.clone()
    };

    let api_key = if info.requires_key && !info.key_slot.is_empty() {
        crate::storage::api_keys::get(&info.key_slot).ok().flatten()
    } else {
        None
    };

    GptConfig {
        endpoint,
        model,
        api_key,
        protocol:    info.protocol,
        temperature: 0.3,
        max_tokens:  Some(2000),
        disable_thinking: settings.text_processor == TextProcessor::BuiltIn,
    }
}

/// Minimal config pointing at an OpenAI-compatible endpoint (tests only).
#[cfg(test)]
pub(crate) fn test_config(endpoint: &str) -> GptConfig {
    GptConfig {
        endpoint: endpoint.to_string(),
        model: "builtin".to_string(),
        api_key: None,
        protocol: Protocol::OpenAiChat,
        temperature: 0.0,
        max_tokens: Some(500),
        disable_thinking: true,
    }
}

/// Token budget proportional to input length. Cleanup/Polish output is at
/// most ~1× the input; translation can be slightly longer than source.
/// Multiplier 0.6 for cleanup (chars→tokens, output ≤ input), 1.5 for translate.
fn token_budget_for(input: &str, multiplier: f32) -> u32 {
    let chars = input.chars().count() as f32;
    let tokens = (chars / 3.5 * multiplier) as u32;
    tokens.clamp(500, 4000)
}

/// Map ISO 639-1 code to a human-readable language name.
fn language_name(code: &str) -> &str {
    match code {
        "cs" => "Czech",
        "sk" => "Slovak",
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "pl" => "Polish",
        "nl" => "Dutch",
        "ru" => "Russian",
        "uk" => "Ukrainian",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" => "Chinese",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "tr" => "Turkish",
        "sv" => "Swedish",
        "da" => "Danish",
        "no" => "Norwegian",
        "fi" => "Finnish",
        "hu" => "Hungarian",
        "ro" => "Romanian",
        "hr" => "Croatian",
        "bg" => "Bulgarian",
        "el" => "Greek",
        "he" => "Hebrew",
        "th" => "Thai",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        _ => code,
    }
}

/// Fall back to the raw transcript when the LLM returns empty text or errors,
/// so the pipeline never pastes nothing. `mode` is "cleanup" or "polish" — used
/// only in the log line.
fn fallback_to_raw(result: Result<String>, raw_text: &str, mode: &str) -> String {
    match result {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            eprintln!("[TypeUrVoice] LLM returned empty {} — falling back to raw text", mode);
            raw_text.to_string()
        }
        Err(e) => {
            eprintln!("[TypeUrVoice] {} failed ({}) — falling back to raw text", mode, e);
            raw_text.to_string()
        }
    }
}

/// Clean must not change what was said — see `guard`.
fn guard_clean(raw_text: &str, cleaned: String, dictionary: &[String]) -> String {
    match guard::check_clean(raw_text, &cleaned, dictionary) {
        guard::Verdict::Ok => cleaned,
        guard::Verdict::Repaired(text, n) => {
            crate::log_to_file(&format!("Clean guard: put back {} word(s) the LLM swapped", n));
            text
        }
        guard::Verdict::Rejected(why) => {
            crate::log_to_file(&format!("Clean guard: LLM output rejected ({}) — using the transcript", why));
            raw_text.to_string()
        }
    }
}

pub async fn process(
    raw_text: &str,
    detected_language: &str,
    settings: &Settings,
    openai_key: Option<&str>,
    http: &reqwest::Client,
) -> Result<ProcessingResult> {
    let cfg = build_gpt_config(settings, openai_key);
    let lang_name = language_name(detected_language);

    // When an occupation profile is active it owns the dictionary hint: we
    // merge the bundled pack for this (locale, occupation) with the user's
    // personal dictionary so domain terms get biased on top of personal ones.
    let active_profile = occupation_profiles::active(settings);
    let merged_dictionary: Vec<String> = match &active_profile {
        Some(p) => {
            let pack = profile_dictionaries::lookup(p.locale, p.occupation);
            profile_dictionaries::merge_with_personal(&settings.dictionary, pack)
        }
        None => settings.dictionary.clone(),
    };
    let dict_hint = dictionary::format_for_prompt(&merged_dictionary);

    // Step 1: Cleanup. When an occupation profile is active, the profile's
    // prompt drives everything (code-switching preserve rule, structure,
    // language). Otherwise fall back to the user's OutputMode selection.
    let processed = if let Some(prof) = &active_profile {
        let profile_cfg = cfg
            .with_temperature(0.0)
            .with_max_tokens(Some(token_budget_for(raw_text, 1.0)));
        let result = cleanup::profile(http, raw_text, &profile_cfg, prof, dict_hint.as_deref()).await;
        fallback_to_raw(result, raw_text, "profile")
    } else { match settings.output_mode {
        OutputMode::Raw => raw_text.to_string(),
        OutputMode::Clean => {
            // Mechanical edit: zero temperature, output ≤ input length.
            let cleanup_cfg = cfg
                .with_temperature(0.0)
                .with_max_tokens(Some(token_budget_for(raw_text, 0.6)));
            let result = cleanup::clean(http, raw_text, &cleanup_cfg, dict_hint.as_deref(), lang_name).await;
            guard_clean(raw_text, fallback_to_raw(result, raw_text, "cleanup"), &merged_dictionary)
        }
        OutputMode::Polished => {
            // Creative rewrite: keep default temperature, allow output ≈ input.
            let polish_cfg = cfg.with_max_tokens(Some(token_budget_for(raw_text, 1.0)));
            let result = cleanup::polish(http, raw_text, &polish_cfg, dict_hint.as_deref(), lang_name).await;
            fallback_to_raw(result, raw_text, "polish")
        }
        OutputMode::Custom => {
            // Active profile's prompt — empty / missing ⇒ behave as Raw
            // (no point spending an LLM call).
            match settings.active_custom_prompt() {
                None => raw_text.to_string(),
                Some(user_prompt) => {
                    let custom_cfg = cfg.with_max_tokens(Some(token_budget_for(raw_text, 1.5)));
                    let result = cleanup::custom(http, raw_text, &custom_cfg, user_prompt, dict_hint.as_deref(), lang_name).await;
                    fallback_to_raw(result, raw_text, "custom")
                }
            }
        }
    }};

    // Step 2: Translation (optional). Always 0.0 temperature — translation is
    // mechanical, deterministic. Source language passed through language_name
    // so prompts read "from Czech to English", not "from cs to English".
    let translated = if settings.translation_enabled && settings.target_language != "auto" {
        let source_code = if settings.source_language == "auto" {
            detected_language
        } else {
            &settings.source_language
        };
        let source = language_name(source_code);
        let target = language_name(&settings.target_language);
        let translate_cfg = cfg
            .with_temperature(0.0)
            .with_max_tokens(Some(token_budget_for(&processed, 1.5)));
        Some(translation::translate(http, &processed, source, target, &translate_cfg).await?)
    } else {
        None
    };

    // Step 3: Summary (optional). Pass source language so the summary stays
    // in the speaker's language — without this hint the LLM defaults to English.
    let summary_text = if settings.summary_enabled {
        let text_to_summarize = translated.as_deref().unwrap_or(&processed);
        // If translation is on, the summary should be in the target language.
        let summary_lang = if settings.translation_enabled {
            language_name(&settings.target_language)
        } else {
            lang_name
        };
        Some(summary::summarize(http, text_to_summarize, &settings.summary_type, summary_lang, &cfg).await?)
    } else {
        None
    };

    // Decide what gets pasted
    let final_text = if settings.summary_enabled {
        summary_text.clone().unwrap_or_else(|| processed.clone())
    } else if settings.translation_enabled {
        translated.clone().unwrap_or_else(|| processed.clone())
    } else {
        processed.clone()
    };

    Ok(ProcessingResult {
        processed_text: processed,
        translated_text: translated,
        summary: summary_text,
        final_text,
    })
}

/// Test connectivity to the configured text-processing endpoint.
pub async fn test_processor(
    http: &reqwest::Client,
    settings: &Settings,
    openai_key: Option<&str>,
) -> Result<()> {
    let cfg = build_gpt_config(settings, openai_key);
    let result = gpt_complete(http, &cfg, "You are a test assistant.", "Reply with OK.").await?;
    if result.is_empty() {
        return Err(anyhow::anyhow!("Empty response from LLM"));
    }
    Ok(())
}
