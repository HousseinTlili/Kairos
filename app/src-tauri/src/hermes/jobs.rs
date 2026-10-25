//! Hermes's scheduled jobs, `/api/jobs` (M8 §5). Hermes stores and runs them; Kairos only lists
//! and changes them. Fields are read leniently: everything but `id` defaults, so a job written by
//! an older or newer Hermes still lists.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Job {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub prompt: String,
    /// Hermes's own wording of the schedule ("every monday 9am", "0 9 * * *").
    #[serde(default)]
    pub schedule_display: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// `scheduled`, `paused`, `completed` or `error`.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    /// `ok`, `error`, `delivery_failed`, … ; `None` before the first run.
    #[serde(default)]
    pub last_status: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct JobList {
    #[serde(default)]
    pub jobs: Vec<Job>,
}

#[derive(Debug, Deserialize)]
pub struct JobEnvelope {
    pub job: Job,
}

/// `POST /api/jobs`. `deliver: "local"` keeps the output in Hermes rather than posting it to a
/// chat platform: Kairos shows it, and nothing is sent anywhere on the user's behalf.
#[derive(Debug, Serialize)]
pub struct NewJob<'a> {
    pub name: &'a str,
    pub schedule: &'a str,
    pub prompt: &'a str,
    pub deliver: &'static str,
}

impl<'a> NewJob<'a> {
    pub fn local(name: &'a str, schedule: &'a str, prompt: &'a str) -> Self {
        Self {
            name,
            schedule,
            prompt,
            deliver: "local",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_list_reads_hermes_jobs_and_ignores_fields_kairos_doesnt_use() {
        let list: JobList = serde_json::from_value(serde_json::json!({"jobs": [
            {
                "id": "a1b2c3d4e5f6", "name": "Weekly summary", "prompt": "Summarise my week",
                "schedule": {"kind": "cron", "expr": "0 9 * * 1", "display": "every monday 9am"},
                "schedule_display": "every monday 9am", "enabled": true, "state": "scheduled",
                "next_run_at": "2026-10-26T09:00:00+00:00", "last_run_at": null,
                "last_status": null, "last_error": null, "repeat": {"times": null, "completed": 0},
                "deliver": "local", "origin": null, "workdir": null
            },
            {"id": "0123456789ab", "enabled": false, "state": "paused", "last_status": "error",
             "last_error": "Provider returned 429"}
        ]}))
        .expect("parses");
        assert_eq!(list.jobs.len(), 2);
        let weekly = &list.jobs[0];
        assert_eq!(weekly.schedule_display, "every monday 9am");
        assert_eq!(
            weekly.next_run_at.as_deref(),
            Some("2026-10-26T09:00:00+00:00")
        );
        assert!(weekly.enabled && weekly.last_status.is_none());
        let paused = &list.jobs[1];
        assert_eq!((paused.enabled, paused.state.as_str()), (false, "paused"));
        assert_eq!(paused.last_error.as_deref(), Some("Provider returned 429"));
        assert_eq!(paused.name, "");
    }

    #[test]
    fn a_job_without_an_enabled_field_counts_as_enabled() {
        let job: Job = serde_json::from_str(r#"{"id": "a1b2c3d4e5f6"}"#).expect("parses");
        assert!(job.enabled);
    }

    #[test]
    fn a_new_job_keeps_its_output_in_hermes() {
        let body =
            serde_json::to_value(NewJob::local("Hello", "every 2m", "say hello")).expect("json");
        assert_eq!(
            body,
            serde_json::json!({"name": "Hello", "schedule": "every 2m", "prompt": "say hello", "deliver": "local"})
        );
    }
}
