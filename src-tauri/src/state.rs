use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
    Done,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecordingMode {
    Hold,
    Toggle,
}

/// What happens to the Mac's sound while recording — see `media`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MediaWhileRecording {
    #[default]
    Off,
    /// Mute the output device, unmute when the recording stops.
    Mute,
    /// Press Play/Pause if something is playing, press it again after.
    Pause,
}

/// Which language a knock-started recording uses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum KnockLanguage {
    /// The primary shortcut's language.
    #[default]
    Primary,
    /// The secondary shortcut's language.
    Secondary,
    /// Double knock = primary, triple knock = secondary (`knock_pattern`
    /// is ignored).
    Split,
    /// Whisper detects the spoken language; output stays in it.
    Auto,
}

/// How many knocks on the MacBook chassis toggle recording.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KnockPattern {
    Double,
    Triple,
}

impl KnockPattern {
    pub fn count(self) -> usize {
        match self {
            KnockPattern::Double => 2,
            KnockPattern::Triple => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KnockSensitivity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Raw,
    Clean,
    Polished,
    /// Apply a user-defined prompt from `Settings::custom_prompt`.
    /// Empty prompt ⇒ behave as Raw (skip the LLM round-trip entirely).
    Custom,
    /// Turn the dictation into a structured Markdown document, following
    /// any instructions spoken in it ("make it a list", "in English") and
    /// the optional `Settings::markdown_template` — see `cleanup::markdown`.
    Markdown,
}

impl OutputMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputMode::Raw => "raw",
            OutputMode::Clean => "clean",
            OutputMode::Polished => "polished",
            OutputMode::Custom => "custom",
            OutputMode::Markdown => "markdown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Cloud,
    Local,
}

/// One named entry in the user's library of `OutputMode::Custom` prompts.
/// IDs are assigned client-side (TypeScript `crypto.randomUUID()`); they
/// stay stable across renames so `active_custom_profile_id` doesn't break.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomProfile {
    pub id:     String,
    pub name:   String,
    pub prompt: String,
}

/// When `Engine::Cloud`, which Whisper host? OpenAI is the de-facto reference;
/// Groq hosts `whisper-large-v3` on the same multipart-form OpenAI-compatible
/// shape and is ~10× faster (free tier available).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CloudTranscriptionProvider {
    OpenAi,
    Groq,
}

impl Default for CloudTranscriptionProvider {
    fn default() -> Self { CloudTranscriptionProvider::OpenAi }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LocalModel {
    SmallQ5,
    MediumQ5,
    LargeQ5,
    TurboQ5,   // recommended: best Czech/Slovak accuracy, fast on Apple GPU
    Small,
    Medium,
    BaseEn,    // English-only — not suitable for Czech/Slovak
    TinyEn,    // English-only
}

impl LocalModel {
    pub fn key(&self) -> &str {
        match self {
            LocalModel::SmallQ5  => "small-q5",
            LocalModel::MediumQ5 => "medium-q5",
            LocalModel::LargeQ5  => "large-q5",
            LocalModel::TurboQ5  => "turbo-q5",
            LocalModel::Small    => "small",
            LocalModel::Medium   => "medium",
            LocalModel::BaseEn   => "base-en",
            LocalModel::TinyEn   => "tiny-en",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SummaryType {
    KeyPoints,
    ActionItems,
    Tldr,
}

/// UI language. Independent of speech language (`source_language`) — a Polish
/// dev might dictate in English but want the Settings UI in Polish.
/// Default is `En`; first-run code probes `sys-locale` and promotes to
/// `Cs`/`Sk`/`Pl` when the system locale starts with the matching prefix.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppLocale {
    En,
    Cs,
    Sk,
    Pl,
}

impl Default for AppLocale {
    fn default() -> Self { AppLocale::En }
}

impl AppLocale {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppLocale::En => "en",
            AppLocale::Cs => "cs",
            AppLocale::Sk => "sk",
            AppLocale::Pl => "pl",
        }
    }

    /// English language name used inside cleanup prompts. LLMs follow
    /// English language names more reliably than ISO codes.
    pub fn english_name(&self) -> &'static str {
        match self {
            AppLocale::En => "English",
            AppLocale::Cs => "Czech",
            AppLocale::Sk => "Slovak",
            AppLocale::Pl => "Polish",
        }
    }
}

/// Occupation grouping for built-in professional profiles. Used to filter
/// + display profiles in the UI ("show me only law profiles"). `Custom` is
/// the sentinel for user-created profiles that don't belong to a category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Occupation {
    Medicine,
    Law,
    Accounting,
    Marketing,
    Software,
    GeneralBusiness,
    Custom,
}

impl Occupation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Occupation::Medicine        => "medicine",
            Occupation::Law             => "law",
            Occupation::Accounting      => "accounting",
            Occupation::Marketing       => "marketing",
            Occupation::Software        => "software",
            Occupation::GeneralBusiness => "generalBusiness",
            Occupation::Custom          => "custom",
        }
    }
}

/// A professional profile: cleanup prompt + preserve-list (English/foreign
/// terms that must NOT be translated even when they appear inside
/// Czech/Slovak/Polish speech) + optional dictionary entries that get merged
/// with the user's personal dictionary for Whisper biasing.
///
/// Built-in profiles (`builtin: true`) live in `src-tauri/profiles/{locale}/`
/// and are loaded at startup. User clones of built-ins are `builtin: false`
/// and stored inside `Settings::user_occupation_profiles`. Built-ins are
/// re-loaded on every startup; user clones persist in settings.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OccupationProfile {
    pub id: String,
    pub name: String,
    pub locale: AppLocale,
    pub occupation: Occupation,
    #[serde(default)]
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub preserve_terms: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default = "default_profile_version")]
    pub version: u32,
}

fn default_profile_version() -> u32 { 1 }

/// Which service handles text processing (Clean/Polish/Translate/Summarize).
///
/// Wire serde tags stay lowercase so existing `settings.json` files keep working
/// — the original three variants (`openai`, `ollama`, `lmstudio`) are unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TextProcessor {
    OpenAi,        // OpenAI ChatCompletions API
    Anthropic,     // Anthropic Messages API (Claude)
    Gemini,        // Google Generative Language API
    OpenAiCompat,  // Generic OpenAI-compatible: Grok, Groq, DeepSeek, Mistral, OpenRouter, Perplexity, Together, or Custom
    Ollama,        // Local Ollama (http://localhost:11434/v1)
    LmStudio,      // LM Studio (http://localhost:1234/v1)
    BuiltIn,       // Bundled llama-server sidecar running a downloaded GGUF (local_llm)
}

impl TextProcessor {
    pub fn as_str(&self) -> &'static str {
        match self {
            TextProcessor::OpenAi       => "openai",
            TextProcessor::Anthropic    => "anthropic",
            TextProcessor::Gemini       => "gemini",
            TextProcessor::OpenAiCompat => "openaicompat",
            TextProcessor::Ollama       => "ollama",
            TextProcessor::LmStudio     => "lmstudio",
            TextProcessor::BuiltIn      => "builtin",
        }
    }
}

/// When `TextProcessor::OpenAiCompat` is active, which preset?
/// Each preset auto-fills a base URL + recommended model list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OpenAiCompatPreset {
    Grok,        // xAI — api.x.ai
    Groq,        // groq.com — fast inference (NOT the same as Grok)
    DeepSeek,    // deepseek.com — cheap reasoning
    Mistral,     // mistral.ai
    OpenRouter,  // openrouter.ai — meta-aggregator
    Perplexity,  // perplexity.ai — web-grounded
    Together,    // together.xyz
    Custom,      // user-supplied URL
}

impl OpenAiCompatPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpenAiCompatPreset::Grok       => "grok",
            OpenAiCompatPreset::Groq       => "groq",
            OpenAiCompatPreset::DeepSeek   => "deepseek",
            OpenAiCompatPreset::Mistral    => "mistral",
            OpenAiCompatPreset::OpenRouter => "openrouter",
            OpenAiCompatPreset::Perplexity => "perplexity",
            OpenAiCompatPreset::Together   => "together",
            OpenAiCompatPreset::Custom     => "custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarding_complete: bool,
    pub shortcut: String,
    pub recording_mode: RecordingMode,
    pub engine: Engine,
    pub local_model: LocalModel,
    pub output_mode: OutputMode,
    pub auto_paste: bool,
    pub show_notification: bool,
    pub translation_enabled: bool,
    pub source_language: String,
    pub target_language: String,
    pub summary_enabled: bool,
    pub summary_type: SummaryType,
    pub summary_shortcut: Option<String>,
    pub history_retention_days: u32,
    pub start_on_login: bool,
    pub dictionary: Vec<String>,
    /// OpenAI model for text processing (gpt-4o-mini, gpt-4o, etc.)
    #[serde(default = "default_processing_model")]
    pub processing_model: String,
    /// Which service handles text cleanup/polish/translation/summary
    #[serde(default = "default_text_processor")]
    pub text_processor: TextProcessor,
    /// Base URL for Ollama or LM Studio (e.g. http://localhost:11434)
    #[serde(default = "default_ollama_url")]
    pub text_processor_url: String,
    /// Model name for Ollama/LM Studio (e.g. llama3.2, mistral)
    #[serde(default = "default_ollama_model")]
    pub text_processor_model: String,
    /// Secondary shortcut — records in a different language than the primary
    #[serde(default)]
    pub secondary_shortcut: Option<String>,
    /// Language for the secondary shortcut (ISO 639-1 code, e.g. "en")
    #[serde(default = "default_secondary_language")]
    pub secondary_language: String,
    /// Language label for the primary shortcut (ISO 639-1 code, e.g. "cs")
    /// When empty or "auto", uses source_language from translation settings
    #[serde(default = "default_primary_language")]
    pub primary_language: String,
    /// When true, crash.log includes transcript text previews. Off by default
    /// so shared logs don't leak dictated content.
    #[serde(default)]
    pub diagnostics_verbose: bool,
    /// Name of the audio input device to record from. `None` ⇒ system default.
    /// Names come from `cpal`; if the named device is missing at record time
    /// we fall back to the default and log a warning.
    #[serde(default)]
    pub input_device: Option<String>,
    /// LEGACY — single user-defined system prompt, kept for backwards compat.
    /// On first launch with this build the value is migrated into a
    /// `CustomProfile` (see `migrate_custom_prompt_if_needed`) and ignored
    /// thereafter. New code reads `custom_profiles` instead.
    #[serde(default)]
    pub custom_prompt: String,
    /// Library of named `OutputMode::Custom` prompts. The active one is
    /// selected by `active_custom_profile_id`. Empty list ⇒ Custom mode
    /// falls back to Raw output (no LLM call).
    #[serde(default)]
    pub custom_profiles: Vec<CustomProfile>,
    /// Which profile in `custom_profiles` is currently active. `None` ⇒ no
    /// profile selected (treated as Raw fallback).
    #[serde(default)]
    pub active_custom_profile_id: Option<String>,
    /// When `text_processor == OpenAiCompat`, which preset is selected.
    #[serde(default = "default_openai_compat_preset")]
    pub openai_compat_preset: OpenAiCompatPreset,
    /// Custom URL for `OpenAiCompatPreset::Custom`. Ignored for built-in presets.
    #[serde(default)]
    pub openai_compat_url: String,
    /// When `engine == Cloud`, which Whisper host to use.
    #[serde(default)]
    pub cloud_transcription_provider: CloudTranscriptionProvider,
    /// Cloud transcription model id. `None` ⇒ the provider's default
    /// (`gpt-transcribe` / `whisper-large-v3-turbo`).
    #[serde(default)]
    pub cloud_transcription_model: Option<String>,
    /// UI language. Independent of `source_language`. Default `En`; first
    /// run promotes to a regional variant via `sys-locale`.
    #[serde(default)]
    pub app_locale: AppLocale,
    /// ID of the currently active occupation profile. References either a
    /// built-in profile (`builtin-cs-medicine-report` etc.) loaded from the
    /// bundled `profiles/` directory, or a user clone stored in
    /// `user_occupation_profiles`. `None` ⇒ no profile active (existing
    /// Clean/Polished/Custom behaviour).
    #[serde(default)]
    pub active_occupation_profile_id: Option<String>,
    /// User-created clones / customisations of occupation profiles. Built-in
    /// profiles are NEVER stored here; they're re-read from disk every run.
    #[serde(default)]
    pub user_occupation_profiles: Vec<OccupationProfile>,
    /// Enables locale-aware post-LLM number/date/quote formatting. Default
    /// on; users can disable if they explicitly want US formatting in CE/PL.
    #[serde(default = "default_locale_format")]
    pub locale_format_enabled: bool,
    /// Strip hesitation sounds (um, uh, ehm, eee, hmm …) from every
    /// transcript with a rule-based filter — applies in Raw mode too.
    /// No field-level serde attr: the struct-level `default` hands missing
    /// fields `Settings::default()`'s value (true), not `bool::default()`.
    pub remove_fillers: bool,
    /// Show partial text in the recording bubble while speaking. Local
    /// engine only (re-runs Whisper on the growing buffer).
    pub live_preview: bool,
    /// Type finished sentences into the focused app while still speaking
    /// (Local engine; see `stream`). The rest is typed when recording stops.
    pub stream_typing: bool,
    /// The user's own note on how they speak ("I can't pronounce ř — it
    /// sounds like ž"). Given to cleanup, translation and profile learning.
    pub speaker_notes: String,
    /// Learn a speaker profile from History (see `processing::learn`) and
    /// use it in cleanup, translation and Whisper's vocabulary hint.
    pub learn_from_history: bool,
    /// Knocking on the MacBook chassis starts/stops recording (primary
    /// language). Needs the Apple Silicon MacBook accelerometer — see `knock`.
    pub knock_enabled: bool,
    pub knock_pattern: KnockPattern,
    pub knock_sensitivity: KnockSensitivity,
    pub knock_language: KnockLanguage,
    /// Mute or pause the Mac's sound while recording (macOS).
    pub media_while_recording: MediaWhileRecording,
    /// Markdown mode: the structure to fill (headings, sections — free
    /// text). Empty ⇒ the model picks a structure that fits the dictation.
    pub markdown_template: String,
    /// Markdown mode: output language code, or "auto" ⇒ the shortcut's /
    /// translation target language, else the spoken one.
    pub markdown_language: String,
    /// Built-in (on-device) LLM: which GGUF from `local_llm::models`.
    pub builtin_llm_model: String,
    /// ON: load the model when recording starts, unload after
    /// `builtin_llm_idle_minutes` idle. OFF: keep it loaded until Quit.
    pub builtin_llm_auto: bool,
    pub builtin_llm_idle_minutes: u32,
    /// One-click "everything stays on my Mac" mode. When ON forces
    /// `engine = Local` and `text_processor ∈ {Ollama, LmStudio}` regardless
    /// of the underlying settings; toggling OFF restores the pre-toggle
    /// snapshot from `privacy_mode_snapshot`.
    #[serde(default)]
    pub privacy_mode: bool,
    /// Snapshot of (engine, text_processor) taken when privacy_mode flips
    /// from OFF→ON, so we can restore exactly on ON→OFF. None when privacy
    /// mode is off.
    #[serde(default)]
    pub privacy_mode_snapshot: Option<PrivacyModeSnapshot>,
}

fn default_locale_format() -> bool { true }

/// Captured at the moment privacy_mode was turned ON so toggling OFF puts
/// the user back exactly where they were.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyModeSnapshot {
    pub engine: Engine,
    pub text_processor: TextProcessor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarding_complete: false,
            shortcut: default_shortcut(),
            recording_mode: RecordingMode::Hold,
            engine: Engine::Cloud,
            local_model: LocalModel::TurboQ5,
            output_mode: OutputMode::Clean,
            auto_paste: true,
            show_notification: true,
            translation_enabled: false,
            source_language: "auto".to_string(),
            target_language: "en".to_string(),
            summary_enabled: false,
            summary_type: SummaryType::KeyPoints,
            summary_shortcut: None,
            history_retention_days: 30,
            start_on_login: false,
            dictionary: vec![],
            processing_model: default_processing_model(),
            text_processor: default_text_processor(),
            text_processor_url: default_ollama_url(),
            text_processor_model: default_ollama_model(),
            secondary_shortcut: None,
            secondary_language: default_secondary_language(),
            primary_language: default_primary_language(),
            diagnostics_verbose: false,
            input_device: None,
            custom_prompt: String::new(),
            custom_profiles: Vec::new(),
            active_custom_profile_id: None,
            openai_compat_preset: default_openai_compat_preset(),
            openai_compat_url: String::new(),
            cloud_transcription_provider: CloudTranscriptionProvider::default(),
            cloud_transcription_model: None,
            app_locale: AppLocale::default(),
            active_occupation_profile_id: None,
            user_occupation_profiles: Vec::new(),
            locale_format_enabled: true,
            remove_fillers: true,
            live_preview: true,
            stream_typing: false,
            speaker_notes: String::new(),
            learn_from_history: true,
            knock_enabled: false,
            knock_pattern: KnockPattern::Double,
            knock_sensitivity: KnockSensitivity::Medium,
            knock_language: KnockLanguage::Primary,
            media_while_recording: MediaWhileRecording::Off,
            markdown_template: String::new(),
            markdown_language: "auto".to_string(),
            builtin_llm_model: crate::local_llm::models::DEFAULT_MODEL.to_string(),
            builtin_llm_auto: true,
            builtin_llm_idle_minutes: 5,
            privacy_mode: false,
            privacy_mode_snapshot: None,
        }
    }
}

fn default_openai_compat_preset() -> OpenAiCompatPreset { OpenAiCompatPreset::Grok }

impl Settings {
    /// Resolve the active Custom profile's prompt. Returns `None` if no
    /// profile is selected, the active id is stale, or the prompt is empty
    /// — caller should fall back to Raw output in any of those cases.
    pub fn active_custom_prompt(&self) -> Option<&str> {
        let id = self.active_custom_profile_id.as_deref()?;
        let profile = self.custom_profiles.iter().find(|p| p.id == id)?;
        let trimmed = profile.prompt.trim();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    }
}

fn default_primary_language() -> String { "cs".to_string() }
fn default_secondary_language() -> String { "en".to_string() }
fn default_processing_model() -> String { "gpt-6-luna".to_string() }
fn default_text_processor() -> TextProcessor { TextProcessor::OpenAi }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_ollama_model() -> String { "gemma4".to_string() }

fn default_shortcut() -> String {
    #[cfg(target_os = "macos")]
    return "Option+Space".to_string();
    #[cfg(not(target_os = "macos"))]
    return "Ctrl+Shift+Space".to_string();
}

pub struct AppState {
    pub recording_state: RecordingState,
    /// Shared audio sample buffer — written by cpal thread, read by pipeline
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    /// Set to true to stop the recording thread
    pub stop_flag: Arc<AtomicBool>,
    pub sample_rate: Arc<Mutex<u32>>,
    /// When the current recording started (for minimum-duration enforcement)
    pub recording_started_at: Option<Instant>,
    /// Language override for the current recording (set by whichever shortcut triggered it)
    pub active_language: Option<String>,
    /// Set by the cpal thread once its stream is dropped and the buffer is
    /// final. Lets the pipeline start as soon as capture ends instead of
    /// sleeping a fixed interval.
    pub capture_done: Arc<AtomicBool>,
    /// True while a live-preview inference is running for the current
    /// session. The final pass waits for it so both don't compete for the GPU.
    pub preview_busy: Arc<AtomicBool>,
    /// Typing-while-speaking session for the current recording, if on.
    pub stream: Option<Arc<crate::stream::StreamSession>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_state: RecordingState::Idle,
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            sample_rate: Arc::new(Mutex::new(16000)),
            recording_started_at: None,
            active_language: None,
            capture_done: Arc::new(AtomicBool::new(true)),
            preview_busy: Arc::new(AtomicBool::new(false)),
            stream: None,
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;
pub type SharedSettings = Arc<Mutex<Settings>>;

/// Set to true when the user explicitly clicks "Quit TypeUrVoice" from the tray.
/// Checked by the ExitRequested handler to allow intentional quits through.
pub struct QuitFlag(pub AtomicBool);
