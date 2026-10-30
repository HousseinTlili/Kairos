//! Runs automations (M17 §4): decides each one with `rules::decide`, records the run, does the
//! toasts and app launches here, and hands voice, the island, offers and "Ask Kairos" to the main
//! window (`automation-fired`), which reports back with `automation_outcome`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use super::rules::{self, Moment, Verdict};
use super::store::{NewRun, Outcome};
use super::{Action, Automation, Happening, fill};
use crate::briefing::{self, BriefingClock};
use crate::commands::AppState;
use crate::island_panel::registry::Registry;
use crate::presence::unix_now;
use crate::system::{BatteryState, SystemMonitor};

pub const FIRED_EVENT: &str = "automation-fired";
pub const CHANGED_EVENT: &str = "automations-changed";
/// Quiet hours are local time: at login the page may not have told the core its offset yet.
const WAIT_FOR_CLOCK: Duration = Duration::from_secs(20);

/// What the main window is to do for a run.
#[derive(Debug, Clone, Serialize)]
pub struct Fired {
    pub run_id: i64,
    pub automation_id: String,
    pub name: String,
    /// What happened, in words; given to "Ask Kairos" as data.
    pub event: String,
    pub test: bool,
    /// Say, Island, Offer and Ask, with their texts filled.
    pub actions: Vec<Action>,
}

fn toast(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Starts an app from the Start menu list by its id (the pinned-apps module checks the id is one it
/// listed, so no command line ever comes from an automation).
fn open_app(app: &AppHandle, app_id: &str) -> Result<(), String> {
    let registry = app.state::<Registry>();
    registry.dispatch(app, "pinned", "catalog", serde_json::Value::Null)?;
    registry
        .dispatch(app, "pinned", "open", json!({ "id": app_id }))
        .map(|_| ())
}

/// Called from the threads that notice things; the work runs on a thread of its own, so a slow
/// app launch never holds up the presence or system monitors.
pub fn on_happening(app: &AppHandle, happening: Happening) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("kairos-automation".into())
        .spawn(move || run_all(&app, &happening));
}

/// "Test now": the automation as edited, with a sample of its happening; its conditions and
/// cooldown are passed over, the AI limits are not.
pub fn test(app: &AppHandle, automation: Automation) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("kairos-automation-test".into())
        .spawn(move || {
            let happening = super::sample(&automation.trigger);
            let state = app.state::<AppState>();
            let now = unix_now();
            let verdict = state.with_store(|store| {
                let hour = store.ai_runs_since(now - 3600)?;
                let day = store.ai_runs_since(now - 24 * 3600)?;
                Ok(Verdict::Run {
                    ask: if !automation.actions.iter().any(Action::uses_ai) {
                        Ok(())
                    } else if hour >= rules::AI_PER_HOUR || day >= rules::AI_PER_DAY {
                        Err("the limit of AI requests by automations is reached".into())
                    } else {
                        Ok(())
                    },
                })
            });
            if let Ok(verdict) = verdict {
                carry_out(&app, &automation, &happening, verdict, true);
            }
        });
}

fn run_all(app: &AppHandle, happening: &Happening) {
    let clock = app.state::<Arc<BriefingClock>>();
    let started = Instant::now();
    while clock.offset().is_none() && started.elapsed() < WAIT_FOR_CLOCK {
        std::thread::sleep(Duration::from_millis(500));
    }
    let on_battery = app
        .state::<Arc<SystemMonitor>>()
        .latest()
        .and_then(|s| s.battery)
        .map(|b| b.state == BatteryState::Discharging);
    let state = app.state::<AppState>();
    let decided = state.with_store(|store| {
        if !store.automations_enabled()? {
            return Ok(Vec::new());
        }
        let quiet = briefing::load_settings(store)?;
        let now = unix_now();
        let ai_last_hour = store.ai_runs_since(now - 3600)?;
        let ai_last_day = store.ai_runs_since(now - 24 * 3600)?;
        let mut out = Vec::new();
        for automation in store.automations()? {
            let moment = Moment {
                now,
                utc_offset_min: clock.offset().unwrap_or(0),
                on_battery,
                quiet_start: quiet.quiet_start,
                quiet_end: quiet.quiet_end,
                last_run_at: store.last_run_at(&automation.id)?,
                ai_last_hour,
                ai_last_day,
            };
            if let Some(verdict) = rules::decide(&automation, happening, &moment) {
                out.push((automation, verdict));
            }
        }
        Ok(out)
    });
    match decided {
        Ok(list) => {
            for (automation, verdict) in list {
                carry_out(app, &automation, happening, verdict, false);
            }
        }
        Err(err) => eprintln!("kairos: automations: {}", err.message),
    }
}

fn carry_out(
    app: &AppHandle,
    automation: &Automation,
    happening: &Happening,
    verdict: Verdict,
    test: bool,
) {
    let state = app.state::<AppState>();
    let event = happening.describe();
    let ask = match verdict {
        Verdict::Skip(reason) => {
            let _ = state.with_store(|store| {
                store.record_run(&NewRun {
                    automation_id: &automation.id,
                    name: &automation.name,
                    at: unix_now(),
                    event: &event,
                    outcome: Outcome::Skipped,
                    reason: Some(&reason),
                    asked: false,
                    test,
                })
            });
            let _ = app.emit(CHANGED_EVENT, ());
            return;
        }
        Verdict::Run { ask } => ask,
    };
    let values = happening.values();
    let page: Vec<Action> = automation
        .actions
        .iter()
        .filter(|a| a.for_page() && (!a.uses_ai() || ask.is_ok()))
        .map(|a| match a {
            Action::Say { text } => Action::Say {
                text: fill(text, &values),
            },
            Action::Island { text } => Action::Island {
                text: fill(text, &values),
            },
            Action::Offer { text } => Action::Offer {
                text: fill(text, &values),
            },
            Action::Ask { text } => Action::Ask {
                text: fill(text, &values),
            },
            other => other.clone(),
        })
        .collect();
    let asked = page.iter().any(Action::uses_ai);
    let held_back = ask.err();
    let run_id = state.with_store(|store| {
        store.record_run(&NewRun {
            automation_id: &automation.id,
            name: &automation.name,
            at: unix_now(),
            event: &event,
            outcome: if page.is_empty() {
                Outcome::Done
            } else {
                Outcome::Waiting
            },
            reason: held_back.as_deref(),
            asked,
            test,
        })
    });
    let Ok(run_id) = run_id else {
        return;
    };

    let title = if test {
        format!("{} (test)", automation.name)
    } else {
        automation.name.clone()
    };
    let mut failures = Vec::new();
    for action in &automation.actions {
        match action {
            Action::Notify { text } => toast(app, &title, &fill(text, &values)),
            // The toast can't carry a button on Windows desktop: the offer's button is in Kairos.
            Action::Offer { text } => toast(
                app,
                &title,
                &format!("{} Kairos is waiting for your answer.", fill(text, &values)),
            ),
            Action::OpenApp { app_id, name } => {
                if let Err(err) = open_app(app, app_id) {
                    failures.push(format!("couldn't open {name}: {err}"));
                }
            }
            _ => {}
        }
    }
    if !failures.is_empty() {
        let reason = failures.join("; ");
        // With a part still to come from the main window, the run stays open; its reason says
        // what already failed.
        let outcome = if page.is_empty() {
            Outcome::Failed
        } else {
            Outcome::Waiting
        };
        let _ = state.with_store(|store| store.finish_run(run_id, outcome, Some(&reason), asked));
    }
    if !page.is_empty() {
        let _ = app.emit(
            FIRED_EVENT,
            Fired {
                run_id,
                automation_id: automation.id.clone(),
                name: automation.name.clone(),
                event,
                test,
                actions: page,
            },
        );
    }
    let _ = app.emit(CHANGED_EVENT, ());
}
