//! Shared `reqwest::Client` used by the transcription + LLM pipeline.
//!
//! `reqwest::Client` wraps an `Arc` internally, so we manage it directly via
//! `tauri::State` and let cheap clones distribute it across the 4–6 HTTPS
//! calls per recording. Per-call timeouts are set on each request builder
//! (60s in cloud.rs, 45s in processing/mod.rs).

pub fn build() -> reqwest::Client {
    reqwest::Client::builder()
        // Pool keepalive set roughly to OpenAI's 90s server-side idle.
        .pool_idle_timeout(std::time::Duration::from_secs(75))
        .pool_max_idle_per_host(4)
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build reqwest::Client — this should be infallible with default roots")
}
