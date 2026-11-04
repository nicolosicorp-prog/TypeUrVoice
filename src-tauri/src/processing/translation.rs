use anyhow::Result;

pub async fn translate(
    http: &reqwest::Client,
    text: &str,
    source: &str,
    target: &str,
    cfg: &super::GptConfig,
) -> Result<String> {
    let system = format!(
        "Translate the following text from {} to {}.\n\
        - Preserve the speaker's tone and intent.\n\
        - Preserve formatting: bullet points, line breaks, paragraphs.\n\
        - Do NOT add notes, comments, or explanations.\n\
        - Return ONLY the translation, nothing else.",
        source, target
    );
    super::gpt_complete(http, cfg, &system, text).await
}
