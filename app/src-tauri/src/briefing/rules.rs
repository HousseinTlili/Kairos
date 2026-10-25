//! When to brief (M5 §2.1). Pure: the trigger, the clock and the settings in, a reason or nothing
//! out.

use serde::{Deserialize, Serialize};

pub const MINUTES_PER_DAY: i64 = 24 * 60;
/// A wake is often followed by an unlock a few seconds later: one briefing, not two.
pub const MIN_GAP_BETWEEN_BRIEFINGS_S: i64 = 10 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefingSettings {
    pub enabled: bool,
    /// Brief after at least this long away (besides the first return of the day).
    pub min_away_min: u32,
    /// Quiet hours as minutes after local midnight; `None` (or start == end) means no quiet hours.
    pub quiet_start: Option<u16>,
    pub quiet_end: Option<u16>,
    /// Start the AI summary by itself after the facts.
    pub ai_summary: bool,
    /// Folders the AI summary may check (`git status`, failing tests), read-only by instruction.
    pub projects: Vec<String>,
}

impl Default for BriefingSettings {
    /// M5 §2 defaults; the product owner can change them in Settings ▸ Briefing.
    fn default() -> Self {
        Self {
            enabled: true,
            min_away_min: 60,
            quiet_start: Some(22 * 60),
            quiet_end: Some(7 * 60),
            ai_summary: true,
            projects: Vec::new(),
        }
    }
}

/// "07:30" → 450. Hours 0–23, minutes 0–59.
pub fn parse_hhmm(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn format_hhmm(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Whether `minute` (after local midnight) falls in quiet hours. A range like 22:00–07:00 runs
/// over midnight; the end minute itself is outside.
pub fn in_quiet_hours(minute: u16, start: Option<u16>, end: Option<u16>) -> bool {
    match (start, end) {
        (Some(start), Some(end)) if start != end => {
            if start < end {
                (start..end).contains(&minute)
            } else {
                minute >= start || minute < end
            }
        }
        _ => false,
    }
}

/// The local day number and minute of the day for a Unix time and a UTC offset.
pub fn local_day_and_minute(unix: i64, utc_offset_min: i32) -> (i64, u16) {
    let local_minutes = unix.div_euclid(60) + i64::from(utc_offset_min);
    (
        local_minutes.div_euclid(MINUTES_PER_DAY),
        local_minutes.rem_euclid(MINUTES_PER_DAY) as u16,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The first return to the PC of the local day.
    FirstOfDay,
    /// Away at least the set time.
    Away,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// Unix seconds.
    pub now: i64,
    pub utc_offset_min: i32,
    /// How long the user was away, when known.
    pub away_s: Option<i64>,
    pub last_briefing_at: Option<i64>,
}

pub fn decide(settings: &BriefingSettings, moment: &Moment) -> Option<Reason> {
    if !settings.enabled {
        return None;
    }
    let (today, minute) = local_day_and_minute(moment.now, moment.utc_offset_min);
    if in_quiet_hours(minute, settings.quiet_start, settings.quiet_end) {
        return None;
    }
    if let Some(last) = moment.last_briefing_at
        && moment.now - last < MIN_GAP_BETWEEN_BRIEFINGS_S
    {
        return None;
    }
    let briefed_today = moment
        .last_briefing_at
        .is_some_and(|last| local_day_and_minute(last, moment.utc_offset_min).0 == today);
    if !briefed_today {
        return Some(Reason::FirstOfDay);
    }
    let away_enough = moment
        .away_s
        .is_some_and(|away| away >= i64::from(settings.min_away_min) * 60);
    away_enough.then_some(Reason::Away)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-25 00:00 UTC.
    const DAY: i64 = 1_792_886_400;

    fn at(hh: i64, mm: i64) -> i64 {
        DAY + hh * 3600 + mm * 60
    }

    fn moment(now: i64, away_min: Option<i64>, last: Option<i64>) -> Moment {
        Moment {
            now,
            utc_offset_min: 0,
            away_s: away_min.map(|m| m * 60),
            last_briefing_at: last,
        }
    }

    #[test]
    fn times_parse_and_print_as_hh_mm() {
        assert_eq!(parse_hhmm("07:30"), Some(450));
        assert_eq!(parse_hhmm(" 22:00 "), Some(1320));
        for bad in ["24:00", "7:60", "seven", "", "12"] {
            assert_eq!(parse_hhmm(bad), None, "{bad}");
        }
        assert_eq!(format_hhmm(450), "07:30");
    }

    #[test]
    fn quiet_hours_over_midnight_include_the_start_and_exclude_the_end() {
        let (s, e) = (Some(22 * 60), Some(7 * 60));
        assert!(in_quiet_hours(22 * 60, s, e));
        assert!(in_quiet_hours(3 * 60, s, e));
        assert!(in_quiet_hours(6 * 60 + 59, s, e));
        assert!(!in_quiet_hours(7 * 60, s, e));
        assert!(!in_quiet_hours(21 * 60 + 59, s, e));
    }

    #[test]
    fn a_daytime_range_and_an_empty_range() {
        assert!(in_quiet_hours(13 * 60, Some(12 * 60), Some(14 * 60)));
        assert!(!in_quiet_hours(14 * 60, Some(12 * 60), Some(14 * 60)));
        assert!(
            !in_quiet_hours(3 * 60, Some(600), Some(600)),
            "start == end: none"
        );
        assert!(!in_quiet_hours(3 * 60, None, Some(600)));
    }

    #[test]
    fn local_time_follows_the_utc_offset() {
        // 23:30 UTC is 00:30 the next day in UTC+1, and 18:30 the same day in UTC-5.
        assert_eq!(local_day_and_minute(at(23, 30), 60).1, 30);
        assert_eq!(
            local_day_and_minute(at(23, 30), 60).0,
            local_day_and_minute(at(23, 30), 0).0 + 1
        );
        assert_eq!(local_day_and_minute(at(23, 30), -300).1, 18 * 60 + 30);
    }

    #[test]
    fn the_first_return_of_the_day_briefs_even_after_a_short_break() {
        let s = BriefingSettings::default();
        let yesterday = at(20, 0) - 86_400;
        assert_eq!(
            decide(&s, &moment(at(8, 0), Some(2), Some(yesterday))),
            Some(Reason::FirstOfDay)
        );
        assert_eq!(
            decide(&s, &moment(at(8, 0), None, None)),
            Some(Reason::FirstOfDay),
            "never briefed"
        );
    }

    #[test]
    fn later_returns_brief_only_after_the_minimum_time_away() {
        let s = BriefingSettings::default();
        let morning = Some(at(8, 0));
        assert_eq!(decide(&s, &moment(at(13, 0), Some(59), morning)), None);
        assert_eq!(
            decide(&s, &moment(at(13, 0), Some(60), morning)),
            Some(Reason::Away),
            "exactly the minimum counts"
        );
        assert_eq!(
            decide(&s, &moment(at(13, 0), None, morning)),
            None,
            "unknown time away"
        );
    }

    #[test]
    fn quiet_hours_win_over_everything() {
        let s = BriefingSettings::default();
        assert_eq!(decide(&s, &moment(at(6, 59), Some(600), None)), None);
        assert_eq!(
            decide(&s, &moment(at(7, 0), Some(600), None)),
            Some(Reason::FirstOfDay)
        );
        assert_eq!(
            decide(&s, &moment(at(23, 0), Some(600), Some(at(8, 0)))),
            None
        );
    }

    #[test]
    fn a_wake_followed_by_an_unlock_gives_one_briefing() {
        let s = BriefingSettings::default();
        let woke = at(9, 0);
        assert_eq!(decide(&s, &moment(woke + 30, Some(600), Some(woke))), None);
        assert_eq!(
            decide(&s, &moment(woke + 10 * 60, Some(600), Some(woke))),
            Some(Reason::Away)
        );
    }

    #[test]
    fn switched_off_never_briefs() {
        let s = BriefingSettings {
            enabled: false,
            ..BriefingSettings::default()
        };
        assert_eq!(decide(&s, &moment(at(9, 0), Some(600), None)), None);
    }
}
