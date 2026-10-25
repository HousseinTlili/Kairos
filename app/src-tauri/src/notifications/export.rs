//! `notifications.json` (M4 §6, D-018 proposed): the last day of notifications, written by the app
//! for the plugin's `recent_notifications` tool. The app is its only writer; the plugin only reads.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::store::StoredNotification;
use crate::store::{StoreError, TaskStore};

pub const FILE_NAME: &str = "notifications.json";
pub const WINDOW_SECS: i64 = 24 * 60 * 60;
pub const MAX_ENTRIES: u32 = 200;

#[derive(Debug, Serialize)]
struct ExportEntry<'a> {
    app: &'a str,
    title: &'a str,
    body: &'a str,
    at: i64,
    unread: bool,
}

#[derive(Debug, Serialize)]
struct Export<'a> {
    version: u32,
    written_at: i64,
    notifications: Vec<ExportEntry<'a>>,
}

/// The file's text for these notifications (newest first, as the store returns them).
pub fn render(notifications: &[StoredNotification], now: i64) -> String {
    let export = Export {
        version: 1,
        written_at: now,
        notifications: notifications
            .iter()
            .map(|n| ExportEntry {
                app: &n.app_name,
                title: &n.title,
                body: &n.body,
                at: n.created_at,
                unread: n.read_at.is_none(),
            })
            .collect(),
    };
    // Serialising plain strings and numbers can't fail; an empty list is the safe fallback.
    serde_json::to_string_pretty(&export).unwrap_or_else(|_| {
        format!("{{\"version\": 1, \"written_at\": {now}, \"notifications\": []}}")
    })
}

/// `%APPDATA%\kairos\notifications.json`, next to `kairos.db`.
pub fn default_path() -> Result<PathBuf, StoreError> {
    crate::store::default_path().map(|db| db.with_file_name(FILE_NAME))
}

/// Brings the file in line with the store: rewritten while collection is on, deleted when it is
/// off or nothing is stored, so the plugin never reads notifications the user turned off.
pub fn sync(store: &TaskStore, path: &Path, enabled: bool, now: i64) -> Result<(), StoreError> {
    let recent = if enabled {
        store.notifications_since(now - WINDOW_SECS, MAX_ENTRIES)?
    } else {
        Vec::new()
    };
    if recent.is_empty() && (!enabled || store.notification_counts()?.is_empty()) {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(StoreError::new(format!(
                "couldn't remove {}: {err}",
                path.display()
            ))),
        };
    }
    crate::rules::write_atomically(path, &render(&recent, now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::store::NewNotification;

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kairos-export-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn add(store: &TaskStore, id: u32, at: i64, title: &str) {
        store
            .insert_notification(&NewNotification {
                app_id: "Example.Chat!App".into(),
                app_name: "Example Chat".into(),
                windows_id: id,
                created_at: at,
                title: title.into(),
                body: "Example message".into(),
            })
            .expect("insert");
    }

    #[test]
    fn the_file_holds_the_last_day_newest_first_with_unread_flags() {
        let store = TaskStore::open_in_memory().expect("store");
        let now = 1_000_000;
        add(&store, 1, now - WINDOW_SECS - 10, "too old");
        add(&store, 2, now - 600, "older");
        add(&store, 3, now - 60, "newer");
        store
            .mark_notifications_read(None, None, now)
            .expect("read all");
        add(&store, 4, now - 30, "newest");
        let dir = temp_dir("window");
        let path = dir.join(FILE_NAME);
        sync(&store, &path, true, now).expect("sync");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let titles: Vec<&str> = json["notifications"]
            .as_array()
            .expect("list")
            .iter()
            .map(|n| n["title"].as_str().expect("title"))
            .collect();
        assert_eq!(titles, ["newest", "newer", "older"]);
        assert_eq!(json["notifications"][0]["unread"], true);
        assert_eq!(json["notifications"][1]["unread"], false);
        assert_eq!(json["version"], 1);
        assert_eq!(json["notifications"][0]["app"], "Example Chat");
    }

    #[test]
    fn switching_collection_off_removes_the_file() {
        let store = TaskStore::open_in_memory().expect("store");
        add(&store, 1, 100, "x");
        let dir = temp_dir("off");
        let path = dir.join(FILE_NAME);
        sync(&store, &path, true, 200).expect("on");
        assert!(path.exists());
        sync(&store, &path, false, 200).expect("off");
        assert!(!path.exists());
        sync(&store, &path, false, 200).expect("off again is fine");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn clearing_everything_removes_the_file() {
        let store = TaskStore::open_in_memory().expect("store");
        add(&store, 1, 100, "x");
        let dir = temp_dir("clear");
        let path = dir.join(FILE_NAME);
        sync(&store, &path, true, 200).expect("on");
        store.clear_notifications(None).expect("clear");
        sync(&store, &path, true, 200).expect("after clear");
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn the_export_is_capped() {
        let store = TaskStore::open_in_memory().expect("store");
        for i in 0..(MAX_ENTRIES + 5) {
            add(&store, i, 1_000 + i64::from(i), "x");
        }
        let rows = store.notifications_since(0, MAX_ENTRIES).expect("rows");
        let json: serde_json::Value = serde_json::from_str(&render(&rows, 5_000)).expect("json");
        assert_eq!(
            json["notifications"].as_array().expect("list").len(),
            MAX_ENTRIES as usize
        );
    }
}
