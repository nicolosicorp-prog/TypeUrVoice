//! Verifies a downloaded GGML model against the SHA-256 pinned in `models::MODELS`.
//!
//! Closes a supply-chain gap: whisper.cpp mmaps the downloaded bytes and
//! walks arbitrary tensor graphs, so a tampered model is effectively
//! arbitrary-code execution.
//!
//! When hashes drift upstream (the HF repo is occasionally re-uploaded),
//! users will see "Model verification failed" after download. The fix is
//! to update the `hash` field in `models::MODELS`.

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub fn expected_hash(model_key: &str) -> Option<&'static str> {
    super::models::lookup(model_key).and_then(|m| m.hash)
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
///   * Unknown model key → pass (key validation happens upstream).
pub fn verify(model_key: &str, path: &Path) -> Result<()> {
    let Some(expected) = expected_hash(model_key) else {
        crate::log_to_file(&format!(
            "model_hashes: no pinned hash for {:?} — skipping verification",
            model_key
        ));
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
        tmp.push(format!("typeurvoice_hash_known_{}.bin", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp).unwrap();
            f.write_all(b"hello").unwrap();
        }
        let got = hash_file(&tmp).unwrap();
        std::fs::remove_file(&tmp).ok();
        assert_eq!(got, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn unknown_model_key_passes() {
        let res = verify("__no_such_model__", std::path::Path::new("/does/not/matter"));
        assert!(res.is_ok());
    }

    #[test]
    fn verify_rejects_mismatch() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("typeurvoice_hash_mismatch_{}.bin", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp).unwrap();
            f.write_all(b"hello").unwrap();
        }
        let res = verify("small-q5", &tmp);
        std::fs::remove_file(&tmp).ok();
        let err = res.expect_err("verify must reject wrong bytes for a pinned model");
        let msg = err.to_string();
        assert!(msg.contains("Model verification failed"), "message missing signal: {}", msg);
        assert!(msg.contains("small-q5"), "message missing model key: {}", msg);
    }

    #[test]
    fn expected_hash_lookup() {
        assert!(expected_hash("small-q5").is_some());
        assert!(expected_hash("definitely-not-a-model").is_none());
    }
}
