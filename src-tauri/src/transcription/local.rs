//! Local Whisper transcription via whisper.cpp (whisper-rs crate).
//!
//! Model metadata (URL, filename, hash) lives in `transcription::models`.
//! This module handles disk operations and inference.

use anyhow::{Context, Result};
use std::path::PathBuf;

use super::models;

pub fn models_dir() -> Result<PathBuf> {
    let dir = crate::storage::settings::typeurvoice_data_dir()?.join("models");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn model_path(model_key: &str) -> Result<PathBuf> {
    models::path(model_key)
}

pub fn is_model_downloaded(model_key: &str) -> bool {
    model_path(model_key).map(|p| p.exists()).unwrap_or(false)
}

/// Returns the file size in bytes for a downloaded model (0 if not downloaded).
pub fn model_size_bytes(model_key: &str) -> u64 {
    model_path(model_key)
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Delete a downloaded model file.
pub fn delete_model(model_key: &str) -> Result<()> {
    let path = model_path(model_key)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Download a model with progress callback, verifying the pinned SHA-256
/// before the file is moved into place.
pub async fn download_model(
    model_key: &str,
    on_progress: impl Fn(f32) + Send + 'static,
) -> Result<()> {
    let info = models::lookup(model_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_key))?;
    let dest = model_path(model_key)?;
    // whisper.cpp mmaps and executes tensor graphs from these bytes — never
    // hand it a file whose hash doesn't match.
    crate::storage::download::download_verified(info.url, &dest, info.hash, on_progress).await
}

/// Transcribe audio using a local whisper.cpp model.
/// This is CPU/GPU intensive — always run on a blocking thread.
#[cfg(feature = "local-whisper")]
pub fn transcribe_blocking(
    cache: &super::whisper_cache::WhisperCache,
    samples: &[f32],
    sample_rate: u32,
    model_key: &str,
    language: &str,
    vocabulary: Option<&str>,
) -> Result<super::TranscriptionResult> {
    let (result, _) = transcribe_segments_blocking(cache, samples, sample_rate, model_key, language, vocabulary, true)?;
    Ok(result)
}

/// One Whisper segment: text plus its span in the input audio (seconds).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// Like `transcribe_blocking`, but also returns the segments with their
/// timestamps (used by streaming to know which audio is already typed).
/// `check_language`: run the spoken-language check when a language is
/// forced (an extra encoder pass) — streaming does it once per recording.
#[cfg(feature = "local-whisper")]
pub fn transcribe_segments_blocking(
    cache: &super::whisper_cache::WhisperCache,
    samples: &[f32],
    sample_rate: u32,
    model_key: &str,
    language: &str,
    vocabulary: Option<&str>,
    check_language: bool,
) -> Result<(super::TranscriptionResult, Vec<Segment>)> {
    use whisper_rs::{FullParams, SamplingStrategy};

    let model = model_path(model_key)?;

    if !model.exists() {
        return Err(anyhow::anyhow!(
            "Model '{}' not downloaded. Go to Settings → Engine to download it.",
            model_key
        ));
    }

    // SHA-256 verify on-disk bytes against the pinned hash. ~2–3s on the 1.5 GB
    // medium model. The cache remembers (key, mtime, verified) so subsequent
    // recordings skip this — full hash still runs the first time, after a
    // re-download (mtime changes), or after `cache.invalidate()`.
    if !cache.is_verified(model_key, &model) {
        if let Err(e) = super::model_hashes::verify(model_key, &model) {
            return Err(anyhow::anyhow!(
                "{}\n\nDelete the model in Settings → Engine and re-download.",
                e
            ));
        }
    }

    let ctx = cache.get_or_load(model_key, &model)?;
    cache.mark_verified(model_key, &model);

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    params.set_suppress_blank(true);
    params.set_token_timestamps(false);
    // No context from previous chunks (we transcribe one shot at a time)
    params.set_no_context(true);
    // whisper.cpp defaults to min(4, cores). On Apple Silicon the encoder runs
    // on the GPU, but the CPU still drives sampling — use the performance
    // cores without oversubscribing efficiency cores.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    params.set_n_threads(threads as i32);

    // Resample to 16kHz if needed (whisper.cpp requires 16kHz)
    let mut samples_16k: Vec<f32> = if sample_rate == 16000 {
        samples.to_vec()
    } else {
        resample(samples, sample_rate, 16000)
    };

    let mut state = ctx.create_state().context("Failed to create Whisper state")?;

    // A requested language is a hint, not an order: if the audio is clearly
    // another language, transcribe what was spoken and let the pipeline
    // translate. Checked on the unpadded audio.
    let requested = (language != "auto" && !language.is_empty()).then_some(language);
    let spoken = requested
        .filter(|_| check_language)
        .and_then(|req| spoken_language_if_different(&mut state, &samples_16k, req, threads));
    // No language ⇒ detect. Leaving it unset would mean whisper.cpp's
    // default, "en" — Czech speech came out translated into English.
    params.set_language(Some(spoken.as_deref().or(requested).unwrap_or("auto")));
    // Names and terms from the dictionary + learned profile bias the
    // decoder towards spelling them right.
    if let Some(v) = vocabulary.filter(|v| !v.trim().is_empty()) {
        params.set_initial_prompt(v);
    }

    // Whisper requires at least 1 second of audio — pad with silence to 1.5s
    let min_samples = 24000usize; // 1.5 seconds at 16kHz
    if samples_16k.len() < min_samples {
        samples_16k.resize(min_samples, 0.0);
    }

    state.full(params, &samples_16k).context("Whisper transcription failed")?;

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    let mut segments = Vec::new();
    let mut detected_language = "en".to_string();

    for i in 0..num_segments {
        if let Some(seg) = state.get_segment(i) {
            // Lossy: a multi-byte character split across segments must not
            // drop the whole segment.
            if let Ok(seg_text) = seg.to_str_lossy() {
                text.push_str(&seg_text);
                // Timestamps are in centiseconds of the 16 kHz input.
                segments.push(Segment {
                    start: seg.start_timestamp() as f32 / 100.0,
                    end: seg.end_timestamp() as f32 / 100.0,
                    text: seg_text.trim().to_string(),
                });
            }
        }
    }

    // On silence Whisper sometimes just echoes the vocabulary prompt.
    let text = match vocabulary {
        Some(v) if super::echoes_prompt(&text, v) => {
            crate::log_to_file("Whisper echoed the vocabulary hint — treating as no speech");
            segments.clear();
            String::new()
        }
        _ => text,
    };

    let lang = state.full_lang_id_from_state();
    if lang >= 0 {
        detected_language = whisper_rs::get_lang_str(lang).unwrap_or("en").to_string();
    }

    // Hallucination filtering now happens centrally in transcription/mod.rs
    // (applied to both local and cloud paths).
    Ok((
        super::TranscriptionResult {
            text: text.trim().to_string(),
            language: detected_language,
            requested_language: spoken.and(requested.map(str::to_string)),
        },
        segments,
    ))
}

/// Run Whisper's language detection and return the spoken language when it
/// confidently differs from `requested`. Costs one extra encoder pass, so it
/// only runs when a language is forced and there are ≥2 s of audio (shorter
/// clips detect unreliably). Any failure keeps the requested language.
#[cfg(feature = "local-whisper")]
fn spoken_language_if_different(
    state: &mut whisper_rs::WhisperState,
    pcm_16k: &[f32],
    requested: &str,
    threads: usize,
) -> Option<String> {
    if pcm_16k.len() < 2 * 16_000 {
        return None;
    }
    let req_id = whisper_rs::get_lang_id(requested)?;
    if let Err(e) = state.pcm_to_mel(pcm_16k, threads) {
        crate::log_to_file(&format!("Language check skipped: {:?}", e));
        return None;
    }
    let (top_id, probs) = match state.lang_detect(0, threads) {
        Ok(v) => v,
        Err(e) => {
            crate::log_to_file(&format!("Language check skipped: {:?}", e));
            return None;
        }
    };
    let detected = whisper_rs::get_lang_str(top_id)?;
    let p_detected = probs.get(top_id as usize).copied().unwrap_or(0.0);
    let p_requested = probs.get(req_id as usize).copied().unwrap_or(0.0);
    let switch = super::should_use_spoken_language(requested, detected, p_detected, p_requested);
    crate::log_to_file(&format!(
        "Language check: requested={} p={:.2}, detected={} p={:.2}{}",
        requested, p_requested, detected, p_detected,
        if switch { " → transcribing as spoken" } else { "" }
    ));
    switch.then(|| detected.to_string())
}

/// Load + verify the model and run one tiny inference so the first real
/// recording skips model load, hash verification, and Metal kernel warm-up.
/// Blocking — call from `spawn_blocking`.
#[cfg(feature = "local-whisper")]
pub fn prewarm(cache: &super::whisper_cache::WhisperCache, model_key: &str) -> Result<()> {
    if !is_model_downloaded(model_key) {
        return Ok(());
    }
    let started = std::time::Instant::now();
    // 1 s of near-silence; the result is discarded.
    let silence = vec![0.0f32; 16_000];
    transcribe_blocking(cache, &silence, 16_000, model_key, "en", None)?;
    crate::log_to_file(&format!("Prewarmed local model {} in {:?}", model_key, started.elapsed()));
    Ok(())
}

#[cfg(not(feature = "local-whisper"))]
pub fn prewarm(_cache: &(), _model_key: &str) -> Result<()> {
    Ok(())
}

#[cfg(not(feature = "local-whisper"))]
pub fn transcribe_blocking(
    _cache: &(),
    _samples: &[f32],
    _sample_rate: u32,
    _model_key: &str,
    _language: &str,
    _vocabulary: Option<&str>,
) -> Result<super::TranscriptionResult> {
    Err(anyhow::anyhow!(
        "Local Whisper is not compiled in. Rebuild with --features local-whisper."
    ))
}

#[cfg(not(feature = "local-whisper"))]
pub fn transcribe_segments_blocking(
    _cache: &(),
    _samples: &[f32],
    _sample_rate: u32,
    _model_key: &str,
    _language: &str,
    _vocabulary: Option<&str>,
    _check_language: bool,
) -> Result<(super::TranscriptionResult, Vec<Segment>)> {
    Err(anyhow::anyhow!(
        "Local Whisper is not compiled in. Rebuild with --features local-whisper."
    ))
}

#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = (input.len() as f64 * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;
        let a = input.get(src_idx).copied().unwrap_or(0.0);
        let b = input.get(src_idx + 1).copied().unwrap_or(0.0);
        out.push(a + (b - a) * frac as f32);
    }
    out
}

/// Manual benchmark: `TYPEURVOICE_BENCH_WAV=/path/16k-mono.wav cargo test --release
/// --lib bench_local -- --ignored --nocapture`. Uses the user's downloaded
/// turbo-q5 model; prints load and inference time.
#[cfg(all(test, feature = "local-whisper"))]
mod bench {
    #[test]
    #[ignore]
    fn bench_local() {
        let path = std::env::var("TYPEURVOICE_BENCH_WAV").expect("set TYPEURVOICE_BENCH_WAV");
        let model = std::env::var("TYPEURVOICE_BENCH_MODEL").unwrap_or_else(|_| "turbo-q5".into());
        let mut reader = hound::WavReader::open(path).unwrap();
        let samples: Vec<f32> = reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        let lang = std::env::var("TYPEURVOICE_BENCH_LANG").unwrap_or_else(|_| "cs".into());
        let cache = super::super::whisper_cache::WhisperCache::new();
        for run in 0..3 {
            let t = std::time::Instant::now();
            let r = super::transcribe_blocking(&cache, &samples, 16000, &model, &lang, None).unwrap();
            println!("run {run}: {:?} → [{}, requested {:?}] {:?}", t.elapsed(), r.language, r.requested_language, r.text);
        }
    }
}
