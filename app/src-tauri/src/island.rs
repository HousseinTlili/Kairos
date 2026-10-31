//! The Kairos island (M14, D-029; the top bar, D-034): a bar across the top of the main screen that
//! shows what Kairos is doing, and a panel with the Master's standing Kairos that slides in on the
//! wake word.
//!
//! The bar stays out of the way: hidden while a window on its screen is maximized, until the mouse
//! touches the top edge (D-034, asked for 2026-10-31), and while a full-screen app or a screen
//! capture is in front. Neither window ever takes the focus when it appears.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

pub const ISLAND_WINDOW: &str = "island";
pub const PANEL_WINDOW: &str = "presence-panel";
pub const ENABLED_SETTING: &str = "island_enabled";
pub const PANEL_SETTING: &str = "island_panel";
pub const AUTOHIDE_SETTING: &str = "island_autohide";
pub const HOTKEY_SETTING: &str = "island_hotkey";
/// Hides and shows the island. Next to the quick box's Ctrl+Alt+K and push-to-talk's Ctrl+Alt+J,
/// so the three sit under one hand (asked for 2026-10-30).
pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+L";

/// Logical heights. The bar is the whole width of the screen; its window is a little taller than
/// the bar for the glow under it.
pub const BAR_H: f64 = 52.0;
pub const BAR_WINDOW_H: f64 = 64.0;
/// The bar opened into its panel (M15): the panel (600 wide, in CSS) hangs under the bar's centre.
pub const EXPANDED_H: f64 = 380.0;
const EXPANDED_GAP: f64 = 6.0;
pub const PANEL_W: f64 = 300.0;
pub const PANEL_H: f64 = 560.0;
const EDGE_MARGIN: f64 = 16.0;
const QUICK_GAP: f64 = 8.0;
/// The watch's beat: the cursor is read this often, so the bar answers the top edge at once.
const TICK: Duration = Duration::from_millis(100);
/// Maximized windows are looked for every few ticks; the full-screen state every 2 s.
const COVER_EVERY: u32 = 3;
const FULL_SCREEN_EVERY: u32 = 20;
/// How long the bar stays after the mouse leaves it, so a pass across it doesn't make it flicker.
pub const HIDE_DELAY: Duration = Duration::from_millis(600);
/// The top edge, in physical pixels from the top of the work area.
const EDGE_PX: i32 = 2;

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

/// The island window's place and size (physical): the full width of the top of the work area;
/// opened, tall enough for the panel under the bar.
pub fn island_rect(area: Area, expanded: bool) -> (i32, i32, u32, u32) {
    let h = if expanded {
        BAR_H + EXPANDED_GAP + EXPANDED_H
    } else {
        BAR_WINDOW_H
    };
    (area.x, area.y, area.w, px(h, area.scale) as u32)
}

/// The wake panel: against the right edge, centred vertically.
pub fn panel_position(area: Area) -> (i32, i32) {
    (
        area.x + area.w as i32 - px(PANEL_W + EDGE_MARGIN, area.scale),
        area.y + (area.h as i32 - px(PANEL_H, area.scale)) / 2,
    )
}

/// The quick box opened from the bar: just under its centre, kept inside the work area.
pub fn quick_position(area: Area, quick_w: u32) -> (i32, i32) {
    let centre = area.x + area.w as i32 / 2;
    // A screen narrower than the box: start at its left edge (clamp would panic on min > max).
    let right_most = (area.x + area.w as i32 - quick_w as i32).max(area.x);
    let left = (centre - quick_w as i32 / 2).clamp(area.x, right_most);
    (left, area.y + px(BAR_H + QUICK_GAP, area.scale))
}

/// Windows's notification state says a full-screen app, a full-screen game or a presentation is
/// in front: `QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`.
pub fn hides_for(notification_state: i32) -> bool {
    matches!(notification_state, 2..=4)
}

/// Whether the bar may show while a maximized window covers its screen (D-034): from the moment
/// the mouse touches the top edge, for as long as it stays on the bar, and `HIDE_DELAY` after.
#[derive(Debug, Default)]
pub struct Reveal {
    revealed: bool,
    left_at: Option<Instant>,
}

impl Reveal {
    /// `covered`: a maximized window is on the bar's screen. `at_edge`: the cursor is at the top
    /// edge. `over_bar`: the cursor is on the bar. `expanded`: the panel is open.
    pub fn tick(
        &mut self,
        now: Instant,
        covered: bool,
        at_edge: bool,
        over_bar: bool,
        expanded: bool,
    ) -> bool {
        if !covered || expanded {
            self.revealed = expanded;
            self.left_at = None;
            return true;
        }
        if at_edge || (self.revealed && over_bar) {
            self.revealed = true;
            self.left_at = None;
            return true;
        }
        if !self.revealed {
            return false;
        }
        let left = *self.left_at.get_or_insert(now);
        if now.duration_since(left) < HIDE_DELAY {
            return true;
        }
        self.revealed = false;
        self.left_at = None;
        false
    }
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

#[cfg(windows)]
mod desktop {
    use windows::Win32::Foundation::{HWND, LPARAM, POINT};
    use windows::Win32::Graphics::Gdi::{
        MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint, MonitorFromWindow,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetCursorPos, IsIconic, IsWindowVisible, IsZoomed,
    };
    use windows::core::BOOL;

    /// A window hidden by Windows (another virtual desktop, a suspended Store app) is "cloaked":
    /// visible to the API, not on screen.
    fn cloaked(hwnd: HWND) -> bool {
        use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
        let mut value = 0u32;
        // SAFETY: a 4-byte out-parameter for DWMWA_CLOAKED, as documented.
        let result = unsafe {
            DwmGetWindowAttribute(
                hwnd.0,
                DWMWA_CLOAKED as u32,
                (&mut value as *mut u32).cast(),
                4,
            )
        };
        result == 0 && value != 0
    }

    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the address of the `bool` in `maximized_on_primary`, alive for the
        // whole EnumWindows call.
        let found = unsafe { &mut *(found.0 as *mut bool) };
        // SAFETY: plain queries on a window handle EnumWindows gave.
        let maximized = unsafe {
            IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() && IsZoomed(hwnd).as_bool()
        };
        if maximized && !cloaked(hwnd) {
            // SAFETY: as above.
            let on_primary = unsafe {
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
                    == MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
            };
            if on_primary {
                *found = true;
                return BOOL(0); // stop: one is enough
            }
        }
        BOOL(1)
    }

    /// A maximized window is on the main screen, where the bar is.
    pub fn maximized_on_primary() -> bool {
        let mut found = false;
        // SAFETY: the callback only writes `found`, which outlives the call.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut found as *mut bool as isize)) };
        found
    }

    pub fn cursor() -> Option<(i32, i32)> {
        let mut point = POINT::default();
        // SAFETY: one out-parameter.
        unsafe { GetCursorPos(&mut point) }.ok()?;
        Some((point.x, point.y))
    }
}

#[cfg(not(windows))]
mod desktop {
    pub fn maximized_on_primary() -> bool {
        false
    }
    pub fn cursor() -> Option<(i32, i32)> {
        None
    }
}

/// Why the bar is hidden right now, besides the Master's setting.
pub struct IslandState {
    snoozed_until: Mutex<Option<Instant>>,
    capturing: AtomicBool,
    full_screen: AtomicBool,
    expanded: AtomicBool,
    /// Hide while a window is maximized (the setting; on by default).
    autohide: AtomicBool,
    /// The reveal's verdict: false while a maximized window covers the screen and the mouse isn't
    /// at the top.
    uncovered: AtomicBool,
}

impl Default for IslandState {
    fn default() -> Self {
        Self {
            snoozed_until: Mutex::new(None),
            capturing: AtomicBool::new(false),
            full_screen: AtomicBool::new(false),
            expanded: AtomicBool::new(false),
            autohide: AtomicBool::new(true),
            uncovered: AtomicBool::new(true),
        }
    }
}

impl IslandState {
    fn snoozed(&self) -> bool {
        self.snoozed_until
            .lock()
            .ok()
            .and_then(|until| *until)
            .is_some_and(|until| Instant::now() < until)
    }

    pub fn set_autohide(&self, on: bool) {
        self.autohide.store(on, Ordering::SeqCst);
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

fn place<R: Runtime>(window: &WebviewWindow<R>, expanded: bool) {
    if let Some(area) = area_of(window) {
        let (x, y, w, h) = island_rect(area, expanded);
        let _ = window.set_size(PhysicalSize::new(w, h));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Shows or hides the bar from everything that decides it: the setting, the hour off, a
/// full-screen app, a screen capture in progress, a maximized window (unless revealed).
pub fn refresh<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let Some(window) = app.get_webview_window(ISLAND_WINDOW) else {
        return;
    };
    let state = app.state::<IslandState>();
    let visible = enabled
        && !state.snoozed()
        && !state.capturing.load(Ordering::SeqCst)
        && !state.full_screen.load(Ordering::SeqCst)
        && state.uncovered.load(Ordering::SeqCst);
    if visible {
        // The size follows the state every time, so the window never stays larger than what shows.
        place(&window, state.expanded.load(Ordering::SeqCst));
        if !window.is_visible().unwrap_or(false) {
            // Shown without focus: the bar must never take the keyboard from the Master's app.
            let _ = window.set_focusable(false);
            let _ = window.show();
            let _ = app.emit_to(ISLAND_WINDOW, "island-shown", ());
        }
    } else {
        let _ = window.hide();
    }
}

/// Registers the hide/show key; on failure the old one stays. `toggle` flips the setting.
pub fn register_hotkey<R: Runtime>(
    app: &AppHandle<R>,
    old: Option<&str>,
    new: &str,
    toggle: impl Fn(&AppHandle<R>) + Send + Sync + 'static,
) -> Result<(), String> {
    crate::quick::replace(app, old, new, move |app, state| {
        if state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            toggle(app);
        }
    })
}

/// The watch: full-screen apps (every 2 s), maximized windows and the cursor at the top edge
/// (D-034). Reading the cursor and listing windows a few times a second costs next to nothing; the
/// window is shown or hidden only when the verdict changes.
pub fn watch<R: Runtime>(
    app: AppHandle<R>,
    enabled: impl Fn(&AppHandle<R>) -> bool + Send + 'static,
) {
    let _ = std::thread::Builder::new()
        .name("kairos-island".into())
        .spawn(move || {
            let mut reveal = Reveal::default();
            let mut covered = false;
            let mut area = None;
            let mut beat: u32 = 0;
            loop {
                let state = app.state::<IslandState>();
                let mut changed = false;
                if beat.is_multiple_of(FULL_SCREEN_EVERY) {
                    let full = notification_state().is_some_and(hides_for);
                    changed |= state.full_screen.swap(full, Ordering::SeqCst) != full;
                    area = app
                        .get_webview_window(ISLAND_WINDOW)
                        .and_then(|w| area_of(&w));
                }
                if beat.is_multiple_of(COVER_EVERY) {
                    covered =
                        state.autohide.load(Ordering::SeqCst) && desktop::maximized_on_primary();
                }
                let (at_edge, over_bar) = match (area, desktop::cursor()) {
                    (Some(a), Some((x, y))) if x >= a.x && x < a.x + a.w as i32 => {
                        (y <= a.y + EDGE_PX, y < a.y + px(BAR_WINDOW_H, a.scale))
                    }
                    _ => (false, false),
                };
                let uncovered = reveal.tick(
                    Instant::now(),
                    covered,
                    at_edge,
                    over_bar,
                    state.expanded.load(Ordering::SeqCst),
                );
                changed |= state.uncovered.swap(uncovered, Ordering::SeqCst) != uncovered;
                let snooze_over = state
                    .snoozed_until
                    .lock()
                    .ok()
                    .and_then(|mut until| until.take_if(|until| Instant::now() >= *until))
                    .is_some();
                if changed || snooze_over {
                    refresh(&app, enabled(&app));
                }
                beat = beat.wrapping_add(1);
                std::thread::sleep(TICK);
            }
        });
}

/// "Hide for an hour" from the bar's menu.
pub fn snooze<R: Runtime>(app: &AppHandle<R>, minutes: u64, enabled: bool) {
    if let Ok(mut until) = app.state::<IslandState>().snoozed_until.lock() {
        *until = Some(Instant::now() + Duration::from_secs(minutes * 60));
    }
    refresh(app, enabled);
}

/// While Kairos captures the screen the bar steps aside, as the quick box does (M8 §4).
pub fn set_capturing<R: Runtime>(app: &AppHandle<R>, capturing: bool, enabled: bool) {
    app.state::<IslandState>()
        .capturing
        .store(capturing, Ordering::SeqCst);
    refresh(app, enabled);
}

/// The quick box, opened just under the bar's centre instead of in the middle of the screen.
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

/// The bar opens its panel or folds back (M15). Opened, the window grows down to hold the panel
/// and takes the focus (the Master clicked it), so a click elsewhere folds it back.
pub fn set_expanded<R: Runtime>(app: &AppHandle<R>, expanded: bool) {
    let Some(window) = app.get_webview_window(ISLAND_WINDOW) else {
        return;
    };
    app.state::<IslandState>()
        .expanded
        .store(expanded, Ordering::SeqCst);
    place(&window, expanded);
    let _ = window.set_focusable(expanded);
    if expanded {
        let _ = window.set_focus();
    }
    let _ = app.emit_to(ISLAND_WINDOW, "island-expanded", expanded);
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
    fn the_bar_spans_the_top_of_the_screen() {
        assert_eq!(island_rect(HD, false), (0, 0, 1920, 64));
        // Opened, it grows down to hold the panel, still full width.
        assert_eq!(island_rect(HD, true), (0, 0, 1920, 52 + 6 + 380));
    }

    #[test]
    fn scaling_works_in_physical_pixels() {
        let hidpi = Area {
            w: 2880,
            h: 1728,
            scale: 1.5,
            ..HD
        };
        assert_eq!(island_rect(hidpi, false), (0, 0, 2880, 96));
    }

    #[test]
    fn a_monitor_left_of_the_main_one_has_a_negative_origin() {
        let left = Area {
            x: -1920,
            y: 0,
            ..HD
        };
        assert_eq!(island_rect(left, false).0, -1920);
        assert_eq!(panel_position(left), (-1920 + 1920 - 316, (1032 - 560) / 2));
    }

    #[test]
    fn the_quick_box_opens_under_the_bar_and_inside_the_screen() {
        assert_eq!(quick_position(HD, 640), (960 - 320, 52 + 8));
        let tiny = Area { w: 500, ..HD };
        assert_eq!(quick_position(tiny, 640).0, 0);
    }

    #[test]
    fn full_screen_apps_games_and_presentations_hide_it() {
        assert!(hides_for(2) && hides_for(3) && hides_for(4));
        // Not present (locked), accepts notifications, quiet time, a Windows Store app in front.
        assert!(!hides_for(1) && !hides_for(5) && !hides_for(6) && !hides_for(7));
    }

    #[test]
    fn with_nothing_maximized_the_bar_always_shows() {
        let mut r = Reveal::default();
        let t = Instant::now();
        assert!(r.tick(t, false, false, false, false));
    }

    #[test]
    fn a_maximized_window_hides_it_until_the_mouse_touches_the_top() {
        let mut r = Reveal::default();
        let t = Instant::now();
        assert!(!r.tick(t, true, false, false, false));
        assert!(
            r.tick(t, true, true, true, false),
            "the top edge reveals it"
        );
        // Moving down onto the bar keeps it.
        assert!(r.tick(t + Duration::from_secs(1), true, false, true, false));
    }

    #[test]
    fn it_hides_a_moment_after_the_mouse_leaves_not_at_once() {
        let mut r = Reveal::default();
        let t = Instant::now();
        r.tick(t, true, true, true, false);
        assert!(r.tick(t, true, false, false, false), "just left");
        assert!(r.tick(t + Duration::from_millis(300), true, false, false, false));
        assert!(!r.tick(t + Duration::from_millis(700), true, false, false, false));
        // Gone: moving over where the bar was doesn't bring it back; only the edge does.
        assert!(!r.tick(t + Duration::from_millis(800), true, false, true, false));
    }

    #[test]
    fn an_open_panel_stays_while_a_window_is_maximized() {
        let mut r = Reveal::default();
        let t = Instant::now();
        assert!(r.tick(t, true, false, false, true));
    }
}
