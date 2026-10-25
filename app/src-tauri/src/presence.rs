//! When the user comes back to the PC (M3 §8, D-013): app start, resume from sleep, and unlock.
//! M3 records them in `kairos.db` and tells the app; M5's welcome-back briefing builds on them,
//! using how long the user was away (`away_s`).
//!
//! No window messages: a thread ticks every 5 s. A long gap in wall-clock time between two ticks
//! means the PC was asleep (the thread doesn't run while it sleeps), and on Windows the session's
//! lock state is read on each tick (`WTSQuerySessionInformationW`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub const TICK: Duration = Duration::from_secs(5);
/// More than this between 5 s ticks means the PC slept, not that the thread was slow.
pub const SLEEP_GAP: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceKind {
    Startup,
    Resume,
    Unlock,
}

impl PresenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PresenceKind::Startup => "startup",
            PresenceKind::Resume => "resume",
            PresenceKind::Unlock => "unlock",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "startup" => PresenceKind::Startup,
            "resume" => PresenceKind::Resume,
            "unlock" => PresenceKind::Unlock,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PresenceEvent {
    /// Unix seconds.
    pub at: i64,
    pub kind: PresenceKind,
    /// "autostart" for a login start; the gap for a resume ("slept 2 h 5 min"); the time locked
    /// for an unlock ("away 40 min").
    pub detail: Option<String>,
    /// How long the user was away, in seconds, when known (M5 §2.1). Not stored in `kairos.db`.
    #[serde(default)]
    pub away_s: Option<i64>,
}

/// Spots sleep from the wall clock jumping between ticks.
#[derive(Debug, Default)]
pub struct ResumeDetector {
    last: Option<SystemTime>,
}

impl ResumeDetector {
    /// The length of the sleep, if the PC slept since the previous tick. A clock set backwards is
    /// not a sleep.
    pub fn tick(&mut self, now: SystemTime) -> Option<Duration> {
        let gap = self.last.and_then(|last| now.duration_since(last).ok());
        self.last = Some(now);
        gap.filter(|gap| *gap > SLEEP_GAP)
    }
}

/// Spots unlock from the lock state going from locked to unlocked, and times the lock.
#[derive(Debug, Default)]
pub struct UnlockDetector {
    locked: Option<bool>,
    /// When the lock was first seen (a tick after it happened, so up to 5 s late).
    locked_since: Option<SystemTime>,
}

impl UnlockDetector {
    /// `locked` is `None` when the state couldn't be read; that keeps the last known state. On an
    /// unlock, returns how long the session was locked (zero if the clock went backwards).
    pub fn feed(&mut self, locked: Option<bool>, now: SystemTime) -> Option<Duration> {
        let locked = locked?;
        let was_locked = self.locked == Some(true);
        self.locked = Some(locked);
        match (was_locked, locked) {
            (false, true) => {
                self.locked_since = Some(now);
                None
            }
            (true, false) => {
                let since = self.locked_since.take().unwrap_or(now);
                Some(now.duration_since(since).unwrap_or_default())
            }
            _ => None,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked == Some(true)
    }
}

pub fn describe_away(away: Duration) -> String {
    let minutes = away.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("away {m} min"),
        (h, 0) => format!("away {h} h"),
        (h, m) => format!("away {h} h {m} min"),
    }
}

/// How often the "PC is in use" heartbeat (`last_seen`) is written: startup measures time away
/// from it.
pub const HEARTBEAT_EVERY_TICKS: u32 = 12; // one minute

pub fn describe_gap(gap: Duration) -> String {
    let minutes = gap.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("slept {m} min"),
        (h, 0) => format!("slept {h} h"),
        (h, m) => format!("slept {h} h {m} min"),
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Whether this Windows session is locked; `None` when it can't be told.
#[cfg(windows)]
pub fn session_locked() -> Option<bool> {
    use windows_sys::Win32::System::RemoteDesktop::{
        WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK,
        WTS_SESSIONSTATE_UNLOCK, WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW,
        WTSSessionInfoEx,
    };
    let mut buffer: windows_sys::core::PWSTR = std::ptr::null_mut();
    let mut bytes: u32 = 0;
    // SAFETY: Windows allocates `buffer` and fills `bytes`; it is read only when the call
    // succeeded and is at least one WTSINFOEXW long, then released with WTSFreeMemory.
    unsafe {
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        ) == 0
            || buffer.is_null()
        {
            return None;
        }
        let state = if (bytes as usize) >= std::mem::size_of::<WTSINFOEXW>() {
            let info = &*(buffer as *const WTSINFOEXW);
            if info.Level == 1 {
                match info.Data.WTSInfoExLevel1.SessionFlags as u32 {
                    WTS_SESSIONSTATE_LOCK => Some(true),
                    WTS_SESSIONSTATE_UNLOCK => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        WTSFreeMemory(buffer.cast());
        state
    }
}

#[cfg(not(windows))]
pub fn session_locked() -> Option<bool> {
    None
}

/// Ticks forever, calling `on_event` for each resume and unlock, and `on_heartbeat` once a minute
/// while the session is unlocked. Runs on its own thread.
pub fn watch(
    mut on_event: impl FnMut(PresenceEvent) + Send + 'static,
    mut on_heartbeat: impl FnMut(i64) + Send + 'static,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("kairos-presence".into())
        .spawn(move || {
            let mut resume = ResumeDetector::default();
            let mut unlock = UnlockDetector::default();
            let mut ticks: u32 = 0;
            loop {
                let now = SystemTime::now();
                if let Some(gap) = resume.tick(now) {
                    on_event(PresenceEvent {
                        at: unix_now(),
                        kind: PresenceKind::Resume,
                        detail: Some(describe_gap(gap)),
                        away_s: Some(gap.as_secs() as i64),
                    });
                }
                if let Some(away) = unlock.feed(session_locked(), now) {
                    on_event(PresenceEvent {
                        at: unix_now(),
                        kind: PresenceKind::Unlock,
                        detail: Some(describe_away(away)),
                        away_s: Some(away.as_secs() as i64),
                    });
                }
                ticks = ticks.wrapping_add(1);
                if ticks.is_multiple_of(HEARTBEAT_EVERY_TICKS) && !unlock.is_locked() {
                    on_heartbeat(unix_now());
                }
                std::thread::sleep(TICK);
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_ticks_are_not_a_sleep_but_a_long_gap_is() {
        let start = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let mut detector = ResumeDetector::default();
        assert_eq!(detector.tick(start), None, "nothing to compare yet");
        assert_eq!(detector.tick(start + Duration::from_secs(5)), None);
        assert_eq!(
            detector.tick(start + Duration::from_secs(50)),
            None,
            "a slow tick"
        );
        let woke = start + Duration::from_secs(50 + 2 * 3600);
        assert_eq!(detector.tick(woke), Some(Duration::from_secs(2 * 3600)));
        assert_eq!(detector.tick(woke + Duration::from_secs(5)), None);
    }

    #[test]
    fn a_clock_set_backwards_is_not_a_sleep() {
        let start = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let mut detector = ResumeDetector::default();
        detector.tick(start);
        assert_eq!(detector.tick(start - Duration::from_secs(3600)), None);
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000 + secs)
    }

    #[test]
    fn unlock_is_locked_then_unlocked_and_nothing_else() {
        let mut detector = UnlockDetector::default();
        assert_eq!(
            detector.feed(Some(false), at(0)),
            None,
            "already unlocked at start"
        );
        assert_eq!(detector.feed(Some(true), at(5)), None);
        assert_eq!(
            detector.feed(None, at(10)),
            None,
            "an unreadable state keeps the last one"
        );
        assert!(detector.feed(Some(false), at(15)).is_some());
        assert_eq!(detector.feed(Some(false), at(20)), None);
    }

    #[test]
    fn an_unlock_reports_how_long_the_session_was_locked() {
        let mut detector = UnlockDetector::default();
        detector.feed(Some(false), at(0));
        detector.feed(Some(true), at(100));
        detector.feed(Some(true), at(105));
        assert_eq!(
            detector.feed(Some(false), at(100 + 40 * 60)),
            Some(Duration::from_secs(40 * 60))
        );
        assert!(!detector.is_locked());
    }

    #[test]
    fn starting_while_locked_then_unlocking_counts_from_the_first_tick() {
        let mut detector = UnlockDetector::default();
        assert_eq!(detector.feed(Some(true), at(0)), None);
        assert!(detector.is_locked());
        assert_eq!(
            detector.feed(Some(false), at(30)),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn time_away_reads_naturally() {
        assert_eq!(describe_away(Duration::from_secs(40 * 60)), "away 40 min");
        assert_eq!(describe_away(Duration::from_secs(2 * 3600)), "away 2 h");
        assert_eq!(
            describe_away(Duration::from_secs(3600 + 600)),
            "away 1 h 10 min"
        );
    }

    #[test]
    fn sleep_lengths_read_naturally() {
        assert_eq!(describe_gap(Duration::from_secs(5 * 60)), "slept 5 min");
        assert_eq!(describe_gap(Duration::from_secs(3 * 3600)), "slept 3 h");
        assert_eq!(
            describe_gap(Duration::from_secs(3600 + 125)),
            "slept 1 h 2 min"
        );
    }

    #[test]
    fn kinds_survive_the_database_round_trip() {
        for kind in [
            PresenceKind::Startup,
            PresenceKind::Resume,
            PresenceKind::Unlock,
        ] {
            assert_eq!(PresenceKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(PresenceKind::parse("nap"), None);
    }
}
