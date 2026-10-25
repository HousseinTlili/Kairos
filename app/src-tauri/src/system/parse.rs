//! Parsers for the console tools the system page reads (M3 §5.1). Pure functions, so they're
//! tested on sample output instead of a real Wi-Fi card or GPU.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WifiInfo {
    pub ssid: String,
    pub signal_percent: Option<u8>,
}

/// `netsh wlan show interfaces`. Its labels are translated, but `SSID` and `Signal` are the same
/// in English, French and German, so those are the only labels read. The first interface that
/// has an SSID is the connected one; no SSID line means not connected.
pub fn parse_netsh_wlan(text: &str) -> Option<WifiInfo> {
    let mut found: Option<WifiInfo> = None;
    for line in text.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let label = label.trim();
        let value = value.trim();
        match (label, found.as_mut()) {
            ("SSID", None) if !value.is_empty() => {
                found = Some(WifiInfo {
                    ssid: value.to_string(),
                    signal_percent: None,
                });
            }
            // A second interface's SSID starts another block; keep the first.
            ("SSID", Some(_)) => break,
            ("Signal", Some(wifi)) => wifi.signal_percent = leading_number(value),
            _ => {}
        }
    }
    found
}

/// The digits a value starts with: "92%", "92 %" with a (possibly mangled) non-breaking space.
fn leading_number(value: &str) -> Option<u8> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u8>().ok().filter(|n| *n <= 100)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub load_percent: Option<f32>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub temperature_c: Option<f32>,
}

const MIB: u64 = 1024 * 1024;

/// One line per GPU from `nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,
/// temperature.gpu --format=csv,noheader,nounits`. Memory is in MiB; fields a card doesn't
/// report read `[N/A]` or `[Not Supported]` and become `None`.
pub fn parse_nvidia_smi(text: &str) -> Vec<GpuInfo> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            if fields.len() != 5 || fields[0].is_empty() {
                return None;
            }
            let number = |s: &str| s.parse::<f32>().ok().filter(|n| n.is_finite());
            let mib = |s: &str| s.parse::<u64>().ok().map(|n| n * MIB);
            Some(GpuInfo {
                name: fields[0].to_string(),
                load_percent: number(fields[1]),
                memory_used: mib(fields[2]),
                memory_total: mib(fields[3]),
                temperature_c: number(fields[4]),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented sample: no real network names or addresses.
    const NETSH_EN: &str = "
There is 1 interface on the system:

    Name                   : Wi-Fi
    Description            : Example Wireless Adapter
    GUID                   : 00000000-0000-0000-0000-000000000000
    Physical address       : 00:00:00:00:00:00
    Interface type         : Primary
    State                  : connected
    SSID                   : Example Net
    AP BSSID               : 00:00:00:00:00:01
    Band                   : 5 GHz
    Radio type             : 802.11ax
    Receive rate (Mbps)    : 1201
    Signal                 : 92%
    Profile                : Example Net
";

    // French Windows: translated labels, and a non-breaking space before "%" that arrives
    // mangled when the console code page isn't UTF-8.
    const NETSH_FR: &str = "
Il existe 1 interface sur le système :

    Nom                    : Wi-Fi
    Description            : Example Wireless Adapter
    État                   : connecté
    SSID                   : Maison
    BSSID                  : 00:00:00:00:00:01
    Type de radio          : 802.11ac
    Signal                 : 78\u{fffd}%
    Profil                 : Maison
";

    const NETSH_DISCONNECTED: &str = "
There is 1 interface on the system:

    Name                   : Wi-Fi
    State                  : disconnected
    Radio status           : Hardware On
";

    #[test]
    fn the_network_name_and_signal_are_read_from_english_netsh_output() {
        assert_eq!(
            parse_netsh_wlan(NETSH_EN),
            Some(WifiInfo {
                ssid: "Example Net".into(),
                signal_percent: Some(92)
            })
        );
    }

    #[test]
    fn french_netsh_output_with_a_mangled_space_before_the_percent_still_reads() {
        assert_eq!(
            parse_netsh_wlan(NETSH_FR),
            Some(WifiInfo {
                ssid: "Maison".into(),
                signal_percent: Some(78)
            })
        );
    }

    #[test]
    fn a_disconnected_or_missing_wifi_interface_reads_as_no_wifi() {
        assert_eq!(parse_netsh_wlan(NETSH_DISCONNECTED), None);
        assert_eq!(
            parse_netsh_wlan("The Wireless AutoConfig Service (wlansvc) is not running."),
            None
        );
        assert_eq!(parse_netsh_wlan(""), None);
    }

    #[test]
    fn the_access_point_bssid_is_not_mistaken_for_the_network_name() {
        let wifi = parse_netsh_wlan(NETSH_EN).expect("connected");
        assert_ne!(wifi.ssid, "00:00:00:00:00:01");
    }

    #[test]
    fn with_two_wifi_interfaces_the_first_connected_one_is_shown() {
        let text = format!("{NETSH_EN}\n    Name : Wi-Fi 2\n    SSID : Other\n    Signal : 10%\n");
        let wifi = parse_netsh_wlan(&text).expect("connected");
        assert_eq!(wifi.ssid, "Example Net");
        assert_eq!(wifi.signal_percent, Some(92));
    }

    #[test]
    fn one_gpu_line_becomes_load_memory_and_temperature() {
        let gpus = parse_nvidia_smi("NVIDIA GeForce RTX 3060, 37, 1536, 12288, 54\n");
        assert_eq!(
            gpus,
            vec![GpuInfo {
                name: "NVIDIA GeForce RTX 3060".into(),
                load_percent: Some(37.0),
                memory_used: Some(1536 * MIB),
                memory_total: Some(12288 * MIB),
                temperature_c: Some(54.0),
            }]
        );
    }

    #[test]
    fn two_gpus_give_two_entries_and_unsupported_fields_are_empty() {
        let text = "NVIDIA RTX A2000, 5, 300, 6144, 41\r\n\
                    NVIDIA Example, [N/A], [Not Supported], 4096, [N/A]\r\n";
        let gpus = parse_nvidia_smi(text);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[1].load_percent, None);
        assert_eq!(gpus[1].memory_used, None);
        assert_eq!(gpus[1].memory_total, Some(4096 * MIB));
        assert_eq!(gpus[1].temperature_c, None);
    }

    #[test]
    fn an_error_message_from_nvidia_smi_is_not_a_gpu() {
        let text = "NVIDIA-SMI has failed because it couldn't communicate with the NVIDIA driver.";
        assert!(parse_nvidia_smi(text).is_empty());
        assert!(parse_nvidia_smi("").is_empty());
    }
}
