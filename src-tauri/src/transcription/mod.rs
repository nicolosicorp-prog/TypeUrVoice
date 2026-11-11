pub mod cloud;
pub mod live;
pub mod local;
pub mod model_hashes;
pub mod models;
#[cfg(feature = "local-whisper")]
pub mod whisper_cache;

use anyhow::Result;
use crate::state::{Engine, Settings};

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: String,
    /// Set when a language was requested (shortcut or Settings) but the audio
    /// was clearly in another one. `text` is then in `language` (what was
    /// spoken) and holds the requested language, which the pipeline translates into.
    pub requested_language: Option<String>,
}

/// Whether to transcribe in the detected language instead of the one the user
/// asked for. Forcing Whisper to the wrong language makes it write a garbled
/// half-translation (Czech speech on the English shortcut came out as
/// "I wanted to use the matici, but also the same."), so a confident mismatch
/// is transcribed as spoken and translated afterwards.
///
/// Conservative on purpose: short Czech clips are sometimes misdetected as
/// English (see Issue #5), so the detected language must be confident and the
/// requested one near-zero. Close pairs (cs/sk) are never switched — Whisper
/// confuses them and a forced hint there is what the user wants.
pub(crate) fn should_use_spoken_language(
    requested: &str,
    detected: &str,
    p_detected: f32,
    p_requested: f32,
) -> bool {
    const CLOSE_PAIRS: &[(&str, &str)] = &[("cs", "sk")];
    let close = CLOSE_PAIRS
        .iter()
        .any(|&(a, b)| (requested == a && detected == b) || (requested == b && detected == a));
    detected != requested && !close && p_detected >= 0.70 && p_requested <= 0.10
}

/// Whisper's vocabulary hint: dictionary terms plus terms learned from the
/// speaker's history. `None` when there are none.
pub fn vocabulary_terms(settings: &Settings) -> Vec<String> {
    let mut terms: Vec<String> = settings.dictionary.iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if settings.learn_from_history {
        for t in crate::storage::speaker::load().terms {
            if !terms.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                terms.push(t);
            }
        }
    }
    terms.truncate(40);
    terms
}

pub fn vocabulary_prompt(settings: &Settings) -> Option<String> {
    let terms = vocabulary_terms(settings);
    (!terms.is_empty()).then(|| format!("{}.", terms.join(", ")))
}

/// The transcript is nothing but (part of) the vocabulary prompt — what
/// Whisper produces on silence when it has a prompt.
pub(crate) fn echoes_prompt(text: &str, prompt: &str) -> bool {
    let norm = |s: &str| -> String {
        s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>()
            .split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let (t, p) = (norm(text), norm(prompt));
    !t.is_empty() && p.contains(&t)
}

// ─── Hallucination filter (shared by cloud + local) ─────────────────────────
// Whisper (both local GGML and OpenAI's whisper-1) hallucinates these patterns
// on silence, noise, or quiet audio. They share the same training corpus, so
// they share the same failure modes — filter both paths identically.

const HALLUCINATION_EXACT: &[&str] = &[
    "[music]",
    "(music)",
    "[ music ]",
    "[silence]",
    "(silence)",
    "[ silence ]",
    "[noise]",
    "(noise)",
    "[ noise ]",
    "[applause]",
    "(applause)",
    "[laughing]",
    "(laughing)",
    "[singing]",
    "(singing)",
    "[background noise]",
    "(background noise)",
    "[blank_audio]",
    "(blank_audio)",
    "[speaking in foreign language]",
    "(speaking in foreign language)",
    "(foreign language)",
    "(speaking foreign language)",
    ".",
    "...",
    "thank you.",
    "thanks for watching!",
    "bye.",
];

const HALLUCINATION_SUBSTR: &[&str] = &[
    "titulky vytvořil",
    "překlad titulků",
    "subtitles by",
    "www.amara.org",
    "www.zeoranger.co.uk",
    "hradeckesluzby",
    "♪",
];

/// Detect whether a string is *only* a URL / domain (Whisper's bare-URL fallback).
/// Real speech contains spaces; URLs don't. Length-bounded to avoid stripping
/// legitimate sentences that happen to contain a dot.
fn looks_like_url_only(s: &str) -> bool {
    let trimmed = s.trim().trim_end_matches('.');
    if trimmed.is_empty() || trimmed.contains(' ') {
        return false;
    }
    let len = trimmed.chars().count();
    if !(5..=80).contains(&len) {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.") {
        return true;
    }
    // At least two alphanumeric dot-separated labels (e.g. example.com, sub.example.co.uk)
    let parts: Vec<&str> = trimmed.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return true;
    }
    false
}

pub fn filter_hallucinations(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_lowercase();

    // 1. Full-output exact match → drop entirely
    for pattern in HALLUCINATION_EXACT {
        if lower == *pattern {
            eprintln!("[TypeUrVoice] Hallucination filtered (exact): {:?}", text);
            return String::new();
        }
    }

    // 2. Subtitle-credit substrings → drop entirely when they dominate the output
    for pattern in HALLUCINATION_SUBSTR {
        if lower.contains(pattern) {
            eprintln!("[TypeUrVoice] Hallucination filtered (substring {:?}): {:?}", pattern, text);
            return String::new();
        }
    }

    // 3. Bare URL output → drop entirely
    if looks_like_url_only(trimmed) {
        eprintln!("[TypeUrVoice] Hallucination filtered (url-only): {:?}", text);
        return String::new();
    }

    // 4. Strip bracketed ASR tokens that appear inside otherwise-real speech
    let mut result = text.to_string();
    for pattern in HALLUCINATION_EXACT {
        // Skip bare-punctuation patterns ('.', '...') — substring-stripping them
        // would corrupt real sentences.
        if pattern.chars().all(|c| c == '.') {
            continue;
        }
        let upper = pattern.to_uppercase();
        let title: String = pattern
            .split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        result = result.replace(pattern, "").replace(&upper, "").replace(&title, "");
    }

    result.trim().to_string()
}

pub async fn transcribe(
    audio_wav: Vec<u8>,
    samples: &[f32],
    sample_rate: u32,
    settings: &Settings,
    api_key: Option<String>,
    http: &reqwest::Client,
    #[cfg(feature = "local-whisper")] cache: std::sync::Arc<whisper_cache::WhisperCache>,
    #[cfg(not(feature = "local-whisper"))] _cache: std::sync::Arc<()>,
) -> Result<TranscriptionResult> {
    let mut result = match settings.engine {
        Engine::Cloud => {
            let key = api_key.ok_or_else(|| {
                let vendor = match settings.cloud_transcription_provider {
                    crate::state::CloudTranscriptionProvider::OpenAi => "OpenAI",
                    crate::state::CloudTranscriptionProvider::Groq   => "Groq",
                };
                anyhow::anyhow!("No API key configured. Add your {} key in Settings → Engine.", vendor)
            })?;
            cloud::transcribe(
                audio_wav,
                &key,
                &settings.source_language,
                &vocabulary_terms(settings),
                &settings.cloud_transcription_provider,
                settings.cloud_transcription_model.as_deref(),
                http,
            ).await
        }
        Engine::Local => {
            let model_key = settings.local_model.key().to_string();
            let language = settings.source_language.clone();
            let vocabulary = vocabulary_prompt(settings);
            let samples_owned = samples.to_vec();
            #[cfg(feature = "local-whisper")]
            let cache_for_task = cache.clone();
            #[cfg(not(feature = "local-whisper"))]
            let cache_for_task: std::sync::Arc<()> = std::sync::Arc::new(());

            // Run on blocking thread (CPU-intensive).
            // Wrap in catch_unwind because whisper.cpp (FFI) can panic/crash.
            // Add a 120-second timeout to prevent whisper.cpp from running forever.
            let transcription_task = tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    local::transcribe_blocking(&*cache_for_task, &samples_owned, sample_rate, &model_key, &language, vocabulary.as_deref())
                }))
            });

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                transcription_task,
            ).await;

            match result {
                Ok(Ok(Ok(inner))) => inner,
                Ok(Ok(Err(_panic))) => {
                    eprintln!("[TypeUrVoice] Whisper transcription panicked!");
                    Err(anyhow::anyhow!("Local transcription crashed. Try a different model or use Cloud engine."))
                }
                Ok(Err(e)) => {
                    eprintln!("[TypeUrVoice] Whisper task join error: {}", e);
                    Err(anyhow::anyhow!("Transcription task failed: {}", e))
                }
                Err(_) => {
                    eprintln!("[TypeUrVoice] Whisper transcription timed out after 120s!");
                    Err(anyhow::anyhow!("Transcription timed out. Try a shorter recording or a smaller model."))
                }
            }
        }
    }?;

    // Log the raw Whisper response BEFORE filtering so we can tell the
    // difference between "Whisper returned nothing" and "filter wiped it".
    let preview = if settings.diagnostics_verbose {
        format!(", text={:?}", crate::logging::redact_preview(&result.text, 120))
    } else {
        String::new()
    };
    crate::log_to_file(&format!(
        "Whisper raw: lang={}, {} chars{}",
        result.language, result.text.len(), preview
    ));

    // Same Whisper model family in both paths ⇒ same training-data artifacts.
    // Filter cloud and local identically so hallucinated URLs / subtitle credits
    // don't leak through one engine.
    let before_filter_len = result.text.len();
    result.text = filter_hallucinations(&result.text);
    if result.text.len() != before_filter_len {
        crate::log_to_file(&format!(
            "Hallucination filter: {} → {} chars",
            before_filter_len, result.text.len()
        ));
    }

    if settings.remove_fillers {
        let before = result.text.len();
        result.text = crate::processing::fillers::remove_fillers(&result.text);
        if result.text.len() != before {
            crate::log_to_file(&format!("Filler filter: {} → {} chars", before, result.text.len()));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod vocabulary_tests {
    #[test]
    fn echo_detection() {
        assert!(super::echoes_prompt("Acme API, Tauri.", "Acme API, Tauri, TypeUrVoice."));
        assert!(!super::echoes_prompt("Musíme používat Acme API.", "Acme API, Tauri."));
        assert!(!super::echoes_prompt("", "Acme"));
    }
}

#[cfg(test)]
mod language_switch_tests {
    use super::should_use_spoken_language as switch;

    #[test]
    fn czech_on_english_shortcut_switches() {
        assert!(switch("en", "cs", 0.93, 0.01));
    }

    #[test]
    fn matching_language_never_switches() {
        assert!(!switch("cs", "cs", 0.99, 0.99));
    }

    #[test]
    fn unsure_detection_keeps_the_hint() {
        // Short/quiet Czech clips misdetected as English (Issue #5).
        assert!(!switch("cs", "en", 0.55, 0.30));
        assert!(!switch("cs", "en", 0.80, 0.15));
    }

    #[test]
    fn close_pair_keeps_the_hint() {
        assert!(!switch("cs", "sk", 0.95, 0.02));
        assert!(!switch("sk", "cs", 0.95, 0.02));
    }
}
