//! Multi-provider API key store.
//!
//! Replaces the single `.api_key` file (OpenAI-only) with a `keys.json`
//! object that has one slot per provider. Migration of the legacy file
//! runs once at app start (`migrate_legacy_if_needed`).
//!
//! Provider keys are stored under these slot names:
//!   "openai"        — OpenAI API key
//!   "anthropic"     — Anthropic / Claude
//!   "gemini"        — Google Gemini
//!   "compat:grok"   — xAI Grok (per-preset slot, see plan E5(c))
//!   "compat:groq"   — Groq
//!   "compat:deepseek" / "compat:mistral" / "compat:openrouter" /
//!   "compat:perplexity" / "compat:together" / "compat:custom"
//!
//! File mode is 0600 on Unix (owner-only read/write).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{restrict_permissions, settings::voiceink_data_dir};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KeyStore {
    #[serde(flatten)]
    pub keys: BTreeMap<String, String>,
}

fn keys_path() -> Result<PathBuf> {
    Ok(voiceink_data_dir()?.join("keys.json"))
}

fn legacy_openai_path() -> Result<PathBuf> {
    Ok(voiceink_data_dir()?.join(".api_key"))
}

pub fn load() -> Result<KeyStore> {
    let path = keys_path()?;
    if !path.exists() {
        return Ok(KeyStore::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let store: KeyStore = serde_json::from_str(&content).unwrap_or_default();
    Ok(store)
}

pub fn save(store: &KeyStore) -> Result<()> {
    let path = keys_path()?;
    let content = serde_json::to_string_pretty(store)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)?;
    let _ = restrict_permissions(&tmp);
    std::fs::rename(&tmp, &path)?;
    let _ = restrict_permissions(&path);
    Ok(())
}

/// Get the API key for `slot`. Returns `None` if absent or empty.
pub fn get(slot: &str) -> Result<Option<String>> {
    let store = load()?;
    Ok(store.keys.get(slot).filter(|k| !k.trim().is_empty()).cloned())
}

/// Set the API key for `slot`. Empty string deletes the slot.
pub fn set(slot: &str, key: &str) -> Result<()> {
    let mut store = load()?;
    if key.trim().is_empty() {
        store.keys.remove(slot);
    } else {
        store.keys.insert(slot.to_string(), key.trim().to_string());
    }
    save(&store)?;
    Ok(())
}

/// True if any provider slot holds a non-empty key. Used by the launch
/// path as a defense-in-depth signal that the user has already been past
/// onboarding (Whisper Cloud or any LLM provider configured).
pub fn has_any() -> bool {
    match load() {
        Ok(store) => store.keys.values().any(|v| !v.trim().is_empty()),
        Err(_) => false,
    }
}

/// Remove the API key for `slot`.
pub fn delete(slot: &str) -> Result<()> {
    let mut store = load()?;
    store.keys.remove(slot);
    save(&store)
}

/// One-shot migration: if the legacy `.api_key` exists and the new
/// `keys.json` doesn't, copy the OpenAI key over and delete the legacy file.
pub fn migrate_legacy_if_needed() -> Result<()> {
    let new_path = keys_path()?;
    if new_path.exists() {
        return Ok(());
    }
    let legacy = legacy_openai_path()?;
    if !legacy.exists() {
        return Ok(());
    }
    let key = std::fs::read_to_string(&legacy)?.trim().to_string();
    if !key.is_empty() {
        let mut store = KeyStore::default();
        store.keys.insert("openai".to_string(), key);
        save(&store)?;
    }
    let _ = std::fs::remove_file(&legacy);
    crate::log_to_file("api_keys: migrated legacy .api_key → keys.json");
    Ok(())
}
