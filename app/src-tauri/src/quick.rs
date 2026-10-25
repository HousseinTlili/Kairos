//! The quick box (M8 §3, D-021): a small always-on-top window toggled by a global hotkey.

use std::str::FromStr;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const QUICK_WINDOW: &str = "quick";
pub const HOTKEY_SETTING: &str = "quick_hotkey";
/// Alt+Space is Windows's own window-menu key (and PowerToys Run's), and Ctrl+Alt+Space was already
/// taken on the product owner's PC (Windows refused it at M8 testing), so the default is neither.
pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+K";

/// Checks a hotkey's text the way the plugin will read it ("Ctrl+Alt+K"); a lone key without a
/// modifier would steal that key from every app, so it is refused.
pub fn parse_hotkey(text: &str) -> Result<Shortcut, String> {
    let text = text.trim();
    let has_modifier = text.split('+').count() > 1;
    if !has_modifier {
        return Err("Use at least one of Ctrl, Alt, Shift or Win with the key.".into());
    }
    Shortcut::from_str(text)
        .map_err(|err| format!("\"{text}\" isn't a hotkey Kairos understands ({err})."))
}

pub fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW) {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to(QUICK_WINDOW, "quick-shown", ());
    }
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW) {
        let _ = window.hide();
    }
}

pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let visible = app
        .get_webview_window(QUICK_WINDOW)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible { hide(app) } else { show(app) }
}

/// Replaces the registered hotkey; on failure the old one stays.
pub fn register<R: Runtime>(
    app: &AppHandle<R>,
    old: Option<&str>,
    new: &str,
) -> Result<(), String> {
    let shortcut = parse_hotkey(new)?;
    let manager = app.global_shortcut();
    if let Some(old) = old.and_then(|o| parse_hotkey(o).ok())
        && old != shortcut
    {
        let _ = manager.unregister(old);
    }
    if manager.is_registered(shortcut) {
        return Ok(());
    }
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle(app);
            }
        })
        .map_err(|err| format!("Windows refused {new}: it may be taken by another app ({err})."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_and_common_hotkeys_parse() {
        for text in [DEFAULT_HOTKEY, "Alt+Space", "Ctrl+Shift+K", "Super+J"] {
            assert!(parse_hotkey(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn a_key_without_a_modifier_or_nonsense_is_refused() {
        assert!(parse_hotkey("Space").is_err());
        assert!(parse_hotkey("K").is_err());
        assert!(parse_hotkey("Ctrl+Banana").is_err());
    }
}
