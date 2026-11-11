//! Live preview: re-transcribe the growing recording buffer while the user
//! is still speaking and stream partial text to the overlay bubble.
//!
//! Local engine only. Each tick snapshots the audio (the last `WINDOW_SECS`,
//! or — when streaming — everything not typed yet) and runs the cached
//! Whisper context on it (a fresh `WhisperState` per run, so it never
//! touches the final pass's state). Ticks never overlap — when inference is
//! slower than `TICK`, ticks are simply skipped.
//!
//! With `stream` set, finished sentences are committed to the typing
//! worker (see `crate::stream`). Otherwise the pasted text always comes
//! from the full-audio pass after recording stops.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::stream::{self, LangPlan, StreamSession};

const TICK: Duration = Duration::from_millis(450);
const WINDOW_SECS: usize = 30;
/// Don't bother before there's enough audio for Whisper to say something.
/// Kept low so even a 2 s hold-to-talk clip shows text before release.
const MIN_AUDIO_SECS: f32 = 0.5;
/// The spoken-language check needs this much audio to be reliable.
const LANG_CHECK_SECS: f32 = 2.0;

pub struct Session {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub stop_flag: Arc<AtomicBool>,
    pub busy: Arc<AtomicBool>,
    pub model_key: String,
    pub language: String,
    pub remove_fillers: bool,
    pub vocabulary: Option<String>,
    /// Show partial text in the bubble.
    pub show_preview: bool,
    /// Type finished sentences while speaking.
    pub stream: Option<Arc<StreamSession>>,
}

#[cfg(feature = "local-whisper")]
pub fn spawn(app: AppHandle, cache: Arc<super::whisper_cache::WhisperCache>, session: Session) {
    tauri::async_runtime::spawn(async move {
        let forced = session.language != "auto" && !session.language.is_empty();
        // Decided once: forced languages after the first ≥2 s pass (which runs
        // the spoken-language check), "auto" right away.
        let mut plan: Option<LangPlan> = (!forced).then(|| LangPlan {
            whisper: "auto".into(),
            translate_to: None,
        });
        let mut last_len = 0usize;
        let mut last_text = String::new();
        let mut prev_segments: Vec<super::local::Segment> = Vec::new();
        let mut updates = 0u32;
        let mut commits = 0u32;
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
            let committed = session.stream.as_ref().map_or(0, |s| s.committed.load(Ordering::Acquire));
            let (snapshot, base, total_len) = {
                let buf = session.samples.lock().unwrap_or_else(|e| e.into_inner());
                let start = committed.max(buf.len().saturating_sub(rate as usize * WINDOW_SECS));
                (buf[start.min(buf.len())..].to_vec(), start, buf.len())
            };
            let secs = snapshot.len() as f32 / rate as f32;
            if secs < MIN_AUDIO_SECS || total_len == last_len {
                session.busy.store(false, Ordering::Release);
                continue;
            }
            last_len = total_len;

            let check_language = plan.is_none() && secs >= LANG_CHECK_SECS;
            let whisper_lang = plan.as_ref().map_or(session.language.clone(), |p| p.whisper.clone());
            let cache = cache.clone();
            let model_key = session.model_key.clone();
            let vocabulary = session.vocabulary.clone();
            let result = tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::local::transcribe_segments_blocking(
                        &cache, &snapshot, rate, &model_key, &whisper_lang, vocabulary.as_deref(), check_language,
                    )
                }))
            })
            .await;
            session.busy.store(false, Ordering::Release);

            if session.stop_flag.load(Ordering::Acquire) {
                break; // recording ended mid-inference — the final pass takes over
            }

            let (result, segments) = match result {
                Ok(Ok(Ok(r))) => r,
                Ok(Ok(Err(e))) => {
                    crate::log_to_file(&format!("Live preview stopped: {}", e));
                    break;
                }
                _ => {
                    crate::log_to_file("Live preview inference panicked — disabling for this recording");
                    break;
                }
            };
            if check_language {
                plan = Some(match &result.requested_language {
                    Some(requested) => LangPlan {
                        whisper: result.language.clone(),
                        translate_to: Some(requested.clone()),
                    },
                    None => LangPlan { whisper: session.language.clone(), translate_to: None },
                });
                prev_segments.clear(); // that pass may have used another language
            }

            // ── Streaming: commit finished sentences ──
            if let (Some(st), Some(plan)) = (&session.stream, &plan) {
                // Only when the snapshot starts exactly at the committed point —
                // otherwise audio between them would be skipped.
                let k = if base == committed {
                    stream::segments_to_commit(&prev_segments, &segments, secs, secs >= stream::FORCE_COMMIT_SECS)
                } else {
                    0
                };
                if k > 0 {
                    let raw = segments[..k].iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                    let mut text = super::filter_hallucinations(&raw);
                    if session.remove_fillers {
                        text = crate::processing::fillers::remove_fillers(&text);
                    }
                    let cut = (stream::commit_cut_secs(&segments, k) * rate as f32) as usize;
                    let new_committed = (committed + cut).min(total_len);
                    if !text.trim().is_empty() {
                        st.commit(
                            stream::Chunk { raw: text, language: result.language.clone(), plan: plan.clone() },
                            new_committed,
                        );
                        commits += 1;
                    } else {
                        st.committed.store(new_committed, Ordering::Release);
                    }
                    prev_segments.clear();
                    last_len = 0; // re-run on the remainder next tick
                    continue;
                }
                prev_segments = segments;
            }

            if !session.show_preview {
                continue;
            }
            let mut text = super::filter_hallucinations(&result.text);
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
        if session.stream.is_some() {
            crate::log_to_file(&format!("Live preview: {} update(s), {} chunk(s) committed for typing", updates, commits));
        } else {
            crate::log_to_file(&format!("Live preview: {} update(s)", updates));
        }
    });
}

#[cfg(not(feature = "local-whisper"))]
pub fn spawn(_app: AppHandle, _cache: Arc<()>, _session: Session) {}
