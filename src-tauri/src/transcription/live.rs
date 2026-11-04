//! Live preview: re-transcribe the growing recording buffer while the user
//! is still speaking and stream partial text to the overlay bubble.
//!
//! Local engine only. Each tick snapshots the last `WINDOW_SECS` of audio
//! and runs the cached Whisper context on it (a fresh `WhisperState` per
//! run, so it never touches the final pass's state). Ticks never overlap —
//! when inference is slower than `TICK`, ticks are simply skipped. The final
//! pasted text always comes from the full-audio pass after recording stops.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

const TICK: Duration = Duration::from_millis(450);
const WINDOW_SECS: usize = 30;
/// Don't bother before there's enough audio for Whisper to say something.
/// Kept low so even a 2 s hold-to-talk clip shows text before release.
const MIN_AUDIO_SECS: f32 = 0.5;

pub struct Session {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub stop_flag: Arc<AtomicBool>,
    pub busy: Arc<AtomicBool>,
    pub model_key: String,
    pub language: String,
    pub remove_fillers: bool,
}

#[cfg(feature = "local-whisper")]
pub fn spawn(app: AppHandle, cache: Arc<super::whisper_cache::WhisperCache>, session: Session) {
    tauri::async_runtime::spawn(async move {
        let mut last_len = 0usize;
        let mut last_text = String::new();
        let mut updates = 0u32;
        loop {
            tokio::time::sleep(TICK).await;

            // Claim the busy flag BEFORE re-checking stop, so the stopping side
            // either sees busy=true and waits, or we see stop and bail.
            session.busy.store(true, Ordering::Release);
            if session.stop_flag.load(Ordering::Acquire) {
                session.busy.store(false, Ordering::Release);
                break;
            }

            let rate = *session.sample_rate.lock().unwrap_or_else(|e| e.into_inner());
            let snapshot: Vec<f32> = {
                let buf = session.samples.lock().unwrap_or_else(|e| e.into_inner());
                let window = rate as usize * WINDOW_SECS;
                buf[buf.len().saturating_sub(window)..].to_vec()
            };
            let total_len = snapshot.len();
            if (total_len as f32) < rate as f32 * MIN_AUDIO_SECS || total_len == last_len {
                session.busy.store(false, Ordering::Release);
                continue;
            }
            last_len = total_len;

            let cache = cache.clone();
            let model_key = session.model_key.clone();
            let language = session.language.clone();
            let result = tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::local::transcribe_blocking(&cache, &snapshot, rate, &model_key, &language)
                }))
            })
            .await;
            session.busy.store(false, Ordering::Release);

            if session.stop_flag.load(Ordering::Acquire) {
                break; // recording ended mid-inference — the final pass takes over
            }

            let text = match result {
                Ok(Ok(Ok(r))) => r.text,
                Ok(Ok(Err(e))) => {
                    crate::log_to_file(&format!("Live preview stopped: {}", e));
                    break;
                }
                _ => {
                    crate::log_to_file("Live preview inference panicked — disabling for this recording");
                    break;
                }
            };
            let mut text = super::filter_hallucinations(&text);
            if session.remove_fillers {
                text = crate::processing::fillers::remove_fillers(&text);
            }
            if !text.is_empty() && text != last_text {
                crate::show_live_overlay(&app);
                let _ = app.emit("partial-transcript", &text);
                last_text = text;
                updates += 1;
            }
        }
        crate::log_to_file(&format!("Live preview: {} update(s)", updates));
    });
}

#[cfg(not(feature = "local-whisper"))]
pub fn spawn(_app: AppHandle, _cache: Arc<()>, _session: Session) {}
