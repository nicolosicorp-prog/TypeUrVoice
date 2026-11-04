//! Streamed, hash-verified model downloads (Whisper GGML + built-in LLM GGUF).
//!
//! Bytes go to `<dest>.partial` and are only renamed into place after the
//! SHA-256 matches the pinned value, so a truncated or tampered file never
//! sits at the path the inference engines load from.

use anyhow::{Context, Result};
use std::path::Path;

/// Download `url` to `dest`, reporting progress in `0.0..=1.0`, then verify
/// against `expected_sha256` (lowercase hex; `None` ⇒ skip with a log line).
pub async fn download_verified(
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
    on_progress: impl Fn(f32) + Send + 'static,
) -> Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    // Dedicated client: multi-GB downloads would hog the shared pipeline pool
    // and need a far longer timeout than any API call.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?
        .error_for_status()
        .context("Download server returned an error")?;

    let total = response.content_length().unwrap_or(0);
    let partial = dest.with_extension("partial");
    let mut file = tokio::fs::File::create(&partial).await?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(b) => b,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&partial);
                return Err(anyhow::anyhow!("Download interrupted: {}", e));
            }
        };
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        if total > 0 {
            on_progress(downloaded as f32 / total as f32);
        }
    }
    file.flush().await?;
    drop(file); // close the handle before hashing

    let label = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(expected) = expected_sha256 {
        let partial_for_hash = partial.clone();
        let actual = tokio::task::spawn_blocking(move || {
            crate::transcription::model_hashes::hash_file(&partial_for_hash)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Hash verification task failed: {}", e))??;
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = std::fs::remove_file(&partial);
            return Err(anyhow::anyhow!(
                "Model verification failed for {}: expected {}, got {}\n\n\
                 The downloaded file was deleted. Please try downloading again.",
                label, expected, actual
            ));
        }
    } else {
        crate::log_to_file(&format!("download: no pinned hash for {} — skipping verification", label));
    }

    std::fs::rename(&partial, dest)?;
    Ok(())
}
