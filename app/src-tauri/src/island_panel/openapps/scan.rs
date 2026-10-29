//! Enumerating the top-level windows a person would call "open apps", and naming them by their
//! executable (or Store AppUserModelID). Titles are checked for emptiness only, never read.
use std::path::Path;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, WPARAM},
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetWindow,
            GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindow,
            IsWindowVisible, PostMessageW, SW_RESTORE, SetForegroundWindow, ShowWindow, WM_CLOSE,
            WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
        },
    },
    core::{BOOL, PWSTR},
};

pub struct Found {
    /// Stable identity: lower-cased exe path or AppUserModelID.
    pub key: String,
    /// Short label for a tooltip (no window title).
    pub name: String,
    /// What the shell can turn into an icon.
    pub parsing_name: String,
    /// The app's top window (the first one in z-order), so a click can bring it forward.
    pub hwnd: isize,
}

/// Window classes that are the shell itself, not apps.
const SHELL_CLASSES: [&str; 4] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    // SAFETY: `buf` is a valid writable buffer for the call.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn cloak_reason(h: HWND) -> u32 {
    let mut v = 0u32;
    // SAFETY: `v` is a valid u32 the size we pass.
    unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            &mut v as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    }
    .map_or(0, |_| v)
}

/// Cloaked windows are not on this virtual desktop (or are hidden by their app, or are the ghost
/// windows Store apps leave behind). The pill shows the current desktop's apps only.
fn on_other_desktop_or_hidden(h: HWND) -> bool {
    cloak_reason(h) != 0
}

fn is_app_window(h: HWND) -> bool {
    // SAFETY: plain queries on a window handle that may have vanished; failures are just "no".
    unsafe {
        if !IsWindowVisible(h).as_bool() || GetWindowTextLengthW(h) == 0 {
            return false;
        }
        let owned = GetWindow(h, GW_OWNER).is_ok_and(|o| !o.is_invalid());
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let tool = ex & WS_EX_TOOLWINDOW.0 != 0 && ex & WS_EX_APPWINDOW.0 == 0;
        if owned || tool || on_other_desktop_or_hidden(h) {
            return false;
        }
    }
    !SHELL_CLASSES.contains(&class_name(h).as_str())
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

fn exe_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed on every path; the buffer is valid for the stated length.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize * 2];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn aumid(pid: u32) -> Option<String> {
    // SAFETY: two-call pattern with a correctly sized buffer; the handle is closed afterwards.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut len = 0u32;
        let _ = GetApplicationUserModelId(h, &mut len, None); // APPMODEL_ERROR_NO_APPLICATION
        let mut out = None;
        if len > 1 {
            let mut buf = vec![0u16; len as usize];
            let rc = GetApplicationUserModelId(h, &mut len, Some(PWSTR(buf.as_mut_ptr())));
            if rc.0 == 0 {
                out = Some(String::from_utf16_lossy(
                    &buf[..(len as usize).saturating_sub(1)],
                ));
            }
        }
        let _ = CloseHandle(h);
        out
    }
}

/// Store apps are drawn inside `ApplicationFrameHost.exe`; the real app owns a child window.
fn real_pid(h: HWND, pid: u32) -> u32 {
    let is_host = exe_path(pid)
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
        })
        .is_some_and(|n| n == "applicationframehost.exe");
    if !is_host {
        return pid;
    }
    struct Ctx {
        host: u32,
        found: u32,
    }
    unsafe extern "system" fn each(child: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the &mut Ctx passed below, alive for the whole enumeration.
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
        let p = pid_of(child);
        if p != 0 && p != ctx.host {
            ctx.found = p;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut ctx = Ctx {
        host: pid,
        found: 0,
    };
    // SAFETY: the callback only touches `ctx`, which outlives the call.
    let _ = unsafe { EnumChildWindows(Some(h), Some(each), LPARAM(&mut ctx as *mut Ctx as isize)) };
    if ctx.found != 0 { ctx.found } else { pid }
}

/// A short label for a Store AppUserModelID like `Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`.
pub fn label_from_aumid(id: &str) -> String {
    let family = id.split('!').next().unwrap_or(id);
    let name = family.split('_').next().unwrap_or(family);
    name.rsplit('.').next().unwrap_or(name).to_string()
}

fn identify(h: HWND, own_pid: u32) -> Option<Found> {
    let pid = real_pid(h, pid_of(h));
    if pid == 0 || pid == own_pid {
        return None;
    }
    if let Some(id) = aumid(pid).filter(|s| !s.is_empty()) {
        return Some(Found {
            key: id.to_lowercase(),
            name: label_from_aumid(&id),
            parsing_name: format!(r"shell:AppsFolder\{id}"),
            hwnd: h.0 as isize,
        });
    }
    let path = exe_path(pid)?;
    let name = Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())?;
    Some(Found {
        key: path.to_lowercase(),
        name,
        parsing_name: path,
        hwnd: h.0 as isize,
    })
}

/// Bring one of the windows found by `open_apps` to the front (restoring it first if minimised).
/// The handle must still be a live, visible, app-like window; anything else is refused, so this can
/// only ever act on the windows that were listed.
pub fn bring_to_front(hwnd: isize) -> Result<(), String> {
    let h = HWND(hwnd as *mut _);
    // SAFETY: plain window-management calls on a handle that is validated first.
    unsafe {
        if !IsWindow(Some(h)).as_bool() || !is_app_window(h) {
            return Err("That window is gone.".into());
        }
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if !SetForegroundWindow(h).as_bool() {
            return Err("Windows would not bring that app forward.".into());
        }
    }
    Ok(())
}

/// Ask one of the listed windows to close, the way its own close button would (WM_CLOSE): the app
/// can still refuse or ask to save. Same validation as `bring_to_front`.
pub fn request_close(hwnd: isize) -> Result<(), String> {
    let h = HWND(hwnd as *mut _);
    // SAFETY: validated handle; PostMessage only queues a message.
    unsafe {
        if !IsWindow(Some(h)).as_bool() || !is_app_window(h) {
            return Err("That window is gone.".into());
        }
        PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)).map_err(|e| e.to_string())
    }
}

/// Every app with a visible top-level window, in z-order (top first), one entry per app.
pub fn open_apps() -> Vec<Found> {
    struct Acc {
        hwnds: Vec<HWND>,
    }
    unsafe extern "system" fn each(h: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the &mut Acc passed below, alive for the whole enumeration.
        let acc = unsafe { &mut *(lparam.0 as *mut Acc) };
        if is_app_window(h) {
            acc.hwnds.push(h);
        }
        BOOL(1)
    }
    let mut acc = Acc { hwnds: Vec::new() };
    // SAFETY: the callback only touches `acc`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut acc as *mut Acc as isize)) };
    let own = std::process::id();
    let mut out: Vec<Found> = Vec::new();
    for h in acc.hwnds {
        if let Some(f) = identify(h, own)
            && !out.iter().any(|o| o.key == f.key)
        {
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focusing_a_handle_that_is_not_a_listed_app_window_is_refused() {
        // A made-up handle and the desktop window are not app windows: nothing is touched.
        assert!(bring_to_front(1).is_err());
        assert!(bring_to_front(0).is_err());
        assert!(request_close(1).is_err());
        assert!(request_close(0).is_err());
    }

    #[test]
    fn store_labels_are_short_and_readable() {
        assert_eq!(
            label_from_aumid("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
            "WindowsCalculator"
        );
        assert_eq!(
            label_from_aumid("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "SpotifyMusic"
        );
        assert_eq!(label_from_aumid("plain"), "plain");
    }

    #[test]
    fn a_live_scan_never_lists_elissa_itself_and_has_unique_keys() {
        let apps = open_apps();
        let mut keys: Vec<_> = apps.iter().map(|a| a.key.clone()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), apps.len());
        let me = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        assert!(apps.iter().all(|a| a.key != me));
    }
}
