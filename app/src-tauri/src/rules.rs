//! `%APPDATA%\kairos\policy.json`: the Kairos rules, which the app owns and the plugin reads
//! (D-015, D-016, M2 Step 7).
//!
//! Validation runs the installed plugin's own `policy.py` on Hermes's bundled Python, so what the
//! Rules page accepts is exactly what the plugin accepts, and the app needs no regex crate.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::store::StoreError;

/// The starter rules, built into the app from the plugin's own copy.
pub const STARTER_RULES: &str = include_str!("../../../plugin/kairos/policy.default.json");

const VALIDATE_TIMEOUT: Duration = Duration::from_secs(15);

// Prints "ok" or "error: <reason>" for the rules on stdin, using the installed plugin's validator.
const VALIDATOR: &str = r#"
import sys
sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, sys.argv[1])
from kairos.policy import PolicyError, load_policy
try:
    load_policy(sys.stdin.buffer.read().decode("utf-8"))
    print("ok")
except PolicyError as err:
    print("error: " + str(err))
"#;

pub fn default_path() -> Result<PathBuf, StoreError> {
    crate::store::default_path().map(|db| db.with_file_name("policy.json"))
}

/// Where the plugin is installed and the Python that runs it.
#[derive(Debug, Clone)]
pub struct PluginRuntime {
    pub python: PathBuf,
    /// The folder that contains the `kairos` plugin package.
    pub plugins_dir: PathBuf,
}

impl PluginRuntime {
    /// `<hermes home>\tools\python-*\python.exe` and `<hermes home>\plugins`, or None when either
    /// is missing (Hermes not installed, or the Kairos plugin not installed).
    pub fn find(hermes_home: &Path) -> Option<Self> {
        let plugins_dir = hermes_home.join("plugins");
        if !plugins_dir.join("kairos").join("policy.py").is_file() {
            return None;
        }
        let mut pythons: Vec<PathBuf> = std::fs::read_dir(hermes_home.join("tools"))
            .ok()?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("python-"))
            })
            .map(|p| p.join("python.exe"))
            .filter(|p| p.is_file())
            .collect();
        // Newest first, so an upgraded Hermes's interpreter wins over a leftover old one.
        pythons.sort();
        let python = pythons.pop()?;
        Some(Self {
            python,
            plugins_dir,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Validation {
    Valid,
    Invalid {
        reason: String,
    },
    /// The plugin or Hermes's Python isn't there, so the rules can't be checked.
    Unavailable {
        reason: String,
    },
}

/// Checks `text` with the plugin's validator. Blocking (about 0.1 s); call it off the async runtime.
pub fn validate(runtime: Option<&PluginRuntime>, text: &str) -> Validation {
    let Some(runtime) = runtime else {
        return Validation::Unavailable {
            reason: "the Kairos plugin isn't installed in Hermes (scripts/install-plugin.ps1)"
                .into(),
        };
    };
    match run_validator(runtime, text) {
        Ok(out) if out == "ok" => Validation::Valid,
        Ok(out) => match out.strip_prefix("error: ") {
            Some(reason) => Validation::Invalid {
                reason: reason.to_string(),
            },
            None => Validation::Unavailable {
                reason: format!("unexpected validator output: {out}"),
            },
        },
        Err(reason) => Validation::Unavailable { reason },
    }
}

fn run_validator(runtime: &PluginRuntime, text: &str) -> Result<String, String> {
    let mut command = Command::new(&runtime.python);
    command
        .args(["-I", "-c", VALIDATOR])
        .arg(&runtime.plugins_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("couldn't start Hermes's Python: {err}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(text.as_bytes())
        .map_err(|err| format!("couldn't send the rules to the validator: {err}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < VALIDATE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                return Err("the rules validator didn't answer in time".into());
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("validator: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("no output");
        return Err(format!("the rules validator failed: {last}"));
    }
    Ok(stdout)
}

/// Writes `text` so the plugin never reads a half-written file: a temporary file in the same
/// folder, flushed to disk, then renamed over the old one (a replace on Windows).
pub fn write_atomically(path: &Path, text: &str) -> Result<(), StoreError> {
    let dir = path
        .parent()
        .ok_or_else(|| StoreError::new("the rules path has no folder"))?;
    std::fs::create_dir_all(dir).map_err(|err| StoreError::new(err.to_string()))?;
    let temp = dir.join(format!(".policy.json.{}.tmp", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(StoreError::new(format!("couldn't save the rules: {err}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("kairos-rules-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    /// The real plugin runtime on this machine, if Hermes and the plugin are installed.
    fn installed_runtime() -> Option<PluginRuntime> {
        crate::hermes::local::hermes_home()
            .ok()
            .and_then(|home| PluginRuntime::find(&home))
    }

    #[test]
    fn an_atomic_save_replaces_the_file_and_leaves_no_temp_file() {
        let dir = temp_dir("save");
        let path = dir.join("policy.json");
        write_atomically(&path, "first").expect("save");
        write_atomically(&path, "second").expect("save again");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "second");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .expect("list")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn without_the_plugin_validation_is_unavailable_not_valid() {
        let empty_home = temp_dir("nohermes");
        assert!(PluginRuntime::find(&empty_home).is_none());
        assert!(matches!(
            validate(None, STARTER_RULES),
            Validation::Unavailable { .. }
        ));
    }

    #[test]
    fn the_plugins_validator_accepts_the_starter_rules_and_names_what_is_wrong() {
        let Some(runtime) = installed_runtime() else {
            eprintln!("skipped: the Kairos plugin isn't installed on this machine");
            return;
        };
        assert_eq!(validate(Some(&runtime), STARTER_RULES), Validation::Valid);
        let broken = r#"{"version": 1, "rules": [{"id": "x", "tools": ["terminal"], "action": "deny",
                        "match": {"commands": ["(unclosed"]}}]}"#;
        match validate(Some(&runtime), broken) {
            Validation::Invalid { reason } => assert!(reason.contains("invalid regex"), "{reason}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
        match validate(Some(&runtime), "{ nope") {
            Validation::Invalid { reason } => {
                assert!(reason.contains("not valid JSON"), "{reason}")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn non_ascii_rules_survive_the_trip_to_the_validator() {
        let Some(runtime) = installed_runtime() else {
            return;
        };
        let text = r#"{"version": 1, "rules": [{"id": "café", "tools": ["*"], "action": "deny"}]}"#;
        // "café" isn't a valid id ([a-z0-9-]), and the reason must quote it intact.
        match validate(Some(&runtime), text) {
            Validation::Invalid { reason } => assert!(reason.contains("id must match"), "{reason}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }
}
