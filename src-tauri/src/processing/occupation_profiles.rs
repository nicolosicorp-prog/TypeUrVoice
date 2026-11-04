//! Built-in professional profiles loaded from `src-tauri/profiles/{locale}/*.json`.
//!
//! Each JSON file is compiled into the binary via `include_str!`, so the
//! shipped app needs no on-disk files for built-ins to work. User-created
//! clones live in `Settings::user_occupation_profiles` (persisted) and are
//! merged with the built-ins at lookup time.

use crate::state::{AppLocale, OccupationProfile, Settings};
use std::sync::OnceLock;

/// Compile-time bundle of every built-in profile JSON we ship. Adding a new
/// profile = one `include_str!` line + one parse. Build fails loudly if the
/// JSON is malformed (the parse runs on first call, see `builtins()`).
static BUILTINS: OnceLock<Vec<OccupationProfile>> = OnceLock::new();

macro_rules! include_profiles {
    ($($path:expr),* $(,)?) => {
        &[
            $( include_str!($path) ),*
        ]
    };
}

/// Every bundled profile JSON. Order doesn't matter — UI sorts/filters.
const PROFILE_JSON_FILES: &[&str] = include_profiles!(
    // Czech
    "../../profiles/cs/medicine-report.json",
    "../../profiles/cs/prescription.json",
    "../../profiles/cs/legal-filing.json",
    "../../profiles/cs/client-email.json",
    "../../profiles/cs/invoice-description.json",
    "../../profiles/cs/accounting-note.json",
    "../../profiles/cs/marketing-copy.json",
    "../../profiles/cs/slack-message.json",
    // Slovak
    "../../profiles/sk/medicine-report.json",
    "../../profiles/sk/prescription.json",
    "../../profiles/sk/legal-filing.json",
    "../../profiles/sk/client-email.json",
    "../../profiles/sk/invoice-description.json",
    "../../profiles/sk/accounting-note.json",
    "../../profiles/sk/marketing-copy.json",
    "../../profiles/sk/slack-message.json",
    // Polish
    "../../profiles/pl/medicine-report.json",
    "../../profiles/pl/prescription.json",
    "../../profiles/pl/legal-filing.json",
    "../../profiles/pl/client-email.json",
    "../../profiles/pl/invoice-description.json",
    "../../profiles/pl/accounting-note.json",
    "../../profiles/pl/marketing-copy.json",
    "../../profiles/pl/slack-message.json",
);

/// Parse all bundled JSON once and cache. Malformed JSON in a shipped profile
/// is a build-time mistake — we log it and skip that profile rather than
/// panicking the app at startup.
pub fn builtins() -> &'static [OccupationProfile] {
    BUILTINS.get_or_init(|| {
        let mut out = Vec::with_capacity(PROFILE_JSON_FILES.len());
        for raw in PROFILE_JSON_FILES {
            match serde_json::from_str::<OccupationProfile>(raw) {
                Ok(mut p) => {
                    p.builtin = true;
                    out.push(p);
                }
                Err(e) => {
                    crate::log_to_file(&format!(
                        "occupation_profiles: failed to parse bundled profile: {e}"
                    ));
                }
            }
        }
        out
    })
}

/// Combined view: built-ins + user clones. User clones win on id collision.
pub fn all_for_settings(settings: &Settings) -> Vec<OccupationProfile> {
    let mut by_id: std::collections::HashMap<String, OccupationProfile> =
        std::collections::HashMap::new();
    for p in builtins() {
        by_id.insert(p.id.clone(), p.clone());
    }
    for p in &settings.user_occupation_profiles {
        by_id.insert(p.id.clone(), p.clone());
    }
    let mut out: Vec<OccupationProfile> = by_id.into_values().collect();
    // Stable display order: locale, then occupation, then name.
    out.sort_by(|a, b| {
        a.locale.as_str().cmp(b.locale.as_str())
            .then_with(|| a.occupation.as_str().cmp(b.occupation.as_str()))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// Resolve the currently-active profile. Returns `None` when no profile is
/// selected, the active id is stale, or the profile was removed. Caller
/// should fall back to the user's existing OutputMode in that case.
pub fn active(settings: &Settings) -> Option<OccupationProfile> {
    let id = settings.active_occupation_profile_id.as_deref()?;
    // User clones take precedence if id collides.
    if let Some(p) = settings.user_occupation_profiles.iter().find(|p| p.id == id) {
        return Some(p.clone());
    }
    builtins().iter().find(|p| p.id == id).cloned()
}

/// Filter built-ins by locale. Used by the UI's locale-tab filtering and by
/// auto-suggest at first run ("Hi, looks like you're in Prague — try these
/// Czech profiles").
pub fn builtins_for_locale(locale: AppLocale) -> Vec<&'static OccupationProfile> {
    builtins().iter().filter(|p| p.locale == locale).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: every bundled profile parses, has non-empty fields,
    /// and the id matches the expected `builtin-<locale>-<slug>` pattern.
    /// Runs in CI before we ship.
    #[test]
    fn builtins_parse_and_validate() {
        let profiles = builtins();
        assert_eq!(profiles.len(), 24, "expected 24 bundled profiles (3 locales × 8 occupations)");
        for p in profiles {
            assert!(!p.id.is_empty(), "profile id is empty");
            assert!(p.id.starts_with("builtin-"), "id must start with 'builtin-': {}", p.id);
            assert!(!p.name.is_empty(), "name empty for {}", p.id);
            assert!(!p.prompt.is_empty(), "prompt empty for {}", p.id);
            assert!(p.builtin, "builtin flag must be true for {}", p.id);
        }
    }

    #[test]
    fn three_locales_present() {
        let profiles = builtins();
        for loc in [AppLocale::Cs, AppLocale::Sk, AppLocale::Pl] {
            let count = profiles.iter().filter(|p| p.locale == loc).count();
            assert_eq!(count, 8, "expected 8 profiles for {:?}, got {}", loc, count);
        }
    }
}
