mod audit;
mod commands;
// Public so the fixture tests in tests/ can drive it.
pub mod hermes;
mod keyring;
mod rules;
mod store;
mod system;

use std::sync::Arc;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .manage(Arc::new(system::SystemMonitor::default()))
        .setup(|app| {
            let handle = app.handle().clone();
            app.state::<Arc<system::SystemMonitor>>()
                .start(move |snapshot| {
                    let _ = handle.emit("system-snapshot", snapshot);
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
            commands::set_system_page_visible
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
