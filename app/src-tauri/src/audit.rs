//! Read-only access to `%APPDATA%\kairos\audit.db`, which the Kairos Hermes plugin writes
//! (D-015, M2 spec §7). The app never writes to it.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;

use crate::store::StoreError;

/// The newest audit schema this app understands.
const SUPPORTED_SCHEMA: i32 = 1;

/// One tool call as the plugin recorded it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditEvent {
    /// Unix seconds.
    pub at: f64,
    pub tool_call_id: Option<String>,
    pub tool_name: String,
    /// Redacted by the plugin before it was written.
    pub args_redacted: String,
    /// allow | deny | ask | inherit | invalid_rules
    pub decision: String,
    pub rule_id: Option<String>,
    /// ok | error | blocked, or None while the call is still running.
    pub status: Option<String>,
    pub error_type: Option<String>,
    pub duration_ms: Option<i64>,
    /// The choice made on an approval for this call, if there was one: once, session, always, deny…
    pub approval_choice: Option<String>,
    /// "aux_llm" when Hermes's smart mode decided instead of a person.
    pub approval_decided_by: Option<String>,
}

pub fn default_path() -> Result<PathBuf, StoreError> {
    crate::store::default_path().map(|db| db.with_file_name("audit.db"))
}

/// A conversation's audited tool calls, oldest first. A missing `audit.db` (the plugin isn't
/// installed, or hasn't recorded anything yet) is an empty list, not an error.
pub fn events_for_session(path: &Path, session_id: &str) -> Result<Vec<AuditEvent>, StoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SUPPORTED_SCHEMA {
        return Err(StoreError::new(format!(
            "audit.db is schema {version}; this Kairos reads up to {SUPPORTED_SCHEMA}. Update Kairos."
        )));
    }
    if version == 0 {
        return Ok(Vec::new()); // created but not initialised yet
    }
    let mut statement = conn.prepare(
        "SELECT e.at, e.tool_call_id, e.tool_name, e.args_redacted, e.decision, e.rule_id,
                e.status, e.error_type, e.duration_ms,
                (SELECT a.choice FROM approval a WHERE a.tool_call_id = e.tool_call_id
                 ORDER BY a.id DESC LIMIT 1),
                (SELECT a.decided_by FROM approval a WHERE a.tool_call_id = e.tool_call_id
                 ORDER BY a.id DESC LIMIT 1)
         FROM tool_event e WHERE e.session_id = ?1 ORDER BY e.at, e.id",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok(AuditEvent {
            at: row.get(0)?,
            tool_call_id: row.get(1)?,
            tool_name: row.get(2)?,
            args_redacted: row.get(3)?,
            decision: row.get(4)?,
            rule_id: row.get(5)?,
            status: row.get(6)?,
            error_type: row.get(7)?,
            duration_ms: row.get(8)?,
            approval_choice: row.get(9)?,
            approval_decided_by: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plugin's schema (plugin/kairos/audit.py), so the reader is tested against what the
    /// writer creates.
    const PLUGIN_SCHEMA: &str = "
        CREATE TABLE tool_event (
          id INTEGER PRIMARY KEY, at REAL NOT NULL, session_id TEXT, tool_call_id TEXT,
          tool_name TEXT NOT NULL, args_redacted TEXT NOT NULL, decision TEXT NOT NULL,
          rule_id TEXT, status TEXT, duration_ms INTEGER, result_preview TEXT, error_type TEXT);
        CREATE TABLE approval (
          id INTEGER PRIMARY KEY, at REAL NOT NULL, session_key TEXT, session_id TEXT,
          tool_call_id TEXT, command TEXT, description TEXT, pattern_key TEXT,
          choice TEXT NOT NULL, decided_by TEXT);
        PRAGMA user_version = 1;";

    fn temp_db(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("kairos-audit-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("dir");
        dir.join("audit.db")
    }

    fn seeded(name: &str) -> PathBuf {
        let path = temp_db(name);
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(PLUGIN_SCHEMA).expect("schema");
        conn.execute_batch(
            "INSERT INTO tool_event (at, session_id, tool_call_id, tool_name, args_redacted, decision,
                                     rule_id, status, duration_ms, error_type)
             VALUES (1.0, 'kairos-s1', 'call_a', 'read_file', '{}', 'inherit', NULL, 'ok', 12, NULL),
                    (2.0, 'kairos-s1', 'call_b', 'read_file', '{}', 'deny', 'no-credentials', 'blocked', 0, 'plugin_block'),
                    (3.0, 'kairos-s1', 'call_c', 'terminal', '{}', 'ask', 'destructive-commands', 'error', 1702, 'tool_error'),
                    (4.0, 'kairos-s2', 'call_d', 'terminal', '{}', 'inherit', NULL, 'ok', 5, NULL);
             INSERT INTO approval (at, session_id, tool_call_id, choice, decided_by)
             VALUES (3.5, 'kairos-s1', 'call_c', 'deny', NULL);",
        )
        .expect("rows");
        path
    }

    #[test]
    fn a_sessions_calls_come_back_in_order_with_their_decisions_and_approvals() {
        let path = seeded("read");
        let events = events_for_session(&path, "kairos-s1").expect("read");
        let summary: Vec<(&str, &str, Option<&str>, Option<&str>)> = events
            .iter()
            .map(|e| {
                (
                    e.tool_call_id.as_deref().unwrap_or(""),
                    e.decision.as_str(),
                    e.rule_id.as_deref(),
                    e.approval_choice.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("call_a", "inherit", None, None),
                ("call_b", "deny", Some("no-credentials"), None),
                ("call_c", "ask", Some("destructive-commands"), Some("deny")),
            ]
        );
    }

    #[test]
    fn no_audit_file_means_no_events() {
        assert_eq!(
            events_for_session(&temp_db("missing"), "kairos-s1").expect("read"),
            Vec::new()
        );
    }

    #[test]
    fn a_newer_audit_schema_is_reported_not_misread() {
        let path = seeded("newer");
        Connection::open(&path)
            .expect("open")
            .execute_batch("PRAGMA user_version = 2;")
            .expect("bump");
        let err = events_for_session(&path, "kairos-s1").expect_err("refused");
        assert!(err.to_string().contains("Update Kairos"));
    }

    #[test]
    fn the_reader_cannot_write() {
        let path = seeded("readonly");
        let conn =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open");
        assert!(conn.execute("DELETE FROM tool_event", []).is_err());
    }
}
