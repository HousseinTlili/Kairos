//! Facts about the local Hermes install that the API doesn't expose (M1 §2): where its home
//! is, the API key in its `.env`, and its approval mode (`admin_config_rw` is false, so the
//! mode has to come from the Hermes CLI).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::client::ApiKey;

const CLI_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LocalError {
    #[error("can't find the Hermes home (set HERMES_HOME, or install Hermes)")]
    NoHome,

    #[error("can't read {path}: {message}")]
    ReadEnv { path: String, message: String },

    #[error("{path} has no API_SERVER_KEY; run scripts/setup.ps1")]
    NoKeyInEnv { path: String },
}

/// `HERMES_HOME` if set, else `%LOCALAPPDATA%\hermes`, as Hermes itself resolves it.
pub fn hermes_home() -> Result<PathBuf, LocalError> {
    if let Some(home) = std::env::var_os("HERMES_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(|base| PathBuf::from(base).join("hermes"))
        .ok_or(LocalError::NoHome)
}

/// Reads `API_SERVER_KEY` from `<home>\.env`. Done once at first launch, and when the user asks
/// to re-read it; otherwise the key comes from the keyring.
pub fn read_api_key(home: &Path) -> Result<ApiKey, LocalError> {
    let path = home.join(".env");
    let display = path.display().to_string();
    let text = std::fs::read_to_string(&path).map_err(|err| LocalError::ReadEnv {
        path: display.clone(),
        message: err.to_string(),
    })?;
    env_value(&text, "API_SERVER_KEY")
        .filter(|key| !key.is_empty())
        .map(ApiKey::new)
        .ok_or(LocalError::NoKeyInEnv { path: display })
}

/// Value of `name` in dotenv text, following the same rules as `scripts/setup.ps1`: optional
/// `export `, optional matching quotes, and the last occurrence wins.
pub fn env_value(text: &str, name: &str) -> Option<String> {
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        let line = line
            .strip_prefix("export ")
            .map(str::trim_start)
            .unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != name {
            continue;
        }
        let value = value.trim();
        let unquoted = ['"', '\'']
            .iter()
            .find_map(|q| {
                value
                    .strip_prefix(*q)
                    .and_then(|rest| rest.strip_suffix(*q))
            })
            .unwrap_or(value);
        found = Some(unquoted.to_string());
    }
    found
}

/// The Hermes CLI: `<home>\bin\hermes.exe`, else `hermes` on PATH.
fn hermes_cli(home: &Path) -> PathBuf {
    let bundled = home.join("bin").join("hermes.exe");
    if bundled.is_file() {
        bundled
    } else {
        PathBuf::from("hermes")
    }
}

/// `hermes config get approvals.mode`, or `None` if the CLI can't answer. Blocking (the CLI takes
/// about a second); call it off the async runtime.
pub fn approval_mode(home: &Path) -> Option<String> {
    let mut command = Command::new(hermes_cli(home));
    command
        .args(["config", "get", "approvals.mode"])
        .env("HERMES_HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: don't flash a console window over the app.
        command.creation_flags(0x0800_0000);
    }

    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < CLI_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_mode(&String::from_utf8_lossy(&output.stdout))
}

/// The CLI prints the value on its last line; anything that isn't a plain mode name is ignored.
fn parse_mode(stdout: &str) -> Option<String> {
    let last = stdout.lines().map(str::trim).rfind(|l| !l.is_empty())?;
    last.chars()
        .all(|c| c.is_ascii_lowercase() || c == '_')
        .then(|| last.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_assignment_wins_and_quotes_and_export_are_stripped() {
        let text = "API_SERVER_KEY=old\n# comment\nexport API_SERVER_KEY=\"new\"\nOTHER=1\n";
        assert_eq!(env_value(text, "API_SERVER_KEY").as_deref(), Some("new"));
        assert_eq!(
            env_value("  API_SERVER_KEY = 'abc' \r\n", "API_SERVER_KEY").as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn a_similar_name_is_not_a_match() {
        let text = "API_SERVER_KEY_OLD=x\nMY_API_SERVER_KEY=y\n";
        assert_eq!(env_value(text, "API_SERVER_KEY"), None);
    }

    #[test]
    fn a_missing_or_empty_key_is_reported_with_the_file_path() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("kairos-local-test-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join(".env"), "API_SERVER_KEY=\nOTHER=1\n").expect("write");
        let result = read_api_key(&dir);
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(matches!(result, Err(LocalError::NoKeyInEnv { .. })));
    }

    #[test]
    fn the_mode_is_the_last_non_empty_line_of_cli_output() {
        assert_eq!(
            parse_mode("Loading config...\nmanual\n\n").as_deref(),
            Some("manual")
        );
        assert_eq!(parse_mode("smart").as_deref(), Some("smart"));
        assert_eq!(parse_mode("Error: no such key"), None);
        assert_eq!(parse_mode(""), None);
    }
}
