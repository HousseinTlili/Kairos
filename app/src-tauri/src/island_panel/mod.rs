//! The island's panel (M15, D-030): wallpaper, media, open apps and pinned apps, ported from Elissa,
//! the product owner's island app on the same stack. Elissa's own module seam is kept (one
//! `island_module` command routes to a module by id), so the ported code stays close to its source;
//! the modules' settings live in `%APPDATA%\kairos\island.db`, apart from Kairos's own database.

pub mod media;
pub mod openapps;
pub mod pinned;
pub mod png;
pub mod registry;
pub mod settings;
pub mod shell_icon;
pub mod wallpaper;

use tauri::{AppHandle, Manager};

pub fn all() -> Vec<Box<dyn registry::Module>> {
    vec![
        media::module(),
        openapps::module(),
        pinned::module(),
        wallpaper::module(),
    ]
}

/// Opens the island's settings and starts the modules (their workers idle until a tab is shown).
pub fn setup(app: &AppHandle) -> Result<(), String> {
    let dir = std::env::var_os("APPDATA")
        .map(|d| std::path::PathBuf::from(d).join("kairos"))
        .ok_or("APPDATA isn't set")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.manage(settings::Settings::open(&dir.join("island.db"))?);
    app.manage(registry::Registry::new(all()));
    app.state::<registry::Registry>().setup_all(app);
    Ok(())
}
