//! Type finished sentences into the focused app while the user is still
//! speaking.
//!
//! The live loop (`transcription::live`) re-transcribes the audio that
//! hasn't been typed yet. Once the leading segments end a sentence and
//! come out the same on two ticks in a row, they are committed: the audio
//! up to there is marked as done and the text goes to a worker that runs
//! the normal cleanup/translation on it (with the earlier chunks as
//! context) and types the result. When recording stops, only the audio
//! after the last commit is transcribed — see `run_pipeline`'s `prior`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::state::Settings;
use crate::transcription::local::Segment;

/// A segment must end this long before the end of the audio to count as
/// finished — the speaker may still be in the middle of it.
pub const TAIL_MARGIN_SECS: f32 = 1.0;
/// Commit even without a sentence end once this much audio is waiting
/// (keeps the live window under Whisper's 30 s).
pub const FORCE_COMMIT_SECS: f32 = 20.0;

/// How this recording's language is handled, decided once the live loop
/// has ≥2 s of audio (the spoken-language check needs that much).
#[derive(Debug, Clone, PartialEq)]
pub struct LangPlan {
    /// Language Whisper transcribes in ("auto" = detect per pass).
    pub whisper: String,
    /// Spoke one language on another's shortcut: translate into this one.
    pub translate_to: Option<String>,
}

pub struct Chunk {
    pub raw: String,
    /// Language of `raw` (for cleanup).
    pub language: String,
    pub plan: LangPlan,
}

#[derive(Debug, Default, Clone)]
pub struct Output {
    pub raw: Vec<String>,
    pub processed: Vec<String>,
    /// Some(..) per chunk when translated.
    pub translated: Vec<Option<String>>,
    /// Final text of each chunk, in order.
    pub outputs: Vec<String>,
    /// How many of `outputs` made it into the app.
    pub typed: usize,
    pub plan: Option<LangPlan>,
}

impl Output {
    pub fn untyped(&self) -> String {
        self.outputs[self.typed..].join(" ")
    }
}

pub struct StreamSession {
    /// Samples (at the capture rate) already handed to the worker.
    pub committed: AtomicUsize,
    tx: Mutex<Option<mpsc::UnboundedSender<Chunk>>>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<Chunk>>>,
    pub output: Arc<Mutex<Output>>,
    worker: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl StreamSession {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            committed: AtomicUsize::new(0),
            tx: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
            output: Arc::new(Mutex::new(Output::default())),
            worker: Mutex::new(None),
        })
    }

    /// Hand a chunk to the worker and mark its audio as done.
    pub fn commit(&self, chunk: Chunk, committed_samples: usize) {
        if let Some(tx) = self.tx.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            self.committed.store(committed_samples, Ordering::Release);
            let _ = tx.send(chunk);
        }
    }

    /// Stop accepting chunks and wait for the worker to type what it has.
    pub async fn finish(&self) -> Output {
        drop(self.tx.lock().unwrap_or_else(|e| e.into_inner()).take());
        let handle = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = handle {
            if tokio::time::timeout(std::time::Duration::from_secs(90), h).await.is_err() {
                crate::log_to_file("Stream: worker still busy after 90 s — finishing without it");
            }
        }
        self.output.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Start the worker that processes and types committed chunks in order.
    pub fn spawn_worker(self: &Arc<Self>, app: AppHandle, settings: Settings) {
        let Some(mut rx) = self.rx.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        let output = Arc::clone(&self.output);
        let handle = tauri::async_runtime::spawn(async move {
            let http = app.state::<reqwest::Client>().inner().clone();
            while let Some(chunk) = rx.recv().await {
                let started = std::time::Instant::now();
                let (earlier_source, earlier_output, first) = {
                    let o = output.lock().unwrap_or_else(|e| e.into_inner());
                    (o.processed.join(" "), o.outputs.join(" "), o.outputs.is_empty())
                };
                let (processed, translated, final_text) =
                    process_chunk(&app, &http, &settings, &chunk, earlier_source, earlier_output).await;
                if final_text.trim().is_empty() {
                    continue;
                }
                let failed_before = {
                    let mut o = output.lock().unwrap_or_else(|e| e.into_inner());
                    o.raw.push(chunk.raw.clone());
                    o.processed.push(processed);
                    o.translated.push(translated);
                    o.outputs.push(final_text.clone());
                    o.plan = Some(chunk.plan.clone());
                    o.typed < o.outputs.len() - 1
                };
                // Once typing failed, stop typing — the rest is pasted at the end
                // in one go, so the user never gets text out of order.
                if failed_before {
                    continue;
                }
                let text = if first { final_text } else { format!(" {}", final_text) };
                let typed = tokio::task::spawn_blocking(move || {
                    std::panic::catch_unwind(|| crate::paste::type_text(&text))
                })
                .await;
                match typed {
                    Ok(Ok(Ok(()))) => {
                        output.lock().unwrap_or_else(|e| e.into_inner()).typed += 1;
                        crate::log_to_file(&format!("Stream: typed chunk in {}ms", started.elapsed().as_millis()));
                    }
                    other => {
                        let why = match other {
                            Ok(Ok(Err(e))) => e.to_string(),
                            _ => "typing panicked".to_string(),
                        };
                        crate::log_to_file(&format!("Stream: typing failed ({}) — the rest is pasted at the end", why));
                    }
                }
            }
        });
        *self.worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }
}

/// Settings for one chunk/remainder: the recording's language plan applied
/// on top of the user's settings.
pub fn apply_plan(settings: &mut Settings, plan: &LangPlan, chunk_language: &str) {
    if plan.whisper != "auto" {
        settings.source_language = plan.whisper.clone();
    } else if !chunk_language.is_empty() {
        settings.source_language = chunk_language.to_string();
    }
    if let Some(target) = &plan.translate_to {
        settings.translation_enabled = true;
        settings.target_language = target.clone();
    }
}

/// Clean up / translate one chunk. Never fails: on any error the chunk's
/// raw text is used, so nothing the user said is dropped.
async fn process_chunk(
    app: &AppHandle,
    http: &reqwest::Client,
    base: &Settings,
    chunk: &Chunk,
    earlier_source: String,
    earlier_output: String,
) -> (String, Option<String>, String) {
    let mut settings = base.clone();
    apply_plan(&mut settings, &chunk.plan, &chunk.language);
    if !crate::needs_text_processing(&settings) {
        return (chunk.raw.clone(), None, chunk.raw.clone());
    }
    if settings.text_processor == crate::state::TextProcessor::BuiltIn {
        let llm = app.state::<crate::SharedLlm>().inner().clone();
        match llm.ensure_running(app, &settings.builtin_llm_model).await {
            Ok(base_url) => settings.text_processor_url = base_url,
            Err(e) => {
                crate::log_to_file(&format!("Stream: built-in model unavailable ({}) — typing the transcript", e));
                return (chunk.raw.clone(), None, chunk.raw.clone());
            }
        }
    }
    let mut speaker = crate::processing::SpeakerContext::for_settings(&settings);
    speaker.earlier_source = earlier_source;
    speaker.earlier_output = earlier_output;
    let cleanup_language = settings.source_language.clone();
    match crate::processing::process(&chunk.raw, &cleanup_language, &settings, None, http, &speaker).await {
        Ok(r) => (r.processed_text, r.translated_text, r.final_text),
        Err(e) => {
            crate::log_to_file(&format!("Stream: processing failed ({}) — typing the transcript", e));
            (chunk.raw.clone(), None, chunk.raw.clone())
        }
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn ends_sentence(s: &str) -> bool {
    s.trim_end().ends_with(['.', '?', '!', '…'])
}

/// How many leading segments of `cur` to commit. A segment qualifies when
/// it ended at least `TAIL_MARGIN_SECS` before the end of the audio and
/// the previous tick produced the same text at the same position; the
/// commit ends at the last qualifying segment that ends a sentence. With
/// `force` (too much audio waiting) a stable prefix is committed even
/// without a sentence end, keeping at least the last segment open.
pub fn segments_to_commit(prev: &[Segment], cur: &[Segment], audio_secs: f32, force: bool) -> usize {
    let mut stable = 0;
    let mut sentence_end = 0;
    for (i, seg) in cur.iter().enumerate() {
        let same = prev.get(i).is_some_and(|p| norm(&p.text) == norm(&seg.text));
        if !same || seg.end > audio_secs - TAIL_MARGIN_SECS || norm(&seg.text).is_empty() {
            break;
        }
        stable = i + 1;
        if ends_sentence(&seg.text) {
            sentence_end = i + 1;
        }
    }
    if sentence_end == 0 && force {
        stable.min(cur.len().saturating_sub(1))
    } else {
        sentence_end
    }
}

/// Where to cut the audio after committing `k` segments: between the last
/// committed segment and the next one (segment timestamps are coarse, so
/// cutting right at `end` can clip the next word's first syllable).
pub fn commit_cut_secs(cur: &[Segment], k: usize) -> f32 {
    let end = cur[k - 1].end;
    match cur.get(k) {
        Some(next) if next.start > end => (end + next.start) / 2.0,
        _ => end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f32, end: f32, text: &str) -> Segment {
        Segment { start, end, text: text.into() }
    }

    #[test]
    fn needs_two_matching_ticks() {
        let cur = vec![seg(0.0, 2.0, "Ahoj, jak se máš."), seg(2.2, 4.0, "Já dobře")];
        assert_eq!(segments_to_commit(&[], &cur, 6.0, false), 0);
        let prev = vec![seg(0.0, 2.0, "Ahoj jak se máš."), seg(2.2, 3.5, "Já")];
        assert_eq!(segments_to_commit(&prev, &cur, 6.0, false), 1);
    }

    #[test]
    fn waits_for_a_sentence_end() {
        let cur = vec![seg(0.0, 2.0, "Musíme používat"), seg(2.0, 3.0, "Acme API.")];
        assert_eq!(segments_to_commit(&cur, &cur, 6.0, false), 2);
        let open = vec![seg(0.0, 2.0, "Musíme používat"), seg(2.0, 3.0, "Acme API")];
        assert_eq!(segments_to_commit(&open, &open, 6.0, false), 0);
    }

    #[test]
    fn never_commits_the_last_second() {
        let cur = vec![seg(0.0, 2.6, "Hotovo.")];
        assert_eq!(segments_to_commit(&cur, &cur, 3.0, false), 0);
        assert_eq!(segments_to_commit(&cur, &cur, 3.7, false), 1);
    }

    #[test]
    fn force_keeps_the_last_segment_open() {
        let cur = vec![seg(0.0, 8.0, "a b c"), seg(8.0, 16.0, "d e f"), seg(16.0, 19.0, "g")];
        assert_eq!(segments_to_commit(&cur, &cur, 22.0, true), 2);
        assert_eq!(segments_to_commit(&cur, &cur, 22.0, false), 0);
    }

    #[test]
    fn cut_lands_in_the_gap() {
        let cur = vec![seg(0.0, 2.0, "Ano."), seg(2.4, 3.0, "Ne")];
        assert!((commit_cut_secs(&cur, 1) - 2.2).abs() < 1e-6);
        assert_eq!(commit_cut_secs(&cur[..1], 1), 2.0);
    }

    #[test]
    fn plan_applies_translation() {
        let mut s = Settings::default();
        apply_plan(&mut s, &LangPlan { whisper: "cs".into(), translate_to: Some("en".into()) }, "cs");
        assert_eq!(s.source_language, "cs");
        assert!(s.translation_enabled);
        assert_eq!(s.target_language, "en");
    }
}
