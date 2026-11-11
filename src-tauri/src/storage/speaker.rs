//! Learned speaker profile — what TypeUrVoice has picked up about how the
//! user speaks: topics, names/terms, words the recognizer keeps mishearing,
//! and phrasing style. Written by `processing::learn` from history, read by
//! cleanup, translation and Whisper's vocabulary hint.
//!
//! Kept in its own file, not in settings.json: the Settings UI writes the
//! whole settings object back on every change, which would overwrite a
//! profile updated in the background.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeakerProfile {
    pub topics: String,
    pub terms: Vec<String>,
    /// "heard → meant" pairs and the pronunciation pattern behind them.
    pub recognition: String,
    pub style: String,
    /// Unix seconds of the last update (0 = never).
    pub updated_at: i64,
    /// Highest history id the profile has seen — new dictations after it
    /// count towards the next update.
    pub last_history_id: i64,
}

impl SpeakerProfile {
    pub fn is_empty(&self) -> bool {
        self.topics.trim().is_empty()
            && self.terms.is_empty()
            && self.recognition.trim().is_empty()
            && self.style.trim().is_empty()
    }
}

fn path() -> Result<PathBuf> {
    Ok(super::settings::typeurvoice_data_dir()?.join("speaker_profile.json"))
}

pub fn load() -> SpeakerProfile {
    path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(profile: &SpeakerProfile) -> Result<()> {
    let path = path()?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(profile)?)?;
    let _ = super::restrict_permissions(&tmp);
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn clear() -> Result<()> {
    let path = path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
