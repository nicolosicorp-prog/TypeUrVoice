use anyhow::Result;

pub async fn clean(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    dict_hint: Option<&str>,
    language: &str,
) -> Result<String> {
    let system = build_clean_prompt(language, dict_hint);
    super::gpt_complete(http, cfg, &system, text).await
}

pub async fn polish(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    dict_hint: Option<&str>,
    language: &str,
) -> Result<String> {
    let system = build_polish_prompt(language, dict_hint);
    super::gpt_complete(http, cfg, &system, text).await
}

/// Apply a user-defined prompt. We trust the user's instructions but always
/// append the same hard rules: language must be preserved, no markdown
/// fences, no surrounding commentary. Without these the LLM can drift to
/// English or wrap output in ```.
pub async fn custom(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    user_prompt: &str,
    dict_hint: Option<&str>,
    language: &str,
) -> Result<String> {
    let mut system = String::with_capacity(user_prompt.len() + 256);
    system.push_str(user_prompt.trim());
    system.push_str("\n\nHARD RULES (always apply, regardless of the instructions above):\n");
    system.push_str(&format!("- The text is in {lang}; output MUST be in {lang}. Never translate.\n", lang = language));
    system.push_str("- Do NOT wrap the output in code fences, quotes, or surrounding commentary.\n");
    system.push_str("- Return ONLY the rewritten text.");
    if let Some(hint) = dict_hint {
        system.push_str("\n\n");
        system.push_str(hint);
    }
    super::gpt_complete(http, cfg, &system, text).await
}

/// Apply a built-in occupation profile (Lékařská zpráva, Pismo procesowe, etc.).
/// The profile's `prompt` already contains the full system prompt — code-switching
/// rule, preserve list inlined, occupation-specific structure. We append the
/// dictionary hint (merged personal + bundled pack) and the standard "no fences"
/// tail. We do NOT inject our own language directive; the profile prompt
/// declares the language authoritatively.
pub async fn profile(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    profile: &crate::state::OccupationProfile,
    dict_hint: Option<&str>,
) -> Result<String> {
    let mut system = String::with_capacity(profile.prompt.len() + 256);
    system.push_str(&profile.prompt);
    if let Some(hint) = dict_hint {
        system.push_str("\n\n");
        system.push_str(hint);
    }
    super::gpt_complete(http, cfg, &system, text).await
}

fn build_clean_prompt(language: &str, dict_hint: Option<&str>) -> String {
    let mut prompt = format!(
        "You are a transcript cleanup assistant. The text is in {lang}; output MUST be in {lang} (no translation).\n\n\
        Apply ONLY these edits to this speech transcript:\n\
        - Fix grammar, spelling, punctuation, and capitalization.\n\
        - Remove obvious filler words and hesitations: 'um', 'uh', 'er', 'ah', 'hmm', and the discourse-marker uses of 'like', 'you know', 'I mean', 'sort of', 'kind of', 'basically'.\n\
        - Remove false starts and self-corrections; keep the corrected version.\n\
        - Preserve the speaker's wording, tone, sentence structure, and meaning everywhere else. Do NOT rephrase or rewrite.\n\
        - Do NOT summarize, expand, or add new ideas.\n\
        - Do NOT wrap the output in code fences or quotes.\n\
        Return ONLY the cleaned transcript.",
        lang = language
    );
    if let Some(hint) = dict_hint {
        prompt.push_str("\n\n");
        prompt.push_str(hint);
    }
    prompt
}

fn build_polish_prompt(language: &str, dict_hint: Option<&str>) -> String {
    let mut prompt = format!(
        "You are a transcript polishing assistant. The text is in {lang}; output MUST be in {lang} (no translation).\n\n\
        Rewrite this speech transcript into clear, professional prose:\n\
        - Fix all grammar, punctuation, and structure.\n\
        - Remove all filler words and hesitations.\n\
        - Improve clarity and flow while preserving the core message.\n\
        - Use paragraphs naturally. Use bullet lists ONLY when the speech is genuinely a list of items.\n\
        - Do NOT wrap the output in code fences or quotes.\n\
        Return ONLY the polished text.",
        lang = language
    );
    if let Some(hint) = dict_hint {
        prompt.push_str("\n\n");
        prompt.push_str(hint);
    }
    prompt
}
