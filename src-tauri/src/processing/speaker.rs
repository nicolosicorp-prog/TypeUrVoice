//! Everything the text-processing prompts know about the speaker beyond
//! the current recording: their own note ("I can't pronounce ř"), the
//! learned profile, corrections they made in History, and — while
//! streaming — the earlier part of the same dictation.

use crate::state::Settings;
use crate::storage::history::Correction;
use crate::storage::speaker::SpeakerProfile;

/// Each field is capped so the whole hint stays a small share of the
/// built-in model's 4096-token context.
const MAX_FIELD: usize = 600;
const MAX_EXAMPLE_FIELD: usize = 300;
const MAX_EARLIER: usize = 700;

#[derive(Debug, Clone, Default)]
pub struct SpeakerContext {
    pub notes: String,
    pub profile: SpeakerProfile,
    /// Earlier part of this dictation (streaming): the cleaned source text
    /// and what was already typed for it.
    pub earlier_source: String,
    pub earlier_output: String,
}

impl SpeakerContext {
    pub fn for_settings(settings: &Settings) -> Self {
        Self {
            notes: settings.speaker_notes.trim().to_string(),
            profile: if settings.learn_from_history {
                crate::storage::speaker::load()
            } else {
                SpeakerProfile::default()
            },
            ..Default::default()
        }
    }

    fn speaker_lines(&self, full: bool) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.notes.is_empty() {
            lines.push(format!("- The speaker says about themselves: {}", cap(&self.notes, MAX_FIELD)));
        }
        let p = &self.profile;
        if !p.recognition.trim().is_empty() {
            lines.push(format!("- Words the recognizer often gets wrong for them: {}", cap(&p.recognition, MAX_FIELD)));
        }
        if !p.terms.is_empty() {
            lines.push(format!("- Names and terms they use (spell exactly): {}", cap(&p.terms.join(", "), MAX_FIELD)));
        }
        if full {
            if !p.topics.trim().is_empty() {
                lines.push(format!("- What they usually talk about: {}", cap(&p.topics, MAX_FIELD)));
            }
            if !p.style.trim().is_empty() {
                lines.push(format!("- How they express themselves: {}", cap(&p.style, MAX_FIELD)));
            }
        }
        lines
    }

    /// Hint for Clean / Polished: recognition fixes and terms only — topics
    /// and style would invite a rewrite.
    pub fn cleanup_hint(&self, examples: &[Correction]) -> Option<String> {
        let mut out = String::new();
        let lines = self.speaker_lines(false);
        if !lines.is_empty() {
            out.push_str("About this speaker (use it only to fix recognition mistakes; never add content):\n");
            out.push_str(&lines.join("\n"));
        }
        if !examples.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str("Fixes this speaker made to earlier cleanups — make the same fixes when the same words come up:");
            for c in examples {
                out.push_str(&format!(
                    "\n- Recognized: \"{}\" → Correct: \"{}\"",
                    cap(&c.source_text, MAX_EXAMPLE_FIELD),
                    cap(&c.corrected_text, MAX_EXAMPLE_FIELD)
                ));
            }
        }
        (!out.is_empty()).then_some(out)
    }

    /// Hint for translation: the full profile, their corrections, and the
    /// earlier part of the dictation for context.
    pub fn translation_hint(&self, examples: &[Correction]) -> Option<String> {
        let mut parts = Vec::new();
        let lines = self.speaker_lines(true);
        if !lines.is_empty() {
            parts.push(format!("About this speaker:\n{}", lines.join("\n")));
        }
        if !examples.is_empty() {
            let mut s = String::from("Corrections this speaker made to earlier translations — follow their choices:");
            for c in examples {
                s.push_str(&format!(
                    "\nSource: {}\nWrong: {}\nCorrect: {}\n",
                    cap(&c.source_text, MAX_EXAMPLE_FIELD),
                    cap(&c.output_text, MAX_EXAMPLE_FIELD),
                    cap(&c.corrected_text, MAX_EXAMPLE_FIELD)
                ));
            }
            parts.push(s.trim_end().to_string());
        }
        if !self.earlier_source.trim().is_empty() {
            parts.push(format!(
                "Earlier in this same dictation (context only — do NOT translate or repeat it):\n{}{}",
                tail(&self.earlier_source, MAX_EARLIER),
                if self.earlier_output.trim().is_empty() {
                    String::new()
                } else {
                    format!("\nAlready translated as:\n{}", tail(&self.earlier_output, MAX_EARLIER))
                }
            ));
        }
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

/// First `max` chars (on a char boundary), "…" when cut.
fn cap(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Last `max` chars, "…" when cut.
fn tail(s: &str, max: usize) -> String {
    let s = s.trim();
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        format!("…{}", s.chars().skip(n - max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_adds_nothing() {
        let c = SpeakerContext::default();
        assert!(c.cleanup_hint(&[]).is_none());
        assert!(c.translation_hint(&[]).is_none());
    }

    #[test]
    fn cleanup_hint_leaves_out_style() {
        let c = SpeakerContext {
            notes: "Neumím ř".into(),
            profile: SpeakerProfile { style: "very direct".into(), terms: vec!["Acme".into()], ..Default::default() },
            ..Default::default()
        };
        let h = c.cleanup_hint(&[]).unwrap();
        assert!(h.contains("Neumím ř") && h.contains("Acme"));
        assert!(!h.contains("very direct"));
        assert!(c.translation_hint(&[]).unwrap().contains("very direct"));
    }

    #[test]
    fn caps_are_char_safe() {
        assert_eq!(cap("řřřř", 2), "řř…");
        assert_eq!(tail("řřřř", 2), "…řř");
    }
}
