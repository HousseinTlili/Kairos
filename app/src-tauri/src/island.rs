//! The Kairos island (M14, D-029): a pill at the top-centre of the screen that shows what Kairos is
//! doing, and a panel with the Master's standing Kairos that slides in on the wake word.
//!
//! Both windows are their own size (Elissa's lesson: a transparent window larger than its pill
//! has to be made click-through by polling the cursor), never take the focus when they appear,
//! and stay out of the way of full-screen apps and screen captures.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};

pub const ISLAND_WINDOW: &str = "island";
pub const PANEL_WINDOW: &str = "presence-panel";
pub const ENABLED_SETTING: &str = "island_enabled";
pub const PANEL_SETTING: &str = "island_panel";

/// Logical sizes; tauri.conf.json creates the windows with the same numbers.
pub const PILL_W: f64 = 300.0;
pub const PILL_H: f64 = 40.0;
pub const PANEL_W: f64 = 300.0;
pub const PANEL_H: f64 = 560.0;
const TOP_MARGIN: f64 = 8.0;
const EDGE_MARGIN: f64 = 16.0;
const QUICK_GAP: f64 = 8.0;
/// How often the full-screen check runs: cheap (one shell call), and a game starting or ending
/// is noticed within this.
const WATCH: Duration = Duration::from_secs(2);

/// A monitor's work area (the screen without the taskbar), in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub scale: f64,
}

fn px(logical: f64, scale: f64) -> i32 {
    (logical * scale).round() as i32
}

/// The pill: centred at the top of the work area.
pub fn pill_position(area: Area) -> (i32, i32) {
    (
        area.x + (area.w as i32 - px(PILL_W, area.scale)) / 2,
        area.y + px(TOP_MARGIN, area.scale),
    )
}

/// The wake panel: against the right edge, centred vertically.
pub fn panel_position(area: Area) -> (i32, i32) {
    (
        area.x + area.w as i32 - px(PANEL_W + EDGE_MARGIN, area.scale),
        area.y + (area.h as i32 - px(PANEL_H, area.scale)) / 2,
    )
}

/// The quick box opened from the pill: just under it, centred on it, kept inside the work area.
pub fn quick_position(area: Area, quick_w: u32) -> (i32, i32) {
    let (pill_x, pill_y) = pill_position(area);
    let centre = pill_x + px(PILL_W, area.scale) / 2;
    // A screen narrower than the box: start at its left edge (clamp would panic on min > max).
    let right_most = (area.x + area.w as i32 - quick_w as i32).max(area.x);
    let left = (centre - quick_w as i32 / 2).clamp(area.x, right_most);
    (left, pill_y + px(PILL_H + QUICK_GAP, area.scale))
}

/// Windows's notification state says a full-screen app, a full-screen game or a presentation is
/// in front: `QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`.
pub fn hides_for(notification_state: i32) -> bool {
    matches!(notification_state, 2..=4)
}

#[cfg(windows)]
fn notification_state() -> Option<i32> {
    use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
    let mut state = 0;
    // SAFETY: one out-parameter, written by the call.
    let result = unsafe { SHQueryUserNotificationState(&mut state) };
    (result == 0).then_some(state)
}

#[cfg(not(windows))]
fn notification_state() -> Option<i32> {
    None
}

/// Why the pill is hidden right now, besides the Master's setting.
#[derive(Default)]
pub struct IslandState {
    snoozed_until: Mutex<Option<Instant>>,
    capturing: AtomicBool,
    full_screen: AtomicBool,
}

impl IslandState {
    fn snoozed(&self) -> bool {
        self.snoozed_until
            .lock()
            .ok()
            .and_then(|until| *until)
            .is_some_and(|until| Instant::now() < until)
    }
}

fn area_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Area> {
    let monitor = window.primary_monitor().ok().flatten()?;
    let work = monitor.work_area();
    Some(Area {
        x: work.position.x,
        y: work.position.y,
        w: work.size.width,
        h: work.size.height,
        scale: monitor.scale_factor(),
    })
}

/// Shows or hides the pill from everything that decides it: the setting, the hour off, a
/// full-screen app, a screen capture in progress.
pub fn refresh<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let Some(window) = app.get_webview_window(ISLAND_WINDOW) else {
        return;
    };
    let state = app.state::<IslandState>();
    let visible = enabled
        && !state.snoozed()
        && !state.capturing.load(Ordering::SeqCst)
        && !state.full_screen.load(Ordering::SeqCst);
    if visible {
        if let Some(area) = area_of(&window) {
            let (x, y) = pill_position(area);
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        if !window.is_visible().unwrap_or(false) {
            // Shown without focus: the pill must never take the keyboard from the Master's app.
            let _ = window.set_focusable(false);
            let _ = window.show();
        }
    } else {
        let _ = window.hide();
    }
}

/// The full-screen watch: hides the pill while a full-screen app is in front, shows it after.
pub fn watch<R: Runtime>(
    app: AppHandle<R>,
    enabled: impl Fn(&AppHandle<R>) -> bool + Send + 'static,
) {
    let _ = std::thread::Builder::new()
        .name("kairos-island".into())
        .spawn(move || {
            loop {
                let full = notification_state().is_some_and(hides_for);
                let state = app.state::<IslandState>();
                let changed = state.full_screen.swap(full, Ordering::SeqCst) != full;
                let snooze_over = state
                    .snoozed_until
                    .lock()
                    .ok()
                    .and_then(|mut until| until.take_if(|until| Instant::now() >= *until))
                    .is_some();
                if changed || snooze_over {
                    refresh(&app, enabled(&app));
                }
                std::thread::sleep(WATCH);
            }
        });
}

/// "Hide for an hour" from the pill's menu.
pub fn snooze<R: Runtime>(app: &AppHandle<R>, minutes: u64, enabled: bool) {
    if let Ok(mut until) = app.state::<IslandState>().snoozed_until.lock() {
        *until = Some(Instant::now() + Duration::from_secs(minutes * 60));
    }
    refresh(app, enabled);
}

/// While Kairos captures the screen the pill steps aside, as the quick box does (M8 §4).
pub fn set_capturing<R: Runtime>(app: &AppHandle<R>, capturing: bool, enabled: bool) {
    app.state::<IslandState>()
        .capturing
        .store(capturing, Ordering::SeqCst);
    refresh(app, enabled);
}

/// The quick box, opened just under the pill instead of in the middle of the screen.
pub fn open_quick<R: Runtime>(app: &AppHandle<R>) {
    if let (Some(island), Some(quick)) = (
        app.get_webview_window(ISLAND_WINDOW),
        app.get_webview_window(crate::quick::QUICK_WINDOW),
    ) && let (Some(area), Ok(size)) = (area_of(&island), quick.outer_size())
    {
        let (x, y) = quick_position(area, size.width);
        let _ = quick.set_position(PhysicalPosition::new(x, y));
        let _ = quick.show();
        let _ = quick.set_focus();
        let _ = app.emit_to(crate::quick::QUICK_WINDOW, "quick-shown", ());
        return;
    }
    crate::quick::show(app);
}

/// The wake panel slides in (its page does the sliding; this places and shows the window).
pub fn show_panel<R: Runtime>(app: &AppHandle<R>) {
    if let Some(panel) = app.get_webview_window(PANEL_WINDOW) {
        if let Some(area) = area_of(&panel) {
            let (x, y) = panel_position(area);
            let _ = panel.set_position(PhysicalPosition::new(x, y));
        }
        let _ = panel.set_focusable(false);
        let _ = panel.show();
        let _ = app.emit_to(PANEL_WINDOW, "panel-shown", ());
    }
}

pub fn hide_panel<R: Runtime>(app: &AppHandle<R>) {
    if let Some(panel) = app.get_webview_window(PANEL_WINDOW) {
        let _ = panel.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HD: Area = Area {
        x: 0,
        y: 0,
        w: 1920,
        h: 1032,
        scale: 1.0,
    };

    #[test]
    fn the_pill_sits_at_the_top_centre() {
        assert_eq!(pill_position(HD), (810, 8));
    }

    #[test]
    fn scaling_keeps_it_centred_in_physical_pixels() {
        let hidpi = Area {
            w: 2880,
            h: 1728,
            scale: 1.5,
            ..HD
        };
        // 300 logical px are 450 physical at 150 %.
        assert_eq!(pill_position(hidpi), ((2880 - 450) / 2, 12));
    }

    #[test]
    fn a_monitor_left_of_the_main_one_has_a_negative_origin() {
        let left = Area {
            x: -1920,
            y: 0,
            ..HD
        };
        assert_eq!(pill_position(left), (-1920 + 810, 8));
        assert_eq!(panel_position(left), (-1920 + 1920 - 316, (1032 - 560) / 2));
    }

    #[test]
    fn the_quick_box_opens_under_the_pill_and_inside_the_screen() {
        assert_eq!(quick_position(HD, 640), (960 - 320, 8 + 48));
        let tiny = Area { w: 500, ..HD };
        assert_eq!(quick_position(tiny, 640).0, 0);
    }

    #[test]
    fn full_screen_apps_games_and_presentations_hide_it() {
        assert!(hides_for(2) && hides_for(3) && hides_for(4));
        // Not present (locked), accepts notifications, quiet time, a Windows Store app in front.
        assert!(!hides_for(1) && !hides_for(5) && !hides_for(6) && !hides_for(7));
    }
}
