//! Wallpaper: pick images from a configured folder, show thumbnails, set the wallpaper per
//! monitor (or on all monitors) through IDesktopWallpaper, with next/previous.
//!
//! Scope of this first slice: no folder watching, rotation timer, or hotkeys (Phase 2). The
//! folder is read on demand; non-recursive; jpg/jpeg/png/bmp only (webp needs a codec that is
//! not guaranteed to be installed).
mod com;
mod encode;
mod images;

use super::{registry::Module, settings::Settings};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tauri::{AppHandle, Manager};

const FOLDER_KEY: &str = "wallpaper.folder";

struct Wallpaper {
    worker: OnceLock<com::Worker>,
}

pub fn module() -> Box<dyn Module> {
    Box::new(Wallpaper {
        worker: OnceLock::new(),
    })
}

/// Pictures\Wallpapers, else Pictures, else the folder Windows ships its own wallpapers in, so a
/// first run shows something. Overridden by the `wallpaper.folder` setting.
fn default_folder() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE") {
        let pictures = Path::new(&home).join("Pictures");
        let wallpapers = pictures.join("Wallpapers");
        if wallpapers.is_dir() {
            return wallpapers;
        }
        if pictures.is_dir() {
            return pictures;
        }
    }
    PathBuf::from(r"C:\Windows\Web\Wallpaper")
}

/// The shell parses only backslashes ("The parameter is incorrect" for `C:/Users/...`), and a
/// folder typed or pasted with forward slashes gives every image path its slashes (found in
/// Kairos, M15).
fn windows_path(path: &str) -> String {
    path.trim().replace('/', "\\")
}

fn folder(app: &AppHandle) -> PathBuf {
    match app.state::<Settings>().get(FOLDER_KEY) {
        Ok(Some(Value::String(s))) if !s.trim().is_empty() => PathBuf::from(windows_path(&s)),
        _ => default_folder(),
    }
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Only files that sit directly in the configured folder and look like images may be read or
/// applied, whatever the UI asks for.
fn check_in_folder(app: &AppHandle, path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let in_folder = p
        .parent()
        .map(|d| images::same_path(&d.to_string_lossy(), &folder(app).to_string_lossy()))
        .unwrap_or(false);
    if in_folder && images::is_image(p) {
        Ok(())
    } else {
        Err("that file is not in the wallpaper folder".to_string())
    }
}

impl Wallpaper {
    fn worker(&self) -> Result<&com::Worker, String> {
        self.worker
            .get()
            .ok_or_else(|| "wallpaper module not started".to_string())
    }

    fn state(&self, app: &AppHandle) -> Result<Value, String> {
        let dir = folder(app);
        let (images, error) = match images::list(&dir) {
            Ok(v) => (v, None),
            Err(e) => (vec![], Some(e)),
        };
        Ok(json!({
            "folder": dir.to_string_lossy(),
            "images": images,
            "monitors": self.worker()?.monitors()?,
            "error": error,
        }))
    }

    /// Apply the next/previous image. For a single monitor, step from that monitor's current
    /// wallpaper; for all monitors, step from the main monitor's and apply the same image everywhere.
    fn step(&self, app: &AppHandle, monitor: Option<&str>, forward: bool) -> Result<Value, String> {
        let list = images::list(&folder(app))?;
        let monitors = self.worker()?.monitors()?;
        let reference = match monitor {
            Some(id) => monitors.iter().find(|m| m.id == id),
            None => monitors.iter().find(|m| m.primary).or(monitors.first()),
        }
        .ok_or_else(|| "monitor not found".to_string())?;
        let idx = images::step(
            images::position_of(&list, &reference.current),
            list.len(),
            forward,
        )
        .ok_or_else(|| "no images in the wallpaper folder".to_string())?;
        let target = &list[idx];
        self.worker()?.set(monitor, &target.path)?;
        Ok(json!({ "applied": target.path, "monitors": self.worker()?.monitors()? }))
    }
}

impl Module for Wallpaper {
    fn id(&self) -> &'static str {
        "wallpaper"
    }

    fn setup(&self, _app: &AppHandle) -> Result<(), String> {
        let _ = self.worker.set(com::Worker::start()?);
        Ok(())
    }

    fn invoke(&self, app: &AppHandle, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "state" => self.state(app),
            "set_folder" => {
                let path = &windows_path(str_arg(&args, "path").ok_or("path is required")?);
                if !Path::new(path).is_dir() {
                    return Err(format!("'{path}' is not a folder"));
                }
                app.state::<Settings>().set(FOLDER_KEY, &json!(path))?;
                self.state(app)
            }
            "thumbnail" => {
                let path = str_arg(&args, "path").ok_or("path is required")?;
                // The picture Windows shows now may be read from anywhere (it's on screen already:
                // Home shows it behind the hour); anything else only from the folder.
                let on_screen = self
                    .worker()?
                    .monitors()?
                    .iter()
                    .any(|m| images::same_path(&m.current, path));
                if !(on_screen && images::is_image(Path::new(path))) {
                    check_in_folder(app, path)?;
                }
                Ok(json!(self.worker()?.thumbnail(path)?))
            }
            "set" => {
                let path = str_arg(&args, "path").ok_or("path is required")?;
                check_in_folder(app, path)?;
                self.worker()?.set(str_arg(&args, "monitor"), path)?;
                Ok(json!({ "applied": path, "monitors": self.worker()?.monitors()? }))
            }
            "next" => self.step(app, str_arg(&args, "monitor"), true),
            "previous" => self.step(app, str_arg(&args, "monitor"), false),
            other => Err(format!("wallpaper: unknown command '{other}'")),
        }
    }
}

#[cfg(test)]
mod path_tests {
    use super::windows_path;

    #[test]
    fn a_folder_with_forward_slashes_is_given_windows_slashes() {
        assert_eq!(
            windows_path(" C:/Users/U/Desktop/Wallpaper "),
            r"C:\Users\U\Desktop\Wallpaper"
        );
        assert_eq!(windows_path(r"D:\Pictures"), r"D:\Pictures");
    }
}
