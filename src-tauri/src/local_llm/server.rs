//! Lifecycle of the bundled `llama-server` child process.
//!
//! - `ensure_running` starts (or reuses) a server for a model and returns its
//!   OpenAI-compatible base URL. Concurrent callers (hotkey warm-up + the
//!   pipeline) are serialised by `load_lock`, so the second caller simply
//!   waits for the first load instead of spawning twice.
//! - `stop` / `stop_now` kill it. `stop_now` is lock-free so it can run from
//!   the synchronous `RunEvent::Exit` handler even mid-load.
//! - A PID file lets the next launch clean up an orphan left by a crash.
//! - `run_idle_watcher` unloads the model after N idle minutes when the user
//!   has "load automatically and unload when idle" on.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

use super::models;

const READY_TIMEOUT: Duration = Duration::from_secs(90);
const CONTEXT_TOKENS: &str = "4096";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    Unloaded,
    Loading { model: String },
    Ready { model: String },
    Error { message: String },
}

struct Running {
    child: Child,
    model_key: String,
    port: u16,
}

pub struct LlmServer {
    /// Serialises start/stop so two callers never spawn two servers.
    load_lock: tokio::sync::Mutex<()>,
    proc: Mutex<Option<Running>>,
    status: Mutex<Status>,
    last_used: Mutex<Instant>,
    /// (model key, file mtime) whose SHA-256 has been verified this run.
    verified: Mutex<Vec<(String, Option<SystemTime>)>>,
}

impl Default for LlmServer {
    fn default() -> Self {
        Self {
            load_lock: tokio::sync::Mutex::new(()),
            proc: Mutex::new(None),
            status: Mutex::new(Status::Unloaded),
            last_used: Mutex::new(Instant::now()),
            verified: Mutex::new(Vec::new()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl LlmServer {
    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }

    fn set_status<R: tauri::Runtime>(&self, app: &AppHandle<R>, status: Status) {
        *lock(&self.status) = status.clone();
        let _ = app.emit("builtin-llm-status", &status);
    }

    /// Mark the server as just used (resets the idle timer).
    pub fn touch(&self) {
        *lock(&self.last_used) = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        lock(&self.last_used).elapsed()
    }

    /// True if a server process is alive (any model).
    pub fn is_running(&self) -> bool {
        let mut guard = lock(&self.proc);
        match guard.as_mut() {
            Some(r) => matches!(r.child.try_wait(), Ok(None)),
            None => false,
        }
    }

    /// Start the server for `model_key` if it isn't already serving it, and
    /// return the base URL (e.g. `http://127.0.0.1:52811`).
    pub async fn ensure_running<R: tauri::Runtime>(&self, app: &AppHandle<R>, model_key: &str) -> Result<String> {
        self.touch();
        let _guard = self.load_lock.lock().await;

        if let Some(url) = self.running_url_for(model_key) {
            return Ok(url);
        }
        self.kill_current();

        let result = self.start(app, model_key).await;
        match &result {
            Ok(_) => self.set_status(app, Status::Ready { model: model_key.to_string() }),
            Err(e) => {
                self.kill_current();
                crate::log_to_file(&format!("Built-in LLM failed to start: {}", e));
                self.set_status(app, Status::Error { message: e.to_string() });
            }
        }
        self.touch();
        result
    }

    fn running_url_for(&self, model_key: &str) -> Option<String> {
        let mut guard = lock(&self.proc);
        let r = guard.as_mut()?;
        let alive = matches!(r.child.try_wait(), Ok(None));
        (alive && r.model_key == model_key).then(|| format!("http://127.0.0.1:{}", r.port))
    }

    async fn start<R: tauri::Runtime>(&self, app: &AppHandle<R>, model_key: &str) -> Result<String> {
        let model_path = models::path(model_key)?;
        if !model_path.exists() {
            return Err(anyhow!(
                "Built-in model '{}' is not downloaded. Download it in Settings → Engine.",
                model_key
            ));
        }
        self.set_status(app, Status::Loading { model: model_key.to_string() });
        self.verify_once(model_key, &model_path).await?;

        let binary = sidecar_path()?;
        let port = free_port()?;
        let log_path = crate::storage::settings::typeurvoice_data_dir()?.join("llama-server.log");
        let log = std::fs::File::create(&log_path)?;

        let started = Instant::now();
        let child = Command::new(&binary)
            .arg("-m").arg(&model_path)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .args(["-c", CONTEXT_TOKENS, "-ngl", "99", "-np", "1", "--no-webui"])
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("Failed to launch {}", binary.display()))?;
        write_pid_file(child.id());
        *lock(&self.proc) = Some(Running { child, model_key: model_key.to_string(), port });

        let base = format!("http://127.0.0.1:{}", port);
        let health = format!("{}/health", base);
        let http = reqwest::Client::new();
        loop {
            if started.elapsed() > READY_TIMEOUT {
                return Err(anyhow!("Built-in model took longer than {}s to load", READY_TIMEOUT.as_secs()));
            }
            if !self.is_running() {
                return Err(anyhow!(
                    "Built-in model server exited while loading{}. Your Mac may not have enough free memory for this model.",
                    log_tail(&log_path).map(|t| format!(" ({})", t)).unwrap_or_default()
                ));
            }
            // /health returns 503 while the model loads, 200 once ready.
            if let Ok(resp) = http.get(&health).timeout(Duration::from_secs(2)).send().await {
                if resp.status().is_success() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        crate::log_to_file(&format!(
            "Built-in LLM ready: {} on port {} in {}ms",
            model_key, port, started.elapsed().as_millis()
        ));
        Ok(base)
    }

    /// SHA-256 the model once per (key, mtime) per app run — same policy as
    /// the Whisper cache. A tampered GGUF is handed to native tensor code.
    async fn verify_once(&self, model_key: &str, path: &std::path::Path) -> Result<()> {
        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        let entry = (model_key.to_string(), mtime);
        if lock(&self.verified).contains(&entry) {
            return Ok(());
        }
        let expected = models::lookup(model_key)
            .ok_or_else(|| anyhow!("Unknown built-in model: {}", model_key))?
            .hash;
        let p = path.to_path_buf();
        let actual = tokio::task::spawn_blocking(move || crate::transcription::model_hashes::hash_file(&p))
            .await
            .map_err(|e| anyhow!("Hash verification task failed: {}", e))??;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(anyhow!(
                "Model verification failed for {}. Delete it in Settings → Engine and download it again.",
                model_key
            ));
        }
        lock(&self.verified).push(entry);
        Ok(())
    }

    /// Kill the current child (if any) and wait for it. Caller holds `load_lock`
    /// or is shutting down.
    fn kill_current(&self) {
        if let Some(mut r) = lock(&self.proc).take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
            remove_pid_file();
        }
    }

    /// Unload the model and emit `Unloaded`.
    pub async fn stop<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        let _guard = self.load_lock.lock().await;
        let was_running = lock(&self.proc).is_some();
        self.kill_current();
        if was_running {
            crate::log_to_file("Built-in LLM unloaded");
        }
        self.set_status(app, Status::Unloaded);
    }

    /// Synchronous, lock-free kill for app exit (may run mid-load).
    pub fn stop_now(&self) {
        self.kill_current();
    }
}

/// `llama-server` lives next to the main executable in the bundle
/// (Contents/MacOS/llama-server — Tauri strips the target-triple suffix from
/// `externalBin`, and also copies it into target/<profile>/ for `tauri dev`).
fn sidecar_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| anyhow!("No executable directory"))?;
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    let candidate = dir.join(name);
    if candidate.exists() {
        return Ok(candidate);
    }
    // Plain `cargo build` / `cargo test` don't copy sidecars — fall back to
    // the build script's output in debug builds and tests only.
    #[cfg(any(debug_assertions, test))]
    {
        let triple = env!("TYPEURVOICE_TARGET_TRIPLE");
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("llama-server-{}{}", triple, if cfg!(windows) { ".exe" } else { "" }));
        if dev.exists() {
            return Ok(dev);
        }
    }
    Err(anyhow!(
        "Built-in model server is missing from the app bundle ({}). Reinstall TypeUrVoice.",
        candidate.display()
    ))
}

fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn log_tail(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    Some(crate::logging::redact_preview(line.trim(), 160))
}

fn pid_file() -> Option<PathBuf> {
    crate::storage::settings::typeurvoice_data_dir().ok().map(|d| d.join("llama-server.pid"))
}

fn write_pid_file(pid: u32) {
    if let Some(p) = pid_file() {
        let _ = std::fs::write(p, pid.to_string());
    }
}

fn remove_pid_file() {
    if let Some(p) = pid_file() {
        let _ = std::fs::remove_file(p);
    }
}

/// Kill a `llama-server` left running by a previous TypeUrVoice that crashed.
/// Only kills the PID if it still belongs to a process named llama-server,
/// so a recycled PID can never take out an unrelated process.
pub fn kill_orphan() {
    let Some(path) = pid_file() else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    let Ok(pid) = text.trim().parse::<u32>() else { return };
    #[cfg(unix)]
    {
        let comm = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output();
        if let Ok(out) = comm {
            if String::from_utf8_lossy(&out.stdout).trim().ends_with("llama-server") {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                crate::log_to_file(&format!("Killed orphaned llama-server (pid {})", pid));
            }
        }
    }
}

/// Background loop: unload after the configured idle time when auto mode is on.
pub async fn run_idle_watcher(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(20)).await;
        let (auto, idle_minutes) = {
            let settings = app.state::<crate::state::SharedSettings>();
            let s = settings.lock().unwrap_or_else(|e| e.into_inner());
            (s.builtin_llm_auto, s.builtin_llm_idle_minutes)
        };
        if !auto || idle_minutes == 0 {
            continue;
        }
        let server = app.state::<std::sync::Arc<LlmServer>>().inner().clone();
        if server.is_running() && server.idle_for() >= Duration::from_secs(idle_minutes as u64 * 60) {
            crate::log_to_file(&format!("Built-in LLM idle for {} min — unloading", idle_minutes));
            server.stop(&app).await;
        }
    }
}

/// End-to-end check against the real sidecar + a downloaded model:
/// `cargo test --lib builtin_llm_lifecycle -- --ignored --nocapture`
/// (needs scripts/build-llama-server.sh output and qwen3.5-2b downloaded).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn builtin_llm_lifecycle() {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let server = LlmServer::default();
        tauri::async_runtime::block_on(async {
            let t = Instant::now();
            let base = server.ensure_running(&handle, models::DEFAULT_MODEL).await.expect("start");
            println!("cold start: {:?} → {}", t.elapsed(), base);
            assert_eq!(server.status(), Status::Ready { model: models::DEFAULT_MODEL.into() });

            // Second call reuses the same process.
            let t = Instant::now();
            assert_eq!(server.ensure_running(&handle, models::DEFAULT_MODEL).await.unwrap(), base);
            assert!(t.elapsed() < Duration::from_millis(100), "reuse took {:?}", t.elapsed());

            // A real cleanup request through the production request builder.
            let cfg = crate::processing::test_config(&format!("{}/v1/chat/completions", base));
            let t = Instant::now();
            let out = crate::processing::providers::complete(
                &reqwest::Client::new(), &cfg,
                "Fix punctuation and capitalization. Output only the corrected text. The text is in Czech; output MUST be in Czech.",
                "ehm takže zítra v deset máme poradu a pak oběd",
            ).await.expect("completion");
            println!("cleanup: {:?} → {:?}", t.elapsed(), out);
            assert!(!out.is_empty());

            let pid = lock(&server.proc).as_ref().unwrap().child.id();
            server.stop(&handle).await;
            assert_eq!(server.status(), Status::Unloaded);
            assert!(!server.is_running());
            let alive = Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
            assert!(!alive, "llama-server pid {} still alive after stop", pid);
        });
    }
}
