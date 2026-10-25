//! The `notification` table of `kairos.db` (M4 §6). Kept until the user clears it (D-011).

use rusqlite::params;
use serde::Serialize;

use crate::store::{StoreError, TaskStore};

/// A notification as read from Windows, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NewNotification {
    pub app_id: String,
    pub app_name: String,
    pub windows_id: u32,
    /// Unix seconds, from Windows.
    pub created_at: i64,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoredNotification {
    pub id: i64,
    pub app_id: String,
    pub app_name: String,
    pub created_at: i64,
    pub title: String,
    pub body: String,
    pub read_at: Option<i64>,
}

/// One app's line in the panel's filter row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppCount {
    pub app_id: String,
    pub app_name: String,
    pub total: i64,
    pub unread: i64,
}

fn row_to_notification(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredNotification> {
    Ok(StoredNotification {
        id: row.get(0)?,
        app_id: row.get(1)?,
        app_name: row.get(2)?,
        created_at: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        read_at: row.get(6)?,
    })
}

const COLUMNS: &str = "id, app_id, app_name, created_at, title, body, read_at";

impl TaskStore {
    /// Stores a notification unless it is already there (the same app, Windows id and time:
    /// every 3 s poll sees it again while it stays in Action Center). True if it was new.
    #[allow(dead_code)] // Called by the Windows listener (Step 4, waiting for approval).
    pub fn insert_notification(&self, n: &NewNotification) -> Result<bool, StoreError> {
        let changed = self.conn().execute(
            "INSERT OR IGNORE INTO notification
               (app_id, app_name, windows_id, created_at, title, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                n.app_id,
                n.app_name,
                n.windows_id,
                n.created_at,
                n.title,
                n.body
            ],
        )?;
        Ok(changed == 1)
    }

    /// Newest first; one app's, or everyone's; optionally only unread.
    pub fn notifications(
        &self,
        app_id: Option<&str>,
        unread_only: bool,
        limit: u32,
    ) -> Result<Vec<StoredNotification>, StoreError> {
        let mut statement = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM notification
             WHERE (?1 IS NULL OR app_id = ?1) AND (?2 = 0 OR read_at IS NULL)
             ORDER BY created_at DESC, id DESC
             LIMIT ?3"
        ))?;
        let rows = statement.query_map(params![app_id, unread_only, limit], row_to_notification)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Total and unread per app, the apps with unread ones first, then by name. The latest
    /// display name wins when an app's name changed.
    pub fn notification_counts(&self) -> Result<Vec<AppCount>, StoreError> {
        let mut statement = self.conn().prepare(
            "SELECT app_id,
                    (SELECT app_name FROM notification n2 WHERE n2.app_id = n.app_id
                     ORDER BY created_at DESC, id DESC LIMIT 1),
                    COUNT(*),
                    SUM(read_at IS NULL)
             FROM notification n
             GROUP BY app_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AppCount {
                app_id: row.get(0)?,
                app_name: row.get(1)?,
                total: row.get(2)?,
                unread: row.get(3)?,
            })
        })?;
        let mut counts = rows.collect::<Result<Vec<_>, _>>()?;
        counts.sort_by(|a, b| {
            (b.unread > 0)
                .cmp(&(a.unread > 0))
                .then_with(|| a.app_name.to_lowercase().cmp(&b.app_name.to_lowercase()))
        });
        Ok(counts)
    }

    pub fn unread_notifications(&self) -> Result<i64, StoreError> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM notification WHERE read_at IS NULL",
            [],
            |row| row.get(0),
        )?)
    }

    /// Marks one notification, one app's, or all (both `None`) as read at `at`. Already-read ones
    /// keep their first read time. Returns how many changed.
    pub fn mark_notifications_read(
        &self,
        id: Option<i64>,
        app_id: Option<&str>,
        at: i64,
    ) -> Result<usize, StoreError> {
        Ok(self.conn().execute(
            "UPDATE notification SET read_at = ?1
             WHERE read_at IS NULL AND (?2 IS NULL OR id = ?2) AND (?3 IS NULL OR app_id = ?3)",
            params![at, id, app_id],
        )?)
    }

    /// Deletes one app's notifications, or all of them (`None`). Returns how many went.
    pub fn clear_notifications(&self, app_id: Option<&str>) -> Result<usize, StoreError> {
        Ok(self.conn().execute(
            "DELETE FROM notification WHERE ?1 IS NULL OR app_id = ?1",
            params![app_id],
        )?)
    }

    /// The newest notifications created at or after `since`, at most `limit`, for
    /// `notifications.json` (D-018).
    #[allow(dead_code)] // Used by the notifications.json writer (Step 6).
    pub fn notifications_since(
        &self,
        since: i64,
        limit: u32,
    ) -> Result<Vec<StoredNotification>, StoreError> {
        let mut statement = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM notification WHERE created_at >= ?1
             ORDER BY created_at DESC, id DESC LIMIT ?2"
        ))?;
        let rows = statement.query_map(params![since, limit], row_to_notification)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[cfg(test)]
    pub fn notification(&self, id: i64) -> Result<Option<StoredNotification>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {COLUMNS} FROM notification WHERE id = ?1"),
                params![id],
                row_to_notification,
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented apps and senders only (fixtures/README.md).
    fn note(app: &str, windows_id: u32, at: i64, title: &str) -> NewNotification {
        NewNotification {
            app_id: format!("{app}.App"),
            app_name: app.to_string(),
            windows_id,
            created_at: at,
            title: title.to_string(),
            body: "Example message".into(),
        }
    }

    fn store() -> TaskStore {
        TaskStore::open_in_memory().expect("store")
    }

    #[test]
    fn the_same_notification_seen_by_every_poll_is_stored_once() {
        let store = store();
        let n = note("Discord", 7, 100, "Alex");
        assert!(store.insert_notification(&n).expect("first"));
        assert!(!store.insert_notification(&n).expect("again"));
        assert_eq!(store.notifications(None, false, 50).expect("list").len(), 1);
    }

    #[test]
    fn windows_reusing_an_id_later_is_a_new_notification() {
        let store = store();
        store
            .insert_notification(&note("Discord", 7, 100, "Alex"))
            .expect("first");
        assert!(
            store
                .insert_notification(&note("Discord", 7, 900, "Sam"))
                .expect("reused id")
        );
    }

    #[test]
    fn lists_are_newest_first_and_filter_by_app_and_unread() {
        let store = store();
        store
            .insert_notification(&note("Discord", 1, 100, "a"))
            .expect("1");
        store
            .insert_notification(&note("Phone Link", 2, 300, "b"))
            .expect("2");
        store
            .insert_notification(&note("Discord", 3, 200, "c"))
            .expect("3");
        let all = store.notifications(None, false, 50).expect("all");
        assert_eq!(
            all.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        let discord = store
            .notifications(Some("Discord.App"), false, 50)
            .expect("discord");
        assert_eq!(discord.len(), 2);
        store
            .mark_notifications_read(Some(all[0].id), None, 400)
            .expect("read");
        let unread = store.notifications(None, true, 50).expect("unread");
        assert_eq!(
            unread.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
            ["c", "a"]
        );
        assert_eq!(store.notifications(None, false, 1).expect("limit").len(), 1);
    }

    #[test]
    fn counts_per_app_put_apps_with_unread_first() {
        let store = store();
        store
            .insert_notification(&note("Discord", 1, 100, "a"))
            .expect("1");
        store
            .insert_notification(&note("Discord", 2, 110, "b"))
            .expect("2");
        store
            .insert_notification(&note("Outlook", 3, 120, "c"))
            .expect("3");
        store
            .mark_notifications_read(None, Some("Discord.App"), 200)
            .expect("read");
        let counts = store.notification_counts().expect("counts");
        assert_eq!(counts[0].app_name, "Outlook");
        assert_eq!((counts[0].total, counts[0].unread), (1, 1));
        assert_eq!((counts[1].total, counts[1].unread), (2, 0));
        assert_eq!(store.unread_notifications().expect("unread"), 1);
    }

    #[test]
    fn marking_read_keeps_the_first_read_time_and_all_marks_everything() {
        let store = store();
        store
            .insert_notification(&note("Discord", 1, 100, "a"))
            .expect("1");
        store
            .insert_notification(&note("Outlook", 2, 100, "b"))
            .expect("2");
        let first = store.notifications(None, false, 50).expect("list")[0].id;
        assert_eq!(
            store
                .mark_notifications_read(Some(first), None, 10)
                .expect("one"),
            1
        );
        assert_eq!(
            store.mark_notifications_read(None, None, 20).expect("all"),
            1
        );
        assert_eq!(
            store
                .notification(first)
                .expect("get")
                .expect("kept")
                .read_at,
            Some(10)
        );
        assert_eq!(store.unread_notifications().expect("unread"), 0);
    }

    #[test]
    fn clearing_one_app_leaves_the_others_and_clear_all_empties_it() {
        let store = store();
        store
            .insert_notification(&note("Discord", 1, 100, "a"))
            .expect("1");
        store
            .insert_notification(&note("Outlook", 2, 100, "b"))
            .expect("2");
        assert_eq!(
            store
                .clear_notifications(Some("Discord.App"))
                .expect("clear one"),
            1
        );
        assert_eq!(store.notifications(None, false, 50).expect("left").len(), 1);
        assert_eq!(store.clear_notifications(None).expect("clear all"), 1);
        assert!(store.notification_counts().expect("counts").is_empty());
    }

    #[test]
    fn the_export_window_keeps_only_recent_ones_up_to_the_cap() {
        let store = store();
        for i in 0..5 {
            store
                .insert_notification(&note("Discord", i, 1000 + i64::from(i), "x"))
                .expect("insert");
        }
        store
            .insert_notification(&note("Discord", 99, 10, "old"))
            .expect("old");
        let recent = store.notifications_since(1000, 3).expect("since");
        assert_eq!(
            recent.iter().map(|n| n.created_at).collect::<Vec<_>>(),
            [1004, 1003, 1002]
        );
    }
}
