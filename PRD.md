# Product Requirements Document (PRD)

## VoiceInk — Intelligent Voice-to-Text for Desktop

**Version:** 1.0  
**Last Updated:** April 13, 2026  
**Status:** Draft  

---

## 1. Overview

VoiceInk is a lightweight, always-on desktop application for Windows and macOS that converts speech into clean, polished text in real time. It runs quietly in the system tray with minimal resource usage and activates instantly via a global keyboard shortcut. The app leverages either the OpenAI API (Whisper + GPT) or a local LLM to transcribe, clean, correct, summarize, and translate spoken input — then pastes the result directly into whatever application the user is working in.

The core philosophy: **speak naturally, get perfect text.** No switching apps, no copy-pasting, no editing out "umm" and "uhh" afterward.

---

## 2. Problem Statement

Typing is slow. Most people speak 3–4× faster than they type, yet existing voice dictation tools produce raw, unstructured transcripts full of filler words, missing punctuation, and broken grammar. Users end up spending as much time cleaning the output as they saved by not typing.

Current solutions also tend to be heavy, cloud-only, expensive, or locked to a single platform. There is a clear gap for a minimal, cross-platform, privacy-conscious tool that handles the full pipeline — from raw speech to publication-ready text — in one step.

---

## 3. Target Users

- **Knowledge workers** who write emails, Slack messages, documents, and reports throughout the day.
- **Developers** who want to dictate comments, documentation, commit messages, or instructions to AI coding agents.
- **Content creators** drafting blog posts, social media copy, or scripts.
- **Non-native speakers** who want real-time translation of their speech into another language.
- **Users with RSI, carpal tunnel, or motor impairments** who need an alternative to keyboard input.

---

## 4. Goals and Success Metrics

| Goal | Metric | Target |
|---|---|---|
| Fast activation | Time from shortcut press to recording start | < 300ms |
| Low resource usage | Idle RAM consumption | < 150 MB |
| High accuracy | Word error rate on clean speech (English) | < 5% |
| Filler removal | "um", "uh", "hmm" removed from output | 100% |
| User retention | Weekly active users returning after 4 weeks | > 60% |

---

## 5. Core Features

### 5.1 Global Shortcut Activation

The app supports two recording modes triggered by a configurable global hotkey (default: `Ctrl+Win` on Windows, `Fn` on macOS):

- **Hold-to-Record:** Press and hold the shortcut to record. Release to stop and process. Best for short dictations (messages, quick notes).
- **Push-to-Start / Push-to-Stop (Toggle Mode):** Press once to begin recording, press again to stop. Best for longer dictations (paragraphs, brain dumps). A subtle audio chime and a small floating indicator confirm start and stop.

Users select their preferred mode in settings. The shortcut is fully customizable and supports modifier combinations and mouse buttons.

### 5.2 Speech-to-Text Engine

Transcription is handled by one of two backends, selectable in settings:

- **Cloud (OpenAI API):** Uses the Whisper API for transcription. Requires a user-provided API key. Offers the highest accuracy and supports 90+ languages out of the box.
- **Local LLM:** Uses a locally running Whisper model (e.g., `whisper.cpp`, `faster-whisper`) for fully offline, private transcription. Model size is configurable (tiny → large) to balance speed vs. accuracy.

The app records audio to a temporary buffer, sends it to the selected engine upon recording stop, and receives the raw transcript.

### 5.3 Intelligent Text Cleanup (Post-Processing)

After transcription, the raw text passes through an LLM-powered cleanup step. This can use OpenAI (GPT-4o-mini or similar) or a local model. The cleanup performs:

- **Punctuation and grammar:** Adds commas, periods, question marks, capitalization, and paragraph breaks based on natural speech cadence and content.
- **Filler word removal:** Strips out "um", "uh", "hmm", "like", "you know", "so basically", repeated words, false starts, and self-corrections.
- **Sentence restructuring:** Lightly smooths awkward phrasing while preserving the speaker's original meaning and voice. Does not rewrite — only cleans.
- **Formatting:** Applies contextual formatting such as bullet lists (when the user says "first… second… third…"), numbered steps, or paragraph separation.

The cleanup prompt is tunable. Users can choose between three output modes:

| Mode | Behavior |
|---|---|
| **Raw** | No cleanup. Exact transcription including fillers. |
| **Clean** | Punctuation, grammar, filler removal. Meaning-preserving. |
| **Polished** | Full rewrite into clear, professional prose. |

### 5.4 Translation Mode

Users can set a target language in settings. When enabled, the app transcribes speech in the spoken language and then translates the cleaned output into the target language before pasting. Useful for writing emails in a second language by speaking in your native tongue.

Supported via OpenAI API or a local translation model. Language pairs are configurable (e.g., Czech → English, Spanish → French).

### 5.5 Summary Mode

For longer recordings (toggle mode), the user can enable summarization. Instead of (or in addition to) the full transcript, the app generates a concise summary of the spoken content. Output options:

- **Key points** — Bullet-point summary of main ideas.
- **Action items** — Extracted tasks and next steps.
- **TL;DR** — One-paragraph condensed version.

Summary mode can be activated per-session via a separate shortcut or always-on in settings.

### 5.6 Auto-Paste into Active Application

After processing, the final text is automatically pasted into the currently focused text field (Slack, Gmail, VS Code, Notion, Word, browser, terminal, etc.) using system clipboard simulation. The user never leaves their workflow.

An optional notification toast shows a preview of the pasted text with an "Undo" action (restores the clipboard and removes the pasted text via `Ctrl+Z` / `Cmd+Z`).

---

## 6. Additional Features

### 6.1 Personal Dictionary

Users can add custom words, names, technical terms, and brand names that the transcription engine should recognize and spell correctly (e.g., "Kubernetes", "Svelte", "VoiceInk").

### 6.2 History and Recent Activity

A small panel accessible from the tray icon shows recent transcriptions with timestamps. Users can copy, re-paste, or edit past results.

### 6.3 Context-Aware Tone (Future)

The app detects which application is focused and adjusts the cleanup tone accordingly — more formal for email clients, casual for messaging apps, technical for IDEs.

### 6.4 Voice Shortcuts (Future)

Users can define trigger phrases that expand into predefined text snippets (e.g., saying "my email" pastes the user's email address, saying "standard greeting" pastes a saved intro paragraph).

---

## 7. Architecture

```
┌─────────────────────────────────────────────────────┐
│                    System Tray App                   │
│          (Electron / Tauri — cross-platform)         │
├──────────┬──────────┬───────────┬────────────────────┤
│ Shortcut │  Audio   │  Engine   │   Post-Processor   │
│ Listener │ Capture  │ Selector  │                    │
│          │ (mic →   │           │  ┌──────────────┐  │
│  Global  │  buffer) │ ┌──────┐  │  │ Filler strip │  │
│  hotkey  │          │ │Cloud │  │  │ Punctuation  │  │
│  (hold / │          │ │Whisper│ │  │ Grammar fix  │  │
│  toggle) │          │ ├──────┤  │  │ Translation  │  │
│          │          │ │Local │  │  │ Summary      │  │
│          │          │ │LLM   │  │  └──────────────┘  │
├──────────┴──────────┴─┴──────┴──┴────────────────────┤
│              Clipboard → Active App Paste             │
└─────────────────────────────────────────────────────┘
```

### Technology Considerations

| Component | Options |
|---|---|
| App shell | **Tauri** (preferred — Rust backend, small binary, low RAM) or Electron |
| Audio capture | Platform-native APIs (CoreAudio on macOS, WASAPI on Windows) |
| Cloud STT | OpenAI Whisper API |
| Local STT | `whisper.cpp` / `faster-whisper` bundled or downloadable |
| Cloud LLM | OpenAI GPT-4o-mini (cleanup, translation, summary) |
| Local LLM | Ollama, llama.cpp, or similar (user-managed) |
| Clipboard | Platform-native clipboard + synthetic key events |
| Storage | SQLite for history, JSON for settings |

---

## 8. User Interface

The app is intentionally minimal. There is no main window during normal use.

### 8.1 System Tray Icon

- Left-click: Open recent activity panel.
- Right-click: Settings, mode selection (Clean / Polished / Raw), translation on/off, quit.

### 8.2 Floating Recording Indicator

When recording is active, a small, unobtrusive overlay appears near the cursor or screen edge showing:

- A pulsing microphone icon.
- A live audio waveform.
- Recording duration.
- The selected mode (Clean, Translate, Summarize).

The indicator disappears automatically after processing is complete.

### 8.3 Settings Window

Accessed from the tray menu. Organized into tabs:

- **General** — Shortcut key, recording mode (hold vs. toggle), startup on login, theme (light/dark/system).
- **Engine** — Cloud vs. local, API key input, model selection, local model download/management.
- **Output** — Cleanup mode (Raw/Clean/Polished), auto-paste on/off, notification preferences.
- **Translation** — Enable/disable, source language (auto-detect or fixed), target language.
- **Summary** — Enable/disable, summary type (key points / action items / TL;DR), separate shortcut.
- **Dictionary** — Add/remove custom words and voice shortcuts.
- **History** — Retention period, clear history, export.

---

## 9. Privacy and Security

- **Local-first option:** Users who choose the local LLM path have fully offline processing. No audio or text leaves the device.
- **Cloud path:** Audio is sent to OpenAI's API using the user's own API key. No data is stored by VoiceInk servers (there are no VoiceInk servers — the app communicates directly with the provider).
- **Temporary audio:** Audio buffers are held in memory only during processing and discarded immediately after transcription.
- **No telemetry** by default. Optional anonymous usage analytics (opt-in only).

---

## 10. Performance Requirements

| Metric | Target |
|---|---|
| Idle CPU usage | < 0.5% |
| Idle RAM usage | < 150 MB (Tauri) / < 300 MB (Electron) |
| Recording start latency | < 300ms from shortcut press |
| Cloud processing (10s clip) | < 3s total (transcribe + cleanup) |
| Local processing (10s clip, medium model) | < 5s on Apple Silicon / modern x86 |
| App binary size | < 30 MB (without local models) |
| Startup time | < 2s to system tray ready |

---

## 11. Platform Support

| Platform | Version |
|---|---|
| Windows | 10 (21H2+), 11 |
| macOS | 12 Monterey and later (Intel + Apple Silicon) |

Linux support is a future consideration.

---

## 12. Competitive Landscape

| Feature | VoiceInk | Wispr Flow | SuperWhisper | WhisperTyping |
|---|---|---|---|---|
| Windows + macOS | ✅ | ✅ | ✅ | Windows only |
| Hold-to-record | ✅ | ✅ | ✅ | ✅ |
| Toggle record | ✅ | ✅ (hands-free) | ✅ | ❌ |
| Filler removal | ✅ | ✅ | ✅ | ❌ |
| Punctuation/grammar | ✅ | ✅ | ✅ | ✅ |
| Translation | ✅ | Limited | ✅ | ❌ |
| Summary mode | ✅ | ❌ | ❌ | ❌ |
| Local/offline LLM | ✅ | ❌ (cloud only) | ✅ | ❌ |
| Open API key (BYOK) | ✅ | ❌ (subscription) | ✅ | ✅ |
| Lightweight (< 150 MB) | ✅ | ❌ (~800 MB) | ~500 MB | ✅ |
| Open source | Planned | ❌ | ❌ | ❌ |

### Key Differentiators

1. **Hybrid engine** — seamless switch between cloud and local processing.
2. **Summary mode** — unique feature for extracting key points from longer recordings.
3. **Ultra-lightweight** — designed to run unnoticed in the background.
4. **BYOK (Bring Your Own Key)** — no subscription, no vendor lock-in.

---

## 13. Milestones and Roadmap

### Phase 1 — MVP (v0.1)

- System tray app with global shortcut (hold-to-record).
- OpenAI Whisper API transcription.
- GPT-powered cleanup (filler removal, punctuation, grammar).
- Auto-paste into active application.
- Settings: API key, shortcut customization, output mode.
- Windows and macOS builds.

### Phase 2 — Core Features (v0.5)

- Toggle (push-to-start / push-to-stop) recording mode.
- Local Whisper model support (offline transcription).
- Translation mode.
- Summary mode (key points, action items, TL;DR).
- Personal dictionary.
- History panel with search.

### Phase 3 — Polish (v1.0)

- Local LLM support for cleanup/translation/summary (Ollama integration).
- Context-aware tone adjustment.
- Voice shortcuts.
- Onboarding flow and first-run tutorial.
- Auto-update mechanism.
- Performance optimization pass.

### Phase 4 — Growth (v1.x+)

- Linux support.
- Team features (shared dictionaries, usage analytics).
- Plugin/extension API.
- Open-source release.

---

## 14. Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| OpenAI API latency spikes | Slow user experience | Local fallback engine; queue and retry logic |
| Global shortcut conflicts | Shortcut doesn't trigger | Conflict detection on setup; fully customizable keys |
| Microphone permissions (macOS) | App cannot record | Clear permission request flow; troubleshooting guide |
| Local model download size (~1–3 GB) | Friction for offline users | Progressive download; start with cloud, download in background |
| Clipboard conflicts with other apps | Paste fails or overwrites | Clipboard save/restore around paste; configurable paste method |
| Privacy concerns with cloud processing | User distrust | Prominent local-only mode; no VoiceInk servers in the data path |

---

## 15. Open Questions

1. Should the app support streaming transcription (show words as they are spoken) or only batch (process after recording stops)?
2. What is the right default cleanup mode — Clean or Polished?
3. Should summary mode output to a separate window/panel rather than pasting inline?
4. Is there demand for a "command mode" (similar to Wispr Flow) where the user can give voice instructions to transform highlighted text?
5. Pricing model: fully free + BYOK, freemium with built-in credits, or one-time purchase?

---

## 16. Appendix: Inspiration Sources

- **Wispr Flow** — market leader in intelligent voice dictation; strong UX with hold-to-record and auto-formatting; cloud-only; subscription model; high RAM usage.
- **WhisperTyping** — Windows-focused dictation tool; uses OpenAI Whisper; simple hotkey activation; lacks cleanup/filler removal.
- **SuperWhisper** — macOS/Windows; local + cloud models; custom AI modes; SOC 2 compliant.
- **Quick Whisper** — open-source speech-to-copy-edited-text for Windows; uses OpenAI API; auto-paste feature; good reference for MVP scope.
