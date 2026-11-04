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
    dirs::data_dir().map(|d| d.join("VoiceInk").join("crash.log"))
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
