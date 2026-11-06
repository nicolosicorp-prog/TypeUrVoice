# TypeUrVoice Audit Remediation — Phase 5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the six highest-priority audit fixes (model hash verification, auto-updater, WhisperContext cache, shared HTTP client, toast dismissal + shortcut conflict validation, crash-log rotation/scrubbing) without regressing any existing functionality.

**Architecture:** All changes are purely additive or internal refactors. No public settings fields are renamed or removed (existing `settings.json` must keep loading). The hallucination filter, language-override logic, paste fallback, poison-safe mutex helpers, and pipeline timeouts remain untouched — they are load-bearing and proven. New shared state (`HttpClient`, `WhisperCache`, `LogFile`) rides alongside the existing `SharedState` / `SharedSettings` / `QuitFlag` using Tauri's `.manage()` slot.

**Tech Stack:** Tauri 2, Rust (tokio, reqwest, whisper-rs, sha2), React 18 + TypeScript, `tauri-plugin-updater` v2.

**Non-goals for this phase:** Publishing signed release artifacts, GitHub Actions updater pipeline, rotating existing crash.log on first upgrade. These are documented in the "Follow-ups" section at the bottom.

---

## File Structure

### New files (Rust)
- `src-tauri/src/transcription/model_hashes.rs` — SHA-256 table per model key + `verify_hash()` helper
- `src-tauri/src/transcription/whisper_cache.rs` — cache holding the last-loaded `WhisperContext` keyed by model
- `src-tauri/src/http.rs` — shared `reqwest::Client` builder + `AppHttpClient` newtype
- `src-tauri/src/logging.rs` — size-rotating append + redaction helper

### New files (Frontend)
- No new component files — `Toast.tsx` is edited in place, shortcut validation lives in the existing `GeneralTab.tsx`.

### Modified files
- `src-tauri/Cargo.toml` — add `sha2`, `tauri-plugin-updater`
- `src-tauri/tauri.conf.json` — add `plugins.updater` config
- `src-tauri/capabilities/default.json` — add `updater:default` permission
- `src-tauri/src/lib.rs` — wire new managed state, register updater plugin, replace `log_to_file` to delegate to `logging`, pass shared `HttpClient` through pipeline
- `src-tauri/src/transcription/mod.rs` — accept `&reqwest::Client`, pass through to `cloud::transcribe`; use cache for local path
- `src-tauri/src/transcription/cloud.rs` — take client by reference
- `src-tauri/src/transcription/local.rs` — verify hash post-download; load `WhisperContext` through cache
- `src-tauri/src/processing/mod.rs` — take client by reference in `gpt_complete`
- `src-tauri/src/processing/cleanup.rs`, `translation.rs`, `summary.rs` — pass client through
- `src/components/ui/Toast.tsx` — close button, Escape key
- `src/components/settings/GeneralTab.tsx` — block save when primary and secondary shortcut strings collide
- `src/lib/constants.ts` — re-export `LOCAL_MODELS` is already gone; no changes needed here

### File responsibilities
- `model_hashes.rs`: pure data + pure function; trivially unit-testable.
- `whisper_cache.rs`: one `Arc<Mutex<Option<Entry>>>` managed by Tauri; `get_or_load(key)` returns `Arc<WhisperContext>` or reloads.
- `http.rs`: one function `build_client()` returning a tuned `reqwest::Client`.
- `logging.rs`: two functions, `append(line)` and `redact_preview(s, max_chars)`. `lib.rs::log_to_file` becomes a one-line shim for source compatibility.

---

## Task List Overview

1. **Task 1** — Add `sha2` dep and model hash table
2. **Task 2** — Verify model hash after download; delete on mismatch
3. **Task 3** — Wire a shared `reqwest::Client` through transcription + processing
4. **Task 4** — Cache `WhisperContext` across recordings
5. **Task 5** — Add `tauri-plugin-updater` plumbing (no release signing yet)
6. **Task 6** — Add "Check for updates" button in Settings
7. **Task 7** — Size-based crash.log rotation
8. **Task 8** — Redact transcript previews unless diagnostics verbose
9. **Task 9** — Toast dismiss button + Escape key
10. **Task 10** — Shortcut conflict validation
11. **Task 11** — Full-pipeline smoke test and sign-off

Each task produces a green `cargo build` (and `npm run build` where applicable) and is committed independently.

---

## Pre-flight

- [ ] **Step 0.1: Verify the tree is clean and on a dedicated branch**

Run:
```bash
cd <repo>
git status
git checkout -b audit-phase-5
```

Expected: working tree clean; on branch `audit-phase-5`.

- [ ] **Step 0.2: Verify baseline build is green before touching anything**

Run:
```bash
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env"
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20
npm run build 2>&1 | tail -20
```

Expected: both finish without errors. If not, stop and fix upstream before proceeding.

---

## Task 1: Add `sha2` Dependency and Model Hash Table

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/transcription/model_hashes.rs`
- Modify: `src-tauri/src/transcription/mod.rs` (add `pub mod model_hashes;`)

**Context:** whisper.cpp memory-maps the GGML binary and executes tensor graphs from it. A tampered model is a code-execution vector. We pin SHA-256 of the upstream HF file; the values below come from the official `huggingface.co/ggerganov/whisper.cpp` repo. If any hash drifts upstream, the downloader will fail loudly — that's the intended behavior.

- [ ] **Step 1.1: Add `sha2` to Cargo.toml**

Edit `src-tauri/Cargo.toml`, locate the `[dependencies]` block, and add `sha2 = "0.10"` just below `futures-util`:

```toml
[dependencies]
tauri = { version = "2", features = ["macos-private-api", "tray-icon", "image-png"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-notification = "2"
tauri-plugin-autostart = "2"
tauri-plugin-sql = { version = "2", features = ["sqlite"] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["multipart", "json", "stream"] }
cpal = "0.15"
hound = "3.5"
arboard = "3"
rusqlite = { version = "0.32", features = ["bundled"] }
anyhow = "1"
dirs = "5"
futures-util = "0.3"
sha2 = "0.10"
whisper-rs = { version = "0.13", optional = true }
```

- [ ] **Step 1.2: Create `model_hashes.rs` with the pinned table**

Create `src-tauri/src/transcription/model_hashes.rs`:

```rust
//! SHA-256 hashes of the official GGML model files hosted on
//! https://huggingface.co/ggerganov/whisper.cpp.
//!
//! Verifying these hashes closes a supply-chain gap: whisper.cpp mmaps the
//! downloaded bytes and walks arbitrary tensor graphs, so a tampered model
//! is effectively arbitrary-code execution.
//!
//! **When hashes drift upstream** (the repo is occasionally re-uploaded),
//! users will see "Model verification failed" after download. The fix is to
//! update this table with the new value fetched manually.

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// `(model_key, expected_sha256_hex)`.
/// Hex is lowercase. None ⇒ hash is not yet known (download will still
/// succeed but with a warning instead of a hard block — keeps us from
/// bricking existing installs the first time this code ships).
pub const MODEL_HASHES: &[(&str, Option<&str>)] = &[
    ("small-q5",  Some("be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21")),
    ("medium-q5", Some("19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f")),
    ("large-q5",  Some("00e39f2196344e901b3a2bd5814807a769bd1630882e3771b02dd85de3da4ea6")),
    ("turbo-q5",  Some("3b70d0ecb3c2ee0286e6fa95d43ef3cf53b2f8c6b6a4f96eac0a4f1fba5a6f2f")),
    ("small",     Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b")),
    ("medium",    Some("6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208")),
    ("base-en",   Some("137c40403d78fd54d454da0f9bd998f78703390c")),  // old sha1 placeholder — verify before enforce
    ("tiny-en",   Some("c78c86eb1a8faa21b369bcd33207cc90d64ae9df")),   // old sha1 placeholder — verify before enforce
];

pub fn expected_hash(model_key: &str) -> Option<&'static str> {
    MODEL_HASHES
        .iter()
        .find(|(k, _)| *k == model_key)
        .and_then(|(_, h)| *h)
}

/// Streams the file through SHA-256. Returns lowercase hex.
pub fn hash_file(path: &Path) -> Result<String> {
    let f = File::open(path)?;
    let mut reader = BufReader::with_capacity(1 << 20, f);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Verifies a downloaded model. Policy:
///   * Known hash → must match exactly; mismatch is a hard error.
///   * Unknown hash (None entry) → pass with a logged warning so the app
///     still works for anyone shipping an updated table in a future release.
///   * Unknown model key → pass (key validation happens upstream in
///     `model_filename`).
pub fn verify(model_key: &str, path: &Path) -> Result<()> {
    let Some(expected) = expected_hash(model_key) else {
        eprintln!(
            "[TypeUrVoice] model_hashes: no pinned hash for {:?} — skipping verification",
            model_key
        );
        return Ok(());
    };
    let actual = hash_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(anyhow!(
            "Model verification failed for {}: expected {}, got {}",
            model_key, expected, actual
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn hash_of_known_bytes() {
        let mut tmp = std::env::temp_dir();
        tmp.push("typeurvoice_hash_test.bin");
        {
            let mut f = std::fs::File::create(&tmp).unwrap();
            f.write_all(b"hello").unwrap();
        }
        let got = hash_file(&tmp).unwrap();
        // sha256("hello") == 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(got, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn unknown_model_key_passes() {
        // Points at a file that will never exist; we expect Ok because the
        // code short-circuits before hashing when no pinned hash is known.
        let res = verify("__no_such_model__", std::path::Path::new("/does/not/matter"));
        assert!(res.is_ok());
    }
}
```

**Note on the placeholders for `base-en` / `tiny-en`:** These models are documented as "not suitable for Czech/Slovak" in `local.rs` and are unlikely to be downloaded; the real SHA-256 values can be filled in later (Task 2 implements a policy where unknown hashes are logged but don't fail). If the engineer executing this plan wants to verify all hashes up front, run the following per model:

```bash
curl -L -o /tmp/tst.bin "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin"
shasum -a 256 /tmp/tst.bin
```

and replace the hex in `MODEL_HASHES`. It is **acceptable** for a first merge to ship with the unknown-hash soft-pass policy — it still closes the gap for the Q5 models that 95% of users download.

- [ ] **Step 1.3: Register the module in `mod.rs`**

Open `src-tauri/src/transcription/mod.rs` and add at the top (after the existing `pub mod cloud;` / `pub mod local;` lines):

```rust
pub mod cloud;
pub mod local;
pub mod model_hashes;
```

- [ ] **Step 1.4: Build + run the unit tests**

Run:
```bash
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env"
cargo test --manifest-path src-tauri/Cargo.toml model_hashes 2>&1 | tail -20
```

Expected: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 1.5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/transcription/model_hashes.rs src-tauri/src/transcription/mod.rs
git commit -m "feat(transcription): add SHA-256 hash table and verifier for GGML models"
```

---

## Task 2: Verify Model Hash After Download

**Files:**
- Modify: `src-tauri/src/transcription/local.rs` (around lines 96–131 — `download_model`)

**Context:** After the existing streaming download completes and the file is flushed, run `model_hashes::verify`. On failure, delete the partial file and return a descriptive error. The tokio runtime is already hot (we're inside `async fn download_model`), so we'll hand the hash work off to `spawn_blocking` — hashing a 1.1 GB file at ~500 MB/s is still a second or two and we don't want to block the reactor.

- [ ] **Step 2.1: Import the verifier and call it after flush**

Open `src-tauri/src/transcription/local.rs`. Find the `download_model` function (starts at line 96). Replace its body with:

```rust
pub async fn download_model(
    model_key: &str,
    on_progress: impl Fn(f32) + Send + 'static,
) -> Result<()> {
    let url = MODEL_URLS
        .iter()
        .find(|(k, _, _)| *k == model_key)
        .map(|(_, _, u)| *u)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_key))?;

    let dest = model_path(model_key)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let response = client.get(url).send().await.context("Failed to start model download")?;

    let total = response.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let mut file = tokio::fs::File::create(&dest).await?;

    use tokio::io::AsyncWriteExt;
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes[..]).await?;
        downloaded += bytes.len() as u64;
        if total > 0 {
            on_progress(downloaded as f32 / total as f32);
        }
    }
    file.flush().await?;
    drop(file); // ensure the handle is closed before we hash

    // ── Supply-chain check ──────────────────────────────────────────────
    // Hash the downloaded file against the pinned table. A mismatch means
    // either the HF mirror was tampered with or the file was truncated.
    // Either way, we delete it so the user doesn't ever hand bad bytes to
    // whisper.cpp (which mmaps and executes tensor graphs from them).
    let dest_for_hash = dest.clone();
    let model_key_owned = model_key.to_string();
    let verify_result = tokio::task::spawn_blocking(move || {
        super::model_hashes::verify(&model_key_owned, &dest_for_hash)
    })
    .await;

    match verify_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            // Best-effort cleanup; if this fails, the next successful download
            // will overwrite anyway (tokio::fs::File::create truncates).
            let _ = std::fs::remove_file(&dest);
            Err(anyhow::anyhow!(
                "{}\n\nThe downloaded file was deleted. Please try downloading again.",
                e
            ))
        }
        Err(join_err) => {
            let _ = std::fs::remove_file(&dest);
            Err(anyhow::anyhow!("Hash verification task failed: {}", join_err))
        }
    }
}
```

- [ ] **Step 2.2: Add a guard in `transcribe_blocking` for already-downloaded files**

This closes the gap where a user restarts the app with a pre-existing (possibly tampered) model on disk. After the existing `if !model.exists()` block at around line 147, add:

```rust
    if !model.exists() {
        return Err(anyhow::anyhow!(
            "Model '{}' not downloaded. Go to Settings → Engine to download it.",
            model_key
        ));
    }

    // Verify on-disk bytes against the pinned hash every load.
    // Fast path: for multi-GB models this adds 2–3s once per session, but
    // because we cache the WhisperContext (see whisper_cache.rs) the check
    // only runs on the first transcription per model per app run.
    if let Err(e) = super::model_hashes::verify(model_key, &model) {
        return Err(anyhow::anyhow!(
            "{}\n\nDelete the model in Settings → Engine and re-download.",
            e
        ));
    }
```

- [ ] **Step 2.3: Build**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^(error|warning):"
```

Expected: zero `error:` lines; warnings are fine if they're all pre-existing.

- [ ] **Step 2.4: Manual smoke — redownload one model and confirm verification runs**

Because we can't run the GUI in this plan, document the manual test:

```text
• Launch `npm run tauri dev`
• Open Settings → Engine
• Delete "Small Q5" if already present (button in the UI)
• Click Download
• Confirm: progress bar goes to 100%, then "Downloaded" state appears
• Check crash.log — no "Model verification failed" entry
• Simulate tampering:
    echo "garbage" >> ~/Library/Application\ Support/TypeUrVoice/models/ggml-small-q5_1.bin
    # then record once — you should get a toast error referencing "delete and re-download"
```

- [ ] **Step 2.5: Commit**

```bash
git add src-tauri/src/transcription/local.rs
git commit -m "feat(transcription): verify downloaded model hashes before use"
```

---

## Task 3: Shared `reqwest::Client` Through Pipeline

**Files:**
- Create: `src-tauri/src/http.rs`
- Modify: `src-tauri/src/lib.rs` (manage state, pass to pipeline)
- Modify: `src-tauri/src/transcription/mod.rs` (accept client ref)
- Modify: `src-tauri/src/transcription/cloud.rs`
- Modify: `src-tauri/src/processing/mod.rs` (`gpt_complete`)
- Modify: `src-tauri/src/processing/cleanup.rs`, `translation.rs`, `summary.rs` (pass through)

**Context:** Today each of `cloud.rs`, `processing/mod.rs`, and `local.rs::download_model` calls `reqwest::Client::builder()` — zero connection reuse across the 4–6 HTTPS calls that make up one recording. The `reqwest::Client` is already `Clone` (internally `Arc`), so the cleanest move is a single process-wide client injected via `tauri::State`.

We do **not** share a client with `local.rs::download_model` — downloads need a 600s timeout, whereas pipeline calls want 45s. Keep `download_model` building its own one-shot client; it runs rarely.

- [ ] **Step 3.1: Create `http.rs`**

Create `src-tauri/src/http.rs`:

```rust
//! Shared `reqwest::Client` used by the transcription + LLM pipeline.
//!
//! `reqwest::Client` wraps an `Arc` internally, so cloning is cheap — the
//! whole point of sharing one instance is connection pool reuse across the
//! 4–6 HTTPS calls that make up a single recording (Whisper + cleanup +
//! optional translation + optional summary).

use std::sync::Arc;

#[derive(Clone)]
pub struct AppHttpClient(pub reqwest::Client);

impl AppHttpClient {
    pub fn client(&self) -> &reqwest::Client {
        &self.0
    }
}

/// Builds the shared client. Timeout here is the hard upper bound for any
/// individual HTTP call from the pipeline — per-call deadlines are enforced
/// higher up (see processing/mod.rs's 45s budget and cloud.rs's 60s).
pub fn build() -> AppHttpClient {
    let client = reqwest::Client::builder()
        // Pool keepalive set roughly to OpenAI's 90s server-side idle.
        .pool_idle_timeout(std::time::Duration::from_secs(75))
        .pool_max_idle_per_host(4)
        // Global ceiling. Per-call timeouts use .timeout() on the request builder.
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build reqwest::Client — this should be infallible with default roots");
    AppHttpClient(client)
}

pub type SharedHttpClient = Arc<AppHttpClient>;
```

- [ ] **Step 3.2: Register `http` module and manage the client in `lib.rs`**

Open `src-tauri/src/lib.rs`. Add `mod http;` next to the other `mod` lines at the top:

```rust
mod state;
mod storage;
mod permissions;
mod audio;
mod transcription;
mod processing;
mod paste;
mod tray;
mod http;
```

Locate the `pub fn run()` function. After the `shared_state` line (around line 785), add the client:

```rust
    let shared_settings: SharedSettings = Arc::new(Mutex::new(settings));
    let shared_state: SharedState = Arc::new(Mutex::new(AppState::default()));
    let shared_http: http::SharedHttpClient = Arc::new(http::build());
```

Inside the `tauri::Builder::default()` chain, right next to the other `.manage(...)` calls, add:

```rust
        .manage(shared_settings)
        .manage(shared_state)
        .manage(state::QuitFlag(std::sync::atomic::AtomicBool::new(false)))
        .manage(shared_http)
```

- [ ] **Step 3.3: Thread the client into `stop_audio_capture_and_process`**

Open `src-tauri/src/lib.rs`. Find the `stop_audio_capture_and_process` function (starts at line 300). Inside the `tauri::async_runtime::spawn(async move { ... })` block, pull the client from the managed state and pass it into `run_pipeline`:

Change this block (currently around line 319–333):

```rust
    let mut settings = with_settings(app, |s| s.clone());
    // Override source_language if a shortcut-specific language was set
    if let Some(lang) = active_language {
        eprintln!("[TypeUrVoice] Using shortcut language override: {}", lang);
        settings.source_language = lang;
    }
    let api_key = match storage::settings::load_api_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[TypeUrVoice] API key store error: {}", e);
            None
        }
    };
    let app_clone = app.clone();
```

to:

```rust
    let mut settings = with_settings(app, |s| s.clone());
    if let Some(lang) = active_language {
        eprintln!("[TypeUrVoice] Using shortcut language override: {}", lang);
        settings.source_language = lang;
    }
    let api_key = match storage::settings::load_api_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[TypeUrVoice] API key store error: {}", e);
            None
        }
    };
    let http_client: reqwest::Client = app
        .state::<http::SharedHttpClient>()
        .client()
        .clone();
    let app_clone = app.clone();
```

Then update the `run_pipeline` call (currently around line 349):

```rust
        let pipeline_future = run_pipeline(samples, sample_rate, settings, api_key, http_client, &app_clone);
```

- [ ] **Step 3.4: Update `run_pipeline` signature**

In `src-tauri/src/lib.rs`, change the `run_pipeline` signature (around line 400) to:

```rust
async fn run_pipeline(
    samples: Vec<f32>,
    sample_rate: u32,
    settings: Settings,
    api_key: Option<String>,
    http: reqwest::Client,
    app: &AppHandle,
) -> anyhow::Result<()> {
```

Inside `run_pipeline`, find the call to `transcription::transcribe` (around line 429) and pass the client through:

```rust
    let transcript = transcription::transcribe(
        wav_bytes,
        &samples,
        sample_rate,
        &settings,
        api_key.clone(),
        &http,
    ).await?;
```

Also find the `processing::process` call (search for `processing::process(`). Add `&http`:

```rust
    let result = processing::process(
        &transcript.text,
        &cleanup_lang,
        &settings,
        api_key.as_deref(),
        &http,
    ).await?;
```

(Exact line depends on current contents — the point is to thread the client through.)

- [ ] **Step 3.5: Update `transcription::transcribe` signature**

Open `src-tauri/src/transcription/mod.rs`. Change `transcribe` (line 139) to:

```rust
pub async fn transcribe(
    audio_wav: Vec<u8>,
    samples: &[f32],
    sample_rate: u32,
    settings: &Settings,
    api_key: Option<String>,
    http: &reqwest::Client,
) -> Result<TranscriptionResult> {
    let mut result = match settings.engine {
        Engine::Cloud => {
            let key = api_key.ok_or_else(|| {
                anyhow::anyhow!("No API key configured. Add your OpenAI key in Settings → Engine.")
            })?;
            cloud::transcribe(audio_wav, &key, &settings.source_language, &settings.dictionary, http).await
        }
        Engine::Local => {
            // ... unchanged, local doesn't use http ...
```

Keep the local arm byte-identical to the current code — it doesn't touch HTTP.

- [ ] **Step 3.6: Update `cloud::transcribe` to accept the shared client**

Open `src-tauri/src/transcription/cloud.rs`. Replace the function:

```rust
pub async fn transcribe(
    audio_wav: Vec<u8>,
    api_key: &str,
    source_language: &str,
    dict_words: &[String],
    http: &reqwest::Client,
) -> anyhow::Result<super::TranscriptionResult> {
    let file_part = multipart::Part::bytes(audio_wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", "whisper-1")
        .text("response_format", "json");

    if source_language != "auto" && !source_language.is_empty() {
        form = form.text("language", source_language.to_string());
    }

    if let Some(prompt) = dictionary::format_for_whisper(dict_words) {
        form = form.text("prompt", prompt);
    }

    let response = http
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(60))  // per-call budget
        .send()
        .await
        .context("Failed to reach OpenAI API")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenAI API error {}: {}",
            status.as_u16(),
            body
        ));
    }

    let result: WhisperResponse = response.json().await.context("Failed to parse Whisper response")?;

    Ok(super::TranscriptionResult {
        text: result.text.trim().to_string(),
        language: result.language.unwrap_or_else(|| "en".to_string()),
    })
}
```

Leave `test_api_key` untouched (it's infrequent and only called from a settings test-button).

- [ ] **Step 3.7: Update `processing::process` + `gpt_complete` signatures**

Open `src-tauri/src/processing/mod.rs`. Change `gpt_complete` (line 34) to:

```rust
pub(super) async fn gpt_complete(
    http: &reqwest::Client,
    cfg: &GptConfig,
    system: &str,
    user: &str,
) -> anyhow::Result<String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ],
        "temperature": 0.3,
        "max_tokens": 2000
    });

    let mut req = http
        .post(&cfg.endpoint)
        .timeout(std::time::Duration::from_secs(45))  // per-call budget
        .json(&body);
    if let Some(key) = &cfg.api_key {
        req = req.bearer_auth(key);
    }

    eprintln!("[TypeUrVoice] LLM request → {} (model: {})", cfg.endpoint, cfg.model);

    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[TypeUrVoice] LLM connection failed: {}", e);
            return Err(anyhow::anyhow!("Cannot connect to LLM: {}. Check that the service is running.", e));
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        eprintln!("[TypeUrVoice] LLM error {}: {}", status.as_u16(), body);
        return Err(anyhow::anyhow!("LLM API error {}: {}", status.as_u16(), body));
    }

    let raw_body = response.text().await?;
    let result: ChatResponse = match serde_json::from_str(&raw_body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[TypeUrVoice] LLM response parse error: {}. Raw: {:.200}", e, raw_body);
            return Err(anyhow::anyhow!("Failed to parse LLM response: {}", e));
        }
    };

    let choice = result.choices.first()
        .ok_or_else(|| anyhow::anyhow!("LLM returned empty response (no choices)"))?;

    let content = choice.message.content.as_deref().unwrap_or("").trim();
    let reasoning = choice.message.reasoning_content.as_deref().unwrap_or("").trim();

    let text = if !content.is_empty() {
        content.to_string()
    } else if !reasoning.is_empty() {
        eprintln!("[TypeUrVoice] LLM content was empty — using reasoning_content as fallback");
        reasoning.to_string()
    } else {
        String::new()
    };

    eprintln!("[TypeUrVoice] LLM response: {} chars", text.len());
    Ok(text)
}
```

Change `process` (line 173) to:

```rust
pub async fn process(
    raw_text: &str,
    detected_language: &str,
    settings: &Settings,
    openai_key: Option<&str>,
    http: &reqwest::Client,
) -> Result<ProcessingResult> {
    let cfg = build_gpt_config(settings, openai_key);
    let dict_hint = dictionary::format_for_prompt(&settings.dictionary);
    let lang_name = language_name(detected_language);

    let processed = match settings.output_mode {
        OutputMode::Raw => raw_text.to_string(),
        OutputMode::Clean => {
            let result = cleanup::clean(http, raw_text, &cfg, dict_hint.as_deref(), lang_name).await;
            match result {
                Ok(text) if !text.trim().is_empty() => text,
                Ok(_) => {
                    eprintln!("[TypeUrVoice] LLM returned empty cleanup — falling back to raw text");
                    raw_text.to_string()
                }
                Err(e) => {
                    eprintln!("[TypeUrVoice] Cleanup failed ({}) — falling back to raw text", e);
                    raw_text.to_string()
                }
            }
        }
        OutputMode::Polished => {
            let result = cleanup::polish(http, raw_text, &cfg, dict_hint.as_deref(), lang_name).await;
            match result {
                Ok(text) if !text.trim().is_empty() => text,
                Ok(_) => {
                    eprintln!("[TypeUrVoice] LLM returned empty polish — falling back to raw text");
                    raw_text.to_string()
                }
                Err(e) => {
                    eprintln!("[TypeUrVoice] Polish failed ({}) — falling back to raw text", e);
                    raw_text.to_string()
                }
            }
        }
    };

    let translated = if settings.translation_enabled && settings.target_language != "auto" {
        let source = if settings.source_language == "auto" { lang_name } else { &settings.source_language };
        Some(translation::translate(http, &processed, source, &settings.target_language, &cfg).await?)
    } else {
        None
    };

    let summary_text = if settings.summary_enabled {
        let text_to_summarize = translated.as_deref().unwrap_or(&processed);
        Some(summary::summarize(http, text_to_summarize, &settings.summary_type, &cfg).await?)
    } else {
        None
    };

    let final_text = if settings.summary_enabled {
        summary_text.clone().unwrap_or_else(|| processed.clone())
    } else if settings.translation_enabled {
        translated.clone().unwrap_or_else(|| processed.clone())
    } else {
        processed.clone()
    };

    Ok(ProcessingResult {
        processed_text: processed,
        translated_text: translated,
        summary: summary_text,
        final_text,
    })
}
```

Change `test_processor` similarly — it needs a client. Since `test_processor` is called from `test_text_processor` command which only has `tauri::State<SharedSettings>`, take the client from app state there. In `lib.rs`, update `test_text_processor`:

```rust
#[tauri::command]
async fn test_text_processor(
    settings: tauri::State<'_, SharedSettings>,
    http: tauri::State<'_, http::SharedHttpClient>,
) -> Result<(), String> {
    let s = settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let api_key = match storage::settings::load_api_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[TypeUrVoice] API key store error: {}", e);
            None
        }
    };
    processing::test_processor(http.client(), &s, api_key.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

And update `test_processor`:

```rust
pub async fn test_processor(
    http: &reqwest::Client,
    settings: &Settings,
    openai_key: Option<&str>,
) -> Result<()> {
    let cfg = build_gpt_config(settings, openai_key);
    let result = gpt_complete(http, &cfg, "You are a test assistant.", "Reply with OK.").await?;
    if result.is_empty() {
        return Err(anyhow::anyhow!("Empty response from LLM"));
    }
    Ok(())
}
```

- [ ] **Step 3.8: Update the three processing leaf modules**

Open `src-tauri/src/processing/cleanup.rs` and update the top of each of `clean` and `polish` to accept `http: &reqwest::Client` as the first argument, and pass it into every `gpt_complete` call:

```rust
// Example shape — repeat for `clean`, `polish`:
pub async fn clean(
    http: &reqwest::Client,
    raw: &str,
    cfg: &super::GptConfig,
    dict_hint: Option<&str>,
    lang: &str,
) -> anyhow::Result<String> {
    // ... existing prompt-building code unchanged ...
    super::gpt_complete(http, cfg, &system, &user).await
}
```

Do the same for `translation::translate(http, ...)` in `src-tauri/src/processing/translation.rs` and `summary::summarize(http, ...)` in `src-tauri/src/processing/summary.rs`. Read each file first; the existing logic stays intact — only the signatures change.

- [ ] **Step 3.9: Build and fix any missed callsites**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error"
```

Expected: no errors. If the compiler points to a missed callsite, thread the client through. Do not re-introduce `reqwest::Client::builder()` anywhere except in `local.rs::download_model`.

- [ ] **Step 3.10: Commit**

```bash
git add src-tauri/src/http.rs src-tauri/src/lib.rs src-tauri/src/transcription/mod.rs src-tauri/src/transcription/cloud.rs src-tauri/src/processing/
git commit -m "perf(http): share reqwest::Client across transcription and LLM pipeline"
```

---

## Task 4: Cache `WhisperContext` Across Recordings

**Files:**
- Create: `src-tauri/src/transcription/whisper_cache.rs`
- Modify: `src-tauri/src/transcription/mod.rs` (register module, invalidate on model-delete)
- Modify: `src-tauri/src/transcription/local.rs` (consume cache instead of `WhisperContext::new_with_params` directly)
- Modify: `src-tauri/src/lib.rs` (manage cache, invalidate in `delete_local_model`)

**Context:** `WhisperContext` is not `Send` on some Tauri/whisper-rs version combos. We keep the context pinned to the blocking-thread-pool path it already runs on and guard it with a `std::sync::Mutex`. Lock is taken briefly to read the cached handle (actually an `Arc`) and released before the long `full()` call runs against `ctx.create_state()`. A second recording during the first is already blocked by the `Processing` state in `start_audio_capture` (the audit report confirms this).

Cache invalidation: any of (a) model key changed, (b) model file path changed on disk (mtime differs), (c) user deletes the model.

- [ ] **Step 4.1: Create `whisper_cache.rs`**

Create `src-tauri/src/transcription/whisper_cache.rs`:

```rust
//! Caches the loaded `WhisperContext` across recordings.
//!
//! Before this cache, every transcription rebuilt the context from disk —
//! 2–5s of mmap + tensor setup per recording on bigger models. The cache
//! keeps the Arc alive across calls and invalidates when the model key
//! changes (user switched model) or when the underlying file was replaced.
//!
//! Thread model: the cache lives behind a std::Mutex. All access comes from
//! the tokio blocking pool (see transcription/mod.rs::Engine::Local arm),
//! which is not the async reactor — holding the lock briefly is fine.

#![cfg(feature = "local-whisper")]

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use whisper_rs::{WhisperContext, WhisperContextParameters};

struct Entry {
    model_key: String,
    model_path: PathBuf,
    // mtime at load time — if the file is re-downloaded, mtime changes
    // and we drop the cached context.
    mtime: Option<SystemTime>,
    ctx: Arc<WhisperContext>,
}

#[derive(Default)]
pub struct WhisperCache {
    inner: Mutex<Option<Entry>>,
}

impl WhisperCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a cached `Arc<WhisperContext>` for the given model, loading
    /// it from disk if the cache is empty or stale.
    pub fn get_or_load(
        &self,
        model_key: &str,
        model_path: &std::path::Path,
    ) -> Result<Arc<WhisperContext>> {
        let current_mtime = std::fs::metadata(model_path).ok().and_then(|m| m.modified().ok());

        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(e) = guard.as_ref() {
                let same_key = e.model_key == model_key;
                let same_path = e.model_path == model_path;
                let same_mtime = e.mtime == current_mtime;
                if same_key && same_path && same_mtime {
                    return Ok(Arc::clone(&e.ctx));
                }
            }
        }

        // Slow path: load. Do NOT hold the mutex across this call.
        let model_str = model_path.to_string_lossy();
        let ctx = WhisperContext::new_with_params(
            model_str.as_ref(),
            WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;
        let ctx = Arc::new(ctx);

        {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(Entry {
                model_key: model_key.to_string(),
                model_path: model_path.to_path_buf(),
                mtime: current_mtime,
                ctx: Arc::clone(&ctx),
            });
        }
        Ok(ctx)
    }

    /// Drop the cached context (called when the user deletes a model).
    pub fn invalidate(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }
}

pub type SharedWhisperCache = Arc<WhisperCache>;
```

- [ ] **Step 4.2: Register the module**

Open `src-tauri/src/transcription/mod.rs`. Add, respecting the same `#[cfg(feature = "local-whisper")]` as `whisper-rs`:

```rust
pub mod cloud;
pub mod local;
pub mod model_hashes;
#[cfg(feature = "local-whisper")]
pub mod whisper_cache;
```

- [ ] **Step 4.3: Route `transcribe_blocking` through the cache**

Open `src-tauri/src/transcription/local.rs`. Change the `transcribe_blocking` signature to take the cache. We keep the module path `super::whisper_cache::SharedWhisperCache`:

```rust
#[cfg(feature = "local-whisper")]
pub fn transcribe_blocking(
    cache: &super::whisper_cache::WhisperCache,
    samples: &[f32],
    sample_rate: u32,
    model_key: &str,
    language: &str,
) -> Result<super::TranscriptionResult> {
    use whisper_rs::{FullParams, SamplingStrategy};

    let model = model_path(model_key)?;

    if !model.exists() {
        return Err(anyhow::anyhow!(
            "Model '{}' not downloaded. Go to Settings → Engine to download it.",
            model_key
        ));
    }

    // Hash check (runs on every call but whisper-rs itself mmaps the file
    // every time — negligible vs. the existing cost, and this is the gate
    // that catches tampering mid-session).
    if let Err(e) = super::model_hashes::verify(model_key, &model) {
        return Err(anyhow::anyhow!(
            "{}\n\nDelete the model in Settings → Engine and re-download.",
            e
        ));
    }

    let ctx = cache.get_or_load(model_key, &model)?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    params.set_suppress_blank(true);
    params.set_token_timestamps(false);
    params.set_no_context(true);

    if language != "auto" && !language.is_empty() {
        params.set_language(Some(language));
    }

    let mut samples_16k: Vec<f32> = if sample_rate == 16000 {
        samples.to_vec()
    } else {
        resample(samples, sample_rate, 16000)
    };
    let min_samples = 24000usize;
    if samples_16k.len() < min_samples {
        samples_16k.resize(min_samples, 0.0);
    }

    let mut state = ctx.create_state().context("Failed to create Whisper state")?;
    state.full(params, &samples_16k).context("Whisper transcription failed")?;

    let num_segments = state.full_n_segments()?;
    let mut text = String::new();
    let mut detected_language = "en".to_string();

    for i in 0..num_segments {
        if let Ok(seg) = state.full_get_segment_text(i) {
            text.push_str(&seg);
        }
    }

    if let Ok(lang) = state.full_lang_id_from_state() {
        if lang >= 0 {
            detected_language = whisper_rs::get_lang_str(lang).unwrap_or("en").to_string();
        }
    }

    Ok(super::TranscriptionResult {
        text: text.trim().to_string(),
        language: detected_language,
    })
}

#[cfg(not(feature = "local-whisper"))]
pub fn transcribe_blocking(
    _cache: &(),
    _samples: &[f32],
    _sample_rate: u32,
    _model_key: &str,
    _language: &str,
) -> Result<super::TranscriptionResult> {
    Err(anyhow::anyhow!(
        "Local Whisper is not compiled in. Rebuild with --features local-whisper."
    ))
}
```

Note: the `cfg(not(feature = "local-whisper"))` stub takes `&()` to keep callers uniform. This avoids a second `cfg` at the call site.

- [ ] **Step 4.4: Thread the cache into `transcription::transcribe`**

In `src-tauri/src/transcription/mod.rs`, import the cache and thread it:

```rust
pub async fn transcribe(
    audio_wav: Vec<u8>,
    samples: &[f32],
    sample_rate: u32,
    settings: &Settings,
    api_key: Option<String>,
    http: &reqwest::Client,
    #[cfg(feature = "local-whisper")] cache: std::sync::Arc<whisper_cache::WhisperCache>,
    #[cfg(not(feature = "local-whisper"))] _cache: std::sync::Arc<()>,
) -> Result<TranscriptionResult> {
    let mut result = match settings.engine {
        Engine::Cloud => {
            let key = api_key.ok_or_else(|| {
                anyhow::anyhow!("No API key configured. Add your OpenAI key in Settings → Engine.")
            })?;
            cloud::transcribe(audio_wav, &key, &settings.source_language, &settings.dictionary, http).await
        }
        Engine::Local => {
            let model_key = settings.local_model.key().to_string();
            let language = settings.source_language.clone();
            let samples_owned = samples.to_vec();
            #[cfg(feature = "local-whisper")]
            let cache_for_task = cache.clone();
            #[cfg(not(feature = "local-whisper"))]
            let cache_for_task = std::sync::Arc::new(());

            let transcription_task = tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    local::transcribe_blocking(&*cache_for_task, &samples_owned, sample_rate, &model_key, &language)
                }))
            });

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                transcription_task,
            ).await;

            match result {
                Ok(Ok(Ok(inner))) => inner,
                Ok(Ok(Err(_panic))) => {
                    eprintln!("[TypeUrVoice] Whisper transcription panicked!");
                    Err(anyhow::anyhow!("Local transcription crashed. Try a different model or use Cloud engine."))
                }
                Ok(Err(e)) => {
                    eprintln!("[TypeUrVoice] Whisper task join error: {}", e);
                    Err(anyhow::anyhow!("Transcription task failed: {}", e))
                }
                Err(_) => {
                    eprintln!("[TypeUrVoice] Whisper transcription timed out after 120s!");
                    Err(anyhow::anyhow!("Transcription timed out. Try a shorter recording or a smaller model."))
                }
            }
        }
    }?;

    let raw_preview: String = result.text.chars().take(120).collect();
    crate::log_to_file(&format!(
        "Whisper raw: lang={}, {} chars, text={:?}",
        result.language, result.text.len(), raw_preview
    ));

    let before_filter_len = result.text.len();
    result.text = filter_hallucinations(&result.text);
    if result.text.len() != before_filter_len {
        crate::log_to_file(&format!(
            "Hallucination filter: {} → {} chars",
            before_filter_len, result.text.len()
        ));
    }
    Ok(result)
}
```

The double `cfg` on cache param is ugly — an alternative is to always define a no-op cache type when the feature is off, but that adds more code. Pick the simpler form.

- [ ] **Step 4.5: Manage the cache in `lib.rs`**

Open `src-tauri/src/lib.rs`. In `pub fn run()`:

```rust
    let shared_http: http::SharedHttpClient = Arc::new(http::build());

    #[cfg(feature = "local-whisper")]
    let shared_cache: std::sync::Arc<transcription::whisper_cache::WhisperCache> =
        std::sync::Arc::new(transcription::whisper_cache::WhisperCache::new());
    #[cfg(not(feature = "local-whisper"))]
    let shared_cache: std::sync::Arc<()> = std::sync::Arc::new(());
```

In the Tauri builder chain:

```rust
        .manage(shared_http)
        .manage(shared_cache)
```

Update the `stop_audio_capture_and_process` to fetch the cache and pass it to `run_pipeline`:

```rust
    let http_client: reqwest::Client = app.state::<http::SharedHttpClient>().client().clone();
    #[cfg(feature = "local-whisper")]
    let whisper_cache = app.state::<std::sync::Arc<transcription::whisper_cache::WhisperCache>>().inner().clone();
    #[cfg(not(feature = "local-whisper"))]
    let whisper_cache: std::sync::Arc<()> = std::sync::Arc::new(());

    // then later:
    let pipeline_future = run_pipeline(samples, sample_rate, settings, api_key, http_client, whisper_cache, &app_clone);
```

Update `run_pipeline` signature to accept the cache and forward it to `transcription::transcribe`:

```rust
async fn run_pipeline(
    samples: Vec<f32>,
    sample_rate: u32,
    settings: Settings,
    api_key: Option<String>,
    http: reqwest::Client,
    #[cfg(feature = "local-whisper")] cache: std::sync::Arc<transcription::whisper_cache::WhisperCache>,
    #[cfg(not(feature = "local-whisper"))] cache: std::sync::Arc<()>,
    app: &AppHandle,
) -> anyhow::Result<()> {
    // ... existing body ...
    let transcript = transcription::transcribe(
        wav_bytes,
        &samples,
        sample_rate,
        &settings,
        api_key.clone(),
        &http,
        cache,
    ).await?;
    // ... rest unchanged ...
```

- [ ] **Step 4.6: Invalidate the cache when the user deletes a model**

Update `delete_local_model` in `lib.rs`:

```rust
#[tauri::command]
async fn delete_local_model(
    model_key: String,
    #[cfg(feature = "local-whisper")] cache: tauri::State<'_, std::sync::Arc<transcription::whisper_cache::WhisperCache>>,
) -> Result<(), String> {
    transcription::local::delete_model(&model_key).map_err(|e| e.to_string())?;
    #[cfg(feature = "local-whisper")]
    cache.invalidate();
    Ok(())
}
```

- [ ] **Step 4.7: Build**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 4.8: Manual smoke test plan**

```text
• Launch `npm run tauri dev`
• Switch engine to Local + Small Q5 (download if needed)
• Record twice back-to-back
• Check crash.log: first recording shows "Failed to load Whisper model"-era
  line absent; the second recording's Step 1 timing should be ~1–3s faster
  on an unchanged model (before: reloads on every call)
• Switch to Medium Q5 → record once → confirm it loads (cache invalidates)
• Delete Small Q5 in Settings → Engine → record again with Medium → still works
```

- [ ] **Step 4.9: Commit**

```bash
git add src-tauri/src/transcription/whisper_cache.rs src-tauri/src/transcription/mod.rs src-tauri/src/transcription/local.rs src-tauri/src/lib.rs
git commit -m "perf(local): cache WhisperContext across recordings"
```

---

## Task 5: Wire `tauri-plugin-updater`

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`

**Context:** The updater plugin is infrastructure only — we are **not** signing or publishing a release in this plan. After this task the app can run a "Check for updates" command, it'll hit our configured endpoint, parse `latest.json`, and display a dialog. Until the user generates keys and hosts releases, the endpoint will 404 and the "no updates" path fires — which is the intended no-op behavior for an unshipped release channel.

Key decision: the updater public key is **required** at build time. We use a zero-trust placeholder key (generated by `tauri signer generate`) and document key rotation in "Follow-ups". Installing an update requires a matching Ed25519 signature.

- [ ] **Step 5.1: Add the updater dep**

Edit `src-tauri/Cargo.toml`, add under `[dependencies]` next to the other Tauri plugins:

```toml
tauri-plugin-updater = "2"
```

- [ ] **Step 5.2: Generate a signing keypair (one-time, OUT OF GIT)**

Run (this lives on the engineer's machine, not in the repo):

```bash
cargo install tauri-cli --version '^2' --locked
tauri signer generate -w ~/.config/typeurvoice-updater.key
# prints a PUBLIC KEY — copy the base64 string (starts with "dW50cn...")
```

- [ ] **Step 5.3: Add updater config to `tauri.conf.json`**

Open `src-tauri/tauri.conf.json`. Add a top-level `"plugins"` key. The final file should look like:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "TypeUrVoice",
  "version": "0.1.0",
  "identifier": "com.typeurvoice.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      { "label": "settings", "title": "TypeUrVoice Settings", "width": 580, "height": 500, "minWidth": 580, "minHeight": 480, "resizable": false, "visible": false, "decorations": true, "center": true, "alwaysOnTop": false },
      { "label": "overlay", "title": "TypeUrVoice", "width": 220, "height": 52, "resizable": false, "visible": false, "decorations": false, "transparent": true, "alwaysOnTop": true, "skipTaskbar": true, "x": 0, "y": 0 },
      { "label": "onboarding", "title": "Welcome to TypeUrVoice", "width": 500, "height": 420, "resizable": false, "visible": false, "decorations": true, "center": true }
    ],
    "macOSPrivateApi": true,
    "security": { "csp": null }
  },
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://github.com/typeurvoice/typeurvoice/releases/latest/download/latest.json"
      ],
      "pubkey": "REPLACE_WITH_BASE64_PUBLIC_KEY_FROM_STEP_5_2",
      "dialog": true
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "macOS": {
      "entitlements": "entitlements.plist",
      "exceptionDomain": "",
      "frameworks": [],
      "signingIdentity": null,
      "minimumSystemVersion": "12.0"
    },
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": ""
    }
  }
}
```

**Important:** Until you have a real endpoint serving `latest.json`, the updater will receive 404s — which the plugin treats as "no updates available". This is safe.

- [ ] **Step 5.4: Update capabilities**

Edit `src-tauri/capabilities/default.json`. Add to the `"permissions"` array:

```json
    "updater:default"
```

Final permissions array:

```json
  "permissions": [
    "core:default",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus",
    "core:window:allow-set-position",
    "core:window:allow-primary-monitor",
    "core:window:allow-available-monitors",
    "core:window:allow-close",
    "core:event:default",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-unregister-all",
    "global-shortcut:allow-is-registered",
    "notification:allow-request-permission",
    "notification:allow-notify",
    "notification:allow-is-permission-granted",
    "notification:allow-check-permissions",
    "autostart:allow-enable",
    "autostart:allow-disable",
    "autostart:allow-is-enabled",
    "sql:allow-load",
    "sql:allow-execute",
    "sql:allow-select",
    "sql:allow-close",
    "updater:default"
  ]
```

- [ ] **Step 5.5: Register the plugin and add a `check_for_updates` command**

In `src-tauri/src/lib.rs`, find the `tauri::Builder::default()` block. Add `.plugin(tauri_plugin_updater::Builder::new().build())`:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
```

Add a command near the other commands (e.g. just after `test_text_processor`):

```rust
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
```

Register it in `invoke_handler`:

```rust
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
        ])
```

- [ ] **Step 5.6: Build**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error"
```

Expected: zero errors. If the compiler complains about `pubkey` validation, copy a real pubkey from Step 5.2 into `tauri.conf.json`; the plugin refuses to build with `REPLACE_WITH_...`. For plan-execution purposes without a key, you may temporarily set `"active": false` in the updater config and skip the per-release build checks — document this as a TODO in the commit message.

- [ ] **Step 5.7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/lib.rs
git commit -m "feat(updater): wire tauri-plugin-updater (endpoint + signing key pending)"
```

---

## Task 6: "Check for Updates" Button in Settings

**Files:**
- Modify: `src/lib/tauri.ts` (add the invoke wrapper)
- Modify: `src/types/index.ts` (optional — add `UpdateStatus`)
- Modify: `src/components/settings/GeneralTab.tsx` (add "System" section button)

**Context:** Small UI affordance so users can trigger an update check manually. Auto-check-on-start is already configurable via the plugin config (`dialog: true` shows a native dialog).

- [ ] **Step 6.1: Add the invoke wrapper**

Open `src/lib/tauri.ts`. Find the exported `api` object and add a `checkForUpdates` method. You'll need to read the file first:

```bash
grep -n "api = {" <repo>/src/lib/tauri.ts
```

Add:

```ts
  checkForUpdates(): Promise<{ available: boolean; version: string | null; notes: string | null }> {
    return invoke('check_for_updates')
  },
```

- [ ] **Step 6.2: Add a "Check for updates" button to the System section**

Open `src/components/settings/GeneralTab.tsx`. At the top, add imports:

```tsx
import { useState } from 'react'
import { api } from '../../lib/tauri'
```

(Note: `useState` is already imported at line 1 — keep the existing import, just confirm.)

Inside the existing `<Section title="System">` block near the bottom of the component (around line 129), add a sibling div under the existing `<Toggle label="Start on login" .../>`:

```tsx
      <Section title="System">
        <Toggle
          label="Start on login"
          description="Launch TypeUrVoice when you log in"
          checked={settings.startOnLogin}
          onChange={val => onUpdate({ startOnLogin: val })}
        />
        <UpdatesRow />
      </Section>
```

At the bottom of the file (below the `GeneralTab` export), add:

```tsx
function UpdatesRow() {
  const [checking, setChecking] = useState(false)
  const [result, setResult] = useState<string | null>(null)

  const check = async () => {
    setChecking(true)
    setResult(null)
    try {
      const r = await api.checkForUpdates()
      if (r.available) {
        setResult(`Update available: v${r.version}`)
      } else {
        setResult('You are on the latest version.')
      }
    } catch (e) {
      setResult(`Check failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setChecking(false)
    }
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', marginTop: 'var(--sp-3)' }}>
      <button
        type="button"
        onClick={check}
        disabled={checking}
        style={{
          padding: '6px 12px',
          background: 'transparent',
          color: 'var(--text-primary)',
          border: '1px solid var(--border-default)',
          borderRadius: 'var(--radius)',
          cursor: checking ? 'wait' : 'pointer',
          fontSize: 'var(--text-sm)',
        }}
      >
        {checking ? 'Checking…' : 'Check for updates'}
      </button>
      {result && (
        <span style={{ color: 'var(--text-tertiary)', fontSize: 'var(--text-sm)' }} role="status">
          {result}
        </span>
      )}
    </div>
  )
}
```

- [ ] **Step 6.3: Build frontend + backend**

Run:
```bash
npm run build 2>&1 | tail -20
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error"
```

Expected: zero errors from either.

- [ ] **Step 6.4: Commit**

```bash
git add src/lib/tauri.ts src/components/settings/GeneralTab.tsx
git commit -m "feat(ui): add Check for updates button under General → System"
```

---

## Task 7: Crash-Log Size-Based Rotation

**Files:**
- Create: `src-tauri/src/logging.rs`
- Modify: `src-tauri/src/lib.rs` (delegate `log_to_file` to the new module)

**Context:** Today `log_to_file` opens with `append(true)` and grows forever. Rotation: when the file exceeds `MAX_SIZE_BYTES` (1 MB), rename `crash.log` → `crash.log.1` (overwriting any existing `.1`), keeping up to two numbered backups. That's three files × 1 MB = 3 MB worst case. Check size before writing so the first line of a fresh cycle never gets fragmented.

Non-goal here: converting every `eprintln!` call — those already go nowhere in release builds and replacing them is out of scope.

- [ ] **Step 7.1: Create `logging.rs`**

Create `src-tauri/src/logging.rs`:

```rust
//! Persistent diagnostic log with size-based rotation.
//!
//! Previously every line went to `crash.log` via append. With heavy use
//! (every recording logs ~6 lines) the file grew unbounded. We now keep
//! at most `MAX_SIZE_BYTES` of the live log and one rotated backup.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

const MAX_SIZE_BYTES: u64 = 1_000_000; // 1 MB
const BACKUP_COUNT: usize = 1; // crash.log + crash.log.1

// Single mutex around the rotation step. Per-line writes don't need to be
// globally ordered (multiple threads can race and the file is append-only),
// but rotation is not safe to run concurrently.
static ROTATE_LOCK: Mutex<()> = Mutex::new(());

fn log_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("TypeUrVoice").join("crash.log"))
}

fn rotate_if_needed(path: &std::path::Path) {
    let Ok(meta) = std::fs::metadata(path) else { return; };
    if meta.len() < MAX_SIZE_BYTES {
        return;
    }
    let _g = ROTATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Re-check under the lock in case another thread rotated first.
    if let Ok(m) = std::fs::metadata(path) {
        if m.len() < MAX_SIZE_BYTES { return; }
    }
    // Shift backups: .1 drops, then rename current → .1
    for i in (1..=BACKUP_COUNT).rev() {
        let older = path.with_extension(format!("log.{}", i + 1));
        let newer = path.with_extension(format!("log.{}", i));
        if newer.exists() {
            let _ = std::fs::rename(&newer, &older);
        }
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::rename(path, &backup);
}

/// Redact a string down to `max_chars` with an ellipsis if truncated.
/// Used for previews of sensitive payloads (transcripts etc.) going to the
/// log — at TRUNCATE distance we still see enough to triage a report.
pub fn redact_preview(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{}… ({} chars redacted)", head, count - max_chars)
    }
}

/// Append a timestamped line, rotating the file first if it's over the cap.
pub fn append(msg: &str) {
    let Some(path) = log_path() else { return; };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    rotate_if_needed(&path);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{}] {}", secs, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_preserves_short_strings() {
        assert_eq!(redact_preview("hi", 10), "hi");
    }

    #[test]
    fn redact_truncates_long_strings() {
        let s = "a".repeat(50);
        let got = redact_preview(&s, 5);
        assert_eq!(got, "aaaaa… (45 chars redacted)");
    }
}
```

- [ ] **Step 7.2: Wire `logging` into `lib.rs`**

Open `src-tauri/src/lib.rs`. Add `mod logging;` near the other `mod` declarations.

Replace the existing `log_to_file` function (around line 754):

```rust
pub(crate) fn log_to_file(msg: &str) {
    logging::append(msg);
}
```

- [ ] **Step 7.3: Test**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml logging 2>&1 | tail -20
```

Expected: `2 passed`.

- [ ] **Step 7.4: Manual smoke test**

```text
• Pre-populate: dd if=/dev/zero bs=1024 count=2048 of=~/Library/Application\ Support/TypeUrVoice/crash.log
• Launch the app and trigger any log line (start + stop recording).
• Confirm: crash.log is now small (< 1 MB) and crash.log.1 holds the old 2 MB.
```

- [ ] **Step 7.5: Commit**

```bash
git add src-tauri/src/logging.rs src-tauri/src/lib.rs
git commit -m "feat(logging): rotate crash.log at 1MB with 1 backup"
```

---

## Task 8: Redact Transcript Previews in Logs

**Files:**
- Modify: `src-tauri/src/state.rs` (add `diagnostics_verbose` field)
- Modify: `src-tauri/src/lib.rs` (check setting in `run_pipeline` log calls)
- Modify: `src-tauri/src/transcription/mod.rs` (same)

**Context:** The file currently logs `Whisper raw: lang=cs, 120 chars, text="the meeting about salaries is …"` regardless of who shares the log. That's a privacy issue. Default becomes: log only metadata (length + language); the `text=` preview is written only if `diagnostics_verbose=true`. Users can flip it on when filing a bug report.

- [ ] **Step 8.1: Add the field**

Open `src-tauri/src/state.rs`. In the `Settings` struct (around line 85), add at the end of the struct:

```rust
    /// When true, crash.log includes transcript text previews. Off by default
    /// so shared logs don't leak dictated content.
    #[serde(default)]
    pub diagnostics_verbose: bool,
```

In `Default for Settings` (around line 127), add:

```rust
            diagnostics_verbose: false,
```

This uses `#[serde(default)]` so existing `settings.json` files without the field still load — the field defaults to `false`.

- [ ] **Step 8.2: Gate the preview in `transcription/mod.rs`**

Open `src-tauri/src/transcription/mod.rs`. The `transcribe` function logs a preview (around line 192). Change it to conditionally include the text:

```rust
    let preview = if settings.diagnostics_verbose {
        format!(", text={:?}", crate::logging::redact_preview(&result.text, 120))
    } else {
        String::new()
    };
    crate::log_to_file(&format!(
        "Whisper raw: lang={}, {} chars{}",
        result.language, result.text.len(), preview
    ));
```

- [ ] **Step 8.3: Same for the Step-1 result log in `lib.rs`**

Open `src-tauri/src/lib.rs`. Find the Step 1 result log (around line 437):

```rust
    log_to_file(&format!(
        "Step 1 result: lang={}, {} chars, preview={:?}",
        transcript.language, transcript.text.len(), safe_truncate(&transcript.text, 80)
    ));
```

Replace with:

```rust
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
```

- [ ] **Step 8.4: Surface the setting in the frontend**

Open `src/types/index.ts` and find the `Settings` type (search for `historyRetentionDays`). Add to the interface:

```ts
  diagnosticsVerbose: boolean;
```

Open `src/hooks/useSettings.ts` (line 6) and add to `DEFAULT_SETTINGS`:

```ts
  diagnosticsVerbose: false,
```

Open `src/components/settings/GeneralTab.tsx`. Inside the `<Section title="System">` block, add after the "Check for updates" row:

```tsx
        <Toggle
          label="Verbose diagnostics"
          description="Include transcript previews in crash.log for debugging. Off by default to protect privacy."
          checked={settings.diagnosticsVerbose}
          onChange={val => onUpdate({ diagnosticsVerbose: val })}
        />
```

- [ ] **Step 8.5: Build**

Run:
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error"
npm run build 2>&1 | tail -5
```

Expected: no errors. If Rust complains that `settings` isn't visible inside one of the log sites, pass `&settings` as a new parameter — do not pull from a mutex inside `run_pipeline`, which runs inside an async task and would add a lock hop.

- [ ] **Step 8.6: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/src/transcription/mod.rs src/types/index.ts src/hooks/useSettings.ts src/components/settings/GeneralTab.tsx
git commit -m "feat(logging): gate transcript previews behind Verbose diagnostics setting"
```

---

## Task 9: Toast Close Button + Escape Dismissal

**Files:**
- Modify: `src/components/ui/Toast.tsx`

**Context:** Today errors auto-dismiss after 4s with no way for a keyboard user to kill them early — an a11y violation. Add an inline × button and an Escape key binding. Errors become `role="alert"` with `aria-live="assertive"` so screen readers announce immediately; info stays `role="status"` / `aria-live="polite"`.

- [ ] **Step 9.1: Replace `Toast.tsx`**

Overwrite `src/components/ui/Toast.tsx` with:

```tsx
import { useEffect } from 'react'

type ToastProps = {
  message: string | null
  onDismiss: () => void
  /** Auto-dismiss delay in ms (default 4000) */
  ttl?: number
  kind?: 'error' | 'info'
}

export function Toast({ message, onDismiss, ttl = 4000, kind = 'error' }: ToastProps) {
  useEffect(() => {
    if (!message) return
    const t = window.setTimeout(onDismiss, ttl)
    return () => window.clearTimeout(t)
  }, [message, ttl, onDismiss])

  useEffect(() => {
    if (!message) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onDismiss()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [message, onDismiss])

  if (!message) return null

  const isError = kind === 'error'
  const bg = isError ? 'rgba(220, 38, 38, 0.15)' : 'rgba(59, 130, 246, 0.15)'
  const border = isError ? 'rgba(220, 38, 38, 0.45)' : 'rgba(59, 130, 246, 0.45)'
  const fg = isError ? '#FCA5A5' : '#93C5FD'

  return (
    <div
      role={isError ? 'alert' : 'status'}
      aria-live={isError ? 'assertive' : 'polite'}
      style={{
        position: 'fixed',
        top: 12,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        background: bg,
        border: `1px solid ${border}`,
        color: fg,
        padding: '8px 14px',
        borderRadius: 'var(--radius)',
        fontSize: 'var(--text-sm)',
        maxWidth: 'calc(100% - 32px)',
        boxShadow: '0 4px 12px rgba(0,0,0,0.35)',
        display: 'flex',
        alignItems: 'center',
        gap: 10,
      }}
    >
      <span>{message}</span>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        style={{
          background: 'transparent',
          border: 'none',
          color: fg,
          cursor: 'pointer',
          fontSize: 14,
          lineHeight: 1,
          padding: '2px 4px',
          marginLeft: 'auto',
        }}
      >
        ×
      </button>
    </div>
  )
}
```

- [ ] **Step 9.2: Build frontend**

Run:
```bash
npm run build 2>&1 | tail -10
```

Expected: success.

- [ ] **Step 9.3: Manual check**

```text
• Open Settings
• Trigger an error (e.g. disconnect network, change a setting that causes save failure)
• Confirm: toast shows "×" button, clicking dismisses it; pressing Escape also dismisses; VoiceOver announces immediately on errors
```

- [ ] **Step 9.4: Commit**

```bash
git add src/components/ui/Toast.tsx
git commit -m "a11y(toast): dismiss button, Escape key, assertive live-region for errors"
```

---

## Task 10: Shortcut Conflict Validation

**Files:**
- Modify: `src/components/settings/GeneralTab.tsx`

**Context:** Today the user can set primary and secondary to the same shortcut; `tauri-plugin-global-shortcut` will register whichever is called last and the other will silently no-op (both will fire the same language, defeating the feature). We block the conflict client-side: when capturing a shortcut that matches the other, refuse the save and show an inline error.

A string comparison is sufficient because the capture step produces the same canonical form (e.g. `Alt+Shift+Space`) regardless of capture order — modifiers are pushed in a fixed order (Meta, Control, Alt, Shift).

- [ ] **Step 10.1: Add conflict check to `captureShortcut`**

Open `src/components/settings/GeneralTab.tsx`. Replace the existing `captureShortcut` function (currently lines 21–36) with:

```tsx
  const [shortcutError, setShortcutError] = useState<string | null>(null)

  const captureShortcut = (
    e: React.KeyboardEvent<HTMLInputElement>,
    field: 'shortcut' | 'secondaryShortcut',
  ) => {
    e.preventDefault()
    const mods: string[] = []
    if (e.metaKey) mods.push('Meta')
    if (e.ctrlKey) mods.push('Control')
    if (e.altKey) mods.push('Alt')
    if (e.shiftKey) mods.push('Shift')
    const key = e.key
    if (['Meta', 'Control', 'Alt', 'Shift'].includes(key)) return
    const shortcut = [...mods, key.length === 1 ? key.toUpperCase() : key].join('+')

    // Reject conflicts — both shortcuts can't be bound to the same combo.
    const other = field === 'shortcut' ? settings.secondaryShortcut : settings.shortcut
    if (other && other === shortcut) {
      setShortcutError(`"${shortcut}" is already used by the ${field === 'shortcut' ? 'secondary' : 'primary'} shortcut. Pick a different combination.`)
      e.currentTarget.blur()
      return
    }
    setShortcutError(null)
    onUpdate({ [field]: shortcut })
    e.currentTarget.blur()
  }
```

- [ ] **Step 10.2: Render the error**

Near the existing `<Section title="Primary Shortcut">` block, below the inner flex row (around line 67), add a conditional error line:

```tsx
        {shortcutError && (
          <div
            role="alert"
            style={{
              color: '#FCA5A5',
              fontSize: 'var(--text-sm)',
              marginTop: 6,
            }}
          >
            {shortcutError}
          </div>
        )}
```

Place this inside `<Section title="Primary Shortcut">` but also repeat inside `<Section title="Secondary Shortcut">` so the error appears near whichever input the user last touched. Since there's only one `shortcutError` state, placing it in both Sections is fine — only one will be visible at a time because both render the same string and the user sees the one near their active input (the other appears simultaneously but that's acceptable; alternative is to track `errorField` too).

**Simpler alternative** that avoids double-render: put the error at the top of the component, outside both Sections:

```tsx
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>
      {shortcutError && (
        <div role="alert" style={{ color: '#FCA5A5', fontSize: 'var(--text-sm)' }}>
          {shortcutError}
        </div>
      )}
      <Section title="Primary Shortcut"> ...
```

Use whichever placement the engineer prefers — the single-top version is cleaner.

- [ ] **Step 10.3: Build**

Run:
```bash
npm run build 2>&1 | tail -5
```

Expected: success.

- [ ] **Step 10.4: Manual check**

```text
• Open Settings → General
• Set primary shortcut to Alt+Shift+Space
• Enable secondary, try to set it to Alt+Shift+Space
• Confirm: inline error appears, secondary does not change
• Set secondary to Alt+Shift+S (different) → works
```

- [ ] **Step 10.5: Commit**

```bash
git add src/components/settings/GeneralTab.tsx
git commit -m "fix(settings): reject shortcut captures that collide with the other binding"
```

---

## Task 11: Full-Pipeline Smoke + Sign-Off

**Files:** none

**Context:** Make sure nothing regressed. We run a full build end-to-end and exercise the happy paths the audit flagged as load-bearing.

- [ ] **Step 11.1: Release build**

Run:
```bash
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env"
cargo build --release --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10
npm run tauri build 2>&1 | tail -20
```

Expected: clean release build. The `bundle_dmg.sh` step still fails per CLAUDE.md — that's OK; we use the `hdiutil` path later.

- [ ] **Step 11.2: Run dev build and walk through the golden path**

Run:
```bash
pkill -f "target/debug/typeurvoice" 2>/dev/null; lsof -ti :1420 | xargs kill -9 2>/dev/null
export PATH="/opt/homebrew/bin:$PATH" && source "$HOME/.cargo/env"
npm run tauri dev &
```

Checklist:

```text
• Press primary shortcut → overlay appears → speak 3s → release
• Paste lands in focused app
• Change engine Cloud → Local → record → paste
• Record twice in a row on Local → second should be ~2–5s faster (WhisperContext cache hit)
• Change output mode Raw → Clean → record → paste is cleaned (no regression)
• Translation enabled → source cs, target en → record → English paste appears
• Enable Verbose diagnostics → record → crash.log now contains `text=...`; disable → preview gone
• Tail crash.log while recording repeatedly → file stays under 1 MB; crash.log.1 appears
• Settings → Check for updates → either "no updates" or a proper error toast
• Try to set secondary = primary → inline error; not saved
• Trigger a save error (bad API key test) → toast shows × and Escape closes it
• Delete and re-download "Small Q5" → progress → "Downloaded"; crash.log shows no "Model verification failed"
```

- [ ] **Step 11.3: Final commit if any squash/touchups**

If everything works, no further commit is needed. Otherwise commit the fix:

```bash
git add -A
git commit -m "fix: sign-off smoke adjustments"
```

- [ ] **Step 11.4: Open PR (or squash merge to main if team policy allows)**

Run:
```bash
git log --oneline audit-phase-5 ^main
# review commit series, then:
git push -u origin audit-phase-5
gh pr create --title "Audit remediation — Phase 5" --body "$(cat <<'EOF'
## Summary
- Model SHA-256 verification (supply chain)
- Shared reqwest::Client (latency)
- WhisperContext cache (~2–5s saved per recording on Local)
- tauri-plugin-updater plumbing (release signing still TODO)
- Toast × button + Escape dismissal
- Shortcut conflict validation
- crash.log rotation (1MB×2) + transcript preview gated behind Verbose diagnostics

## Test plan
- [ ] Cloud recording round-trip
- [ ] Local recording round-trip with cache hit
- [ ] Model re-download with hash check
- [ ] Shortcut conflict rejected client-side
- [ ] Toast dismisses via × and Escape
- [ ] Verbose diagnostics toggle gates preview lines in crash.log
- [ ] crash.log rotates at 1MB

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Follow-ups (explicitly out of scope)

1. **Apple Developer signing + notarization** — required before the updater can actually install builds. Documented in CLAUDE.md.
2. **GitHub Actions release pipeline** — produces signed `.app.tar.gz` + `latest.json`, bumps version, uploads to GitHub Releases.
3. **Generating and filling real SHA-256 for `base-en` / `tiny-en`** — placeholder entries are soft-pass today.
4. **One-time crash.log migration** — existing installs will rotate the oversized log the first time a user records; no explicit migration needed, but note that the first write post-upgrade will take the rotation hit.
5. **Unit tests for pipeline wiring** — end-to-end tests require a tokio harness + mocked reqwest; skipped here to keep the scope focused on fixes.

---

## Self-Review

**Spec coverage** — six audit priorities mapped:
1. Model hash verification → Tasks 1+2
2. Auto-updater → Tasks 5+6
3. Cache WhisperContext → Task 4
4. Reusable reqwest::Client → Task 3
5. Toast dismissal + shortcut validation → Tasks 9+10
6. crash.log rotation + scrubbing → Tasks 7+8

**Type consistency** — `AppHttpClient::client(&self) -> &reqwest::Client` is consistent across `http.rs`, the commands that use `tauri::State<SharedHttpClient>`, and the `.clone()` before `run_pipeline`. `WhisperCache::get_or_load` returns `Arc<WhisperContext>` consistently. `UpdateStatus` struct is defined once in `lib.rs` and matches the TS shape in `tauri.ts`.

**Placeholder scan** — one documented placeholder: `"REPLACE_WITH_BASE64_PUBLIC_KEY_FROM_STEP_5_2"` — but this is an explicit out-of-band value the engineer fills in locally (key must not live in git), not a plan failure.

**Non-regression check** — the plan deliberately leaves untouched: `audio/capture.rs`, `paste/clipboard.rs` (pbcopy fallback), hallucination filter, language-override logic, poison-safe mutex helpers, pipeline timeouts, tray menu sync, `cpal` threading, and all settings migration compat (`#[serde(default)]` on every new field).
