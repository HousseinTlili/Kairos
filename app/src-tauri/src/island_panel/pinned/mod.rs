//! "pinned": a grid of apps you launch with one click. Pins live in SQLite (name, launch target,
//! order). The picker lists Start Menu shortcuts and Store apps; a pin can only be created from a
//! listed app, and launching takes a pin id, so no command string ever comes from the UI.
mod service;
mod win;

use super::{registry::Module, settings::Settings};
use serde_json::{Value, json};
use service::Service;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager};

struct Pinned {
    service: OnceLock<Service>,
}

pub fn module() -> Box<dyn Module> {
    Box::new(Pinned {
        service: OnceLock::new(),
    })
}

/// The one Windows object serves as both the app list and the launcher.
struct Shared(Arc<win::WinPlatform>);
impl service::AppSource for Shared {
    fn list(&self) -> Result<Vec<service::AppEntry>, String> {
        self.0.list()
    }
    fn icon(&self, kind: service::Kind, target: &str) -> Option<String> {
        self.0.icon(kind, target)
    }
}
impl service::Launcher for Shared {
    fn launch(&self, kind: service::Kind, target: &str) -> Result<(), service::LaunchError> {
        self.0.launch(kind, target)
    }
}

fn id_arg(args: &Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing '{key}'"))
}

impl Module for Pinned {
    fn id(&self) -> &'static str {
        "pinned"
    }

    fn setup(&self, _app: &AppHandle) -> Result<(), String> {
        let platform = Arc::new(win::WinPlatform::start()?);
        let _ = self.service.set(Service::new(
            Box::new(Shared(platform.clone())),
            Box::new(Shared(platform)),
        ));
        Ok(())
    }

    fn invoke(&self, app: &AppHandle, command: &str, args: Value) -> Result<Value, String> {
        let svc = self.service.get().ok_or("pinned apps are not ready yet")?;
        let db = app.state::<Settings>();
        match command {
            "list" => Ok(json!(svc.list(&db)?)),
            "catalog" => Ok(json!(svc.catalog(&db)?)),
            "add" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing 'id'")?;
                Ok(json!(svc.add(&db, id)?))
            }
            "remove" => Ok(json!(svc.remove(&db, id_arg(&args, "id")?)?)),
            "move" => Ok(json!(svc.shift(
                &db,
                id_arg(&args, "id")?,
                id_arg(&args, "delta")?
            )?)),
            "open" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing 'id'")?;
                match svc.open_listed(id) {
                    Ok(name) => Ok(json!(name)),
                    Err(f) => Err(f.message),
                }
            }
            "launch" => match svc.launch(&db, id_arg(&args, "id")?) {
                Ok(()) => Ok(Value::Null),
                // The message is shown in the panel.
                Err(f) => Err(f.message),
            },
            other => Err(format!("pinned: unknown command '{other}'")),
        }
    }
}
