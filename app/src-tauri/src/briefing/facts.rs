//! The instant part of the briefing (M5 §2.2): facts Kairos already has, no AI. Pure, so each
//! line is tested; notification *text* never appears, only app names and counts.

use serde::{Deserialize, Serialize};

use super::rules::Reason;
use crate::notifications::store::AppCount;
use crate::store::TaskSummary;
use crate::system::{BatteryState, SystemSnapshot};

const GB: u64 = 1024 * 1024 * 1024;
const DISK_WARN_PERCENT: f64 = 90.0;
const DISK_MIN_SIZE: u64 = 4 * GB;
const BATTERY_WARN_PERCENT: f32 = 20.0;
/// App names listed in the notifications line before "and N more".
const APPS_LISTED: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    Away,
    Battery,
    Network,
    Disk,
    Notifications,
    Tasks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Info,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub kind: FactKind,
    pub tone: Tone,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Briefing {
    /// Unix seconds.
    pub at: i64,
    pub reason: Reason,
    pub away_s: Option<i64>,
    pub facts: Vec<Fact>,
}

pub struct Inputs<'a> {
    pub away_s: Option<i64>,
    pub snapshot: Option<&'a SystemSnapshot>,
    /// Per-app counts from the notification store (M4).
    pub notifications: &'a [AppCount],
    pub tasks: TaskSummary,
}

fn duration(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    match (minutes / 60, minutes % 60) {
        (0, 0) => "less than a minute".into(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn fact(kind: FactKind, tone: Tone, text: String) -> Fact {
    Fact { kind, tone, text }
}

pub fn build(inputs: &Inputs<'_>) -> Vec<Fact> {
    let mut facts = Vec::new();

    if let Some(away) = inputs.away_s.filter(|a| *a >= 60) {
        facts.push(fact(
            FactKind::Away,
            Tone::Info,
            format!("You were away {}.", duration(away)),
        ));
    }

    if let Some(snapshot) = inputs.snapshot {
        if let Some(battery) = &snapshot.battery {
            let percent = battery.percent.round();
            let (tone, text) = match battery.state {
                BatteryState::Charging => (Tone::Info, format!("Battery {percent} %, charging.")),
                BatteryState::Full => (Tone::Info, "Battery full, plugged in.".to_string()),
                BatteryState::Discharging => {
                    let left = battery
                        .time_to_empty_s
                        .map(|s| format!(", about {} left", duration(s as i64)))
                        .unwrap_or_default();
                    let tone = if battery.percent < BATTERY_WARN_PERCENT {
                        Tone::Warn
                    } else {
                        Tone::Info
                    };
                    (tone, format!("Battery {percent} %{left}."))
                }
                BatteryState::Unknown => (Tone::Info, format!("Battery {percent} %.")),
            };
            facts.push(fact(FactKind::Battery, tone, text));
        }

        if let Some(wifi) = &snapshot.wifi {
            let signal = wifi
                .signal_percent
                .map(|s| format!(" (signal {s} %)"))
                .unwrap_or_default();
            facts.push(fact(
                FactKind::Network,
                Tone::Info,
                format!("On Wi-Fi \u{201c}{}\u{201d}{signal}.", wifi.ssid),
            ));
        }

        for disk in &snapshot.disks {
            if disk.removable || disk.total < DISK_MIN_SIZE {
                continue;
            }
            let used =
                (disk.total - disk.available.min(disk.total)) as f64 / disk.total as f64 * 100.0;
            if used >= DISK_WARN_PERCENT {
                let drive = disk.mount.trim_end_matches(['\\', '/']);
                facts.push(fact(
                    FactKind::Disk,
                    Tone::Warn,
                    format!(
                        "Drive {drive} is {used:.0} % full ({:.0} GB free).",
                        disk.available as f64 / GB as f64
                    ),
                ));
            }
        }
    }

    let unread: Vec<&AppCount> = inputs
        .notifications
        .iter()
        .filter(|c| c.unread > 0)
        .collect();
    let total_unread: i64 = unread.iter().map(|c| c.unread).sum();
    if total_unread > 0 {
        let mut apps: Vec<String> = unread
            .iter()
            .take(APPS_LISTED)
            .map(|c| format!("{} {}", c.app_name, c.unread))
            .collect();
        if unread.len() > APPS_LISTED {
            apps.push(format!("and {} more", unread.len() - APPS_LISTED));
        }
        facts.push(fact(
            FactKind::Notifications,
            Tone::Info,
            format!(
                "{}: {}.",
                plural(total_unread, "unread notification", "unread notifications"),
                apps.join(", ")
            ),
        ));
    }

    let tasks = inputs.tasks;
    if tasks.waiting > 0 {
        facts.push(fact(
            FactKind::Tasks,
            Tone::Warn,
            format!(
                "{} waiting for your approval.",
                plural(tasks.waiting, "request is", "requests are")
            ),
        ));
    }
    if tasks.finished > 0 || tasks.failed > 0 {
        let mut parts = Vec::new();
        if tasks.finished > 0 {
            parts.push(format!(
                "{} finished",
                plural(tasks.finished, "request", "requests")
            ));
        }
        if tasks.failed > 0 {
            parts.push(format!(
                "{} didn't finish",
                plural(tasks.failed, "request", "requests")
            ));
        }
        facts.push(fact(
            FactKind::Tasks,
            if tasks.failed > 0 {
                Tone::Warn
            } else {
                Tone::Info
            },
            format!("While you were away: {}.", parts.join(", ")),
        ));
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::parse::WifiInfo;
    use crate::system::{
        BatteryReading, CpuReading, DiskReading, MemoryReading, NetworkReading, SystemSnapshot,
    };

    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            taken_at: 0,
            cpu: CpuReading {
                name: String::new(),
                frequency_mhz: 0,
                total: Some(5.0),
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

    fn count(app: &str, unread: i64) -> AppCount {
        AppCount {
            app_id: format!("{app}.App"),
            app_name: app.into(),
            total: unread + 1,
            unread,
        }
    }

    fn texts(facts: &[Fact]) -> Vec<&str> {
        facts.iter().map(|f| f.text.as_str()).collect()
    }

    #[test]
    fn nothing_to_say_gives_no_facts() {
        let facts = build(&Inputs {
            away_s: Some(20),
            snapshot: Some(&snapshot()),
            notifications: &[count("Discord", 0)],
            tasks: TaskSummary::default(),
        });
        assert!(facts.is_empty(), "{facts:?}");
    }

    #[test]
    fn every_fact_in_order_when_there_is_something_to_say() {
        let mut s = snapshot();
        s.battery = Some(BatteryReading {
            percent: 18.4,
            state: BatteryState::Discharging,
            time_to_empty_s: Some(40 * 60),
            time_to_full_s: None,
            health_percent: None,
        });
        s.wifi = Some(WifiInfo {
            ssid: "Example Net".into(),
            signal_percent: Some(80),
        });
        s.disks.push(DiskReading {
            mount: "C:\\".into(),
            name: String::new(),
            kind: "ssd",
            removable: false,
            total: 100 * GB,
            available: 7 * GB,
            read_per_s: None,
            write_per_s: None,
        });
        let facts = build(&Inputs {
            away_s: Some(2 * 3600 + 10 * 60),
            snapshot: Some(&s),
            notifications: &[count("Phone Link", 2), count("Discord", 3)],
            tasks: TaskSummary {
                finished: 2,
                failed: 0,
                waiting: 1,
            },
        });
        assert_eq!(
            texts(&facts),
            [
                "You were away 2 h 10 min.",
                "Battery 18 %, about 40 min left.",
                "On Wi-Fi \u{201c}Example Net\u{201d} (signal 80 %).",
                "Drive C: is 93 % full (7 GB free).",
                "5 unread notifications: Phone Link 2, Discord 3.",
                "1 request is waiting for your approval.",
                "While you were away: 2 requests finished.",
            ]
        );
        assert_eq!(facts[1].tone, Tone::Warn, "battery under 20 %");
        assert_eq!(facts[3].tone, Tone::Warn);
    }

    #[test]
    fn charging_and_full_batteries_read_calmly() {
        let mut s = snapshot();
        s.battery = Some(BatteryReading {
            percent: 54.0,
            state: BatteryState::Charging,
            time_to_empty_s: None,
            time_to_full_s: Some(3600),
            health_percent: None,
        });
        let facts = build(&Inputs {
            away_s: None,
            snapshot: Some(&s),
            notifications: &[],
            tasks: TaskSummary::default(),
        });
        assert_eq!(texts(&facts), ["Battery 54 %, charging."]);
        assert_eq!(facts[0].tone, Tone::Info);
    }

    #[test]
    fn many_apps_are_shortened_and_failures_are_a_warning() {
        let counts: Vec<AppCount> = ["A", "B", "C", "D", "E", "F"]
            .iter()
            .map(|a| count(a, 1))
            .collect();
        let facts = build(&Inputs {
            away_s: None,
            snapshot: None,
            notifications: &counts,
            tasks: TaskSummary {
                finished: 1,
                failed: 2,
                waiting: 0,
            },
        });
        assert_eq!(
            texts(&facts),
            [
                "6 unread notifications: A 1, B 1, C 1, D 1, and 2 more.",
                "While you were away: 1 request finished, 2 requests didn't finish.",
            ]
        );
        assert_eq!(facts[1].tone, Tone::Warn);
    }

    #[test]
    fn notification_text_never_reaches_the_briefing() {
        // The store's counts carry no text at all; this guards the type staying that way.
        let facts = build(&Inputs {
            away_s: None,
            snapshot: None,
            notifications: &[count("Discord", 1)],
            tasks: TaskSummary::default(),
        });
        assert_eq!(texts(&facts), ["1 unread notification: Discord 1."]);
    }
}
