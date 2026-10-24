//! Local `kairos.db` and its `task` table (M1 §10, ARCHITECTURE.md §4).
//!
//! A task row is written *before* Hermes is called and its ID is the run's Idempotency-Key, so a
//! retry after a crash or a network error can never start a second run (M1 §9 rule 1).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::hermes::events::RunEvent;

const SCHEMA_VERSION: i32 = 1;

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
                "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome
                 FROM task WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()?)
    }

    /// A conversation's tasks, oldest first.
    pub fn for_session(&self, session_id: &str) -> Result<Vec<Task>, StoreError> {
        self.query_tasks(
            "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome
             FROM task WHERE session_id = ?1 ORDER BY created_at, rowid",
            params![session_id],
        )
    }

    /// Tasks that haven't reached a final status: candidates for reconnecting after a restart
    /// (M1 §9 rule 6).
    pub fn open_tasks(&self) -> Result<Vec<Task>, StoreError> {
        self.query_tasks(
            "SELECT id, session_id, run_id, goal, status, created_at, finished_at, outcome
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
            completed: true,
            partial: false,
            interrupted: false,
        };
        assert_eq!(TaskStatus::after(&completed), Some(TaskStatus::Completed));
        let partial = RunEvent::RunCompleted {
            output: String::new(),
            usage: Default::default(),
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
    fn a_database_from_a_newer_kairos_is_refused() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.pragma_update(None, "user_version", 99)
            .expect("pragma");
        assert!(TaskStore::init(conn).is_err());
    }
}
