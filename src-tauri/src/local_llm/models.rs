//! Catalog of downloadable GGUF models for the Built-in text processor.
//!
//! Hashes are the Hugging Face LFS SHA-256 values (verified against a local
//! `shasum -a 256` of the downloaded file). All models are Apache-2.0.

use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

pub struct LlmModel {
    pub key: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub url: &'static str,
    pub filename: &'static str,
    pub hash: &'static str,
    pub size_mb: u32,
    /// Approximate resident memory while loaded (weights + 4k context).
    pub ram_gb: f32,
}

pub const DEFAULT_MODEL: &str = "qwen3.5-2b";

pub const MODELS: &[LlmModel] = &[
    LlmModel {
        key: "qwen3.5-0.8b",
        name: "Qwen3.5 0.8B",
        note: "Tiniest and fastest. Fine for punctuation fixes; weaker rewriting.",
        url: "https://huggingface.co/ggml-org/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q8_0.gguf",
        filename: "Qwen3.5-0.8B-Q8_0.gguf",
        hash: "37ae482d336108d23516fa35e8e0c4126688d81018b87178a18d752a1357814f",
        size_mb: 830,
        ram_gb: 1.1,
    },
    LlmModel {
        key: "qwen3.5-2b",
        name: "Qwen3.5 2B",
        note: "Recommended. Fast cleanup in 100+ languages, including Czech and Slovak.",
        url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf",
        filename: "Qwen3.5-2B-Q4_K_M.gguf",
        hash: "aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223",
        size_mb: 1280,
        ram_gb: 1.6,
    },
    LlmModel {
        key: "qwen3.5-4b",
        name: "Qwen3.5 4B",
        note: "Best quality of the small models — better for Polished and Custom prompts.",
        url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf",
        filename: "Qwen3.5-4B-Q4_K_M.gguf",
        hash: "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4",
        size_mb: 2740,
        ram_gb: 3.2,
    },
    LlmModel {
        key: "gemma-4-e2b",
        name: "Gemma 4 E2B",
        note: "Google's small multilingual model (140+ languages). Strong on Central European text.",
        url: "https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf",
        filename: "gemma-4-E2B-it-Q4_K_M.gguf",
        hash: "740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8",
        size_mb: 3110,
        ram_gb: 3.6,
    },
];

pub fn lookup(key: &str) -> Option<&'static LlmModel> {
    MODELS.iter().find(|m| m.key == key)
}

pub fn models_dir() -> Result<PathBuf> {
    let dir = crate::storage::settings::voiceink_data_dir()?.join("llm-models");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn path(key: &str) -> Result<PathBuf> {
    let info = lookup(key).ok_or_else(|| anyhow::anyhow!("Unknown built-in model: {}", key))?;
    Ok(models_dir()?.join(info.filename))
}

pub fn is_downloaded(key: &str) -> bool {
    path(key).map(|p| p.exists()).unwrap_or(false)
}

/// Row for the Settings UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub key: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub size_mb: u32,
    pub ram_gb: f32,
    pub downloaded: bool,
    pub recommended: bool,
}

pub fn rows() -> Vec<ModelRow> {
    MODELS
        .iter()
        .map(|m| ModelRow {
            key: m.key,
            name: m.name,
            note: m.note,
            size_mb: m.size_mb,
            ram_gb: m.ram_gb,
            downloaded: is_downloaded(m.key),
            recommended: m.key == DEFAULT_MODEL,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert!(lookup(DEFAULT_MODEL).is_some());
        for m in MODELS {
            assert_eq!(m.hash.len(), 64, "{}", m.key);
            assert!(m.hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(m.url.ends_with(m.filename), "{}", m.key);
            assert_eq!(MODELS.iter().filter(|o| o.key == m.key).count(), 1);
        }
    }
}
