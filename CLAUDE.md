# TypeUrVoice – Project Notes for Claude

## What Was Built

TypeUrVoice is a macOS/Windows desktop voice-to-text app built with Tauri 2 (Rust backend + React/TypeScript frontend). It lives in the system tray (menu bar), has no dock icon, and transcribes speech via a global hotkey.

### Core Architecture
- **Tauri 2** — Rust backend + Vite/React frontend
- **Audio capture** — `cpal` via a dedicated `std::thread::spawn` (cpal::Stream is !Send on macOS)
- **Transcription** — OpenAI Whisper API (cloud) or local whisper.cpp via `whisper-rs` (local)
- **Text processing** — LLM-based cleanup/polish via OpenAI, Ollama, or LM Studio (all OpenAI-compatible endpoints)
- **Auto-paste** — `arboard` (clipboard) + `osascript` AppleScript on macOS (enigo on Windows only)
- **Hotkey** — `tauri-plugin-global-shortcut` with Hold and Toggle recording modes
- **Storage** — `rusqlite` for history, JSON file for settings, file-based API key storage
- **Tray** — built programmatically in `tray.rs`, icon embedded via `include_bytes!`

### Key Files
- `src-tauri/src/lib.rs` — main pipeline, hotkey handler, Tauri setup, `AXIsProcessTrustedWithOptions` FFI, `log_to_file()` crash logger
- `src-tauri/src/tray.rs` — tray icon, menu, event handling, `refresh_menu()` for syncing state
- `src-tauri/src/state.rs` — shared app state (`AppState`, `Settings`, `QuitFlag`), all enums and config structs; includes dual shortcut fields (`shortcut`/`primary_language`, `secondary_shortcut`/`secondary_language`) and `active_language` override per recording
- `src-tauri/src/audio/capture.rs` — cpal audio recording with 5-minute max
- `src-tauri/src/audio/buffer.rs` — PCM to WAV conversion
- `src-tauri/src/transcription/mod.rs` — engine dispatcher (cloud vs local) with timeout + catch_unwind
- `src-tauri/src/transcription/cloud.rs` — OpenAI Whisper API transcription
- `src-tauri/src/transcription/local.rs` — local whisper.cpp inference, model download, hallucination filter
- `src-tauri/src/processing/mod.rs` — LLM orchestration (cleanup → translation → summary), `GptConfig`, `gpt_complete()`
- `src-tauri/src/processing/cleanup.rs` — Clean and Polished prompt builders
- `src-tauri/src/paste/clipboard.rs` — clipboard write + paste simulation (osascript on macOS, enigo on Windows)
- `src-tauri/src/storage/settings.rs` — JSON settings load/save
- `src-tauri/src/storage/history.rs` — SQLite history CRUD
- `src-tauri/src/storage/dictionary.rs` — personal dictionary for Whisper bias
- `src-tauri/src/permissions/platform.rs` — macOS/Windows permission checks
- `src/components/settings/SettingsWindow.tsx` — settings window with tab navigation
- `src/components/settings/TranslationTab.tsx` — source language (always visible) + translation toggle + target language
- `src/components/settings/OutputTab.tsx` — output mode selector (Raw/Clean/Polished) + auto-paste + notification toggles
- `src/components/ui/Toggle.tsx` — toggle switch component (label-based, single onChange)
- `src/hooks/useSettings.ts` — settings state management with optimistic updates + `settings-changed` event listener
- `src/lib/tauri.ts` — Tauri invoke wrappers
- `src/types/index.ts` — TypeScript types
- `src/lib/constants.ts` — LANGUAGES list, LOCAL_MODELS list

### Local Model Support
- Feature flag: `local-whisper` (enabled by default in `Cargo.toml`)
- Requires `cmake` to build — installed via Homebrew at `/opt/homebrew/bin/cmake`
- Models stored at `~/Library/Application Support/TypeUrVoice/models/`
- All recommended models are **multilingual Q5** (support Czech, Slovak, English, etc.)
- English-only models (base-en, tiny-en) are available but not suitable for Czech/Slovak
- Short audio (<1.5s) is padded with silence to meet whisper.cpp minimum

### Important Rust Patterns
- `tokio::spawn` → must use `tauri::async_runtime::spawn` in sync hotkey callbacks
- `cpal::Stream` is `!Send` on macOS → audio runs on dedicated `std::thread`
- `RunEvent::ExitRequested` checks `QuitFlag` — prevents exit unless user clicked "Quit TypeUrVoice" from tray
- `on_window_event` with `prevent_close` + `window.hide()` hides windows instead of destroying them
- `AXIsProcessTrustedWithOptions` called via FFI at startup to prompt for accessibility permission
- Recording state resets to Idle after 2s so subsequent recordings work
- Cmd+Q intercepted via global shortcut — hides windows instead of quitting (macOS bypasses `prevent_exit`)
- Hold mode enforces 600ms minimum recording — if key released too quickly, waits the remaining time
- `recording_started_at` timestamp on `AppState` tracks when recording began
- Overlay window uses `set_ignore_cursor_events(true)` to avoid stealing focus
- Overlay is hidden BEFORE paste step (step 3.5 in pipeline) so focus returns to user's app
- Paste uses `osascript` on macOS (not enigo) — runs as subprocess, can never SIGSEGV the main process
- `enigo` is Windows-only dependency (`[target.'cfg(target_os = "windows")'.dependencies]`)
- `log_to_file()` writes to `~/Library/Application Support/TypeUrVoice/crash.log` — survives process death
- Panic hook logs to crash.log before crashing
- Pipeline steps logged to crash.log for post-mortem diagnosis

### LLM Text Processing
- Supports three backends: OpenAI API, Ollama (local), LM Studio (local)
- All use the same OpenAI-compatible `/v1/chat/completions` endpoint
- `gpt_complete()` handles reasoning models (e.g. Gemma) — reads `reasoning_content` field as fallback when `content` is empty
- `Message.content` is `Option<String>` to handle null responses
- If cleanup/polish returns empty text, pipeline falls back to raw transcript (never pastes nothing)
- Clean mode prompt: grammar-focused, preserves speaker's exact words
- Polished mode prompt: rewrites into professional prose

### Settings Sync
- Tray menu → Settings UI: tray emits `settings-changed` event, `useSettings` hook listens and reloads
- Settings UI → Tray menu: `update_settings` command calls `tray::refresh_menu()` to rebuild tray menu
- `refresh_menu()` rebuilds the menu with correct output mode checkmark
- Both directions keep tray checkmarks and UI radio buttons in sync

### Settings Files
- **Main settings:** `~/Library/Application Support/TypeUrVoice/settings.json` — all settings except API key
  - Defaults: engine=local, localModel=small-q5, outputMode=clean, sourceLanguage=auto, recordingMode=hold, shortcut=Option+Space (macOS), primaryLanguage=cs, secondaryLanguage=en, secondaryShortcut=null
- **API key:** `~/.local/share/TypeUrVoice/.api_key` — OpenAI API key (file-based, not keychain)

---

## Resolved Issues

### 1. Accessibility Permission for Auto-paste
App calls `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt: true` at startup via FFI. macOS shows the native dialog. User must grant in System Settings → Privacy & Security → Accessibility.

### 2. Tray Icon
Embedded icon via `include_bytes!("../icons/32x32.png")` in `tray.rs`. Works in both dev and production builds.

### 3. App Crash on Paste (SIGSEGV from enigo)
**Root cause:** `enigo` called CoreGraphics FFI to simulate Cmd+V. Without Accessibility permission, `CGEventCreateKeyboardEvent` returned NULL, enigo dereferenced it → SIGSEGV. `catch_unwind` cannot catch OS signals.
**Fix:** Replaced enigo with `osascript` (AppleScript) on macOS. Runs as a child process — if it fails, TypeUrVoice gets an error message, never crashes. enigo kept only for Windows.
**Diagnosis method:** `crash.log` showed "Step 4: Pasting text" with no subsequent "Pipeline completed" entry.

### 4. Recording Too Short in Hold Mode
**Root cause:** Global shortcut Released event could fire almost immediately after Pressed.
**Fix:** Added 600ms minimum recording duration. If key released before 600ms, a background thread waits the remaining time before stopping.

### 5. Czech Language Not Detected
**Root cause:** `sourceLanguage: "auto"` — Whisper auto-detect unreliable for Czech on short clips.
**Fix:** Source language selector moved to always-visible section in Translation tab (not hidden behind translation toggle). User can set explicitly to "cs" for Czech.

### 6. Overlay Stealing Focus → Paste Goes to Wrong App
**Root cause:** Overlay window (`alwaysOnTop: true`) stole focus when shown. Paste simulated Cmd+V into the overlay instead of user's app.
**Fix:** (a) `set_ignore_cursor_events(true)` on overlay, (b) overlay hidden before paste step with 200ms delay for macOS focus return.

### 7. Quit TypeUrVoice Did Nothing
**Root cause:** `ExitRequested` handler called `api.prevent_exit()` unconditionally — blocked even intentional quit from tray menu.
**Fix:** Added `QuitFlag` (atomic bool). Tray quit handler sets flag before `app.exit(0)`. `ExitRequested` handler checks flag — only prevents exit when flag is false.

### 8. Translation Toggle Not Working
**Root cause:** Toggle component had both `<label>` wrapper (which toggles the hidden checkbox on click) AND an `onClick` on the visual `<div>` — double-firing `onChange`.
**Fix:** Removed `onClick` from the visual toggle div. The `<label>` already handles all clicks.

### 9. LLM Returning Empty Content (Reasoning Models)
**Root cause:** Gemma and other reasoning models put output in `reasoning_content` field and return `content: ""`.
**Fix:** `gpt_complete()` checks `reasoning_content` as fallback. `Message.content` is now `Option<String>`. If both empty, cleanup falls back to raw transcript.

### 10. Clean Mode Changing Meaning
**Root cause:** Prompt instructed LLM to "remove filler words and false starts" which changed speaker's meaning.
**Fix:** Clean prompt now focuses on grammar/punctuation only: "Keep ALL of the speaker's original words — do not remove, rephrase, or rewrite anything."

### 11. Tray Menu / Settings UI Mode Desync
**Root cause:** Changing output mode in tray didn't update Settings UI radio button, and vice versa.
**Fix:** `tray::refresh_menu()` rebuilds tray menu. Called from both `handle_menu_event` (tray → tray checkmark) and `update_settings` command (UI → tray). Tray → UI already worked via `settings-changed` event.

### 12. DMG Bundling Script Fails
Tauri's `bundle_dmg.sh` fails. Workaround: create DMG manually with `hdiutil create`.

### 13. OpenAI API Key Not Persisting
**Root cause:** `keyring` crate (v3) fails silently on unsigned dev binaries (no keychain entitlements). UI showed key saved but it never made it to the keychain. On load, `load_api_key()` returned None → "No API key configured" error.
**Fix:** Removed `keyring` dependency entirely. API key now saves directly to `~/.local/share/TypeUrVoice/.api_key` (file-based). File always writable, no OS permission barriers. On load, checks file existence and reads key. Simple, reliable, works in dev and prod.

### 14. Dual Language Shortcuts
**Feature:** Users can set two separate shortcuts (primary + secondary) to record in different languages without visiting Settings.
**Implementation:**
  - `Settings` struct: `shortcut` + `primary_language` (e.g., "cs"), `secondary_shortcut` + `secondary_language` (e.g., "en")
  - `AppState` struct: `active_language` field set by whichever shortcut triggered recording
  - `re_register_shortcuts()` in `lib.rs` registers both, passing language to `on_shortcut()`
  - `start_audio_capture()` stores `active_language` in state
  - `stop_audio_capture_and_process()` reads `active_language`, overrides `settings.source_language` for transcription
  - UI: `GeneralTab.tsx` shows both shortcuts side-by-side with language pickers; secondary shortcut toggle to enable/disable
  - Example: Option+Space for Czech, Alt+Shift+Space for English

### 15. History Clear Button Not Working
**Root cause:** Tauri webviews block `window.confirm()` dialogs — they always return `false`, so the clear function silently aborted.
**Fix:** Replaced native confirm with two-click pattern. First click shows "Confirm Clear?" in red. Second click within 3 seconds clears all history. Auto-resets after 3 seconds if user doesn't confirm.

---

## Things That Work
- Onboarding flow (with skip)
- Settings UI (all tabs render and sync with tray)
- Local Whisper transcription — Czech and English speech recognized correctly
- Model download with progress bar
- History storage with clear all functionality (two-click confirmation)
- Tray menu (output mode switching with checkmarks, settings open, quit)
- Overlay window shows during recording, hides before paste
- Auto-paste into user's focused application
- Notification after transcription
- Clipboard always gets the transcribed text
- Accessibility permission prompt on first launch
- App stays alive in tray after windows close, exits on explicit Quit
- Crash logging to `~/Library/Application Support/TypeUrVoice/crash.log`
- Translation toggle works
- Source language selector always visible
- LLM text processing with OpenAI, Ollama, and LM Studio backends
- Reasoning model support (Gemma, etc.)
- Hold mode with 600ms minimum recording duration
- Cloud transcription (OpenAI Whisper API) with persistent API key storage
- Dual language shortcuts — record in different languages with separate hotkeys

---

## Dev Commands
```bash
# Run app in dev mode (cmake must be in PATH for whisper-rs)
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env" && npm run tauri dev

# Kill app + dev server
pkill -f "target/debug/typeurvoice"; lsof -ti :1420 | xargs kill -9

# Check for Rust errors only
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env" && cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error"

# Build release + create styled DMG (canonical path — Phase 5)
# Wraps `npm run tauri build`, ignores Tauri CLI's broken bundle_dmg.sh
# invocation, cleans orphan rw.*.dmg temps, then re-invokes bundle_dmg.sh
# with the right arguments. Reads version from Cargo.toml so it never drifts.
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env" && npm run dmg

# Fallback if the styled DMG path breaks: plain UDZO via hdiutil
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env" && npm run tauri build
hdiutil create -volname "TypeUrVoice" \
  -srcfolder src-tauri/target/release/bundle/macos/TypeUrVoice.app \
  -ov -format UDZO \
  src-tauri/target/release/bundle/dmg/TypeUrVoice_0.1.0_aarch64.dmg

# View crash log
cat ~/Library/Application\ Support/TypeUrVoice/crash.log
```

---

## Audit Remediation (April 2026)

Four-phase hardening pass across safety, stability, codebase hygiene, and design.
**No user-visible functionality changed** — same shortcuts, same pipeline, same settings, same DB schema.

### Phase 1 — Safety
- **Secure file permissions (0600)** — `settings.json`, `.api_key`, `history.db` all chmod 0o600 on Unix via new `restrict_permissions()` helper in `storage/settings.rs`. Owner-only read/write.
- **Atomic settings write** — `save()` writes to `settings.json.tmp` then `rename` into place; mid-write crashes no longer zero user config.
- **Capped audio buffer** — `audio/capture.rs` enforces `sample_rate * MAX_RECORDING_SECS` byte cap; belt-and-suspenders to the existing 5-min timer.
- **Atomic recording state transition** — `start_audio_capture` / `stop_audio_capture_and_process` do check-and-set inside a single `with_state` closure; double-press can't start two sessions.
- **Per-session Arcs** — each recording allocates fresh `samples`, `stop_flag`, `sample_rate` Arcs so an old capture thread can never write into a new session's buffer.
- **Poison-safe tray locks** — `tray.rs` uses `unwrap_or_else(|e| e.into_inner())` consistently; no more panic on poisoned mutex.
- **No-panic path handling** — `transcription/local.rs` uses `to_string_lossy()` instead of `to_str().unwrap()` for model paths.
- **Explicit error surfacing** — `window.hide().unwrap_or_default()` replaced with logged error branch; `lib.rs` log lines renamed from "Keychain error" to "API key store error" (keychain was removed in issue #13).

### Phase 2 — Stability
- **Aligned timeout budget** — outer pipeline timeout raised to 300s; each LLM call clamped to 45s in `processing/mod.rs`. No succeeding call gets killed by the wrapper.
- **Validated `model_key`** — `delete_local_model` / `model_filename` now reject unknown keys instead of silently falling back to `ggml-small-q5_1.bin`.
- **Toast for save errors** — new `src/components/ui/Toast.tsx`; `useSettings` exposes `lastError` + `clearError`; `SettingsWindow` renders errors that were previously only console-logged.

### Phase 3 — Codebase hygiene
- **Dead code removed** — `src-tauri/src/hotkey/` directory (no-op `parse_shortcut`), `src/App.css`, `src/assets/react.svg`, unused `LOCAL_MODELS` constant, `current_level()` RMS helper, `duration_ms` field on `TranscriptionResult`, `pub use capture::*` re-export.
- **Trimmed Cargo deps** — removed `log`, `env_logger`, `rodio`, `uuid`, `thiserror`, `objc`, `objc-foundation` (~30% dep bloat gone).
- **Shared `Section` component** — `src/components/ui/Section.tsx` replaces the 6 identical local copies in every settings tab.
- **Unified `RadioCard` primitive** — `src/components/ui/RadioCard.tsx` + `RadioCardGroup` cover both visual variants (`variant='dot'` and `variant='native'`) that used to be three separate hand-rolled radios in `OutputTab`, `SummaryTab`, `GeneralTab`. Proper ARIA (`role="radio"`/`radiogroup"`, `aria-checked`, Space/Enter keyboard activation).
- **Shared data-dir helper** — `typeurvoice_data_dir()` in `storage/settings.rs` is the single source of truth; `settings_path()`, `api_key_path()`, `history.rs` all use it.
- **Dropped runtime CSS parser** — `Button.tsx` uses plain inline style objects per variant; the `parseStyle` string-splitter is gone.
- **Tightened TypeScript** — `as any` on `model.key` replaced with narrow `LocalModel` union.
- **Fixed misleading microcopy**:
  - `"No speech detected... set Source Language to 'Czech'"` → generic "set Source Language explicitly"
  - EngineTab empty-state distinguishes `Loading model list…` (while fetching) from `No models available.` (after)
  - Onboarding mic copy branches on engine: local → "transcribed entirely on your device"; cloud → "sent to your API provider"
  - API key store copy: `"Stored in OS keychain"` → `"Stored locally on your device with owner-only permissions"`

### Phase 4 — Design (a11y + contrast)
- **ARIA on Tabs** — `role="tablist"` + `aria-orientation="vertical"`; each button `role="tab"`, `aria-selected`, `aria-controls`, roving `tabIndex`; panel `role="tabpanel"` + `aria-labelledby`. Arrow-key / Home / End navigation between tabs.
- **Accessible Toggle** — swapped `display:none` for standard visually-hidden pattern (`position:absolute; width:1px; height:1px; clip:rect(0,0,0,0);`) with `role="switch"`. Screen readers now announce "switch, on/off".
- **Focus ring visible** — `--border-focus` bumped from `#444` to `#6B8BFF` (passes AA on black). `:focus-visible` block unchanged.
- **Raised contrast** — `--text-tertiary` bumped from `#555` to `#8A8A8A` (6.3:1 on black, passes AA). EN ONLY badge bumped from 9px→10px with a darker pill background.
- **Spacing tokens adopted** — `gap: 8|12|16|20` swept to `var(--sp-2|3|4|5)` across all settings tabs, `Tabs.tsx`, `Onboarding.tsx`. Single source of truth for future adjustments.
- **Window min-size** — Settings window gets `minWidth: 580, minHeight: 480` (defensive; currently `resizable: false`).
- **Shortcut input labels** — primary/secondary shortcut capture inputs get `aria-label="Primary recording shortcut"` / `"Secondary recording shortcut"`.
- **History clear UX** — inline hint `"Click again within 3s · Esc to cancel"` next to the "Confirm Clear?" button; Escape key now cancels the pending-confirm state.

### Files added
- `src/components/ui/Section.tsx`
- `src/components/ui/RadioCard.tsx`
- `src/components/ui/Toast.tsx`

### Files removed
- `src-tauri/src/hotkey/` (directory)
- `src/App.css`
- `src/assets/react.svg`

### Verification
- `cargo build` → clean (0 warnings, 0 errors) in release mode
- `npm run build` (tsc + vite) → clean
- DMG size: 7.1 MB (`TypeUrVoice_0.1.0_aarch64.dmg`)

---

## Cloud Reliability Pass (April 2026)

Port of the Windows cloud-section fixes onto the macOS build plus a batch of diagnostic/UX improvements driven by real user-reported failures on an M1 Air and a second Mac. Zero platform-conditional code added; every change applies to both OSes. DMG still 7.1 MB.

### What changed

1. **Default `OutputMode` flipped to `Raw`** — `state.rs:135`.
   - Whisper (local *and* OpenAI `whisper-1`) already returns capitalized, punctuated text. Clean mode was sending a second gpt-4o-mini round-trip on every recording, costing 1–3 s and occasionally mangling the output. Raw is strictly faster and more accurate for dictation.
   - Clean / Polished remain available in Settings → Output for users who want filler-word stripping or prose rewriting.
   - **Existing installs keep whatever is already in `settings.json`** — no migration, user flips it manually one time.

2. **Hallucination filter moved to `transcription/mod.rs`** and applied to **both** cloud and local results.
   - OpenAI `whisper-1` is the same model family as the local multilingual GGML — same training corpus, same failure modes on silence / quiet audio. Filtering only local was a bug.
   - `HALLUCINATION_EXACT` — full-output patterns (`[music]`, `"."`, `"..."`, `Thank you.`, `Thanks for watching!`, bracketed ASR tokens like `[ Silence ]`).
   - `HALLUCINATION_SUBSTR` — subtitle-credit patterns (`titulky vytvořil`, `překlad titulků`, `subtitles by`, `www.amara.org`, `www.zeoranger.co.uk`, `hradeckesluzby`, `♪`).
   - `looks_like_url_only()` — drops Whisper's bare-URL fallbacks. Trims trailing `.`, requires no spaces, length 5–80, and either starts with `http(s)://` / `www.` OR is ≥2 alphanumeric dot-separated labels. Real speech has spaces so it never trips on that.
   - In-string stripping of bracketed tokens is still applied; bare-punctuation patterns (`"."`, `"..."`) are skipped during substring stripping so they don't corrupt real sentences.
   - Old local-only filter in `transcription/local.rs` removed.

3. **Error notifications — `show_error_notification()`** in `lib.rs:552`.
   - Uses `NotificationExt` with title "TypeUrVoice — recording failed" and body truncated via `safe_truncate(msg, 140)`.
   - Called from both pipeline-error branches (`Ok(Err(e))` and outer-timeout `Err(_)`), and from every paste-step failure branch.
   - **Not** gated on `settings.show_notification` — errors are the one place silent-by-default is worse than noisy. Settings/main windows are usually hidden, so the Tauri event alone was invisible.

4. **Rewrote the "no speech" error copy** — `lib.rs:424`. Old copy blamed language settings; real cause is almost always mic level.
   - Also introduced a **dead-mic vs quiet-mic split** at the same site: if `peak < 0.001` (effectively digital silence) the error points at Microphone permission and input device; otherwise it says "mic may be too quiet or muted."

5. **Cleanup language override** — `lib.rs:454–472`.
   - The transcription pipeline passes `settings.source_language` (which is overridden from the shortcut's `active_language` when present) into the LLM cleanup step, *not* Whisper's detected language.
   - Bug this fixed: press Czech shortcut → Whisper gets `language: "cs"` hint and transcribes fine → but on short / quiet clips Whisper's response still returns `language: "en"` → cleanup prompt claimed "text is in English" → gpt-4o-mini refused with *"I'm sorry, but I can only assist with English text."*
   - Priority: shortcut/settings override (if not `"auto"` and not empty) > Whisper's detected language.
   - Also logged: `Cleanup language: {resolved} (whisper detected: {X}, source setting: {Y})` for future diagnosis.

6. **`pbcopy` fallback for macOS clipboard writes** — `paste/clipboard.rs`.
   - `arboard::Clipboard::new()` / `set_text` can fail silently on some Macs (seen in practice on a Sonoma install where the NSPasteboard call from a tokio worker thread returned an error). Symptom: recording completes, "Done" fires, but `Cmd+V` pastes the user's *previous* clipboard content.
   - New flow in `write_clipboard()`: try `arboard` first; on any error, shell out to `pbcopy` via `std::process::Command`. `pbcopy` is a subprocess — can't fail for permission or thread-context reasons.
   - Applies to both `paste_text()` and `copy_to_clipboard()` (shared helper).

7. **Paste failure visibility** — `lib.rs:499–522`.
   - Previously, every paste/copy outcome went only to `eprintln!` (invisible in release builds) and the pipeline reported "completed successfully" even when nothing made it to the clipboard.
   - Now each of the four branches (`Ok(Ok(Ok(())))`, clipboard error, panic, task join failure) writes to `crash.log` AND — for the three failure branches — fires `show_error_notification`.

8. **Audio RMS / peak logging** — `lib.rs:13` (helper) + pipeline step 1.
   - `audio_level(samples)` returns `(rms, peak)`. Computed before transcription and logged to `crash.log`:
     `Audio captured: 2.1s, 33600 samples @ 16000Hz, rms=0.0001, peak=0.0003`
   - Lets us tell immediately from a user's `crash.log` whether the mic was dead (permission missing / wrong input device / muted) or whether Whisper just failed to find speech in a real signal.

9. **Raw Whisper response logged before filtering** — `transcription/mod.rs`.
   - `log_to_file` made `pub(crate)` so modules can reach it.
   - Before the hallucination filter runs, writes: `Whisper raw: lang={}, {} chars, text={:?}` (first 120 chars).
   - When the filter changes length, writes: `Hallucination filter: N → M chars`.
   - Distinguishes "Whisper returned nothing" from "filter wiped it out."

10. **Pipeline timeout log string corrected** to `300s` (matches actual wrapper).

### Files changed
- `src-tauri/src/state.rs` — default `output_mode`
- `src-tauri/src/transcription/mod.rs` — shared filter + raw-response logging
- `src-tauri/src/transcription/local.rs` — removed duplicated filter
- `src-tauri/src/processing/cleanup.rs` — unchanged (prompt still respects whatever `lang` is passed in; the fix is upstream, passing the correct `lang`)
- `src-tauri/src/lib.rs` — error notification helper, pipeline language resolution, audio level helper, paste-failure logging, `log_to_file` made `pub(crate)`, improved "no speech" copy with dead-mic split
- `src-tauri/src/paste/clipboard.rs` — unified `write_clipboard` with `pbcopy` fallback on macOS

### Root causes confirmed from user reports
- **M1 Air "no transcription" + "no TypeUrVoice folder in Library"**: Microphone permission wasn't granted / the mic was system-muted on that machine. `cpal` captured 2.1 s of pure silence → Whisper returned empty → "No speech" error. The folder *was* there (hidden `~/Library/Application Support/TypeUrVoice/`). New RMS logging now makes this diagnosable from a single `crash.log` line.
- **"I can only assist with English text"**: Whisper misdetected language on short/quiet Czech clips and cleanup trusted Whisper over the user's explicit shortcut-language setting. Fixed by the override-priority change in item 5.
- **Windows mysteriously worked for the same flow**: because on Windows the user had `source_language: "cs"` saved explicitly — both Whisper got the hint AND the cleanup prompt's language claim was correct.

### New things to check in `crash.log` when diagnosing future reports
- `Audio captured: … rms=X peak=Y` — if peak < 0.001, mic is dead. Point at System Settings → Privacy & Security → Microphone.
- `Whisper raw: lang=…, N chars, text=…` — shows exactly what Whisper returned before any filtering.
- `Hallucination filter: N → M chars` — tells you the filter wiped real content vs. genuine silence.
- `Cleanup language: cs (whisper detected: en, source setting: cs)` — proves the shortcut-language override is taking effect.
- `Clipboard: wrote N chars via pbcopy` — fell back from arboard; first sign that something's off on the user's clipboard stack.

### Verification
- `cargo build` (release) → clean
- `npm run tauri build` → `bundle_dmg.sh` still fails (known); `hdiutil create` path from CLAUDE.md produces a 7.1 MB DMG.
- No user-visible functionality regressed — same shortcuts, same settings schema, same SQLite schema.

### Public-distribution notes (not yet implemented — discussed, deferred)
To ship TypeUrVoice publicly we still need:
1. **Apple Developer account** ($99/yr) → Developer ID Application cert → sign the `.app`.
2. **Notarization** → Tauri handles this if `APPLE_CERTIFICATE`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` are set during `tauri build`.
3. **`tauri-plugin-updater`** → in-place updates from a hosted `latest.json` + signed `.app.tar.gz` (Ed25519). The plugin is wired in Phase 5 but currently `active: false` (no keys yet). Once keys exist, flip to `true` and paste the public key.
4. **GitHub Releases** is the pragmatic host (free, public CDN, native support in the updater). Mac App Store is ruled out: sandboxing breaks the global hotkey, Accessibility, and `osascript` paste.
5. Keep the updater's Ed25519 private key out of the repo. Losing it means all existing installs can no longer auto-update.

---

## Audit Remediation — Phase 5 (April 2026)

Priority-ordered fixes from the detailed audit: supply-chain hardening, latency/battery wins, updater infrastructure, logging hygiene, accessibility + correctness polish. Still zero user-visible regressions — same shortcuts, same SQLite schema, same settings schema (one additive field, `diagnosticsVerbose`, with `#[serde(default)]`). Branch `audit-phase-5`, 8 commits on top of the baseline.

**Scope decisions made with the user:**
- Repo was initialised as a git repo during this phase (it wasn't before) — baseline commit `2a9a043` captures pre-Phase-5 state. Every new change is a named commit with a scoped rollback target.
- Updater plugin shipped with `"active": false` by user choice (option **c**). No signing keys generated; UI button registered but will surface "updater is disabled" until release signing lands in a follow-up phase.
- Model SHA-256 hashes were verified by downloading every GGML file from Hugging Face and computing the real hash — user chose option **a** to avoid placeholder drift. All 8 pinned hashes are real.

### 1. Shared `reqwest::Client` across the pipeline (`e4758d1`)

**Why:** Each recording used to call `reqwest::Client::builder()` 4–6 times (Whisper + cleanup + optional translation + optional summary) — zero connection pool reuse, one extra TLS handshake per call, battery/latency tax.

- **New file:** `src-tauri/src/http.rs` — `AppHttpClient(pub reqwest::Client)` + `build()` + `SharedHttpClient = Arc<AppHttpClient>`. Single process-wide client, `pool_idle_timeout(75s)` (matches OpenAI server-side idle), `pool_max_idle_per_host(4)`, `connect_timeout(10s)` (closes a pre-existing silent-hang vector on slow DNS).
- Per-call timeouts migrated to `RequestBuilder::timeout()`: **60 s** for Whisper (`cloud.rs`), **45 s** for LLM calls (`processing/mod.rs::gpt_complete`).
- Threaded `&reqwest::Client` through: `run_pipeline` → `transcription::transcribe` → `cloud::transcribe`, and `processing::process` → `gpt_complete` → `cleanup::clean/polish` / `translation::translate` / `summary::summarize`. `test_text_processor` Tauri command pulls from managed state.
- **Kept intentionally separate** (not routed through the shared pool): `transcription/cloud.rs::test_api_key` (runs once at onboarding, cheap enough), and `transcription/local.rs::download_model` (needs 600 s timeout, would dominate the pool).
- **Known NIT (deferred):** `Arc<AppHttpClient>` is one indirection more than needed — `reqwest::Client` is already `Arc` internally. Harmless, deletable in a later cleanup pass.

### 2. SHA-256 model hash pinning + verification (`8f48b1d` + `c9871d7`)

**Why:** whisper.cpp memory-maps the GGML binary and walks arbitrary tensor graphs from it. A tampered model is effectively arbitrary-code execution. Before this phase, there was nothing stopping a compromised HF mirror (or a replaced model file on disk) from being handed to whisper-rs.

- **New file:** `src-tauri/src/transcription/model_hashes.rs` — `MODEL_HASHES` table (8 entries, all real SHA-256 values fetched from HF), `hash_file()` streaming hasher (64 KB read buffer), `verify()` with "known hash → must match; unknown hash → log + soft-pass" policy, `expected_hash()` lookup.
- **New dep:** `sha2 = "0.10"` (direct — `whisper-rs` pulls it transitively but explicit dep is the right call for something we use directly).
- `verify()` soft-pass routes through `crate::log_to_file` (not `eprintln!`) so drift upstream leaves a `crash.log` trail in release builds.
- **4 unit tests** in `#[cfg(test)] mod tests`: `hash_of_known_bytes` (sha256("hello") sanity), `unknown_model_key_passes` (short-circuit), `verify_rejects_mismatch` (Err branch — the actual security property), `expected_hash_lookup`. Temp filenames include `std::process::id()` to avoid parallel-test races.
- **Wired in two places:** (a) `download_model` runs `verify()` via `tokio::task::spawn_blocking` AFTER `file.flush()` + `drop(file)` — mismatch deletes the partial file. (b) `transcribe_blocking` runs `verify()` immediately after `!model.exists()` check, BEFORE `WhisperContext` cache lookup — tampering caught first, even if the cache is hot.
- **Performance note:** `verify()` runs on *every* `transcribe_blocking` call, not just cache misses. At ~500 MB/s stream hashing, that's ~2–3 s for the 1.5 GB `ggml-medium.bin`, amortised across the ~20 s of Whisper inference that follows. The comment at `local.rs:184-187` was initially wrong ("once per app run") — corrected in commit `a16b698`.

The 8 pinned hashes (lowercase hex):

| model_key | SHA-256 |
|---|---|
| `small-q5` | `ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb` |
| `medium-q5` | `19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f` |
| `large-q5` | `d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1` |
| `turbo-q5` | `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2` |
| `small` | `1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b` |
| `medium` | `6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208` |
| `base-en` | `a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002` |
| `tiny-en` | `921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f` |

If HF ever re-uploads a model, the value drifts and users get `"Model verification failed"` — fix is to update `MODEL_HASHES` after manually re-running `shasum -a 256`. The soft-pass policy on `None` entries means ship-with-unknown is possible without bricking users, but the real hashes are the shipped defaults.

### 3. `WhisperContext` cache across recordings (`bf70ed2`)

**Why:** Every local-engine recording called `WhisperContext::new_with_params(model_path, ...)` — full GGML mmap + tensor setup on every key press. 2–5 s per recording on large models, just wasted.

- **New file:** `src-tauri/src/transcription/whisper_cache.rs` (feature-gated `#[cfg(feature = "local-whisper")]`) — `WhisperCache { inner: Mutex<Option<Entry>> }` with `Entry { model_key, model_path, mtime, ctx: Arc<WhisperContext> }`.
- **Invalidation keyed on 3 signals:** different `model_key` (user switched model), different `model_path` (defensive), different `mtime` (file was re-downloaded).
- **Double-check locking:** fast path reads under the mutex and returns `Arc::clone(&ctx)` — slow path (actual `WhisperContext::new_with_params`) runs **outside** the mutex, so a concurrent fast-path reader is never blocked by a slow loader.
- **Thread safety confirmed:** `whisper-rs 0.13`'s `WhisperContext` is `Send + Sync` on the CPU path (verified via `unsafe impl` in the crate source). `Arc<WhisperContext>` works directly — no inner Mutex needed.
- **`delete_local_model` Tauri command** invalidates the cache on successful delete. Because `tauri::generate_handler!` can't handle a `#[cfg]`-gated parameter, the command is split into two full bodies under `#[cfg(feature = "local-whisper")]` / `#[cfg(not(...))]` — runtime behaviour identical, macro limitation only.
- Race into a cold cache is architecturally impossible in prod: `RecordingState::Processing` gates a second recording start in `lib.rs:305`.

### 4. `tauri-plugin-updater` plumbing (`f7d0bd9`, inactive config)

**Why:** Until this phase, every new build required uninstall + reinstall. Public distribution needs in-place updates — but those need Ed25519 signing keys and a hosted `latest.json`, which aren't ready yet. Land the plumbing; activate later.

- **New dep:** `tauri-plugin-updater = "2"` in `src-tauri/Cargo.toml`.
- **Plugin config** in `tauri.conf.json` under a new top-level `plugins.updater`:
  ```json
  { "active": false, "endpoints": ["https://github.com/typeurvoice/typeurvoice/releases/latest/download/latest.json"], "pubkey": "", "dialog": true }
  ```
  Empty `pubkey` is accepted by the plugin when `active: false` — it short-circuits validation.
- `"updater:default"` added to `src-tauri/capabilities/default.json`.
- `.plugin(tauri_plugin_updater::Builder::new().build())` added to the Tauri Builder chain in `lib.rs`.
- **New command:** `check_for_updates` → `UpdateStatus { available, version, notes }`. Calls `app.updater()?.check().await`.
- **UI:** new `UpdatesRow` component in `GeneralTab.tsx` under the System section, below "Start on login". Button → "Checking…" spinner → result line. When `active: false`, `updater.check()` returns an error and the UI surfaces "Check failed: …" — known UX quirk for this variant.

### 5. crash.log rotation + verbose-gated previews (`7db9aab`)

**Why:** `log_to_file` appended forever (every recording writes 6+ lines). Also, `Whisper raw: … text="<full transcript>"` leaked PII/meeting content into logs users share for debugging.

- **New file:** `src-tauri/src/logging.rs` — `append(msg)` with size-based rotation (`MAX_SIZE_BYTES = 1_000_000`, `BACKUP_COUNT = 1`), `redact_preview(s, max_chars)`, and a module-scoped `ROTATE_LOCK: Mutex<()>`. 2 unit tests.
- **`log_to_file` in `lib.rs`** reduced to a one-line delegate to `logging::append`. Visibility stays `pub(crate)`; existing call sites (`transcription/mod.rs`, `model_hashes.rs`, etc.) continue to use `crate::log_to_file(...)` unchanged.
- **Rotation triggers when the live log exceeds 1 MB:** `crash.log.1` → `crash.log.2` (overwriting), then `crash.log` → `crash.log.1`. Net worst case: 3 files × 1 MB = 3 MB on disk. The `ROTATE_LOCK` is held only across `metadata()` + renames (all atomic metadata ops); non-rotating writes skip the lock entirely.
- **New setting `diagnostics_verbose: bool`** on `Settings` struct (default `false`, `#[serde(default)]` ensures backwards-compat with existing `settings.json` files). Matching `diagnosticsVerbose` field on TS `Settings` type + `DEFAULT_SETTINGS` in `useSettings.ts`.
- **Two log sites gated:** `transcription/mod.rs` (`Whisper raw:` line — text preview wrapped in `redact_preview`) and `lib.rs::run_pipeline` Step 1 result. When `diagnosticsVerbose=false`, the log line shows only `lang=…, N chars` with no text content.
- **UI toggle:** "Verbose diagnostics" Toggle in General → System, copy: "Include transcript previews in crash.log for debugging. Off by default to protect privacy."
- **Known scope limit:** the privacy contract applies to `crash.log` only. `eprintln!` transcript lines in `transcription/mod.rs:97,105,112` and `lib.rs:500,568` still run unconditionally — invisible in signed/notarised `.app` bundles (stderr goes nowhere) but visible when dev-running via `npm run tauri dev`. Listed as follow-up.

### 6. Toast dismissal + shortcut conflict validation (`07d40e5`)

**Why (Toast):** Error toasts auto-dismissed after 4 s with no keyboard path to kill them early — a11y violation (WCAG: users must be able to dismiss notifications).

**Why (shortcut validation):** Nothing stopped the user from binding primary and secondary to the same combo. `tauri-plugin-global-shortcut::register()` silently no-ops the second registration — both shortcuts fired the same language, defeating the feature.

- `src/components/ui/Toast.tsx` rewritten: close `×` button with `aria-label="Dismiss notification"`, second `useEffect` adds/removes a `window` `keydown` Escape listener (scoped by `message != null` so no global listener pollution when hidden), errors now use `role="alert"` + `aria-live="assertive"` (assertive interrupts screen readers immediately), info stays `role="status"` + `aria-live="polite"`. Props signature unchanged — all existing callers still work.
- `GeneralTab.tsx::captureShortcut` checks the *other* field (primary vs secondary, whichever isn't being captured) and rejects when the new combo would collide. `shortcutError` state renders at the TOP of the General panel inside `role="alert"`. Self-clears on the next successful (non-conflicting) capture. Canonical comparison works because both captures run the same modifier-push order (Meta, Control, Alt, Shift).
- Existing Escape handler in `HistoryTab` (for the "Confirm Clear?" dance) is untouched and doesn't conflict — both handlers scope their listeners by their own state flag.

### Files added this phase
- `src-tauri/src/http.rs`
- `src-tauri/src/transcription/model_hashes.rs`
- `src-tauri/src/transcription/whisper_cache.rs` (feature-gated)
- `src-tauri/src/logging.rs`
- `docs/superpowers/plans/2026-04-21-audit-remediation-phase-5.md` (the plan itself)

### Files modified this phase
- `src-tauri/Cargo.toml` (+`sha2`, +`tauri-plugin-updater`)
- `src-tauri/tauri.conf.json` (+`plugins.updater` block, inactive config)
- `src-tauri/capabilities/default.json` (+`updater:default`)
- `src-tauri/src/lib.rs` (module registrations, shared state, shared client + whisper cache threading, `check_for_updates` command, `log_to_file` delegate, verbose-gated Step 1 log)
- `src-tauri/src/state.rs` (+`diagnostics_verbose` field with `#[serde(default)]`)
- `src-tauri/src/transcription/mod.rs` (verbose-gated `Whisper raw:` log, threaded `&reqwest::Client` + cache)
- `src-tauri/src/transcription/cloud.rs` (accept shared client, per-call timeout on request builder)
- `src-tauri/src/transcription/local.rs` (hash verify pre-download + pre-transcribe, use `WhisperCache::get_or_load`)
- `src-tauri/src/processing/{mod,cleanup,translation,summary}.rs` (accept `&reqwest::Client`)
- `src/types/index.ts` (+`diagnosticsVerbose`)
- `src/hooks/useSettings.ts` (+`diagnosticsVerbose` default)
- `src/lib/tauri.ts` (+`checkForUpdates` invoke wrapper)
- `src/components/ui/Toast.tsx` (close button, Escape, assertive/polite split)
- `src/components/settings/GeneralTab.tsx` (shortcut conflict validation, Updates row, Verbose diagnostics toggle)

### New things to check in `crash.log` (Phase 5)

- `model_hashes: no pinned hash for "xxx" — skipping verification` — a `MODEL_HASHES` entry is `None`; hashes need to be filled in.
- `Model verification failed for xxx: expected …, got …` — either the upstream file drifted, or the on-disk model was tampered with. User's path forward is "delete + re-download" (wording is in the error message itself).
- After 1 MB the live `crash.log` gets rotated; old content moves to `crash.log.1` and then `crash.log.2`. If reports stop mentioning earlier events, the rotation ate them — ask the user to also share the numbered backups.

### Verification (Phase 5)
- `cargo build --release` → clean, 34 s, 0 warnings
- `cargo build --no-default-features` → clean (one pre-existing `resample` dead-code warning — unrelated to this phase; queued as a `#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]` follow-up)
- `cargo test --lib` → 6/6 passing (4 model_hashes + 2 logging)
- `npm run build` → clean, 266 KB bundle
- TS `Settings` ↔ Rust `Settings` agree on all 28 fields
- `diagnostics_verbose` defaults to `false` at every entry point (Rust `Default`, `#[serde(default)]`, TS `DEFAULT_SETTINGS`)

### Commits on `audit-phase-5` (in order)
1. `e4758d1` — perf(http): share reqwest::Client across transcription and LLM pipeline
2. `8f48b1d` — feat(transcription): add SHA-256 hash table and verifier for GGML models
3. `c9871d7` — feat(transcription): verify downloaded model hashes before use
4. `bf70ed2` — perf(local): cache WhisperContext across recordings
5. `f7d0bd9` — feat(updater): wire tauri-plugin-updater (inactive config) + Check for updates button
6. `7db9aab` — feat(logging): crash.log rotation + verbose-gated transcript previews
7. `07d40e5` — feat(ui): toast dismissal (close button + Escape) and shortcut conflict validation
8. `a16b698` — docs(transcription): correct comment about hash verification cadence

### Phase 5 follow-up work (not blocking merge)

1. **Release signing + auto-update activation.** Generate Ed25519 keypair via `tauri signer generate -w ~/.config/typeurvoice-updater.key` → copy public key into `tauri.conf.json::plugins.updater.pubkey` → flip `active: true` → set up GitHub Actions release pipeline that produces signed `.app.tar.gz` + `latest.json`. Until then, the "Check for updates" button will always show "Check failed: updater is disabled". See the "Public-distribution notes" section above.
2. **Drop the `Arc<AppHttpClient>` double-wrap in `http.rs`.** Replace with `.manage(http::build())` where `build() -> reqwest::Client`, and `tauri::State<'_, reqwest::Client>` in commands. Saves one indirection; the client is already `Arc` internally.
3. **Gate the 4 remaining `eprintln!` transcript sites on `diagnostics_verbose`** so dev-mode stderr respects the same privacy contract as `crash.log`. Sites: `transcription/mod.rs:97,105,112`, `lib.rs:500,568`.
4. **Silence `resample` dead-code warning** under `--no-default-features` with `#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]`.
5. **Consolidate `dirs::data_dir()` re-derivation.** Currently in `logging.rs`, `storage/settings.rs`, and `transcription/local.rs` (the last adds `/models` so that one's fine). The Phase-3 `typeurvoice_data_dir()` helper could absorb the other two.
6. **One-time manual GUI smoke** — was not run in the Phase 5 loop. Checklist: record via primary + secondary shortcuts → paste lands → switch engines → verify second local recording is ~2–5 s faster (cache hit) → change output mode → translate → enable verbose diagnostics and confirm `crash.log` shows/hides `text=…` accordingly → trigger a save error to confirm Toast × + Escape dismissal → attempt to set primary = secondary and confirm inline rejection → redownload a model and confirm hash check passes → tamper a model file and confirm next recording errors with the re-download hint.

---

## Onboarding Persistence Fix (April 2026)

Users reported the first-run onboarding flow reappearing after they restarted their Mac and re-launched TypeUrVoice — even though they had already completed setup. Root-caused to a fragile settings-load path that silently wiped user config whenever the struct schema changed between versions. DMG still 7.1 MB.

### Root cause
1. `storage::settings::load()` in `storage/settings.rs` uses `serde_json::from_str::<Settings>` to deserialize `settings.json`.
2. Before this fix, only newer fields on `state::Settings` (`processing_model`, `text_processor`, `secondary_shortcut`, etc.) had `#[serde(default)]`. Older fields — including `onboarding_complete`, `output_mode`, `source_language` — did **not**.
3. Every time a new required field was added to the struct in a subsequent app release, the *existing* user's `settings.json` (written by an older version, missing the new field) would fail to deserialize with a `missing field X` error.
4. `lib.rs` called `storage::settings::load().unwrap_or_default()` → the `Err` was swallowed → a fresh `Settings::default()` was used → `onboarding_complete = false` → onboarding window shown on every launch until the user clicked through it again.
5. "Restart PC and the app wants to set up again" fits because users typically update the app between reboots; the reboot itself is incidental.

### Fix
- **`state.rs:83-85`** — `#[serde(rename_all = "camelCase")]` → `#[serde(rename_all = "camelCase", default)]` at the `Settings` struct level. Missing fields now fall back to each field's `Default` value instead of failing the whole deserialize.
- **`lib.rs:780-789`** — replaced `storage::settings::load().unwrap_or_default()` with an explicit `match` that logs the parse error to `crash.log` (`"Failed to load settings.json: {err} — falling back to defaults (onboarding will re-show)"`) before falling back. The silent re-onboarding path is now observable.

### Why struct-level `#[serde(default)]` is the right tool
`Default` is already implemented for `Settings`, and `serde(default)` at the struct level hands each missing field its field-level default (via `#[serde(default = "...")]`) or, failing that, the value from `<T as Default>::default()` on that one field. So existing users' partial JSON merges cleanly with the current struct — no migration code, no version field, no manual field-by-field annotation.

### Files changed
- `src-tauri/src/state.rs`
- `src-tauri/src/lib.rs`

### New crash.log entry to watch for
- `Failed to load settings.json: <serde err> — falling back to defaults (onboarding will re-show)` — if this *ever* appears in a future report, the struct has drifted in an incompatible way that `#[serde(default)]` can't cover (e.g. a field type changed, or a required enum variant was renamed). Handle explicitly.

### Verification
- `cargo build --release` → clean (0 warnings, 0 errors).
- `npm run build` (tsc + vite) → clean.
- `npm run tauri build` → `bundle_dmg.sh` fails (known); `hdiutil create` produces `TypeUrVoice_0.1.0_aarch64.dmg` at 7.1 MB.
- Fix landed on `main` as commit `bda2672` ("fix: make settings deserialization tolerant to missing fields"). Not pushed to any remote.

---

## Simplification Pass (April 2026)

Tight, behaviour-preserving cleanup driven by the `code-simplifier` subagent. Explicit constraints: zero behavioural change, no feature removal, no module-boundary reshapes, no UI visual changes, all stability guards from CLAUDE.md left untouched (`catch_unwind`, per-session Arcs, 600ms hold minimum, `QuitFlag`, `osascript` paste, hallucination filter, `pbcopy` fallback, cleanup-language override, `#[serde(default)]`, atomic settings write, capped audio buffer, poison-safe locks). DMG still 7.1 MB.

### What changed

1. **Merged OpenAI-compatible processor arms** — `processing/mod.rs:112-128`.
   - `TextProcessor::Ollama` and `TextProcessor::LmStudio` used identical `GptConfig` construction. Collapsed with `|` pattern.
   - Moved `text_processor_url.trim_end_matches('/')` *inside* the merged arm so the OpenAI path no longer computes a base URL it never uses.

2. **Extracted `fallback_to_raw` helper** — `processing/mod.rs:171-212`.
   - The Clean and Polished branches of `process()` both did the same three-way match: `Ok(non-empty) → use it`, `Ok(empty) → log + raw`, `Err → log + raw`.
   - New private helper `fn fallback_to_raw(result: Result<String>, raw_text: &str, mode: &str) -> String` collapses both ~12-line blocks to a single call. Net ~18 lines saved.
   - Intentional tiny log-copy change: `"Cleanup failed (...)"` → `"cleanup failed (...)"` (lowercase mode token is now substituted from the `mode` arg). No runtime behaviour difference — only the spelling of an `eprintln!` line.

3. **Idiomatic `query_map` collect** — `storage/history.rs:84-101`.
   - Replaced the hand-rolled `let mut entries = Vec::new(); for row in rows { entries.push(row?); }` with `.collect::<rusqlite::Result<Vec<_>>>()?`. Same `?`-propagation semantics, fewer lines.

4. **Tightened stray `as any`** — `src/components/settings/EngineTab.tsx:107`.
   - `onUpdate({ localModel: remaining.key as any })` → `as LocalModel`. The narrow union was already imported; this was the last loose cast in the settings UI (CLAUDE.md flagged this kind of sloppiness during the Phase 3 hygiene pass).

### What was considered and rejected
- Splitting `safe_truncate` / `audio_level` in `lib.rs` — already minimal.
- Sharing `restrict_permissions` between `storage/settings.rs` and `storage/history.rs` — would cross module boundaries; constraint forbids it.
- Factoring `build_clean_prompt` / `build_polish_prompt` in `processing/cleanup.rs` — prompt text is content, not code; factoring would hurt version-diff readability.
- Simplifying the `on_shortcut` Hold branch — the 600ms minimum logic (Issue #4) is load-bearing and splitting it obscures the timing invariants.
- Collapsing the `final_text` priority chain to `summary_text.or(translated).unwrap_or(processed)` — would change behaviour when `summary_enabled` is true but `summary_text` is `None` (would fall through to `translated` instead of `processed`). The explicit if/else-if chain encodes the settings-toggle priority.
- `language.clone()` micro-optimizations in `start_audio_capture` — not hot-path, removing would reorder log lines.

### Files changed
- `src-tauri/src/processing/mod.rs`
- `src-tauri/src/storage/history.rs`
- `src/components/settings/EngineTab.tsx`

### Verification
- `cargo build --release` → clean (0 warnings, 0 errors).
- `npm run build` (tsc + vite) → clean, frontend bundle unchanged at 264.23 kB gzipped-79.51 kB.
- Net ~33 lines of app code removed.
- All stability guards intact; settings schema byte-identical; SQLite schema untouched; tray/menu/onboarding flows unchanged.

---

## In-App Help Tab (May 2026)

Added a dedicated **Help** tab to the Settings window so users can read plain-English
explanations of every feature without leaving the app. Pure additive UI — no behaviour
changes, no new settings fields, no Rust touched.

### What was added

- **New file:** `src/components/settings/HelpTab.tsx` — long-form help organised by
  Settings tab. Self-contained presentational component (no props, no state, no Tauri
  calls). Re-uses the shared `Section` component so spacing/typography matches every
  other tab.
- **New tab id `'help'`** added to the `TabId` union in `src/types/index.ts`. Wired
  into the tab list in `src/components/settings/SettingsWindow.tsx` with the
  `HelpCircle` lucide icon, last in the list (after History).

### Sections in the Help tab

1. **Quick Start** — 4-step walkthrough plus the Microphone + Accessibility permissions
   note (covers Issue #1).
2. **General Tab** — every field explained: Primary/Secondary Shortcut, Microphone
   picker (with not-connected fallback note), Hold vs Toggle modes (with the 600 ms
   minimum from Issue #4), Start on login, Updates, Verbose diagnostics, Show
   crash.log.
3. **Engine Tab — Three Ways to Run TypeUrVoice** — the headline section. Three
   `ModeCard` sub-components labelled with coloured pill tags:
   - **Fully Online** — Cloud Whisper + OpenAI text processing. Easiest, best
     accuracy, audio sent to OpenAI.
   - **Semi-Offline** — Local Whisper + OpenAI text processing. Audio stays on
     device, only transcripts sent to OpenAI.
   - **Fully Offline** — Local Whisper + Ollama or LM Studio. Nothing leaves the
     Mac.
   Followed by per-control notes: transcription engine choice, OpenAI API key
   (matches the wording fixed in Phase 3), GPT model picker, Local Models (mentions
   the SHA-256 verification from Phase 5), text-processor backends with their
   default URLs.
4. **Output Tab** — Raw vs Clean vs Polished vs Custom (matches the actual modes in
   `OutputMode`), Auto-paste, Show notification (with the "errors are always shown"
   carve-out from the Cloud Reliability Pass).
5. **Translation Tab** — Source language (with the auto-detect-on-short-clips
   caveat from Issue #5 / Cloud Reliability Pass item 4), enable toggle, target
   language.
6. **Summary Tab** — three summary types, dedicated summary shortcut.
7. **Dictionary Tab** — what entries are for and that they're a Whisper bias hint,
   not a hard rule.
8. **History Tab** — list, retention, two-click clear with Esc cancel (matches the
   UX from Issue #15 / Phase 4).
9. **Troubleshooting** — five common failure modes mapped to fixes:
   "no text" → mic permission + crash.log peak check;
   "doesn't paste" → Accessibility permission;
   "wrong language" → set source explicitly;
   "I can only assist with English text" → same root cause (Cloud Reliability Pass
   item 5);
   "frozen mid-recording" → 300 s timeout from Phase 2.
10. **Privacy Notes** — what stays local, what gets sent where, what crash.log
    contains, no telemetry.

### Helper components inside HelpTab.tsx
Plain inline-styled presentational atoms scoped to the file (kept private, not
exported, since they have no use elsewhere): `Para`, `Note`, `Field` (label +
description), `Bullets`, `Steps`, `ModeCard` (the tagged engine-mode card),
`B` (bold accent), `I` (italic), `Code` (inline monospace pill).

### Files changed
- `src/components/settings/HelpTab.tsx` (new)
- `src/components/settings/SettingsWindow.tsx` (+import, +tab entry, +render branch)
- `src/types/index.ts` (`TabId` union extended with `'help'`)

### Verification
- `npm run build` (tsc + vite) → clean. Frontend bundle 285.01 kB / 85.64 kB gzipped
  (was 264.23 kB before — Help content adds ~20 kB raw, ~6 kB gzipped).
- No Rust changes; `cargo build --release` not re-run.
- DMG rebuilt via `npm run dmg` after the commit.

---

## May 2026 Audit Follow-up — Phases A/B/C/D (commit `4274fd0`)

Action on the audit findings: simplification, prompt hygiene, stability, and three optional features. Single squashed commit on `main` (file overlaps across phases made it impractical to split). DMG: 7.8 MB. cargo test --lib 6/6 pass.

### Phase A — Foundation simplification

**A1. Single `ModelInfo` table replaces four parallel tables.** New file `src-tauri/src/transcription/models.rs` owns `pub struct ModelInfo { key, name, url, filename, hash: Option<&'static str> }` and `pub const MODELS: &[ModelInfo]`. Removes the 4-way duplication of model metadata that previously lived across `MODEL_URLS` (in `local.rs`), `model_filename()` (in `local.rs`), `MODEL_HASHES` (in `model_hashes.rs`), and `LocalModel::key()` impl (in `state.rs`). Adding a model is now one row instead of four. `model_hashes::verify` reads `hash` via `super::models::lookup(key).and_then(|m| m.hash)`.

**A2. Drop `AppHttpClient` wrapper.** `reqwest::Client` is already `Arc` internally; the wrapper struct + `SharedHttpClient = Arc<AppHttpClient>` + `.client()` accessor were redundant. `http::build()` now returns plain `reqwest::Client`. Tauri state: `tauri::State<'_, reqwest::Client>` instead of `tauri::State<'_, http::SharedHttpClient>`. Phase 5 follow-up #2 — done.

**A3. `restrict_permissions` moved to `storage/mod.rs`.** The 0600 chmod helper had identical copies in `storage/settings.rs` and `storage/history.rs`. Now lives once as `pub(crate) fn restrict_permissions(path: &Path)`, consumed by both via `use super::restrict_permissions;`.

**A4. `OutputMode::as_str()` kills three `format!("{:?}").to_lowercase()` sites.** Added the obvious `impl OutputMode { pub fn as_str(&self) -> &'static str { ... } }` and replaced the Debug-format-then-lowercase trick at `tray.rs:14`, `tray.rs:100`, `lib.rs:653`.

**A5. TS `DEFAULT_SETTINGS.outputMode = 'raw'`.** Brought TS default in line with Rust default. (Note: later reverted to `'clean'` during the Custom Profiles work below — see that section for rationale.)

### Phase B — Prompts + LLM hygiene

All in `processing/`. Protocol abstraction (E1 below) wraps these but the prompt + temperature work is conceptually Phase B.

**B1. `GptConfig` grows per-call `temperature` + `max_tokens` with builder methods.** Old shape hardcoded `temperature: 0.3` and `max_tokens: 2000` for every call. New `with_temperature(f32)` and `with_max_tokens(Option<u32>)` let each call site override. Token budget is now proportional to input length via `token_budget_for(input, multiplier)` (multiplier 0.6 for cleanup, 1.0 for polish, 1.5 for translate). Long dictations no longer get silently truncated. Per-task settings:
- Cleanup → temperature 0.0 (mechanical), `token_budget_for(raw, 0.6)`
- Polish → temperature 0.3 (creative), `token_budget_for(raw, 1.0)`
- Translation → temperature 0.0, `token_budget_for(processed, 1.5)`
- Summary → keeps default 0.3, max_tokens 2000

**B2. Translation prompt uses language names on both sides.** Was: "Translate the following text from cs to English" (mixed code + name). Now: always passes `language_name(code)` for both source and target, so prompts read "from Czech to English". Translation prompt itself also adds tone-preservation rule: "Preserve the speaker's tone and intent. Do NOT add notes, comments, or explanations."

**B3. Summary prompts respect source language.** Was: summarising Czech text returned English summaries because the prompt never claimed a language. New `summary::summarize(http, text, type, language, cfg)` takes an explicit `language` arg and prepends `"Write the output in {lang}."` to every variant prompt. If translation is enabled, the summary uses `target_language` (the post-translation language); otherwise it uses `lang_name`.

**B4. Cleanup prompt actually strips fillers.** The old `build_clean_prompt` said "Keep ALL of the speaker's original words" — but the OutputTab UI advertised Clean as "Punctuation, grammar, filler removal." Mismatch. Prompt now explicitly strips: "um", "uh", "er", "ah", "hmm", and the discourse-marker uses of "like", "you know", "I mean", "sort of", "kind of", "basically". Plus false starts and self-corrections. Other wording (tone, structure, meaning) is preserved. Polish prompt also softened: "Use paragraphs naturally. Use bullet lists ONLY when the speech is genuinely a list of items." (Was: "Format with paragraphs and bullet lists where appropriate" — over-formatted single sentences.)

**B5. Strip markdown code-fences from LLM output.** New `strip_code_fence(s: &str) -> String` runs after the response is resolved (content or reasoning_content). Drops opening ` ``` ` (with optional language tag on the same line) and trailing ` ``` `. Idempotent on un-fenced output. Gemini and smaller models occasionally wrap output in code blocks despite the system prompt telling them not to.

**B6. Single retry on transient errors.** `gpt_complete` (now in `providers/mod.rs::complete`) retries once with 2-second backoff on HTTP 429 and 5xx. 4xx auth/bad-request errors don't retry — they never recover. Connection errors also retry once. Visible in `crash.log` as `LLM transient error 429 — retrying in 2s`.

### Phase C — Stability

**C1. Bumped pipeline timeouts.** Outer wrapper 300s → **360s** (`lib.rs::stop_audio_capture_and_process`). Per-LLM-call 45s → **60s** (`gpt_complete`'s request builder). Math: 60s Whisper + 4×60s LLM ≤ 300s, 60s slack. Accommodates 4-call pipelines (cleanup + translation + summary + the rare 4th hop) without tripping the wrapper.

**C2. Cache hash-verification result per `(key, mtime)`.** `WhisperCache::Entry` grew a `hash_verified: bool` field. New methods `is_verified(model_key, model_path) -> bool` and `mark_verified(model_key, model_path)`. `transcribe_blocking` only runs `model_hashes::verify()` when `!cache.is_verified(...)`. Effect: full SHA-256 stream (~2–3 s on the 1.5 GB medium model) runs **once per app start per model**, not on every recording. Mtime-keyed → hot-swapping the file mid-session still triggers re-verify on the next press. Phase 5's `local.rs:184-187` performance comment is now correct (used to say "once per app run", was wrong, now actually is once per app run thanks to this).

**C3. Selective shortcut unregistration + Cmd+Q intercept moved to one-time setup.** Was: `re_register_shortcuts()` called `unregister_all()` then re-registered everything including the Cmd+Q intercept. Every settings save churned the intercept. Now: `re_register_shortcuts(app, old: Option<(String, Option<String>)>)` only unregisters the specific previous primary + secondary shortcuts. Cmd+Q intercept registers exactly once in `setup()`. `update_settings` captures the old shortcut values BEFORE overwriting and passes them in.

### Phase D — Optional features

**D1. Microphone device selector.** New `Settings::input_device: Option<String>` (`#[serde(default)]`, `None` = system default). New `audio::capture::list_input_devices() -> Vec<String>` enumerates cpal input devices. New Tauri command `list_input_devices`. `record_until_stopped` now takes `device_name: Option<String>` and falls back to `host.default_input_device()` with a `crash.log` warning if the saved device isn't found. UI: new "Microphone" section in General tab with a dropdown including "System default" + "(not connected)" hint when the saved device is missing.

**D4. "Show crash.log" button** in General → System. New Tauri command `show_crash_log` that runs `open -R <path>` on macOS / `explorer /select,<path>` on Windows. Falls back to opening the parent directory if `crash.log` doesn't exist yet (fresh install). No new dependencies — just `std::process::Command`.

**D6. Custom output mode with user-defined prompt.** New `OutputMode::Custom` variant + `Settings::custom_prompt: String`. New `cleanup::custom(http, text, cfg, user_prompt, dict_hint, language)` wraps the user prompt with hard rules ("preserve language, no code fences, no commentary"). Empty prompt → behaves as Raw (no LLM call). Tray submenu gets a Custom entry. UI: 4th radio in OutputTab + 3000-char textarea. **Note:** This single-textarea design was replaced by a profile library in the next commit (see "Custom Output Profiles" below) — the migration handles that.

**D7 deferred.** Live partial-transcript overlay (whisper-rs segment callback streaming to the overlay) was scoped out at user request after the plan was reviewed. Local-only feature; OpenAI Whisper API doesn't stream.

### Files added (Phase A-D)
- `src-tauri/src/transcription/models.rs` (A1)

### Files modified (Phase A-D)
Most of the codebase. Notable: `state.rs` (Settings fields, OutputMode::as_str, OutputMode::Custom), `lib.rs` (timeouts, mic plumbing, show_crash_log command, re_register_shortcuts signature, register_cmd_q_intercept moved to setup), `tray.rs` (Custom in submenu), `processing/{mod,cleanup,translation,summary}.rs` (full prompt + dispatch rework), `transcription/{local,whisper_cache}.rs` (hash-verify caching), `audio/capture.rs` (device picker), `EngineTab.tsx` (mic selector wiring), `GeneralTab.tsx` (mic selector + crash.log button), `OutputTab.tsx` (Custom mode), `HelpTab.tsx` (now exists post-bde2f7f and references new features).

### Verification
- cargo build --release: clean (3 dead-code warnings on unused `as_str` impls — exercised by tray code paths in run-time builds)
- cargo test --lib: 6/6 pass (4 model_hashes + 2 logging)
- npm run build: clean, 268.95 kB / 80.69 kB gzipped
- DMG: 7.8 MB

---

## Multi-Provider LLM Support (May 2026, commit `a77d332`)

Big-feature add: **9 new text-processing providers** + **1 new cloud Whisper option** + **multi-key storage**. Architecture-wide change touching backend protocols, settings shape, key persistence, and Settings UI.

### What was added

**3 wire protocols** under `processing/providers/`:
- `Protocol::OpenAiChat` — OpenAI + every "OpenAI-compatible" host (Grok, Groq, DeepSeek, Mistral, OpenRouter, Perplexity, Together, Ollama, LM Studio).
- `Protocol::Anthropic` — `/v1/messages` with `system` top-level, `x-api-key` header, `anthropic-version: 2023-06-01`, required `max_tokens`. Response is `content[].text` array.
- `Protocol::Gemini` — `generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`. `system_instruction` top-level. `safetySettings` block with all 4 categories at `BLOCK_NONE` (this is a transcription tool — without the override, mundane content gets blocked as "harassment").

**6 user-facing provider options** in the Engine tab dropdown:
- OpenAI (existing)
- **Anthropic Claude** — Haiku 4.5 default; Sonnet 4.5; Opus 4.5
- **Google Gemini** — Flash 2.5 default; Pro 2.5; Flash-Lite 2.5
- **OpenAI-compatible (cloud)** — single dropdown with 8 sub-presets:
  - xAI Grok (`api.x.ai`)
  - Groq (`api.groq.com` — fast inference; **NOT** the same as xAI Grok — UI tooltip clarifies)
  - DeepSeek (`api.deepseek.com`)
  - Mistral (`api.mistral.ai`)
  - OpenRouter (`openrouter.ai` — meta-aggregator, 100+ models, free-form picker, sends `HTTP-Referer` + `X-Title` headers)
  - Perplexity (`api.perplexity.ai`)
  - Together AI (`api.together.xyz` — free-form picker)
  - Custom URL — auto-appends `/chat/completions` if missing
- Ollama (existing)
- LM Studio (existing)

**1 new cloud Whisper transcription host:**
- `Settings::cloud_transcription_provider: CloudTranscriptionProvider` (`OpenAi | Groq`)
- Groq Whisper-Large-v3 — same multipart-form OpenAI-style endpoint, ~10× faster, generous free tier
- Key slot follows: `openai` for OpenAI Whisper, `compat:groq` for Groq Whisper (shared with Groq text processing)

### Architecture

**`processing/providers/`** is the new home for protocol code:
- `mod.rs` — `Protocol` enum, `complete()` dispatcher with shared retry + fence-strip
- `openai.rs` — `build_request` + `parse_response` for OpenAI chat
- `anthropic.rs` — Anthropic Messages handler
- `gemini.rs` — Gemini handler with safety + URL-path model
- `catalog.rs` — single source of provider metadata (display name, endpoint, default model, recommended list, key slot, protocol, stable-catalog flag)
- `dynamic_models.rs` — `/models` fetch with 24h disk cache in `model_cache.json`

**`processing/mod.rs::gpt_complete`** is now a 3-line wrapper around `providers::complete`. The retry loop, error mapping, and code-fence stripping all live in `providers/mod.rs::complete`. Per-protocol code only handles request building + response parsing.

**`GptConfig::protocol: Protocol`** field added — `build_gpt_config(settings, _)` resolves it from `providers::catalog::for_settings(settings)`. Local providers (Ollama, LM Studio) build endpoint from `settings.text_processor_url` at runtime; hosted providers use the catalog's static endpoint.

### Multi-key storage

**`storage/api_keys.rs`** — replaces single `~/.local/share/TypeUrVoice/.api_key` with `~/Library/Application Support/TypeUrVoice/keys.json` (chmod 0600, atomic temp+rename writes). One slot per provider. Slot keys mirror the catalog:
- `openai`, `anthropic`, `gemini`
- `compat:grok`, `compat:groq`, `compat:deepseek`, `compat:mistral`, `compat:openrouter`, `compat:perplexity`, `compat:together`, `compat:custom`

Each OpenAI-compatible preset has its own slot (option `c` from the implementation plan) — switching from Grok to DeepSeek does NOT delete your Grok key, so users can flip back instantly.

**Migration:** `migrate_legacy_if_needed()` runs once at app start. If `.api_key` exists and `keys.json` doesn't: copy old → `keys.json["openai"]`, delete old. Logged as `api_keys: migrated legacy .api_key → keys.json`.

**Tauri commands** all grew a `slot: String` parameter: `save_api_key`, `get_api_key`, `delete_api_key`, `test_api_key`. `test_provider_key` in `transcription/cloud.rs` probes the right endpoint per slot:
- `openai` / `compat:*` → `GET /models` with Bearer key
- `anthropic` → tiny `POST /v1/messages` with `max_tokens: 1` (no reliable GET /models)
- `gemini` → `GET /v1beta/models` with Bearer key
- `compat:custom` → can't probe without a known URL; returns false

### Dynamic model catalogue (E8)

`fetch_provider_models(slot)` Tauri command + `dynamic_models::fetch_models()` backend. Cache: 24h, persisted to `~/Library/Application Support/TypeUrVoice/model_cache.json`. Per-provider parsers:
- OpenAI-style → `data: [{id}]` (works for OpenAI, Grok, Groq, DeepSeek, Mistral, OpenRouter, Together)
- Anthropic → `data: [{id}]`
- Gemini → `models: [{name, supported_generation_methods}]`, filtered for `generateContent`, prefix-stripped to remove `models/`
- Perplexity → no `/models` endpoint, returns empty (UI keeps the static list)

UI in `EngineTab::ModelPicker` exposes a "Refresh from provider" button. Result merges with the catalog's `recommended` list (catalog first, then any new ones from /models). Free-form (non-stable-catalogue) providers get an Input field with suggestions instead of a dropdown.

### Settings additions (all `#[serde(default)]`)

```rust
pub enum TextProcessor {
    OpenAi, Anthropic, Gemini, OpenAiCompat, Ollama, LmStudio,
}
pub enum OpenAiCompatPreset {
    Grok, Groq, DeepSeek, Mistral, OpenRouter, Perplexity, Together, Custom,
}
pub enum CloudTranscriptionProvider { OpenAi, Groq }

pub struct Settings {
    // ... existing
    pub openai_compat_preset: OpenAiCompatPreset,    // default Grok
    pub openai_compat_url: String,                    // default empty
    pub cloud_transcription_provider: CloudTranscriptionProvider,  // default OpenAi
}
```

`TextProcessor` and `OpenAiCompatPreset` both have `as_str()` impls (still warned as dead-code in this build — exercised at runtime for serde).

### UI rewrite (EngineTab)

**Provider section becomes a `<select>` dropdown** (was: 3-button row). Six options. When OpenAI-compatible is selected, a second dropdown appears for the preset; selecting Custom shows a URL input.

**`KeyRow` component** (new, scoped to a slot) handles the key input + saved-key display + Test button + status badge. Used by both the cloud Whisper key field (slot resolved from `cloud_transcription_provider`) and the text-processing key field (slot resolved from `providerInfo.keySlot`).

**`ModelPicker` component** (new) picks between dropdown vs free-form input based on `providerInfo.stableCatalog`. "Refresh from provider" button hits `/models` and merges results.

**`get_active_provider_info` Tauri command** returns `ProviderInfo { displayName, endpoint, defaultModel, recommended, keySlot, requiresKey, stableCatalog }` for the currently selected provider — drives the UI's conditional rendering.

### Help tab refresh (E9)

Engine section completely rewritten — every new provider documented with a one-paragraph blurb + where to get a key (platform.openai.com, console.anthropic.com, aistudio.google.com, etc.). Privacy Notes updated for `keys.json`. Troubleshooting section added entries for "LLM connection failed / 429" (mentions the retry) and "My provider released a new model — where is it?" (points at Refresh from provider).

### Default outputMode

Reverted to `Clean` (was flipped to `Raw` in Phase A5). Rationale: B4 made Clean actually do something useful (filler removal + grammar), so it's worth running. Affects both Rust `Settings::default` and TS `DEFAULT_SETTINGS`.

### New things to check in `crash.log`

- `api_keys: migrated legacy .api_key → keys.json` — one-time migration completed
- `LLM request → <endpoint> (model: <m>, protocol: <p>, temp: <t>)` — every LLM call shows protocol + temp
- `LLM transient error 429 — retrying in 2s` — retry happened (B6)
- Anthropic-specific: `Anthropic: hit max_tokens — output may be truncated`
- Gemini-specific: `Gemini finish_reason: <reason> — output may be incomplete` (anything other than STOP / MAX_TOKENS)

### Files added
- `src-tauri/src/processing/providers/mod.rs`
- `src-tauri/src/processing/providers/openai.rs`
- `src-tauri/src/processing/providers/anthropic.rs`
- `src-tauri/src/processing/providers/gemini.rs`
- `src-tauri/src/processing/providers/catalog.rs`
- `src-tauri/src/processing/providers/dynamic_models.rs`
- `src-tauri/src/storage/api_keys.rs`

### Verification
- cargo build --release: clean
- cargo test --lib: 6/6 pass
- npm run build: clean, 293.22 kB / 88.16 kB gzipped
- DMG: 7.9 MB

---

## Custom Output Profiles (May 2026, commit `87e45d2`)

Replaces the single Custom prompt textarea (Phase D6) with a library of named profiles. User can keep "Email tone", "Tweet under 280", "Bullet points only", "Strip emojis" etc. side by side and quick-switch between them from either the tray or Settings.

### State changes (`state.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomProfile {
    pub id:     String,    // crypto.randomUUID() from JS — stable across renames
    pub name:   String,
    pub prompt: String,
}

pub struct Settings {
    // ... existing
    pub custom_prompt: String,                            // LEGACY — kept for migration
    pub custom_profiles: Vec<CustomProfile>,              // #[serde(default)]
    pub active_custom_profile_id: Option<String>,         // #[serde(default)]
}

impl Settings {
    pub fn active_custom_prompt(&self) -> Option<&str> {
        let id = self.active_custom_profile_id.as_deref()?;
        let p = self.custom_profiles.iter().find(|p| p.id == id)?;
        let trimmed = p.prompt.trim();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    }
}
```

### Migration in `lib.rs::run` setup

If `custom_profiles` is empty AND `custom_prompt` is non-empty (post-trim):
- Generate id = `format!("legacy-{}", now_ms)`
- Push `CustomProfile { id, name: "My prompt", prompt: <legacy> }`
- Set `active_custom_profile_id = Some(id)`
- Clear `custom_prompt` to suppress re-migration on subsequent runs
- `storage::settings::save()` immediately, log `"Migrated legacy custom_prompt → custom_profiles[0]"`

### Pipeline (`processing/mod.rs::process`)

Custom branch now reads `settings.active_custom_prompt()`. None ⇒ behave as Raw (no LLM call). Same fallback semantics as before.

### Tray menu (`tray.rs`)

When `settings.custom_profiles` is non-empty, the "Output Mode → Custom" entry expands into a sub-submenu with one item per profile + a "Manage profiles…" entry that opens Settings → Output. Empty list ⇒ flat "Custom" menu item (current behaviour).

Menu IDs use the prefix `mode_custom_profile:<uuid>` so the click handler can extract the profile id and atomically set both `output_mode = Custom` and `active_custom_profile_id = Some(id)`. Defensive check: if the profile id no longer exists (race with deletion in UI), the click is a no-op.

`build_menu` signature changed: now takes `&Settings` instead of `&str` so it has access to the profile list. `refresh_menu` snapshots the settings once and passes them in. Both `build_tray` (initial) and `refresh_menu` (on update) take this path.

### UI (`OutputTab.tsx::ProfileManager`)

- Chip row with each profile. Active chip is highlighted (`var(--bg-active)` + `var(--border-focus)`).
- Pencil icon → inline rename. Input auto-focuses + selects-all. Enter saves; Escape cancels; blur saves.
- Trash icon → delete. Auto-fallback: if the active profile is removed, jumps to the first remaining one (or `null` if list empties).
- "+ New profile" creates `{ id: crypto.randomUUID(), name: "Profile N", prompt: "" }`, immediately activates it, opens rename mode so the user names it.
- Empty state: dashed-border card with "Create your first profile" CTA.
- Active profile's prompt sits in a 140 px-min textarea below (3000-char hard cap). Empty prompt shows "Empty prompt → output falls back to Raw" hint.
- Char counter bottom-right: `<n> / 3000`.

### IDs

Generated client-side via `crypto.randomUUID()` (available in Tauri's WebView). Stable across renames so `active_custom_profile_id` never goes stale. The migration uses `legacy-<timestamp_ms>` as a special ID.

### Help tab

OutputTab section's Custom mode entry rewritten to mention profile chips, pencil/trash icons, the tray sub-submenu, and the "switch profile = effective on next recording" semantic.

### Files changed
- `src-tauri/src/state.rs` — `CustomProfile` struct, two new Settings fields, `active_custom_prompt()` helper
- `src-tauri/src/processing/mod.rs` — Custom branch reads `active_custom_prompt()`
- `src-tauri/src/lib.rs` — first-run migration in `run()` setup
- `src-tauri/src/tray.rs` — `build_menu(&Settings)` + sub-submenu + per-profile click handler + `manage_profiles` entry
- `src/types/index.ts` — `CustomProfile` type + Settings fields
- `src/hooks/useSettings.ts` — defaults
- `src/components/settings/OutputTab.tsx` — full ProfileManager component
- `src/components/settings/HelpTab.tsx` — Custom mode docs refresh

### Verification
- cargo build --release: clean (2 dead-code warnings on `as_str` impls — pre-existing from Phase E)
- cargo test --lib: 6/6 pass
- npm run build: clean, 297.78 kB / 89.50 kB gzipped
- DMG: 8.0 MB

### New things to check in `crash.log`

- `Migrated legacy custom_prompt → custom_profiles[0]` — first launch with this build, only fires for users who had set up the single-textarea Custom mode in the previous build
- `Failed to persist custom-prompt migration: <err>` — settings.json couldn't be written during migration; non-fatal, in-memory state still valid for this session

---

## Onboarding Reappearance Fix — round 2 (May 2026)

The April fix made deserialization tolerant to missing fields, but users could
still see the Welcome flow on every launch when `onboardingComplete` was
already `true` on disk. Different root cause this time: the in-memory state
in the Settings UI was overwriting the on-disk flag with `false` before the
next launch could read it. Plus one Tauri 2 footgun in the onboarding
finish() path that left users in a half-completed state.

### Three concurrent root causes

1. **`useSettings` race writing back `DEFAULT_SETTINGS`.**
   `src/hooks/useSettings.ts` initialises React state with `DEFAULT_SETTINGS`
   (which has `onboardingComplete: false`) and then resolves the real
   settings via `api.getSettings()` asynchronously. The `update()` callback
   used `settings` from its closure as the merge base. If anything triggered
   an `update()` before the initial fetch resolved — a `settings-changed`
   event from the tray, a tab effect, a programmatic auto-save — the merge
   base was `DEFAULT_SETTINGS` and the write to disk included
   `onboardingComplete: false`. Next launch: onboarding shows even though
   the user had completed it.

2. **`Onboarding.tsx::finish()` threw on Tauri 2 duplicate-label.**
   ```js
   const settingsWin = new WebviewWindow('settings')   // throws — label already in tauri.conf.json
   ```
   `'settings'` is already declared in `tauri.conf.json` with `visible: false`.
   On Tauri 2, calling `new WebviewWindow` with a label that already exists
   throws synchronously. The promise rejected after `api.updateSettings`
   resolved, so the flag *did* save — but the user saw onboarding hide
   without the settings window opening, and any errors after the save
   weren't surfaced.

3. **No defensive guard.** A single bool on disk was the only signal of
   "user has been through setup." Any future drift in the save path could
   re-trigger onboarding with no fallback. Users who clearly had prior
   configuration (API keys, downloaded models) deserved a self-healing
   path.

### Fix

**`src/hooks/useSettings.ts`** — `update()` now reads its merge base from
a `useRef` mirror that's only populated after `getSettings()` resolves,
and refuses to write while `loadedRef.current === false`:

```ts
const latestRef = useRef<Settings>(DEFAULT_SETTINGS)
const loadedRef = useRef(false)

const update = useCallback(async (patch) => {
  if (!loadedRef.current) {
    console.warn('useSettings.update() called before initial load — ignoring patch:', patch)
    return
  }
  const next = { ...latestRef.current, ...patch }
  applySettings(next)
  await api.updateSettings(next)
}, [applySettings])
```

The `applySettings` helper writes to both the ref and React state so the
two stay in sync; the `settings-changed` listener routes through it too.

**`src/components/onboarding/Onboarding.tsx`** — `finish()` saves first
(unchanged), then reaches for the pre-declared settings window by label
instead of constructing a new one:

```ts
const existing = await WebviewWindow.getByLabel('settings')
if (existing) {
  await existing.show()
  await existing.setFocus()
}
```

`getCurrentWindow().hide()` now wraps in its own try/catch so a hide
failure doesn't bubble up.

**`src-tauri/src/storage/api_keys.rs`** — added `pub fn has_any() -> bool`
that returns true if any provider slot in `keys.json` holds a non-empty
value.

**`src-tauri/src/lib.rs`** launch path — defense-in-depth + diagnostic
logging. After loading settings, if `onboarding_complete == false` but
the user has *any* saved API key or *any* downloaded `*.bin` in the models
directory, mark onboarding complete in memory and self-heal the flag on
disk:

```rust
let has_any_key = storage::api_keys::has_any();
let has_any_model = /* scan models_dir for *.bin */;
let onboarding_complete = if settings.onboarding_complete {
    true
} else if has_any_key || has_any_model {
    log_to_file(&format!(
        "Onboarding self-heal: onboarding_complete was false but has_any_key={} has_any_model={} — marking complete",
        has_any_key, has_any_model
    ));
    settings.onboarding_complete = true;
    let _ = storage::settings::save(&settings);
    true
} else {
    false
};
log_to_file(&format!(
    "Launch: onboarding_complete={} (has_any_key={}, has_any_model={})",
    onboarding_complete, has_any_key, has_any_model
));
```

Every launch now logs the resolved `onboarding_complete` plus both
signals, so future bug reports can pinpoint the cause from a single
`crash.log` line. The self-heal also writes settings back to disk
immediately so the flag stays correct from then on.

### Diagnostic dead-end that proved useful

Before patching, I dropped the user's live `~/Library/Application Support/TypeUrVoice/settings.json`
into a standalone Cargo project with a hand-stripped copy of the `d7ba29f`
(May 11 build) `Settings` struct and confirmed `serde_json::from_str` returns
`Ok(s)` with `s.onboarding_complete == true`. That ruled out a parse-error
regression and pointed the investigation at write-side races instead of
read-side tolerance. Useful technique to keep in mind for future "settings
file looks fine but app disagrees" reports.

### Files changed
- `src/hooks/useSettings.ts`
- `src/components/onboarding/Onboarding.tsx`
- `src-tauri/src/storage/api_keys.rs`
- `src-tauri/src/lib.rs`

### New things to check in `crash.log`

- `Launch: onboarding_complete=<bool> (has_any_key=<bool>, has_any_model=<bool>)` — logged unconditionally on every launch. First place to look if onboarding shows unexpectedly: is `onboarding_complete=false`? Then check the two signals — if both are false the user really hasn't configured anything yet.
- `Onboarding self-heal: onboarding_complete was false but has_any_key=… has_any_model=… — marking complete` — the defensive path fired. Indicates the disk flag was wrong; we corrected it. If this fires repeatedly across launches, settings.json save is failing somehow — chase that.
- `Failed to persist onboarding self-heal: <err>` — the self-heal couldn't write back; in-memory state still treats onboarding as complete for this session.

### Verification
- `cargo build --release` → clean (6 pre-existing dead-code warnings on
  `DictEntry`/`DictPack` fields from the in-progress CE/PL profile work).
- `npm run build` → clean, 303.47 kB / 91.04 kB gzipped.
- `npm run tauri build` → `.app` produced; `bundle_dmg.sh` still fails
  (known issue, unrelated).
- Killed the running May 11 process and `ditto`-replaced
  `/Applications/TypeUrVoice.app` with the fresh build. Installed binary
  now dated May 23 07:11.


---

## October 2026 Refresh (branch `oct-2026-refresh`)

Six commits: model refresh, filler removal, GPU speed-up, live preview, built-in LLMs, docs. Settings changes are additive (struct-level `#[serde(default)]` hands missing fields `Settings::default()` values — **don't add field-level `#[serde(default)]` to a bool that defaults to `true`**, it would give `false`). DMG: 16 MB (+8 MB is the llama-server sidecar).

### Toolchain notes (read first)
- **Rust ≥ 1.99 required.** Rust 1.94 + the Aug-2026 macOS linker (ld-27037) produce broken proc-macro dylibs in release builds ("mis-aligned LINKEDIT string pool"). `rustup update stable` fixed it.
- **`src-tauri/.cargo/config.toml` sets `GGML_NATIVE=OFF` + `GGML_CPU_ARM_ARCH=armv8.5-a+fp16`.** whisper-rs-sys forwards `GGML_*` env vars to cmake. Before this, whisper.cpp was built with `-mcpu=native`, so a DMG built on an M4 could SIGILL on an M1. Changing these needs `cargo clean -p whisper-rs-sys` (the build script doesn't watch them).

### 1. Models (`b7e1d98`)
- Catalog defaults: OpenAI `gpt-6-luna`, Anthropic `claude-haiku-5-5`, Gemini `gemini-3.5-flash-lite` (2.5 family shuts down Oct 2026), Grok `grok-4.1-fast`, DeepSeek `deepseek-v4-flash`, Groq `gpt-oss` added, Ollama `gemma4`.
- Request shapes: OpenAI reasoning models (`gpt-5*`, `gpt-6*`, `o*`, api.openai.com only) get `reasoning_effort` (`minimal` for gpt-5/-mini/-nano, `none` for 5.x/6, `low` for o-series) + `max_completion_tokens`, no `temperature` — see `openai_reasoning_effort()`. Claude 5-gen: no temperature, `output_config.effort: low`, +2048 max_tokens headroom. Gemini 3: default temperature, `thinkingLevel: low` (not for flash-lite).
- Cloud STT: OpenAI default `gpt-transcribe`, Groq `whisper-large-v3-turbo`; `Settings::cloud_transcription_model: Option<String>` + picker (override ignored if it doesn't belong to the provider).
- `storage::settings::migrate_retired_models()` rewrites old/retired `processing_model` ids on launch (`RETIRED_MODELS` table). Logs `Migrated retired model X → Y`.

### 2. Filler removal (`f92c1fa`)
- `processing/fillers.rs::remove_fillers()` — shape matchers for um/uh/er/ah/hmm/mm + Czech/Slovak ehm/eee/ééé. Pieces with a hyphen or any non-letter are never touched; single letters never removed. Repairs commas, sentence terminators, capitalisation.
- Runs in `transcription::transcribe` after the hallucination filter (all modes incl. Raw) and in live preview. `Settings::remove_fillers` (default true). Log: `Filler filter: N → M chars`.

### 3. Speed (`f15410e`)
- whisper-rs 0.13 → **0.16** (API: `full_n_segments()` returns i32, `get_segment(i)?.to_str_lossy()`, `full_lang_id_from_state()` returns i32) with **`metal`** feature (macOS target dep) + `flash_attn`, explicit `n_threads` (≤8).
- Benchmark (13.5 s Czech clip, turbo-q5, M4 Pro, warm): CPU 2.4 s → Metal 0.73 s. Run: `TYPEURVOICE_BENCH_WAV=<16k mono wav> cargo test --release --lib bench_local -- --ignored --nocapture`.
- `prewarm_local_model()` loads+verifies+runs 1 s of silence at launch, on engine/model change, after download. `audio::buffer::trim_silence()` (relative-energy, 250 ms padding, untouched for pure silence so dead-mic diagnostics still work). `AppState::capture_done` replaces the fixed 200 ms post-stop sleep. New-install default `turbo-q5`.
- crash.log: `Timing: recorded Xms`, `Timing: transcribe/process/paste`, `Silence trim: a → b s`, `Prewarmed local model … in …`.

### 4. Live preview (`7e4d0a5`)
- `transcription/live.rs`: every 700 ms re-transcribes the last 30 s with the cached context (separate WhisperState), non-overlapping; emits `partial-transcript`. `AppState::preview_busy` — stop path waits ≤1.5 s for an in-flight preview before the final pass. Busy flag is claimed *before* re-checking stop (race-free).
- Overlay: `set_overlay_size()`; pill 200×44, live 460×96 (`show_live_overlay`). `RecordingOverlay.tsx` shows the ~110-char tail. Final paste still comes from the full pass. `Settings::live_preview` (default true, Local engine only).

### 5. Built-in LLMs (`5b0c5fc`)
- `scripts/build-llama-server.sh` — llama.cpp **v0.6.0**, static, Metal embedded, `LLAMA_OPENSSL=OFF`, M1 baseline, fails if any non-system dylib is linked. Output `src-tauri/binaries/llama-server-<triple>` (git-ignored), bundled via `bundle.externalBin`, built automatically by `npm run dmg` (step 0).
- Why a sidecar: whisper-rs and llama.cpp both vendor ggml (link collision); separate process = model crash can't kill TypeUrVoice.
- `local_llm/models.rs`: qwen3.5-0.8b (Q8), **qwen3.5-2b** (default, Q4_K_M), qwen3.5-4b, gemma-4-e2b — Apache-2.0, HF LFS SHA-256 pinned (0.8b and 2b re-verified with `shasum`; 4b/gemma not downloaded locally — disk was full — rely on HF's published oid, still verified on download). Stored in `~/Library/Application Support/TypeUrVoice/llm-models/`.
- `local_llm/server.rs::LlmServer` (managed as `Arc`): `ensure_running` (serialised by `load_lock`, reuses a live process, SHA-256 once per (key, mtime), free 127.0.0.1 port, `-c 4096 -ngl 99 -np 1 --no-webui`, waits for `/health` ≤90 s), `stop`, lock-free `stop_now` on `RunEvent::Exit`, PID file + `kill_orphan()` (only kills a process named llama-server), `run_idle_watcher` (20 s tick). Logs go to `llama-server.log` next to crash.log.
- Settings: `builtin_llm_model`, `builtin_llm_auto` (true = load on hotkey press while speaking, unload after `builtin_llm_idle_minutes`, default 5; false = load at launch, keep until Quit). Raw mode never loads it.
- Pipeline: `TextProcessor::BuiltIn` → `run_pipeline` calls `ensure_running` and puts the base URL into the per-recording settings clone's `text_processor_url`; `build_gpt_config` treats BuiltIn like Ollama, sets `GptConfig::disable_thinking` → `chat_template_kwargs.enable_thinking=false`.
- Measured (release, M4 Pro): cold start 3.0 s incl. hash; cleanup call ~0.2–0.6 s; ~1.5 GB RSS for 2B.
- Lifecycle test: `cargo test --release --lib builtin_llm_lifecycle -- --ignored --nocapture` (uses `tauri::test::mock_app`; `LlmServer` methods are generic over `R: Runtime` for this).
- `storage/download.rs::download_verified` — shared by Whisper + LLM downloads; writes `<file>.partial`, verifies, then renames.
- UI: `BuiltInLlmPanel.tsx` (models, download progress, auto-unload toggle + minutes, status dot, Unload now). Events: `builtin-llm-status`, `llm-download-progress|done|error`.

### New crash.log lines
`Migrated retired model …`, `Filler filter: …`, `Silence trim: …`, `Timing: …`, `Prewarmed local model …`, `Live preview stopped: …`, `Built-in LLM ready: <model> on port <p> in <ms>`, `Built-in LLM failed to start: …`, `Built-in LLM idle for N min — unloading`, `Killed orphaned llama-server (pid …)`.

### Known follow-ups
- Release signing/notarization + updater activation (Developer ID now available) — sign the `llama-server` sidecar too (needs the same hardened-runtime entitlements; JIT not required).
- `src/locales/en.json` is not imported anywhere and still describes old models — delete or wire up.
- `privacy_mode` is defined in Settings but not enforced anywhere; when it is, include `BuiltIn` as a local processor.
- Pre-existing: `run_pipeline` checks the *Whisper-slot* key when the text processor is OpenAI (`lib.rs` Step 2) — wrong slot when cloud STT is Groq.

---

## Knock to Record (October 2026, branch `knock-to-record`)

A double (or triple) knock on the MacBook case starts/stops recording — Toggle semantics, primary-shortcut language. Settings → General → "Knock to Record" (only rendered when `knock_available` is true). Off by default. Settings: `knockEnabled`, `knockPattern` (`double`|`triple`), `knockSensitivity` (`low`|`medium`|`high`) — additive, struct-level `#[serde(default)]` covers old files.

### Sensor (`src-tauri/src/knock/sensor.rs`, macOS only)
- Apple Silicon MacBooks have an undocumented IMU behind the SPU: `AppleSPUHIDDevice`, usage page 0xFF00, usage 3. Streams only after `SensorPropertyReportingState=1`, `SensorPropertyPowerState=1`, `ReportInterval=1000` are set on **its own** `AppleSPUHIDDriver` (a descendant of the device — we don't wake the gyro/ALS drivers). 22-byte reports, X/Y/Z int32 LE Q16 (g) at offsets 6/10/14, ~800 Hz.
- No sudo needed (verified M4 Pro). Raw IOKit/CF FFI, callback on a dedicated `knock-sensor` thread's CFRunLoop; callback context owned via `Box::into_raw`, panics caught before the C boundary.
- **Never powers the sensor down or changes its rate** — other apps (e.g. the Knock app, Logi Options+) hold it open too. Re-sends the wake properties if no samples arrive for 2 s (system sleep).
- Typing veto: `CGEventSourceSecondsSinceLastEventType` (key down/up, mouse down/up) — needs no Input Monitoring permission.

### Detector (`src-tauri/src/knock/detector.rs`, pure, unit-tested)
Tuned on a labelled calibration recording. Findings that shaped it:
- Every bump (knock, keystroke, desk thump) makes the chassis ring at ~14 Hz for ~150 ms, identically — the waveform can't tell a knock from typing. A fresh strike is distinguished by 40–150 Hz energy, which ringing lacks → 40 Hz Butterworth high-pass per axis, then magnitude.
- Knuckle knocks peak at 0.019–0.042 g after the high-pass; still laptop < 0.006 g. Thresholds: High 0.010, Medium 0.013, Low 0.017.
- Natural double knocks are fast (~0.15–0.18 s apart); the second lands while the first still rings, which plain energy/STA-LTA detectors miss.
- Rules: 20 ms peak hold, 120 ms refractory, echo = impact <0.3 s after a 2.5× stronger one (ignored), knocks within 2.5× of each other, gaps ≤ 0.6 s, 0.5 s quiet before, and a confirmation wait of max(0.35 s, 1.3 × longest gap) with no further impact before firing (steady rhythms always show their next beat first). Recording therefore starts ~0.35 s after a quick knock-knock.
- Replay result on the calibration data: 11/11 doubles detected at every sensitivity; 0 fires on single knocks, typing (even without the keyboard veto), trackpad, handling. Desk knocks next to the laptop also register (5–8 of 8).

### App wiring (`lib.rs`, `knock/mod.rs`)
- `KnockListener` (managed state) + `knock::sync(app)` — called in `setup()` and on every `update_settings`; starts/restarts/stops the sensor thread to match settings (no-op if unchanged).
- A pattern → `on_knock(app, since_first_knock)` on a fresh thread → start (primary language) / stop / ignore while Processing.
- Knock-stop trims `since_first_knock + 150 ms` off the end of the audio (`stop_audio_capture_and_process(app, trim_tail)`), so the thumps never reach Whisper.

### Testing
- Unit: `cargo test --lib knock`
- Replay a recording: `TYPEURVOICE_KNOCK_CSV=samples.csv TYPEURVOICE_KNOCK_EVENTS=events.csv cargo test --release --lib knock_replay -- --ignored --nocapture`
- Live, real sensor + typing veto: `TYPEURVOICE_KNOCK_SECS=120 [TYPEURVOICE_KNOCK_SENS=high] [TYPEURVOICE_KNOCK_TRIPLE=1] cargo test --release --lib knock_live -- --ignored --nocapture`

### crash.log lines
`Knock: listening (2 knocks, Medium sensitivity)`, `Knock: listener stopped`, `Knock: 2 knocks over 170ms, peak 0.03g`, `Knock: starting recording` / `stopping recording` / `ignored while processing`, `Knock: trimmed last Nms of audio`, `Knock: accelerometer silent (N samples so far) — re-waking`, `Knock: listener failed: …`, `Knock: enabled, but this Mac has no accelerometer`.

### Follow-ups on the same branch (Oct 31)
- **Paste** (`paste/clipboard.rs`): osascript `keystroke` replaced by a native CGEvent Cmd+V posted by TypeUrVoice after `AXIsProcessTrusted()` (all pointers null-checked; no prompt per paste). A blocked paste is now an error → crash.log `Paste: Cmd+V simulation failed: …` + notification "Text copied — press Cmd+V…". Launch logs `Launch: accessibility_trusted=<bool>`.
- **Signing**: unsigned (ad-hoc) builds change signature every rebuild, so macOS silently drops the Accessibility grant — the recurring "paste stopped working". The repo default `tauri.conf.json` → `bundle.macOS.signingIdentity` is `"-"` (ad-hoc) so anyone can build. For your own builds, export `APPLE_SIGNING_IDENTITY="Apple Development: <you> (<TEAMID>)"` before `npm run tauri build` / `npm run dmg` so the signature (and the Accessibility grant) stays stable. `entitlements.plist` gained `com.apple.security.automation.apple-events` (hardened runtime). If trust is still false after granting: remove every TypeUrVoice entry in Accessibility and re-add `/Applications/TypeUrVoice.app` (stale entries from old builds share the bundle id).
- **Overlay** is non-focusable (`focus/focusable: false` + `set_focusable(false)`); with live text on, the bubble returns after the paste for 3.5 s with the final text (`final-transcript` event). Live preview starts at 0.5 s of audio, 450 ms ticks; crash.log `Live preview: N update(s)`.
- **Clean guard** (`processing/guard.rs`): word-aligns Clean output with the transcript; a swap to a dissimilar word (≥4 letters, accent-folded similarity < 0.6, not a dictionary term) is put back; >40% of words changed ⇒ raw transcript. crash.log `Clean guard: put back N word(s)…` / `…rejected (…) — using the transcript`. Replay on a history.db: `TYPEURVOICE_HISTORY_DB=… cargo test --lib guard_history -- --ignored --nocapture`.

---

## Rename to TypeUrVoice (November 2026, branch `nov-2026-model-refresh`)

The app, crate, bundle and data folder are now **TypeUrVoice** (was VoiceInk). Historical sections above were search-and-replaced, so older entries say "TypeUrVoice" even where the build at the time was called VoiceInk.

- `productName` TypeUrVoice, bundle id **`com.typeurvoice.app`** (was `com.voiceink.app`), Cargo crates `typeurvoice` / `typeurvoice_lib`, npm package `typeurvoice`, env vars `TYPEURVOICE_*`, log prefix `[TypeUrVoice]`, DMG `TypeUrVoice_<ver>_aarch64.dmg`.
- **Data folder** `~/Library/Application Support/TypeUrVoice/`. `storage::settings::migrate_legacy_data_dir()` runs first thing in `run()` and renames `…/VoiceInk` → `…/TypeUrVoice` if only the old one exists (settings, keys.json, models, llm-models, history.db, logs all move). If both exist the old one is left alone. crash.log: `Data dir: moved … → …` / `Data dir: both … exist …` / `Data dir: could not move …`.
- New bundle id ⇒ macOS treats it as a new app: Microphone, Accessibility and Notifications must be granted again, and "Start on login" must be re-enabled. Remove the old `/Applications/VoiceInk.app` (quit it first — it would otherwise keep writing to the old folder).

---

## Spoken-Language Check (November 2026, branch `nov-2026-model-refresh`)

**Bug:** Czech speech on the English shortcut (Control+W → `en`) came out as nonsense English with Czech words left in ("I wanted to use the matici, but also the same."). The forced `language=en` made whisper do its own speech translation, which turbo (large-v3-turbo) was never trained for. Translation then ran English → English and changed nothing (history ids 40–43).

**Fix:**
- `transcription/local.rs::spoken_language_if_different()` — when a language is forced and there are ≥2 s of audio, runs `pcm_to_mel` + `lang_detect` first. If `should_use_spoken_language()` (in `transcription/mod.rs`, unit-tested) says so, Whisper transcribes in the detected language instead. Rule: detected p ≥ 0.70, requested p ≤ 0.10, and the pair is not cs/sk.
- `TranscriptionResult::requested_language` is set when that happens. `lib.rs` then sets `source_language` to the spoken language and turns on translation into the requested one. The shortcut's language is always the output language; it overrides the Settings translation target.
- `processing::process` skips the LLM translation call when source == target (it used to translate en → en).
- Cloud engine unchanged (the APIs don't return language probabilities).
- Cost: one extra encoder pass, ~0.6 s on M4 Pro with turbo-q5 (6 s clip: 0.68 s → 1.28 s).
- Bench any language: `TYPEURVOICE_BENCH_WAV=… TYPEURVOICE_BENCH_LANG=en cargo test --release --lib bench_local -- --ignored --nocapture`.

crash.log: `Language check: requested=en p=0.00, detected=cs p=1.00 → transcribing as spoken`, `Language switch: spoke cs with en requested — translating to en`.

---

## Speaker Learning, Better Translation, Typing While Speaking (November 2026, branch `nov-2026-model-refresh`)

User reports: Czech → English translations flipped meaning ("u sebe" → "scattered", negative questions came out as negative statements), misheard Czech words were translated literally ("loop" → "thief", "pozádku" → "in the back"), and the speaker can't pronounce ř (Whisper writes ž/z: "heždčí" = hezčí). They also wanted text to land in the text field sentence by sentence, not only in the bubble.

### 1. Translation prompt (`processing/translation.rs`)
- Tells the model the input is a speech transcript with possibly misheard words → translate what was most likely said; never add/drop a negation; questions stay questions; sound like the speaker.
- `language_notes("cs"|"sk")`: negative questions are suggestions ("Nebylo by lepší…?" = "Wouldn't it be better…?"), double negation = one negation, ř/ž/z and accent mishearings.
- Gets `SpeakerContext::translation_hint` (notes, learned profile, up to 3 relevant History corrections, earlier part of the dictation when streaming).
- Replay on real history: `TYPEURVOICE_LLM_URL=http://127.0.0.1:8090 TYPEURVOICE_HISTORY_DB=… [TYPEURVOICE_SPEAKER_NOTE=…] cargo test --release --lib translate_history -- --ignored --nocapture` (start `src-tauri/binaries/llama-server-*` with the GGUF first). With Gemma 4 E2B: fixed "pozádku", the negative question and "nothing is spoken"; still wrong on "u sebe", "heždčí", "kytičků" — the 2B model is the limit. Model notes now point at Gemma 4 E4B / Qwen3.5 9B for translation.
- Clean prompt gained a mishearing rule (replace a non-word with the near-identical real word). The Clean guard allows these (accent-folded similarity stays high).

### 2. Speaker profile (`storage/speaker.rs`, `processing/learn.rs`, `processing/speaker.rs`)
- `speaker_profile.json` (0600, atomic write) — NOT in settings.json, because the Settings UI writes the whole settings object back and would overwrite background updates. Fields: topics, terms, recognition ("heard → meant" + pronunciation pattern), style, updatedAt, lastHistoryId.
- `learn()` sends the current profile, `speakerNotes`, ≤6 corrections and the newest dictations (≤5000 chars, RECOGNIZED vs FINAL) to the text processor; expects four `KEY:` lines; `parse()` rejects anything else (old profile kept, `lastHistoryId` advanced so it doesn't retry every recording).
- `maybe_learn` (lib.rs) runs after a successful pipeline when `learnFromHistory`, the processor is on-device (BuiltIn/Ollama/LmStudio) and ≥8 new dictations since `lastHistoryId`; waits 10 s and only runs if Idle. `start_audio_capture` aborts it (`LEARN_TASK`) so it never holds the single-slot llama-server during a recording. Online processors: only via "Update now" (`learn_speaker_profile_now`).
- Used by: Clean/Polished (`cleanup_hint`: notes + recognition + terms only — topics/style would invite rewriting), translation (full), Whisper (`transcription::vocabulary_prompt` = dictionary + learned terms, ≤40).
- **Local Whisper now gets an initial prompt** (it never used the dictionary before; cloud already did). If the output is just an echo of the prompt (silence), it's dropped (`echoes_prompt`).
- Settings: `speakerNotes` (string), `learnFromHistory` (default true). UI: `YourVoicePanel.tsx` at the top of the Dictionary tab (notes saved on blur; profile editable; Save / Update now / Clear).

### 3. Corrections (`storage/history.rs`)
- New table `corrections` (history_id, kind `translation`|`cleanup`, source, output, corrected). `save_correction` replaces the entry's translated/processed text, one correction per entry. `relevant_corrections` picks the 3 sharing the most ≥4-letter words with the new text (else most recent).
- `HistoryEntry.corrected` (EXISTS subquery). `clear_all` also clears corrections; retention cleanup doesn't.
- UI: History shows "You said" + "Translation", a pencil to fix the output (`correct_history_entry`), "translated" / "fixed by you" badges; copy copies the output (translation when present — it used to copy the Czech).

### 4. Typing while speaking (`stream.rs`, `transcription/live.rs`)
- Setting `streamTyping` (default false), active only with Local engine + autoPaste + no summary (`stream_typing_active`). Runs the live loop even with the bubble preview off (`Session::show_preview`).
- `local::transcribe_segments_blocking` returns segments with timestamps; `check_language` flag so the spoken-language check runs once per recording in the live loop (it used to run every tick), giving a `LangPlan` (whisper language + translate_to).
- Commit rule (`segments_to_commit`, unit-tested): leading segments that ended ≥1 s before the end of the audio, identical on two consecutive ticks, up to the last one ending in . ? ! …; ≥20 s waiting forces a commit of all but the last segment. Cut point = middle of the gap to the next segment. Only commits when the snapshot starts exactly at the committed sample (never skips audio).
- Worker types each chunk in order: `processing::process` with `earlier_source/earlier_output` context (translation keeps context), fallback to raw text on any error. `paste::type_text` posts CGEvent unicode strings (≤16 UTF-16 units/event, flags 0 so a held Option/Control can't make shortcuts); multi-line text is pasted instead (a typed Return would send chat messages). After a typing failure the rest is pasted at the end, in order.
- Stop: `StreamSession::finish()` waits for the worker (≤90 s); if anything was typed, the audio up to `committed` is dropped and `run_pipeline(prior)` transcribes only the remainder (skipped if <0.3 s), types it (paste fallback), puts the WHOLE dictation on the clipboard and in history. Nothing typed ⇒ normal full pass.
- Not testable without a mic — logic covered by unit tests only; needs a manual try in the app.

### New crash.log lines
`Speaker profile updated from N dictation(s) in Xms (K term(s))`, `Speaker profile: update failed: …`, `History: correction saved`, `Whisper echoed the vocabulary hint — treating as no speech`, `Stream: typed chunk in Xms`, `Stream: typing failed (…) — the rest is pasted at the end`, `Stream: processing failed (…)`, `Stream: N chunk(s) typed while speaking, Xs of audio left`, `Live preview: N update(s), K chunk(s) committed for typing`.

### Fix: "No such file or directory (os error 2)" after a model download
Switching Settings tabs unmounts the download panels, which reset `downloading` and re-enabled the Download button mid-download; a second click ran a second download into the same `<file>.partial`. The first one verified and renamed it, then the second failed on the missing `.partial` (the model itself was fine — SHA-256 matched). Fix: `DownloadSlot` in `lib.rs` (static in-flight list keyed `whisper:<key>` / `llm:<key>`, released on Drop) makes a duplicate request a no-op, and both panels re-attach from `*-download-progress` events. crash.log: `Download of llm:<key> already running — ignoring the second request`.

---

## Recording Bubble Fix, Hold-Key Watch, Sound While Recording, Markdown Mode (November 2026, branch `nov-2026-model-refresh`)

User report: during a 76 s dictation the bubble vanished ("didn't know if it was still recording"), and once a recording seemed to stop mid-sentence. crash.log/History showed every pipeline completing, so both came from the UI/hotkey side.

### 1. Bubble vanished after 60 s (`RecordingOverlay.tsx`)
- Root cause: a "stuck state" safety timer reset the overlay to idle after 60 s in *any* non-idle state — including Recording. Every dictation over a minute lost its indicator while audio kept recording.
- Fix: safety only in Processing, 380 s (backend pipeline timeout is 360 s and always ends in done/error). The timer turns orange in the last 30 s of the 5-min cap and shows "5:00 max reached".

### 2. Hold mode follows the physical key (`hold_key.rs`, `lib.rs::on_shortcut`)
- Carbon hot-key events can report Released while the key is still held (cuts the dictation) or lose the release (secure input — recording never stops).
- `hold_key::key_code(shortcut)` maps the parsed shortcut's `Code` to a mac virtual key code; `is_down()` = `CGEventSourceKeyState(HIDSystemState)` (no permission needed).
- Released while the key reads down ⇒ ignored. `spawn_hold_watcher` (per recording, keyed on `recording_started_at`) stops the recording when the key has read up for 400 ms with no release event. It only runs if the key reads down 150 ms after the press, so a key-state API that can't see keys falls back to the old event-only behaviour.
- `start_audio_capture` now returns `Option<Instant>` (start time; `None` if already recording).
- Note: the shortcut parser doesn't accept `Meta+…` (`Super+…` works) — pre-existing; the GeneralTab capture writes `Meta`.
- crash.log: `Hold: release event while the key is still held — ignored`, `Hold: key released without a release event — stopping`.

### 3. Sound while recording (`media.rs`, setting `mediaWhileRecording`: off|mute|pause, default off)
- `media::quiet` in `start_audio_capture` (before the mic opens), `media::restore` right after the stop transition and on `RunEvent::Exit`. Only what quiet changed is undone (static `UNDO`).
- Mute: CoreAudio `kAudioDevicePropertyMute` on the default output (main element, else channels 1–2); already-muted elements are left alone; unmutes the same device id even if the default changed.
- Pause: only if `kAudioDevicePropertyDeviceIsRunningSomewhere` on the default output (pressing Play/Pause with nothing playing would start the last player). Posts NX_KEYTYPE_PLAY as an `NSSystemDefined` event (raw objc runtime FFI → `CGEventPost`), again on restore. Needs Accessibility; without it falls back to Mute. Known limit: a browser can keep the device running a few seconds after the user pauses — then the key would resume it (documented in Help).
- macOS only; `media_control_available` command hides the UI elsewhere.
- Probe (read-only): `cargo test --lib media_probe -- --ignored --nocapture`.
- crash.log: `Media: sound muted while recording` / `unmuted`, `Media: playback paused while recording` / `resumed`, `Media: pausing needs Accessibility permission — muting instead`.

### 4. Markdown output mode (`OutputMode::Markdown`, `cleanup::markdown`)
- Dictation → structured Markdown, following spoken meta-instructions ("make it a list", "in English") without writing them into the text; technical terms stay in their usual (English) form.
- Settings: `markdownTemplate` (free text, empty ⇒ model picks; UI presets: AI-agent task, meeting notes, bug report), `markdownLanguage` ("auto" ⇒ translation target, which the shortcut-language switch sets, else spoken).
- Writes directly in the output language; the translation step is skipped for this mode (one LLM call). Overrides an active occupation profile's prompt (its dictionary still applies). Disables stream typing. Temperature 0.2, budget `token_budget_for(raw, 2.0)`.
- Learning shows the transcript as FINAL for markdown entries (restructured output says nothing about recognition).
- `strip_code_fence` now only strips fences tagged ""/markdown/md/text/plaintext, so a document that starts with a real code block survives.
- Tray: Output Mode → Markdown.
- Replay: `TYPEURVOICE_LLM_URL=http://127.0.0.1:8090 TYPEURVOICE_HISTORY_DB=… TYPEURVOICE_IDS=77,75 [TYPEURVOICE_MD_LANG=en] [TYPEURVOICE_MD_TEMPLATE='## Goal\n## Requirements'] cargo test --release --lib markdown_history -- --ignored --nocapture`. With Gemma 4 E4B: 2–3.5 s per dictation, correct English, sensible structure; "nahrávání" still sometimes becomes "upload" (recording/upload ambiguity, as in translation).
- crash.log: `Markdown: cs → en`.

### 5. Knock language (`Settings::knock_language`: primary|secondary|split|auto, default primary)
- User report: knocking always recorded in the primary language — English speech came out Czech (by design: a forced language is the output language, the spoken-language check translates into it).
- `split`: detector listens for 2 *and* 3 knocks (`Config { knocks: 2, max_knocks: 3 }`); `Pattern::knocks` says which. A double that a third knock could extend keeps its knocks in `pending` and waits `max(1.7 × gap, 0.45 s)` (vs `max(1.3 × gap, 0.35 s)`); a third knock inside the wait and ≤ MAX_GAP re-arms as a triple; a 4th cancels. `knock_pattern` is ignored in split mode.
- `auto`: passes `"auto"` as the recording language → Whisper detects, no translation.
- `on_knock(app, since_first, knocks)` picks the language; any pattern stops.
- Tests: `split_mode_tells_double_from_triple`, `split_mode_rejects_four_knocks_and_steady_bumps` (synthetic). Live: `TYPEURVOICE_KNOCK_SPLIT=1 cargo test --release --lib knock_live -- --ignored --nocapture`. Not re-tuned on a real calibration recording (none kept).
- crash.log: `Knock: listening (2–3 knocks, …)`, `Knock: starting recording (3 knocks, language en)`.

### 6. First word lost (Nov 2026)

**First word lost.** The bubble appeared before the mic opened: `start_audio_capture` emitted `recording` + `show_overlay` first and spawned the cpal thread last (after live preview / stream / LLM warm-up); device lookup + stream start take ~100 ms here (measured 86–112 ms), Bluetooth up to ~1 s. Now:
- the mic thread starts first; the bubble + `recording` event come from `announce_recording`, called on the first audio (`record_until_stopped`'s `on_live` callback) or after a 1 s fallback. crash.log `Timing: mic live after Xms` / `not delivering audio`.
- `run_pipeline` puts 0.25 s of silence in front of the audio (Whisper drops a first word that starts at sample 0).
- `record_until_stopped` no longer `clear()`s the buffer (fresh per session anyway).

**Auto language was English.** `transcribe_segments_blocking` never called `set_language` when no language was forced, so whisper.cpp used its default `"en"` — "auto" (Settings source language, knock "Detect") translated Czech to English. Now `set_language(Some("auto"))`. Verified: Czech clip → `cs p=0.997`.

**Mute recovery.** `media` writes `media_mute.txt` (device + elements) while it has the Mac muted; `media::recover()` at the top of `run()` unmutes if a previous run crashed / was force-quit mid-recording (happened once during an app swap). crash.log `Media: unmuted the sound a previous run left muted`.

**Wake phrase ("Hey MacBook") — built, then removed at the user's request (not reliable enough).** Approach for the record: always-open cpal stream + energy VAD + Whisper on ≤ 2.5 s utterance starts + fuzzy prefix match. Findings: turbo-q5 costs ~0.62 s per check whatever the clip length (30 s encoder window; `audio_ctx` shortening garbles turbo; reusing WhisperState doesn't help); small-q5 ~0.14 s and fine on real words, weak on invented names; prompting Whisper with the phrase makes it hallucinate the phrase (false triggers); auto language detect doubles the cost. In real use it still felt slow/unreliable. A dedicated keyword-spotting model would be the way if this comes back. Leftover settings keys `wakeEnabled`, `wakePhrase`, … in existing settings.json files are ignored.

---

## Windows port status — what the Windows build gets from the Nov 12 2026 work

Merge GitHub `main` into the Windows branch (`git fetch origin && git merge origin/main`); conflicts, if any, will mostly be in `src-tauri/src/lib.rs` and `state.rs`. Settings are additive (struct-level `#[serde(default)]`), so old settings.json files load.

Works on Windows as-is (platform-independent code):
- Recording bubble no longer vanishes after 60 s; 5-min cap warning (`RecordingOverlay.tsx`).
- Bubble appears only when the mic delivers audio + 0.25 s lead-in silence for Whisper (first word no longer lost).
- Local engine "auto" language really auto-detects (was silently English).
- Markdown output mode (+ tray item, template presets, language choice), code-fence strip tightened.
- Knock language setting (UI hidden on Windows — no accelerometer), detector changes are pure Rust.

macOS-only, with no-op stand-ins on Windows (compiles; feature simply absent / hidden in UI):
- **Sound while recording** (`media.rs`, `media_control_available` → false hides the UI). Windows implementation sketch: mute via `IAudioEndpointVolume::GetMute/SetMute` on the default render endpoint (`windows` crate, `Win32_Media_Audio_Endpoints`); "is something playing" via `IAudioMeterInformation::GetPeakValue` (> ~0.001 over a few reads) and pause/resume via `SendInput` with `VK_MEDIA_PLAY_PAUSE`. Keep the same contract: `quiet` before the mic opens, `restore` in the stop path and on Exit, persist the mute marker for `recover()`, only undo what we changed. Then return true from `is_available` on Windows.
- **Hold mode follows the physical key** (`hold_key.rs`): `is_down` returns false elsewhere, so Hold mode trusts the hot-key events as before. Windows: `GetAsyncKeyState(vk) & 0x8000 != 0` (user32), mapping `keyboard_types::Code` names to VK codes (`KeyQ` → 0x51, `Space` → 0x20, `F5` → 0x74 …) in a `win_key_code` next to `mac_key_code`.
- Mute recovery marker — part of media.

Not verified: no Windows toolchain on the Mac (MSVC C deps), so the Windows compile was reviewed, not run. Build it on Windows after merging.
