mod accounts;
mod audit;
mod automations;
mod briefing;
mod commands;
mod desk;
mod net;
// Public so the fixture tests in tests/ can drive it.
pub mod hermes;
mod island;
mod island_panel;
mod keyring;
mod media;
mod memory;
mod models;
mod notifications;
mod persona;
mod presence;
mod providers;
mod quick;
mod rules;
mod store;
mod system;
mod titlebar;
mod tray;
mod voice;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use automations::{Happening, engine as automation_engine, watch::SnapshotWatch};
use presence::{PresenceEvent, PresenceKind};
use system::alerts::{Alerts, Evaluation};
use system::{AlertSwitch, SystemMonitor};

/// The argument Kairos's login entry starts it with: start hidden in the tray.
const AUTOSTART_ARG: &str = "--autostart";

/// What a presence event means to the automations (M17): a login start, or a return after a while.
fn presence_happening(event: &PresenceEvent) -> Option<Happening> {
    match event.kind {
        PresenceKind::Startup if event.detail.as_deref() == Some("autostart") => {
            Some(Happening::Login)
        }
        PresenceKind::Resume | PresenceKind::Unlock => {
            event.away_s.map(|away_s| Happening::Back { away_s })
        }
        PresenceKind::Startup => None,
    }
}

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
        .manage(island::IslandState::default())
        // Before setup: the page reports its UTC offset as soon as it loads, which can be before
        // setup gets this far (a panic seen 2026-10-31 on a fast restart of the dev app).
        .manage(Arc::new(briefing::BriefingClock::default()))
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
            if let Some(main) = app.get_webview_window(tray::MAIN_WINDOW) {
                titlebar::paint(&main);
            }
            let hotkey = commands::stored_hotkey(&app.state::<commands::AppState>());
            if let Err(err) = quick::register(app.handle(), None, &hotkey) {
                eprintln!("kairos: {err}");
            }
            // The island (M14): shown from the start unless switched off, out of the way of
            // full-screen apps.
            app.state::<island::IslandState>().set_autohide(
                commands::island_settings(&app.state::<commands::AppState>()).autohide,
            );
            island::refresh(app.handle(), commands::island_enabled(app.handle()));
            island::watch(app.handle().clone(), commands::island_enabled);
            let island_key = commands::stored_island_hotkey(&app.state::<commands::AppState>());
            if let Err(err) =
                island::register_hotkey(app.handle(), None, &island_key, commands::toggle_island)
            {
                eprintln!("kairos: the island's hotkey: {err}");
            }
            if let Err(err) = island_panel::setup(app.handle()) {
                eprintln!("kairos: the island's panel: {err}");
            }
            let ptt = commands::stored_ptt_hotkey(&app.state::<commands::AppState>());
            if let Err(err) = quick::register_ptt(app.handle(), None, &ptt) {
                eprintln!("kairos: push-to-talk: {err}");
            }
            // The voice models are freed after a while without speech (M9 §3, Step 8).
            let voice = Arc::new(voice::VoiceState::default());
            app.manage(Arc::clone(&voice));
            std::thread::Builder::new()
                .name("kairos-voice-idle".into())
                .spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(60));
                        voice.engine.unload_if_idle();
                        voice.voice.unload_if_idle();
                    }
                })?;
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
            commands::on_presence(app.handle(), &startup);
            if let Some(happening) = presence_happening(&startup) {
                automation_engine::on_happening(app.handle(), happening);
            }
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
                    if let Some(happening) = presence_happening(&event) {
                        automation_engine::on_happening(&handle, happening);
                    }
                },
                move |now| commands::set_last_seen(&heartbeat.state::<commands::AppState>(), now),
            )?;

            let enabled = commands::stored_alerts_enabled(&app.state::<commands::AppState>());
            let switch = Arc::new(AlertSwitch(AtomicBool::new(enabled)));
            app.manage(Arc::clone(&switch));

            let handle = app.handle().clone();
            let mut alerts = Alerts::for_this_pc();
            let mut watch = SnapshotWatch::default();
            app.state::<Arc<SystemMonitor>>().start(move |snapshot| {
                let _ = handle.emit("system-snapshot", snapshot);
                // Evaluated even while switched off, so turning alerts on doesn't replay old ones.
                let evaluation = alerts.evaluate(snapshot);
                if switch.enabled() {
                    announce(&handle, &evaluation);
                }
                // M17: the charger, the battery and Wi-Fi from two snapshots; a full disk from
                // its alert (whether or not alerts are shown).
                let mut happenings = watch.observe(snapshot);
                happenings.extend(
                    evaluation
                        .fired
                        .iter()
                        .filter(|a| a.kind == system::alerts::AlertKind::DiskFull)
                        .map(|a| Happening::DiskFull {
                            message: a.message.clone(),
                        }),
                );
                for happening in happenings {
                    automation_engine::on_happening(&handle, happening);
                }
            })?;

            // M17: program crashes from the Application event log.
            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                let mut recent = std::collections::HashMap::new();
                automations::crash::watch(move |crash| {
                    let ignore = handle
                        .state::<commands::AppState>()
                        .with_store(|store| store.crash_ignore())
                        .unwrap_or_default();
                    if automations::crash::worth_telling(
                        &crash,
                        presence::unix_now(),
                        &ignore,
                        &mut recent,
                    ) {
                        automation_engine::on_happening(&handle, Happening::Crash(crash));
                    }
                })?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::reload_key,
            commands::start_hermes,
            commands::set_start_hermes,
            commands::create_run,
            commands::subscribe_run,
            commands::answer_approval,
            commands::stop_run,
            commands::resume_open_tasks,
            commands::list_conversations,
            commands::conversation_tasks,
            commands::delete_conversation,
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
            commands::delete_job,
            commands::voice_state,
            commands::save_voice_settings,
            commands::download_voice_model,
            commands::delete_voice_model,
            commands::transcribe,
            commands::ptt_hotkey,
            commands::set_ptt_hotkey,
            commands::persona_state,
            commands::save_persona,
            commands::restore_persona,
            commands::save_speech_settings,
            commands::install_natural_voice,
            commands::remove_natural_voice,
            commands::speak_natural,
            commands::warm_natural_voice,
            commands::memory_state,
            commands::save_wake_settings,
            commands::install_wake_word,
            commands::wake_feed,
            commands::wake_transcribe,
            commands::wake_cancel,
            commands::media_image,
            commands::save_image,
            commands::island_state,
            commands::save_island_settings,
            commands::island_open_quick,
            commands::island_push_to_talk,
            commands::island_snooze,
            commands::island_capturing,
            commands::hide_presence_panel,
            commands::show_main_window,
            commands::island_expand,
            commands::island_fit,
            commands::bar_state,
            commands::bar_switch_desktop,
            commands::bar_set_volume,
            commands::bar_set_muted,
            commands::bar_open,
            commands::providers_state,
            commands::add_provider_key,
            commands::add_provider_endpoint,
            commands::remove_provider,
            commands::restart_hermes,
            commands::wifi_state,
            commands::wifi_set,
            commands::wifi_connect,
            commands::wifi_disconnect,
            commands::bluetooth_state,
            commands::bluetooth_set,
            commands::set_island_hotkey,
            commands::appearance_state,
            commands::save_appearance,
            commands::paint_titlebar,
            island_panel::registry::island_module,
            automations::api::automations_state,
            automations::api::save_automation,
            automations::api::delete_automation,
            automations::api::set_automations_enabled,
            automations::api::set_crash_ignore,
            automations::api::test_automation,
            automations::api::automation_outcome
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
