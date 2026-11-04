# VoiceInk

A fast, private, keyboard-driven voice-to-text app for macOS and Windows. Press a hotkey, speak, release — your words are transcribed and pasted directly into whatever app you're using. Lives quietly in the menu bar, never steals focus, and runs fully offline if you want it to.

---

## What It Does

VoiceInk turns speech into text wherever your cursor is. The flow is:

1. **Press your hotkey** (default: Option+Space on macOS).
2. **Speak** — a small overlay shows you're recording.
3. **Release** (Hold mode) or **press again** (Toggle mode).
4. **Your transcription appears** in the focused app, automatically pasted from the clipboard.

No window switching. No "send" button. No web app. Just speak and keep working.

---

## Three Ways to Run It

VoiceInk is designed around a single privacy/convenience tradeoff that you control:

### Fully Online
- **Transcription:** OpenAI Whisper API (or Groq Whisper)
- **Text cleanup:** OpenAI / Anthropic / Google Gemini / xAI Grok / DeepSeek / Mistral / OpenRouter / Perplexity / Together
- Easiest setup, best accuracy, audio leaves your machine

### Semi-Offline
- **Transcription:** Local Whisper (runs on your CPU)
- **Text cleanup:** Any cloud LLM provider
- Audio stays on your device — only the transcript text is sent to the LLM

### Fully Offline
- **Transcription:** Local Whisper
- **Text cleanup:** Ollama or LM Studio (both run locally)
- Nothing ever leaves your Mac

You can switch between these in Settings → Engine at any time.

---

## Core Features

### Recording & Hotkeys
- **Global hotkey** — works from any app, no need to focus VoiceInk
- **Two recording modes:**
  - **Hold** — press and hold to record, release to stop (600 ms minimum to avoid accidental triggers)
  - **Toggle** — press once to start, press again to stop
- **Dual language shortcuts** — bind two separate hotkeys for two languages (e.g. Option+Space for Czech, Alt+Shift+Space for English) so you can switch languages without touching Settings
- **Conflict detection** — UI prevents you from binding the same combo to both shortcuts
- **5-minute hard cap** per recording with a safety-capped audio buffer

### Transcription Engines
- **Local Whisper** via whisper.cpp — fully offline, multilingual (Czech, Slovak, English, and 90+ more)
- **OpenAI Whisper API** — cloud, fastest setup
- **Groq Whisper-Large-v3** — cloud, ~10× faster than OpenAI, generous free tier
- **8 model sizes** for local use, from `tiny` to `large` (multilingual Q5-quantized models recommended)
- **One-click model download** with progress bar
- **SHA-256 verified** — every downloaded model is hash-checked against a pinned table, so a tampered file can never be loaded
- **Hot model cache** — the Whisper model loads once and stays in memory, so the second recording onwards is 2–5 seconds faster
- **Hallucination filter** — strips out the well-known Whisper junk outputs (`[music]`, `Thank you for watching!`, subtitle credits, bare URLs, etc.)

### Text Processing (Output Modes)
- **Raw** — just the Whisper output, untouched. Fastest, most faithful for dictation
- **Clean** — fixes punctuation and grammar, strips fillers ("um", "uh", "like", "you know", "I mean", etc.) and false starts. Preserves your meaning and tone
- **Polished** — rewrites your speech into professional prose with paragraphs and structure
- **Custom** — define your own prompt, or build a library of named profiles for different contexts (Email tone, Tweet under 280, Bullet points only, Strip emojis, etc.)

### Custom Profiles
- Save unlimited prompts as named profiles
- Quick-switch between them from the tray menu (sub-submenu under Custom)
- Pencil to rename, trash to delete, "+ New profile" to create
- Active profile persists; switching takes effect on the next recording
- 3000-character prompt limit

### Translation
- Translate any transcription on the fly
- Independent **source language** (what you're speaking) and **target language** (what you want out)
- Source language is always visible in the Translation tab — set it explicitly for short clips where Whisper's auto-detect is unreliable
- Translation prompt preserves your tone and intent; no commentary added

### Summarization
- Three summary types: short summary, bullet points, action items
- Dedicated summary shortcut so you can dictate one way and summarize another
- Summaries respect your chosen language (Czech speech → Czech summary unless translation is enabled)

### Personal Dictionary
- Add proper nouns, brand names, technical terms, acronyms
- Acts as a Whisper bias hint to improve recognition
- Note: it's a hint, not a hard rule — Whisper still decides

### History
- Every transcription is saved locally to a SQLite database
- Owner-only file permissions (chmod 0600)
- Two-click clear (Confirm Clear? → click again within 3s, or Esc to cancel)

### Auto-paste
- Transcribed text is automatically pasted into the app you were using
- Uses AppleScript on macOS (subprocess-based, can't crash the app)
- Falls back to clipboard if paste fails — your text is never lost
- Notification (optional) tells you when it's done

---

## Multi-Provider LLM Support

VoiceInk speaks three wire protocols (OpenAI Chat, Anthropic Messages, Google Gemini) and ships with **10 built-in providers**:

| Provider | Default Model | Key Storage Slot |
|---|---|---|
| OpenAI | gpt-4o-mini | `openai` |
| Anthropic Claude | Haiku 4.5 | `anthropic` |
| Google Gemini | Flash 2.5 | `gemini` |
| xAI Grok | grok-2 | `compat:grok` |
| Groq | llama-3.3-70b | `compat:groq` |
| DeepSeek | deepseek-chat | `compat:deepseek` |
| Mistral | mistral-small | `compat:mistral` |
| OpenRouter | meta-aggregator (100+ models) | `compat:openrouter` |
| Perplexity | sonar | `compat:perplexity` |
| Together AI | free-form picker | `compat:together` |
| Custom URL | any OpenAI-compatible endpoint | `compat:custom` |
| Ollama (local) | any installed model | — |
| LM Studio (local) | any loaded model | — |

- **Per-provider key storage** — switching from Grok to DeepSeek doesn't delete your Grok key
- **Refresh from provider** — fetch the latest model list from any provider's `/models` endpoint (24h cached)
- **Test key** button — every key field has a probe that confirms the key works before you save

---

## Privacy & Security

- **Files are owner-only** — `settings.json`, `keys.json`, `history.db` all chmod 0600 on Unix
- **Atomic writes** — settings file is written to a temp file then renamed, so a mid-write crash never zeroes your config
- **Local API keys** — stored in `~/Library/Application Support/VoiceInk/keys.json` with restricted permissions (no keychain dependency)
- **No telemetry, no analytics, no tracking**
- **Verbose diagnostics opt-in** — transcript previews in `crash.log` are off by default; enable in General → System if you need to share logs
- **Crash log rotation** — capped at 1 MB live + 1 MB backup (3 MB worst case on disk)

---

## Accessibility

- **Full keyboard navigation** — tabs use proper ARIA roles, arrow keys / Home / End move between them
- **Screen-reader friendly** — toggles announce as switches with on/off state, alerts use the right `aria-live` politeness
- **Visible focus rings** — passes WCAG AA contrast on the dark theme
- **Dismissable toasts** — close button + Escape key
- **Tertiary text contrast** — bumped to AA (6.3:1 on black)

---

## System Integration

- **Lives in the menu bar** — no Dock icon, no taskbar
- **Tray menu** — quick output mode switching (with checkmarks), open Settings, quit
- **Two-way sync** — changing the output mode in the tray updates the Settings UI, and vice versa
- **Microphone picker** — choose from any installed input device (or system default), with a "(not connected)" hint if your saved device is unplugged
- **Start on login** option
- **Show crash.log** button — reveals the log file in Finder for easy sharing
- **Updates check** — built on tauri-plugin-updater (auto-update activation pending release signing)

---

## Reliability & Robustness

- **Pipeline timeouts:** 360s outer wrapper, 60s per LLM call — long pipelines (cleanup + translation + summary) fit comfortably with slack
- **Single retry on transient errors** — HTTP 429 and 5xx get one retry with 2-second backoff
- **Code-fence stripping** — LLM outputs wrapped in ` ``` ` blocks (Gemini, smaller models) are unwrapped automatically
- **Cleanup language override** — the shortcut's language wins over Whisper's detected language, fixing the classic "I can only assist with English text" bug on short Czech clips
- **Shared HTTP client** with connection pooling — TLS handshakes reused across the pipeline
- **Per-session audio buffers** — old capture threads can never write into a new session's buffer
- **Poison-safe locks** — mutex poisoning won't crash the app
- **Atomic recording state transitions** — double-pressing the hotkey can't start two simultaneous sessions

---

## Settings Window

Tabs:
1. **General** — shortcuts, recording mode, microphone, start on login, updates, verbose diagnostics, crash.log access
2. **Engine** — transcription engine (cloud/local), local model picker with download, text-processing provider, API keys, model picker with refresh
3. **Output** — Raw / Clean / Polished / Custom mode, custom profile manager, auto-paste toggle, notification toggle
4. **Translation** — source language (always visible), enable toggle, target language
5. **Summary** — summary type, dedicated summary shortcut
6. **Dictionary** — personal terms for Whisper bias
7. **History** — past transcriptions with two-click clear
8. **Help** — long-form documentation of every feature, in plain English

---

## Tech Stack

- **Tauri 2** — Rust backend + React/TypeScript frontend
- **whisper.cpp / whisper-rs** — local transcription (feature-flagged, enabled by default)
- **cpal** — cross-platform audio capture
- **rusqlite** — local history storage
- **arboard + osascript** — clipboard + paste (macOS) / arboard + enigo (Windows)
- **reqwest** — shared HTTP client with connection pooling
- **tauri-plugin-global-shortcut** — system-wide hotkey registration
- **tauri-plugin-updater** — in-place updates (plumbed, activation pending)

DMG size: ~8 MB.

---

## File Locations (macOS)

- **Settings:** `~/Library/Application Support/VoiceInk/settings.json`
- **API keys:** `~/Library/Application Support/VoiceInk/keys.json`
- **History:** `~/Library/Application Support/VoiceInk/history.db`
- **Whisper models:** `~/Library/Application Support/VoiceInk/models/`
- **Model verification cache:** `~/Library/Application Support/VoiceInk/model_cache.json`
- **Crash log:** `~/Library/Application Support/VoiceInk/crash.log` (rotates at 1 MB)
