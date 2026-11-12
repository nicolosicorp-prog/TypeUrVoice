mod state;
mod storage;
mod permissions;
mod audio;
mod transcription;
mod processing;
mod paste;
mod tray;
mod http;
mod logging;
mod local_llm;
mod knock;
mod stream;
mod hold_key;
mod media;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use state::{AppState, RecordingState, Settings, SharedSettings, SharedState};
use storage::history;

// ─── String Helpers ─────────────────────────────────────────────────────────
// Byte-slicing Czech/Slovak text at arbitrary positions panics on multi-byte
// characters (č, ř, ž, etc.). Always truncate at a char boundary.

fn audio_level(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let sum_sq: f64 = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    let peak = samples.iter().fold(0f32, |a, b| a.max(b.abs()));
    (rms, peak)
}

fn safe_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

// ─── Safe Mutex Helpers ─────────────────────────────────────────────────────
// All mutex access goes through these — they recover from poisoning instead of
// panicking, so a crash in one thread can never cascade to kill the whole app.

fn with_state<R>(app: &AppHandle, f: impl FnOnce(&mut AppState) -> R) -> R {
    let r = app.state::<SharedState>();
    let mut s = r.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut s)
}

fn with_settings<R>(app: &AppHandle, f: impl FnOnce(&mut Settings) -> R) -> R {
    let r = app.state::<SharedSettings>();
    let mut s = r.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut s)
}

/// Request macOS accessibility permission with a user-facing prompt.
#[cfg(target_os = "macos")]
fn request_accessibility_if_needed() {
    use std::ffi::c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_cb: *const c_void,
            value_cb: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: *const c_void;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: *const c_void;
    }

    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        );
        let trusted = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict);
        if trusted {
            eprintln!("[TypeUrVoice] Accessibility: granted");
        } else {
            eprintln!("[TypeUrVoice] Accessibility: not granted — macOS prompt shown");
        }
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn get_settings(settings: tauri::State<'_, SharedSettings>) -> Result<Settings, String> {
    Ok(settings.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

#[tauri::command]
async fn update_settings(
    new_settings: Settings,
    settings: tauri::State<'_, SharedSettings>,
    app: AppHandle,
) -> Result<(), String> {
    let shortcuts_change = {
        let mut s = settings.lock().unwrap_or_else(|e| e.into_inner());
        let changed = s.shortcut != new_settings.shortcut
            || s.secondary_shortcut != new_settings.secondary_shortcut
            || s.primary_language != new_settings.primary_language
            || s.secondary_language != new_settings.secondary_language;
        let old = if changed {
            Some((s.shortcut.clone(), s.secondary_shortcut.clone()))
        } else {
            None
        };
        let model_changed = s.engine != new_settings.engine || s.local_model != new_settings.local_model;
        let llm_changed = s.text_processor != new_settings.text_processor
            || s.builtin_llm_model != new_settings.builtin_llm_model
            || s.builtin_llm_auto != new_settings.builtin_llm_auto;
        *s = new_settings.clone();
        (old, model_changed, llm_changed)
    };
    let (shortcuts_change, model_changed, llm_changed) = shortcuts_change;
    storage::settings::save(&new_settings).map_err(|e| e.to_string())?;
    if let Some((old_primary, old_secondary)) = shortcuts_change {
        re_register_shortcuts(&app, Some((old_primary, old_secondary)));
    }
    if model_changed {
        prewarm_local_model(&app);
    }
    if llm_changed {
        sync_builtin_llm(&app);
    }
    knock::sync(&app);
    // Keep tray menu checkmarks in sync with settings UI
    tray::refresh_menu(&app);
    Ok(())
}

/// Save the API key for a provider slot. Slots:
///   "openai", "anthropic", "gemini",
///   "compat:grok", "compat:groq", "compat:deepseek", "compat:mistral",
///   "compat:openrouter", "compat:perplexity", "compat:together", "compat:custom"
#[tauri::command]
async fn save_api_key(slot: String, key: String) -> Result<(), String> {
    storage::api_keys::set(&slot, &key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_api_key(slot: String) -> Result<Option<String>, String> {
    storage::api_keys::get(&slot).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_api_key(slot: String) -> Result<(), String> {
    storage::api_keys::delete(&slot).map_err(|e| e.to_string())
}

/// Test an API key against the provider's lightest available endpoint.
/// `slot` selects which provider's protocol to probe.
#[tauri::command]
async fn test_api_key(
    slot: String,
    key: String,
    http: tauri::State<'_, reqwest::Client>,
) -> Result<bool, String> {
    transcription::cloud::test_provider_key(&slot, &key, &http)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_permissions() -> Result<permissions::PermissionsState, String> {
    Ok(permissions::check_all_permissions())
}

#[tauri::command]
async fn open_permission_settings(permission: String) -> Result<(), String> {
    permissions::open_permission_settings(&permission);
    Ok(())
}

#[tauri::command]
async fn get_history(limit: u32, offset: u32) -> Result<Vec<storage::history::HistoryEntry>, String> {
    let conn = history::open().map_err(|e| e.to_string())?;
    history::get_recent(&conn, limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_history_entry(id: i64) -> Result<(), String> {
    let conn = history::open().map_err(|e| e.to_string())?;
    history::delete(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_history() -> Result<(), String> {
    let conn = history::open().map_err(|e| e.to_string())?;
    history::clear_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_local_models() -> Result<Vec<serde_json::Value>, String> {
    use transcription::local::{is_model_downloaded, model_size_bytes};
    let models: Vec<_> = transcription::models::MODELS.iter().map(|m| {
        let downloaded = is_model_downloaded(m.key);
        let size_mb = if downloaded { model_size_bytes(m.key) / 1_000_000 } else { 0 };
        serde_json::json!({
            "key": m.key,
            "name": m.name,
            "downloaded": downloaded,
            "sizeMb": size_mb,
        })
    }).collect();
    Ok(models)
}

#[cfg(feature = "local-whisper")]
#[tauri::command]
async fn delete_local_model(
    model_key: String,
    cache: tauri::State<'_, std::sync::Arc<transcription::whisper_cache::WhisperCache>>,
) -> Result<(), String> {
    transcription::local::delete_model(&model_key).map_err(|e| e.to_string())?;
    cache.invalidate();
    Ok(())
}

#[cfg(not(feature = "local-whisper"))]
#[tauri::command]
async fn delete_local_model(model_key: String) -> Result<(), String> {
    transcription::local::delete_model(&model_key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_text_processor(
    settings: tauri::State<'_, SharedSettings>,
    http: tauri::State<'_, reqwest::Client>,
    llm: tauri::State<'_, SharedLlm>,
    app: AppHandle,
) -> Result<(), String> {
    let mut s = settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if s.text_processor == state::TextProcessor::BuiltIn {
        s.text_processor_url = llm.ensure_running(&app, &s.builtin_llm_model)
            .await
            .map_err(|e| e.to_string())?;
    }
    // OpenAI key — used by Whisper transcription (Engine=Cloud) and by the
    // OpenAI text processor. Other providers' keys are read by build_gpt_config.
    let api_key = match storage::api_keys::get("openai") {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[TypeUrVoice] API key store error: {}", e);
            None
        }
    };
    processing::test_processor(&http, &s, api_key.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<UpdateStatus, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| format!("Updater init failed: {}", e))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateStatus {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateStatus { available: false, version: None, notes: None }),
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

#[derive(serde::Serialize)]
struct UpdateStatus {
    available: bool,
    version: Option<String>,
    notes: Option<String>,
}

/// Model downloads in flight, by "<kind>:<key>". A second request for the
/// same model is ignored: both would write the same `.partial` file, and
/// the slower one then fails with "No such file or directory" once the
/// first has renamed it into place. (Switching Settings tabs mid-download
/// re-enabled the Download button, which is how users hit this.)
static DOWNLOADS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Removes its key from `DOWNLOADS` when the download task ends, however it ends.
struct DownloadSlot(String);

impl DownloadSlot {
    fn claim(id: String) -> Option<Self> {
        let mut active = DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner());
        if active.contains(&id) {
            log_to_file(&format!("Download of {} already running — ignoring the second request", id));
            return None;
        }
        active.push(id.clone());
        Some(Self(id))
    }
}

impl Drop for DownloadSlot {
    fn drop(&mut self) {
        DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner()).retain(|k| k != &self.0);
    }
}

#[tauri::command]
async fn download_local_model(model_key: String, app: AppHandle) -> Result<(), String> {
    let Some(slot) = DownloadSlot::claim(format!("whisper:{}", model_key)) else { return Ok(()) };
    let key_for_progress = model_key.clone();
    let key_for_result   = model_key.clone();
    let app_for_progress = app.clone();
    let app_for_done     = app.clone();
    let app_for_error    = app.clone();

    tauri::async_runtime::spawn(async move {
        let _slot = slot;
        let result = transcription::local::download_model(
            &key_for_result,
            move |progress| {
                let _ = app_for_progress.emit("model-download-progress", serde_json::json!({
                    "model": key_for_progress,
                    "progress": progress,
                }));
            },
        ).await;

        match result {
            Ok(_) => {
                let _ = app_for_done.emit("model-download-done", &key_for_result);
                prewarm_local_model(&app_for_done);
            }
            Err(e) => {
                let _ = app_for_error.emit("model-download-error", serde_json::json!({
                    "model": key_for_result,
                    "error": e.to_string(),
                }));
            }
        }
    });
    Ok(())
}

// ─── Built-in LLM (bundled llama-server) ─────────────────────────────────────

pub(crate) type SharedLlm = Arc<local_llm::server::LlmServer>;

#[tauri::command]
async fn list_builtin_llm_models() -> Result<Vec<local_llm::models::ModelRow>, String> {
    Ok(local_llm::models::rows())
}

#[tauri::command]
async fn download_builtin_llm_model(model_key: String, app: AppHandle) -> Result<(), String> {
    let info = local_llm::models::lookup(&model_key)
        .ok_or_else(|| format!("Unknown built-in model: {}", model_key))?;
    let dest = local_llm::models::path(&model_key).map_err(|e| e.to_string())?;
    let Some(slot) = DownloadSlot::claim(format!("llm:{}", model_key)) else { return Ok(()) };
    tauri::async_runtime::spawn(async move {
        let _slot = slot;
        let app_for_progress = app.clone();
        let key_for_progress = model_key.clone();
        let result = storage::download::download_verified(info.url, &dest, Some(info.hash), move |progress| {
            let _ = app_for_progress.emit("llm-download-progress", serde_json::json!({
                "model": key_for_progress,
                "progress": progress,
            }));
        }).await;
        match result {
            Ok(()) => {
                log_to_file(&format!("Built-in model downloaded: {}", model_key));
                let _ = app.emit("llm-download-done", &model_key);
                sync_builtin_llm(&app);
            }
            Err(e) => {
                log_to_file(&format!("Built-in model download failed ({}): {}", model_key, e));
                let _ = app.emit("llm-download-error", serde_json::json!({
                    "model": model_key,
                    "error": e.to_string(),
                }));
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn delete_builtin_llm_model(
    model_key: String,
    app: AppHandle,
    llm: tauri::State<'_, SharedLlm>,
) -> Result<(), String> {
    // Never delete a file out from under a running server.
    llm.stop(&app).await;
    let path = local_llm::models::path(&model_key).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_builtin_llm_status(
    llm: tauri::State<'_, SharedLlm>,
) -> Result<local_llm::server::Status, String> {
    Ok(llm.status())
}

#[tauri::command]
async fn unload_builtin_llm(app: AppHandle, llm: tauri::State<'_, SharedLlm>) -> Result<(), String> {
    llm.stop(&app).await;
    Ok(())
}

/// Whether a recording with these settings will call the text processor.
pub(crate) fn needs_text_processing(s: &Settings) -> bool {
    s.output_mode != state::OutputMode::Raw || s.translation_enabled || s.summary_enabled
}

/// Start the built-in server in the background (used for hotkey warm-up and
/// for "keep loaded" mode). Errors surface via the status event + crash.log.
fn warm_builtin_llm(app: &AppHandle) {
    let model = with_settings(app, |s| s.builtin_llm_model.clone());
    let llm = app.state::<SharedLlm>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = llm.ensure_running(&app, &model).await;
    });
}

/// Reconcile the server with settings: keep-loaded mode loads it now; a
/// different processor unloads it to free memory. Auto mode loads lazily
/// (on hotkey press), so nothing to do there.
fn sync_builtin_llm(app: &AppHandle) {
    let (active, auto, downloaded) = with_settings(app, |s| (
        s.text_processor == state::TextProcessor::BuiltIn,
        s.builtin_llm_auto,
        local_llm::models::is_downloaded(&s.builtin_llm_model),
    ));
    let llm = app.state::<SharedLlm>().inner().clone();
    if !active {
        if llm.is_running() {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { llm.stop(&app).await });
        }
    } else if !auto && downloaded {
        warm_builtin_llm(app);
    }
}

// ─── Recording Pipeline ───────────────────────────────────────────────────────

#[tauri::command]
async fn list_input_devices() -> Result<Vec<String>, String> {
    Ok(audio::capture::list_input_devices())
}

/// True when this machine has the accelerometer knock-to-record needs.
#[tauri::command]
async fn knock_available() -> bool {
    knock::is_available()
}

/// True where muting / pausing media while recording is implemented (macOS).
#[tauri::command]
async fn media_control_available() -> bool {
    media::is_available()
}

/// Return provider metadata (display name, default model, recommended models,
/// requires_key, key_slot, stable_catalog) for the *currently selected*
/// provider. UI uses this to populate the model dropdown + show key inputs.
#[tauri::command]
async fn get_active_provider_info(
    settings: tauri::State<'_, SharedSettings>,
) -> Result<serde_json::Value, String> {
    let s = settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let info = processing::providers::catalog::for_settings(&s);
    Ok(serde_json::json!({
        "displayName":   info.display_name,
        "endpoint":      info.endpoint,
        "defaultModel":  info.default_model,
        "recommended":   info.recommended,
        "keySlot":       info.key_slot,
        "requiresKey":   info.requires_key,
        "stableCatalog": info.stable_catalog,
    }))
}

/// Fetch the live model list for a provider slot. Cached on disk for 24h.
/// Returns an empty Vec if the provider has no usable /models endpoint
/// (Perplexity) or the API key is missing.
#[tauri::command]
async fn fetch_provider_models(
    slot: String,
    http: tauri::State<'_, reqwest::Client>,
) -> Result<Vec<String>, String> {
    processing::providers::dynamic_models::fetch_models(&slot, &http)
        .await
        .map_err(|e| e.to_string())
}

/// List every occupation profile available — built-ins + user clones,
/// merged and sorted (locale → occupation → name). Used by Output tab to
/// render the Professional Profiles section.
#[tauri::command]
fn list_occupation_profiles(
    settings: tauri::State<'_, state::SharedSettings>,
) -> Result<Vec<state::OccupationProfile>, String> {
    let s = settings.lock().map_err(|e| e.to_string())?;
    Ok(processing::occupation_profiles::all_for_settings(&s))
}

/// Activate an occupation profile by id. `None` clears the selection
/// (reverts to the user's OutputMode). Returns the resolved profile
/// (or None) so the UI can confirm what's now active.
#[tauri::command]
fn set_active_occupation_profile(
    profile_id: Option<String>,
    settings: tauri::State<'_, state::SharedSettings>,
    app: tauri::AppHandle,
) -> Result<Option<state::OccupationProfile>, String> {
    let mut s = settings.lock().map_err(|e| e.to_string())?;
    s.active_occupation_profile_id = profile_id;
    let resolved = processing::occupation_profiles::active(&s);
    storage::settings::save(&s).map_err(|e| e.to_string())?;
    // Notify the rest of the app (tray, useSettings hook) that something changed.
    use tauri::Emitter;
    let _ = app.emit("settings-changed", ());
    Ok(resolved)
}

/// Save (create or update) a user-owned occupation profile. The frontend
/// generates the id via `crypto.randomUUID()` for new profiles; passing an
/// existing id updates in place.
#[tauri::command]
fn save_user_occupation_profile(
    profile: state::OccupationProfile,
    settings: tauri::State<'_, state::SharedSettings>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut s = settings.lock().map_err(|e| e.to_string())?;
    // Force builtin=false on user clones — builtins are immutable and live in JSON.
    let mut profile = profile;
    profile.builtin = false;
    if let Some(existing) = s.user_occupation_profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile;
    } else {
        s.user_occupation_profiles.push(profile);
    }
    storage::settings::save(&s).map_err(|e| e.to_string())?;
    use tauri::Emitter;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

/// Delete a user-owned occupation profile. Built-ins can't be deleted —
/// trying to delete one is a no-op. If the deleted profile was active,
/// `active_occupation_profile_id` is cleared.
#[tauri::command]
fn delete_user_occupation_profile(
    profile_id: String,
    settings: tauri::State<'_, state::SharedSettings>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut s = settings.lock().map_err(|e| e.to_string())?;
    s.user_occupation_profiles.retain(|p| p.id != profile_id);
    if s.active_occupation_profile_id.as_deref() == Some(&profile_id) {
        s.active_occupation_profile_id = None;
    }
    storage::settings::save(&s).map_err(|e| e.to_string())?;
    use tauri::Emitter;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

#[tauri::command]
async fn show_crash_log() -> Result<(), String> {
    let path = storage::settings::typeurvoice_data_dir()
        .map_err(|e| format!("Could not resolve data dir: {}", e))?
        .join("crash.log");

    // Reveal in Finder on macOS, Explorer on Windows. The log file may not
    // exist yet if nothing has gone wrong — in that case, open the parent dir.
    let target = if path.exists() { path } else {
        path.parent().map(|p| p.to_path_buf()).unwrap_or(path)
    };

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&target)
            .status()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&target)
            .status()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }
    Ok(())
}

/// Show the recording bubble once the microphone actually delivers audio —
/// the bubble is the user's cue to speak, so it must not appear before the
/// first word can be heard. Called by whichever comes first: the first
/// audio, or a 1 s fallback (a slow Bluetooth mic, or no mic at all).
fn announce_recording(app: &AppHandle, started_at: std::time::Instant, shown: &AtomicBool, reason: &str) {
    if shown.swap(true, Ordering::AcqRel) {
        return;
    }
    log_to_file(&format!("Timing: mic {} after {}ms", reason, started_at.elapsed().as_millis()));
    let still_recording = with_state(app, |s| {
        s.recording_state == RecordingState::Recording && s.recording_started_at == Some(started_at)
    });
    if still_recording {
        let _ = app.emit("recording-state", "recording");
    }
    show_overlay(app);
}

/// Returns when the recording started, or `None` if one was already running.
fn start_audio_capture(app: &AppHandle, language: Option<String>) -> Option<std::time::Instant> {
    // Single atomic check-and-transition under the state mutex. Guards against
    // concurrent shortcut presses (primary + secondary fired nearly together)
    // that could otherwise both see Idle and double-start. Also allocates fresh
    // per-session Arcs so a still-draining prior thread cannot collide with
    // the new capture buffer.
    let acquired = with_state(app, |s| match s.recording_state {
        RecordingState::Recording | RecordingState::Processing => {
            eprintln!("[TypeUrVoice] Ignoring start — already {:?}", s.recording_state);
            None
        }
        _ => {
            let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
            let stop_flag = Arc::new(AtomicBool::new(false));
            let sample_rate = Arc::new(Mutex::new(16_000u32));
            s.audio_buffer = Arc::clone(&samples);
            s.stop_flag = Arc::clone(&stop_flag);
            s.sample_rate = Arc::clone(&sample_rate);
            s.recording_state = RecordingState::Recording;
            let started_at = std::time::Instant::now();
            s.recording_started_at = Some(started_at);
            s.active_language = language.clone();
            let capture_done = Arc::new(AtomicBool::new(false));
            s.capture_done = Arc::clone(&capture_done);
            let preview_busy = Arc::new(AtomicBool::new(false));
            s.preview_busy = Arc::clone(&preview_busy);
            s.stream = None;
            Some((samples, stop_flag, sample_rate, capture_done, preview_busy, started_at))
        }
    });
    let Some((samples, stop_flag, sample_rate_arc, capture_done, preview_busy, started_at)) = acquired else { return None; };

    // Microphone first — everything else can happen while it starts. On
    // its own thread: cpal::Stream is !Send on macOS.
    let input_device = with_settings(app, |s| s.input_device.clone());
    let shown = Arc::new(AtomicBool::new(false));
    {
        let (samples, sample_rate_arc, stop_flag, capture_done) =
            (Arc::clone(&samples), Arc::clone(&sample_rate_arc), Arc::clone(&stop_flag), Arc::clone(&capture_done));
        let (live_app, live_shown) = (app.clone(), Arc::clone(&shown));
        let on_live: Box<dyn FnOnce() + Send> =
            Box::new(move || announce_recording(&live_app, started_at, &live_shown, "live"));
        std::thread::spawn(move || {
            if let Err(e) = audio::capture::record_until_stopped(samples, sample_rate_arc, stop_flag, input_device, on_live) {
                log_to_file(&format!("Recording error: {}", e));
            }
            capture_done.store(true, Ordering::Release);
        });
        let (fallback_app, fallback_shown) = (app.clone(), Arc::clone(&shown));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            announce_recording(&fallback_app, started_at, &fallback_shown, "not delivering audio");
        });
    }

    // A profile-learning pass would hold the (single-slot) LLM while this
    // recording needs it — stop it; it runs again after the next recording.
    abort_learning();

    // Mute / pause the Mac's sound before the mic opens (fast — CoreAudio
    // property calls and at most one key event). Undone in the stop path.
    let media_mode = with_settings(app, |s| s.media_while_recording);
    media::quiet(&media_mode);

    eprintln!("[TypeUrVoice] Recording started (language: {:?})", language);

    let settings_now = with_settings(app, |s| s.clone());
    let streaming = stream_typing_active(&settings_now);
    let live = ((settings_now.live_preview || streaming) && settings_now.engine == state::Engine::Local).then(|| {
        let lang = language.clone().unwrap_or_else(|| settings_now.source_language.clone());
        (settings_now.local_model.key().to_string(), lang, settings_now.remove_fillers)
    });
    let stream_session = streaming.then(|| {
        let st = stream::StreamSession::new();
        st.spawn_worker(app.clone(), settings_now.clone());
        with_state(app, |s| s.stream = Some(Arc::clone(&st)));
        st
    });

    // Built-in LLM: start loading while the user speaks so the model is ready
    // by the time transcription finishes (no-op if already running).
    let warm_llm = with_settings(app, |s| {
        s.text_processor == state::TextProcessor::BuiltIn && needs_text_processing(s)
    });
    if warm_llm {
        warm_builtin_llm(app);
    }

    if let Some((model_key, language, remove_fillers)) = live {
        #[cfg(feature = "local-whisper")]
        let cache = app.state::<Arc<transcription::whisper_cache::WhisperCache>>().inner().clone();
        #[cfg(not(feature = "local-whisper"))]
        let cache = Arc::new(());
        transcription::live::spawn(app.clone(), cache, transcription::live::Session {
            samples: Arc::clone(&samples),
            sample_rate: Arc::clone(&sample_rate_arc),
            stop_flag: Arc::clone(&stop_flag),
            busy: preview_busy,
            model_key,
            language,
            remove_fillers,
            vocabulary: transcription::vocabulary_prompt(&settings_now),
            show_preview: settings_now.live_preview,
            stream: stream_session,
        });
    }

    Some(started_at)
}

/// `trim_tail`: drop this much audio from the end of the recording before
/// transcription — used when a knock stopped it, so the thumps aren't sent
/// to Whisper.
fn stop_audio_capture_and_process(app: &AppHandle, trim_tail: Option<std::time::Duration>) {
    // Atomic transition — if we're not currently recording, do nothing.
    // Guards against double-stop races (e.g. toggle-mode quick repress).
    let captured = with_state(app, |s| match s.recording_state {
        RecordingState::Recording => {
            s.stop_flag.store(true, Ordering::Relaxed);
            s.recording_state = RecordingState::Processing;
            Some((
                s.active_language.clone(),
                Arc::clone(&s.audio_buffer),
                Arc::clone(&s.sample_rate),
                Arc::clone(&s.capture_done),
                s.recording_started_at,
                Arc::clone(&s.preview_busy),
                s.stream.take(),
            ))
        }
        _ => None,
    });
    let Some((active_language, samples_arc, sample_rate_arc, capture_done, started_at, preview_busy, stream_session)) = captured else { return; };
    // Sound back on / playback resumed as soon as the user stops speaking.
    media::restore();
    let stopped_at = std::time::Instant::now();
    if let Some(t) = started_at {
        log_to_file(&format!("Timing: recorded {}ms", stopped_at.duration_since(t).as_millis()));
    }

    let _ = app.emit("recording-state", "processing");

    let mut settings = with_settings(app, |s| s.clone());
    // Override source_language if a shortcut-specific language was set
    if let Some(lang) = active_language {
        eprintln!("[TypeUrVoice] Using shortcut language override: {}", lang);
        settings.source_language = lang;
    }
    // Whisper key — slot depends on which cloud Whisper provider is active.
    // OpenAI Whisper uses "openai"; Groq Whisper uses "compat:groq" (the Groq
    // key is shared between Groq Whisper and Groq text processing).
    // Text-processor keys for other providers are read by build_gpt_config.
    let whisper_key_slot = transcription::cloud::key_slot_for(&settings.cloud_transcription_provider);
    let api_key = match storage::api_keys::get(whisper_key_slot) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[TypeUrVoice] API key store error: {}", e);
            None
        }
    };
    let http_client: reqwest::Client = app
        .state::<reqwest::Client>()
        .inner()
        .clone();
    #[cfg(feature = "local-whisper")]
    let whisper_cache: std::sync::Arc<transcription::whisper_cache::WhisperCache> =
        app.state::<std::sync::Arc<transcription::whisper_cache::WhisperCache>>().inner().clone();
    #[cfg(not(feature = "local-whisper"))]
    let whisper_cache: std::sync::Arc<()> = std::sync::Arc::new(());
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        // Wait for the cpal thread to drop its stream (usually ~10 ms — it
        // polls the stop flag every 10 ms). Capped at the old fixed 200 ms.
        let wait_started = std::time::Instant::now();
        while !capture_done.load(Ordering::Acquire)
            && wait_started.elapsed() < std::time::Duration::from_millis(200)
        {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
        // Let an in-flight live-preview inference finish so it doesn't
        // compete with the final pass for the GPU (bounded: a preview run is
        // ≤ 30 s of audio, well under a second on Metal).
        let wait_started = std::time::Instant::now();
        while preview_busy.load(Ordering::Acquire)
            && wait_started.elapsed() < std::time::Duration::from_millis(1500)
        {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Extract samples safely
        let mut samples = samples_arc.lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let sample_rate = *sample_rate_arc.lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(tail) = trim_tail {
            let cut = ((tail.as_secs_f64() * sample_rate as f64) as usize).min(samples.len());
            samples.truncate(samples.len() - cut);
            log_to_file(&format!("Knock: trimmed last {}ms of audio", tail.as_millis()));
        }

        // Typing while speaking: let the worker type what it has, then only
        // the audio after the last typed sentence goes through the pipeline.
        let prior = match stream_session {
            Some(st) => {
                let output = st.finish().await;
                let committed = st.committed.load(Ordering::Acquire);
                if output.typed > 0 {
                    samples.drain(..committed.min(samples.len()));
                    log_to_file(&format!(
                        "Stream: {} chunk(s) typed while speaking, {:.1}s of audio left",
                        output.typed, samples.len() as f64 / sample_rate as f64
                    ));
                    Some(output)
                } else {
                    // Nothing reached the app (no sentence finished, or typing
                    // was blocked): process the whole recording as usual.
                    None
                }
            }
            None => None,
        };

        eprintln!("[TypeUrVoice] Pipeline: {} samples, {}Hz", samples.len(), sample_rate);

        // Outer pipeline timeout — sized to accommodate transcription + up to three
        // LLM calls (cleanup, translation, summary) each bounded to 45s below.
        let pipeline_future = run_pipeline(samples, sample_rate, settings, api_key, http_client, whisper_cache, &app_clone, prior);
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(360),
            pipeline_future,
        ).await;

        match result {
            Ok(Ok(_)) => {
                log_to_file("Pipeline completed successfully");
                eprintln!("[TypeUrVoice] Pipeline completed successfully");
                maybe_learn(&app_clone);
            }
            Ok(Err(e)) => {
                let msg = format!("Pipeline error: {}", e);
                log_to_file(&msg);
                eprintln!("[TypeUrVoice] {}", msg);
                let _ = app_clone.emit("recording-error", e.to_string());
                let _ = app_clone.emit("recording-state", "error");
                show_error_notification(&app_clone, &e.to_string());
                with_state(&app_clone, |s| {
                    s.recording_state = RecordingState::Error(e.to_string());
                });
            }
            Err(_) => {
                log_to_file("Pipeline timed out after 360s!");
                eprintln!("[TypeUrVoice] Pipeline timed out after 360s!");
                let msg = "Pipeline timed out. Try a shorter recording or smaller model.";
                let _ = app_clone.emit("recording-error", msg);
                let _ = app_clone.emit("recording-state", "error");
                show_error_notification(&app_clone, msg);
                with_state(&app_clone, |s| {
                    s.recording_state = RecordingState::Error(msg.to_string());
                });
            }
        }

        // Hide overlay (in case the pipeline errored before reaching the hide step)
        if let Some(overlay) = app_clone.get_webview_window("overlay") {
            let _ = overlay.hide();
        }

        // Reset to idle after a short delay so user can record again
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        with_state(&app_clone, |s| {
            if matches!(s.recording_state, RecordingState::Done | RecordingState::Error(_)) {
                s.recording_state = RecordingState::Idle;
                eprintln!("[TypeUrVoice] State reset to Idle — ready for next recording");
            }
        });
    });
}

async fn run_pipeline(
    samples: Vec<f32>,
    sample_rate: u32,
    mut settings: Settings,
    api_key: Option<String>,
    http: reqwest::Client,
    #[cfg(feature = "local-whisper")] cache: std::sync::Arc<transcription::whisper_cache::WhisperCache>,
    #[cfg(not(feature = "local-whisper"))] cache: std::sync::Arc<()>,
    app: &AppHandle,
    // Sentences already typed while speaking; `samples` is then only the
    // audio after them.
    prior: Option<stream::Output>,
) -> anyhow::Result<()> {
    if samples.is_empty() && prior.is_none() {
        return Err(anyhow::anyhow!("No audio recorded. Hold the shortcut while speaking."));
    }
    // Same language handling as the typed sentences (decided while speaking).
    if let Some(plan) = prior.as_ref().and_then(|p| p.plan.as_ref()) {
        stream::apply_plan(&mut settings, plan, "");
    }

    let duration_secs = samples.len() as f64 / sample_rate as f64;

    // Compute RMS + peak to detect a dead mic (Microphone permission not granted
    // → cpal returns silence → Whisper returns empty → confusing "no speech" error).
    let (rms, peak) = audio_level(&samples);
    log_to_file(&format!(
        "Audio captured: {:.1}s, {} samples @ {}Hz, rms={:.4}, peak={:.4}",
        duration_secs, samples.len(), sample_rate, rms, peak
    ));
    eprintln!("[TypeUrVoice] Audio: {:.1}s rms={:.4} peak={:.4}", duration_secs, rms, peak);

    // Trim leading/trailing silence (after the level check above, so the
    // dead-mic diagnostics still see the full capture).
    let trimmed = audio::buffer::trim_silence(&samples, sample_rate);
    if trimmed.len() != samples.len() {
        log_to_file(&format!(
            "Silence trim: {:.1}s → {:.1}s",
            duration_secs, trimmed.len() as f64 / sample_rate as f64
        ));
    }
    // A moment of silence in front: Whisper tends to drop a first word that
    // starts right at the first sample.
    let lead = sample_rate as usize / 4;
    let mut samples: Vec<f32> = Vec::with_capacity(lead + trimmed.len());
    samples.resize(lead, 0.0);
    samples.extend_from_slice(trimmed);
    let wav_bytes = audio::buffer::to_wav(&samples, sample_rate)?;
    let t_transcribe = std::time::Instant::now();

    // ── Step 1: Transcribe ──────────────────────────────────────────────────
    let _ = app.emit("pipeline-step", "transcribing");
    log_to_file(&format!("Step 1: Transcribing ({:?} engine, {:.1}s audio)", settings.engine, duration_secs));
    eprintln!("[TypeUrVoice] Step 1: Transcribing ({:?} engine)...", settings.engine);

    // After typing while speaking, a remainder under 0.3 s is just the
    // release of the key — nothing to transcribe.
    let transcript = if prior.is_some() && (samples.len() as f64) < 0.3 * sample_rate as f64 {
        transcription::TranscriptionResult {
            text: String::new(),
            language: settings.source_language.clone(),
            requested_language: None,
        }
    } else {
        transcription::transcribe(
            wav_bytes,
            &samples,
            sample_rate,
            &settings,
            api_key.clone(),
            &http,
            cache,
        ).await?
    };

    let transcribe_ms = t_transcribe.elapsed().as_millis();
    if settings.diagnostics_verbose {
        log_to_file(&format!(
            "Step 1 result: lang={}, {} chars, preview={:?}",
            transcript.language, transcript.text.len(), safe_truncate(&transcript.text, 80)
        ));
    } else {
        log_to_file(&format!(
            "Step 1 result: lang={}, {} chars",
            transcript.language, transcript.text.len()
        ));
    }
    eprintln!("[TypeUrVoice] Transcription: lang={}, text={:?}",
        transcript.language,
        safe_truncate(&transcript.text, 60),
    );


    if transcript.text.trim().is_empty() && prior.is_none() {
        // Distinguish "mic is dead" from "mic is fine but Whisper gave nothing".
        // rms < 0.001 is effectively pure silence — almost always a permission
        // issue or wrong input device.
        let msg = if peak < 0.001 {
            "Microphone captured silence (rms ≈ 0). Grant Microphone permission in System Settings → Privacy & Security → Microphone → TypeUrVoice, or check your input device."
        } else {
            "No clear speech captured. Your mic may be too quiet or muted — try speaking closer to it, or check Source Language in Settings."
        };
        return Err(anyhow::anyhow!(msg));
    }

    // Spoke one language on another language's shortcut (e.g. Czech on the
    // English one): the transcript is in the spoken language, so clean it up
    // in that language and translate into the requested one. The shortcut's
    // language is the output language — it overrides the Settings target.
    if let Some(requested) = &transcript.requested_language {
        settings.source_language = transcript.language.clone();
        settings.translation_enabled = true;
        settings.target_language = requested.clone();
        log_to_file(&format!(
            "Language switch: spoke {} with {} requested — translating to {}",
            transcript.language, requested, settings.target_language
        ));
    }

    // ── Step 2: Process text ────────────────────────────────────────────────
    let _ = app.emit("pipeline-step", "processing");
    let t_process = std::time::Instant::now();
    let needs_processing = needs_text_processing(&settings);

    let mut speaker = processing::SpeakerContext::for_settings(&settings);
    if let Some(p) = &prior {
        speaker.earlier_source = p.processed.join(" ");
        speaker.earlier_output = p.outputs.join(" ");
    }
    let processed = if transcript.text.trim().is_empty() {
        processing::ProcessingResult {
            processed_text: String::new(),
            translated_text: None,
            summary: None,
            final_text: String::new(),
        }
    } else if !needs_processing {
        eprintln!("[TypeUrVoice] Step 2: Raw mode — skipping processing");
        processing::ProcessingResult {
            processed_text: transcript.text.clone(),
            translated_text: None,
            summary: None,
            final_text: transcript.text.clone(),
        }
    } else {
        // Only OpenAI text processor requires an API key
        if settings.text_processor == state::TextProcessor::OpenAi && api_key.is_none() {
            return Err(anyhow::anyhow!(
                "No API key configured. Add your OpenAI key in Settings → Engine."
            ));
        }
        eprintln!("[TypeUrVoice] Step 2: Processing ({:?} mode, {:?} processor)...",
            settings.output_mode, settings.text_processor);

        // Built-in LLM: usually already warm (started on hotkey press). The
        // per-recording settings clone carries the server URL to build_gpt_config.
        let builtin_llm = (settings.text_processor == state::TextProcessor::BuiltIn)
            .then(|| app.state::<SharedLlm>().inner().clone());
        if let Some(llm) = &builtin_llm {
            let base = llm.ensure_running(app, &settings.builtin_llm_model).await
                .map_err(|e| anyhow::anyhow!("Built-in model unavailable: {}", e))?;
            settings.text_processor_url = base;
        }

        // Resolve the language for LLM cleanup. Priority:
        //   1. Shortcut / Settings override (settings.source_language was already
        //      overridden above if a shortcut-specific language was set).
        //   2. Whisper's auto-detected language, as a last resort.
        // Whisper sometimes mislabels short Czech clips as "en"; passing that
        // wrong hint to cleanup caused the LLM to refuse with
        // "I can only assist with English text". Trust the user's explicit
        // choice over Whisper's guess whenever one exists.
        let cleanup_language = if settings.source_language != "auto"
            && !settings.source_language.is_empty()
        {
            settings.source_language.clone()
        } else {
            transcript.language.clone()
        };
        eprintln!("[TypeUrVoice] Cleanup language: {} (whisper detected: {}, source setting: {})",
            cleanup_language, transcript.language, settings.source_language);

        let result = processing::process(
            &transcript.text,
            &cleanup_language,
            &settings,
            api_key.as_deref(),
            &http,
            &speaker,
        ).await?;
        if let Some(llm) = &builtin_llm {
            llm.touch(); // idle timer counts from the end of the last use
        }
        result
    };

    let process_ms = t_process.elapsed().as_millis();

    // With sentences typed while speaking: insert only what isn't in the
    // app yet, but keep the whole dictation for history and the clipboard.
    let (transcript, processed, insert_text) = match &prior {
        None => {
            let insert = processed.final_text.clone();
            (transcript, processed, insert)
        }
        Some(p) => {
            let rest = processed.final_text.trim().to_string();
            let pending = join_nonempty(&[p.untyped(), rest.clone()]);
            let insert = if pending.is_empty() { pending } else { format!(" {}", pending) };
            let any_translation = p.translated.iter().any(Option::is_some) || processed.translated_text.is_some();
            let translated = any_translation.then(|| {
                let mut parts: Vec<String> = p.translated.iter().zip(&p.outputs)
                    .map(|(t, o)| t.clone().unwrap_or_else(|| o.clone()))
                    .collect();
                parts.push(processed.translated_text.clone().unwrap_or_else(|| rest.clone()));
                join_nonempty(&parts)
            });
            let mut all_raw = p.raw.clone();
            all_raw.push(transcript.text.clone());
            let mut all_processed = p.processed.clone();
            all_processed.push(processed.processed_text.clone());
            let mut all_final = p.outputs.clone();
            all_final.push(rest);
            (
                transcription::TranscriptionResult { text: join_nonempty(&all_raw), ..transcript },
                processing::ProcessingResult {
                    processed_text: join_nonempty(&all_processed),
                    translated_text: translated,
                    summary: None,
                    final_text: join_nonempty(&all_final),
                },
                insert,
            )
        }
    };
    let streamed = prior.is_some();
    eprintln!("[TypeUrVoice] Final text: {:?}", safe_truncate(&processed.final_text, 60));

    // ── Step 3: Save to history ─────────────────────────────────────────────
    if let Err(e) = save_to_history(&transcript, &processed, &settings, duration_secs) {
        eprintln!("[TypeUrVoice] History save failed (non-fatal): {}", e);
    }

    // ── Step 3.5: Hide overlay BEFORE paste so focus returns to the user's app ──
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
    // Give macOS time to return focus to the previously active application
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // ── Step 4: Paste or copy ───────────────────────────────────────────────
    let _ = app.emit("pipeline-step", "pasting");
    let t_paste = std::time::Instant::now();
    let final_text = processed.final_text.clone();
    let auto_paste = settings.auto_paste;

    log_to_file(&format!("Step 4: {} text ({} chars)", if auto_paste { "Pasting" } else { "Copying" }, insert_text.len()));
    eprintln!("[TypeUrVoice] Step 4: {} text ({} chars)",
        if auto_paste { "Pasting" } else { "Copying" }, insert_text.len());

    // Paste on a blocking thread (enigo uses system APIs).
    // Wrap in catch_unwind so a panic in enigo doesn't kill the app.
    let paste_result = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if streamed {
                // Type the rest like the sentences before it (paste if typing is
                // blocked), then leave the whole dictation on the clipboard.
                if !insert_text.is_empty() && paste::type_text(&insert_text).is_err() {
                    paste::paste_text(insert_text.trim_start())?;
                }
                paste::copy_to_clipboard(&final_text)
            } else if auto_paste {
                paste::paste_text(&final_text)
            } else {
                paste::copy_to_clipboard(&final_text)
            }
        }))
    }).await;

    match paste_result {
        Ok(Ok(Ok(()))) => {
            log_to_file("Step 4: paste/copy succeeded");
            eprintln!("[TypeUrVoice] Paste/copy succeeded");
        }
        Ok(Ok(Err(e))) => {
            let msg = e.to_string();
            log_to_file(&msg);
            eprintln!("[TypeUrVoice] {}", msg);
            show_error_notification(app, &msg);
        }
        Ok(Err(_panic)) => {
            let msg = "Paste step panicked — text may not be on clipboard.";
            log_to_file(msg);
            eprintln!("[TypeUrVoice] {}", msg);
            show_error_notification(app, msg);
        }
        Err(e) => {
            let msg = format!("Paste task failed: {}", e);
            log_to_file(&msg);
            eprintln!("[TypeUrVoice] {}", msg);
            show_error_notification(app, &msg);
        }
    }

    log_to_file(&format!(
        "Timing: transcribe {}ms, process {}ms, paste {}ms",
        transcribe_ms, process_ms, t_paste.elapsed().as_millis()
    ));

    // ── Step 5: Update state and notify ─────────────────────────────────────
    with_state(app, |s| s.recording_state = RecordingState::Done);

    // With live text on, bring the bubble back for a moment with the final,
    // corrected text, so it replaces the rough preview the user just saw.
    // The overlay is non-focusable, so this can't pull focus off the paste.
    if settings.live_preview && !processed.final_text.is_empty() {
        let _ = app.emit("final-transcript", &processed.final_text);
        if let Some(overlay) = app.get_webview_window("overlay") {
            set_overlay_size(&overlay, OVERLAY_LIVE);
            let _ = overlay.set_focusable(false);
            let _ = overlay.show();
        }
    }

    let _ = app.emit("recording-state", "done");
    let _ = app.emit("transcription-result", &processed.final_text);

    if settings.show_notification {
        show_notification(app, &processed.final_text);
    }

    eprintln!("[TypeUrVoice] Pipeline complete");
    Ok(())
}

// ─── Speaker profile learning ───────────────────────────────────────────────

/// The background learning pass, so a new recording can stop it (it would
/// hold the single-slot built-in model the recording needs).
static LEARN_TASK: Mutex<Option<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(None);

fn abort_learning() {
    if let Some(h) = LEARN_TASK.lock().unwrap_or_else(|e| e.into_inner()).take() {
        h.abort();
    }
}

fn is_on_device_processor(s: &Settings) -> bool {
    matches!(
        s.text_processor,
        state::TextProcessor::BuiltIn | state::TextProcessor::Ollama | state::TextProcessor::LmStudio
    )
}

/// After a recording: update the speaker profile once enough new
/// dictations piled up. Automatic only with an on-device text processor —
/// with a cloud one it would send old dictations out without being asked
/// (the "Update now" button does it on request).
fn maybe_learn(app: &AppHandle) {
    let settings = with_settings(app, |s| s.clone());
    if !settings.learn_from_history || !is_on_device_processor(&settings) {
        return;
    }
    let prev = storage::speaker::load();
    let new = history::open()
        .and_then(|c| history::count_after(&c, prev.last_history_id))
        .unwrap_or(0);
    if new < processing::learn::LEARN_EVERY {
        return;
    }
    let mut slot = LEARN_TASK.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_some() {
        return; // one already scheduled/running
    }
    let app = app.clone();
    *slot = Some(tauri::async_runtime::spawn(async move {
        // Give the user's next dictation the model first.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let idle = with_state(&app, |s| matches!(s.recording_state, RecordingState::Idle));
        if idle {
            if let Err(e) = run_learning(&app).await {
                log_to_file(&format!("Speaker profile: update failed: {}", e));
            }
        }
        LEARN_TASK.lock().unwrap_or_else(|e| e.into_inner()).take();
    }));
}

async fn run_learning(app: &AppHandle) -> anyhow::Result<storage::speaker::SpeakerProfile> {
    let mut settings = with_settings(app, |s| s.clone());
    let llm = (settings.text_processor == state::TextProcessor::BuiltIn)
        .then(|| app.state::<SharedLlm>().inner().clone());
    if let Some(llm) = &llm {
        settings.text_processor_url = llm.ensure_running(app, &settings.builtin_llm_model).await
            .map_err(|e| anyhow::anyhow!("Built-in model unavailable: {}", e))?;
    }
    let prev = storage::speaker::load();
    let (entries, corrections) = {
        let conn = history::open()?;
        (history::entries_after(&conn, 0, 40)?, history::recent_corrections(&conn, 6)?)
    };
    let http = app.state::<reqwest::Client>().inner().clone();
    let started = std::time::Instant::now();
    let result = processing::learn::learn(&http, &settings, &prev, &entries, &corrections).await;
    if let Some(llm) = &llm {
        llm.touch();
    }
    let profile = match result {
        Ok(p) => p,
        Err(e) => {
            // Don't retry on every recording: wait for the next batch.
            let mut kept = prev.clone();
            kept.last_history_id = entries.iter().map(|e| e.id).max().unwrap_or(prev.last_history_id);
            let _ = storage::speaker::save(&kept);
            return Err(e);
        }
    };
    storage::speaker::save(&profile)?;
    log_to_file(&format!(
        "Speaker profile updated from {} dictation(s) in {}ms ({} term(s))",
        entries.len(), started.elapsed().as_millis(), profile.terms.len()
    ));
    let _ = app.emit("speaker-profile-updated", &profile);
    Ok(profile)
}

#[tauri::command]
async fn get_speaker_profile() -> storage::speaker::SpeakerProfile {
    storage::speaker::load()
}

/// The user edited the learned profile in Settings.
#[tauri::command]
async fn save_speaker_profile(profile: storage::speaker::SpeakerProfile) -> Result<(), String> {
    let on_disk = storage::speaker::load();
    let profile = storage::speaker::SpeakerProfile {
        terms: profile.terms.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        last_history_id: on_disk.last_history_id,
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        ..profile
    };
    storage::speaker::save(&profile).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_speaker_profile() -> Result<(), String> {
    abort_learning();
    storage::speaker::clear().map_err(|e| e.to_string())
}

#[tauri::command]
async fn learn_speaker_profile_now(app: AppHandle) -> Result<storage::speaker::SpeakerProfile, String> {
    abort_learning();
    run_learning(&app).await.map_err(|e| e.to_string())
}

/// The user fixed an output in History — store it so cleanup and
/// translation learn from it.
#[tauri::command]
async fn correct_history_entry(id: i64, text: String) -> Result<storage::history::HistoryEntry, String> {
    let conn = history::open().map_err(|e| e.to_string())?;
    let entry = history::save_correction(&conn, id, &text).map_err(|e| e.to_string())?;
    log_to_file("History: correction saved");
    Ok(entry)
}

fn join_nonempty(parts: &[String]) -> String {
    parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Typing while speaking is on and can work for this recording: local
/// engine (it needs the live transcription), auto-paste on, and no summary
/// (a summary needs the whole dictation first).
fn stream_typing_active(s: &Settings) -> bool {
    // Summary and Markdown both need the whole dictation at once.
    s.stream_typing
        && s.engine == state::Engine::Local
        && s.auto_paste
        && !s.summary_enabled
        && s.output_mode != state::OutputMode::Markdown
}

fn save_to_history(
    transcript: &transcription::TranscriptionResult,
    processed: &processing::ProcessingResult,
    settings: &Settings,
    duration_secs: f64,
) -> anyhow::Result<()> {
    let conn = history::open()?;
    history::insert(
        &conn,
        &transcript.text,
        &processed.processed_text,
        settings.output_mode.as_str(),
        Some(&transcript.language),
        processed.translated_text.as_deref(),
        processed.summary.as_deref(),
        Some(duration_secs),
    )?;
    Ok(())
}

fn show_notification(app: &AppHandle, text: &str) {
    use tauri_plugin_notification::NotificationExt;
    let preview = safe_truncate(text, 80);
    let _ = app.notification()
        .builder()
        .title("TypeUrVoice")
        .body(&preview)
        .show();
}

/// Always-on error notification — unlike success notifications, errors bypass
/// the show_notification setting. Settings windows are usually hidden, so the
/// Tauri event alone is invisible; the toast is the only reliable feedback.
fn show_error_notification(app: &AppHandle, msg: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification()
        .builder()
        .title("TypeUrVoice — recording failed")
        .body(&safe_truncate(msg, 140))
        .show();
}

/// Pill size while recording / processing, and the expanded size used once
/// live-preview text arrives.
const OVERLAY_PILL: (f64, f64) = (200.0, 44.0);
const OVERLAY_LIVE: (f64, f64) = (460.0, 96.0);

/// Resize the overlay (logical points) and keep it centred horizontally,
/// ~15% from the top of the primary monitor (above notifications).
fn set_overlay_size(overlay: &tauri::WebviewWindow, (ow, oh): (f64, f64)) {
    if let Ok(Some(monitor)) = overlay.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let screen_w = size.width as f64 / scale;
        let screen_h = size.height as f64 / scale;
        let x = ((screen_w - ow) / 2.0 * scale) as i32;
        let y = (screen_h * 0.15 * scale) as i32;
        let _ = overlay.set_size(tauri::PhysicalSize::new((ow * scale) as u32, (oh * scale) as u32));
        let _ = overlay.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn show_overlay(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        set_overlay_size(&overlay, OVERLAY_PILL);
        // Never key, never clickable: the overlay must not steal focus from
        // the app the text will be pasted into.
        let _ = overlay.set_focusable(false);
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.show();
    }
}

/// Grow the overlay to fit live-preview text. Idempotent — called on every
/// partial transcript; resizing to the same size is a no-op for the window.
pub(crate) fn show_live_overlay(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        set_overlay_size(&overlay, OVERLAY_LIVE);
    }
}

/// Load the selected local Whisper model in the background (no-op for the
/// cloud engine or when the model isn't downloaded yet).
fn prewarm_local_model(app: &AppHandle) {
    let (engine, model_key) = with_settings(app, |s| (s.engine.clone(), s.local_model.key().to_string()));
    if engine != state::Engine::Local {
        return;
    }
    #[cfg(feature = "local-whisper")]
    let cache = app.state::<std::sync::Arc<transcription::whisper_cache::WhisperCache>>().inner().clone();
    #[cfg(not(feature = "local-whisper"))]
    let cache = std::sync::Arc::new(());
    tauri::async_runtime::spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcription::local::prewarm(&cache, &model_key)
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => log_to_file(&format!("Prewarm of {} failed: {}", model_key, e)),
            Err(_) => log_to_file(&format!("Prewarm of {} panicked", model_key)),
        }
    });
}

// ─── Shortcut Management ────────────────────────────────────────────────────

fn register_cmd_q_intercept(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let app_handle = app.clone();
        if let Ok(sc) = "Super+Q".parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().on_shortcut(sc, move |_app, _sc, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Some(w) = app_handle.get_webview_window("settings") {
                        let _ = w.hide();
                    }
                    if let Some(w) = app_handle.get_webview_window("onboarding") {
                        let _ = w.hide();
                    }
                    eprintln!("[TypeUrVoice] Cmd+Q intercepted — windows hidden, app stays in tray");
                }
            });
        }
    }
}

/// Re-register the recording shortcuts after a settings change.
///
/// Only unregisters the specific shortcuts we previously bound (when `old` is
/// `Some`) — `unregister_all()` would also nuke the Cmd+Q intercept that
/// `setup()` registers once at app start.
fn re_register_shortcuts(app: &AppHandle, old: Option<(String, Option<String>)>) {
    if let Some((old_primary, old_secondary)) = old {
        if let Ok(sc) = old_primary.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().unregister(sc);
        }
        if let Some(old_sec) = old_secondary {
            if !old_sec.is_empty() {
                if let Ok(sc) = old_sec.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    let _ = app.global_shortcut().unregister(sc);
                }
            }
        }
    }

    let (primary, primary_lang, secondary, secondary_lang) = with_settings(app, |s| {
        (
            s.shortcut.clone(),
            s.primary_language.clone(),
            s.secondary_shortcut.clone(),
            s.secondary_language.clone(),
        )
    });
    register_shortcut(app, &primary, Some(primary_lang));
    if let Some(sec) = secondary {
        if !sec.is_empty() {
            register_shortcut(app, &sec, Some(secondary_lang));
        }
    }
}

fn register_shortcut(app: &AppHandle, shortcut: &str, language: Option<String>) {
    if let Ok(sc) = shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        let app_clone = app.clone();
        let lang = language.clone();
        let key_code = hold_key::key_code(shortcut);
        let _ = app.global_shortcut().on_shortcut(sc, move |_app, _sc, event| {
            on_shortcut(&app_clone, event.state(), lang.clone(), key_code);
        });
        eprintln!("[TypeUrVoice] Shortcut registered: {} (lang: {:?})", shortcut, language);
    } else {
        eprintln!("[TypeUrVoice] Failed to parse shortcut: {}", shortcut);
    }
}

/// `key_code`: the shortcut's main key (see `hold_key`), used in Hold mode to
/// check hot-key events against the key's physical state.
fn on_shortcut(app: &AppHandle, shortcut_state: ShortcutState, language: Option<String>, key_code: Option<u16>) {
    let recording_mode = with_settings(app, |s| s.recording_mode.clone());
    let current_state = with_state(app, |s| s.recording_state.clone());

    match recording_mode {
        state::RecordingMode::Hold => match shortcut_state {
            ShortcutState::Pressed => {
                if let Some(started_at) = start_audio_capture(app, language) {
                    if let Some(code) = key_code {
                        spawn_hold_watcher(app.clone(), code, started_at);
                    }
                }
            }
            ShortcutState::Released => {
                // A release event while the key is still physically held is
                // false — it used to cut long dictations off mid-sentence.
                // The hold watcher stops the recording at the real release.
                if current_state == RecordingState::Recording
                    && key_code.is_some_and(hold_key::is_down)
                {
                    log_to_file("Hold: release event while the key is still held — ignored");
                    return;
                }
                if current_state == RecordingState::Recording {
                    // Enforce minimum 600ms recording so quick key releases
                    // don't produce empty/useless clips.
                    let started = with_state(app, |s| s.recording_started_at);
                    let elapsed = started
                        .map(|t| t.elapsed())
                        .unwrap_or(std::time::Duration::from_secs(10));

                    if elapsed >= std::time::Duration::from_millis(600) {
                        stop_audio_capture_and_process(app, None);
                    } else {
                        // Wait until 600ms have passed, then stop
                        let remaining = std::time::Duration::from_millis(600) - elapsed;
                        let app_clone = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(remaining);
                            let still_recording = with_state(&app_clone, |s| {
                                s.recording_state == RecordingState::Recording
                            });
                            if still_recording {
                                stop_audio_capture_and_process(&app_clone, None);
                            }
                        });
                    }
                }
            }
        },
        state::RecordingMode::Toggle => {
            if shortcut_state == ShortcutState::Pressed {
                match current_state {
                    RecordingState::Recording => stop_audio_capture_and_process(app, None),
                    RecordingState::Idle | RecordingState::Done | RecordingState::Error(_) => {
                        start_audio_capture(app, language);
                    }
                    RecordingState::Processing => {} // ignore while processing
                }
            }
        }
    }
}

/// A completed knock pattern (see `knock`): acts like a Toggle-mode press of
/// a shortcut — which language depends on `Settings::knock_language` (and,
/// in Split mode, on `knocks`). `since_first_knock` is how long ago the
/// pattern began, so a stopping knock can be cut out of the recording.
fn on_knock(app: &AppHandle, since_first_knock: std::time::Duration, knocks: usize) {
    // Margin covers the thump starting slightly before the detected peak
    // plus the few ms cpal keeps capturing after the stop flag is set.
    const TRIM_MARGIN: std::time::Duration = std::time::Duration::from_millis(150);
    match with_state(app, |s| s.recording_state.clone()) {
        RecordingState::Recording => {
            log_to_file("Knock: stopping recording");
            stop_audio_capture_and_process(app, Some(since_first_knock + TRIM_MARGIN));
        }
        RecordingState::Processing => log_to_file("Knock: ignored while processing"),
        RecordingState::Idle | RecordingState::Done | RecordingState::Error(_) => {
            let language = with_settings(app, |s| match s.knock_language {
                state::KnockLanguage::Primary => s.primary_language.clone(),
                state::KnockLanguage::Secondary => s.secondary_language.clone(),
                state::KnockLanguage::Split if knocks >= 3 => s.secondary_language.clone(),
                state::KnockLanguage::Split => s.primary_language.clone(),
                state::KnockLanguage::Auto => "auto".to_string(),
            });
            log_to_file(&format!("Knock: starting recording ({} knocks, language {})", knocks, language));
            start_audio_capture(app, Some(language));
        }
    }
}

/// Hold mode: stop the recording when the shortcut key is physically up
/// but its release event never arrived (secure input, a dropped Carbon
/// event). Only watches when the key reads as down right after the press —
/// otherwise the key state can't be trusted (or it was a quick tap that the
/// release event handles) and the events alone decide, as before.
fn spawn_hold_watcher(app: AppHandle, code: u16, started_at: std::time::Instant) {
    use std::time::Duration;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        if !hold_key::is_down(code) {
            return;
        }
        let mut up_since: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let same_recording = with_state(&app, |s| {
                s.recording_state == RecordingState::Recording && s.recording_started_at == Some(started_at)
            });
            if !same_recording {
                return;
            }
            if hold_key::is_down(code) {
                up_since = None;
                continue;
            }
            // Up long enough that a normal release event would have stopped
            // the recording already (also past the 600 ms minimum).
            let since = *up_since.get_or_insert_with(std::time::Instant::now);
            let recorded = started_at.elapsed();
            if since.elapsed() >= Duration::from_millis(400) && recorded >= Duration::from_millis(600) {
                log_to_file("Hold: key released without a release event — stopping");
                stop_audio_capture_and_process(&app, None);
                return;
            }
        }
    });
}

// ─── App Entry Point ─────────────────────────────────────────────────────────

/// Write a message to the persistent crash log (~/Library/Application Support/TypeUrVoice/crash.log).
/// This survives app termination, unlike stderr which goes nowhere in production builds.
pub(crate) fn log_to_file(msg: &str) {
    logging::append(msg);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must come first: move the pre-rename `VoiceInk` data folder before
    // anything (logging included) creates the new `TypeUrVoice` folder.
    if let Some(msg) = storage::settings::migrate_legacy_data_dir() {
        log_to_file(&msg);
    }
    // A previous run that crashed or was force-quit mid-recording may have
    // left the Mac muted.
    media::recover();

    // Install a panic hook that logs to file before crashing — stderr is lost in production
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!("PANIC: {}", info);
        eprintln!("[TypeUrVoice] {}", msg);
        log_to_file(&msg);
        default_hook(info);
    }));

    // One-shot migration: legacy `.api_key` (OpenAI-only) → `keys.json`.
    if let Err(e) = storage::api_keys::migrate_legacy_if_needed() {
        log_to_file(&format!("api_keys migration error: {}", e));
    }

    let mut settings = match storage::settings::load() {
        Ok(s) => s,
        Err(e) => {
            log_to_file(&format!(
                "Failed to load settings.json: {} — falling back to defaults (onboarding will re-show)",
                e
            ));
            Settings::default()
        }
    };

    // One-shot migration: legacy single `custom_prompt` → first named profile.
    // Runs whenever `custom_profiles` is empty AND `custom_prompt` has content,
    // so users who set up Custom mode in the previous build keep their prompt.
    if settings.custom_profiles.is_empty() && !settings.custom_prompt.trim().is_empty() {
        let id = format!("legacy-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        settings.custom_profiles.push(state::CustomProfile {
            id: id.clone(),
            name: "My prompt".to_string(),
            prompt: settings.custom_prompt.clone(),
        });
        settings.active_custom_profile_id = Some(id);
        settings.custom_prompt.clear();
        if let Err(e) = storage::settings::save(&settings) {
            log_to_file(&format!("Failed to persist custom-prompt migration: {}", e));
        } else {
            log_to_file("Migrated legacy custom_prompt → custom_profiles[0]");
        }
    }

    if let Some((old, new)) = storage::settings::migrate_retired_models(&mut settings) {
        match storage::settings::save(&settings) {
            Ok(()) => log_to_file(&format!("Migrated retired model {} → {}", old, new)),
            Err(e) => log_to_file(&format!("Failed to persist model migration: {}", e)),
        }
    }

    // Defense-in-depth onboarding skip. The on-disk flag is the source of
    // truth, but if it's somehow false while the user clearly has prior
    // configuration (any saved API key, any downloaded local model), treat
    // onboarding as already complete and self-heal the flag. Prevents the
    // welcome flow from re-appearing whenever something writes back stale
    // defaults to settings.json.
    let has_any_key = storage::api_keys::has_any();
    let has_any_model: bool = {
        #[cfg(feature = "local-whisper")]
        {
            transcription::local::models_dir()
                .map(|d| std::fs::read_dir(&d)
                    .map(|rd| rd.filter_map(|e| e.ok())
                        .any(|e| e.path().extension()
                            .and_then(|x| x.to_str()) == Some("bin")))
                    .unwrap_or(false))
                .unwrap_or(false)
        }
        #[cfg(not(feature = "local-whisper"))]
        { false }
    };
    let onboarding_complete = if settings.onboarding_complete {
        true
    } else if has_any_key || has_any_model {
        log_to_file(&format!(
            "Onboarding self-heal: onboarding_complete was false but has_any_key={} has_any_model={} — marking complete",
            has_any_key, has_any_model
        ));
        settings.onboarding_complete = true;
        if let Err(e) = storage::settings::save(&settings) {
            log_to_file(&format!("Failed to persist onboarding self-heal: {}", e));
        }
        true
    } else {
        false
    };
    log_to_file(&format!(
        "Launch: onboarding_complete={} (has_any_key={}, has_any_model={})",
        onboarding_complete, has_any_key, has_any_model
    ));


    let shared_settings: SharedSettings = Arc::new(Mutex::new(settings));
    let shared_state: SharedState = Arc::new(Mutex::new(AppState::default()));
    let shared_http: reqwest::Client = http::build();

    #[cfg(feature = "local-whisper")]
    let shared_cache: std::sync::Arc<transcription::whisper_cache::WhisperCache> =
        std::sync::Arc::new(transcription::whisper_cache::WhisperCache::new());
    #[cfg(not(feature = "local-whisper"))]
    let shared_cache: std::sync::Arc<()> = std::sync::Arc::new(());

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(shared_settings)
        .manage(shared_state)
        .manage(state::QuitFlag(std::sync::atomic::AtomicBool::new(false)))
        .manage(shared_http)
        .manage(shared_cache)
        .manage::<SharedLlm>(Arc::new(local_llm::server::LlmServer::default()))
        .manage(knock::KnockListener::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            save_api_key,
            get_api_key,
            delete_api_key,
            test_api_key,
            check_permissions,
            open_permission_settings,
            get_history,
            delete_history_entry,
            clear_history,
            list_local_models,
            download_local_model,
            delete_local_model,
            test_text_processor,
            check_for_updates,
            list_input_devices,
            show_crash_log,
            get_active_provider_info,
            fetch_provider_models,
            list_occupation_profiles,
            set_active_occupation_profile,
            save_user_occupation_profile,
            delete_user_occupation_profile,
            list_builtin_llm_models,
            download_builtin_llm_model,
            delete_builtin_llm_model,
            get_builtin_llm_status,
            unload_builtin_llm,
            knock_available,
            media_control_available,
            get_speaker_profile,
            save_speaker_profile,
            clear_speaker_profile,
            learn_speaker_profile_now,
            correct_history_entry,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Err(e) = window.hide() {
                    log_to_file(&format!("window.hide() failed: {}", e));
                }
                api.prevent_close();
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                request_accessibility_if_needed();
                log_to_file(&format!("Launch: accessibility_trusted={}", paste::accessibility_trusted()));
            }

            tray::build_tray(&app.handle())?;
            // Cmd+Q intercept is registered once here, not from re_register_shortcuts,
            // so it survives every settings save (which only re-binds primary/secondary).
            register_cmd_q_intercept(&app.handle());
            re_register_shortcuts(&app.handle(), None);

            if !onboarding_complete {
                if let Some(w) = app.get_webview_window("onboarding") {
                    let _ = w.show();
                }
            } else {
                if let Some(w) = app.get_webview_window("settings") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }

            if let Ok(conn) = history::open() {
                let retention = with_settings(&app.handle(), |s| s.history_retention_days);
                let _ = history::cleanup_old(&conn, retention);
            }

            prewarm_local_model(&app.handle());

            local_llm::server::kill_orphan();
            sync_builtin_llm(&app.handle());
            tauri::async_runtime::spawn(local_llm::server::run_idle_watcher(app.handle().clone()));
            knock::sync(&app.handle());

            eprintln!("[TypeUrVoice] App started — ready");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building TypeUrVoice")
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    // Allow exit if the user explicitly clicked "Quit TypeUrVoice"
                    let quitting = app.try_state::<state::QuitFlag>()
                        .map(|f| f.0.load(Ordering::Relaxed))
                        .unwrap_or(false);
                    if quitting {
                        eprintln!("[TypeUrVoice] ExitRequested — user quit, allowing exit");
                    } else {
                        eprintln!("[TypeUrVoice] ExitRequested — preventing exit (app stays in tray)");
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::Exit => {
                    eprintln!("[TypeUrVoice] Exit event — app shutting down");
                    // Never leave the Mac muted or a video paused after Quit.
                    media::restore();
                    // Never leave a multi-GB model server running after Quit.
                    if let Some(llm) = app.try_state::<SharedLlm>() {
                        llm.stop_now();
                    }
                }
                _ => {}
            }
        });
}
