//! Module registry. A module is one Rust command set; its UI half lives in src/modules/<id>.
//!
//! Tauri fixes its command list at build time, so modules do not register `#[tauri::command]`s.
//! They implement `Module::invoke`, and the single `island_module` command routes to them by id.
//! Ported from Elissa (M15); Kairos lists its four modules in `island_panel::all()`.
use serde_json::Value;
use tauri::{AppHandle, State};

pub trait Module: Send + Sync {
    /// Must match the `id` of the module's React widget.
    fn id(&self) -> &'static str;

    /// Called once at startup, after settings and the window exist. Start background tasks here.
    fn setup(&self, _app: &AppHandle) -> Result<(), String> {
        Ok(())
    }

    /// Handle one command. Keep it quick; spawn work for anything slow.
    fn invoke(&self, app: &AppHandle, command: &str, args: Value) -> Result<Value, String>;
}

pub struct Registry {
    modules: Vec<Box<dyn Module>>,
}

impl Registry {
    pub fn new(modules: Vec<Box<dyn Module>>) -> Self {
        Self { modules }
    }

    #[cfg(test)]
    pub fn ids(&self) -> Vec<&'static str> {
        self.modules.iter().map(|m| m.id()).collect()
    }

    pub fn setup_all(&self, app: &AppHandle) {
        for m in &self.modules {
            if let Err(e) = m.setup(app) {
                eprintln!("module '{}' failed to start: {e}", m.id());
            }
        }
    }

    pub fn dispatch(
        &self,
        app: &AppHandle,
        module: &str,
        command: &str,
        args: Value,
    ) -> Result<Value, String> {
        let m = self
            .modules
            .iter()
            .find(|m| m.id() == module)
            .ok_or_else(|| format!("unknown module '{module}'"))?;
        m.invoke(app, command, args)
    }
}

#[tauri::command]
pub async fn island_module(
    app: AppHandle,
    registry: State<'_, Registry>,
    module: String,
    command: String,
    args: Option<Value>,
) -> Result<Value, String> {
    registry.dispatch(&app, &module, &command, args.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl Module for Echo {
        fn id(&self) -> &'static str {
            "echo"
        }
        fn invoke(&self, _: &AppHandle, _: &str, args: Value) -> Result<Value, String> {
            Ok(args)
        }
    }

    #[test]
    fn ids_lists_registered_modules() {
        let r = Registry::new(vec![Box::new(Echo)]);
        assert_eq!(r.ids(), vec!["echo"]);
    }
}
