# VoiceInk — Implementation Plan

**Based on:** PRD.md v1.0  
**Tech Stack:** Tauri 2.x (Rust + TypeScript/React)  
**Design:** Dark/black-only theme  
**Platforms:** Windows 10/11 + macOS 12+  

---

## Design System

### Color Tokens

```
--bg-primary:       #000000    // Pure black — main background
--bg-secondary:     #0A0A0A    // Slightly lifted panels
--bg-tertiary:      #141414    // Cards, modals, settings panels
--bg-hover:         #1A1A1A    // Hover states
--bg-active:        #222222    // Active/pressed states

--border-subtle:    #1E1E1E    // Dividers, card borders
--border-default:   #2A2A2A    // Input borders
--border-focus:     #3B3B3B    // Focus rings

--text-primary:     #EBEBEB    // Main text
--text-secondary:   #888888    // Labels, descriptions
--text-tertiary:    #555555    // Placeholders, disabled
--text-inverse:     #000000    // Text on accent backgrounds

--accent-red:       #FF3B3B    // Recording active (pulsing)
--accent-red-dim:   #661717    // Recording idle glow
--accent-green:     #34D399    // Success, connected
--accent-yellow:    #FBBF24    // Warnings
--accent-blue:      #60A5FA    // Links, info
--accent-white:     #FFFFFF    // Primary CTA text
```

### Typography

```
Font:           "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
--text-xs:      11px / 1.4   (labels, captions)
--text-sm:      12px / 1.5   (secondary text, descriptions)
--text-base:    13px / 1.5   (body text, inputs)
--text-lg:      15px / 1.4   (section headers)
--text-xl:      18px / 1.3   (page titles)
Weight-regular: 400
Weight-medium:  500
Weight-semi:    600
```

### Spacing Scale

```
--space-1:  4px     --space-2:  8px     --space-3:  12px
--space-4:  16px    --space-5:  20px    --space-6:  24px
--space-8:  32px    --space-10: 40px    --space-12: 48px
```

### Components

```
Border radius:     6px (buttons, inputs)  |  8px (cards, panels)  |  12px (modals)  |  999px (pills, badges)
Shadows:           0 4px 12px rgba(0,0,0,0.5)  (dropdowns, modals)
Transitions:       150ms ease-out (hover)  |  200ms ease-out (expand/collapse)  |  300ms ease-in-out (modals)
Button height:     28px (sm)  |  32px (md)  |  36px (lg)
Input height:      32px
Toggle:            36×20px, 2px border, 16px knob
```

### Icon Style

- Stroke-based, 1.5px weight
- 16×16px default, 20×20px for emphasis
- Use Lucide icons (MIT, consistent, tree-shakeable)

---

## UX Copy

### System Tray Menu
```
VoiceInk                        [header]
─────────────
Recording Mode
  ○ Hold to Record
  ● Toggle (Push-to-Start)
─────────────
Output Mode
  ○ Raw
  ● Clean
  ○ Polished
─────────────
Translation        ▸ Off
Summary            ▸ Off
─────────────
Recent Activity
Settings...
─────────────
Quit VoiceInk
```

### Permission Dialogs (macOS)

**Microphone:**
> **VoiceInk needs microphone access**
> This is required to capture your speech for transcription. Audio is processed locally or sent to your configured API — never stored.
> [Open System Settings]  [Not Now]

**Accessibility (Input Monitoring):**
> **VoiceInk needs Accessibility access**
> This allows the global keyboard shortcut to work in any app and enables auto-paste into your active text field.
> [Open System Settings]  [Not Now]

**Notifications:**
> **Enable notifications?**
> Get a quick preview when transcription is complete.
> [Enable]  [Skip]

### Recording Indicator
```
State: Idle          →  (hidden)
State: Recording     →  🔴 "Recording..."  [0:03]  [Clean]
State: Processing    →  ⟳  "Transcribing..."
State: Done          →  ✓  "Pasted" (fades out after 1.5s)
State: Error         →  ✕  "Failed — tap to retry"
```

### Error States

**No API Key:**
> No API key configured. Add your OpenAI key in Settings → Engine to start transcribing.
> [Open Settings]

**Microphone Not Found:**
> No microphone detected. Connect a mic and try again.

**Transcription Failed:**
> Transcription failed. Check your connection and API key.
> [Retry]  [Copy Audio]

**Network Error:**
> Can't reach OpenAI. Check your internet or switch to local mode.
> [Retry]  [Settings]

### Empty States

**No History:**
> No transcriptions yet. Press [shortcut] to start recording.

**No Dictionary Entries:**
> Your dictionary is empty. Add custom words to improve transcription accuracy.
> [Add Word]

### Settings Window Labels

**General Tab:**
- Shortcut Key: "Activate recording" → [Key picker]
- Recording Mode: "Hold to Record" / "Toggle (Push-to-Start)"
- Start on Login: "Launch VoiceInk when you log in"
- Theme: Always dark (no toggle needed)

**Engine Tab:**
- Backend: "Cloud (OpenAI)" / "Local (Whisper)"
- API Key: "Your OpenAI API key" → [password input] → "Used for transcription and text processing. Never stored on our servers."
- Model (local): "Tiny — Fast, lower accuracy" / "Base — Balanced" / "Small — Better accuracy" / "Medium — High accuracy" / "Large — Best accuracy, slowest"
- Test: [Test Connection]

**Output Tab:**
- Mode: Raw / Clean / Polished
  - Raw: "Exact transcription. No cleanup."
  - Clean: "Punctuation, grammar, filler removal."
  - Polished: "Rewritten into clear, professional prose."
- Auto-paste: "Paste result into active app"
- Show notification: "Preview transcription after processing"

**Translation Tab:**
- Enable Translation: toggle
- Source Language: "Auto-detect" or pick
- Target Language: dropdown

**Summary Tab:**
- Enable Summary: toggle
- Summary Type: "Key Points" / "Action Items" / "TL;DR"
- Shortcut: separate key picker

**Dictionary Tab:**
- Add Word: [input] [Add]
- Word list with [×] delete buttons

**History Tab:**
- Retention: "Keep history for" → 7 days / 30 days / 90 days / Forever
- [Clear History]  [Export]

---

## Project Structure

```
voiceink/
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/             # Tauri v2 permissions
│   │   └── default.json
│   ├── icons/                    # App icons (all sizes)
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── lib.rs                # Tauri setup, plugin registration
│   │   ├── tray.rs               # System tray menu + events
│   │   ├── audio/
│   │   │   ├── mod.rs
│   │   │   ├── capture.rs        # Mic recording (cpal)
│   │   │   └── buffer.rs         # Audio buffer management
│   │   ├── transcription/
│   │   │   ├── mod.rs
│   │   │   ├── cloud.rs          # OpenAI Whisper API client
│   │   │   └── local.rs          # whisper.cpp bindings
│   │   ├── processing/
│   │   │   ├── mod.rs
│   │   │   ├── cleanup.rs        # LLM text cleanup (Raw/Clean/Polished)
│   │   │   ├── translation.rs    # Translation via LLM
│   │   │   └── summary.rs        # Summary generation
│   │   ├── paste/
│   │   │   ├── mod.rs
│   │   │   └── clipboard.rs      # Clipboard write + synthetic paste
│   │   ├── hotkey/
│   │   │   ├── mod.rs
│   │   │   └── global.rs         # Global shortcut registration
│   │   ├── permissions/
│   │   │   ├── mod.rs
│   │   │   └── platform.rs       # OS permission checks + requests
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── settings.rs       # JSON settings read/write
│   │   │   ├── history.rs        # SQLite history CRUD
│   │   │   └── dictionary.rs     # Personal dictionary
│   │   └── state.rs              # App state management
│   └── resources/
│       └── sounds/
│           ├── start.wav         # Recording start chime
│           └── stop.wav          # Recording stop chime
├── src/                          # Frontend (React + TypeScript)
│   ├── index.html
│   ├── main.tsx                  # React entry
│   ├── App.tsx                   # Router: Settings vs Overlay
│   ├── styles/
│   │   ├── globals.css           # CSS variables (design tokens)
│   │   ├── reset.css             # Minimal CSS reset
│   │   └── components.css        # Component styles
│   ├── components/
│   │   ├── ui/                   # Base UI components
│   │   │   ├── Button.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Toggle.tsx
│   │   │   ├── Select.tsx
│   │   │   ├── Tabs.tsx
│   │   │   ├── Badge.tsx
│   │   │   └── Toast.tsx
│   │   ├── settings/             # Settings window panels
│   │   │   ├── SettingsWindow.tsx
│   │   │   ├── GeneralTab.tsx
│   │   │   ├── EngineTab.tsx
│   │   │   ├── OutputTab.tsx
│   │   │   ├── TranslationTab.tsx
│   │   │   ├── SummaryTab.tsx
│   │   │   ├── DictionaryTab.tsx
│   │   │   └── HistoryTab.tsx
│   │   ├── overlay/              # Floating recording indicator
│   │   │   └── RecordingOverlay.tsx
│   │   ├── history/              # Recent activity panel
│   │   │   └── HistoryPanel.tsx
│   │   └── onboarding/           # First-run setup
│   │       └── Onboarding.tsx
│   ├── hooks/
│   │   ├── useSettings.ts        # Settings state bridge
│   │   ├── useRecording.ts       # Recording state
│   │   └── useTauriEvent.ts      # Tauri event listener
│   ├── lib/
│   │   ├── tauri.ts              # Tauri invoke wrappers
│   │   └── constants.ts          # Shared constants
│   └── types/
│       └── index.ts              # TypeScript types
├── package.json
├── tsconfig.json
├── vite.config.ts
├── PRD.md
└── PLAN.md
```

---

## Implementation Phases

### Phase 1 — Scaffold & Permissions (Foundation)

**Step 1.1: Initialize Tauri 2.x project**
- `cargo install create-tauri-app` / `npm create tauri-app@latest`
- Configure for React + TypeScript + Vite
- Set app name, identifier (`com.voiceink.app`), icons
- Configure `tauri.conf.json`:
  - Window: hidden by default (tray-only app)
  - System tray: enabled
  - Permissions: microphone, clipboard, global-shortcut, notification, shell

**Step 1.2: System tray setup** (`tray.rs`)
- Create tray icon (white mic icon on transparent bg)
- Build context menu matching UX copy above
- Handle menu item clicks → emit Tauri events
- Left-click → open recent activity window
- Right-click → context menu

**Step 1.3: OS permissions module** (`permissions/platform.rs`)
- macOS:
  - Check microphone: `AVCaptureDevice.authorizationStatus(for: .audio)`
  - Request microphone: `AVCaptureDevice.requestAccess(for: .audio)`
  - Check accessibility: `AXIsProcessTrusted()`
  - Prompt accessibility: `AXIsProcessTrustedWithOptions()` with prompt key
  - Use `macos-accessibility-client` crate
  - Detect if running on macOS 12+ for notification permissions
- Windows:
  - Check microphone: Windows Privacy settings API
  - No accessibility permission needed (global hotkeys work natively)
  - Notification permissions via Windows notification API
- Expose `check_permissions` and `request_permission` as Tauri commands
- On first launch, guide user through permission grants sequentially

**Step 1.4: First-run onboarding flow** (`Onboarding.tsx`)
- Step 1: "Welcome to VoiceInk" — app description, [Get Started]
- Step 2: "Microphone access" — explain why, [Grant Access] triggers OS prompt
- Step 3: "Accessibility access" (macOS only) — explain why, [Open System Settings]
- Step 4: "Set your shortcut" — key picker, default Fn (macOS) / Ctrl+Win (Windows)
- Step 5: "Add your API key" — input field, [Test] button, or [Use Local Mode]
- Step 6: "You're ready" — quick summary, [Start Using VoiceInk]
- Save `onboarding_complete: true` to settings
- Show onboarding window only on first launch

**Step 1.5: Settings storage** (`storage/settings.rs`)
- JSON file at `app_data_dir/settings.json`
- Default settings struct with serde:
  ```rust
  struct Settings {
      onboarding_complete: bool,
      shortcut: String,           // "Fn" or "Control+Super"
      recording_mode: RecordingMode, // Hold | Toggle
      engine: Engine,             // Cloud | Local
      api_key: Option<String>,    // encrypted at rest
      local_model: LocalModel,    // Tiny | Base | Small | Medium | Large
      output_mode: OutputMode,    // Raw | Clean | Polished
      auto_paste: bool,
      show_notification: bool,
      translation_enabled: bool,
      source_language: String,    // "auto" or ISO code
      target_language: String,    // ISO code
      summary_enabled: bool,
      summary_type: SummaryType,  // KeyPoints | ActionItems | TLDR
      summary_shortcut: Option<String>,
      history_retention_days: u32,
      start_on_login: bool,
      dictionary: Vec<String>,
  }
  ```
- Expose `get_settings`, `update_settings` as Tauri commands
- Store API key using OS keychain (keyring crate): macOS Keychain / Windows Credential Manager

---

### Phase 2 — Audio Capture & Global Hotkey

**Step 2.1: Global hotkey registration** (`hotkey/global.rs`)
- Use `tauri-plugin-global-shortcut` (Tauri v2 plugin)
- Register configured shortcut on app startup
- Hold-to-record mode:
  - Key down → start recording, emit `recording-started` event
  - Key up → stop recording, emit `recording-stopped` event
- Toggle mode:
  - Key press → toggle recording state
  - First press → start, second press → stop
- Re-register when user changes shortcut in settings
- Handle shortcut conflicts gracefully — show warning toast

**Step 2.2: Audio capture** (`audio/capture.rs`)
- Use `cpal` crate for cross-platform mic access
- List available input devices, use system default
- Record to in-memory buffer (`Vec<f32>` samples)
- Sample rate: 16kHz (Whisper optimal), mono
- Convert to WAV format in memory for API upload
- Convert to f32 PCM for local whisper.cpp
- Play start/stop chime sounds via `rodio` crate
- Emit `audio-level` events for waveform visualization (every 50ms)

**Step 2.3: Recording overlay window** (`RecordingOverlay.tsx`)
- Tauri: create a secondary window — transparent, always-on-top, no decorations, click-through
- Position: bottom-center of screen, 20px from bottom
- Size: ~200×48px
- Shows:
  - Pulsing red dot (CSS animation, 1s pulse) when recording
  - Duration counter (mm:ss)
  - Current output mode badge ("Clean", "Polished", etc.)
  - Spinner icon when processing
  - Checkmark when done, auto-hide after 1.5s
  - Error with retry on failure
- Communicate state via Tauri events from Rust → JS
- Frosted glass effect: `backdrop-filter: blur(20px)` on `--bg-tertiary` with 80% opacity

---

### Phase 3 — Transcription Engines

**Step 3.1: OpenAI Whisper API client** (`transcription/cloud.rs`)
- Use `reqwest` for HTTP client
- Endpoint: `POST https://api.openai.com/v1/audio/transcriptions`
- Send WAV audio as multipart form data
- Model: `whisper-1`
- Include language hint if user set source language
- Parse JSON response → raw transcript string
- Handle errors: 401 (bad key), 413 (file too large), 429 (rate limit), 500+
- Timeout: 30 seconds
- Return `Result<String, TranscriptionError>`

**Step 3.2: Local Whisper integration** (`transcription/local.rs`)
- Use `whisper-rs` crate (Rust bindings for whisper.cpp)
- On first use of local mode, download model:
  - Show progress bar in settings window
  - Store models in `app_data_dir/models/`
  - Model sizes: tiny (~75MB), base (~142MB), small (~466MB), medium (~1.5GB), large (~2.9GB)
- Load model into memory on engine switch
- Feed PCM audio buffer → get transcript segments
- Run transcription on a dedicated thread (not blocking UI)
- Support Apple Silicon acceleration (Core ML / Metal via whisper.cpp)
- Support CUDA on Windows if available

**Step 3.3: Engine selector** (`transcription/mod.rs`)
- Match on `settings.engine`:
  - `Cloud` → call cloud::transcribe(audio_buffer, api_key, language)
  - `Local` → call local::transcribe(audio_buffer, model, language)
- Return unified `TranscriptionResult { text: String, language: String, duration_ms: u64 }`

---

### Phase 4 — Text Processing Pipeline

**Step 4.1: Text cleanup** (`processing/cleanup.rs`)
- Three modes implemented as different system prompts:
  - **Raw:** No processing, return transcript as-is
  - **Clean:** System prompt:
    ```
    Clean up this speech transcript. Fix punctuation, capitalization, and grammar.
    Remove filler words (um, uh, hmm, like, you know, so basically) and false starts.
    Preserve the speaker's original meaning and voice. Do not rewrite.
    If the speaker lists items, use bullet points.
    Return only the cleaned text, nothing else.
    ```
  - **Polished:** System prompt:
    ```
    Rewrite this speech transcript into clear, professional prose.
    Fix all grammar, punctuation, and structure.
    Remove all filler words and hesitations.
    Improve clarity while preserving the core message.
    Format with paragraphs and lists where appropriate.
    Return only the polished text, nothing else.
    ```
- Cloud: OpenAI `POST /v1/chat/completions` with `gpt-4o-mini`
- Local: Send to Ollama API (`http://localhost:11434/api/generate`) if configured
- Include personal dictionary words as context: "These proper nouns should be spelled exactly: [words]"

**Step 4.2: Translation** (`processing/translation.rs`)
- When `settings.translation_enabled`:
  - After cleanup, send to LLM with prompt:
    ```
    Translate the following text from {source} to {target}.
    Preserve formatting. Return only the translation.
    ```
  - Source language: auto-detected from transcript or user-set
  - Target language: from settings
- Use same backend as cleanup (cloud or local LLM)

**Step 4.3: Summary** (`processing/summary.rs`)
- When `settings.summary_enabled`:
  - After cleanup (and optionally translation), summarize:
  - **Key Points:**
    ```
    Extract the key points from this text as a bullet-point list.
    Be concise. Return only the bullet points.
    ```
  - **Action Items:**
    ```
    Extract all action items and tasks from this text as a checklist.
    Return only the action items, each prefixed with "- [ ] ".
    ```
  - **TL;DR:**
    ```
    Write a one-paragraph summary of this text. Be concise and clear.
    Return only the summary paragraph.
    ```

**Step 4.4: Processing pipeline** (`processing/mod.rs`)
- Chain: `raw_transcript → cleanup → translation → summary`
- Each step optional based on settings
- Emit progress events: `processing-step: { step: "cleanup" | "translation" | "summary", status: "started" | "done" }`
- Total pipeline timeout: 30s cloud, 60s local

---

### Phase 5 — Paste & Clipboard

**Step 5.1: Clipboard + paste** (`paste/clipboard.rs`)
- Save current clipboard contents before overwriting
- Write processed text to system clipboard:
  - macOS: `NSPasteboard` via `arboard` crate
  - Windows: Win32 clipboard API via `arboard` crate
- Simulate Cmd+V (macOS) / Ctrl+V (Windows):
  - macOS: `CGEvent` with virtual keycodes
  - Windows: `SendInput` with `INPUT_KEYBOARD`
  - Use `enigo` crate for cross-platform key simulation
- After paste, restore original clipboard contents (with 500ms delay)
- Handle edge case: no focused text field → show toast "Couldn't paste. Text copied to clipboard."

**Step 5.2: Notification toast**
- Use `tauri-plugin-notification` for system notifications
- Show: preview of first 100 chars of transcribed text
- Action: "Undo" button → simulate Cmd+Z / Ctrl+Z in the active app
- Auto-dismiss after 4 seconds

---

### Phase 6 — Settings UI & History

**Step 6.1: Settings window** (`SettingsWindow.tsx`)
- Tauri: create window on demand, centered, 560×480px, resizable
- Tab navigation: left sidebar with icon + label per tab
- All tabs as described in UX copy section above
- Shortcut key picker: capture next key combo on focus
- API key: password input with show/hide toggle
- Local model: download button with progress bar per model
- All changes save immediately (no Save button needed)
- Close window with Escape or window close button

**Step 6.2: Design token implementation** (`globals.css`)
- All CSS variables from design system above
- Dark theme only — no light mode
- Inter font loaded locally (bundled, not from CDN)
- Smooth transitions on interactive elements
- Focus-visible outlines for keyboard navigation

**Step 6.3: Base UI components** (`components/ui/`)
- `Button`: variants (primary, secondary, ghost, danger), sizes (sm, md, lg), loading state
- `Input`: text, password (with toggle), with optional label + description
- `Toggle`: on/off with label
- `Select`: dropdown with options
- `Tabs`: horizontal tab bar
- `Badge`: small colored label (for mode indicators)
- `Toast`: slide-up notification with auto-dismiss
- All components: keyboard accessible, proper ARIA attributes

**Step 6.4: History** (`storage/history.rs` + `HistoryPanel.tsx`)
- SQLite database at `app_data_dir/history.db`
- Schema:
  ```sql
  CREATE TABLE transcriptions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      raw_text TEXT NOT NULL,
      processed_text TEXT NOT NULL,
      mode TEXT NOT NULL,          -- Raw/Clean/Polished
      language TEXT,
      translated_text TEXT,
      summary TEXT,
      duration_secs REAL,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );
  ```
- Use `rusqlite` or `tauri-plugin-sql`
- Tauri commands: `get_history(limit, offset)`, `delete_history(id)`, `clear_history()`, `export_history()`
- UI: scrollable list, each entry shows timestamp + first line preview
- Click to expand full text
- Copy button per entry
- Re-paste button per entry
- Auto-cleanup based on retention setting

---

### Phase 7 — Personal Dictionary & Launch on Login

**Step 7.1: Personal dictionary** (`storage/dictionary.rs` + `DictionaryTab.tsx`)
- Store in settings JSON as array of strings
- UI: input field + [Add] button, list with delete per word
- Inject into cleanup prompt as context
- Inject into Whisper API as `prompt` parameter (improves recognition)

**Step 7.2: Launch on login**
- macOS: Create/remove Login Item via `SMAppService` (macOS 13+) or LaunchAgent plist
- Windows: Add/remove registry key at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- Use `tauri-plugin-autostart`
- Toggle in settings General tab

---

### Phase 8 — Polish & Platform Builds

**Step 8.1: Auto-update**
- Use `tauri-plugin-updater`
- Check for updates on launch + every 24h
- Download in background, prompt to restart

**Step 8.2: Performance optimization**
- Lazy-load local Whisper model (don't load until first local transcription)
- Release audio buffer immediately after transcription
- Keep idle RAM < 150MB (monitor with Instruments / Task Manager)
- App binary < 30MB without models

**Step 8.3: Platform builds**
- macOS: Universal binary (Intel + Apple Silicon), .dmg installer
  - Sign with Apple Developer certificate
  - Notarize for Gatekeeper
  - Info.plist: microphone usage description, accessibility description
- Windows: .msi installer via WiX
  - Sign with code signing certificate
  - Include VC++ redistributable if needed

**Step 8.4: Error handling & resilience**
- Graceful fallback: if cloud fails, suggest switching to local
- Retry logic: 1 retry with 2s delay for transient network errors
- All errors surface as user-friendly toasts (per UX copy)
- Crash handler: log to file, offer to send report (opt-in)

---

## Dependency Summary

### Rust (Cargo.toml)
```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "protocol-asset"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-notification = "2"
tauri-plugin-autostart = "2"
tauri-plugin-updater = "2"
tauri-plugin-sql = { version = "2", features = ["sqlite"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["multipart", "json"] }
tokio = { version = "1", features = ["full"] }
cpal = "0.15"                    # Audio capture
rodio = "0.19"                   # Audio playback (chimes)
arboard = "3"                    # Clipboard
enigo = "0.2"                    # Key simulation
whisper-rs = "0.12"              # Local Whisper
keyring = "3"                    # OS keychain for API key
rusqlite = { version = "0.32", features = ["bundled"] }
hound = "3.5"                    # WAV encoding
uuid = { version = "1", features = ["v4"] }
log = "0.4"
env_logger = "0.11"
anyhow = "1"
thiserror = "2"
```

### macOS-specific
```toml
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"
objc = "0.2"
core-foundation = "0.10"
```

### Frontend (package.json)
```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-global-shortcut": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "@tauri-apps/plugin-sql": "^2",
    "react": "^19",
    "react-dom": "^19",
    "lucide-react": "^0.400"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "typescript": "^5.5",
    "vite": "^6",
    "@vitejs/plugin-react": "^4"
  }
}
```

---

## Required OS Permissions

| Permission | macOS | Windows | Why |
|---|---|---|---|
| Microphone | `NSMicrophoneUsageDescription` + runtime request | Settings → Privacy → Microphone | Audio capture |
| Accessibility | `AXIsProcessTrustedWithOptions` prompt | Not needed | Global shortcut + paste simulation |
| Input Monitoring | Bundled with Accessibility on macOS 10.15+ | Not needed | Detecting key events globally |
| Notifications | `UNUserNotificationCenter.requestAuthorization` | Auto-granted | Toast previews |
| Autostart | Login Items / LaunchAgent | Registry entry | Start on login |
| Network | App Sandbox exception or disable sandbox | Auto-granted | Cloud API calls |
| Keychain | Automatic with app signature | Credential Manager | Secure API key storage |

---

## Build Order (What to Code First)

```
1.  Scaffold Tauri 2 project + React + Vite
2.  Design tokens CSS + base UI components
3.  System tray with context menu
4.  Settings storage (JSON) + settings window UI
5.  Permission checks + request flows
6.  Onboarding flow
7.  Global hotkey (hold + toggle modes)
8.  Audio capture with cpal
9.  Recording overlay window
10. OpenAI Whisper API transcription
11. Text cleanup (Raw/Clean/Polished) via GPT
12. Clipboard paste into active app
13. End-to-end flow: shortcut → record → transcribe → clean → paste
14. Translation mode
15. Summary mode
16. SQLite history
17. Personal dictionary
18. Local Whisper model support
19. Launch on login
20. Auto-update
21. Platform builds (dmg + msi)
```

Each step produces a testable increment. Step 13 is the first full working demo.
