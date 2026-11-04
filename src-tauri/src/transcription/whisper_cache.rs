//! Caches the loaded `WhisperContext` across recordings.
//!
//! Before this cache, every transcription rebuilt the context from disk —
//! 2–5s of mmap + tensor setup per recording on bigger models. The cache
//! keeps the Arc alive across calls and invalidates when the model key
//! changes (user switched model) or when the underlying file was replaced.
//!
//! Thread model: the cache lives behind a std::Mutex. All access comes from
//! the tokio blocking pool (see transcription/mod.rs::Engine::Local arm),
//! which is not the async reactor — holding the lock briefly is fine.

#![cfg(feature = "local-whisper")]

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use whisper_rs::{WhisperContext, WhisperContextParameters};

struct Entry {
    model_key: String,
    model_path: PathBuf,
    // mtime at load time — if the file is re-downloaded, mtime changes
    // and we drop the cached context.
    mtime: Option<SystemTime>,
    ctx: Arc<WhisperContext>,
    /// `true` once we've successfully SHA-256-verified the file at the
    /// current `mtime`. Lets the transcribe path skip the 2–3s rehash on
    /// every recording — full hash still runs whenever the file changes.
    hash_verified: bool,
}

#[derive(Default)]
pub struct WhisperCache {
    inner: Mutex<Option<Entry>>,
}

impl WhisperCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a cached `Arc<WhisperContext>` for the given model, loading
    /// it from disk if the cache is empty or stale.
    pub fn get_or_load(
        &self,
        model_key: &str,
        model_path: &std::path::Path,
    ) -> Result<Arc<WhisperContext>> {
        let current_mtime = std::fs::metadata(model_path).ok().and_then(|m| m.modified().ok());

        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(e) = guard.as_ref() {
                let same_key = e.model_key == model_key;
                let same_path = e.model_path == model_path;
                let same_mtime = e.mtime == current_mtime;
                if same_key && same_path && same_mtime {
                    return Ok(Arc::clone(&e.ctx));
                }
            }
        }

        // Slow path: load. Do NOT hold the mutex across this call.
        let model_str = model_path.to_string_lossy();
        // GPU (Metal) is on by default when whisper-rs is built with the
        // `metal` feature. Flash attention cuts decoder time further on the
        // GPU path; we don't use DTW token timestamps, so nothing is lost.
        let mut params = WhisperContextParameters::default();
        params.flash_attn(cfg!(target_os = "macos"));
        let ctx = WhisperContext::new_with_params(model_str.as_ref(), params)
        .context("Failed to load Whisper model")?;
        let ctx = Arc::new(ctx);

        {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(Entry {
                model_key: model_key.to_string(),
                model_path: model_path.to_path_buf(),
                mtime: current_mtime,
                ctx: Arc::clone(&ctx),
                hash_verified: false,
            });
        }
        Ok(ctx)
    }

    /// Has the file at this path already been SHA-256-verified at its
    /// current mtime? Used by the transcribe path to skip the 2–3s rehash
    /// when nothing has changed since the last successful verification.
    pub fn is_verified(&self, model_key: &str, model_path: &std::path::Path) -> bool {
        let current_mtime = std::fs::metadata(model_path).ok().and_then(|m| m.modified().ok());
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(e) => {
                e.model_key == model_key
                    && e.model_path == model_path
                    && e.mtime == current_mtime
                    && e.hash_verified
            }
            None => false,
        }
    }

    /// Mark the currently-cached entry as hash-verified at its current mtime.
    /// Caller is responsible for having actually run `model_hashes::verify`.
    pub fn mark_verified(&self, model_key: &str, model_path: &std::path::Path) {
        let current_mtime = std::fs::metadata(model_path).ok().and_then(|m| m.modified().ok());
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = guard.as_mut() {
            if e.model_key == model_key
                && e.model_path == model_path
                && e.mtime == current_mtime
            {
                e.hash_verified = true;
            }
        }
    }

    /// Drop the cached context (called when the user deletes a model).
    pub fn invalidate(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }
}
