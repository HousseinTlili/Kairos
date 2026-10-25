//! The tray icon and the window's life in it (M3 §8): closing the window hides it, the tray brings
//! it back, and only "Quit" ends Kairos, so alerts and presence keep working in the background.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub const MAIN_WINDOW: &str = "main";

/// Shows, un-minimises and focuses the main window.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Kairos", true, None::<&str>)?;
    let system = MenuItem::with_id(app, "system", "System", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Kairos", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &system, &separator, &quit])?;

    let mut tray = TrayIconBuilder::with_id("kairos")
        .tooltip("Kairos")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "system" => {
                show_main(app);
                // The app switches to its System page.
                let _ = app.emit("navigate", "system");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
