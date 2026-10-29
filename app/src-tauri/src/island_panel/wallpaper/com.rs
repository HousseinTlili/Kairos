//! Windows side of the wallpaper module: IDesktopWallpaper (per-monitor wallpaper) and the Shell
//! thumbnail cache (IShellItemImageFactory). All COM calls run on one dedicated apartment thread,
//! fed through a channel, so no other thread ever needs COM initialised.
use super::encode;
use serde::Serialize;
use std::{
    ffi::c_void,
    sync::{
        Mutex,
        mpsc::{Sender, channel},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
            GetDIBits, GetObjectW, HGDIOBJ, ReleaseDC,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        },
        UI::Shell::{
            DesktopWallpaper, IDesktopWallpaper, IShellItem, IShellItemImageFactory,
            SHCreateItemFromParsingName, SIIGBF_THUMBNAILONLY,
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};

const THUMB_W: i32 = 160;
const THUMB_H: i32 = 90;
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Serialize)]
pub struct Monitor {
    /// IDesktopWallpaper's monitor device path; pass it back to `set`.
    pub id: String,
    pub label: String,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    /// Wallpaper currently applied on this monitor (may be empty or a Windows-managed file).
    pub current: String,
}

type Reply<T> = Sender<Result<T, String>>;

enum Job {
    Monitors(Reply<Vec<Monitor>>),
    Thumbnail(String, Reply<String>),
    Set(Option<String>, String, Reply<()>),
}

pub struct Worker {
    tx: Mutex<Sender<Job>>,
}

impl Worker {
    pub fn start() -> Result<Self, String> {
        let (tx, rx) = channel::<Job>();
        thread::Builder::new()
            .name("wallpaper-com".into())
            .spawn(move || {
                // SAFETY: first COM call on this thread; the thread owns the apartment for life.
                let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Monitors(r) => {
                            let _ = r.send(monitors());
                        }
                        Job::Thumbnail(p, r) => {
                            let _ = r.send(thumbnail(&p));
                        }
                        Job::Set(m, p, r) => {
                            let _ = r.send(set_wallpaper(m.as_deref(), &p));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx: Mutex::new(tx) })
    }

    fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Job) -> Result<T, String> {
        let (rtx, rrx) = channel();
        self.tx
            .lock()
            .map_err(|e| e.to_string())?
            .send(make(rtx))
            .map_err(|_| "wallpaper worker stopped".to_string())?;
        rrx.recv_timeout(TIMEOUT)
            .map_err(|_| "wallpaper operation timed out".to_string())?
    }

    pub fn monitors(&self) -> Result<Vec<Monitor>, String> {
        self.call(Job::Monitors)
    }
    pub fn thumbnail(&self, path: &str) -> Result<String, String> {
        self.call(|r| Job::Thumbnail(path.to_string(), r))
    }
    pub fn set(&self, monitor: Option<&str>, path: &str) -> Result<(), String> {
        self.call(|r| Job::Set(monitor.map(str::to_string), path.to_string(), r))
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

fn desktop_wallpaper() -> Result<IDesktopWallpaper, String> {
    // SAFETY: plain COM activation on the COM-initialised worker thread.
    unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }
        .map_err(|e| format!("IDesktopWallpaper unavailable: {e}"))
}

fn monitors() -> Result<Vec<Monitor>, String> {
    let dw = desktop_wallpaper()?;
    let mut out = Vec::new();
    // SAFETY: COM calls on valid interface pointers; returned strings are freed by take_pwstr.
    unsafe {
        let count = dw.GetMonitorDevicePathCount().map_err(|e| e.to_string())?;
        for i in 0..count {
            let Ok(id_ptr) = dw.GetMonitorDevicePathAt(i) else {
                continue;
            };
            let id = take_pwstr(id_ptr);
            if id.is_empty() {
                continue;
            }
            let id_w = wide(&id);
            // Paths for monitors that are no longer attached fail here; skip them.
            let Ok(rect) = dw.GetMonitorRECT(PCWSTR(id_w.as_ptr())) else {
                continue;
            };
            let current = dw
                .GetWallpaper(PCWSTR(id_w.as_ptr()))
                .map(|p| take_pwstr(p))
                .unwrap_or_default();
            let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
            let primary = rect.left == 0 && rect.top == 0;
            out.push(Monitor {
                id,
                label: format!("{}{}", out.len() + 1, if primary { " (main)" } else { "" }),
                width,
                height,
                primary,
                current,
            });
        }
    }
    Ok(out)
}

fn set_wallpaper(monitor: Option<&str>, path: &str) -> Result<(), String> {
    let dw = desktop_wallpaper()?;
    let path_w = wide(path);
    let mon_w = monitor.map(wide);
    let mon = mon_w
        .as_ref()
        .map(|m| PCWSTR(m.as_ptr()))
        .unwrap_or(PCWSTR::null());
    // SAFETY: both wide strings outlive the call and are NUL-terminated.
    unsafe { dw.SetWallpaper(mon, PCWSTR(path_w.as_ptr())) }
        .map_err(|e| format!("could not set wallpaper: {e}"))?;
    wait_until_applied(monitor, path);
    Ok(())
}

/// SetWallpaper returns at once but GetWallpaper keeps reporting the old file for ~0.4 s
/// (measured). Wait, bounded, until it reflects the change, so the state we reply with is true and
/// a quick second "next" steps from the right place. Giving up after the timeout is not an error:
/// the wallpaper was set.
fn wait_until_applied(monitor: Option<&str>, path: &str) {
    const POLL: Duration = Duration::from_millis(40);
    const LIMIT: Duration = Duration::from_millis(2000);
    let start = Instant::now();
    while start.elapsed() < LIMIT {
        let applied = monitors().is_ok_and(|ms| {
            ms.iter()
                .filter(|m| monitor.is_none_or(|id| m.id == id))
                .all(|m| super::images::same_path(&m.current, path))
        });
        if applied {
            return;
        }
        thread::sleep(POLL);
    }
}

fn thumbnail(path: &str) -> Result<String, String> {
    let path_w = wide(path);
    // SAFETY: Shell/GDI calls on valid handles; the HBITMAP is deleted on every path below.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(path_w.as_ptr()), None)
            .map_err(|e| format!("shell item: {e}"))?;
        let factory: IShellItemImageFactory = item.cast().map_err(|e| e.to_string())?;
        let hbmp = factory
            .GetImage(
                SIZE {
                    cx: THUMB_W,
                    cy: THUMB_H,
                },
                SIIGBF_THUMBNAILONLY,
            )
            .map_err(|e| format!("no thumbnail: {e}"))?;

        let result = (|| {
            let mut bm = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(hbmp.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut c_void),
            ) == 0
            {
                return Err("GetObject failed".to_string());
            }
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            if w <= 0 || h <= 0 {
                return Err("empty thumbnail".to_string());
            }
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: h, // positive: bottom-up, which is what the BMP writer expects
                    biPlanes: 1,
                    biBitCount: 24,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0u8; encode::bmp24_row_bytes(w as u32) * h as usize];
            let hdc = GetDC(None);
            let lines = GetDIBits(
                hdc,
                hbmp,
                0,
                h as u32,
                Some(pixels.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            ReleaseDC(None, hdc);
            if lines == 0 {
                return Err("GetDIBits failed".to_string());
            }
            encode::bmp_data_url(w as u32, h as u32, &pixels)
        })();
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        result
    }
}
