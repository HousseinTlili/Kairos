//! The notifications hub (M4, D-011): Windows notifications read with the user's permission,
//! stored in `kairos.db`, shown in the panel, and summarised on request.

// The collector's poll side runs only in the Windows poller.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod collect;
pub mod export;
#[cfg(windows)]
pub mod poller;
pub mod store;
#[cfg(windows)]
pub mod windows;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use collect::Access;

pub const ENABLED_SETTING: &str = "notifications_enabled";
pub const SINCE_SETTING: &str = "notifications_since";
/// How often Action Center is read while collection is on (M4 §5: "within seconds").
#[cfg(windows)]
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// The Kairos switch as the poller sees it, so it doesn't read `kairos.db` every 3 s. Loaded at
/// startup; changed by `set_notifications_enabled`.
#[derive(Debug, Default)]
pub struct NotificationSwitch {
    enabled: AtomicBool,
    /// Unix seconds when it was last switched on: older notifications are left alone.
    since: AtomicI64,
}

impl NotificationSwitch {
    pub fn new(enabled: bool, since: i64) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            since: AtomicI64::new(since),
        }
    }

    pub fn set(&self, enabled: bool, since: i64) {
        self.since.store(since, Ordering::SeqCst);
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    #[cfg_attr(not(windows), allow(dead_code))] // read by the Windows poller
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn since(&self) -> i64 {
        self.since.load(Ordering::SeqCst)
    }
}

/// Whether Windows lets Kairos read notifications.
#[cfg(windows)]
pub fn access() -> Access {
    windows::access()
}

#[cfg(not(windows))]
pub fn access() -> Access {
    Access::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_off_is_seen_at_once_and_switching_on_moves_the_start() {
        let switch = NotificationSwitch::new(true, 100);
        switch.set(false, 100);
        assert!(!switch.enabled());
        switch.set(true, 500);
        assert!(switch.enabled());
        assert_eq!(switch.since(), 500);
    }

    #[test]
    fn off_this_platform_the_listener_is_unavailable() {
        if !cfg!(windows) {
            assert_eq!(access(), Access::Unavailable);
        }
    }
}
