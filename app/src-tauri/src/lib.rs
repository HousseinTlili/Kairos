mod accounts;
mod audit;
mod briefing;
mod commands;
// Public so the fixture tests in tests/ can drive it.
pub mod hermes;
mod keyring;
mod models;
mod notifications;
mod presence;
mod quick;
mod rules;
mod store;
mod system;
mod tray;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use presence::{PresenceEvent, PresenceKind};
use system::alerts::{Alerts, Evaluation};
use system::{AlertSwitch, SystemMonitor};

/// The argument Kairos's login entry starts it with: start hidden in the tray.
const AUTOSTART_ARG: &str = "--autostart";

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
    let at_login = std::env::args().any(|arg| arg == AUTOSTART_ARG);
    commands::LAUNCHED_AT_LOGIN.store(at_login, std::sync::atomic::Ordering::Relaxed);

    tauri::Builder::default()
        // First, so a second launch hands over to this one before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(commands::AppState::default())
        .manage(Arc::new(SystemMonitor::default()))
        .on_window_event(|window, event| {
            // Closing the window keeps Kairos in the tray; "Quit" in the tray menu ends it.
            match event {
                WindowEvent::CloseRequested { api, .. }
                    if window.label() == tray::MAIN_WINDOW
                        || window.label() == quick::QUICK_WINDOW =>
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // The quick box goes away when the user clicks elsewhere (M8 §3).
                WindowEvent::Focused(false) if window.label() == quick::QUICK_WINDOW => {
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .setup(move |app| {
            tray::build(app.handle())?;
            let hotkey = commands::stored_hotkey(&app.state::<commands::AppState>());
            if let Err(err) = quick::register(app.handle(), None, &hotkey) {
                eprintln!("kairos: {err}");
            }
            // The window starts hidden (tauri.conf.json); a login start stays in the tray.
            if !at_login {
                tray::show_main(app.handle());
            }
            commands::first_launch_autostart(app.handle(), &app.state::<commands::AppState>());

            let now = presence::unix_now();
            let startup = PresenceEvent {
                at: now,
                kind: PresenceKind::Startup,
                detail: at_login.then(|| "autostart".to_string()),
                away_s: commands::last_seen(&app.state::<commands::AppState>())
                    .map(|seen| (now - seen).max(0)),
            };
            commands::set_last_seen(&app.state::<commands::AppState>(), now);
            commands::record_presence(&app.state::<commands::AppState>(), &startup);
            app.manage(Arc::new(briefing::BriefingClock::default()));
            commands::on_presence(app.handle(), &startup);
            let notification_switch =
                commands::stored_notification_switch(&app.state::<commands::AppState>());
            app.manage(Arc::new(notification_switch));
            #[cfg(windows)]
            notifications::poller::start(app.handle().clone())?;
            // The file follows the store from the start (deleted if reading was switched off).
            commands::notifications_changed(app.handle());

            let handle = app.handle().clone();
            let heartbeat = app.handle().clone();
            presence::watch(
                move |event| {
                    commands::record_presence(&handle.state::<commands::AppState>(), &event);
                    let _ = handle.emit("presence", &event);
                    commands::on_presence(&handle, &event);
                },
                move |now| commands::set_last_seen(&heartbeat.state::<commands::AppState>(), now),
            )?;

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
            commands::set_alerts_enabled,
            commands::startup_state,
            commands::set_kairos_at_login,
            commands::notifications_state,
            commands::set_notifications_enabled,
            commands::list_notifications,
            commands::mark_notifications_read,
            commands::clear_notifications,
            commands::set_utc_offset,
            commands::briefing_settings,
            commands::save_briefing_settings,
            commands::latest_briefing,
            commands::brief_now,
            commands::accounts_state,
            commands::connect_account,
            commands::disconnect_account,
            commands::terminal_backend,
            commands::model_catalog,
            commands::conversation_model,
            commands::cost_cap,
            commands::set_cost_cap,
            commands::quick_hotkey,
            commands::set_quick_hotkey,
            commands::hide_quick,
            commands::show_quick,
            commands::open_in_main,
            commands::list_jobs,
            commands::create_job,
            commands::job_action,
            commands::delete_job
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
