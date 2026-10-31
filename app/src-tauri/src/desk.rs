//! The top bar's Windows controls (D-036), the counterparts of Omarchy's bar modules: virtual
//! desktops for its workspaces, the speakers' volume for its audio module, and the Windows panels
//! its other modules open (Wi-Fi, Bluetooth, power). Pure parts are tested; the Windows calls are
//! thin.

use serde::Serialize;

/// The virtual desktops: how many, and which one is in front (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Desktops {
    pub count: usize,
    pub current: usize,
}

/// The default speakers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Volume {
    /// 0–100.
    pub level: u8,
    pub muted: bool,
}

/// Windows lists its desktops as 16-byte ids, one after the other, and names the current one by
/// its id. A current id not in the list (being changed) counts as the first desktop.
pub fn current_index(ids: &[u8], current: &[u8]) -> Desktops {
    let count = (ids.len() / 16).max(1);
    let current = ids
        .as_chunks::<16>()
        .0
        .iter()
        .position(|id| id.as_slice() == current)
        .unwrap_or(0);
    Desktops { count, current }
}

/// The Windows panels the bar's modules open. Only these: the page names one, never a command.
pub fn panel_uri(which: &str) -> Option<&'static str> {
    Some(match which {
        "wifi" => "ms-availablenetworks:",
        "bluetooth" => "ms-settings:bluetooth",
        "sound" => "ms-settings:sound",
        "mixer" => "ms-settings:apps-volume",
        "power" => "ms-settings:powersleep",
        "battery" => "ms-settings:batterysaver",
        "date" => "ms-settings:dateandtime",
        _ => return None,
    })
}

/// How many presses of Ctrl+Win+→ (positive) or ← (negative) go from one desktop to another.
pub fn steps(from: usize, to: usize) -> i32 {
    to as i32 - from as i32
}

#[cfg(windows)]
pub use self::windows_impl::{desktops, open_panel, set_muted, set_volume, switch_desktop, volume};

#[cfg(windows)]
mod windows_impl {
    use super::{Desktops, Volume, current_index, panel_uri, steps};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_CONTROL, VK_LEFT, VK_LWIN, VK_RIGHT,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, w};

    const DESKTOPS_KEY: windows::core::PCWSTR =
        w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops");

    fn binary(value: windows::core::PCWSTR) -> Option<Vec<u8>> {
        let mut size = 0u32;
        // SAFETY: a size query (no buffer), then a read into a buffer of that size.
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                DESKTOPS_KEY,
                value,
                RRF_RT_REG_BINARY,
                None,
                None,
                Some(&mut size),
            )
            .ok()
            .ok()?;
            let mut data = vec![0u8; size as usize];
            RegGetValueW(
                HKEY_CURRENT_USER,
                DESKTOPS_KEY,
                value,
                RRF_RT_REG_BINARY,
                None,
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            )
            .ok()
            .ok()?;
            data.truncate(size as usize);
            Some(data)
        }
    }

    pub fn desktops() -> Desktops {
        match (
            binary(w!("VirtualDesktopIDs")),
            binary(w!("CurrentVirtualDesktop")),
        ) {
            (Some(ids), Some(current)) => current_index(&ids, &current),
            // One desktop that was never split: Windows writes neither value.
            _ => Desktops {
                count: 1,
                current: 0,
            },
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Windows's own shortcut, pressed as many times as it takes: there's no public API for it.
    pub fn switch_desktop(to: usize) -> Result<(), String> {
        let now = desktops();
        if to >= now.count {
            return Err("there is no such desktop".into());
        }
        let n = steps(now.current, to);
        let arrow = if n > 0 { VK_RIGHT } else { VK_LEFT };
        for _ in 0..n.unsigned_abs() {
            let presses = [
                key(VK_CONTROL, false),
                key(VK_LWIN, false),
                key(arrow, false),
                key(arrow, true),
                key(VK_LWIN, true),
                key(VK_CONTROL, true),
            ];
            // SAFETY: an array of fully initialised keyboard inputs.
            let sent = unsafe { SendInput(&presses, std::mem::size_of::<INPUT>() as i32) };
            if sent as usize != presses.len() {
                return Err("Windows didn't take the keys".into());
            }
            // The animation between desktops must start before the next press.
            std::thread::sleep(std::time::Duration::from_millis(180));
        }
        Ok(())
    }

    fn endpoint() -> Result<IAudioEndpointVolume, String> {
        // SAFETY: COM on this (pool) thread, as many times as it's called: a repeat is harmless.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.message())?;
            let speakers = devices
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|_| "no speakers".to_string())?;
            speakers
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .map_err(|e| e.message())
        }
    }

    pub fn volume() -> Result<Volume, String> {
        let endpoint = endpoint()?;
        // SAFETY: plain reads on a live endpoint.
        unsafe {
            let level = endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|e| e.message())?;
            let muted = endpoint.GetMute().map_err(|e| e.message())?.as_bool();
            Ok(Volume {
                level: (level * 100.0).round().clamp(0.0, 100.0) as u8,
                muted,
            })
        }
    }

    pub fn set_volume(level: u8) -> Result<Volume, String> {
        let endpoint = endpoint()?;
        // SAFETY: a level in 0.0–1.0 on a live endpoint; no event context.
        unsafe {
            endpoint
                .SetMasterVolumeLevelScalar(f32::from(level.min(100)) / 100.0, std::ptr::null())
                .map_err(|e| e.message())?;
            // Turning it up means wanting to hear it.
            if level > 0 {
                let _ = endpoint.SetMute(false, std::ptr::null());
            }
        }
        volume()
    }

    pub fn set_muted(muted: bool) -> Result<Volume, String> {
        let endpoint = endpoint()?;
        // SAFETY: as above.
        unsafe {
            endpoint
                .SetMute(muted, std::ptr::null())
                .map_err(|e| e.message())?;
        }
        volume()
    }

    pub fn open_panel(which: &str) -> Result<(), String> {
        let uri = panel_uri(which).ok_or("unknown panel")?;
        // SAFETY: two NUL-terminated strings that outlive the call.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(uri),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute reports success with a value above 32.
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!("Windows couldn't open {uri}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> [u8; 16] {
        [n; 16]
    }

    #[test]
    fn the_current_desktop_is_found_among_the_listed_ones() {
        let ids = [id(1), id(2), id(3)].concat();
        assert_eq!(
            current_index(&ids, &id(3)),
            Desktops {
                count: 3,
                current: 2
            }
        );
        // Changing over: an unknown current id counts as the first.
        assert_eq!(current_index(&ids, &id(9)).current, 0);
        // Never split: one desktop.
        assert_eq!(current_index(&[], &[]).count, 1);
    }

    #[test]
    fn only_the_named_windows_panels_can_be_opened() {
        assert_eq!(panel_uri("wifi"), Some("ms-availablenetworks:"));
        assert_eq!(panel_uri("power"), Some("ms-settings:powersleep"));
        assert_eq!(panel_uri("cmd.exe /c del"), None);
        assert_eq!(panel_uri("ms-settings:sound"), None, "a name, never a URI");
    }

    #[test]
    fn switching_desktops_counts_the_presses_each_way() {
        assert_eq!(steps(0, 2), 2);
        assert_eq!(steps(2, 0), -2);
        assert_eq!(steps(1, 1), 0);
    }
}
