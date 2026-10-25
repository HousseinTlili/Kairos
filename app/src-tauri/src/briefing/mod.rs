//! The welcome-back briefing (M5 §2): when to brief, the instant facts, and the settings.

pub mod rules;

use rules::{BriefingSettings, format_hhmm, parse_hhmm};

use crate::store::{StoreError, TaskStore};

const ENABLED: &str = "briefing_enabled";
const MIN_AWAY: &str = "briefing_min_away_min";
const QUIET_START: &str = "briefing_quiet_start";
const QUIET_END: &str = "briefing_quiet_end";
const AI_SUMMARY: &str = "briefing_ai_summary";
const PROJECTS: &str = "briefing_projects";
pub const LAST_BRIEFING: &str = "last_briefing";

/// The stored settings; anything missing or unreadable falls back to its default.
pub fn load_settings(store: &TaskStore) -> Result<BriefingSettings, StoreError> {
    let defaults = BriefingSettings::default();
    let flag = |key: &str, default: bool| -> Result<bool, StoreError> {
        Ok(match store.setting(key)?.as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        })
    };
    // An empty quiet-hours value means "no quiet hours"; a missing one means the default.
    let quiet = |key: &str, default: Option<u16>| -> Result<Option<u16>, StoreError> {
        Ok(match store.setting(key)? {
            None => default,
            Some(text) if text.trim().is_empty() => None,
            Some(text) => parse_hhmm(&text).or(default),
        })
    };
    Ok(BriefingSettings {
        enabled: flag(ENABLED, defaults.enabled)?,
        min_away_min: store
            .setting(MIN_AWAY)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.min_away_min),
        quiet_start: quiet(QUIET_START, defaults.quiet_start)?,
        quiet_end: quiet(QUIET_END, defaults.quiet_end)?,
        ai_summary: flag(AI_SUMMARY, defaults.ai_summary)?,
        projects: store
            .setting(PROJECTS)?
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or(defaults.projects),
    })
}

pub fn save_settings(store: &TaskStore, settings: &BriefingSettings) -> Result<(), StoreError> {
    let bool_text = |b: bool| if b { "true" } else { "false" };
    store.set_setting(ENABLED, bool_text(settings.enabled))?;
    store.set_setting(MIN_AWAY, &settings.min_away_min.to_string())?;
    store.set_setting(
        QUIET_START,
        &settings.quiet_start.map(format_hhmm).unwrap_or_default(),
    )?;
    store.set_setting(
        QUIET_END,
        &settings.quiet_end.map(format_hhmm).unwrap_or_default(),
    )?;
    store.set_setting(AI_SUMMARY, bool_text(settings.ai_summary))?;
    let projects = serde_json::to_string(&settings.projects)
        .map_err(|err| StoreError::new(err.to_string()))?;
    store.set_setting(PROJECTS, &projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_stored_means_the_defaults() {
        let store = TaskStore::open_in_memory().expect("store");
        assert_eq!(
            load_settings(&store).expect("load"),
            BriefingSettings::default()
        );
    }

    #[test]
    fn settings_survive_a_save_and_load_including_no_quiet_hours() {
        let store = TaskStore::open_in_memory().expect("store");
        let settings = BriefingSettings {
            enabled: false,
            min_away_min: 15,
            quiet_start: None,
            quiet_end: None,
            ai_summary: false,
            projects: vec![r"C:\Users\me\Projects\Kairos".into()],
        };
        save_settings(&store, &settings).expect("save");
        assert_eq!(load_settings(&store).expect("load"), settings);
    }

    #[test]
    fn an_unreadable_value_falls_back_to_its_default() {
        let store = TaskStore::open_in_memory().expect("store");
        store.set_setting(MIN_AWAY, "soon").expect("set");
        store.set_setting(QUIET_START, "25:99").expect("set");
        let settings = load_settings(&store).expect("load");
        assert_eq!(settings.min_away_min, 60);
        assert_eq!(settings.quiet_start, Some(22 * 60));
    }
}
