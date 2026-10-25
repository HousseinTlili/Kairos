//! Proactive alerts (M3 §7): low battery, a nearly full disk, a process hogging the CPU.
//!
//! Pure: `Alerts::evaluate` takes a snapshot and returns what to announce and what has cleared.
//! Each alert fires once per episode and re-arms only after its condition clears with a margin,
//! so a battery hovering around 20 % doesn't announce itself every 15 s.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::processes::{base_name, is_protected};
use super::{BatteryState, SystemSnapshot};

const BATTERY_LOW: f32 = 20.0;
const BATTERY_LOW_CLEAR: f32 = 25.0;
const BATTERY_CRITICAL: f32 = 10.0;
const BATTERY_CRITICAL_CLEAR: f32 = 15.0;
const DISK_FULL: f64 = 90.0;
const DISK_FULL_CLEAR: f64 = 88.0;
const GB: u64 = 1024 * 1024 * 1024;
const SYSTEM_DRIVE_MIN_FREE: u64 = 5 * GB;
const SYSTEM_DRIVE_CLEAR_FREE: u64 = 6 * GB;
/// Smaller volumes (recovery, EFI) are meant to be full.
const DISK_MIN_SIZE: u64 = 4 * GB;
const CPU_HOG: f32 = 80.0;
const CPU_HOG_CLEAR: f32 = 50.0;
const CPU_HOG_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    BatteryLow,
    BatteryCritical,
    DiskFull,
    CpuHog,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alert {
    /// Stable for the episode: `battery`, `disk:C:\`, `cpu:chrome`.
    pub id: String,
    pub kind: AlertKind,
    pub title: String,
    pub message: String,
    /// For `cpu_hog`: the executable "Close it?" asks Hermes to close.
    pub process: Option<String>,
    pub cpu_percent: Option<f32>,
}

#[derive(Debug, Default)]
struct CpuStreak {
    /// When the process went over the threshold, in snapshot time (Unix ms).
    since: u64,
    fired: bool,
}

#[derive(Debug, Default)]
pub struct Alerts {
    /// `C:\` on Windows: the drive whose free space matters even when it is under 90 % full.
    system_mount: Option<String>,
    battery_low: bool,
    battery_critical: bool,
    disks: HashSet<String>,
    cpu: HashMap<String, CpuStreak>,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Evaluation {
    pub fired: Vec<Alert>,
    /// Ids of earlier alerts whose condition has gone away.
    pub cleared: Vec<String>,
}

fn gb(bytes: u64) -> String {
    format!("{:.0} GB", bytes as f64 / GB as f64)
}

fn minutes(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes >= 60 {
        format!("{} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

impl Alerts {
    pub fn new(system_mount: Option<String>) -> Self {
        Self {
            system_mount: system_mount.map(|m| m.to_lowercase()),
            ..Self::default()
        }
    }

    /// The system drive as Windows names it (`%SystemDrive%`, normally `C:`), as a mount point.
    pub fn for_this_pc() -> Self {
        Self::new(std::env::var("SystemDrive").ok().map(|d| format!("{d}\\")))
    }

    pub fn evaluate(&mut self, snapshot: &SystemSnapshot) -> Evaluation {
        let mut out = Evaluation::default();
        self.battery(snapshot, &mut out);
        self.disks(snapshot, &mut out);
        self.cpu(snapshot, &mut out);
        out
    }

    fn battery(&mut self, snapshot: &SystemSnapshot, out: &mut Evaluation) {
        let Some(battery) = &snapshot.battery else {
            return;
        };
        let discharging = battery.state == BatteryState::Discharging;
        let percent = battery.percent;
        let was_alerting = self.battery_low || self.battery_critical;
        if !discharging || percent > BATTERY_CRITICAL_CLEAR {
            self.battery_critical = false;
        }
        if !discharging || percent > BATTERY_LOW_CLEAR {
            self.battery_low = false;
        }
        if was_alerting && !self.battery_low && !self.battery_critical {
            out.cleared.push("battery".into());
        }
        if !discharging {
            return;
        }
        let left = battery
            .time_to_empty_s
            .map(|s| format!(" — about {} left", minutes(s)))
            .unwrap_or_default();
        if percent <= BATTERY_CRITICAL && !self.battery_critical {
            // Critical also counts as low: no "low" toast after a "critical" one.
            self.battery_critical = true;
            self.battery_low = true;
            out.fired.push(Alert {
                id: "battery".into(),
                kind: AlertKind::BatteryCritical,
                title: "Battery critical".into(),
                message: format!("Battery at {percent:.0} %{left}. Plug in soon."),
                process: None,
                cpu_percent: None,
            });
        } else if percent <= BATTERY_LOW && !self.battery_low {
            self.battery_low = true;
            out.fired.push(Alert {
                id: "battery".into(),
                kind: AlertKind::BatteryLow,
                title: "Battery low".into(),
                message: format!("Battery at {percent:.0} %{left}."),
                process: None,
                cpu_percent: None,
            });
        }
    }

    fn disks(&mut self, snapshot: &SystemSnapshot, out: &mut Evaluation) {
        let mut seen = HashSet::new();
        for disk in &snapshot.disks {
            // USB sticks and card readers are often full on purpose.
            if disk.removable || disk.total < DISK_MIN_SIZE {
                continue;
            }
            let key = disk.mount.to_lowercase();
            seen.insert(key.clone());
            let used =
                (disk.total - disk.available.min(disk.total)) as f64 / disk.total as f64 * 100.0;
            let system = self.system_mount.as_deref() == Some(key.as_str());
            let full = used >= DISK_FULL || (system && disk.available < SYSTEM_DRIVE_MIN_FREE);
            let clear =
                used < DISK_FULL_CLEAR && (!system || disk.available >= SYSTEM_DRIVE_CLEAR_FREE);
            let id = format!("disk:{}", disk.mount);
            if self.disks.contains(&key) {
                if clear {
                    self.disks.remove(&key);
                    out.cleared.push(id);
                }
            } else if full {
                self.disks.insert(key);
                let drive = disk.mount.trim_end_matches(['\\', '/']);
                out.fired.push(Alert {
                    id,
                    kind: AlertKind::DiskFull,
                    title: format!("Drive {drive} is nearly full"),
                    message: format!(
                        "Drive {drive} is {used:.0} % full ({} free).",
                        gb(disk.available)
                    ),
                    process: None,
                    cpu_percent: None,
                });
            }
        }
        // A drive that went away (unplugged) is no longer full.
        let gone: Vec<String> = self.disks.difference(&seen).cloned().collect();
        for key in gone {
            self.disks.remove(&key);
            if let Some(disk) = snapshot
                .disks
                .iter()
                .find(|d| d.mount.to_lowercase() == key)
            {
                out.cleared.push(format!("disk:{}", disk.mount));
            } else {
                out.cleared.push(format!("disk:{key}"));
            }
        }
    }

    fn cpu(&mut self, snapshot: &SystemSnapshot, out: &mut Evaluation) {
        let now = snapshot.taken_at;
        let mut seen = HashSet::new();
        for group in &snapshot.all_processes {
            if is_protected(&group.name) {
                continue;
            }
            let key = base_name(&group.name);
            seen.insert(key.clone());
            let percent = group.cpu_percent;
            if percent >= CPU_HOG {
                let streak = self.cpu.entry(key.clone()).or_insert(CpuStreak {
                    since: now,
                    fired: false,
                });
                if !streak.fired && now.saturating_sub(streak.since) >= CPU_HOG_MS {
                    streak.fired = true;
                    out.fired.push(Alert {
                        id: format!("cpu:{key}"),
                        kind: AlertKind::CpuHog,
                        title: format!("{} is busy", group.name),
                        message: format!(
                            "{} has used {percent:.0} % of the CPU for a minute. Close it?",
                            group.name
                        ),
                        process: Some(group.name.clone()),
                        cpu_percent: Some(percent),
                    });
                }
            } else if percent < CPU_HOG_CLEAR {
                if let Some(streak) = self.cpu.remove(&key)
                    && streak.fired
                {
                    out.cleared.push(format!("cpu:{key}"));
                }
            } else if let Some(streak) = self.cpu.get_mut(&key)
                && !streak.fired
            {
                // Dipped under the threshold before a minute was up: start counting again.
                streak.since = now;
            }
        }
        // A process that exited has stopped hogging the CPU.
        let gone: Vec<String> = self
            .cpu
            .keys()
            .filter(|k| !seen.contains(*k))
            .cloned()
            .collect();
        for key in gone {
            if self.cpu.remove(&key).is_some_and(|s| s.fired) {
                out.cleared.push(format!("cpu:{key}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::processes::ProcessGroup;
    use crate::system::{
        BatteryReading, CpuReading, DiskReading, MemoryReading, NetworkReading, SystemSnapshot,
    };

    fn snapshot(at_ms: u64) -> SystemSnapshot {
        SystemSnapshot {
            taken_at: at_ms,
            cpu: CpuReading {
                name: String::new(),
                frequency_mhz: 0,
                total: Some(10.0),
                per_core: vec![],
            },
            memory: MemoryReading {
                used: 1,
                total: 2,
                swap_used: 0,
                swap_total: 0,
            },
            disks: vec![],
            network: NetworkReading {
                down_per_s: None,
                up_per_s: None,
            },
            wifi: None,
            gpus: vec![],
            battery: None,
            uptime_s: 1,
            top_cpu: vec![],
            top_memory: vec![],
            all_processes: vec![],
            notes: vec![],
        }
    }

    fn on_battery(percent: f32, state: BatteryState) -> SystemSnapshot {
        let mut s = snapshot(0);
        s.battery = Some(BatteryReading {
            percent,
            state,
            time_to_empty_s: Some(40 * 60),
            time_to_full_s: None,
            health_percent: None,
        });
        s
    }

    fn with_disk(mount: &str, total_gb: u64, free_gb: u64) -> SystemSnapshot {
        let mut s = snapshot(0);
        s.disks.push(DiskReading {
            mount: mount.into(),
            name: String::new(),
            kind: "ssd",
            removable: false,
            total: total_gb * GB,
            available: free_gb * GB,
            read_per_s: None,
            write_per_s: None,
        });
        s
    }

    fn with_process(at_ms: u64, name: &str, cpu: f32) -> SystemSnapshot {
        let mut s = snapshot(at_ms);
        s.all_processes.push(ProcessGroup {
            name: name.into(),
            count: 1,
            cpu_percent: cpu,
            memory: 0,
        });
        s
    }

    fn kinds(e: &Evaluation) -> Vec<AlertKind> {
        e.fired.iter().map(|a| a.kind).collect()
    }

    #[test]
    fn low_battery_fires_once_then_rearms_only_after_charging_or_a_margin() {
        let mut alerts = Alerts::new(None);
        let first = alerts.evaluate(&on_battery(19.0, BatteryState::Discharging));
        assert_eq!(kinds(&first), [AlertKind::BatteryLow]);
        assert_eq!(
            first.fired[0].message,
            "Battery at 19 % — about 40 min left."
        );
        // Hovering around the threshold doesn't repeat it.
        for percent in [18.0, 21.0, 19.5, 24.0] {
            assert!(
                alerts
                    .evaluate(&on_battery(percent, BatteryState::Discharging))
                    .fired
                    .is_empty()
            );
        }
        let charging = alerts.evaluate(&on_battery(19.0, BatteryState::Charging));
        assert_eq!(charging.cleared, ["battery"]);
        let again = alerts.evaluate(&on_battery(19.0, BatteryState::Discharging));
        assert_eq!(kinds(&again), [AlertKind::BatteryLow]);
    }

    #[test]
    fn critical_battery_fires_after_low_but_low_never_follows_critical() {
        let mut alerts = Alerts::new(None);
        alerts.evaluate(&on_battery(18.0, BatteryState::Discharging));
        let critical = alerts.evaluate(&on_battery(9.0, BatteryState::Discharging));
        assert_eq!(kinds(&critical), [AlertKind::BatteryCritical]);
        assert!(
            alerts
                .evaluate(&on_battery(16.0, BatteryState::Discharging))
                .fired
                .is_empty()
        );

        let mut straight_to_critical = Alerts::new(None);
        let e = straight_to_critical.evaluate(&on_battery(7.0, BatteryState::Discharging));
        assert_eq!(kinds(&e), [AlertKind::BatteryCritical]);
        assert!(
            straight_to_critical
                .evaluate(&on_battery(12.0, BatteryState::Discharging))
                .fired
                .is_empty()
        );
    }

    #[test]
    fn a_low_battery_while_charging_is_not_an_alert() {
        let mut alerts = Alerts::new(None);
        assert!(
            alerts
                .evaluate(&on_battery(5.0, BatteryState::Charging))
                .fired
                .is_empty()
        );
    }

    #[test]
    fn a_full_disk_fires_once_and_clears_below_the_margin() {
        let mut alerts = Alerts::new(Some("C:\\".into()));
        let e = alerts.evaluate(&with_disk("D:\\", 1000, 70));
        assert_eq!(kinds(&e), [AlertKind::DiskFull]);
        assert_eq!(e.fired[0].message, "Drive D: is 93 % full (70 GB free).");
        assert!(
            alerts
                .evaluate(&with_disk("D:\\", 1000, 110))
                .fired
                .is_empty()
        ); // 89 %
        let cleared = alerts.evaluate(&with_disk("D:\\", 1000, 130)); // 87 %
        assert_eq!(cleared.cleared, ["disk:D:\\"]);
    }

    #[test]
    fn the_system_drive_also_alerts_on_little_free_space() {
        let mut alerts = Alerts::new(Some("C:\\".into()));
        // 4 GB free of 30 is only 87 % full, but a C: with 4 GB free is a problem anyway.
        let e = alerts.evaluate(&with_disk("C:\\", 30, 4));
        assert_eq!(kinds(&e), [AlertKind::DiskFull]);
        let mut other = Alerts::new(Some("C:\\".into()));
        assert!(other.evaluate(&with_disk("E:\\", 30, 4)).fired.is_empty());
    }

    #[test]
    fn small_and_removable_drives_never_alert() {
        let mut alerts = Alerts::new(None);
        assert!(alerts.evaluate(&with_disk("R:\\", 1, 0)).fired.is_empty());
        let mut usb = with_disk("F:\\", 64, 1);
        usb.disks[0].removable = true;
        assert!(alerts.evaluate(&usb).fired.is_empty());
    }

    #[test]
    fn a_cpu_hog_needs_a_full_minute_over_the_threshold() {
        let mut alerts = Alerts::new(None);
        for at in [0, 15_000, 30_000, 45_000] {
            assert!(
                alerts
                    .evaluate(&with_process(at, "chrome.exe", 85.0))
                    .fired
                    .is_empty()
            );
        }
        let e = alerts.evaluate(&with_process(60_000, "chrome.exe", 85.0));
        assert_eq!(kinds(&e), [AlertKind::CpuHog]);
        assert_eq!(e.fired[0].process.as_deref(), Some("chrome.exe"));
        assert!(e.fired[0].message.ends_with("Close it?"));
        // Still busy: no repeat.
        assert!(
            alerts
                .evaluate(&with_process(75_000, "chrome.exe", 90.0))
                .fired
                .is_empty()
        );
    }

    #[test]
    fn a_dip_below_the_threshold_restarts_the_minute() {
        let mut alerts = Alerts::new(None);
        alerts.evaluate(&with_process(0, "render.exe", 85.0));
        alerts.evaluate(&with_process(30_000, "render.exe", 60.0)); // dip
        assert!(
            alerts
                .evaluate(&with_process(60_000, "render.exe", 85.0))
                .fired
                .is_empty()
        );
        let e = alerts.evaluate(&with_process(90_000, "render.exe", 85.0));
        assert_eq!(kinds(&e), [AlertKind::CpuHog]);
    }

    #[test]
    fn a_cpu_alert_clears_when_the_process_calms_down_or_exits() {
        let mut alerts = Alerts::new(None);
        alerts.evaluate(&with_process(0, "a.exe", 85.0));
        alerts.evaluate(&with_process(60_000, "a.exe", 85.0));
        let calm = alerts.evaluate(&with_process(75_000, "a.exe", 10.0));
        assert_eq!(calm.cleared, ["cpu:a"]);

        alerts.evaluate(&with_process(0, "b.exe", 85.0));
        alerts.evaluate(&with_process(60_000, "b.exe", 85.0));
        let exited = alerts.evaluate(&snapshot(75_000));
        assert_eq!(exited.cleared, ["cpu:b"]);
    }

    #[test]
    fn system_processes_never_raise_a_cpu_alert() {
        let mut alerts = Alerts::new(None);
        for name in ["svchost.exe", "dwm.exe", "kairos.exe"] {
            alerts.evaluate(&with_process(0, name, 99.0));
            assert!(
                alerts
                    .evaluate(&with_process(120_000, name, 99.0))
                    .fired
                    .is_empty(),
                "{name}"
            );
        }
    }
}
