//! The Windows side of pinned apps: Start Menu shortcuts and Store apps for the picker, icons through
//! the shell, and launching. All COM runs on one dedicated apartment thread fed through a channel.
//! Targets are never logged; error messages carry the app's display name only.
use super::service::{AppEntry, AppSource, Kind, LaunchError, Launcher};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        mpsc::{Sender, channel},
    },
    thread,
    time::Duration,
};
use windows::{
    Win32::{
        Storage::EnhancedStorage::PKEY_AppUserModel_ID,
        System::Com::{
            CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        },
        UI::{
            Shell::{
                AO_NONE, ApplicationActivationManager, BHID_EnumItems, FOLDERID_AppsFolder,
                IApplicationActivationManager, IEnumShellItems, IShellItem, IShellItem2,
                KF_FLAG_DEFAULT, SHGetKnownFolderItem, SIGDN_NORMALDISPLAY, ShellExecuteW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{Interface, PCWSTR, PWSTR, w},
};

const ICON_PX: i32 = 64;
const TIMEOUT: Duration = Duration::from_secs(20);

enum Job {
    List(Reply<Vec<AppEntry>>),
    Icon(Kind, String, Reply<Option<String>>),
    Launch(Kind, String, Reply<Result<(), LaunchError>>),
}
type Reply<T> = Sender<T>;

pub struct WinPlatform {
    tx: Mutex<Sender<Job>>,
}

impl WinPlatform {
    pub fn start() -> Result<Self, String> {
        let (tx, rx) = channel::<Job>();
        thread::Builder::new()
            .name("pinned-com".into())
            .spawn(move || {
                // SAFETY: first COM call on this thread; it owns the apartment for its lifetime.
                let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::List(r) => {
                            let _ = r.send(list_apps());
                        }
                        Job::Icon(k, t, r) => {
                            let _ = r.send(icon(k, &t));
                        }
                        Job::Launch(k, t, r) => {
                            let _ = r.send(launch(k, &t));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx: Mutex::new(tx) })
    }

    fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Job) -> Option<T> {
        let (rtx, rrx) = channel();
        self.tx.lock().ok()?.send(make(rtx)).ok()?;
        rrx.recv_timeout(TIMEOUT).ok()
    }
}

impl AppSource for WinPlatform {
    fn list(&self) -> Result<Vec<AppEntry>, String> {
        self.call(Job::List)
            .ok_or_else(|| "the app list timed out".to_string())
    }
    fn icon(&self, kind: Kind, target: &str) -> Option<String> {
        self.call(|r| Job::Icon(kind, target.to_string(), r))?
    }
}

impl Launcher for WinPlatform {
    fn launch(&self, kind: Kind, target: &str) -> Result<(), LaunchError> {
        self.call(|r| Job::Launch(kind, target.to_string(), r))
            .unwrap_or_else(|| Err(LaunchError::Failed("the launcher is not responding".into())))
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Take ownership of a COM-allocated string and free it.
unsafe fn take_pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const c_void)) };
    s
}

// ---------- the picker's list ----------

fn start_menu_roots() -> Vec<PathBuf> {
    ["PROGRAMDATA", "APPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|b| PathBuf::from(b).join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

fn scan_lnk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 4 {
                scan_lnk(&p, depth + 1, out);
            }
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
            out.push(p);
        }
    }
}

/// Whether a Start Menu shortcut is something a person would pin (not an uninstaller).
pub fn worth_listing(name: &str) -> bool {
    let n = name.to_lowercase();
    !n.contains("uninstall") && !n.contains("désinstall")
}

fn list_apps() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut lnks = Vec::new();
    for root in start_menu_roots() {
        scan_lnk(&root, 0, &mut lnks);
    }
    for p in lnks {
        let Some(name) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
            continue;
        };
        if !worth_listing(&name) || !seen.insert(name.to_lowercase()) {
            continue;
        }
        let target = p.to_string_lossy().to_string();
        out.push(AppEntry {
            id: format!("c:{}", target.to_lowercase()),
            name,
            kind: Kind::Classic,
            target,
            pinned: false,
        });
    }
    for (name, aumid) in store_apps() {
        if seen.insert(name.to_lowercase()) {
            out.push(AppEntry {
                id: format!("s:{}", aumid.to_lowercase()),
                name,
                kind: Kind::Store,
                target: aumid,
                pinned: false,
            });
        }
    }
    out
}

/// Packaged apps from the shell's "AppsFolder": (display name, AppUserModelID). A packaged app's id
/// has the form `PackageFamilyName!AppId`, which is what tells it apart from a classic app.
fn store_apps() -> Vec<(String, String)> {
    let mut out = Vec::new();
    // SAFETY: COM calls on valid interface pointers on the COM-initialised worker thread; strings
    // are freed by take_pwstr.
    unsafe {
        let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        else {
            return out;
        };
        let Ok(en) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            return out;
        };
        loop {
            let mut items = [None];
            let mut fetched = 0u32;
            if en.Next(&mut items, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = items[0].take() else { break };
            let Ok(name) = item.GetDisplayName(SIGDN_NORMALDISPLAY) else {
                continue;
            };
            let name = take_pwstr(name);
            let Ok(item2) = item.cast::<IShellItem2>() else {
                continue;
            };
            let Ok(id) = item2.GetString(&PKEY_AppUserModel_ID) else {
                continue;
            };
            let id = take_pwstr(id);
            if !name.is_empty() && id.contains('!') {
                out.push((name, id));
            }
        }
    }
    out
}

// ---------- icons ----------

fn parsing_name(kind: Kind, target: &str) -> String {
    match kind {
        Kind::Classic => target.to_string(),
        Kind::Store => format!(r"shell:AppsFolder\{target}"),
    }
}

fn icon(kind: Kind, target: &str) -> Option<String> {
    crate::island_panel::shell_icon::icon_data_url(&parsing_name(kind, target), ICON_PX)
}

// ---------- launching ----------

/// HRESULTs that mean "this app is not there any more".
const MISSING_HRESULTS: [u32; 3] = [0x8007_0002, 0x8007_0003, 0x8007_0490];

fn launch(kind: Kind, target: &str) -> Result<(), LaunchError> {
    match kind {
        Kind::Classic => {
            if !Path::new(target).exists() {
                return Err(LaunchError::Missing);
            }
            let path = wide(target);
            // SAFETY: the wide string outlives the call and is NUL-terminated.
            let h = unsafe {
                ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(path.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            };
            // ShellExecute reports success with a value above 32 and errors with a small code.
            match h.0 as usize {
                2 | 3 => Err(LaunchError::Missing),
                0..=32 => Err(LaunchError::Failed(format!(
                    "Windows could not open it (code {})",
                    h.0 as usize
                ))),
                _ => Ok(()),
            }
        }
        Kind::Store => {
            let id = wide(target);
            // SAFETY: COM activation on the COM-initialised worker thread; `id` is NUL-terminated.
            unsafe {
                let mgr: IApplicationActivationManager =
                    CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_ALL)
                        .map_err(|e| LaunchError::Failed(e.message()))?;
                mgr.ActivateApplication(PCWSTR(id.as_ptr()), PCWSTR::null(), AO_NONE)
                    .map(|_| ())
                    .map_err(|e| {
                        if MISSING_HRESULTS.contains(&(e.code().0 as u32)) {
                            LaunchError::Missing
                        } else {
                            LaunchError::Failed(e.message())
                        }
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstallers_are_not_offered() {
        assert!(worth_listing("Notepad++"));
        assert!(!worth_listing("Uninstall Notepad++"));
        assert!(!worth_listing("Désinstaller Foo"));
        assert!(worth_listing("مفكرة"));
    }

    #[test]
    fn a_missing_classic_target_is_reported_as_missing_without_running_anything() {
        let err = launch(Kind::Classic, r"C:\definitely\not\here\app.lnk").unwrap_err();
        assert_eq!(err, LaunchError::Missing);
    }

    #[test]
    fn parsing_names() {
        assert_eq!(parsing_name(Kind::Classic, r"C:\a.lnk"), r"C:\a.lnk");
        assert_eq!(
            parsing_name(Kind::Store, "Pkg!App"),
            r"shell:AppsFolder\Pkg!App"
        );
    }
}
