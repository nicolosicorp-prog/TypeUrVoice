//! Learn a speaker profile from History: recurring topics, names/terms,
//! words the recognizer keeps getting wrong for this voice (with the
//! pronunciation pattern behind them, e.g. ř heard as ž), and phrasing
//! style. Runs on the configured text processor — automatically only when
//! that is on-device (built-in, Ollama, LM Studio), on request otherwise.

use anyhow::Result;

use crate::state::Settings;
use crate::storage::history::{Correction, HistoryEntry};
use crate::storage::speaker::SpeakerProfile;

/// Dictation text sent to the model, newest kept when over budget. Sized
/// for the built-in model's 4096-token context (Czech ≈ 3 chars/token).
const MAX_INPUT_CHARS: usize = 5000;
const MAX_ENTRY_CHARS: usize = 600;
/// Learn after this many new dictations (and once there are this many).
pub const LEARN_EVERY: i64 = 8;

const SYSTEM: &str = "You keep a short profile of one person who dictates text with speech recognition. \
The profile helps later steps fix recognition mistakes for this voice and translate in the speaker's own style.\n\
\n\
You get: the current profile (may be empty), the speaker's own note about how they speak, corrections they made, \
and recent dictations. Each dictation shows RECOGNIZED (raw speech recognition) and FINAL (after cleanup or the user's fix).\n\
\n\
Write the updated profile in English with exactly these four lines, each at most 400 characters:\n\
TOPICS: what they usually talk about (projects, products, people).\n\
TERMS: comma-separated names and technical terms, spelled correctly.\n\
RECOGNITION: words recognized wrongly for this speaker as \"heard → meant\" pairs, plus the pronunciation pattern behind them if there is one (for example: ř is recognized as ž or z). Write \"none seen\" if there are none.\n\
STYLE: how they phrase things (tone, directness, typical expressions) and how an English translation should sound to match them.\n\
\n\
Only state what the dictations show. Keep still-valid points from the current profile. No other text.";

/// Build the model input. Pure — unit-tested.
pub fn build_input(
    prev: &SpeakerProfile,
    notes: &str,
    entries: &[HistoryEntry],
    corrections: &[Correction],
) -> String {
    let mut out = String::new();
    out.push_str("CURRENT PROFILE:\n");
    if prev.is_empty() {
        out.push_str("(empty)\n");
    } else {
        out.push_str(&format!(
            "TOPICS: {}\nTERMS: {}\nRECOGNITION: {}\nSTYLE: {}\n",
            prev.topics, prev.terms.join(", "), prev.recognition, prev.style
        ));
    }
    out.push_str(&format!(
        "\nSPEAKER'S NOTE:\n{}\n",
        if notes.trim().is_empty() { "(none)" } else { notes.trim() }
    ));
    if !corrections.is_empty() {
        out.push_str("\nCORRECTIONS THE SPEAKER MADE:\n");
        for c in corrections.iter().take(6) {
            out.push_str(&format!(
                "- {} output \"{}\" → corrected to \"{}\"\n",
                c.kind,
                cap(&c.output_text, 250),
                cap(&c.corrected_text, 250)
            ));
        }
    }
    // Newest dictations first until the budget is used, then print oldest-first.
    let mut picked: Vec<String> = Vec::new();
    let mut used = out.chars().count();
    for e in entries.iter().rev() {
        // processed_text is in the spoken language (a History fix to a
        // cleanup lands there too); translations live in the corrections.
        // Markdown mode restructures (and may translate) the dictation, so
        // its output says nothing about recognition — show the transcript.
        let final_text = if e.mode == "markdown" { &e.raw_text } else { &e.processed_text };
        let block = format!(
            "RECOGNIZED: {}\nFINAL: {}\n",
            cap(&e.raw_text, MAX_ENTRY_CHARS),
            cap(final_text, MAX_ENTRY_CHARS)
        );
        let len = block.chars().count();
        if used + len > MAX_INPUT_CHARS {
            break;
        }
        used += len;
        picked.push(block);
    }
    picked.reverse();
    out.push_str("\nRECENT DICTATIONS:\n");
    for (i, b) in picked.iter().enumerate() {
        out.push_str(&format!("#{}\n{}", i + 1, b));
    }
    out
}

/// Parse the model's four lines. `None` when the answer doesn't look like
/// a profile (missing sections, preamble only) — the old profile is kept.
pub fn parse(answer: &str) -> Option<SpeakerProfile> {
    let mut p = SpeakerProfile::default();
    let mut seen = 0;
    for line in answer.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '#', ' ']).replace("**", "");
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim().to_string();
        match key.trim().to_ascii_uppercase().as_str() {
            "TOPICS" => { p.topics = cap(&value, 400); seen += 1; }
            "TERMS" => {
                p.terms = value
                    .split(',')
                    .map(|t| t.trim().trim_matches(['"', '.', '\'']).to_string())
                    .filter(|t| !t.is_empty() && t.chars().count() <= 40)
                    .take(30)
                    .collect();
                seen += 1;
            }
            "RECOGNITION" => {
                let v = value.to_lowercase();
                p.recognition = if v.starts_with("none") { String::new() } else { cap(&value, 400) };
                seen += 1;
            }
            "STYLE" => { p.style = cap(&value, 400); seen += 1; }
            _ => {}
        }
    }
    (seen >= 3 && !p.is_empty()).then_some(p)
}

fn cap(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max { s.to_string() } else { s.chars().take(max).collect() }
}

/// Run one learning pass. `settings.text_processor_url` must already point
/// at a running server for local processors (the caller handles the
/// built-in server, as `run_pipeline` does).
pub async fn learn(
    http: &reqwest::Client,
    settings: &Settings,
    prev: &SpeakerProfile,
    entries: &[HistoryEntry],
    corrections: &[Correction],
) -> Result<SpeakerProfile> {
    if entries.is_empty() {
        return Err(anyhow::anyhow!("No dictations in History yet"));
    }
    let cfg = super::build_gpt_config(settings, None)
        .with_temperature(0.2)
        .with_max_tokens(Some(700));
    let input = build_input(prev, &settings.speaker_notes, entries, corrections);
    let answer = super::gpt_complete(http, &cfg, SYSTEM, &input).await?;
    let mut profile = parse(&answer).ok_or_else(|| {
        anyhow::anyhow!("The model's answer wasn't a profile ({} chars)", answer.chars().count())
    })?;
    profile.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    profile.last_history_id = entries.iter().map(|e| e.id).max().unwrap_or(prev.last_history_id);
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, raw: &str, processed: &str) -> HistoryEntry {
        HistoryEntry {
            id, raw_text: raw.into(), processed_text: processed.into(), mode: "clean".into(),
            language: Some("cs".into()), translated_text: None, summary: None,
            duration_secs: None, created_at: String::new(), corrected: false,
        }
    }

    #[test]
    fn parses_four_lines_with_markdown_noise() {
        let p = parse(
            "**TOPICS:** a game for kids, an accounting integration\n\
             - TERMS: Acme API, TypeUrVoice, \"Tauri\".\n\
             RECOGNITION: pozádku → pořádku; ř is recognized as ž\n\
             STYLE: direct, informal",
        ).unwrap();
        assert_eq!(p.terms, vec!["Acme API", "TypeUrVoice", "Tauri"]);
        assert!(p.recognition.contains("pořádku"));
        assert_eq!(p.style, "direct, informal");
    }

    #[test]
    fn none_seen_means_empty_recognition() {
        let p = parse("TOPICS: x\nTERMS: y\nRECOGNITION: none seen\nSTYLE: z").unwrap();
        assert!(p.recognition.is_empty());
    }

    #[test]
    fn rejects_non_profile() {
        assert!(parse("Sure! Here is the profile you asked for.").is_none());
    }

    #[test]
    fn input_respects_budget_and_keeps_newest() {
        let long = "slovo ".repeat(100);
        let entries: Vec<_> = (1..=40).map(|i| entry(i, &format!("{i} {long}"), "x")).collect();
        let input = build_input(&SpeakerProfile::default(), "", &entries, &[]);
        assert!(input.chars().count() <= MAX_INPUT_CHARS + 200);
        assert!(input.contains("RECOGNIZED: 40 "));
        assert!(!input.contains("RECOGNIZED: 1 "));
    }
}

/// Learn a profile from the real History with a running llama-server and
/// print it (nothing is saved):
///   TYPEURVOICE_LLM_URL=http://127.0.0.1:8090 TYPEURVOICE_HISTORY_DB=…/history.db \
///   [TYPEURVOICE_SPEAKER_NOTE=…] cargo test --release --lib learn_history -- --ignored --nocapture
#[cfg(test)]
mod history_replay {
    #[test]
    #[ignore]
    fn learn_history() {
        let url = std::env::var("TYPEURVOICE_LLM_URL").expect("set TYPEURVOICE_LLM_URL");
        let db = std::env::var("TYPEURVOICE_HISTORY_DB").expect("set TYPEURVOICE_HISTORY_DB");
        let mut settings = crate::state::Settings::default();
        settings.text_processor = crate::state::TextProcessor::BuiltIn;
        settings.text_processor_url = url;
        settings.speaker_notes = std::env::var("TYPEURVOICE_SPEAKER_NOTE").unwrap_or_default();
        let conn = rusqlite::Connection::open(db).unwrap();
        let rows: Vec<crate::storage::history::HistoryEntry> = conn
            .prepare("SELECT id, raw_text, processed_text, mode, language, translated_text FROM transcriptions ORDER BY id DESC LIMIT 40")
            .unwrap()
            .query_map([], |r| Ok(crate::storage::history::HistoryEntry {
                id: r.get(0)?, raw_text: r.get(1)?, processed_text: r.get(2)?, mode: r.get(3)?,
                language: r.get(4)?, translated_text: r.get(5)?, summary: None,
                duration_secs: None, created_at: String::new(), corrected: false,
            }))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter().rev().collect();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let t = std::time::Instant::now();
        let p = rt.block_on(super::learn(&reqwest::Client::new(), &settings, &Default::default(), &rows, &[]));
        println!("{:?}\n{:#?}", t.elapsed(), p);
    }
}
