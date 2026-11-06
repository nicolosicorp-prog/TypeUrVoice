use anyhow::Result;
use rusqlite::{Connection, params};
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

pub fn get_recent(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, raw_text, processed_text, mode, language, translated_text, summary, duration_secs, created_at
         FROM transcriptions ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
    )?;
    let entries = stmt
        .query_map(params![limit, offset], |row| {
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
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM transcriptions WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn clear_all(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM transcriptions", [])?;
    Ok(())
}

pub fn cleanup_old(conn: &Connection, retention_days: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM transcriptions WHERE created_at < datetime('now', ?1)",
        params![format!("-{} days", retention_days)],
    )?;
    Ok(())
}
