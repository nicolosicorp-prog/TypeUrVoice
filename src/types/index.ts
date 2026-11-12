export type RecordingMode = 'hold' | 'toggle'
export type KnockPattern = 'double' | 'triple'
export type KnockSensitivity = 'low' | 'medium' | 'high'
export type KnockLanguage = 'primary' | 'secondary' | 'split' | 'auto'
export type OutputMode = 'raw' | 'clean' | 'polished' | 'markdown' | 'custom'
export type MediaWhileRecording = 'off' | 'mute' | 'pause'
export type Engine = 'cloud' | 'local'
export type CloudTranscriptionProvider = 'openai' | 'groq'

/** One named entry in the user's Custom-prompt library. IDs are assigned
 *  client-side via crypto.randomUUID() and are stable across renames. */
export interface CustomProfile {
  id: string
  name: string
  prompt: string
}

/** UI language. Independent of speech language. */
export type AppLocale = 'en' | 'cs' | 'sk' | 'pl'

/** Occupation grouping for built-in professional profiles. */
export type Occupation =
  | 'medicine'
  | 'law'
  | 'accounting'
  | 'marketing'
  | 'software'
  | 'generalBusiness'
  | 'custom'

/** A professional profile bundling a cleanup prompt + preserve-list of
 *  English/foreign terms that must NOT be translated when they appear inside
 *  CS/SK/PL speech. Built-ins ship in `src-tauri/profiles/`; user clones live
 *  inside `Settings.userOccupationProfiles`. */
export interface OccupationProfile {
  id: string
  name: string
  locale: AppLocale
  occupation: Occupation
  description: string
  prompt: string
  preserveTerms: string[]
  builtin: boolean
  version: number
}

/** Snapshot taken when Privacy Mode flips ON, so toggling OFF restores
 *  exactly the user's prior engine + text-processor choice. */
export interface PrivacyModeSnapshot {
  engine: Engine
  textProcessor: TextProcessor
}
export type LocalModel = 'small-q5' | 'medium-q5' | 'large-q5' | 'turbo-q5' | 'small' | 'medium' | 'base-en' | 'tiny-en'
export type SummaryType = 'keyPoints' | 'actionItems' | 'tldr'
export type RecordingState = 'idle' | 'recording' | 'processing' | 'done' | 'error'
export type TextProcessor =
  | 'openai'
  | 'anthropic'
  | 'gemini'
  | 'openaicompat'
  | 'ollama'
  | 'lmstudio'
  | 'builtin'

export type OpenAiCompatPreset =
  | 'grok'
  | 'groq'
  | 'deepseek'
  | 'mistral'
  | 'openrouter'
  | 'perplexity'
  | 'together'
  | 'custom'

/** Returned by `get_active_provider_info` — describes the currently selected provider. */
export interface ProviderInfo {
  displayName: string
  endpoint: string
  defaultModel: string
  recommended: string[]
  keySlot: string
  requiresKey: boolean
  stableCatalog: boolean
}

export interface Settings {
  onboardingComplete: boolean
  shortcut: string
  recordingMode: RecordingMode
  engine: Engine
  localModel: LocalModel
  outputMode: OutputMode
  autoPaste: boolean
  showNotification: boolean
  translationEnabled: boolean
  sourceLanguage: string
  targetLanguage: string
  summaryEnabled: boolean
  summaryType: SummaryType
  summaryShortcut: string | null
  historyRetentionDays: number
  startOnLogin: boolean
  dictionary: string[]
  /** Hosted-provider model for text processing (gpt-6-luna, claude-haiku-5-5, …). Empty ⇒ provider default. */
  processingModel: string
  /** Which service handles text cleanup/polish/translation/summary */
  textProcessor: TextProcessor
  /** Base URL for Ollama or LM Studio */
  textProcessorUrl: string
  /** Model name for Ollama/LM Studio */
  textProcessorModel: string
  /** Secondary shortcut for recording in a different language */
  secondaryShortcut: string | null
  /** Language for the secondary shortcut (ISO 639-1) */
  secondaryLanguage: string
  /** Language for the primary shortcut (ISO 639-1) */
  primaryLanguage: string
  /** When true, crash.log includes transcript text previews */
  diagnosticsVerbose: boolean
  /** Audio input device name (cpal). null ⇒ system default. */
  inputDevice: string | null
  /** LEGACY — single user-defined Custom prompt. Migrated into customProfiles
   *  on first run; new code reads customProfiles instead. */
  customPrompt: string
  /** Library of named Custom prompts. Empty ⇒ Custom mode falls back to Raw. */
  customProfiles: CustomProfile[]
  /** Which profile in customProfiles is currently active. */
  activeCustomProfileId: string | null
  /** When textProcessor === 'openaicompat', which preset is selected. */
  openaiCompatPreset: OpenAiCompatPreset
  /** Custom URL when openaiCompatPreset === 'custom'. */
  openaiCompatUrl: string
  /** Which Whisper host to use when engine === 'cloud'. */
  cloudTranscriptionProvider: CloudTranscriptionProvider
  /** Cloud transcription model id. null ⇒ provider default. */
  cloudTranscriptionModel: string | null
  /** UI language. */
  appLocale: AppLocale
  /** ID of the currently active occupation profile (built-in or user clone). */
  activeOccupationProfileId: string | null
  /** User-created clones / customisations. Built-ins are loaded from bundled JSON. */
  userOccupationProfiles: OccupationProfile[]
  /** Locale-aware post-LLM number/date/quote formatting. */
  localeFormatEnabled: boolean
  /** Rule-based removal of um/uh/ehm/eee/hmm from every transcript. */
  removeFillers: boolean
  /** Stream partial text into the recording bubble (local engine only). */
  livePreview: boolean
  /** Type finished sentences into the focused app while still speaking (local engine). */
  streamTyping: boolean
  /** The user's own note on how they speak — used by cleanup, translation and learning. */
  speakerNotes: string
  /** Learn a speaker profile from History and use it. */
  learnFromHistory: boolean
  /** Knocking on the MacBook chassis toggles recording (Apple Silicon MacBooks). */
  knockEnabled: boolean
  knockPattern: KnockPattern
  knockSensitivity: KnockSensitivity
  /** Language of knock-started recordings; 'split' = double → primary, triple → secondary. */
  knockLanguage: KnockLanguage
  /** Mute or pause the Mac's sound while recording (macOS). */
  mediaWhileRecording: MediaWhileRecording
  /** Markdown mode: structure to fill (free text, e.g. "## Goal"). Empty ⇒ model picks. */
  markdownTemplate: string
  /** Markdown mode: output language code, or 'auto' ⇒ shortcut/translation language, else spoken. */
  markdownLanguage: string
  /** Built-in (on-device) LLM model key — see local_llm::models. */
  builtinLlmModel: string
  /** Load on recording start and unload when idle (true) vs keep loaded until Quit (false). */
  builtinLlmAuto: boolean
  builtinLlmIdleMinutes: number
  /** Privacy mode: forces local engine + local LLM. */
  privacyMode: boolean
  /** Snapshot to restore when privacy mode toggles OFF. */
  privacyModeSnapshot: PrivacyModeSnapshot | null
}

export interface HistoryEntry {
  id: number
  rawText: string
  processedText: string
  mode: string
  language: string | null
  translatedText: string | null
  summary: string | null
  durationSecs: number | null
  createdAt: string
  /** The user fixed this entry's output in History. */
  corrected: boolean
}

/** What TypeUrVoice learned about how the user speaks (speaker_profile.json). */
export interface SpeakerProfile {
  topics: string
  terms: string[]
  recognition: string
  style: string
  /** Unix seconds; 0 = never updated. */
  updatedAt: number
  lastHistoryId: number
}

export interface PermissionsState {
  microphone: 'granted' | 'denied' | 'notDetermined' | 'restricted'
  accessibility: 'granted' | 'denied' | 'notDetermined' | 'restricted'
}

export type TabId = 'general' | 'engine' | 'output' | 'translation' | 'summary' | 'dictionary' | 'history' | 'help'

/** Row from list_builtin_llm_models. */
export interface BuiltinLlmModel {
  key: string
  name: string
  note: string
  sizeMb: number
  ramGb: number
  downloaded: boolean
  recommended: boolean
}

/** Mirrors local_llm::server::Status (serde tag = "state"). */
export type BuiltinLlmStatus =
  | { state: 'unloaded' }
  | { state: 'loading'; model: string }
  | { state: 'ready'; model: string }
  | { state: 'error'; message: string }
