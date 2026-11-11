use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub raw_text: String,
    pub processed_text: String,
    pub mode: String,
    pub language: Option<String>,
    pub translated_text: Option<String>,
    pub summary: Option<String>,
    pub duration_secs: Option<f64>,
    pub created_at: String,
    /// The user fixed this entry's output in History (see `save_correction`).
    pub corrected: bool,
}

/// A fix the user made to an output in History. `kind` is "translation"
/// (source = cleaned transcript, output = translation) or "cleanup"
/// (source = raw transcript, output = cleaned text).
#[derive(Debug, Clone)]
pub struct Correction {
    pub kind: String,
    pub source_text: String,
    pub output_text: String,
    pub corrected_text: String,
}

fn db_path() -> Result<PathBuf> {
    Ok(crate::storage::settings::typeurvoice_data_dir()?.join("history.db"))
}

use super::restrict_permissions;

pub fn open() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(&path)?;
    let _ = restrict_permissions(&path);
    init_db(&conn)?;
    Ok(conn)
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS transcriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            raw_text TEXT NOT NULL,
            processed_text TEXT NOT NULL,
            mode TEXT NOT NULL,
            language TEXT,
            translated_text TEXT,
            summary TEXT,
            duration_secs REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS corrections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            history_id INTEGER,
            kind TEXT NOT NULL,
            language TEXT,
            source_text TEXT NOT NULL,
            output_text TEXT NOT NULL,
            corrected_text TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );"
    )?;
    Ok(())
}

pub fn insert(
    conn: &Connection,
    raw_text: &str,
    processed_text: &str,
    mode: &str,
    language: Option<&str>,
    translated_text: Option<&str>,
    summary: Option<&str>,
    duration_secs: Option<f64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO transcriptions (raw_text, processed_text, mode, language, translated_text, summary, duration_secs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![raw_text, processed_text, mode, language, translated_text, summary, duration_secs],
    )?;
    Ok(conn.last_insert_rowid())
}

const ENTRY_COLUMNS: &str =
    "t.id, t.raw_text, t.processed_text, t.mode, t.language, t.translated_text, t.summary,
     t.duration_secs, t.created_at,
     EXISTS(SELECT 1 FROM corrections c WHERE c.history_id = t.id)";

fn entry_from_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        raw_text: row.get(1)?,
        processed_text: row.get(2)?,
        mode: row.get(3)?,
        language: row.get(4)?,
        translated_text: row.get(5)?,
        summary: row.get(6)?,
        duration_secs: row.get(7)?,
        created_at: row.get(8)?,
        corrected: row.get(9)?,
    })
}

pub fn get_recent(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM transcriptions t ORDER BY t.created_at DESC, t.id DESC LIMIT ?1 OFFSET ?2"
    ))?;
    let entries = stmt
        .query_map(params![limit, offset], entry_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<HistoryEntry>> {
    let mut stmt = conn.prepare(&format!("SELECT {ENTRY_COLUMNS} FROM transcriptions t WHERE t.id = ?1"))?;
    Ok(stmt.query_row(params![id], entry_from_row).optional()?)
}

/// Entries after `after_id`, oldest first (for profile learning).
pub fn entries_after(conn: &Connection, after_id: i64, limit: u32) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM transcriptions t WHERE t.id > ?1 ORDER BY t.id DESC LIMIT ?2"
    ))?;
    let mut entries = stmt
        .query_map(params![after_id, limit], entry_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    entries.reverse();
    Ok(entries)
}

pub fn count_after(conn: &Connection, after_id: i64) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM transcriptions WHERE id > ?1", params![after_id], |r| r.get(0))?)
}

/// Replace an entry's output with the user's fix and remember the fix so
/// future cleanups/translations learn from it. Returns the updated entry.
pub fn save_correction(conn: &Connection, id: i64, corrected: &str) -> Result<HistoryEntry> {
    let entry = get(conn, id)?.ok_or_else(|| anyhow::anyhow!("History entry {} not found", id))?;
    let corrected = corrected.trim();
    if corrected.is_empty() {
        return Err(anyhow::anyhow!("The corrected text is empty"));
    }
    let translated = entry
        .translated_text
        .as_deref()
        .filter(|t| !t.trim().is_empty() && *t != entry.processed_text);
    let (kind, source, output) = match translated {
        Some(t) => ("translation", entry.processed_text.as_str(), t),
        None => ("cleanup", entry.raw_text.as_str(), entry.processed_text.as_str()),
    };
    if corrected == output.trim() {
        return Ok(entry); // nothing changed
    }
    // One correction per entry: a second edit replaces the first.
    conn.execute("DELETE FROM corrections WHERE history_id = ?1", params![id])?;
    conn.execute(
        "INSERT INTO corrections (history_id, kind, language, source_text, output_text, corrected_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, kind, entry.language, source, output, corrected],
    )?;
    if kind == "translation" {
        conn.execute("UPDATE transcriptions SET translated_text = ?1 WHERE id = ?2", params![corrected, id])?;
    } else {
        conn.execute("UPDATE transcriptions SET processed_text = ?1 WHERE id = ?2", params![corrected, id])?;
    }
    get(conn, id)?.ok_or_else(|| anyhow::anyhow!("History entry {} vanished", id))
}

pub fn recent_corrections(conn: &Connection, limit: u32) -> Result<Vec<Correction>> {
    let mut stmt = conn.prepare(
        "SELECT kind, source_text, output_text, corrected_text
         FROM corrections ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(Correction {
                kind: row.get(0)?,
                source_text: row.get(1)?,
                output_text: row.get(2)?,
                corrected_text: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The user's corrections most similar to `text` (shared words), best
/// first, at most `n`. Falls back to the most recent ones when nothing
/// overlaps, so a fresh correction still teaches tone and terms.
pub fn relevant_corrections(conn: &Connection, kind: &str, text: &str, n: usize) -> Result<Vec<Correction>> {
    let all: Vec<Correction> = recent_corrections(conn, 200)?
        .into_iter()
        .filter(|c| c.kind == kind)
        .collect();
    Ok(pick_relevant(all, text, n))
}

fn words(s: &str) -> std::collections::HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(|w| w.to_lowercase())
        .collect()
}

fn pick_relevant(all: Vec<Correction>, text: &str, n: usize) -> Vec<Correction> {
    let target = words(text);
    let mut scored: Vec<(usize, usize, Correction)> = all
        .into_iter()
        .enumerate()
        .map(|(recency, c)| (words(&c.source_text).intersection(&target).count(), recency, c))
        .collect();
    // Most shared words first; ties → most recent (lower index = newer).
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(n).map(|(_, _, c)| c).collect()
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM transcriptions WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM corrections WHERE history_id = ?1", params![id])?;
    Ok(())
}

/// Clears transcripts AND the corrections learned from them — "Clear"
/// means forget what I said.
pub fn clear_all(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM transcriptions", [])?;
    conn.execute("DELETE FROM corrections", [])?;
    Ok(())
}

/// Retention only expires transcripts; corrections are kept (they're small
/// and are what the user explicitly taught the app).
pub fn cleanup_old(conn: &Connection, retention_days: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM transcriptions WHERE created_at < datetime('now', ?1)",
        params![format!("-{} days", retention_days)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    #[test]
    fn translation_correction_updates_entry_and_is_remembered() {
        let conn = mem();
        let id = insert(&conn, "raw", "Nebylo by lepší to udělat jinak?", "clean", Some("cs"),
            Some("It wouldn't be better to do it differently."), None, None).unwrap();
        let e = save_correction(&conn, id, "Wouldn't it be better to do it differently?").unwrap();
        assert!(e.corrected);
        assert_eq!(e.translated_text.as_deref(), Some("Wouldn't it be better to do it differently?"));
        let c = relevant_corrections(&conn, "translation", "nebylo by lepší", 3).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].output_text, "It wouldn't be better to do it differently.");
    }

    #[test]
    fn cleanup_correction_when_no_translation() {
        let conn = mem();
        let id = insert(&conn, "je to v pozádku", "Je to v pozádku.", "clean", Some("cs"), None, None, None).unwrap();
        save_correction(&conn, id, "Je to v pořádku.").unwrap();
        let c = relevant_corrections(&conn, "cleanup", "pozádku", 3).unwrap();
        assert_eq!(c[0].source_text, "je to v pozádku");
        assert!(relevant_corrections(&conn, "translation", "x", 3).unwrap().is_empty());
    }

    #[test]
    fn relevance_prefers_shared_words() {
        let mk = |s: &str| Correction {
            kind: "translation".into(),
            source_text: s.into(), output_text: String::new(), corrected_text: String::new(),
        };
        let picked = pick_relevant(
            vec![mk("úplně jiná věta"), mk("kočka pije ze studny"), mk("další věta")],
            "kočka stojí u studny", 1,
        );
        assert_eq!(picked[0].source_text, "kočka pije ze studny");
    }
}
