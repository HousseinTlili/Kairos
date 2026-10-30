//! The `automation` and `automation_run` tables of `kairos.db` (M17 §4), and the automations'
//! settings in the `setting` table.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Automation, starters};
use crate::store::{StoreError, TaskStore};

const SEEDED: &str = "automations_seeded";
const ENABLED: &str = "automations_enabled";
const CRASH_IGNORE: &str = "automations_crash_ignore";
/// The history Settings shows, and how much is kept.
pub const HISTORY_SHOWN: usize = 100;
const HISTORY_KEPT: i64 = 1000;

/// Windows's own background programs, whose crashes say nothing to the Master (M17 §3.1). Kairos
/// itself is ignored too: its own crash can't be an automation's business.
pub const DEFAULT_CRASH_IGNORE: &[&str] = &[
    "RuntimeBroker.exe",
    "svchost.exe",
    "backgroundTaskHost.exe",
    "SearchHost.exe",
    "SearchIndexer.exe",
    "SearchProtocolHost.exe",
    "taskhostw.exe",
    "dllhost.exe",
    "WerFault.exe",
    "ShellExperienceHost.exe",
    "StartMenuExperienceHost.exe",
    "TextInputHost.exe",
    "SystemSettings.exe",
    "smartscreen.exe",
    "conhost.exe",
    "MoUsoCoreWorker.exe",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Everything it does was done.
    Done,
    /// The main window is still doing its part (an "Ask Kairos" waiting for the chat to be free).
    Waiting,
    Skipped,
    Failed,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Done => "done",
            Outcome::Waiting => "waiting",
            Outcome::Skipped => "skipped",
            Outcome::Failed => "failed",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "done" => Outcome::Done,
            "waiting" => Outcome::Waiting,
            "skipped" => Outcome::Skipped,
            _ => Outcome::Failed,
        }
    }
}

/// One line of the history.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Run {
    pub id: i64,
    pub automation_id: String,
    pub name: String,
    pub at: i64,
    /// What happened, in words (`Happening::describe`).
    pub event: String,
    pub outcome: Outcome,
    pub reason: Option<String>,
    /// An "Ask Kairos" was to start (counted for the limits).
    pub asked: bool,
    pub test: bool,
}

pub struct NewRun<'a> {
    pub automation_id: &'a str,
    pub name: &'a str,
    pub at: i64,
    pub event: &'a str,
    pub outcome: Outcome,
    pub reason: Option<&'a str>,
    pub asked: bool,
    pub test: bool,
}

impl TaskStore {
    /// Every automation in the Master's order. A fresh profile gets the starters once.
    pub fn automations(&self) -> Result<Vec<Automation>, StoreError> {
        if self.setting(SEEDED)?.is_none() {
            for (i, a) in starters().iter().enumerate() {
                self.write_automation(a, i as i64)?;
            }
            self.set_setting(SEEDED, "1")?;
        }
        let mut statement = self
            .conn()
            .prepare("SELECT definition FROM automation ORDER BY position, id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            // A row this version can't read (written by a newer Kairos) is left alone, not lost.
            if let Ok(a) = serde_json::from_str::<Automation>(&row?) {
                out.push(a);
            }
        }
        Ok(out)
    }

    fn write_automation(&self, automation: &Automation, position: i64) -> Result<(), StoreError> {
        let definition =
            serde_json::to_string(automation).map_err(|e| StoreError::new(e.to_string()))?;
        self.conn().execute(
            "INSERT INTO automation (id, position, definition) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET definition = excluded.definition",
            params![automation.id, position, definition],
        )?;
        Ok(())
    }

    /// Adds a new automation at the end, or replaces one in place.
    pub fn save_automation(&self, automation: &Automation) -> Result<(), StoreError> {
        let position: i64 = self.conn().query_row(
            "SELECT COALESCE((SELECT position FROM automation WHERE id = ?1),
                             (SELECT COALESCE(MAX(position), -1) + 1 FROM automation))",
            params![automation.id],
            |row| row.get(0),
        )?;
        self.write_automation(automation, position)
    }

    /// Removes the automation; its history stays.
    pub fn delete_automation(&self, id: &str) -> Result<(), StoreError> {
        self.conn()
            .execute("DELETE FROM automation WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn automations_enabled(&self) -> Result<bool, StoreError> {
        Ok(self.setting(ENABLED)?.as_deref() != Some("0"))
    }

    pub fn set_automations_enabled(&self, on: bool) -> Result<(), StoreError> {
        self.set_setting(ENABLED, if on { "1" } else { "0" })
    }

    pub fn crash_ignore(&self) -> Result<Vec<String>, StoreError> {
        Ok(match self.setting(CRASH_IGNORE)? {
            Some(text) => serde_json::from_str(&text).unwrap_or_default(),
            None => DEFAULT_CRASH_IGNORE.iter().map(|s| s.to_string()).collect(),
        })
    }

    pub fn set_crash_ignore(&self, names: &[String]) -> Result<(), StoreError> {
        let text = serde_json::to_string(names).map_err(|e| StoreError::new(e.to_string()))?;
        self.set_setting(CRASH_IGNORE, &text)
    }

    pub fn record_run(&self, run: &NewRun<'_>) -> Result<i64, StoreError> {
        self.conn().execute(
            "INSERT INTO automation_run (automation_id, name, at, event, outcome, reason, asked, test)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.automation_id,
                run.name,
                run.at,
                run.event,
                run.outcome.as_str(),
                run.reason,
                run.asked,
                run.test
            ],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn().execute(
            "DELETE FROM automation_run WHERE id <= ?1",
            params![id - HISTORY_KEPT],
        )?;
        Ok(id)
    }

    /// The main window's word on its part of a run. `asked` is cleared when the request never
    /// started, so it doesn't count against the limits.
    pub fn finish_run(
        &self,
        id: i64,
        outcome: Outcome,
        reason: Option<&str>,
        asked: bool,
    ) -> Result<(), StoreError> {
        self.conn().execute(
            "UPDATE automation_run SET outcome = ?2, reason = COALESCE(?3, reason), asked = ?4
             WHERE id = ?1",
            params![id, outcome.as_str(), reason, asked],
        )?;
        Ok(())
    }

    pub fn automation_runs(&self, limit: usize) -> Result<Vec<Run>, StoreError> {
        let mut statement = self.conn().prepare(
            "SELECT id, automation_id, name, at, event, outcome, reason, asked, test
             FROM automation_run ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(Run {
                id: row.get(0)?,
                automation_id: row.get(1)?,
                name: row.get(2)?,
                at: row.get(3)?,
                event: row.get(4)?,
                outcome: Outcome::parse(&row.get::<_, String>(5)?),
                reason: row.get(6)?,
                asked: row.get(7)?,
                test: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// When the automation last ran for real (not skipped, not a test), for its cooldown.
    pub fn last_run_at(&self, automation_id: &str) -> Result<Option<i64>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT MAX(at) FROM automation_run
                 WHERE automation_id = ?1 AND outcome != 'skipped' AND test = 0",
                params![automation_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten())
    }

    /// "Ask Kairos" requests by automations since `since` (Unix seconds), tests included.
    pub fn ai_runs_since(&self, since: i64) -> Result<usize, StoreError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM automation_run WHERE asked = 1 AND at >= ?1",
            params![since],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::{Action, Trigger};

    fn run<'a>(id: &'a str, at: i64, outcome: Outcome, asked: bool) -> NewRun<'a> {
        NewRun {
            automation_id: id,
            name: "Test",
            at,
            event: "Kairos started at login",
            outcome,
            reason: None,
            asked,
            test: false,
        }
    }

    #[test]
    fn a_fresh_profile_gets_the_three_starters_once() {
        let store = TaskStore::open_in_memory().expect("store");
        let first = store.automations().expect("list");
        assert_eq!(first.len(), 3);
        assert!(first[0].enabled, "the crash offer is on");
        store.delete_automation("starter-login").expect("delete");
        assert_eq!(
            store.automations().expect("list").len(),
            2,
            "not seeded again"
        );
    }

    #[test]
    fn a_saved_automation_keeps_its_place_and_a_new_one_goes_last() {
        let store = TaskStore::open_in_memory().expect("store");
        let mut list = store.automations().expect("list");
        list[0].name = "Renamed".into();
        store.save_automation(&list[0]).expect("save");
        let new = Automation {
            id: "mine".into(),
            name: "Mine".into(),
            enabled: true,
            trigger: Trigger::WifiLost,
            only_if: Default::default(),
            actions: vec![Action::Notify {
                text: "Offline".into(),
            }],
            cooldown_min: 5,
        };
        store.save_automation(&new).expect("save");
        let names: Vec<_> = store
            .automations()
            .expect("list")
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(names.first().map(String::as_str), Some("Renamed"));
        assert_eq!(names.last().map(String::as_str), Some("Mine"));
    }

    #[test]
    fn skips_and_tests_dont_start_a_cooldown_and_only_asked_runs_count_for_the_limits() {
        let store = TaskStore::open_in_memory().expect("store");
        store
            .record_run(&run("a", 100, Outcome::Skipped, false))
            .expect("run");
        assert_eq!(store.last_run_at("a").expect("last"), None);
        let mut test = run("a", 150, Outcome::Done, true);
        test.test = true;
        store.record_run(&test).expect("run");
        assert_eq!(store.last_run_at("a").expect("last"), None);
        store
            .record_run(&run("a", 200, Outcome::Done, false))
            .expect("run");
        assert_eq!(store.last_run_at("a").expect("last"), Some(200));
        let waiting = store
            .record_run(&run("b", 300, Outcome::Waiting, true))
            .expect("run");
        assert_eq!(store.ai_runs_since(0).expect("count"), 2);
        // The chat was busy: the request never started, so it doesn't count.
        store
            .finish_run(
                waiting,
                Outcome::Skipped,
                Some("a request was running"),
                false,
            )
            .expect("finish");
        assert_eq!(store.ai_runs_since(0).expect("count"), 1);
        let history = store.automation_runs(10).expect("history");
        assert_eq!(history[0].outcome, Outcome::Skipped);
        assert_eq!(history[0].reason.as_deref(), Some("a request was running"));
    }

    #[test]
    fn the_crash_ignore_list_starts_with_windows_background_programs() {
        let store = TaskStore::open_in_memory().expect("store");
        assert!(
            store
                .crash_ignore()
                .expect("list")
                .iter()
                .any(|n| n == "RuntimeBroker.exe")
        );
        store.set_crash_ignore(&["game.exe".into()]).expect("save");
        assert_eq!(
            store.crash_ignore().expect("list"),
            vec!["game.exe".to_string()]
        );
    }
}
