use anyhow::Result;

/// Translate dictated speech. `source_code` is the ISO code (for
/// language-specific notes), `source` / `target` are display names.
/// `speaker_hint` carries the speaker's note, learned profile, earlier
/// corrections and — when streaming — the earlier part of this dictation.
pub async fn translate(
    http: &reqwest::Client,
    text: &str,
    source_code: &str,
    source: &str,
    target: &str,
    speaker_hint: Option<&str>,
    cfg: &super::GptConfig,
) -> Result<String> {
    let system = build_prompt(source_code, source, target, speaker_hint);
    super::gpt_complete(http, cfg, &system, text).await
}

fn build_prompt(source_code: &str, source: &str, target: &str, speaker_hint: Option<&str>) -> String {
    let mut p = format!(
        "You translate dictated speech from {source} to {target}.\n\
        \n\
        The text comes from speech recognition, so some words may be misheard: a word that is not a real {source} word, or makes no sense in the sentence, is a mishearing — translate the word the speaker most likely said (it sounds almost the same).\n\
        \n\
        Rules:\n\
        - Keep the exact meaning. Never turn a statement into its opposite; never add or drop a negation.\n\
        - Questions stay questions; suggestions stay suggestions.\n\
        - Sound like the speaker: same tone, directness and register, in natural {target} — not word for word.\n\
        - Keep names, product names and technical terms as they are.\n\
        - Preserve formatting: bullet points, line breaks, paragraphs.\n\
        - Do NOT add notes, comments, or explanations.\n\
        - Return ONLY the translation, nothing else."
    );
    if let Some(notes) = language_notes(source_code) {
        p.push_str("\n\n");
        p.push_str(notes);
    }
    if let Some(hint) = speaker_hint.filter(|h| !h.trim().is_empty()) {
        p.push_str("\n\n");
        p.push_str(hint);
    }
    p
}

/// Constructions small models reliably get wrong for a source language.
fn language_notes(source_code: &str) -> Option<&'static str> {
    match source_code {
        "cs" | "sk" => Some(
            "Czech/Slovak specifics:\n\
            - A negative question is usually a polite suggestion, not a negative statement: \
            \"Nebylo by lepší to udělat jinak?\" = \"Wouldn't it be better to do it differently?\"; \
            \"Neměli bychom…?\" = \"Shouldn't we…?\"; \"Nešlo by…?\" = \"Could we…?\"; \
            \"jestli by nebylo lepší…\" = \"whether it might be better to…\".\n\
            - Double negation is a single negation: \"Nikdo nic neřekl\" = \"Nobody said anything\".\n\
            - Recognition often writes ž or z where the speaker said ř, and drops or swaps accents \
            (\"pozádku\" = \"pořádku\", \"heždčí\" = \"hezčí\").",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn czech_prompt_has_negative_question_note() {
        let p = build_prompt("cs", "Czech", "English", None);
        assert!(p.contains("Wouldn't it be better"));
        assert!(!build_prompt("de", "German", "English", None).contains("Czech"));
    }

    #[test]
    fn speaker_hint_is_appended() {
        let p = build_prompt("cs", "Czech", "English", Some("About the speaker: X"));
        assert!(p.ends_with("About the speaker: X"));
    }
}

/// Re-translate the user's translated History entries with the current
/// prompt and print old vs new side by side:
///   TYPEURVOICE_LLM_URL=http://127.0.0.1:8090 TYPEURVOICE_HISTORY_DB=…/history.db \
///   cargo test --release --lib translate_history -- --ignored --nocapture
/// (start llama-server with the model first). TYPEURVOICE_SPEAKER_NOTE
/// adds a speaker note to every prompt.
#[cfg(test)]
mod history_replay {
    #[test]
    #[ignore]
    fn translate_history() {
        let url = std::env::var("TYPEURVOICE_LLM_URL").expect("set TYPEURVOICE_LLM_URL");
        let db = std::env::var("TYPEURVOICE_HISTORY_DB").expect("set TYPEURVOICE_HISTORY_DB");
        let limit: u32 = std::env::var("TYPEURVOICE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        let note = std::env::var("TYPEURVOICE_SPEAKER_NOTE").unwrap_or_default();
        let conn = rusqlite::Connection::open(db).unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, processed_text, translated_text FROM transcriptions
             WHERE translated_text IS NOT NULL ORDER BY id DESC LIMIT ?1",
        ).unwrap();
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
            .collect::<Result<_, _>>().unwrap();
        let ctx = crate::processing::SpeakerContext { notes: note, ..Default::default() };
        let hint = ctx.translation_hint(&[]);
        let cfg = crate::processing::test_config(&format!("{}/v1/chat/completions", url.trim_end_matches('/')))
            .with_max_tokens(Some(1500));
        let http = reqwest::Client::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        for (id, source, old) in rows {
            let t = std::time::Instant::now();
            let new = rt.block_on(super::translate(&http, &source, "cs", "Czech", "English", hint.as_deref(), &cfg));
            println!("── #{id} ({:?})\nCS : {source}\nOLD: {old}\nNEW: {}\n", t.elapsed(), new.unwrap_or_else(|e| format!("ERROR {e}")));
        }
    }
}
