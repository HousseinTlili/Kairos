//! Happenings from two system snapshots in a row (M17 §3.1): the charger, the battery level while
//! discharging, and the Wi-Fi network. Nothing compared snapshots before; the alerts (M3 §7) only
//! look at one.

use super::Happening;
use crate::system::{BatteryState, SystemSnapshot};

/// A Wi-Fi change must be seen in this many snapshots in a row: one failed `netsh` reading is not a
/// disconnection.
const WIFI_STEADY: u8 = 2;

#[derive(Debug, Default)]
pub struct SnapshotWatch {
    /// The last battery reading: (percent, discharging). `None` until the first one, so starting
    /// Kairos isn't "plugged in".
    battery: Option<(f32, bool)>,
    /// The network as last announced (`Some("")` would never happen: an empty SSID is no Wi-Fi).
    wifi: Option<Option<String>>,
    /// A different network seen, and how many times in a row.
    wifi_candidate: Option<(Option<String>, u8)>,
}

impl SnapshotWatch {
    pub fn observe(&mut self, snapshot: &SystemSnapshot) -> Vec<Happening> {
        let mut out = Vec::new();
        self.battery(snapshot, &mut out);
        self.wifi(snapshot, &mut out);
        out
    }

    fn battery(&mut self, snapshot: &SystemSnapshot, out: &mut Vec<Happening>) {
        let Some(reading) = &snapshot.battery else {
            return;
        };
        // "Unknown" says nothing about the charger: keep the previous state.
        let discharging = match reading.state {
            BatteryState::Discharging => true,
            BatteryState::Charging | BatteryState::Full => false,
            BatteryState::Unknown => match self.battery {
                Some((_, was)) => was,
                None => return,
            },
        };
        let percent = reading.percent;
        if let Some((before, was_discharging)) = self.battery {
            if was_discharging != discharging {
                out.push(Happening::Charger {
                    plugged: !discharging,
                    percent: Some(percent),
                });
            }
            if discharging && was_discharging && percent < before {
                out.push(Happening::BatteryDrop {
                    from: before,
                    to: percent,
                    time_left_s: reading.time_to_empty_s,
                });
            }
        }
        self.battery = Some((percent, discharging));
    }

    fn wifi(&mut self, snapshot: &SystemSnapshot, out: &mut Vec<Happening>) {
        let now = snapshot
            .wifi
            .as_ref()
            .map(|w| w.ssid.trim().to_string())
            .filter(|s| !s.is_empty());
        let Some(announced) = &self.wifi else {
            // The network Kairos starts on is not a change.
            self.wifi = Some(now);
            return;
        };
        if *announced == now {
            self.wifi_candidate = None;
            return;
        }
        let seen = match &self.wifi_candidate {
            Some((candidate, n)) if *candidate == now => n + 1,
            _ => 1,
        };
        if seen < WIFI_STEADY {
            self.wifi_candidate = Some((now, seen));
            return;
        }
        if let Some(old) = announced {
            out.push(Happening::WifiLost {
                network: old.clone(),
            });
        }
        if let Some(new) = &now {
            out.push(Happening::WifiJoined {
                network: new.clone(),
            });
        }
        self.wifi = Some(now);
        self.wifi_candidate = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{
        BatteryReading, CpuReading, MemoryReading, NetworkReading, parse::WifiInfo,
    };

    fn snapshot(battery: Option<(f32, BatteryState)>, wifi: Option<&str>) -> SystemSnapshot {
        SystemSnapshot {
            taken_at: 0,
            cpu: CpuReading {
                name: String::new(),
                frequency_mhz: 0,
                total: None,
                per_core: vec![],
            },
            memory: MemoryReading {
                used: 0,
                total: 0,
                swap_used: 0,
                swap_total: 0,
            },
            disks: vec![],
            network: NetworkReading {
                down_per_s: None,
                up_per_s: None,
            },
            wifi: wifi.map(|ssid| WifiInfo {
                ssid: ssid.into(),
                signal_percent: Some(80),
            }),
            gpus: vec![],
            battery: battery.map(|(percent, state)| BatteryReading {
                percent,
                state,
                time_to_empty_s: Some(3600),
                time_to_full_s: None,
                health_percent: None,
            }),
            uptime_s: 0,
            top_cpu: vec![],
            top_memory: vec![],
            all_processes: vec![],
            notes: vec![],
        }
    }

    use BatteryState::{Charging, Discharging, Unknown};

    #[test]
    fn unplugging_fires_once_not_every_snapshot() {
        let mut w = SnapshotWatch::default();
        assert!(
            w.observe(&snapshot(Some((80.0, Charging)), None))
                .is_empty(),
            "start"
        );
        let unplugged = w.observe(&snapshot(Some((80.0, Discharging)), None));
        assert_eq!(
            unplugged,
            vec![Happening::Charger {
                plugged: false,
                percent: Some(80.0)
            }]
        );
        assert!(
            w.observe(&snapshot(Some((80.0, Discharging)), None))
                .is_empty()
        );
        assert!(
            w.observe(&snapshot(Some((80.0, Unknown)), None)).is_empty(),
            "unknown keeps it"
        );
        let plugged = w.observe(&snapshot(Some((80.0, Charging)), None));
        assert!(matches!(
            plugged[..],
            [Happening::Charger { plugged: true, .. }]
        ));
    }

    #[test]
    fn the_battery_level_is_reported_only_while_it_falls_on_battery() {
        let mut w = SnapshotWatch::default();
        w.observe(&snapshot(Some((16.0, Discharging)), None));
        let drop = w.observe(&snapshot(Some((14.5, Discharging)), None));
        assert!(matches!(
            drop[..],
            [Happening::BatteryDrop { from, to, .. }] if from == 16.0 && to == 14.5
        ));
        assert!(
            w.observe(&snapshot(Some((14.5, Discharging)), None))
                .is_empty()
        );
        // Plugged in: rising or not, no drops.
        w.observe(&snapshot(Some((14.5, Charging)), None));
        assert!(
            w.observe(&snapshot(Some((14.0, Charging)), None))
                .is_empty()
        );
    }

    #[test]
    fn a_wifi_change_counts_after_two_snapshots_and_one_bad_reading_is_ignored() {
        let mut w = SnapshotWatch::default();
        assert!(w.observe(&snapshot(None, Some("Home"))).is_empty(), "start");
        assert!(
            w.observe(&snapshot(None, None)).is_empty(),
            "one missing reading"
        );
        assert!(
            w.observe(&snapshot(None, Some("Home"))).is_empty(),
            "back: nothing happened"
        );
        assert!(w.observe(&snapshot(None, Some("Office"))).is_empty());
        assert_eq!(
            w.observe(&snapshot(None, Some("Office"))),
            vec![
                Happening::WifiLost {
                    network: "Home".into()
                },
                Happening::WifiJoined {
                    network: "Office".into()
                }
            ]
        );
        w.observe(&snapshot(None, None));
        assert_eq!(
            w.observe(&snapshot(None, None)),
            vec![Happening::WifiLost {
                network: "Office".into()
            }]
        );
    }
}
