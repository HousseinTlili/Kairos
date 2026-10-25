//! The Windows side of the notifications hub (M4 Step 4): `UserNotificationListener`.
//!
//! Only this file talks to Windows; what to keep is decided in `collect.rs`. Windows only.

use std::time::Duration;

use tauri::{AppHandle, Runtime};
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

use super::collect::{Access, Observed};

/// Windows's clock (100 ns since 1601) to Unix seconds.
const UNIX_EPOCH_AS_FILETIME: i64 = 116_444_736_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

fn map_status(status: UserNotificationListenerAccessStatus) -> Access {
    match status {
        UserNotificationListenerAccessStatus::Allowed => Access::Allowed,
        UserNotificationListenerAccessStatus::Denied => Access::Denied,
        _ => Access::Unspecified,
    }
}

/// What Windows currently allows. `Unavailable` if the listener can't even be asked, which is
/// what an unpackaged app gets if Windows keeps it for packaged ones (M4 §5).
pub fn access() -> Access {
    UserNotificationListener::Current()
        .and_then(|listener| listener.GetAccessStatus())
        .map_or(Access::Unavailable, map_status)
}

/// Asks Windows for access (it may show a prompt). The request has to start on the UI thread;
/// the answer is awaited here, off it. Blocking: call it from a background thread.
pub fn request_access<R: Runtime>(app: &AppHandle<R>) -> Access {
    let (tx, rx) = std::sync::mpsc::channel();
    let started = app.run_on_main_thread(move || {
        let operation = UserNotificationListener::Current().and_then(|l| l.RequestAccessAsync());
        let _ = tx.send(operation);
    });
    if started.is_err() {
        return access();
    }
    match rx.recv_timeout(REQUEST_TIMEOUT) {
        Ok(Ok(operation)) => operation.join().map_or_else(|_| access(), map_status),
        _ => access(),
    }
}

fn text(value: windows::core::Result<windows::core::HSTRING>) -> String {
    value.map(|s| s.to_string_lossy()).unwrap_or_default()
}

fn observe(notification: &UserNotification) -> Option<Observed> {
    let windows_id = notification.Id().ok()?;
    let created = notification.CreationTime().ok()?.UniversalTime;
    let (app_id, app_name) = match notification.AppInfo() {
        Ok(info) => (
            text(info.AppUserModelId()),
            info.DisplayInfo()
                .map(|d| text(d.DisplayName()))
                .unwrap_or_default(),
        ),
        Err(_) => (String::new(), String::new()),
    };
    let mut texts = Vec::new();
    if let Ok(binding) = KnownNotificationBindings::ToastGeneric()
        .and_then(|name| notification.Notification()?.Visual()?.GetBinding(&name))
        && let Ok(elements) = binding.GetTextElements()
    {
        for i in 0..elements.Size().unwrap_or(0) {
            if let Ok(element) = elements.GetAt(i) {
                texts.push(text(element.Text()));
            }
        }
    }
    Some(Observed {
        app_id: if app_id.is_empty() {
            app_name.clone()
        } else {
            app_id
        },
        app_name,
        windows_id,
        created_at: (created - UNIX_EPOCH_AS_FILETIME) / TICKS_PER_SECOND,
        texts,
    })
}

/// Every toast in Action Center right now.
pub fn read_all() -> Result<Vec<Observed>, String> {
    let list = UserNotificationListener::Current()
        .and_then(|listener| listener.GetNotificationsAsync(NotificationKinds::Toast))
        .and_then(|operation| operation.join())
        .map_err(|err| format!("Windows didn't return the notifications: {err}"))?;
    let count = list.Size().map_err(|err| err.to_string())?;
    Ok((0..count)
        .filter_map(|i| list.GetAt(i).ok())
        .filter_map(|n| observe(&n))
        .collect())
}
