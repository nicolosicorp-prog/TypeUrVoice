use anyhow::Result;
use crate::state::SummaryType;

pub async fn summarize(
    http: &reqwest::Client,
    text: &str,
    summary_type: &SummaryType,
    language: &str,
    cfg: &super::GptConfig,
) -> Result<String> {
    let task = match summary_type {
        SummaryType::KeyPoints => {
            "Extract the key points as a concise bullet-point list. \
             Use '• ' as bullet prefix. Return only the bullet points, nothing else."
        }
        SummaryType::ActionItems => {
            "Extract all action items and tasks as a checklist. \
             Each item should start with '- [ ] '. \
             Return only the action items, nothing else."
        }
        SummaryType::Tldr => {
            "Write a one-paragraph TL;DR summary (3-5 sentences). \
             Be concise and clear. Return only the summary paragraph, nothing else."
        }
    };
    let system = format!(
        "Write the output in {lang}. {task}",
        lang = language,
        task = task,
    );
    super::gpt_complete(http, cfg, &system, text).await
}
