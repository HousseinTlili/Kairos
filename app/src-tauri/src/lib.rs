mod commands;
// Public so the fixture tests in tests/ can drive it, and so it isn't dead code before the
// Tauri commands use it (Step 4).
pub mod hermes;
mod keyring;
mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
