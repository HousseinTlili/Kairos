//! Per-second rates from cumulative counters (disk bytes read and written, network bytes).
//!
//! sysinfo also reports "bytes since the last refresh", but that depends on every refresh being
//! seen; totals and our own clock survive a skipped sample or a slow tick.

use std::collections::HashMap;
use std::time::Instant;

/// Bytes per second between two readings of a counter that only goes up. `None` when there is no
/// earlier reading or no time has passed. A counter that went *down* (a drive re-plugged, an
/// adapter reset) is a new counter, not a negative rate: also `None`.
pub fn per_second(previous: Option<u64>, current: u64, elapsed_secs: f64) -> Option<f64> {
    let previous = previous?;
    if elapsed_secs <= 0.0 || current < previous {
        return None;
    }
    Some((current - previous) as f64 / elapsed_secs)
}

/// Remembers the last value of each named counter and when it was read.
#[derive(Debug, Default)]
pub struct Counters {
    last: HashMap<String, u64>,
    at: Option<Instant>,
}

impl Counters {
    /// Starts a new round of readings; returns the seconds since the previous round.
    pub fn begin(&mut self, now: Instant) -> f64 {
        let elapsed = self.at.map_or(0.0, |at| (now - at).as_secs_f64());
        self.at = Some(now);
        elapsed
    }

    /// The rate for one counter in this round, and remembers its new value.
    pub fn rate(&mut self, name: &str, current: u64, elapsed_secs: f64) -> Option<f64> {
        let previous = self.last.insert(name.to_string(), current);
        per_second(previous, current, elapsed_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_rate_is_the_increase_divided_by_the_seconds_elapsed() {
        assert_eq!(per_second(Some(1_000), 5_000, 2.0), Some(2_000.0));
        assert_eq!(per_second(Some(7), 7, 2.0), Some(0.0));
    }

    #[test]
    fn the_first_reading_has_no_rate_yet() {
        assert_eq!(per_second(None, 5_000, 2.0), None);
    }

    #[test]
    fn a_counter_that_restarts_from_zero_is_not_a_negative_rate() {
        assert_eq!(per_second(Some(9_000), 100, 2.0), None);
    }

    #[test]
    fn no_time_elapsed_gives_no_rate_instead_of_dividing_by_zero() {
        assert_eq!(per_second(Some(1), 2, 0.0), None);
    }

    #[test]
    fn counters_track_each_name_separately_across_rounds() {
        let start = Instant::now();
        let mut counters = Counters::default();
        let elapsed = counters.begin(start);
        assert_eq!(counters.rate("C:", 100, elapsed), None);
        assert_eq!(counters.rate("D:", 50, elapsed), None);

        let elapsed = counters.begin(start + Duration::from_secs(2));
        assert_eq!(elapsed, 2.0);
        assert_eq!(counters.rate("C:", 300, elapsed), Some(100.0));
        assert_eq!(counters.rate("D:", 50, elapsed), Some(0.0));
        assert_eq!(counters.rate("E:", 10, elapsed), None);
    }
}
