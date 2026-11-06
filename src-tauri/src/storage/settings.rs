use anyhow::Result;
use std::path::PathBuf;
use crate::state::Settings;

pub fn voiceink_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("VoiceInk");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn settings_path() -> Result<PathBuf> {
    Ok(voiceink_data_dir()?.join("settings.json"))
}

use super::restrict_permissions;

pub fn load() -> Result<Settings> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let settings: Settings = serde_json::from_str(&content)?;
    Ok(settings)
}

pub fn save(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    let content = serde_json::to_string_pretty(settings)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)?;
    let _ = restrict_permissions(&tmp);
    std::fs::rename(&tmp, &path)?;
    let _ = restrict_permissions(&path);
    Ok(())
}

// API keys live in `storage::api_keys::keys.json` (multi-provider). The legacy
// `.api_key` file is migrated on first run by `api_keys::migrate_legacy_if_needed`.

/// Model ids that were retired upstream, or were VoiceInk's own defaults
/// before the Oct 2026 refresh, mapped to their current replacement.
/// `processing_model` is only ever a hosted-provider id, so a flat map is
/// unambiguous.
const RETIRED_MODELS: &[(&str, &str)] = &[
    // OpenAI
    ("gpt-4o-mini", "gpt-6-luna"),
    ("gpt-3.5-turbo", "gpt-6-luna"),
    ("gpt-4-turbo", "gpt-6-luna"),
    ("o1-mini", "gpt-6-luna"),
    // Anthropic
    ("claude-haiku-4-5", "claude-haiku-5-5"),
    ("claude-sonnet-4-5", "claude-sonnet-5-5"),
    ("claude-opus-4-5", "claude-opus-5-5"),
    // Google — the 2.5 family shuts down in October 2026
    ("gemini-2.5-flash", "gemini-3.5-flash-lite"),
    ("gemini-2.5-flash-lite", "gemini-3.5-flash-lite"),
    ("gemini-2.5-pro", "gemini-3.8-flash"),
    // Deprecated Oct 2026 — Google already routes them to the successors
    ("gemini-3.5-flash", "gemini-3.6-flash"),
    ("gemini-3.7-flash", "gemini-3.8-flash"),
    // OpenAI-compatible presets
    // grok-4.1-fast was retired May 2026; xAI redirects it to grok-4.3
    ("grok-4-fast", "grok-4.3"),
    ("grok-4.1-fast", "grok-4.3"),
    ("grok-4", "grok-4.3"),
    ("mixtral-8x7b-32768", "llama-3.3-70b-versatile"),
    // DeepSeek renamed its flash model; the old names are legacy aliases
    ("deepseek-chat", "deepseek-flash"),
    ("deepseek-v4-flash", "deepseek-flash"),
    ("deepseek-reasoner", "deepseek-v4-pro"),
    ("pixtral-large-latest", "mistral-large-latest"),
    ("sonar-reasoning", "sonar-pro"),
    ("openai/gpt-4o-mini", "openai/gpt-6-luna"),
    ("anthropic/claude-haiku-4-5", "anthropic/claude-haiku-5-5"),
];

/// Rewrite retired model ids in place. Returns the `(old, new)` pair when
/// something changed so the caller can persist + log it.
pub fn migrate_retired_models(settings: &mut Settings) -> Option<(String, String)> {
    let current = settings.processing_model.trim();
    let (_, replacement) = RETIRED_MODELS.iter().find(|(old, _)| *old == current)?;
    let old = std::mem::replace(&mut settings.processing_model, replacement.to_string());
    Some((old, replacement.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_retired_and_old_default_ids() {
        let mut s = Settings::default();
        s.processing_model = "gemini-2.5-flash".into();
        assert_eq!(
            migrate_retired_models(&mut s),
            Some(("gemini-2.5-flash".into(), "gemini-3.5-flash-lite".into()))
        );
        assert_eq!(s.processing_model, "gemini-3.5-flash-lite");
    }

    #[test]
    fn leaves_current_and_custom_ids_alone() {
        let mut s = Settings::default();
        for id in ["gpt-6-luna", "my-finetune", ""] {
            s.processing_model = id.into();
            assert_eq!(migrate_retired_models(&mut s), None);
            assert_eq!(s.processing_model, id);
        }
    }

    #[test]
    fn replacements_are_not_themselves_retired() {
        for (_, new) in RETIRED_MODELS {
            assert!(RETIRED_MODELS.iter().all(|(old, _)| old != new), "{new} maps again");
        }
    }
}
