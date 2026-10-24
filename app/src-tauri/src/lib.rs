mod commands;
// Public so the fixture tests in tests/ can drive it.
pub mod hermes;
mod keyring;
mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::create_run,
            commands::subscribe_run,
            commands::connect,
            commands::reload_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
