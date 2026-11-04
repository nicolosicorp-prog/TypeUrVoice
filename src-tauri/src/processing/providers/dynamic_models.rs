//! Optional /models fetch with 24h disk cache.
//!
//! Each provider's hardcoded list in `catalog.rs` is good for v1 but goes
//! stale fast (Anthropic shipped 4.5 in a 6-month gap; OpenAI shipped 5).
//! This module hits each provider's `/models` endpoint when invoked, returns
//! a list of model ids, and persists the result for 24h so the next call
//! is instant.
//!
//! UI side: EngineTab calls `fetch_models(slot)` on demand (e.g. after
//! provider change). On success it merges the returned list with the
//! catalogue's `recommended` list. On failure we silently keep using the
//! recommended list — this is best-effort, not load-bearing.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::settings::voiceink_data_dir;

const CACHE_TTL_SECONDS: u64 = 24 * 60 * 60; // 24h

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,  // unix seconds
    models: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModelCache {
    /// Slot ⇒ entry. Slot keys mirror keys.json (e.g. "openai", "anthropic",
    /// "compat:grok"). Cache is shared across runs, so users see fresh lists
    /// without hammering provider APIs every recording.
    #[serde(flatten)]
    by_slot: BTreeMap<String, CacheEntry>,
}

fn cache_path() -> Result<PathBuf> {
    Ok(voiceink_data_dir()?.join("model_cache.json"))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load_cache() -> ModelCache {
    let path = match cache_path() {
        Ok(p) => p,
        Err(_) => return ModelCache::default(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ModelCache::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_cache(cache: &ModelCache) -> Result<()> {
    let path = cache_path()?;
    let content = serde_json::to_string_pretty(cache)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Get cached models for `slot` if the entry is still fresh, else `None`.
pub fn cached_fresh(slot: &str) -> Option<Vec<String>> {
    let cache = load_cache();
    let entry = cache.by_slot.get(slot)?;
    if now_secs().saturating_sub(entry.fetched_at) < CACHE_TTL_SECONDS {
        Some(entry.models.clone())
    } else {
        None
    }
}

fn write_to_cache(slot: &str, models: &[String]) {
    let mut cache = load_cache();
    cache.by_slot.insert(slot.to_string(), CacheEntry {
        fetched_at: now_secs(),
        models: models.to_vec(),
    });
    let _ = save_cache(&cache);
}

/// Fetch models for `slot`. Returns the cached list if still fresh,
/// otherwise hits the provider's /models endpoint and updates the cache.
/// Best-effort — returns `Ok(empty)` if the slot has no usable /models endpoint.
pub async fn fetch_models(slot: &str, http: &reqwest::Client) -> Result<Vec<String>> {
    if let Some(cached) = cached_fresh(slot) {
        return Ok(cached);
    }

    let api_key = crate::storage::api_keys::get(slot)?
        .ok_or_else(|| anyhow::anyhow!("No API key for slot {} — cannot fetch models", slot))?;

    let models = match slot {
        "openai"            => fetch_openai_style(http, "https://api.openai.com/v1/models", &api_key).await?,
        "anthropic"         => fetch_anthropic(http, &api_key).await?,
        "gemini"            => fetch_gemini(http, &api_key).await?,
        "compat:grok"       => fetch_openai_style(http, "https://api.x.ai/v1/models", &api_key).await?,
        "compat:groq"       => fetch_openai_style(http, "https://api.groq.com/openai/v1/models", &api_key).await?,
        "compat:deepseek"   => fetch_openai_style(http, "https://api.deepseek.com/v1/models", &api_key).await?,
        "compat:mistral"    => fetch_openai_style(http, "https://api.mistral.ai/v1/models", &api_key).await?,
        "compat:openrouter" => fetch_openai_style(http, "https://openrouter.ai/api/v1/models", &api_key).await?,
        "compat:perplexity" => Vec::new(),  // Perplexity has no /models endpoint
        "compat:together"   => fetch_openai_style(http, "https://api.together.xyz/v1/models", &api_key).await?,
        _ => Vec::new(),
    };

    if !models.is_empty() {
        write_to_cache(slot, &models);
    }
    Ok(models)
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}
#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

async fn fetch_openai_style(http: &reqwest::Client, url: &str, api_key: &str) -> Result<Vec<String>> {
    let response = http.get(url)
        .bearer_auth(api_key)
        .timeout(std::time::Duration::from_secs(15))
        .send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Models endpoint returned {}", response.status().as_u16()));
    }
    let parsed: OpenAiModelsResponse = response.json().await?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}
#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}

async fn fetch_anthropic(http: &reqwest::Client, api_key: &str) -> Result<Vec<String>> {
    let response = http.get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(15))
        .send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Anthropic /models returned {}", response.status().as_u16()));
    }
    let parsed: AnthropicModelsResponse = response.json().await?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}
#[derive(Deserialize)]
struct GeminiModel {
    name: String,  // "models/gemini-2.5-flash"
    #[serde(default)]
    supported_generation_methods: Vec<String>,
}

async fn fetch_gemini(http: &reqwest::Client, api_key: &str) -> Result<Vec<String>> {
    let response = http.get("https://generativelanguage.googleapis.com/v1beta/models")
        .bearer_auth(api_key)
        .timeout(std::time::Duration::from_secs(15))
        .send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Gemini /models returned {}", response.status().as_u16()));
    }
    let parsed: GeminiModelsResponse = response.json().await?;
    let mut ids: Vec<String> = parsed.models.into_iter()
        // Keep only models that support generateContent (skip embedding-only models, etc.)
        .filter(|m| m.supported_generation_methods.is_empty()
                 || m.supported_generation_methods.iter().any(|s| s == "generateContent"))
        .map(|m| m.name.trim_start_matches("models/").to_string())
        .collect();
    ids.sort();
    Ok(ids)
}
