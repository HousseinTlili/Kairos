//! What each poll of Windows's notifications adds to the store (M4 §5). Pure: the Windows side
//! (Step 4) turns `UserNotification`s into `Observed` values and stores what `poll` returns.

use std::collections::HashSet;

use serde::Serialize;

use super::store::NewNotification;

/// Longest body kept; a notification is a preview, not the message.
pub const BODY_MAX: usize = 1000;
const TITLE_MAX: usize = 200;

/// One notification as Windows shows it in Action Center.
#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    /// AppUserModelID, e.g. `Microsoft.YourPhone_8wekyb3d8bbwe!App`.
    pub app_id: String,
    /// Display name, e.g. `Phone Link`.
    pub app_name: String,
    pub windows_id: u32,
    /// Unix seconds.
    pub created_at: i64,
    /// The toast's text lines, in order: usually sender (or caller), then the message.
    pub texts: Vec<String>,
}

/// Kairos's own toasts (M3 alerts) are not notifications to collect: Tauri shows them under the
/// app's identifier.
pub const OWN_APP_ID: &str = "com.kairos.desktop";

pub fn is_own(observed: &Observed) -> bool {
    observed.app_id.to_lowercase().contains(OWN_APP_ID) || observed.app_name.trim() == "Kairos"
}

/// Cuts `text` to at most `max` characters (not bytes), marking the cut.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The first non-empty line is the title (the sender or caller); the rest is the body. A toast
/// with no text at all is titled with the app's name, so the panel never shows a blank row.
pub fn to_new(observed: &Observed) -> NewNotification {
    let mut lines = observed
        .texts
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty());
    let title = lines
        .next()
        .map_or_else(|| observed.app_name.clone(), str::to_string);
    let body = lines.collect::<Vec<_>>().join("\n");
    NewNotification {
        app_id: observed.app_id.clone(),
        app_name: if observed.app_name.trim().is_empty() {
            observed.app_id.clone()
        } else {
            observed.app_name.trim().to_string()
        },
        windows_id: observed.windows_id,
        created_at: observed.created_at,
        title: cut(&title, TITLE_MAX),
        body: cut(&body, BODY_MAX),
    }
}

type Key = (String, u32, i64);

fn key(observed: &Observed) -> Key {
    (
        observed.app_id.clone(),
        observed.windows_id,
        observed.created_at,
    )
}

/// Remembers what the previous polls already handed over, so each poll returns only arrivals.
/// (The table's unique key is the real guard; this keeps the store from being asked every 3 s.)
#[derive(Debug)]
pub struct Collector {
    /// Unix seconds when collection was switched on: older notifications arrived before
    /// permission and are left alone (ROADMAP M4).
    since: i64,
    seen: HashSet<Key>,
}

impl Collector {
    pub fn new(since: i64) -> Self {
        Self {
            since,
            seen: HashSet::new(),
        }
    }

    pub fn since(&self) -> i64 {
        self.since
    }

    /// The notifications in this poll that are new, collectable and not Kairos's own.
    pub fn poll(&mut self, observed: &[Observed]) -> Vec<NewNotification> {
        let current: HashSet<Key> = observed.iter().map(key).collect();
        let fresh = observed
            .iter()
            .filter(|o| o.created_at >= self.since && !is_own(o) && !self.seen.contains(&key(o)))
            .map(to_new)
            .collect();
        // Forget what has left Action Center, so the set never grows past what Windows holds.
        self.seen = current;
        fresh
    }
}

/// Windows's answer to "may Kairos read notifications?" (`UserNotificationListenerAccessStatus`),
/// plus `Unavailable` when the listener can't be used at all (not Windows, or refused outright).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Allowed,
    Denied,
    Unspecified,
    Unavailable,
}

/// What Settings ▸ Notifications shows, and whether the poller runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectState {
    /// The Kairos switch is off: nothing is read, whatever Windows allows.
    Off,
    /// Switched on in Kairos, but Windows hasn't allowed it (yet).
    WaitingForWindows,
    /// Kairos can't read notifications on this PC.
    Unavailable,
    On,
}

/// Both the Kairos switch and Windows must say yes (D-011).
pub fn collect_state(enabled: bool, access: Access) -> CollectState {
    match (enabled, access) {
        (false, _) => CollectState::Off,
        (true, Access::Allowed) => CollectState::On,
        (true, Access::Unavailable) => CollectState::Unavailable,
        (true, Access::Denied | Access::Unspecified) => CollectState::WaitingForWindows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented apps and senders only (fixtures/README.md).
    fn seen(app: &str, id: u32, at: i64, texts: &[&str]) -> Observed {
        Observed {
            app_id: format!("Example.{app}!App"),
            app_name: app.to_string(),
            windows_id: id,
            created_at: at,
            texts: texts.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn the_first_line_is_the_sender_and_the_rest_is_the_message() {
        let n = to_new(&seen(
            "Phone Link",
            1,
            10,
            &["Sam", "Running late", "Be there at 8"],
        ));
        assert_eq!(n.title, "Sam");
        assert_eq!(n.body, "Running late\nBe there at 8");
        assert_eq!(n.app_name, "Phone Link");
    }

    #[test]
    fn a_toast_without_text_is_titled_with_its_app() {
        let n = to_new(&seen("Discord", 1, 10, &["", "  "]));
        assert_eq!((n.title.as_str(), n.body.as_str()), ("Discord", ""));
    }

    #[test]
    fn long_text_is_cut_on_characters_not_bytes() {
        let long = "é".repeat(BODY_MAX + 50);
        let n = to_new(&seen("Teams", 1, 10, &["Alex", &long]));
        assert_eq!(n.body.chars().count(), BODY_MAX);
        assert!(n.body.ends_with('…'));
    }

    #[test]
    fn each_notification_is_handed_over_once_across_polls() {
        let mut collector = Collector::new(0);
        let a = seen("Discord", 1, 10, &["Alex", "hi"]);
        let b = seen("Discord", 2, 20, &["Sam", "hey"]);
        assert_eq!(collector.poll(std::slice::from_ref(&a)).len(), 1);
        let second = collector.poll(&[a.clone(), b]);
        assert_eq!(
            second.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
            ["Sam"]
        );
        assert!(collector.poll(&[a]).is_empty());
    }

    #[test]
    fn notifications_from_before_the_switch_was_turned_on_are_left_alone() {
        let mut collector = Collector::new(1_000);
        let before = seen("Outlook", 1, 999, &["Old"]);
        let after = seen("Outlook", 2, 1_000, &["New"]);
        let fresh = collector.poll(&[before, after]);
        assert_eq!(
            fresh.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
            ["New"]
        );
    }

    #[test]
    fn kairos_own_alert_toasts_are_skipped() {
        let mut collector = Collector::new(0);
        let own = Observed {
            app_id: "com.kairos.desktop".into(),
            app_name: "Kairos".into(),
            windows_id: 1,
            created_at: 5,
            texts: vec!["Battery low".into()],
        };
        assert!(is_own(&own));
        assert!(collector.poll(&[own]).is_empty());
    }

    #[test]
    fn the_seen_set_only_holds_what_windows_still_shows() {
        let mut collector = Collector::new(0);
        collector.poll(&[seen("A", 1, 1, &["x"]), seen("A", 2, 2, &["y"])]);
        collector.poll(&[seen("A", 2, 2, &["y"])]);
        assert_eq!(collector.seen.len(), 1);
    }

    #[test]
    fn collection_needs_both_the_kairos_switch_and_windows() {
        assert_eq!(collect_state(false, Access::Allowed), CollectState::Off);
        assert_eq!(collect_state(true, Access::Allowed), CollectState::On);
        assert_eq!(
            collect_state(true, Access::Denied),
            CollectState::WaitingForWindows
        );
        assert_eq!(
            collect_state(true, Access::Unspecified),
            CollectState::WaitingForWindows
        );
        assert_eq!(
            collect_state(true, Access::Unavailable),
            CollectState::Unavailable
        );
    }
}
