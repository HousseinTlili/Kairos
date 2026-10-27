//! What Kairos remembers (M11, D-026): Hermes's built-in memory files and the holographic
//! provider's fact store, read-only. Kairos never edits them; forgetting is a request to Kairos.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

/// Hermes separates entries in USER.md and MEMORY.md with this (tools/memory_tool_store.py).
pub const ENTRY_DELIMITER: &str = "\n§\n";

/// Hermes's defaults for the two files (memory.user_char_limit, memory.memory_char_limit).
pub const USER_LIMIT: usize = 1375;
pub const NOTES_LIMIT: usize = 2200;

/// More than this is a list nobody reads on one page; the newest come first.
const FACT_LIMIT: usize = 500;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("couldn't read Hermes's memory: {0}")]
    Io(#[from] std::io::Error),
    #[error("couldn't read the fact store: {0}")]
    Facts(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryFile {
    pub entries: Vec<String>,
    /// Characters used, counted the way Hermes counts them (entries joined by the delimiter).
    pub used: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fact {
    pub id: i64,
    pub content: String,
    pub category: String,
    /// 0–1: how much the provider trusts it.
    pub trust: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryState {
    pub user: MemoryFile,
    pub notes: MemoryFile,
    /// `None` when there is no fact store (no provider, or nothing stored yet).
    pub facts: Option<Vec<Fact>>,
    /// Hermes's `memory.provider`; empty = built-in only.
    pub provider: Option<String>,
}

pub fn split_entries(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split(ENTRY_DELIMITER)
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn read_file(path: &Path, limit: usize) -> Result<MemoryFile, MemoryError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let entries = split_entries(&text);
    let used = entries.join(ENTRY_DELIMITER).chars().count();
    Ok(MemoryFile {
        entries,
        used,
        limit,
    })
}

/// The newest facts, opened read-only so Kairos can never change or lock the provider's file
/// for writing.
pub fn read_facts(db: &Path) -> Result<Option<Vec<Fact>>, MemoryError> {
    if !db.is_file() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    let mut statement = conn.prepare(
        "SELECT fact_id, content, COALESCE(category, 'general'), COALESCE(trust_score, 0.5), \
         COALESCE(updated_at, created_at, '') FROM facts ORDER BY updated_at DESC, fact_id DESC LIMIT ?1",
    )?;
    let facts = statement
        .query_map([FACT_LIMIT as i64], |row| {
            Ok(Fact {
                id: row.get(0)?,
                content: row.get(1)?,
                category: row.get(2)?,
                trust: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(facts))
}

pub fn read_all(hermes_home: &Path, provider: Option<String>) -> Result<MemoryState, MemoryError> {
    let memories = hermes_home.join("memories");
    Ok(MemoryState {
        user: read_file(&memories.join("USER.md"), USER_LIMIT)?,
        notes: read_file(&memories.join("MEMORY.md"), NOTES_LIMIT)?,
        facts: read_facts(&hermes_home.join("memory_store.db"))?,
        provider: provider.filter(|p| !p.trim().is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kairos-memory-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("memories")).unwrap();
        dir
    }

    #[test]
    fn entries_are_split_on_hermess_delimiter() {
        let text = "Prefers short answers.\n§\nWorks on Kairos.\r\n§\r\n\n§\n";
        assert_eq!(
            split_entries(text),
            vec!["Prefers short answers.", "Works on Kairos."]
        );
        assert!(split_entries("").is_empty());
    }

    #[test]
    fn missing_files_and_a_missing_store_are_simply_empty() {
        let home = temp_dir("empty");
        let state = read_all(&home, Some(String::new())).unwrap();
        assert!(state.user.entries.is_empty());
        assert_eq!(state.user.used, 0);
        assert_eq!(state.notes.limit, NOTES_LIMIT);
        assert_eq!(state.facts, None);
        assert_eq!(state.provider, None);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn use_is_counted_as_hermes_counts_it() {
        let home = temp_dir("count");
        std::fs::write(home.join("memories").join("USER.md"), "abc\n§\nde").unwrap();
        let user = read_file(&home.join("memories").join("USER.md"), USER_LIMIT).unwrap();
        assert_eq!(user.entries, vec!["abc", "de"]);
        assert_eq!(user.used, "abc\n§\nde".chars().count());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn facts_are_read_newest_first_without_writing() {
        let home = temp_dir("facts");
        let db = home.join("memory_store.db");
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE facts (fact_id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL UNIQUE, \
                 category TEXT DEFAULT 'general', tags TEXT DEFAULT '', trust_score REAL DEFAULT 0.5, \
                 retrieval_count INTEGER DEFAULT 0, helpful_count INTEGER DEFAULT 0, \
                 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                 hrr_vector BLOB);
                 INSERT INTO facts (content, category, updated_at) VALUES ('Likes tea', 'user_pref', '2026-10-26 10:00:00');
                 INSERT INTO facts (content, category, updated_at) VALUES ('Kairos is his project', 'project', '2026-10-27 09:00:00');",
            )
            .unwrap();
        }
        let modified = std::fs::metadata(&db).unwrap().modified().unwrap();
        let facts = read_facts(&db).unwrap().unwrap();
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].content, "Kairos is his project");
        assert_eq!(facts[1].category, "user_pref");
        assert_eq!(
            std::fs::metadata(&db).unwrap().modified().unwrap(),
            modified
        );
        let _ = std::fs::remove_dir_all(home);
    }
}
