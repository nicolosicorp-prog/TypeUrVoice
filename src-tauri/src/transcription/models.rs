//! Single source of truth for the GGML Whisper models we ship with.
//!
//! Before this module, the same eight models were described across four
//! places: `MODEL_URLS` (key/name/url), `model_filename()` (key→filename),
//! `MODEL_HASHES` (key→sha256), and the `LocalModel` enum's `key()` impl.
//! Adding a model meant editing four files. Now: one row per model, one
//! lookup helper.

use std::path::PathBuf;

/// All metadata for a single GGML model.
pub struct ModelInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub filename: &'static str,
    /// Pinned SHA-256 (lowercase hex). `None` ⇒ verification is skipped
    /// with a warning (lets us ship before a hash is captured without
    /// bricking installs).
    pub hash: Option<&'static str>,
}

/// All models from https://huggingface.co/ggerganov/whisper.cpp.
///
/// For Czech and Slovak, ONLY use multilingual (no `.en` suffix) models.
/// Q5 quantised variants are ~40-60% smaller with near-identical accuracy.
pub const MODELS: &[ModelInfo] = &[
    // ── Quantized multilingual (recommended) ─────────────────────────────
    ModelInfo {
        key: "small-q5",
        name: "Small Q5 (~270 MB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        filename: "ggml-small-q5_1.bin",
        hash: Some("ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb"),
    },
    ModelInfo {
        key: "medium-q5",
        name: "Medium Q5 (~514 MB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
        filename: "ggml-medium-q5_0.bin",
        hash: Some("19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f"),
    },
    ModelInfo {
        key: "large-q5",
        name: "Large v3 Q5 (~1.1 GB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-q5_0.bin",
        filename: "ggml-large-v3-q5_0.bin",
        hash: Some("d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1"),
    },
    ModelInfo {
        key: "turbo-q5",
        name: "Large Turbo Q5 (~547 MB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        hash: Some("394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"),
    },
    // ── Full-size multilingual ───────────────────────────────────────────
    ModelInfo {
        key: "small",
        name: "Small (~466 MB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        filename: "ggml-small.bin",
        hash: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
    },
    ModelInfo {
        key: "medium",
        name: "Medium (~1.5 GB, multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        filename: "ggml-medium.bin",
        hash: Some("6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208"),
    },
    // ── English-only (NOT suitable for Czech/Slovak) ─────────────────────
    ModelInfo {
        key: "base-en",
        name: "Base English-only (~142 MB)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        filename: "ggml-base.en.bin",
        hash: Some("a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002"),
    },
    ModelInfo {
        key: "tiny-en",
        name: "Tiny English-only (~75 MB)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        filename: "ggml-tiny.en.bin",
        hash: Some("921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"),
    },
];

/// Find a model by its public key. `None` ⇒ unknown key (typo or removed model).
pub fn lookup(key: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.key == key)
}

/// Path to where this model is (or would be) stored on disk.
pub fn path(key: &str) -> anyhow::Result<PathBuf> {
    let info = lookup(key)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", key))?;
    Ok(super::local::models_dir()?.join(info.filename))
}
