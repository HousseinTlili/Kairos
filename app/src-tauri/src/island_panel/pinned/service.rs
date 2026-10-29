//! Pinned apps, independent of Windows: the table, the rules, and the traits the real Windows code
//! and the test fakes both implement. Nothing here names a path format or an API.
use crate::island_panel::settings::Settings;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A Start Menu shortcut or executable, opened with ShellExecute.
    Classic,
    /// A Store (packaged) app, started by its AppUserModelID.
    Store,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Classic => "classic",
            Kind::Store => "store",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "classic" => Some(Kind::Classic),
            "store" => Some(Kind::Store),
            _ => None,
        }
    }
}

/// One app the picker can offer. `target` is a path (classic) or an AppUserModelID (store).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    #[serde(skip)]
    pub target: String,
    /// Already pinned (filled in by `Service::catalog`).
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// The shortcut/app is gone (moved, uninstalled).
    Missing,
    /// Anything else the OS reported.
    Failed(String),
}

impl LaunchError {
    pub fn message(&self, name: &str) -> String {
        match self {
            LaunchError::Missing => {
                format!(
                    "{name} could not be found. It may have been moved or uninstalled. Remove the pin or pick it again."
                )
            }
            LaunchError::Failed(why) => format!("{name} did not start: {why}"),
        }
    }
}

/// Where the picker's apps and their icons come from.
pub trait AppSource: Send + Sync {
    fn list(&self) -> Result<Vec<AppEntry>, String>;
    /// A PNG data URL for the app's icon, or `None` if it cannot be extracted.
    fn icon(&self, kind: Kind, target: &str) -> Option<String>;
}

pub trait Launcher: Send + Sync {
    fn launch(&self, kind: Kind, target: &str) -> Result<(), LaunchError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pin {
    pub id: i64,
    pub name: String,
    pub kind: Kind,
    #[serde(skip)]
    pub target: String,
    pub position: i64,
    /// PNG data URL, or null (the UI draws a placeholder).
    pub icon: Option<String>,
}

pub struct Service {
    source: Box<dyn AppSource>,
    launcher: Box<dyn Launcher>,
    /// The picker's last listing. Pins can only be created from entries in here, so the UI never
    /// supplies a launch target.
    catalog: Mutex<Vec<AppEntry>>,
    icons: Mutex<HashMap<String, Option<String>>>,
}

fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pins (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             name     TEXT NOT NULL,
             kind     TEXT NOT NULL,
             target   TEXT NOT NULL UNIQUE,
             position INTEGER NOT NULL
         );",
    )
    .map_err(|e| e.to_string())
}

fn rows(conn: &Connection) -> Result<Vec<Pin>, String> {
    let mut st = conn
        .prepare("SELECT id, name, kind, target, position FROM pins ORDER BY position, id")
        .map_err(|e| e.to_string())?;
    let it = st
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in it {
        let (id, name, kind, target, position) = row.map_err(|e| e.to_string())?;
        if let Some(kind) = Kind::parse(&kind) {
            out.push(Pin {
                id,
                name,
                kind,
                target,
                position,
                icon: None,
            });
        }
    }
    Ok(out)
}

impl Service {
    pub fn new(source: Box<dyn AppSource>, launcher: Box<dyn Launcher>) -> Self {
        Self {
            source,
            launcher,
            catalog: Mutex::new(Vec::new()),
            icons: Mutex::new(HashMap::new()),
        }
    }

    fn icon_for(&self, kind: Kind, target: &str) -> Option<String> {
        if let Some(hit) = self.icons.lock().ok()?.get(target) {
            return hit.clone();
        }
        let icon = self.source.icon(kind, target);
        if let Ok(mut c) = self.icons.lock() {
            c.insert(target.to_string(), icon.clone());
        }
        icon
    }

    pub fn list(&self, db: &Settings) -> Result<Vec<Pin>, String> {
        let mut pins = db.with_conn(|c| {
            init(c)?;
            rows(c)
        })?;
        for p in &mut pins {
            p.icon = self.icon_for(p.kind, &p.target);
        }
        Ok(pins)
    }

    /// Refresh and return the picker's apps (sorted by name).
    pub fn catalog(&self, db: &Settings) -> Result<Vec<AppEntry>, String> {
        let mut apps = self.source.list()?;
        apps.sort_by_key(|a| a.name.to_lowercase());
        *self.catalog.lock().map_err(|e| e.to_string())? = apps.clone();
        let pinned: std::collections::HashSet<String> = db.with_conn(|c| {
            init(c)?;
            Ok(rows(c)?.into_iter().map(|p| p.target).collect())
        })?;
        for a in &mut apps {
            a.pinned = pinned.contains(&a.target);
        }
        Ok(apps)
    }

    /// Pin an app by its picker id. Unknown ids are refused: only listed apps can be pinned.
    pub fn add(&self, db: &Settings, entry_id: &str) -> Result<Vec<Pin>, String> {
        let entry = self
            .catalog
            .lock()
            .map_err(|e| e.to_string())?
            .iter()
            .find(|e| e.id == entry_id)
            .cloned()
            .ok_or("That app is not in the picker list. Open the picker again.")?;
        db.with_conn(|c| {
            init(c)?;
            let next: i64 = c
                .query_row(
                    "SELECT COALESCE(MAX(position), -1) + 1 FROM pins",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            c.execute(
                "INSERT OR IGNORE INTO pins (name, kind, target, position) VALUES (?1, ?2, ?3, ?4)",
                params![entry.name, entry.kind.as_str(), entry.target, next],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })?;
        self.list(db)
    }

    pub fn remove(&self, db: &Settings, pin_id: i64) -> Result<Vec<Pin>, String> {
        db.with_conn(|c| {
            init(c)?;
            c.execute("DELETE FROM pins WHERE id = ?1", params![pin_id])
                .map_err(|e| e.to_string())?;
            renumber(c)
        })?;
        self.list(db)
    }

    /// Move a pin up (`delta` < 0) or down (> 0) by that many places, clamped to the ends.
    pub fn shift(&self, db: &Settings, pin_id: i64, delta: i64) -> Result<Vec<Pin>, String> {
        db.with_conn(|c| {
            init(c)?;
            let mut ids: Vec<i64> = rows(c)?.into_iter().map(|p| p.id).collect();
            let Some(from) = ids.iter().position(|&i| i == pin_id) else {
                return Err("unknown pin".to_string());
            };
            let to = (from as i64 + delta).clamp(0, ids.len() as i64 - 1) as usize;
            let id = ids.remove(from);
            ids.insert(to, id);
            for (pos, id) in ids.iter().enumerate() {
                c.execute(
                    "UPDATE pins SET position = ?1 WHERE id = ?2",
                    params![pos as i64, id],
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(())
        })?;
        self.list(db)
    }

    /// Launch an app from the last picker listing without pinning it (used by "open <app>"). Only
    /// entries the backend itself listed can be opened; the caller supplies an id, never a target.
    pub fn open_listed(&self, entry_id: &str) -> Result<String, LaunchFailure> {
        let entry = self
            .catalog
            .lock()
            .ok()
            .and_then(|c| c.iter().find(|e| e.id == entry_id).cloned())
            .ok_or_else(|| LaunchFailure {
                name: String::new(),
                message: "That app is not in the list any more.".into(),
            })?;
        self.launcher
            .launch(entry.kind, &entry.target)
            .map_err(|e| LaunchFailure {
                message: e.message(&entry.name),
                name: entry.name.clone(),
            })?;
        Ok(entry.name)
    }

    /// Launch an existing pin. The target comes from the table, never from the caller.
    pub fn launch(&self, db: &Settings, pin_id: i64) -> Result<(), LaunchFailure> {
        let pin = db
            .with_conn(|c| {
                init(c)?;
                c.query_row(
                    "SELECT name, kind, target FROM pins WHERE id = ?1",
                    params![pin_id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|e| e.to_string())
            })
            .map_err(|e| LaunchFailure {
                name: String::new(),
                message: e,
            })?;
        let Some((name, kind, target)) = pin else {
            return Err(LaunchFailure {
                name: String::new(),
                message: "That pin no longer exists.".into(),
            });
        };
        let kind = Kind::parse(&kind).ok_or_else(|| LaunchFailure {
            name: name.clone(),
            message: format!("{name} has an unknown kind."),
        })?;
        self.launcher
            .launch(kind, &target)
            .map_err(|e| LaunchFailure {
                message: e.message(&name),
                name,
            })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LaunchFailure {
    pub name: String,
    pub message: String,
}

fn renumber(c: &Connection) -> Result<(), String> {
    for (pos, p) in rows(c)?.iter().enumerate() {
        c.execute(
            "UPDATE pins SET position = ?1 WHERE id = ?2",
            params![pos as i64, p.id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::{Arc, Mutex};

    pub struct FakeSource {
        pub apps: Vec<AppEntry>,
        pub icon_calls: Arc<Mutex<u32>>,
        pub icons_work: bool,
    }
    impl AppSource for FakeSource {
        fn list(&self) -> Result<Vec<AppEntry>, String> {
            Ok(self.apps.clone())
        }
        fn icon(&self, _: Kind, _: &str) -> Option<String> {
            *self.icon_calls.lock().unwrap() += 1;
            self.icons_work
                .then(|| "data:image/png;base64,AAAA".to_string())
        }
    }

    #[derive(Clone, Default)]
    pub struct FakeLauncher {
        pub launched: Arc<Mutex<Vec<(Kind, String)>>>,
        pub fail_with: Arc<Mutex<Option<LaunchError>>>,
    }
    impl Launcher for FakeLauncher {
        fn launch(&self, kind: Kind, target: &str) -> Result<(), LaunchError> {
            if let Some(e) = self.fail_with.lock().unwrap().clone() {
                return Err(e);
            }
            self.launched
                .lock()
                .unwrap()
                .push((kind, target.to_string()));
            Ok(())
        }
    }

    pub fn entry(id: &str, name: &str, kind: Kind, target: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            kind,
            target: target.into(),
            pinned: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::*;
    use super::*;
    use std::sync::Arc;

    struct Rig {
        svc: Service,
        db: Settings,
        launcher: FakeLauncher,
        icon_calls: Arc<Mutex<u32>>,
    }

    fn rig(icons_work: bool) -> Rig {
        let icon_calls = Arc::new(Mutex::new(0));
        let launcher = FakeLauncher::default();
        let source = FakeSource {
            apps: vec![
                entry(
                    "c:notepad",
                    "Notepad",
                    Kind::Classic,
                    r"C:\Start\Notepad.lnk",
                ),
                entry("s:calc", "Calculator", Kind::Store, "Microsoft.Calc!App"),
                entry("c:brave", "Brave", Kind::Classic, r"C:\Start\Brave.lnk"),
            ],
            icon_calls: icon_calls.clone(),
            icons_work,
        };
        Rig {
            svc: Service::new(Box::new(source), Box::new(launcher.clone())),
            db: Settings::in_memory().unwrap(),
            launcher,
            icon_calls,
        }
    }

    fn names(p: &[Pin]) -> Vec<&str> {
        p.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn catalog_is_sorted_and_hides_targets_from_the_wire() {
        let r = rig(true);
        let apps = r.svc.catalog(&r.db).unwrap();
        assert_eq!(
            apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["Brave", "Calculator", "Notepad"]
        );
        let json = serde_json::to_string(&apps).unwrap();
        assert!(!json.contains("Start") && !json.contains("Calc!"), "{json}");
    }

    #[test]
    fn only_apps_from_the_picker_list_can_be_pinned() {
        let r = rig(true);
        assert!(r.svc.add(&r.db, "c:notepad").is_err(), "no listing yet");
        r.svc.catalog(&r.db).unwrap();
        assert!(r.svc.add(&r.db, "c:evil").is_err());
        assert!(r.svc.add(&r.db, r"cmd.exe /c del *").is_err());
        let pins = r.svc.add(&r.db, "c:notepad").unwrap();
        assert_eq!(names(&pins), ["Notepad"]);
    }

    #[test]
    fn adding_the_same_app_twice_keeps_one_pin_and_order_is_append() {
        let r = rig(true);
        r.svc.catalog(&r.db).unwrap();
        r.svc.add(&r.db, "c:notepad").unwrap();
        r.svc.add(&r.db, "s:calc").unwrap();
        let pins = r.svc.add(&r.db, "c:notepad").unwrap();
        assert_eq!(names(&pins), ["Notepad", "Calculator"]);
    }

    #[test]
    fn remove_and_reorder_keep_positions_dense() {
        let r = rig(true);
        r.svc.catalog(&r.db).unwrap();
        for id in ["c:notepad", "s:calc", "c:brave"] {
            r.svc.add(&r.db, id).unwrap();
        }
        let pins = r.svc.list(&r.db).unwrap();
        let brave = pins[2].id;
        let pins = r.svc.shift(&r.db, brave, -1).unwrap();
        assert_eq!(names(&pins), ["Notepad", "Brave", "Calculator"]);
        let pins = r.svc.shift(&r.db, brave, -10).unwrap();
        assert_eq!(
            names(&pins),
            ["Brave", "Notepad", "Calculator"],
            "clamped at the top"
        );
        let pins = r.svc.shift(&r.db, brave, 10).unwrap();
        assert_eq!(
            names(&pins),
            ["Notepad", "Calculator", "Brave"],
            "clamped at the bottom"
        );
        let pins = r.svc.remove(&r.db, pins[0].id).unwrap();
        assert_eq!(names(&pins), ["Calculator", "Brave"]);
        assert_eq!(pins.iter().map(|p| p.position).collect::<Vec<_>>(), [0, 1]);
        assert!(r.svc.shift(&r.db, 9999, 1).is_err());
    }

    #[test]
    fn launch_uses_the_stored_target_and_kind() {
        let r = rig(true);
        r.svc.catalog(&r.db).unwrap();
        r.svc.add(&r.db, "s:calc").unwrap();
        let pin = &r.svc.list(&r.db).unwrap()[0];
        r.svc.launch(&r.db, pin.id).unwrap();
        assert_eq!(
            *r.launcher.launched.lock().unwrap(),
            vec![(Kind::Store, "Microsoft.Calc!App".to_string())]
        );
    }

    #[test]
    fn a_missing_app_gives_a_clear_error_and_unknown_pins_launch_nothing() {
        let r = rig(true);
        r.svc.catalog(&r.db).unwrap();
        r.svc.add(&r.db, "c:notepad").unwrap();
        let id = r.svc.list(&r.db).unwrap()[0].id;
        *r.launcher.fail_with.lock().unwrap() = Some(LaunchError::Missing);
        let err = r.svc.launch(&r.db, id).unwrap_err();
        assert_eq!(err.name, "Notepad");
        assert!(err.message.contains("could not be found") && err.message.contains("Notepad"));
        assert!(!err.message.contains("Start"), "no path in the message");
        *r.launcher.fail_with.lock().unwrap() = Some(LaunchError::Failed("access denied".into()));
        assert!(
            r.svc
                .launch(&r.db, id)
                .unwrap_err()
                .message
                .contains("access denied")
        );
        assert!(r.svc.launch(&r.db, 4242).is_err());
        assert!(r.launcher.launched.lock().unwrap().is_empty());
    }

    #[test]
    fn icons_are_cached_and_a_failure_degrades_to_none() {
        let r = rig(true);
        r.svc.catalog(&r.db).unwrap();
        r.svc.add(&r.db, "c:notepad").unwrap();
        for _ in 0..3 {
            let pins = r.svc.list(&r.db).unwrap();
            assert!(
                pins[0]
                    .icon
                    .as_deref()
                    .unwrap()
                    .starts_with("data:image/png")
            );
        }
        assert_eq!(
            *r.icon_calls.lock().unwrap(),
            1,
            "extracted once, then served from memory"
        );

        let r = rig(false);
        r.svc.catalog(&r.db).unwrap();
        let pins = r.svc.add(&r.db, "c:notepad").unwrap();
        assert_eq!(pins[0].icon, None);
        r.svc.list(&r.db).unwrap();
        assert_eq!(
            *r.icon_calls.lock().unwrap(),
            1,
            "a failed extraction is not retried every time"
        );
    }

    #[test]
    fn open_listed_launches_only_what_the_backend_listed() {
        let r = rig(true);
        assert!(
            r.svc.open_listed("c:notepad").is_err(),
            "nothing listed yet"
        );
        r.svc.catalog(&r.db).unwrap();
        assert_eq!(r.svc.open_listed("s:calc").unwrap(), "Calculator");
        assert_eq!(
            *r.launcher.launched.lock().unwrap(),
            vec![(Kind::Store, "Microsoft.Calc!App".to_string())]
        );
        for evil in ["cmd.exe", r"C:\Windows\System32\cmd.exe", "c:evil", ""] {
            assert!(r.svc.open_listed(evil).is_err(), "{evil}");
        }
        assert_eq!(
            r.launcher.launched.lock().unwrap().len(),
            1,
            "nothing else launched"
        );
        *r.launcher.fail_with.lock().unwrap() = Some(LaunchError::Missing);
        let e = r.svc.open_listed("c:notepad").unwrap_err();
        assert!(e.message.contains("could not be found") && e.name == "Notepad");
    }

    #[test]
    fn non_latin_names_survive() {
        let source = FakeSource {
            apps: vec![entry("c:ar", "مفكرة", Kind::Classic, r"C:\x.lnk")],
            icon_calls: Arc::new(Mutex::new(0)),
            icons_work: true,
        };
        let svc = Service::new(Box::new(source), Box::new(FakeLauncher::default()));
        let db = Settings::in_memory().unwrap();
        svc.catalog(&db).unwrap();
        let pins = svc.add(&db, "c:ar").unwrap();
        assert_eq!(names(&pins), ["مفكرة"]);
    }
}
