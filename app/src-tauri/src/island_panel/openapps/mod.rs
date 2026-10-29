//! "openapps": the apps that have a window open right now, for the collapsed pill. Elissa lists them
//! and shows their icons, a click brings that app's window to the front, and a right-click then a click
//! on the red x asks it to close (WM_CLOSE: the app can still ask to save). Only a window from the
//! latest scan can be targeted. It does not move, resize or tile anything (CLAUDE.md rule 8).
//! Only the app identity (executable or Store AppUserModelID) and its icon leave this thread; window
//! titles are never read into the list, so page or document names stay private.
mod scan;

use super::{registry::Module, shell_icon};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

const EVENT: &str = "openapps://list";
const POLL: Duration = Duration::from_millis(1500);
const IDLE: Duration = Duration::from_millis(400);
const TICK: Duration = Duration::from_millis(100);
const ICON_PX: i32 = 40;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OpenApp {
    pub key: String,
    pub name: String,
    /// PNG data URL, or null (the pill draws a letter).
    pub icon: Option<String>,
}

struct OpenApps {
    visible: Arc<AtomicBool>,
    last: Arc<Mutex<Vec<OpenApp>>>,
    /// app key -> its top window, from the latest scan. A click can only focus one of these.
    windows: Arc<Mutex<HashMap<String, isize>>>,
}

pub fn module() -> Box<dyn Module> {
    Box::new(OpenApps {
        visible: Arc::new(AtomicBool::new(false)),
        last: Arc::new(Mutex::new(Vec::new())),
        windows: Arc::new(Mutex::new(HashMap::new())),
    })
}

/// `Some(Pending)`-style cache: absent = never asked, `None` = being fetched or failed,
/// `Some(url)` = ready. Icons come from the shell, which can be slow (or stuck) for some apps, so
/// each is fetched on its own thread and the list never waits for it: the pill shows a letter until
/// the icon arrives.
type Icons = Arc<Mutex<HashMap<String, Option<String>>>>;

fn icon_for(
    icons: &Icons,
    dirty: &Arc<AtomicBool>,
    key: &str,
    parsing_name: &str,
) -> Option<String> {
    let Ok(mut cache) = icons.lock() else {
        return None;
    };
    if let Some(known) = cache.get(key) {
        return known.clone();
    }
    cache.insert(key.to_string(), None);
    drop(cache);
    let (icons, dirty) = (icons.clone(), dirty.clone());
    let (key, parsing_name) = (key.to_string(), parsing_name.to_string());
    let _ = thread::Builder::new()
        .name("openapps-icon".into())
        .spawn(move || {
            // SAFETY: first COM call on this short-lived thread.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let url = shell_icon::icon_data_url(&parsing_name, ICON_PX);
            // Only fill the slot if the app is still tracked (it may have closed meanwhile).
            if url.is_some()
                && let Ok(mut c) = icons.lock()
                && let Some(slot) = c.get_mut(&key)
            {
                *slot = url;
                dirty.store(true, Ordering::SeqCst);
            }
        });
    None
}

/// Keep the order stable between scans: apps that were already shown keep their place, new ones go
/// at the end. Otherwise the icons would reshuffle every time you switch windows.
pub fn merge_order(previous: &[String], found: &[String]) -> Vec<String> {
    let mut out: Vec<String> = previous
        .iter()
        .filter(|k| found.contains(k))
        .cloned()
        .collect();
    for k in found {
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    out
}

impl Module for OpenApps {
    fn id(&self) -> &'static str {
        "openapps"
    }

    fn setup(&self, app: &AppHandle) -> Result<(), String> {
        let (app, visible, last, windows) = (
            app.clone(),
            self.visible.clone(),
            self.last.clone(),
            self.windows.clone(),
        );
        let icons: Icons = Arc::new(Mutex::new(HashMap::new()));
        let dirty = Arc::new(AtomicBool::new(false));
        thread::Builder::new()
            .name("openapps".into())
            .spawn(move || {
                let mut order: Vec<String> = Vec::new();
                let mut announced: Option<Vec<OpenApp>> = None;
                let mut last_scan: Option<Instant> = None;
                loop {
                    // Only look while the collapsed pill is on screen (the panel does not show it).
                    if !visible.load(Ordering::SeqCst) {
                        announced = None; // show the current list the moment the pill returns
                        last_scan = None;
                        thread::sleep(IDLE);
                        continue;
                    }
                    let due = last_scan.is_none_or(|t| t.elapsed() >= POLL);
                    if !due && !dirty.swap(false, Ordering::SeqCst) {
                        thread::sleep(TICK);
                        continue;
                    }
                    last_scan = Some(Instant::now());
                    let found = scan::open_apps();
                    if let Ok(mut w) = windows.lock() {
                        *w = found.iter().map(|f| (f.key.clone(), f.hwnd)).collect();
                    }
                    order = merge_order(
                        &order,
                        &found.iter().map(|f| f.key.clone()).collect::<Vec<_>>(),
                    );
                    let list: Vec<OpenApp> = order
                        .iter()
                        .filter_map(|k| found.iter().find(|f| &f.key == k))
                        .map(|f| OpenApp {
                            key: f.key.clone(),
                            name: f.name.clone(),
                            icon: icon_for(&icons, &dirty, &f.key, &f.parsing_name),
                        })
                        .collect();
                    if let Ok(mut c) = icons.lock() {
                        c.retain(|k, _| order.contains(k));
                    }
                    if announced.as_ref() != Some(&list) {
                        if let Ok(mut l) = last.lock() {
                            *l = list.clone();
                        }
                        let _ = app.emit(EVENT, &list);
                        announced = Some(list);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn invoke(&self, _app: &AppHandle, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "set_visible" => {
                let on = args
                    .get("visible")
                    .and_then(Value::as_bool)
                    .ok_or("missing 'visible'")?;
                self.visible.store(on, Ordering::SeqCst);
                Ok(Value::Null)
            }
            // A fresh look at the windows, for actions that name an app (the background scan only
            // runs while the collapsed pill is showing). Same data as the pill, no titles.
            "scan_now" => {
                let found = scan::open_apps();
                if let Ok(mut w) = self.windows.lock() {
                    *w = found.iter().map(|f| (f.key.clone(), f.hwnd)).collect();
                }
                Ok(json!(
                    found
                        .iter()
                        .map(|f| json!({"key": f.key, "name": f.name}))
                        .collect::<Vec<_>>()
                ))
            }
            "focus" | "close" => {
                let key = args
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("missing 'key'")?;
                let hwnd = self
                    .windows
                    .lock()
                    .map_err(|e| e.to_string())?
                    .get(key)
                    .copied()
                    .ok_or("That app is not in the list any more.")?;
                if command == "focus" {
                    scan::bring_to_front(hwnd)?;
                } else {
                    scan::request_close(hwnd)?;
                }
                Ok(Value::Null)
            }
            "list" => Ok(json!(*self.last.lock().map_err(|e| e.to_string())?)),
            other => Err(format!("openapps: unknown command '{other}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn existing_apps_keep_their_place_and_new_ones_append() {
        assert_eq!(
            merge_order(&v(&["a", "b", "c"]), &v(&["c", "x", "a"])),
            v(&["a", "c", "x"])
        );
    }

    #[test]
    fn closed_apps_disappear_and_the_first_scan_uses_scan_order() {
        assert_eq!(merge_order(&v(&["a", "b"]), &v(&["b"])), v(&["b"]));
        assert_eq!(merge_order(&[], &v(&["z", "y"])), v(&["z", "y"]));
        assert!(merge_order(&v(&["a"]), &[]).is_empty());
    }

    #[test]
    fn the_wire_format_has_no_titles() {
        let json = serde_json::to_string(&OpenApp {
            key: "k".into(),
            name: "Notepad".into(),
            icon: None,
        })
        .unwrap();
        assert_eq!(json, r#"{"key":"k","name":"Notepad","icon":null}"#);
    }
}
