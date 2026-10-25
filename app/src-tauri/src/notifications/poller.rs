//! Reads Action Center every 3 s while collection is on (M4 §5), and stores what is new.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::collect::{Access, Collector};
use super::{NotificationSwitch, POLL_INTERVAL};

pub fn start(app: AppHandle) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("kairos-notifications".into())
        .spawn(move || {
            let switch = Arc::clone(&app.state::<Arc<NotificationSwitch>>());
            let mut collector: Option<Collector> = None;
            let mut last_error: Option<String> = None;
            loop {
                std::thread::sleep(POLL_INTERVAL);
                // Checked before every read, so switching off stops collection within one tick.
                if !switch.enabled() || super::access() != Access::Allowed {
                    collector = None;
                    continue;
                }
                let since = switch.since();
                let current = match collector.as_mut() {
                    Some(c) if c.since() == since => c,
                    _ => collector.insert(Collector::new(since)),
                };
                match super::windows::read_all() {
                    Ok(observed) => {
                        last_error = None;
                        let fresh = current.poll(&observed);
                        if !fresh.is_empty() {
                            crate::commands::store_notifications(&app, &fresh);
                        }
                    }
                    Err(err) => {
                        // Logged once per kind of failure, not every 3 s.
                        if last_error.as_deref() != Some(err.as_str()) {
                            eprintln!("kairos: {err}");
                            last_error = Some(err);
                        }
                    }
                }
            }
        })?;
    Ok(())
}
