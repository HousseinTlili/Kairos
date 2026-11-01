//! The switch for Hermes's `computer_use` on the API Kairos uses (M20, D-040): off by default, turned
//! on and off by the Master's click.
//!
//! Both `hermes tools enable` and `hermes config unset` strip comments from `config.yaml` (seen
//! 2026-11-01: 236 lines of them), so this only uses `hermes config set platform_toolsets.api_server`.
//! The cost: after the first use, an explicit list of toolsets stays in the file, turned on or off. It
//! won't turn on without the rules that keep it out of Kairos's own windows, terminals and the rest.
//! The pure parts are tested against fixtures.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::hermes::local::run_cli_output;

const TOOL: &str = "computer_use";
const KEY: &str = "platform_toolsets.api_server";
const LIMIT: Duration = Duration::from_secs(30);
/// The rule without which this must not turn on.
const GUARD_RULE: &str = "computer-use-off-limits";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComputerUseState {
    pub on: bool,
    /// `cua-driver` is installed and its runtime answers.
    pub ready: bool,
    /// The rules are in `policy.json` and on.
    pub rules_ready: bool,
}

/// The toolsets shown as enabled by `hermes tools list --platform api_server`.
fn enabled_toolsets(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix("✓ enabled"))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

fn driver_ready(text: &str) -> bool {
    text.contains("Runtime contract ready")
}

/// Whether `policy.json` has the guarding rule, on. A file that can't be read doesn't.
fn guard_rule_on(policy: &str) -> bool {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(policy) else {
        return false;
    };
    doc.get("rules")
        .and_then(|rules| rules.as_array())
        .is_some_and(|rules| {
            rules.iter().any(|rule| {
                rule.get("id").and_then(|id| id.as_str()) == Some(GUARD_RULE)
                    && rule.get("enabled").and_then(|on| on.as_bool()) != Some(false)
            })
        })
}

fn with_tool(enabled: &[String], on: bool) -> Vec<String> {
    let mut list: Vec<String> = enabled.iter().filter(|t| *t != TOOL).cloned().collect();
    if on {
        list.push(TOOL.to_string());
    }
    list
}

fn run(home: &Path, args: &[&str]) -> Result<(bool, String), String> {
    run_cli_output(home, args, LIMIT)
}

fn enabled_now(home: &Path) -> Result<Vec<String>, String> {
    let (ok, text) = run(home, &["tools", "list", "--platform", "api_server"])?;
    if !ok {
        return Err("Hermes couldn't list its tools.".into());
    }
    Ok(enabled_toolsets(&text))
}

pub fn state(home: &Path, policy: Option<&str>) -> Result<ComputerUseState, String> {
    let on = enabled_now(home)?.iter().any(|t| t == TOOL);
    let (_, status) = run(home, &["computer-use", "status"])?;
    Ok(ComputerUseState {
        on,
        ready: driver_ready(&status),
        rules_ready: policy.is_some_and(guard_rule_on),
    })
}

pub fn turn_on(home: &Path, policy: Option<&str>) -> Result<(), String> {
    if !policy.is_some_and(guard_rule_on) {
        return Err(
            "Kairos's rules for computer use aren't in place. In Settings ▸ Rules, press \"Add the new rules\" and Save first."
                .into(),
        );
    }
    set_list(home, &with_tool(&enabled_now(home)?, true))
}

pub fn turn_off(home: &Path) -> Result<(), String> {
    set_list(home, &with_tool(&enabled_now(home)?, false))
}

fn set_list(home: &Path, list: &[String]) -> Result<(), String> {
    let text = serde_json::to_string(list).map_err(|err| err.to_string())?;
    let (ok, _) = run(home, &["config", "set", KEY, &text])?;
    if ok {
        Ok(())
    } else {
        Err("Hermes didn't save the change.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/hermes-0.22.5")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    #[test]
    fn off_is_how_it_starts_and_reads_as_off() {
        let enabled = enabled_toolsets(&fixture("computer-use/tools-list-off.txt"));
        assert!(!enabled.iter().any(|t| t == TOOL));
        assert!(
            enabled.iter().any(|t| t == "kairos"),
            "plugin toolsets count"
        );
        assert!(enabled.iter().any(|t| t == "terminal"));
        assert!(!enabled.iter().any(|t| t == "a2a"), "disabled ones don't");
    }

    #[test]
    fn turning_on_adds_it_to_what_was_on_and_nothing_else() {
        let enabled = vec!["web".to_string(), "kairos".to_string()];
        assert_eq!(with_tool(&enabled, true), vec!["web", "kairos", TOOL]);
    }

    #[test]
    fn turning_off_only_takes_it_out_of_the_list() {
        let enabled = vec!["web".to_string(), TOOL.to_string(), "kairos".to_string()];
        assert_eq!(with_tool(&enabled, false), vec!["web", "kairos"]);
    }

    #[test]
    fn the_driver_reads_as_ready_from_hermess_status() {
        assert!(driver_ready(&fixture("computer-use/status.txt")));
        assert!(!driver_ready("cua-driver: not installed"));
    }

    #[test]
    fn the_guard_rule_must_be_there_and_on() {
        let starter = include_str!("../../../plugin/kairos/policy.default.json");
        assert!(guard_rule_on(starter));
        assert!(!guard_rule_on(r#"{"rules": []}"#));
        assert!(!guard_rule_on(
            r#"{"rules": [{"id": "computer-use-off-limits", "enabled": false}]}"#
        ));
        assert!(!guard_rule_on("not json"));
    }

    #[test]
    fn it_wont_turn_on_without_the_rules() {
        let nowhere = Path::new("nowhere");
        let err = turn_on(nowhere, Some(r#"{"rules": []}"#)).expect_err("refused");
        assert!(err.contains("Add the new rules"));
        assert!(turn_on(nowhere, None).is_err());
    }
}
