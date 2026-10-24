//! Tauri commands exposed to the UI (M1 §8).
//!
//! Nothing that crosses this boundary contains the API key.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::hermes::client::{
    ApiKey, Capabilities, CreateRunRequest, CreateRunResponse, DEFAULT_BASE_URL, Health,
    HermesClient,
};
use crate::hermes::errors::HermesError;
use crate::hermes::events::{RunEvent, RunEventEnvelope};
use crate::hermes::local::{self, LocalError};
use crate::keyring::{self, KeyStoreError};
use crate::store::{self, StoreError, TaskStatus, TaskStore};

/// Shared between commands.
#[derive(Default)]
pub struct AppState {
    /// Set by `connect` when Hermes answers with every required capability.
    pub client: Mutex<Option<HermesClient>>,
    /// Opened on first use, so a database problem shows as an error instead of a crash at startup.
    pub store: Mutex<Option<TaskStore>>,
}

impl AppState {
    fn connected_client(&self) -> Result<HermesClient, CommandError> {
        self.client
            .lock()
            .map_err(|_| poisoned())?
            .clone()
            .ok_or_else(|| CommandError {
                kind: "not_connected",
                message: "Kairos isn't connected to Hermes yet".into(),
            })
    }

    fn with_store<T>(
        &self,
        action: impl FnOnce(&TaskStore) -> Result<T, StoreError>,
    ) -> Result<T, CommandError> {
        let mut guard = self.store.lock().map_err(|_| poisoned())?;
        if guard.is_none() {
            *guard = Some(TaskStore::open(&store::default_path()?)?);
        }
        let store = guard.as_ref().ok_or_else(poisoned)?;
        Ok(action(store)?)
    }
}

fn poisoned() -> CommandError {
    CommandError {
        kind: "internal",
        message: "app state lock poisoned".into(),
    }
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

impl From<StoreError> for CommandError {
    fn from(err: StoreError) -> Self {
        Self {
            kind: "store",
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

    *state.client.lock().map_err(|_| poisoned())? =
        (status == ConnectStatus::Connected).then_some(probe);

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

// --- Runs ---------------------------------------------------------------------------------------

const CREATE_ATTEMPTS: u32 = 3;
const CREATE_RETRY_DELAY: Duration = Duration::from_secs(1);

/// What `create_run` returns: the Kairos task and the Hermes run behind it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskRef {
    pub task_id: String,
    pub session_id: String,
    pub run_id: String,
    /// Hermes recognised the task ID and returned the run it already had.
    pub replayed: bool,
}

/// One run event, tagged with the Kairos task it belongs to. Sent as the `run-event` Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct RunEventPayload {
    pub task_id: String,
    pub envelope: RunEventEnvelope,
}

/// Sent as `run-stream-end` when a run's event stream closes, with the error if it broke.
#[derive(Debug, Clone, Serialize)]
pub struct RunStreamEnd {
    pub task_id: String,
    pub error: Option<CommandError>,
}

fn new_id() -> String {
    ulid::Ulid::generate().to_string().to_lowercase()
}

/// Kairos only continues sessions it created; Hermes refuses sessions owned by Desktop, CLI or
/// TUI (M1 §5).
fn session_for(requested: Option<String>) -> Result<String, CommandError> {
    match requested {
        None => Ok(format!("kairos-{}", new_id())),
        Some(id) if id.starts_with("kairos-") && id.len() > "kairos-".len() => Ok(id),
        Some(id) => Err(CommandError {
            kind: "invalid",
            message: format!(
                "{id} isn't a Kairos session; only kairos-* sessions can be continued"
            ),
        }),
    }
}

/// Network errors are retried with the same Idempotency-Key, so Hermes runs the request at most
/// once however many attempts reach it (M1 §9 rule 1). Hermes's own refusals aren't retried.
async fn create_with_retry(
    client: &HermesClient,
    input: &str,
    session_id: &str,
    idempotency_key: &str,
) -> Result<CreateRunResponse, HermesError> {
    let request = CreateRunRequest { input, session_id };
    let mut attempt = 1;
    loop {
        match client.create_run(&request, idempotency_key).await {
            Err(HermesError::Unreachable(_)) if attempt < CREATE_ATTEMPTS => {
                attempt += 1;
                let _ =
                    tauri::async_runtime::spawn_blocking(|| std::thread::sleep(CREATE_RETRY_DELAY))
                        .await;
            }
            result => return result,
        }
    }
}

/// Starts a run. The task row exists before Hermes is called and its ID is the Idempotency-Key.
#[tauri::command]
pub async fn create_run(
    state: tauri::State<'_, AppState>,
    session_id: Option<String>,
    input: String,
) -> Result<TaskRef, CommandError> {
    let input = input.trim().to_string();
    if input.is_empty() {
        return Err(CommandError {
            kind: "invalid",
            message: "the request is empty".into(),
        });
    }
    let client = state.connected_client()?;
    let session_id = session_for(session_id)?;
    let task_id = new_id();
    state.with_store(|store| store.create(&task_id, &session_id, &input))?;

    match create_with_retry(&client, &input, &session_id, &task_id).await {
        Ok(created) => {
            state.with_store(|store| store.set_run(&task_id, &created.run_id))?;
            Ok(TaskRef {
                task_id,
                session_id,
                run_id: created.run_id,
                replayed: created.replayed,
            })
        }
        Err(err) => {
            // A network failure may have reached Hermes, so the outcome is unknown, not failed;
            // either way the task is never resubmitted under a new key.
            let status = match err {
                HermesError::Unreachable(_) => TaskStatus::Unknown,
                _ => TaskStatus::Failed,
            };
            let message = err.to_string();
            state.with_store(|store| store.set_status(&task_id, status, Some(&message)))?;
            Err(err.into())
        }
    }
}

/// The outcome to store when an event ends a task.
fn outcome_of(event: &RunEvent) -> Option<String> {
    match event {
        RunEvent::RunCompleted { output, .. } => Some(output.clone()),
        RunEvent::RunCancelled { turn_exit_reason } => turn_exit_reason.clone(),
        RunEvent::RunFailed { error } | RunEvent::RunInterrupted { error } => {
            error.as_ref().map(|e| e.to_string())
        }
        _ => None,
    }
}

/// Streams a run's events to the UI as `run-event`, then `run-stream-end`, and keeps the task row
/// in step. The stream replays from the start (M1 §6), so subscribing again is safe.
#[tauri::command]
pub async fn subscribe_run(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    task_id: String,
    run_id: String,
) -> Result<(), CommandError> {
    let task = state
        .with_store(|store| store.get(&task_id))?
        .filter(|task| task.run_id.as_deref() == Some(run_id.as_str()))
        .ok_or_else(|| CommandError {
            kind: "not_found",
            message: format!("no task {task_id} for run {run_id}"),
        })?;
    let client = state.connected_client()?;
    let mut stream = client.run_events(&run_id).await?;

    tauri::async_runtime::spawn(async move {
        let mut error = None;
        while let Some(item) = stream.next().await {
            match item {
                Ok(envelope) => {
                    if let RunEvent::Unknown { name, .. } = &envelope.event {
                        eprintln!("[kairos] unrecognised Hermes event {name:?} in run {run_id}");
                    }
                    if let Some(status) = TaskStatus::after(&envelope.event) {
                        let outcome = outcome_of(&envelope.event);
                        let state = app.state::<AppState>();
                        if let Err(err) = state.with_store(|store| {
                            store.set_status(&task.id, status, outcome.as_deref())
                        }) {
                            eprintln!("[kairos] couldn't update task {}: {}", task.id, err.message);
                        }
                    }
                    let payload = RunEventPayload {
                        task_id: task.id.clone(),
                        envelope,
                    };
                    let _ = app.emit("run-event", payload);
                }
                Err(err) => error = Some(CommandError::from(err)),
            }
        }
        let _ = app.emit(
            "run-stream-end",
            RunStreamEnd {
                task_id: task.id,
                error,
            },
        );
    });
    Ok(())
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

    #[test]
    fn a_new_conversation_gets_a_fresh_kairos_session() {
        let first = session_for(None).expect("new session");
        let second = session_for(None).expect("new session");
        assert!(first.starts_with("kairos-"));
        assert_ne!(first, second);
    }

    #[test]
    fn only_kairos_sessions_can_be_continued() {
        assert_eq!(
            session_for(Some("kairos-abc".into())).expect("allowed"),
            "kairos-abc"
        );
        for foreign in ["local-f2e15f3e", "kairos-", "agent:main:api_server"] {
            let err = session_for(Some(foreign.into())).expect_err(foreign);
            assert_eq!(err.kind, "invalid");
        }
    }

    #[test]
    fn a_finished_run_stores_its_answer_or_reason() {
        let completed = RunEvent::RunCompleted {
            output: "Kairos OK".into(),
            usage: Default::default(),
            completed: true,
            partial: false,
            interrupted: false,
        };
        assert_eq!(outcome_of(&completed).as_deref(), Some("Kairos OK"));
        let cancelled = RunEvent::RunCancelled {
            turn_exit_reason: Some("interrupted_by_user".into()),
        };
        assert_eq!(
            outcome_of(&cancelled).as_deref(),
            Some("interrupted_by_user")
        );
        assert_eq!(
            outcome_of(&RunEvent::MessageDelta { delta: "x".into() }),
            None
        );
    }
}
