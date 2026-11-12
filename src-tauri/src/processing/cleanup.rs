use anyhow::Result;

/// `speaker_hint`: what we know about the speaker (their note, learned
/// recognition mistakes, terms) plus their past corrections — see
/// `SpeakerContext::cleanup_hint`.
pub async fn clean(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    dict_hint: Option<&str>,
    language: &str,
    speaker_hint: Option<&str>,
) -> Result<String> {
    let mut system = build_clean_prompt(language, dict_hint);
    append_hint(&mut system, speaker_hint);
    super::gpt_complete(http, cfg, &system, text).await
}

pub async fn polish(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    dict_hint: Option<&str>,
    language: &str,
    speaker_hint: Option<&str>,
) -> Result<String> {
    let mut system = build_polish_prompt(language, dict_hint);
    append_hint(&mut system, speaker_hint);
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

/// Markdown mode: turn the dictation into a structured Markdown document in
/// `output_language`, following instructions the speaker gave about the
/// output while dictating, and filling `template` when one is set.
#[allow(clippy::too_many_arguments)]
pub async fn markdown(
    http: &reqwest::Client,
    text: &str,
    cfg: &super::GptConfig,
    spoken_language: &str,
    output_language: &str,
    template: &str,
    dict_hint: Option<&str>,
    speaker_hint: Option<&str>,
) -> Result<String> {
    let mut system = build_markdown_prompt(spoken_language, output_language, template);
    append_hint(&mut system, dict_hint);
    append_hint(&mut system, speaker_hint);
    super::gpt_complete(http, cfg, &system, text).await
}

pub(super) fn build_markdown_prompt(spoken: &str, output: &str, template: &str) -> String {
    let language_rule = if spoken == output {
        format!("- Write the document in {output}. If the speaker explicitly asks for another language, use that one.\n")
    } else {
        format!(
            "- Write the document in {output} — translate from {spoken}, by meaning, not word for word. \
            If the speaker explicitly asks for another language, use that one.\n"
        )
    };
    let mut prompt = format!(
        "You turn a dictated voice transcript into a well-structured Markdown document.\n\n\
        The transcript is in {spoken} and comes from speech recognition: it may contain misheard words, \
        filler words, false starts, repetitions and self-corrections. Write what the speaker meant.\n\n\
        While dictating, the speaker often says what they want from the output — for example \
        \"make it a list\", \"write it in English\", \"add a section about testing\", \"leave the prices out\", \
        \"this is a task for a developer\". Follow these instructions and do NOT include them in the document.\n\n\
        Rules:\n\
        {language_rule}\
        - Keep technical terms, product and brand names, file names, commands and code identifiers in their \
        usual form (usually English) — never translate or localise them, also when writing in another language.\n\
        - Keep every fact, requirement, name and number the speaker gave. Do not invent content, add advice, \
        or comment on the text.\n\
        - Structure with Markdown: `##` headings for distinct topics, `-` bullet lists for items and requirements, \
        numbered lists for steps or an order, **bold** only for key terms, `inline code` for commands, paths and \
        identifiers. A short dictation needs no headings — a few bullets or a short paragraph is enough. \
        No `#` title unless the speaker asks for one.\n"
    );
    let template = template.trim();
    if !template.is_empty() {
        prompt.push_str(
            "- Use the structure below. Put what the speaker said under the matching heading, leave out \
            headings the speaker said nothing about, and don't add headings of your own unless the speaker asks. \
            Lines in the structure that describe a section are guidance for you, not text to copy.\n\
            <structure>\n",
        );
        prompt.push_str(template);
        prompt.push_str("\n</structure>\n");
    }
    prompt.push_str(
        "- Output ONLY the Markdown document — no preamble, no closing remarks, and no code fence around the whole document.",
    );
    prompt
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
        - The text comes from speech recognition. A word that is not a real {lang} word, or makes no sense in the sentence, was misheard: replace it with the real word that sounds almost the same and fits (often just a wrong or missing accent or one wrong letter). Never swap in a word with a different meaning.\n\
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

fn append_hint(prompt: &mut String, hint: Option<&str>) {
    if let Some(hint) = hint.filter(|h| !h.trim().is_empty()) {
        prompt.push_str("\n\n");
        prompt.push_str(hint);
    }
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
