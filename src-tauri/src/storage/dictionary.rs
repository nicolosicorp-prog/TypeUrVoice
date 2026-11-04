// Dictionary is stored as part of Settings (Vec<String>)
// This module provides helper functions for working with it.

pub fn format_for_prompt(words: &[String]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    Some(format!(
        "Please ensure these proper nouns and terms are spelled exactly as shown: {}.",
        words.join(", ")
    ))
}

pub fn format_for_whisper(words: &[String]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    // Whisper API accepts a "prompt" param to bias transcription vocabulary
    Some(words.join(", "))
}
