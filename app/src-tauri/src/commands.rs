//! Tauri commands exposed to the UI (M1 §8).
//!
//! Nothing that crosses this boundary contains the API key.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::hermes::client::{ApiKey, Capabilities, DEFAULT_BASE_URL, Health, HermesClient};
use crate::hermes::errors::HermesError;
use crate::hermes::local::{self, LocalError};
use crate::keyring::{self, KeyStoreError};

/// Shared between commands. Later steps take the connected client from here.
#[derive(Default)]
pub struct AppState {
    pub client: Mutex<Option<HermesClient>>,
}

/// An error the UI shows as-is (M1 §8).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandError {
    pub kind: &'static str,
    pub message: String,
}

impl From<HermesError> for CommandError {
    fn from(err: HermesError) -> Self {
        Self {
            kind: err.kind(),
            message: err.to_string(),
        }
    }
}

impl From<KeyStoreError> for CommandError {
    fn from(err: KeyStoreError) -> Self {
        Self {
            kind: "keyring",
            message: err.to_string(),
        }
    }
}

impl From<LocalError> for CommandError {
    fn from(err: LocalError) -> Self {
        Self {
            kind: "local",
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectStatus {
    Connected,
    NotRunning,
    KeyRejected,
    CapabilityMissing,
    /// There's no key in the keyring and none in Hermes's `.env`.
    NoKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    Keyring,
    /// Read from `.env` on this call and saved to the keyring.
    Env,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConnectInfo {
    pub status: ConnectStatus,
    pub base_url: String,
    pub hermes_home: Option<String>,
    pub version: Option<String>,
    pub missing_capabilities: Vec<String>,
    /// From the Hermes CLI; `None` if it couldn't be read.
    pub approval_mode: Option<String>,
    pub key_source: Option<KeySource>,
    /// Why the status isn't `connected`, in words for the user.
    pub detail: Option<String>,
}

/// What the screen shows, from the health and capability answers. Kept free of I/O so it can be
/// tested against recorded responses.
pub fn classify(
    health: &Result<Health, HermesError>,
    capabilities: Option<&Result<Capabilities, HermesError>>,
) -> (ConnectStatus, Vec<String>, Option<String>) {
    match health {
        Ok(health) if health.status == "ok" => {}
        Ok(health) => {
            return (
                ConnectStatus::NotRunning,
                Vec::new(),
                Some(format!("Hermes reports status \"{}\"", health.status)),
            );
        }
        Err(err) => return (ConnectStatus::NotRunning, Vec::new(), Some(err.to_string())),
    }
    match capabilities {
        None => (ConnectStatus::NoKey, Vec::new(), None),
        Some(Err(HermesError::Unauthorized)) => (
            ConnectStatus::KeyRejected,
            Vec::new(),
            Some(HermesError::Unauthorized.to_string()),
        ),
        Some(Err(err)) => (ConnectStatus::NotRunning, Vec::new(), Some(err.to_string())),
        Some(Ok(caps)) => {
            let missing = caps.missing_required();
            if missing.is_empty() {
                (ConnectStatus::Connected, missing, None)
            } else {
                let detail = HermesError::CapabilityMissing(missing.clone()).to_string();
                (ConnectStatus::CapabilityMissing, missing, Some(detail))
            }
        }
    }
}

/// The stored key; on first launch, the key from `.env`, saved to the keyring.
fn resolve_key(home: Option<&PathBuf>) -> Result<Option<(ApiKey, KeySource)>, CommandError> {
    if let Some(key) = keyring::load()? {
        return Ok(Some((key, KeySource::Keyring)));
    }
    reload_key_from_env(home)
}

fn reload_key_from_env(
    home: Option<&PathBuf>,
) -> Result<Option<(ApiKey, KeySource)>, CommandError> {
    let Some(home) = home else {
        return Ok(None);
    };
    match local::read_api_key(home) {
        Ok(key) => {
            keyring::save(&key)?;
            Ok(Some((key, KeySource::Env)))
        }
        Err(LocalError::NoKeyInEnv { .. } | LocalError::ReadEnv { .. }) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

async fn check(
    state: &AppState,
    home: Option<PathBuf>,
    key: Option<(ApiKey, KeySource)>,
) -> Result<ConnectInfo, CommandError> {
    let (key, key_source) = match key {
        Some((key, source)) => (Some(key), Some(source)),
        None => (None, None),
    };
    // /health needs no key, so even without one it can say whether Hermes is running.
    let probe = HermesClient::new(DEFAULT_BASE_URL, key.clone().unwrap_or(ApiKey::new("")))?;
    let health = probe.health().await;

    let capabilities = match (&health, &key) {
        (Ok(_), Some(_)) => Some(probe.capabilities().await),
        _ => None,
    };
    let (status, missing_capabilities, detail) = classify(&health, capabilities.as_ref());
    let detail = match (status, &home) {
        (ConnectStatus::NoKey, Some(home)) => Some(format!(
            "No API key in the keyring or in {}. Run scripts/setup.ps1.",
            home.join(".env").display()
        )),
        _ => detail,
    };

    let approval_mode = match &home {
        Some(home) if status != ConnectStatus::NotRunning => {
            let home = home.clone();
            tauri::async_runtime::spawn_blocking(move || local::approval_mode(&home))
                .await
                .ok()
                .flatten()
        }
        _ => None,
    };

    *state.client.lock().map_err(|_| CommandError {
        kind: "internal",
        message: "app state lock poisoned".into(),
    })? = (status == ConnectStatus::Connected).then_some(probe);

    Ok(ConnectInfo {
        status,
        base_url: DEFAULT_BASE_URL.to_string(),
        hermes_home: home.map(|h| h.display().to_string()),
        version: health.ok().and_then(|h| h.version),
        missing_capabilities,
        approval_mode,
        key_source,
        detail,
    })
}

/// Checks Hermes with the stored key (reading it from `.env` on first launch).
#[tauri::command]
pub async fn connect(state: tauri::State<'_, AppState>) -> Result<ConnectInfo, CommandError> {
    let home = local::hermes_home().ok();
    let key = resolve_key(home.as_ref())?;
    check(&state, home, key).await
}

/// Re-reads the key from Hermes's `.env`, replaces the stored one, and checks again. For "key
/// rejected" after the key in `.env` changed.
#[tauri::command]
pub async fn reload_key(state: tauri::State<'_, AppState>) -> Result<ConnectInfo, CommandError> {
    let home = local::hermes_home().ok();
    let key = reload_key_from_env(home.as_ref())?;
    check(&state, home, key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Result<Health, HermesError> {
        Ok(Health {
            status: "ok".into(),
            version: Some("0.22.5".into()),
            platform: None,
        })
    }

    fn caps(json: serde_json::Value) -> Result<Capabilities, HermesError> {
        Ok(serde_json::from_value(json).expect("capabilities json"))
    }

    fn all_required() -> serde_json::Value {
        let features: serde_json::Map<String, serde_json::Value> =
            crate::hermes::client::REQUIRED_CAPABILITIES
                .iter()
                .map(|name| (name.to_string(), serde_json::Value::Bool(true)))
                .collect();
        serde_json::json!({ "features": features })
    }

    #[test]
    fn hermes_not_answering_is_not_running() {
        let health = Err(HermesError::Unreachable("connection refused".into()));
        let (status, _, detail) = classify(&health, None);
        assert_eq!(status, ConnectStatus::NotRunning);
        assert!(detail.expect("detail").contains("connection refused"));
    }

    #[test]
    fn a_rejected_key_is_key_rejected() {
        let (status, _, _) = classify(&healthy(), Some(&Err(HermesError::Unauthorized)));
        assert_eq!(status, ConnectStatus::KeyRejected);
    }

    #[test]
    fn running_hermes_without_a_key_is_no_key() {
        let (status, _, _) = classify(&healthy(), None);
        assert_eq!(status, ConnectStatus::NoKey);
    }

    #[test]
    fn all_required_capabilities_is_connected() {
        let (status, missing, detail) = classify(&healthy(), Some(&caps(all_required())));
        assert_eq!(status, ConnectStatus::Connected);
        assert!(missing.is_empty());
        assert_eq!(detail, None);
    }

    #[test]
    fn a_missing_capability_is_named() {
        let mut json = all_required();
        json["features"]["run_stop"] = serde_json::Value::Bool(false);
        let (status, missing, _) = classify(&healthy(), Some(&caps(json)));
        assert_eq!(status, ConnectStatus::CapabilityMissing);
        assert_eq!(missing, ["run_stop"]);
    }

    #[test]
    fn connect_info_serialises_without_any_key_field() {
        let info = ConnectInfo {
            status: ConnectStatus::Connected,
            base_url: DEFAULT_BASE_URL.into(),
            hermes_home: None,
            version: Some("0.22.5".into()),
            missing_capabilities: vec![],
            approval_mode: Some("manual".into()),
            key_source: Some(KeySource::Keyring),
            detail: None,
        };
        let json = serde_json::to_value(&info).expect("serialises");
        assert_eq!(json["status"], "connected");
        assert_eq!(json["key_source"], "keyring");
        let fields: Vec<&String> = json.as_object().expect("object").keys().collect();
        assert!(
            fields
                .iter()
                .all(|f| !f.contains("key") || *f == "key_source")
        );
    }
}
