//! Automations on events (M17, D-033): *when* something happens on the PC, *only if* some
//! conditions hold, *do* one or more things. What happened is a [`Happening`]; `rules::decide`
//! says whether an automation runs; `engine` runs it. Kept in step with
//! `app/src/features/automations/model.ts`.

pub mod api;
pub mod crash;
pub mod engine;
pub mod rules;
pub mod store;
pub mod watch;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What an automation waits for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Kairos started at login.
    Login,
    /// Resume or unlock after at least this long away.
    Back { away_min: u32 },
    /// The battery crossed under this level while discharging.
    BatteryLow { below: u8 },
    /// The charger was plugged in (`true`) or unplugged.
    Charger { plugged: bool },
    /// Joined a Wi-Fi network; an empty name means any.
    WifiJoined { network: String },
    /// Left Wi-Fi (disconnected or changed networks).
    WifiLost,
    /// A drive is nearly full (the M3 alert).
    DiskFull,
    /// A program crashed or hung; an empty list means any program not ignored.
    Crash { programs: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Power {
    Battery,
    Plugged,
}

/// "Only if": every part optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    /// A window of the day in minutes after local midnight; it may run over midnight.
    pub from_min: Option<u16>,
    pub to_min: Option<u16>,
    /// Days of the week, 0 = Sunday; empty means every day.
    #[serde(default)]
    pub days: Vec<u8>,
    pub power: Option<Power>,
    /// Allowed through the quiet hours (off by default: quiet hours hold every automation).
    #[serde(default)]
    pub during_quiet_hours: bool,
}

/// What an automation does. Texts may hold `{placeholders}` filled from the happening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Notify {
        text: String,
    },
    Say {
        text: String,
    },
    Island {
        text: String,
    },
    /// An app from the Start menu list, by its listing id (never a command line).
    OpenApp {
        app_id: String,
        name: String,
    },
    /// A toast and a card with an "Ask Kairos" button: the request starts only on the click.
    Offer {
        text: String,
    },
    /// A request to Kairos, under the rules, started by itself.
    Ask {
        text: String,
    },
}

impl Action {
    pub fn uses_ai(&self) -> bool {
        matches!(self, Action::Ask { .. })
    }

    /// Done by the main window (voice, island, the chat), not by the core.
    pub fn for_page(&self) -> bool {
        matches!(
            self,
            Action::Say { .. } | Action::Island { .. } | Action::Offer { .. } | Action::Ask { .. }
        )
    }
}

pub const DEFAULT_COOLDOWN_MIN: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: Trigger,
    #[serde(default)]
    pub only_if: Conditions,
    pub actions: Vec<Action>,
    pub cooldown_min: u32,
}

/// A program crash or hang, from the Application event log (M17 §3.1). Never a memory dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashInfo {
    /// The event log's record number, to see each event once.
    pub record: u64,
    pub program: String,
    pub version: String,
    /// The faulting module and exception code (crashes only).
    pub module: Option<String>,
    pub code: Option<String>,
    pub hang: bool,
}

/// What happened on the PC.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Happening {
    Login,
    Back {
        away_s: i64,
    },
    /// The battery went from `from` to `to` percent while discharging.
    BatteryDrop {
        from: f32,
        to: f32,
        time_left_s: Option<u64>,
    },
    Charger {
        plugged: bool,
        percent: Option<f32>,
    },
    WifiJoined {
        network: String,
    },
    WifiLost {
        network: String,
    },
    DiskFull {
        message: String,
    },
    Crash(CrashInfo),
}

fn same_program(a: &str, b: &str) -> bool {
    let strip = |s: &str| {
        let s = s.trim().to_lowercase();
        s.strip_suffix(".exe").map(str::to_string).unwrap_or(s)
    };
    strip(a) == strip(b)
}

impl Trigger {
    pub fn matches(&self, happening: &Happening) -> bool {
        match (self, happening) {
            (Trigger::Login, Happening::Login) => true,
            (Trigger::Back { away_min }, Happening::Back { away_s }) => {
                *away_s >= i64::from(*away_min) * 60
            }
            (Trigger::BatteryLow { below }, Happening::BatteryDrop { from, to, .. }) => {
                let below = f32::from(*below);
                *from >= below && *to < below
            }
            (Trigger::Charger { plugged }, Happening::Charger { plugged: now, .. }) => {
                plugged == now
            }
            (Trigger::WifiJoined { network }, Happening::WifiJoined { network: joined }) => {
                network.trim().is_empty() || network.trim().eq_ignore_ascii_case(joined.trim())
            }
            (Trigger::WifiLost, Happening::WifiLost { .. }) => true,
            (Trigger::DiskFull, Happening::DiskFull { .. }) => true,
            (Trigger::Crash { programs }, Happening::Crash(crash)) => {
                programs.is_empty() || programs.iter().any(|p| same_program(p, &crash.program))
            }
            _ => false,
        }
    }
}

fn duration_words(seconds: i64) -> String {
    let minutes = (seconds.max(0) + 30) / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

impl Happening {
    /// The values `{placeholders}` are filled with; unknown ones stay as typed.
    pub fn values(&self) -> BTreeMap<&'static str, String> {
        let mut v = BTreeMap::new();
        match self {
            Happening::Login => {}
            Happening::Back { away_s } => {
                v.insert("away", duration_words(*away_s));
            }
            Happening::BatteryDrop {
                to, time_left_s, ..
            } => {
                v.insert("battery", format!("{to:.0} %"));
                if let Some(left) = time_left_s {
                    v.insert("time_left", duration_words(*left as i64));
                }
            }
            Happening::Charger { percent, .. } => {
                if let Some(p) = percent {
                    v.insert("battery", format!("{p:.0} %"));
                }
            }
            Happening::WifiJoined { network } | Happening::WifiLost { network } => {
                v.insert("network", network.clone());
            }
            Happening::DiskFull { message } => {
                v.insert("disk", message.clone());
            }
            Happening::Crash(c) => {
                v.insert("program", c.program.clone());
                v.insert("crash", self.describe());
            }
        }
        v
    }

    /// One line for the history and for "Ask Kairos" (as data).
    pub fn describe(&self) -> String {
        match self {
            Happening::Login => "Kairos started at login".into(),
            Happening::Back { away_s } => format!("Back after {}", duration_words(*away_s)),
            Happening::BatteryDrop { to, .. } => format!("Battery at {to:.0} %, discharging"),
            Happening::Charger { plugged: true, .. } => "Charger plugged in".into(),
            Happening::Charger { plugged: false, .. } => "Charger unplugged".into(),
            Happening::WifiJoined { network } => format!("Joined the Wi-Fi network {network}"),
            Happening::WifiLost { network } => format!("Left the Wi-Fi network {network}"),
            Happening::DiskFull { message } => message.clone(),
            Happening::Crash(c) => {
                let what = if c.hang {
                    "stopped responding"
                } else {
                    "crashed"
                };
                let mut line = format!("{} {} {what}", c.program, c.version);
                if let Some(module) = &c.module {
                    line.push_str(&format!(" (in {module}"));
                    if let Some(code) = &c.code {
                        line.push_str(&format!(", exception 0x{code}"));
                    }
                    line.push(')');
                }
                line
            }
        }
    }
}

/// Fills `{name}` placeholders. Values are plain text; nothing in them is read as a placeholder
/// again.
pub fn fill(text: &str, values: &BTreeMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if values.contains_key(&after[..end]) => {
                out.push_str(&values[&after[..end]]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A sample of each happening, for "Test now".
pub fn sample(trigger: &Trigger) -> Happening {
    match trigger {
        Trigger::Login => Happening::Login,
        Trigger::Back { away_min } => Happening::Back {
            away_s: i64::from(*away_min).max(1) * 60,
        },
        Trigger::BatteryLow { below } => Happening::BatteryDrop {
            from: f32::from(*below),
            to: f32::from(*below) - 1.0,
            time_left_s: Some(40 * 60),
        },
        Trigger::Charger { plugged } => Happening::Charger {
            plugged: *plugged,
            percent: Some(64.0),
        },
        Trigger::WifiJoined { network } => Happening::WifiJoined {
            network: if network.trim().is_empty() {
                "Home".into()
            } else {
                network.clone()
            },
        },
        Trigger::WifiLost => Happening::WifiLost {
            network: "Home".into(),
        },
        Trigger::DiskFull => Happening::DiskFull {
            message: "Drive C: is 93 % full".into(),
        },
        Trigger::Crash { programs } => Happening::Crash(CrashInfo {
            record: 0,
            program: programs
                .first()
                .cloned()
                .unwrap_or_else(|| "example.exe".into()),
            version: "1.0.0".into(),
            module: Some("example.dll".into()),
            code: Some("c0000005".into()),
            hang: false,
        }),
    }
}

/// The three starters (M17 Step 7), offered once on a fresh profile.
pub fn starters() -> Vec<Automation> {
    vec![
        Automation {
            id: "starter-crash".into(),
            name: "A program crashed".into(),
            enabled: true,
            trigger: Trigger::Crash { programs: vec![] },
            only_if: Conditions::default(),
            actions: vec![Action::Offer {
                text: "{crash}. Shall I look into why?".into(),
            }],
            cooldown_min: 1,
        },
        Automation {
            id: "starter-battery".into(),
            name: "Battery running low".into(),
            enabled: false,
            trigger: Trigger::BatteryLow { below: 15 },
            only_if: Conditions::default(),
            actions: vec![Action::Say {
                text: "Master, the battery is down to {battery}.".into(),
            }],
            cooldown_min: DEFAULT_COOLDOWN_MIN,
        },
        Automation {
            id: "starter-login".into(),
            name: "At login, open my apps".into(),
            enabled: false,
            trigger: Trigger::Login,
            only_if: Conditions::default(),
            actions: vec![],
            cooldown_min: DEFAULT_COOLDOWN_MIN,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crash(program: &str) -> Happening {
        Happening::Crash(CrashInfo {
            record: 1,
            program: program.into(),
            version: "2.0".into(),
            module: Some("ntdll.dll".into()),
            code: Some("c0000409".into()),
            hang: false,
        })
    }

    #[test]
    fn a_battery_trigger_fires_only_when_the_level_is_crossed() {
        let low = Trigger::BatteryLow { below: 15 };
        let drop = |from, to| Happening::BatteryDrop {
            from,
            to,
            time_left_s: None,
        };
        assert!(low.matches(&drop(15.0, 14.0)));
        assert!(!low.matches(&drop(14.0, 13.0)), "already under: no repeat");
        assert!(!low.matches(&drop(30.0, 29.0)));
    }

    #[test]
    fn a_crash_trigger_matches_its_programs_with_or_without_exe() {
        let only = Trigger::Crash {
            programs: vec!["Code".into()],
        };
        assert!(only.matches(&crash("code.exe")));
        assert!(!only.matches(&crash("chrome.exe")));
        assert!(Trigger::Crash { programs: vec![] }.matches(&crash("chrome.exe")));
    }

    #[test]
    fn back_needs_the_time_away_and_wifi_matches_any_network_when_empty() {
        let back = Trigger::Back { away_min: 30 };
        assert!(back.matches(&Happening::Back { away_s: 1800 }));
        assert!(!back.matches(&Happening::Back { away_s: 1799 }));
        let joined = |n: &str| Happening::WifiJoined { network: n.into() };
        let any = Trigger::WifiJoined {
            network: String::new(),
        };
        let home = Trigger::WifiJoined {
            network: "Home".into(),
        };
        assert!(any.matches(&joined("Office")));
        assert!(home.matches(&joined("home")));
        assert!(!home.matches(&joined("Office")));
    }

    #[test]
    fn placeholders_are_filled_once_and_unknown_ones_are_kept() {
        let values = crash("game.exe").values();
        let text = fill("{program} {crash} {nope} {", &values);
        assert!(
            text.starts_with("game.exe game.exe 2.0 crashed (in ntdll.dll, exception 0xc0000409)")
        );
        assert!(text.ends_with("{nope} {"));
        // A value holding braces is not read again.
        let mut v = BTreeMap::new();
        v.insert("network", "{program}".to_string());
        assert_eq!(fill("{network}", &v), "{program}");
    }

    #[test]
    fn an_automation_survives_json_both_ways() {
        for a in starters() {
            let text = serde_json::to_string(&a).expect("json");
            let back: Automation = serde_json::from_str(&text).expect("parse");
            assert_eq!(a, back);
        }
    }
}
