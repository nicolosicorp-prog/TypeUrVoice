//! Built-in dictionary packs bundled at compile time.
//!
//! Each pack is a curated list of domain terms (medical, legal, accounting,
//! marketing, software) per language. They're not laws — whisper.cpp uses
//! them as biasing hints. When an `OccupationProfile` is active, the matching
//! pack is merged with the user's personal dictionary before transcription.

use crate::state::{AppLocale, Occupation};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntry {
    pub term: String,
    #[serde(default)]
    pub pronunciation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictPack {
    pub language: String,
    pub occupation: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub starter: bool,
    #[serde(default)]
    pub description: String,
    pub entries: Vec<DictEntry>,
}

static PACKS: OnceLock<Vec<DictPack>> = OnceLock::new();

macro_rules! include_dicts {
    ($($path:expr),* $(,)?) => {
        &[ $( include_str!($path) ),* ]
    };
}

const DICT_JSON_FILES: &[&str] = include_dicts!(
    "../../dictionaries/cs/medicine.json",
    "../../dictionaries/cs/law.json",
    "../../dictionaries/cs/accounting.json",
    "../../dictionaries/cs/marketing.json",
    "../../dictionaries/cs/software.json",
    "../../dictionaries/sk/medicine.json",
    "../../dictionaries/sk/law.json",
    "../../dictionaries/sk/accounting.json",
    "../../dictionaries/sk/marketing.json",
    "../../dictionaries/sk/software.json",
    "../../dictionaries/pl/medicine.json",
    "../../dictionaries/pl/law.json",
    "../../dictionaries/pl/accounting.json",
    "../../dictionaries/pl/marketing.json",
    "../../dictionaries/pl/software.json",
);

fn packs() -> &'static [DictPack] {
    PACKS.get_or_init(|| {
        let mut out = Vec::with_capacity(DICT_JSON_FILES.len());
        for raw in DICT_JSON_FILES {
            match serde_json::from_str::<DictPack>(raw) {
                Ok(p) => out.push(p),
                Err(e) => crate::log_to_file(&format!(
                    "profile_dictionaries: failed to parse pack: {e}"
                )),
            }
        }
        out
    })
}

/// Find the dictionary pack matching `(locale, occupation)`. Returns None
/// if we don't ship one for that combination yet (e.g. some occupations
/// might not have packs in every language at launch).
pub fn lookup(locale: AppLocale, occupation: Occupation) -> Option<&'static DictPack> {
    let loc_str = locale.as_str();
    let occ_str = occupation.as_str();
    packs().iter().find(|p| p.language == loc_str && p.occupation == occ_str)
}

/// Merge a pack's terms with the user's personal dictionary. Used during
/// transcription when an occupation profile is active. Dedupes
/// case-insensitively while preserving the user's casing on collision.
pub fn merge_with_personal(personal: &[String], pack: Option<&DictPack>) -> Vec<String> {
    let mut out: Vec<String> = personal.to_vec();
    let mut seen: std::collections::HashSet<String> =
        personal.iter().map(|s| s.to_lowercase()).collect();
    if let Some(p) = pack {
        for entry in &p.entries {
            let key = entry.term.to_lowercase();
            if seen.insert(key) {
                out.push(entry.term.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifteen_packs_load() {
        assert_eq!(packs().len(), 15, "expected 15 starter dict packs");
    }

    #[test]
    fn lookup_finds_czech_medicine() {
        let pack = lookup(AppLocale::Cs, Occupation::Medicine).expect("cs/medicine missing");
        assert_eq!(pack.language, "cs");
        assert_eq!(pack.occupation, "medicine");
        assert!(!pack.entries.is_empty());
    }

    #[test]
    fn merge_dedupes_case_insensitive() {
        let personal = vec!["MyTerm".to_string(), "Another".to_string()];
        let pack = DictPack {
            language: "cs".into(),
            occupation: "medicine".into(),
            version: 1,
            starter: true,
            description: String::new(),
            entries: vec![
                DictEntry { term: "myterm".into(), pronunciation: None },
                DictEntry { term: "Brand".into(), pronunciation: None },
            ],
        };
        let merged = merge_with_personal(&personal, Some(&pack));
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0], "MyTerm");
        assert!(merged.contains(&"Brand".to_string()));
    }
}
