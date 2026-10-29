//! Key/value settings in SQLite for the island's modules (ported from Elissa, M15): the wallpaper
//! folder, the pins. `%APPDATA%\kairos\island.db`, apart from kairos.db. Values are JSON. Secrets do
//! NOT belong here.
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{path::Path, sync::Mutex};

pub struct Settings {
    conn: Mutex<Connection>,
}

impl Settings {
    pub fn open(path: &Path) -> Result<Self, String> {
        Self::from_conn(Connection::open(path).map_err(|e| e.to_string())?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, String> {
        Self::from_conn(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn from_conn(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS settings (
                 key   TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Run `f` on the settings database for a module that keeps its own table (e.g. `pins`).
    /// Secrets still do not belong here.
    pub fn with_conn<R>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&mut conn)
    }

    pub fn get(&self, key: &str) -> Result<Option<Value>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        raw.map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .transpose()
    }

    pub fn set(&self, key: &str, value: &Value) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(test)]
    pub fn get_string_or(&self, key: &str, default: &str) -> String {
        match self.get(key) {
            Ok(Some(Value::String(s))) => s,
            _ => default.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_key_is_none() {
        assert_eq!(Settings::in_memory().unwrap().get("nope").unwrap(), None);
    }

    #[test]
    fn set_then_get_round_trips_and_overwrites() {
        let s = Settings::in_memory().unwrap();
        s.set("k", &json!({"a": [1, 2]})).unwrap();
        assert_eq!(s.get("k").unwrap(), Some(json!({"a": [1, 2]})));
        s.set("k", &json!("v2")).unwrap();
        assert_eq!(s.get("k").unwrap(), Some(json!("v2")));
    }

    #[test]
    fn string_default_applies_for_missing_or_wrong_type() {
        let s = Settings::in_memory().unwrap();
        assert_eq!(s.get_string_or("h", "d"), "d");
        s.set("h", &json!(5)).unwrap();
        assert_eq!(s.get_string_or("h", "d"), "d");
        s.set("h", &json!("x")).unwrap();
        assert_eq!(s.get_string_or("h", "d"), "x");
    }
}
