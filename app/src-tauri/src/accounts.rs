//! Settings ▸ Accounts (M6 §3, D-010). Kairos builds no sign-in of its own: Google goes through
//! Hermes's bundled `google-workspace` skill (Google's OAuth, the product owner's own client, the
//! token kept by Hermes), GitHub through `gh auth login`. Kairos only shows the state, starts the
//! sign-in, and disconnects. It never opens a token: for Google it checks the token file exists,
//! for GitHub it reads `gh auth status`'s exit code and throws its output away.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub const GITHUB_HOST: &str = "github.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Google,
    Github,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AccountStatus {
    Connected,
    NotConnected,
    /// The sign-in route isn't installed on this PC; `reason` says what is missing.
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountState {
    pub kind: AccountKind,
    #[serde(flatten)]
    pub status: AccountStatus,
}

/// Where Hermes's Google skill keeps things [source: google-workspace/SKILL.md].
pub fn google_paths(hermes_home: &Path) -> (PathBuf, PathBuf) {
    (
        hermes_home
            .join("skills")
            .join("productivity")
            .join("google-workspace")
            .join("scripts")
            .join("setup.py"),
        hermes_home.join("google_token.json"),
    )
}

pub fn google_status(hermes_home: &Path) -> AccountStatus {
    let (setup, token) = google_paths(hermes_home);
    if !setup.is_file() {
        return AccountStatus::Unavailable {
            reason: "Hermes's google-workspace skill isn't installed.".into(),
        };
    }
    if token.is_file() {
        AccountStatus::Connected
    } else {
        AccountStatus::NotConnected
    }
}

/// `(program, args)` for each GitHub action; pure, so the exact commands are tested.
pub fn github_status_command() -> (&'static str, Vec<&'static str>) {
    ("gh", vec!["auth", "status", "--hostname", GITHUB_HOST])
}

pub fn github_logout_command() -> (&'static str, Vec<&'static str>) {
    ("gh", vec!["auth", "logout", "--hostname", GITHUB_HOST])
}

/// The sign-in needs a terminal the user can see (a one-time code and a browser), so it opens a
/// new PowerShell window instead of running hidden.
pub fn github_login_window_command() -> (&'static str, Vec<String>) {
    (
        "cmd",
        vec![
            "/c".into(),
            "start".into(),
            "Kairos - GitHub sign-in".into(),
            "powershell".into(),
            "-NoExit".into(),
            "-Command".into(),
            format!("gh auth login --hostname {GITHUB_HOST} --web --git-protocol https"),
        ],
    )
}

pub fn google_revoke_command(python: &Path, setup: &Path) -> (PathBuf, Vec<String>) {
    (
        python.to_path_buf(),
        vec![setup.display().to_string(), "--revoke".into()],
    )
}

#[derive(Debug, PartialEq, Eq)]
enum Ran {
    Succeeded,
    Failed,
    Missing,
    TimedOut,
}

/// Runs a command hidden, with stdin closed and its output discarded (it may carry a masked
/// token), and reports only how it ended.
fn run_quietly(program: &Path, args: &[String], env: &[(&str, &Path)]) -> Ran {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ran::Missing,
        Err(_) => return Ran::Failed,
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ran::Succeeded
                } else {
                    Ran::Failed
                };
            }
            Ok(None) if started.elapsed() < COMMAND_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                return Ran::TimedOut;
            }
        }
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

pub fn github_status() -> AccountStatus {
    let (program, args) = github_status_command();
    match run_quietly(Path::new(program), &strings(&args), &[]) {
        Ran::Succeeded => AccountStatus::Connected,
        Ran::Missing => AccountStatus::Unavailable {
            reason: "The GitHub CLI (gh) isn't installed.".into(),
        },
        Ran::TimedOut => AccountStatus::Unavailable {
            reason: "The GitHub CLI didn't answer.".into(),
        },
        Ran::Failed => AccountStatus::NotConnected,
    }
}

pub fn states(hermes_home: Option<&Path>) -> Vec<AccountState> {
    vec![
        AccountState {
            kind: AccountKind::Google,
            status: hermes_home.map_or(
                AccountStatus::Unavailable {
                    reason: "Kairos can't find Hermes's folder.".into(),
                },
                google_status,
            ),
        },
        AccountState {
            kind: AccountKind::Github,
            status: github_status(),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct AccountError(pub String);

pub fn github_connect() -> Result<(), AccountError> {
    if !cfg!(windows) {
        return Err(AccountError(
            "The GitHub sign-in window opens on Windows only.".into(),
        ));
    }
    let (program, args) = github_login_window_command();
    Command::new(program)
        .args(&args)
        .stdin(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|err| AccountError(format!("Couldn't open the sign-in window: {err}")))
}

pub fn github_disconnect() -> Result<(), AccountError> {
    let (program, args) = github_logout_command();
    match run_quietly(Path::new(program), &strings(&args), &[]) {
        Ran::Missing => Err(AccountError("The GitHub CLI (gh) isn't installed.".into())),
        Ran::TimedOut => Err(AccountError("The GitHub CLI didn't answer.".into())),
        // "Not logged in" also fails; what counts is the state afterwards.
        _ => match github_status() {
            AccountStatus::Connected => Err(AccountError(
                "GitHub is still connected. Run `gh auth logout` in a terminal.".into(),
            )),
            _ => Ok(()),
        },
    }
}

pub fn google_disconnect(hermes_home: &Path, python: Option<PathBuf>) -> Result<(), AccountError> {
    let (setup, token) = google_paths(hermes_home);
    let python = python.ok_or_else(|| AccountError("Hermes's Python wasn't found.".into()))?;
    if !setup.is_file() {
        return Err(AccountError(
            "Hermes's google-workspace skill isn't installed.".into(),
        ));
    }
    let (program, args) = google_revoke_command(&python, &setup);
    match run_quietly(&program, &args, &[("HERMES_HOME", hermes_home)]) {
        Ran::TimedOut => Err(AccountError("Google didn't answer in time.".into())),
        _ if token.exists() => Err(AccountError(
            "The Google token is still there; the disconnect didn't finish.".into(),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_commands_name_the_host_and_never_ask_for_the_token() {
        let (program, status) = github_status_command();
        assert_eq!(program, "gh");
        assert_eq!(status, ["auth", "status", "--hostname", "github.com"]);
        assert_eq!(
            github_logout_command().1,
            ["auth", "logout", "--hostname", "github.com"]
        );
        let (_, login) = github_login_window_command();
        assert!(
            login
                .last()
                .expect("command")
                .contains("gh auth login --hostname github.com --web")
        );
        for args in [status, github_logout_command().1] {
            assert!(!args.contains(&"token"));
        }
    }

    #[test]
    fn the_google_disconnect_runs_the_skills_own_revoke() {
        let (program, args) = google_revoke_command(Path::new("py.exe"), Path::new("setup.py"));
        assert_eq!(program, PathBuf::from("py.exe"));
        assert_eq!(args, ["setup.py", "--revoke"]);
    }

    fn temp_home(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("kairos-accounts-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn google_is_unavailable_without_the_skill_and_connected_only_with_a_token() {
        let home = temp_home("google");
        assert!(matches!(
            google_status(&home),
            AccountStatus::Unavailable { .. }
        ));
        let (setup, token) = google_paths(&home);
        std::fs::create_dir_all(setup.parent().expect("dir")).expect("skill dir");
        std::fs::write(&setup, "").expect("setup");
        assert_eq!(google_status(&home), AccountStatus::NotConnected);
        std::fs::write(&token, "{}").expect("token");
        assert_eq!(google_status(&home), AccountStatus::Connected);
        std::fs::remove_dir_all(&home).expect("cleanup");
    }

    #[test]
    fn a_missing_program_is_reported_as_missing() {
        assert_eq!(
            run_quietly(Path::new("kairos-no-such-program"), &[], &[]),
            Ran::Missing
        );
    }

    #[test]
    fn the_state_serialises_flat_for_the_page() {
        let json = serde_json::to_value(AccountState {
            kind: AccountKind::Github,
            status: AccountStatus::Unavailable { reason: "x".into() },
        })
        .expect("json");
        assert_eq!(json["kind"], "github");
        assert_eq!(json["status"], "unavailable");
        assert_eq!(json["reason"], "x");
    }
}
