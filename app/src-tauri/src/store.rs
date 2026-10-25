//! Local `kairos.db`: the `task` table (M1 §10, ARCHITECTURE.md §4), and from schema 2 the
//! `setting` and `presence_event` tables (M3 §8), and from schema 3 the `notification` table
//! (M4 §6, methods in `notifications/store.rs`), and from schema 4 each conversation's model and
//! each request's usage and estimated cost (M7 §5).
//!
//! A task row is written *before* Hermes is called and its ID is the run's Idempotency-Key, so a
//! retry after a crash or a network error can never start a second run (M1 §9 rule 1).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::hermes::events::RunEvent;
use crate::presence::{PresenceEvent, PresenceKind};

const SCHEMA_VERSION: i32 = 4;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("local task database: {0}")]
pub struct StoreError(String);

impl StoreError {
    pub fn new(message: impl Into<String>) -> Self {
        StoreError(message.into())
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        StoreError(err.to_string())
    }
}

/// M1 §10. `lost` means Hermes no longer knows the run; it is never resubmitted automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Running,
    WaitingForApproval,
    Stopping,
    Completed,
    Incomplete,
    Failed,
    Cancelled,
    Interrupted,
    Unknown,
    Lost,
}

impl TaskStatus {
    fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Created => "created",
            TaskStatus::Running => "running",
            TaskStatus::WaitingForApproval => "waiting_for_approval",
            TaskStatus::Stopping => "stopping",
            TaskStatus::Completed => "completed",
            TaskStatus::Incomplete => "incomplete",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Interrupted => "interrupted",
            TaskStatus::Unknown => "unknown",
            TaskStatus::Lost => "lost",
        }
    }

    /// A Hermes run status (`GET /v1/runs/{id}`, M1 §5) as a task status.
    pub fn from_hermes(status: &str) -> Option<Self> {
        Some(match status {
            "started" | "running" => TaskStatus::Running,
            "waiting_for_approval" => TaskStatus::WaitingForApproval,
            "stopping" => TaskStatus::Stopping,
            "completed" => TaskStatus::Completed,
            "cancelled" => TaskStatus::Cancelled,
            "failed" => TaskStatus::Failed,
            "interrupted" => TaskStatus::Interrupted,
            "unknown" => TaskStatus::Unknown,
            _ => return None,
        })
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "created" => TaskStatus::Created,
            "running" => TaskStatus::Running,
            "waiting_for_approval" => TaskStatus::WaitingForApproval,
            "stopping" => TaskStatus::Stopping,
            "completed" => TaskStatus::Completed,
            "incomplete" => TaskStatus::Incomplete,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            "interrupted" => TaskStatus::Interrupted,
            "unknown" => TaskStatus::Unknown,
            "lost" => TaskStatus::Lost,
            _ => return None,
        })
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed
                | TaskStatus::Incomplete
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Interrupted
                | TaskStatus::Lost
        )
    }

    /// The status a run event moves a task to, if it changes it.
    pub fn after(event: &RunEvent) -> Option<Self> {
        Some(match event {
            RunEvent::ApprovalRequest { .. } => TaskStatus::WaitingForApproval,
            RunEvent::ApprovalSettled { .. } => TaskStatus::Running,
            RunEvent::RunCompleted {
                completed, partial, ..
            } => {
                if *completed && !*partial {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Incomplete
                }
            }
            RunEvent::RunCancelled { .. } => TaskStatus::Cancelled,
            RunEvent::RunFailed { .. } => TaskStatus::Failed,
            RunEvent::RunInterrupted { .. } => TaskStatus::Interrupted,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Task {
    pub id: String,
    pub session_id: String,
    pub run_id: Option<String>,
    pub goal: String,
    pub status: TaskStatus,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub outcome: Option<String>,
    /// From `run.completed` (M7): tokens, the estimated cost (`None` = price unknown) and the
    /// provider and model that actually served the request.
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub served_provider: Option<String>,
    pub served_model: Option<String>,
}

/// A conversation's chosen model (M7 §4); both `None` means Hermes's default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, serde::Deserialize)]
pub struct ModelChoice {
    pub provider: Option<String>,
    pub model: Option<String>,
}

pub struct TaskStore {
    conn: Connection,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// `%APPDATA%\kairos\kairos.db` (M1 §3).
pub fn default_path() -> Result<PathBuf, StoreError> {
    std::env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .map(|base| PathBuf::from(base).join("kairos").join("kairos.db"))
        .ok_or_else(|| StoreError("APPDATA is not set".into()))
}

impl TaskStore {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| StoreError(err.to_string()))?;
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError(format!(
                "kairos.db is schema version {version}, newer than this Kairos ({SCHEMA_VERSION})"
            )));
        }
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE task (
                   id          TEXT PRIMARY KEY,
                   session_id  TEXT NOT NULL,
                   run_id      TEXT,
                   goal        TEXT NOT NULL,
                   status      TEXT NOT NULL,
                   created_at  INTEGER NOT NULL,
                   finished_at INTEGER,
                   outcome     TEXT
                 );
                 CREATE INDEX task_session ON task(session_id, created_at);
                 PRAGMA user_version = 1;",
            )?;
        }
        if version < 2 {
            conn.execute_batch(
                "CREATE TABLE presence_event (
                   id     INTEGER PRIMARY KEY,
                   at     INTEGER NOT NULL,
                   kind   TEXT NOT NULL,
                   detail TEXT
                 );
                 CREATE INDEX presence_event_at ON presence_event(at);
                 CREATE TABLE setting (
                   key   TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 PRAGMA user_version = 2;",
            )?;
        }
        if version < 3 {
            conn.execute_batch(
                "CREATE TABLE notification (
                   id         INTEGER PRIMARY KEY,
                   app_id     TEXT NOT NULL,
                   app_name   TEXT NOT NULL,
                   windows_id INTEGER NOT NULL,
                   created_at INTEGER NOT NULL,
                   title      TEXT NOT NULL,
                   body       TEXT NOT NULL,
                   read_at    INTEGER,
                   UNIQUE (app_id, windows_id, created_at)
                 );
                 CREATE INDEX notification_created ON notification(created_at);
                 PRAGMA user_version = 3;",
            )?;
        }
        if version < 4 {
            conn.execute_batch(
                "CREATE TABLE conversation (
                   session_id TEXT PRIMARY KEY,
                   provider   TEXT,
                   model      TEXT
                 );
                 ALTER TABLE task ADD COLUMN input_tokens INTEGER;
                 ALTER TABLE task ADD COLUMN output_tokens INTEGER;
                 ALTER TABLE task ADD COLUMN cache_read_tokens INTEGER;
                 ALTER TABLE task ADD COLUMN cost_usd REAL;
                 ALTER TABLE task ADD COLUMN served_provider TEXT;
                 ALTER TABLE task ADD COLUMN served_model TEXT;
                 PRAGMA user_version = 4;",
            )?;
        }
        Ok(Self { conn })
    }

    pub fn create(&self, id: &str, session_id: &str, goal: &str) -> Result<Task, StoreError> {
        let created_at = now();
        self.conn.execute(
            "INSERT INTO task (id, session_id, goal, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, session_id, goal, TaskStatus::Created.as_str(), created_at],
        )?;
        self.get(id)?
            .ok_or_else(|| StoreError("task vanished after insert".into()))
    }

    pub fn set_run(&self, id: &str, run_id: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE task SET run_id = ?2, status = ?3 WHERE id = ?1",
            params![id, run_id, TaskStatus::Running.as_str()],
        )?;
        Ok(())
    }

    /// Moves a task to `status`. A terminal status also records when it finished and the outcome
    /// (the final answer or the error). Never moves a task out of a terminal status.
    pub fn set_status(
        &self,
        id: &str,
        status: TaskStatus,
        outcome: Option<&str>,
    ) -> Result<(), StoreError> {
        let finished_at = status.is_terminal().then(now);
        self.conn.execute(
            "UPDATE task SET status = ?2,
               finished_at = COALESCE(?3, finished_at),
               outcome = COALESCE(?4, outcome)
             WHERE id = ?1
               AND status NOT IN ('completed', 'incomplete', 'failed', 'cancelled', 'interrupted', 'lost')",
            params![id, status.as_str(), finished_at, outcome],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Task>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome,
                    input_tokens, output_tokens, cache_read_tokens, cost_usd, served_provider, served_model
                 FROM task WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()?)
    }

    /// A conversation's tasks, oldest first.
    pub fn for_session(&self, session_id: &str) -> Result<Vec<Task>, StoreError> {
        self.query_tasks(
            "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome,
                    input_tokens, output_tokens, cache_read_tokens, cost_usd, served_provider, served_model
             FROM task WHERE session_id = ?1 ORDER BY created_at, rowid",
            params![session_id],
        )
    }

    /// Tasks that haven't reached a final status: candidates for reconnecting after a restart
    /// (M1 §9 rule 6).
    /// Requests that ended since `since` (completed, and failed or cut short), and those waiting
    /// for an approval now: the briefing's "while you were away" line (M5 §2.2).
    pub fn task_summary_since(&self, since: i64) -> Result<TaskSummary, StoreError> {
        let count = |sql: &str, with_since: bool| -> Result<i64, StoreError> {
            Ok(if with_since {
                self.conn.query_row(sql, params![since], |row| row.get(0))?
            } else {
                self.conn.query_row(sql, [], |row| row.get(0))?
            })
        };
        Ok(TaskSummary {
            finished: count(
                "SELECT COUNT(*) FROM task WHERE status = 'completed' AND finished_at >= ?1",
                true,
            )?,
            failed: count(
                "SELECT COUNT(*) FROM task
                 WHERE status IN ('failed', 'incomplete', 'interrupted', 'lost')
                   AND finished_at >= ?1",
                true,
            )?,
            waiting: count(
                "SELECT COUNT(*) FROM task WHERE status = 'waiting_for_approval'",
                false,
            )?,
        })
    }

    pub fn open_tasks(&self) -> Result<Vec<Task>, StoreError> {
        self.query_tasks(
            "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome,
                    input_tokens, output_tokens, cache_read_tokens, cost_usd, served_provider, served_model
             FROM task
             WHERE status NOT IN ('completed', 'incomplete', 'failed', 'cancelled', 'interrupted', 'lost')
             ORDER BY created_at, rowid",
            params![],
        )
    }

    /// Kairos conversations, most recently active first. The title is the first request.
    pub fn conversations(&self) -> Result<Vec<Conversation>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT session_id,
                    (SELECT goal FROM task t2 WHERE t2.session_id = t.session_id
                     ORDER BY created_at, rowid LIMIT 1),
                    MAX(created_at),
                    COUNT(*)
             FROM task t GROUP BY session_id ORDER BY MAX(created_at) DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Conversation {
                session_id: row.get(0)?,
                title: row.get(1)?,
                last_active: row.get(2)?,
                task_count: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn query_tasks(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<Task>, StoreError> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map(params, row_to_task)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

impl TaskStore {
    pub fn conversation_model(&self, session_id: &str) -> Result<ModelChoice, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT provider, model FROM conversation WHERE session_id = ?1",
                params![session_id],
                |row| {
                    Ok(ModelChoice {
                        provider: row.get(0)?,
                        model: row.get(1)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    /// Remembers a conversation's model; the default (both `None`) removes the row.
    pub fn set_conversation_model(
        &self,
        session_id: &str,
        choice: &ModelChoice,
    ) -> Result<(), StoreError> {
        if choice.provider.is_none() && choice.model.is_none() {
            self.conn.execute(
                "DELETE FROM conversation WHERE session_id = ?1",
                params![session_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO conversation (session_id, provider, model) VALUES (?1, ?2, ?3)
                 ON CONFLICT(session_id) DO UPDATE SET provider = excluded.provider,
                                                       model = excluded.model",
                params![session_id, choice.provider, choice.model],
            )?;
        }
        Ok(())
    }

    /// What `run.completed` reported for a task (M7 §5).
    pub fn record_usage(
        &self,
        task_id: &str,
        usage: &crate::hermes::events::Usage,
        cost_usd: Option<f64>,
        served: Option<&crate::hermes::events::Runtime>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE task SET input_tokens = ?2, output_tokens = ?3, cache_read_tokens = ?4,
                             cost_usd = ?5, served_provider = ?6, served_model = ?7
             WHERE id = ?1",
            params![
                task_id,
                usage.input_tokens as i64,
                usage.output_tokens as i64,
                usage.cache_read_tokens as i64,
                cost_usd,
                served
                    .map(|r| r.provider.as_str())
                    .filter(|s| !s.is_empty()),
                served.map(|r| r.model.as_str()).filter(|s| !s.is_empty()),
            ],
        )?;
        Ok(())
    }

    /// For the other tables' modules (`notifications/store.rs`); the schema stays here.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// A setting's stored value, or `None` when it was never set.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM setting WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn add_presence(&self, event: &PresenceEvent) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO presence_event (at, kind, detail) VALUES (?1, ?2, ?3)",
            params![event.at, event.kind.as_str(), event.detail],
        )?;
        Ok(())
    }

    /// The latest presence events, newest first.
    pub fn recent_presence(&self, limit: u32) -> Result<Vec<PresenceEvent>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT at, kind, detail FROM presence_event ORDER BY at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            let kind: String = row.get(1)?;
            Ok((
                row.get::<_, i64>(0)?,
                kind,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (at, kind, detail) = row?;
            // A kind from a newer Kairos is skipped rather than failing the list.
            if let Some(kind) = PresenceKind::parse(&kind) {
                events.push(PresenceEvent {
                    at,
                    kind,
                    detail,
                    away_s: None,
                });
            }
        }
        Ok(events)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TaskSummary {
    pub finished: i64,
    pub failed: i64,
    pub waiting: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Conversation {
    pub session_id: String,
    pub title: String,
    pub last_active: i64,
    pub task_count: i64,
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(4)?;
    Ok(Task {
        id: row.get(0)?,
        session_id: row.get(1)?,
        run_id: row.get(2)?,
        goal: row.get(3)?,
        // An unrecognised status (from a newer Kairos) reads as unknown rather than failing.
        status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Unknown),
        created_at: row.get(5)?,
        finished_at: row.get(6)?,
        outcome: row.get(7)?,
        input_tokens: row.get(8)?,
        output_tokens: row.get(9)?,
        cache_read_tokens: row.get(10)?,
        cost_usd: row.get(11)?,
        served_provider: row.get(12)?,
        served_model: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> TaskStore {
        TaskStore::open_in_memory().expect("in-memory store")
    }

    #[test]
    fn a_new_task_is_created_before_it_has_a_run() {
        let store = store();
        let task = store.create("t1", "kairos-s1", "hello").expect("create");
        assert_eq!(task.status, TaskStatus::Created);
        assert_eq!(task.run_id, None);
    }

    #[test]
    fn the_same_task_id_cannot_be_created_twice() {
        let store = store();
        store.create("t1", "kairos-s1", "hello").expect("create");
        assert!(store.create("t1", "kairos-s1", "hello again").is_err());
    }

    #[test]
    fn a_task_moves_through_running_to_completed_with_its_outcome() {
        let store = store();
        store.create("t1", "kairos-s1", "hello").expect("create");
        store.set_run("t1", "run_1").expect("set run");
        assert_eq!(
            store.get("t1").unwrap().unwrap().status,
            TaskStatus::Running
        );
        store
            .set_status("t1", TaskStatus::Completed, Some("Kairos OK"))
            .expect("complete");
        let task = store.get("t1").unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.outcome.as_deref(), Some("Kairos OK"));
        assert!(task.finished_at.is_some());
    }

    #[test]
    fn a_finished_task_never_goes_back_to_running() {
        let store = store();
        store.create("t1", "kairos-s1", "hello").expect("create");
        store
            .set_status("t1", TaskStatus::Cancelled, None)
            .expect("cancel");
        store
            .set_status("t1", TaskStatus::Running, None)
            .expect("no-op");
        assert_eq!(
            store.get("t1").unwrap().unwrap().status,
            TaskStatus::Cancelled
        );
    }

    #[test]
    fn run_events_map_to_task_statuses() {
        let completed = RunEvent::RunCompleted {
            output: String::new(),
            usage: Default::default(),
            runtime: None,
            completed: true,
            partial: false,
            interrupted: false,
        };
        assert_eq!(TaskStatus::after(&completed), Some(TaskStatus::Completed));
        let partial = RunEvent::RunCompleted {
            output: String::new(),
            usage: Default::default(),
            runtime: None,
            completed: true,
            partial: true,
            interrupted: false,
        };
        assert_eq!(TaskStatus::after(&partial), Some(TaskStatus::Incomplete));
        assert_eq!(
            TaskStatus::after(&RunEvent::RunCancelled {
                turn_exit_reason: None
            }),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(
            TaskStatus::after(&RunEvent::MessageDelta { delta: "x".into() }),
            None
        );
    }

    #[test]
    fn conversations_are_titled_by_their_first_request_and_tasks_come_back_in_order() {
        let store = store();
        store.create("a", "kairos-s1", "first").expect("create");
        store.create("b", "kairos-s2", "other").expect("create");
        store.create("c", "kairos-s1", "second").expect("create");
        let goals: Vec<String> = store
            .for_session("kairos-s1")
            .expect("query")
            .into_iter()
            .map(|t| t.goal)
            .collect();
        assert_eq!(goals, ["first", "second"]);
        let conversations = store.conversations().expect("conversations");
        let s1 = conversations
            .iter()
            .find(|c| c.session_id == "kairos-s1")
            .expect("s1");
        assert_eq!(s1.title, "first");
        assert_eq!(s1.task_count, 2);
    }

    #[test]
    fn only_unfinished_tasks_are_open() {
        let store = store();
        store.create("a", "kairos-s1", "running").expect("create");
        store.set_run("a", "run_a").expect("run");
        store.create("b", "kairos-s1", "done").expect("create");
        store
            .set_status("b", TaskStatus::Completed, Some("ok"))
            .expect("done");
        let open: Vec<String> = store
            .open_tasks()
            .expect("open")
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(open, ["a"]);
    }

    #[test]
    fn a_version_1_database_is_migrated_and_keeps_its_tasks() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.execute_batch(
            "CREATE TABLE task (
               id TEXT PRIMARY KEY, session_id TEXT NOT NULL, run_id TEXT, goal TEXT NOT NULL,
               status TEXT NOT NULL, created_at INTEGER NOT NULL, finished_at INTEGER, outcome TEXT
             );
             CREATE INDEX task_session ON task(session_id, created_at);
             INSERT INTO task VALUES ('t1', 'kairos-s1', 'run_1', 'hello', 'completed', 1, 2, 'ok');
             PRAGMA user_version = 1;",
        )
        .expect("v1 schema");
        let store = TaskStore::init(conn).expect("migrate");
        let version: i32 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert_eq!(version, SCHEMA_VERSION);
        let task = store.get("t1").expect("get").expect("kept");
        assert_eq!(task.status, TaskStatus::Completed);
        store.set_setting("alerts_enabled", "false").expect("set");
        assert_eq!(
            store.setting("alerts_enabled").expect("read").as_deref(),
            Some("false")
        );
    }

    #[test]
    fn presence_events_come_back_newest_first_and_limited() {
        let store = store();
        for (at, kind, detail) in [
            (10, PresenceKind::Startup, Some("autostart")),
            (20, PresenceKind::Unlock, None),
            (30, PresenceKind::Resume, Some("slept 2 h")),
        ] {
            store
                .add_presence(&PresenceEvent {
                    at,
                    kind,
                    detail: detail.map(str::to_string),
                    away_s: None,
                })
                .expect("add");
        }
        let recent = store.recent_presence(2).expect("recent");
        assert_eq!(
            recent.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [PresenceKind::Resume, PresenceKind::Unlock]
        );
        assert_eq!(recent[0].detail.as_deref(), Some("slept 2 h"));
    }

    #[test]
    fn a_version_2_database_gains_notifications_and_keeps_presence_and_settings() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.execute_batch(
            "CREATE TABLE task (
               id TEXT PRIMARY KEY, session_id TEXT NOT NULL, run_id TEXT, goal TEXT NOT NULL,
               status TEXT NOT NULL, created_at INTEGER NOT NULL, finished_at INTEGER, outcome TEXT
             );
             CREATE TABLE presence_event (
               id INTEGER PRIMARY KEY, at INTEGER NOT NULL, kind TEXT NOT NULL, detail TEXT
             );
             CREATE TABLE setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO presence_event (at, kind, detail) VALUES (5, 'unlock', NULL);
             INSERT INTO setting VALUES ('alerts_enabled', 'false');
             PRAGMA user_version = 2;",
        )
        .expect("v2 schema");
        let store = TaskStore::init(conn).expect("migrate");
        assert_eq!(
            store.recent_presence(5).expect("presence")[0].kind,
            PresenceKind::Unlock
        );
        assert_eq!(
            store.setting("alerts_enabled").expect("setting").as_deref(),
            Some("false")
        );
        assert_eq!(store.unread_notifications().expect("notifications"), 0);
    }

    #[test]
    fn the_task_summary_counts_what_ended_since_and_what_waits_now() {
        let store = store();
        store.create("old", "kairos-s", "before").expect("create");
        store
            .set_status("old", TaskStatus::Completed, Some("ok"))
            .expect("done");
        store
            .conn
            .execute("UPDATE task SET finished_at = 10 WHERE id = 'old'", [])
            .expect("backdate");
        store.create("a", "kairos-s", "a").expect("create");
        store
            .set_status("a", TaskStatus::Completed, Some("ok"))
            .expect("done");
        store.create("b", "kairos-s", "b").expect("create");
        store
            .set_status("b", TaskStatus::Failed, Some("boom"))
            .expect("failed");
        store.create("c", "kairos-s", "c").expect("create");
        store
            .set_status("c", TaskStatus::WaitingForApproval, None)
            .expect("waiting");
        assert_eq!(
            store.task_summary_since(100).expect("summary"),
            TaskSummary {
                finished: 1,
                failed: 1,
                waiting: 1
            }
        );
    }

    #[test]
    fn a_version_3_database_gains_models_and_usage_and_keeps_its_tasks() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.execute_batch(
            "CREATE TABLE task (
               id TEXT PRIMARY KEY, session_id TEXT NOT NULL, run_id TEXT, goal TEXT NOT NULL,
               status TEXT NOT NULL, created_at INTEGER NOT NULL, finished_at INTEGER, outcome TEXT
             );
             INSERT INTO task VALUES ('t1', 'kairos-s1', 'run_1', 'hello', 'completed', 1, 2, 'ok');
             CREATE TABLE presence_event (id INTEGER PRIMARY KEY, at INTEGER NOT NULL, kind TEXT NOT NULL, detail TEXT);
             CREATE TABLE setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE notification (
               id INTEGER PRIMARY KEY, app_id TEXT NOT NULL, app_name TEXT NOT NULL,
               windows_id INTEGER NOT NULL, created_at INTEGER NOT NULL, title TEXT NOT NULL,
               body TEXT NOT NULL, read_at INTEGER, UNIQUE (app_id, windows_id, created_at)
             );
             PRAGMA user_version = 3;",
        )
        .expect("v3 schema");
        let store = TaskStore::init(conn).expect("migrate");
        let task = store.get("t1").expect("get").expect("kept");
        assert_eq!(task.goal, "hello");
        assert_eq!(task.cost_usd, None);
        assert_eq!(
            store.conversation_model("kairos-s1").expect("model"),
            ModelChoice::default()
        );
    }

    #[test]
    fn a_conversation_remembers_its_model_and_forgets_it_on_default() {
        let store = store();
        let choice = ModelChoice {
            provider: Some("openrouter".into()),
            model: Some("example/large".into()),
        };
        store
            .set_conversation_model("kairos-s", &choice)
            .expect("set");
        assert_eq!(store.conversation_model("kairos-s").expect("get"), choice);
        store
            .set_conversation_model("kairos-s", &ModelChoice::default())
            .expect("default");
        assert_eq!(
            store.conversation_model("kairos-s").expect("get"),
            ModelChoice::default()
        );
    }

    #[test]
    fn usage_and_the_served_model_are_stored_on_the_task() {
        use crate::hermes::events::{Runtime, Usage};
        let store = store();
        store.create("a", "kairos-s", "hi").expect("create");
        let usage = Usage {
            input_tokens: 1200,
            output_tokens: 300,
            total_tokens: 1500,
            cache_read_tokens: 200,
            cache_write_tokens: 0,
        };
        let served = Runtime {
            provider: "openrouter".into(),
            model: "example/large".into(),
        };
        store
            .record_usage("a", &usage, Some(0.0021), Some(&served))
            .expect("record");
        let task = store.get("a").expect("get").expect("task");
        assert_eq!(
            (
                task.input_tokens,
                task.output_tokens,
                task.cache_read_tokens
            ),
            (Some(1200), Some(300), Some(200))
        );
        assert_eq!(task.cost_usd, Some(0.0021));
        assert_eq!(task.served_model.as_deref(), Some("example/large"));
    }

    #[test]
    fn a_setting_is_none_until_set_and_the_last_value_wins() {
        let store = store();
        assert_eq!(store.setting("x").expect("read"), None);
        store.set_setting("x", "1").expect("set");
        store.set_setting("x", "2").expect("set");
        assert_eq!(store.setting("x").expect("read").as_deref(), Some("2"));
    }

    #[test]
    fn a_database_from_a_newer_kairos_is_refused() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.pragma_update(None, "user_version", 99)
            .expect("pragma");
        assert!(TaskStore::init(conn).is_err());
    }
}
