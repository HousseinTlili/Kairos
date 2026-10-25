mod audit;
mod commands;
// Public so the fixture tests in tests/ can drive it.
pub mod hermes;
mod keyring;
mod rules;
mod store;
mod system;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use system::alerts::{Alerts, Evaluation};
use system::{AlertSwitch, SystemMonitor};

/// Sends new alerts to the page (banners) and to Windows (toasts), and cleared ones to the page.
fn announce(app: &AppHandle, evaluation: &Evaluation) {
    if evaluation.fired.is_empty() && evaluation.cleared.is_empty() {
        return;
    }
    let _ = app.emit("system-alerts", evaluation);
    for alert in &evaluation.fired {
        let _ = app
            .notification()
            .builder()
            .title(&alert.title)
            .body(&alert.message)
            .show();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .plugin(tauri_plugin_notification::init())
        .manage(Arc::new(SystemMonitor::default()))
        .setup(|app| {
            let enabled = commands::stored_alerts_enabled(&app.state::<commands::AppState>());
            let switch = Arc::new(AlertSwitch(AtomicBool::new(enabled)));
            app.manage(Arc::clone(&switch));

            let handle = app.handle().clone();
            let mut alerts = Alerts::for_this_pc();
            app.state::<Arc<SystemMonitor>>().start(move |snapshot| {
                let _ = handle.emit("system-snapshot", snapshot);
                // Evaluated even while switched off, so turning alerts on doesn't replay old ones.
                let evaluation = alerts.evaluate(snapshot);
                if switch.enabled() {
                    announce(&handle, &evaluation);
                }
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::reload_key,
            commands::create_run,
            commands::subscribe_run,
            commands::answer_approval,
            commands::stop_run,
            commands::resume_open_tasks,
            commands::list_conversations,
            commands::conversation_tasks,
            commands::audit_for_session,
            commands::rules_state,
            commands::validate_rules,
            commands::save_rules,
            commands::reset_rules,
            commands::system_snapshot,
            commands::set_system_page_visible,
            commands::run_speed_test,
            commands::alerts_enabled,
            commands::set_alerts_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
