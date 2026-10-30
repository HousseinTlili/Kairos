//! Whether an automation runs for a happening (M17 §3.2, §3.4). Pure: the automation, what happened
//! and the moment in; a verdict out.

use super::{Automation, Happening, Power};
use crate::briefing::rules::{in_quiet_hours, local_day_and_minute};

/// Across all automations (M17 §3.4, accepted 2026-10-30).
pub const AI_PER_HOUR: usize = 6;
pub const AI_PER_DAY: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct Moment {
    /// Unix seconds.
    pub now: i64,
    pub utc_offset_min: i32,
    /// `None` on a PC with no battery, which counts as plugged in.
    pub on_battery: Option<bool>,
    /// The briefing's quiet hours (M5).
    pub quiet_start: Option<u16>,
    pub quiet_end: Option<u16>,
    /// When this automation last ran (skips don't count).
    pub last_run_at: Option<i64>,
    /// "Ask Kairos" requests started by automations in the last hour and the last 24 hours.
    pub ai_last_hour: usize,
    pub ai_last_day: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Run it; `ask` says whether its "Ask Kairos" may start, or why not.
    Run { ask: Result<(), String> },
    /// It matched but is held back, with the reason (recorded in the history).
    Skip(String),
}

/// 0 = Sunday. Day 0 of the Unix epoch was a Thursday.
fn weekday(day: i64) -> u8 {
    (day + 4).rem_euclid(7) as u8
}

/// `None` when the automation is off, waits for something else, or its "only if" doesn't hold.
pub fn decide(automation: &Automation, happening: &Happening, moment: &Moment) -> Option<Verdict> {
    if !automation.enabled || !automation.trigger.matches(happening) {
        return None;
    }
    let (day, minute) = local_day_and_minute(moment.now, moment.utc_offset_min);
    let when = &automation.only_if;
    if let (Some(from), Some(to)) = (when.from_min, when.to_min)
        && from != to
        && !in_quiet_hours(minute, Some(from), Some(to))
    {
        return None;
    }
    if !when.days.is_empty() && !when.days.contains(&weekday(day)) {
        return None;
    }
    let on_battery = moment.on_battery.unwrap_or(false);
    match when.power {
        Some(Power::Battery) if !on_battery => return None,
        Some(Power::Plugged) if on_battery => return None,
        _ => {}
    }
    if !when.during_quiet_hours && in_quiet_hours(minute, moment.quiet_start, moment.quiet_end) {
        return Some(Verdict::Skip("quiet hours".into()));
    }
    let cooldown_s = i64::from(automation.cooldown_min.max(1)) * 60;
    if let Some(last) = moment.last_run_at
        && moment.now - last < cooldown_s
    {
        return Some(Verdict::Skip(format!(
            "ran less than {} min ago",
            automation.cooldown_min.max(1)
        )));
    }
    let ask = if !automation.actions.iter().any(|a| a.uses_ai()) {
        Ok(())
    } else if moment.ai_last_hour >= AI_PER_HOUR {
        Err(format!(
            "{AI_PER_HOUR} AI requests by automations in the last hour"
        ))
    } else if moment.ai_last_day >= AI_PER_DAY {
        Err(format!("{AI_PER_DAY} AI requests by automations today"))
    } else {
        Ok(())
    };
    Some(Verdict::Run { ask })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::{Action, Conditions, Trigger};

    /// 2026-10-25 00:00 UTC, a Sunday.
    const DAY: i64 = 1_792_886_400;

    fn at(hh: i64, mm: i64) -> i64 {
        DAY + hh * 3600 + mm * 60
    }

    fn automation(actions: Vec<Action>) -> Automation {
        Automation {
            id: "a".into(),
            name: "Test".into(),
            enabled: true,
            trigger: Trigger::Login,
            only_if: Conditions::default(),
            actions,
            cooldown_min: 10,
        }
    }

    fn moment(now: i64) -> Moment {
        Moment {
            now,
            utc_offset_min: 0,
            on_battery: None,
            quiet_start: Some(22 * 60),
            quiet_end: Some(7 * 60),
            last_run_at: None,
            ai_last_hour: 0,
            ai_last_day: 0,
        }
    }

    fn notify() -> Action {
        Action::Notify { text: "hi".into() }
    }

    fn ask() -> Action {
        Action::Ask { text: "hi".into() }
    }

    const RUN: Option<Verdict> = Some(Verdict::Run { ask: Ok(()) });

    #[test]
    fn day_zero_of_the_test_is_a_sunday() {
        assert_eq!(weekday(local_day_and_minute(DAY, 0).0), 0);
    }

    #[test]
    fn an_automation_runs_when_it_matches_and_not_when_off_or_waiting_for_something_else() {
        let a = automation(vec![notify()]);
        assert_eq!(decide(&a, &Happening::Login, &moment(at(9, 0))), RUN);
        let off = Automation {
            enabled: false,
            ..a.clone()
        };
        assert_eq!(decide(&off, &Happening::Login, &moment(at(9, 0))), None);
        assert_eq!(
            decide(&a, &Happening::Back { away_s: 9999 }, &moment(at(9, 0))),
            None
        );
    }

    #[test]
    fn an_automation_in_its_cooldown_is_skipped_with_the_reason() {
        let a = automation(vec![notify()]);
        let mut m = moment(at(9, 0));
        m.last_run_at = Some(at(8, 55));
        assert_eq!(
            decide(&a, &Happening::Login, &m),
            Some(Verdict::Skip("ran less than 10 min ago".into()))
        );
        m.last_run_at = Some(at(8, 50));
        assert_eq!(decide(&a, &Happening::Login, &m), RUN);
    }

    #[test]
    fn the_seventh_ai_request_in_an_hour_is_held_back_but_the_rest_still_runs() {
        let a = automation(vec![notify(), ask()]);
        let mut m = moment(at(9, 0));
        m.ai_last_hour = 6;
        assert_eq!(
            decide(&a, &Happening::Login, &m),
            Some(Verdict::Run {
                ask: Err("6 AI requests by automations in the last hour".into())
            })
        );
        m.ai_last_hour = 2;
        m.ai_last_day = 30;
        assert!(matches!(
            decide(&a, &Happening::Login, &m),
            Some(Verdict::Run { ask: Err(_) })
        ));
        // An automation without "Ask Kairos" isn't limited.
        let quiet = automation(vec![notify()]);
        assert_eq!(decide(&quiet, &Happening::Login, &m), RUN);
    }

    #[test]
    fn quiet_hours_hold_back_an_automation_unless_it_is_allowed_through() {
        let a = automation(vec![notify()]);
        assert_eq!(
            decide(&a, &Happening::Login, &moment(at(23, 0))),
            Some(Verdict::Skip("quiet hours".into()))
        );
        let through = Automation {
            only_if: Conditions {
                during_quiet_hours: true,
                ..Conditions::default()
            },
            ..a
        };
        assert_eq!(decide(&through, &Happening::Login, &moment(at(23, 0))), RUN);
    }

    #[test]
    fn the_time_window_days_and_power_must_all_hold() {
        let mut a = automation(vec![notify()]);
        a.only_if = Conditions {
            from_min: Some(8 * 60),
            to_min: Some(12 * 60),
            days: vec![1, 2, 3, 4, 5],
            power: Some(Power::Battery),
            during_quiet_hours: false,
        };
        let monday_10 = at(24 + 10, 0);
        let mut m = moment(monday_10);
        m.on_battery = Some(true);
        assert_eq!(decide(&a, &Happening::Login, &m), RUN);
        m.now = at(24 + 13, 0);
        assert_eq!(decide(&a, &Happening::Login, &m), None, "after the window");
        m.now = at(10, 0);
        assert_eq!(decide(&a, &Happening::Login, &m), None, "a Sunday");
        m.now = monday_10;
        m.on_battery = None;
        assert_eq!(
            decide(&a, &Happening::Login, &m),
            None,
            "no battery counts as plugged in"
        );
    }

    #[test]
    fn local_time_follows_the_utc_offset() {
        let a = automation(vec![notify()]);
        let mut m = moment(at(20, 30));
        m.utc_offset_min = 120; // 22:30 local: quiet
        assert!(matches!(
            decide(&a, &Happening::Login, &m),
            Some(Verdict::Skip(_))
        ));
    }
}
