//! The commands behind Settings ▸ Automations and the main window's report on its part of a run.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::engine::{self, CHANGED_EVENT};
use super::store::{HISTORY_SHOWN, Outcome, Run};
use super::{Action, Automation};
use crate::commands::{AppState, CommandError};

const MAX_ACTIONS: usize = 8;
const MAX_TEXT: usize = 2000;

#[derive(Debug, Clone, Serialize)]
pub struct AutomationsState {
    pub enabled: bool,
    pub automations: Vec<Automation>,
    pub runs: Vec<Run>,
    pub crash_ignore: Vec<String>,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError {
        kind: "invalid",
        message: message.into(),
    }
}

fn read(state: &AppState) -> Result<AutomationsState, CommandError> {
    state.with_store(|store| {
        Ok(AutomationsState {
            enabled: store.automations_enabled()?,
            automations: store.automations()?,
            runs: store.automation_runs(HISTORY_SHOWN)?,
            crash_ignore: store.crash_ignore()?,
        })
    })
}

/// What the editor may save: a name, a few actions, texts of a sane length.
fn check(automation: &Automation) -> Result<(), CommandError> {
    if automation.id.trim().is_empty() || automation.name.trim().is_empty() {
        return Err(invalid("An automation needs a name."));
    }
    if automation.actions.len() > MAX_ACTIONS {
        return Err(invalid(format!("At most {MAX_ACTIONS} actions.")));
    }
    for action in &automation.actions {
        let text = match action {
            Action::Notify { text }
            | Action::Say { text }
            | Action::Island { text }
            | Action::Offer { text }
            | Action::Ask { text } => text,
            Action::OpenApp { app_id, .. } => app_id,
        };
        if text.trim().is_empty() {
            return Err(invalid("Every action needs its text (or its app)."));
        }
        if text.len() > MAX_TEXT {
            return Err(invalid(format!(
                "A text is limited to {MAX_TEXT} characters."
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn automations_state(
    state: tauri::State<'_, AppState>,
) -> Result<AutomationsState, CommandError> {
    read(&state)
}

#[tauri::command]
pub async fn save_automation(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    automation: Automation,
) -> Result<AutomationsState, CommandError> {
    check(&automation)?;
    let automation = Automation {
        cooldown_min: automation.cooldown_min.max(1),
        ..automation
    };
    state.with_store(|store| store.save_automation(&automation))?;
    let _ = app.emit(CHANGED_EVENT, ());
    read(&state)
}

#[tauri::command]
pub async fn delete_automation(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<AutomationsState, CommandError> {
    state.with_store(|store| store.delete_automation(&id))?;
    read(&state)
}

#[tauri::command]
pub async fn set_automations_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<AutomationsState, CommandError> {
    state.with_store(|store| store.set_automations_enabled(enabled))?;
    read(&state)
}

#[tauri::command]
pub async fn set_crash_ignore(
    state: tauri::State<'_, AppState>,
    names: Vec<String>,
) -> Result<AutomationsState, CommandError> {
    let names: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    state.with_store(|store| store.set_crash_ignore(&names))?;
    read(&state)
}

/// Runs the automation as edited (saved or not) with a sample event, marked as a test.
#[tauri::command]
pub async fn test_automation(app: AppHandle, automation: Automation) -> Result<(), CommandError> {
    check(&automation)?;
    engine::test(&app, automation);
    Ok(())
}

/// The main window finished its part of a run (or couldn't).
#[tauri::command]
pub async fn automation_outcome(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    run_id: i64,
    outcome: Outcome,
    reason: Option<String>,
    asked: bool,
) -> Result<(), CommandError> {
    state.with_store(|store| store.finish_run(run_id, outcome, reason.as_deref(), asked))?;
    let _ = app.emit(CHANGED_EVENT, ());
    Ok(())
}
