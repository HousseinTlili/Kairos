//! The top bar's own Wi-Fi and Bluetooth menus (D-037). Wi-Fi: the radio, the network in use, the
//! saved networks and, when Windows lets desktop apps see them, the nearby ones; connecting and
//! disconnecting. Bluetooth: the radio and the paired devices.
//!
//! Since Windows 11 24H2, nearby networks' names are given to desktop apps only while location is
//! on (Settings ▸ Privacy & security ▸ Location). With it off, the network in use (through the
//! connection profile) and the saved networks still show; the menu says why the rest doesn't.
//! Windows has no public API for an app to connect a paired Bluetooth device: that stays in its
//! settings, one click away.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WifiNetwork {
    pub ssid: String,
    /// 0–4, as Windows draws them.
    pub bars: u8,
    pub secured: bool,
    /// Windows has a profile for it: a click connects without a password.
    pub saved: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WifiState {
    /// There is a Wi-Fi adapter at all.
    pub adapter: bool,
    /// The radio; `None` if Windows didn't say.
    pub on: Option<bool>,
    /// The network in use, and its signal (0–4).
    pub connected: Option<(String, u8)>,
    /// The saved networks, by name.
    pub saved: Vec<String>,
    /// Nearby networks; `None` while Windows hides them (location off).
    pub nearby: Option<Vec<WifiNetwork>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BluetoothDevice {
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BluetoothState {
    pub adapter: bool,
    pub on: Option<bool>,
    pub devices: Vec<BluetoothDevice>,
}

/// Windows's signal quality (0–100) as its bars (0–4).
pub fn bars(quality: u32) -> u8 {
    match quality {
        0..=4 => 0,
        5..=29 => 1,
        30..=54 => 2,
        55..=79 => 3,
        _ => 4,
    }
}

/// One line per network: the strongest of its access points, connected first, then by signal, then
/// by name. Hidden networks (no name) are left out.
pub fn merge_nearby(mut seen: Vec<WifiNetwork>) -> Vec<WifiNetwork> {
    seen.retain(|n| !n.ssid.trim().is_empty());
    seen.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.bars.cmp(&a.bars))
            .then(a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
    });
    let mut out: Vec<WifiNetwork> = Vec::new();
    for n in seen {
        if let Some(kept) = out.iter_mut().find(|k| k.ssid == n.ssid) {
            kept.saved |= n.saved;
        } else {
            out.push(n);
        }
    }
    out
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The Windows profile for a network joined from the menu: WPA2-Personal with its password, or
/// open. Saved for this user only, connecting automatically next time, as Windows's own menu does.
pub fn profile_xml(ssid: &str, password: Option<&str>) -> String {
    let name = xml_escape(ssid);
    let security = match password {
        Some(key) => format!(
            "<authEncryption><authentication>WPA2PSK</authentication><encryption>AES</encryption>\
             <useOneX>false</useOneX></authEncryption><sharedKey><keyType>passPhrase</keyType>\
             <protected>false</protected><keyMaterial>{}</keyMaterial></sharedKey>",
            xml_escape(key)
        ),
        None => {
            "<authEncryption><authentication>open</authentication><encryption>none</encryption>\
                 <useOneX>false</useOneX></authEncryption>"
                .to_string()
        }
    };
    format!(
        "<?xml version=\"1.0\"?><WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\">\
         <name>{name}</name><SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>\
         <connectionType>ESS</connectionType><connectionMode>auto</connectionMode>\
         <MSM><security>{security}</security></MSM></WLANProfile>"
    )
}

/// A WPA2 passphrase is 8–63 characters.
pub fn password_problem(password: &str) -> Option<&'static str> {
    match password.chars().count() {
        0..=7 => Some("A Wi-Fi password has at least 8 characters."),
        64.. => Some("A Wi-Fi password has at most 63 characters."),
        _ => None,
    }
}

#[cfg(windows)]
pub use self::windows_impl::{
    bluetooth_state, connected, set_bluetooth, set_wifi, wifi_connect, wifi_disconnect, wifi_state,
};

#[cfg(windows)]
mod windows_impl {
    use super::{
        BluetoothDevice, BluetoothState, WifiNetwork, WifiState, bars, merge_nearby,
        password_problem, profile_xml,
    };
    use windows::Devices::Bluetooth::{
        BluetoothConnectionStatus, BluetoothDevice as ClassicDevice, BluetoothLEDevice,
    };
    use windows::Devices::Enumeration::DeviceInformation;
    use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
    use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        DOT11_SSID, WLAN_AVAILABLE_NETWORK_LIST, WLAN_CONNECTION_PARAMETERS,
        WLAN_INTERFACE_INFO_LIST, WLAN_PROFILE_INFO_LIST, WlanCloseHandle, WlanConnect,
        WlanDisconnect, WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList,
        WlanGetProfileList, WlanOpenHandle, WlanScan, WlanSetProfile, dot11_BSS_type_any,
        wlan_connection_mode_profile,
    };
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
    use windows::core::{GUID, HSTRING, PCWSTR};

    const ERROR_ACCESS_DENIED: u32 = 5;
    /// `WLAN_AVAILABLE_NETWORK_CONNECTED`.
    const NETWORK_CONNECTED: u32 = 1;
    /// `WLAN_AVAILABLE_NETWORK_HAS_PROFILE`.
    const NETWORK_HAS_PROFILE: u32 = 2;

    fn winrt() {
        // SAFETY: once per (pool) thread is enough; a repeat only reports "already initialised".
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }

    // --- Radios ---------------------------------------------------------------------------------

    fn radio(kind: RadioKind) -> Option<Radio> {
        winrt();
        let radios = Radio::GetRadiosAsync().ok()?.join().ok()?;
        radios
            .into_iter()
            .find(|r| r.Kind().is_ok_and(|k| k == kind))
    }

    fn radio_on(kind: RadioKind) -> Option<bool> {
        radio(kind)
            .and_then(|r| r.State().ok())
            .map(|s| s == RadioState::On)
    }

    fn set_radio(kind: RadioKind, on: bool) -> Result<(), String> {
        let radio = radio(kind).ok_or("there is no such radio on this PC")?;
        let access = Radio::RequestAccessAsync()
            .and_then(|op| op.join())
            .map_err(|e| e.message())?;
        if access != RadioAccessStatus::Allowed {
            return Err("Windows doesn't let apps turn this radio on or off \
                        (Settings ▸ Privacy & security ▸ Radios)"
                .into());
        }
        let state = if on { RadioState::On } else { RadioState::Off };
        let done = radio
            .SetStateAsync(state)
            .and_then(|op| op.join())
            .map_err(|e| e.message())?;
        if done == RadioAccessStatus::Allowed {
            Ok(())
        } else {
            Err("Windows refused to change the radio".into())
        }
    }

    pub fn set_wifi(on: bool) -> Result<(), String> {
        set_radio(RadioKind::WiFi, on)
    }

    pub fn set_bluetooth(on: bool) -> Result<(), String> {
        set_radio(RadioKind::Bluetooth, on)
    }

    // --- Wi-Fi ----------------------------------------------------------------------------------

    /// The Native Wi-Fi client handle and the first wireless adapter, closed when dropped.
    struct Wlan {
        handle: HANDLE,
        adapter: Option<GUID>,
    }

    impl Wlan {
        fn open() -> Result<Self, String> {
            let mut version = 0u32;
            let mut handle = HANDLE::default();
            // SAFETY: two out-parameters written by the call.
            let code = unsafe { WlanOpenHandle(2, None, &mut version, &mut handle) };
            if code != 0 {
                return Err(format!(
                    "Windows's Wi-Fi service didn't answer (code {code})"
                ));
            }
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            // SAFETY: the list is allocated by WLAN and freed below.
            let adapter = unsafe {
                if WlanEnumInterfaces(handle, None, &mut list) == 0 && !list.is_null() {
                    let first = ((*list).dwNumberOfItems > 0)
                        .then(|| (*list).InterfaceInfo[0].InterfaceGuid);
                    WlanFreeMemory(list.cast());
                    first
                } else {
                    None
                }
            };
            Ok(Self { handle, adapter })
        }

        fn adapter(&self) -> Result<GUID, String> {
            self.adapter
                .ok_or_else(|| "this PC has no Wi-Fi adapter".to_string())
        }

        fn saved(&self) -> Vec<String> {
            let Some(guid) = self.adapter else {
                return Vec::new();
            };
            let mut list: *mut WLAN_PROFILE_INFO_LIST = std::ptr::null_mut();
            // SAFETY: as above; the items are read within the list's own count.
            unsafe {
                if WlanGetProfileList(self.handle, &guid, None, &mut list) != 0 || list.is_null() {
                    return Vec::new();
                }
                let count = (*list).dwNumberOfItems as usize;
                let items = std::slice::from_raw_parts((*list).ProfileInfo.as_ptr(), count);
                let names = items.iter().map(|p| from_wide(&p.strProfileName)).collect();
                WlanFreeMemory(list.cast());
                names
            }
        }

        /// `None` when Windows hides nearby networks (location off).
        fn nearby(&self) -> Option<Vec<WifiNetwork>> {
            let guid = self.adapter?;
            let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
            // SAFETY: as above.
            unsafe {
                let code = WlanGetAvailableNetworkList(self.handle, &guid, 0, None, &mut list);
                if code == ERROR_ACCESS_DENIED || list.is_null() {
                    return None;
                }
                if code != 0 {
                    return Some(Vec::new());
                }
                let count = (*list).dwNumberOfItems as usize;
                let items = std::slice::from_raw_parts((*list).Network.as_ptr(), count);
                let seen = items
                    .iter()
                    .map(|n| {
                        let len = (n.dot11Ssid.uSSIDLength as usize).min(32);
                        WifiNetwork {
                            ssid: String::from_utf8_lossy(&n.dot11Ssid.ucSSID[..len]).into_owned(),
                            bars: bars(n.wlanSignalQuality),
                            secured: n.bSecurityEnabled.as_bool(),
                            saved: n.dwFlags & NETWORK_HAS_PROFILE != 0,
                            connected: n.dwFlags & NETWORK_CONNECTED != 0,
                        }
                    })
                    .collect();
                WlanFreeMemory(list.cast());
                Some(merge_nearby(seen))
            }
        }
    }

    impl Drop for Wlan {
        fn drop(&mut self) {
            // SAFETY: the handle from WlanOpenHandle, closed once.
            unsafe { WlanCloseHandle(self.handle, None) };
        }
    }

    /// The network in use, from its connection profile: given even while location is off.
    pub fn connected() -> Option<(String, u8)> {
        winrt();
        let profiles = NetworkInformation::GetConnectionProfiles().ok()?;
        profiles.into_iter().find_map(|p| {
            let wlan = p.IsWlanConnectionProfile().ok()?;
            let level = p.GetNetworkConnectivityLevel().ok()?;
            if !wlan || level == NetworkConnectivityLevel::None {
                return None;
            }
            let ssid = p
                .WlanConnectionProfileDetails()
                .and_then(|d| d.GetConnectedSsid())
                .ok()?
                .to_string();
            let signal = p
                .GetSignalBars()
                .and_then(|r| r.Value())
                .unwrap_or(0)
                .min(4);
            Some((ssid, signal))
        })
    }

    pub fn wifi_state(scan: bool) -> WifiState {
        let wlan = Wlan::open().ok();
        let adapter = wlan.as_ref().is_some_and(|w| w.adapter.is_some());
        if scan
            && let Some(w) = &wlan
            && let Some(guid) = w.adapter
        {
            // SAFETY: a request to refresh the list; the result arrives within a few seconds.
            unsafe { WlanScan(w.handle, &guid, None, None, None) };
        }
        WifiState {
            adapter,
            on: radio_on(RadioKind::WiFi),
            connected: connected(),
            saved: wlan.as_ref().map(|w| w.saved()).unwrap_or_default(),
            nearby: wlan.as_ref().and_then(|w| w.nearby()),
        }
    }

    /// Joins a network: a saved one by its profile, a new one with a profile made from its
    /// password (or open). Windows reports the outcome later; the menu re-reads the state.
    pub fn wifi_connect(ssid: &str, password: Option<&str>) -> Result<(), String> {
        let wlan = Wlan::open()?;
        let guid = wlan.adapter()?;
        if !wlan.saved().iter().any(|s| s == ssid) {
            if let Some(p) = password
                && let Some(problem) = password_problem(p)
            {
                return Err(problem.into());
            }
            let xml = wide(&profile_xml(ssid, password));
            let mut reason = 0u32;
            // SAFETY: NUL-terminated XML that outlives the call; no all-user security string.
            let code = unsafe {
                WlanSetProfile(
                    wlan.handle,
                    &guid,
                    0,
                    PCWSTR(xml.as_ptr()),
                    PCWSTR::null(),
                    true,
                    None,
                    &mut reason,
                )
            };
            if code != 0 {
                return Err(format!(
                    "Windows didn't accept the network's settings (code {code}, reason {reason})"
                ));
            }
        }
        let profile = wide(ssid);
        let mut ssid_bytes = DOT11_SSID::default();
        let bytes = ssid.as_bytes();
        let len = bytes.len().min(32);
        ssid_bytes.uSSIDLength = len as u32;
        ssid_bytes.ucSSID[..len].copy_from_slice(&bytes[..len]);
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(profile.as_ptr()),
            pDot11Ssid: &mut ssid_bytes,
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_any,
            dwFlags: 0,
        };
        // SAFETY: parameters whose pointers outlive the call.
        let code = unsafe { WlanConnect(wlan.handle, &guid, &params, None) };
        if code == 0 {
            Ok(())
        } else {
            Err(format!("Windows couldn't start connecting (code {code})"))
        }
    }

    pub fn wifi_disconnect() -> Result<(), String> {
        let wlan = Wlan::open()?;
        let guid = wlan.adapter()?;
        // SAFETY: a plain request on the open handle.
        let code = unsafe { WlanDisconnect(wlan.handle, &guid, None) };
        if code == 0 {
            Ok(())
        } else {
            Err(format!("Windows couldn't disconnect (code {code})"))
        }
    }

    // --- Bluetooth ------------------------------------------------------------------------------

    fn paired(
        selector: windows::core::Result<HSTRING>,
        connected: impl Fn(&HSTRING) -> bool,
    ) -> Vec<BluetoothDevice> {
        let Ok(selector) = selector else {
            return Vec::new();
        };
        let Ok(found) =
            DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.join())
        else {
            return Vec::new();
        };
        found
            .into_iter()
            .filter_map(|d| {
                let name = d.Name().ok()?.to_string();
                let id = d.Id().ok()?;
                Some(BluetoothDevice {
                    connected: connected(&id),
                    name,
                })
            })
            .collect()
    }

    pub fn bluetooth_state() -> BluetoothState {
        winrt();
        let on = radio_on(RadioKind::Bluetooth);
        let mut devices = paired(
            ClassicDevice::GetDeviceSelectorFromPairingState(true),
            |id| {
                ClassicDevice::FromIdAsync(id)
                    .and_then(|op| op.join())
                    .and_then(|d| d.ConnectionStatus())
                    .is_ok_and(|s| s == BluetoothConnectionStatus::Connected)
            },
        );
        devices.extend(paired(
            BluetoothLEDevice::GetDeviceSelectorFromPairingState(true),
            |id| {
                BluetoothLEDevice::FromIdAsync(id)
                    .and_then(|op| op.join())
                    .and_then(|d| d.ConnectionStatus())
                    .is_ok_and(|s| s == BluetoothConnectionStatus::Connected)
            },
        ));
        // A device paired both ways shows once; connected ones first.
        devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
        devices.dedup_by(|a, b| a.name == b.name);
        BluetoothState {
            adapter: on.is_some(),
            on,
            devices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(ssid: &str, bars: u8, saved: bool, connected: bool) -> WifiNetwork {
        WifiNetwork {
            ssid: ssid.into(),
            bars,
            secured: true,
            saved,
            connected,
        }
    }

    #[test]
    fn signal_quality_becomes_windows_bars() {
        assert_eq!([0, 10, 40, 70, 95].map(bars), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn nearby_networks_show_once_each_connected_first_then_strongest() {
        let merged = merge_nearby(vec![
            net("Cafe", 2, false, false),
            net("Home", 3, true, false),
            net("Cafe", 4, false, false),
            net("Home", 1, false, true),
            net("", 4, false, false),
        ]);
        let names: Vec<_> = merged.iter().map(|n| (n.ssid.as_str(), n.bars)).collect();
        assert_eq!(names, vec![("Home", 1), ("Cafe", 4)]);
        assert!(
            merged[0].saved,
            "a profile on any of its access points counts"
        );
    }

    #[test]
    fn a_new_networks_profile_holds_its_name_and_password_escaped() {
        let xml = profile_xml("Tom & Jerry's", Some("p<ss>\"word\""));
        assert!(xml.contains("<name>Tom &amp; Jerry&apos;s</name>"));
        assert!(xml.contains("<keyMaterial>p&lt;ss&gt;&quot;word&quot;</keyMaterial>"));
        assert!(xml.contains("WPA2PSK"));
        let open = profile_xml("Free", None);
        assert!(open.contains("<authentication>open</authentication>"));
        assert!(!open.contains("keyMaterial"));
    }

    #[test]
    fn a_wpa2_password_is_eight_to_sixty_three_characters() {
        assert!(password_problem("short").is_some());
        assert!(password_problem("long enough").is_none());
        assert!(password_problem(&"x".repeat(64)).is_some());
    }
}
