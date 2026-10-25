//! Tauri commands exposed to the UI (M1 §8).
//!
//! Nothing that crosses this boundary contains the API key.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::audit::{self, AuditEvent};
use crate::hermes::client::{
    ApiKey, ApprovalChoice, Capabilities, CreateRunRequest, CreateRunResponse, DEFAULT_BASE_URL,
    Health, HermesClient, PendingControl, RunStatus,
};
use crate::hermes::errors::HermesError;
use crate::hermes::events::{RunEvent, RunEventEnvelope};
use crate::hermes::local::{self, LocalError};
use crate::keyring::{self, KeyStoreError};
use crate::rules;
use crate::store::{self, Conversation, StoreError, Task, TaskStatus, TaskStore};
use crate::system::speedtest::{self, SpeedTestResult};
use crate::system::{AlertSwitch, SystemMonitor, SystemSnapshot};

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
    /// For `approval.request` only: whether Hermes still lists it as pending.
    pub approval_live: Option<bool>,
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
                    // M1 §9 rule 2: a replayed stream repeats old approval requests, so only one
                    // Hermes still lists as pending may become a live card.
                    let approval_live = match &envelope.event {
                        RunEvent::ApprovalRequest { prompt_id, .. } => {
                            Some(client.run_status(&run_id).await.is_ok_and(|status| {
                                approval_is_live(&status.pending_controls, prompt_id)
                            }))
                        }
                        _ => None,
                    };
                    let status_change = TaskStatus::after(&envelope.event).filter(|status| {
                        *status != TaskStatus::WaitingForApproval || approval_live == Some(true)
                    });
                    if let Some(status) = status_change {
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
                        approval_live,
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

fn approval_is_live(pending: &[PendingControl], prompt_id: &str) -> bool {
    pending
        .iter()
        .any(|control| control.kind == "approval" && control.prompt_id == prompt_id)
}

/// Answers a pending approval (M1 §9 rule 2). Hermes refuses a stale generation with
/// `stale_generation`, which reaches the UI as kind `stale`.
#[tauri::command]
pub async fn answer_approval(
    state: tauri::State<'_, AppState>,
    run_id: String,
    prompt_id: String,
    execution_generation: u32,
    choice: ApprovalChoice,
) -> Result<(), CommandError> {
    let client = state.connected_client()?;
    client
        .answer_approval(&run_id, &prompt_id, execution_generation, choice)
        .await?;
    Ok(())
}

/// Asks Hermes to stop a run (M1 §9 rule 5). The task shows `stopping` until `run.cancelled`.
#[tauri::command]
pub async fn stop_run(
    state: tauri::State<'_, AppState>,
    task_id: String,
    run_id: String,
) -> Result<(), CommandError> {
    let client = state.connected_client()?;
    client.stop_run(&run_id).await?;
    state.with_store(|store| store.set_status(&task_id, TaskStatus::Stopping, None))?;
    Ok(())
}

/// What a task should become, given Hermes's answer about its run after a restart (M1 §9 rule 6).
fn resumed_status(answer: &Result<RunStatus, HermesError>) -> TaskStatus {
    match answer {
        Ok(status) => TaskStatus::from_hermes(&status.status).unwrap_or(TaskStatus::Unknown),
        Err(HermesError::NotFound { .. }) => TaskStatus::Lost,
        // Hermes unreachable or erroring: say nothing new; the task stays as it was.
        Err(_) => TaskStatus::Unknown,
    }
}

/// After a restart, checks every unfinished task with Hermes. Finished runs get their outcome;
/// runs Hermes no longer knows become `lost` (never resubmitted); the rest are returned so the UI
/// can subscribe to them again.
#[tauri::command]
pub async fn resume_open_tasks(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Task>, CommandError> {
    let client = state.connected_client()?;
    let open = state.with_store(|store| store.open_tasks())?;
    let mut still_running = Vec::new();
    for task in open {
        let Some(run_id) = task.run_id.clone() else {
            // Created but never admitted: unknown whether Hermes got it, so don't guess.
            state.with_store(|store| {
                store.set_status(
                    &task.id,
                    TaskStatus::Unknown,
                    Some("Kairos closed before Hermes confirmed this request"),
                )
            })?;
            continue;
        };
        let answer = client.run_status(&run_id).await;
        if let Err(HermesError::Unreachable(_)) = answer {
            still_running.push(task);
            continue;
        }
        let status = resumed_status(&answer);
        let outcome = match (&answer, status) {
            (Ok(run), s) if s.is_terminal() && !run.output.is_empty() => Some(run.output.clone()),
            (_, TaskStatus::Lost) => Some("Hermes no longer knows this run".to_string()),
            _ => None,
        };
        state.with_store(|store| store.set_status(&task.id, status, outcome.as_deref()))?;
        if !status.is_terminal()
            && let Some(updated) = state.with_store(|store| store.get(&task.id))?
        {
            still_running.push(updated);
        }
    }
    Ok(still_running)
}

/// Kairos conversations from `kairos.db`, and Hermes's other conversations (Desktop, CLI,
/// messaging), which Kairos shows read-only because it can't continue them.
#[derive(Debug, Clone, Serialize)]
pub struct ConversationList {
    pub kairos: Vec<Conversation>,
    pub other: Vec<OtherConversation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OtherConversation {
    pub id: String,
    pub title: Option<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub last_active: Option<f64>,
    pub message_count: u64,
}

#[tauri::command]
pub async fn list_conversations(
    state: tauri::State<'_, AppState>,
) -> Result<ConversationList, CommandError> {
    let kairos = state.with_store(|store| store.conversations())?;
    let other = match state.connected_client() {
        Ok(client) => client
            .list_sessions()
            .await
            .map(|list| {
                list.data
                    .into_iter()
                    .filter(|s| !s.id.starts_with("kairos-"))
                    .map(|s| OtherConversation {
                        title: s.title.or(s.preview),
                        id: s.id,
                        source: s.source,
                        model: s.model,
                        last_active: s.last_active,
                        message_count: s.message_count,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    Ok(ConversationList { kairos, other })
}

/// A Kairos conversation's earlier requests and their outcomes, oldest first.
#[tauri::command]
pub async fn conversation_tasks(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Vec<Task>, CommandError> {
    state.with_store(|store| store.for_session(&session_id))
}

/// The Kairos plugin's record of a conversation's tool calls: each call's rule decision, result and
/// approval (M2 Step 6). Empty when the plugin isn't installed.
#[tauri::command]
pub async fn audit_for_session(session_id: String) -> Result<Vec<AuditEvent>, CommandError> {
    let path = audit::default_path()?;
    Ok(audit::events_for_session(&path, &session_id)?)
}

// --- Rules (M2 Step 7) ---------------------------------------------------------------------

/// The rules file as the Rules page shows it.
#[derive(Debug, Clone, Serialize)]
pub struct RulesState {
    pub path: String,
    pub exists: bool,
    /// The file's text, or the starter rules when there is no file yet.
    pub text: String,
    /// The file as the plugin sees it right now. Anything but `valid` means every tool call is
    /// blocked (or, if unavailable, that Kairos can't tell).
    pub validation: rules::Validation,
}

fn rules_runtime() -> Option<rules::PluginRuntime> {
    local::hermes_home()
        .ok()
        .and_then(|home| rules::PluginRuntime::find(&home))
}

async fn validate_off_thread(text: String) -> rules::Validation {
    tauri::async_runtime::spawn_blocking(move || rules::validate(rules_runtime().as_ref(), &text))
        .await
        .unwrap_or_else(|err| rules::Validation::Unavailable {
            reason: format!("validator crashed: {err}"),
        })
}

async fn read_rules_state() -> Result<RulesState, CommandError> {
    let path = rules::default_path()?;
    let (exists, text) = match std::fs::read_to_string(&path) {
        Ok(text) => (true, text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            (false, rules::STARTER_RULES.to_string())
        }
        Err(err) => {
            return Err(CommandError {
                kind: "store",
                message: format!("can't read {}: {err}", path.display()),
            });
        }
    };
    let validation = if exists {
        validate_off_thread(text.clone()).await
    } else {
        rules::Validation::Invalid {
            reason: format!("{} is missing", path.display()),
        }
    };
    Ok(RulesState {
        path: path.display().to_string(),
        exists,
        text,
        validation,
    })
}

#[tauri::command]
pub async fn rules_state() -> Result<RulesState, CommandError> {
    read_rules_state().await
}

/// Checks rules text without saving it, for live feedback while editing.
#[tauri::command]
pub async fn validate_rules(text: String) -> Result<rules::Validation, CommandError> {
    Ok(validate_off_thread(text).await)
}

/// Saves the rules, but only if the plugin's validator accepts them: an invalid file would block
/// every tool call, so the page never writes one.
#[tauri::command]
pub async fn save_rules(text: String) -> Result<RulesState, CommandError> {
    match validate_off_thread(text.clone()).await {
        rules::Validation::Valid => {}
        rules::Validation::Invalid { reason } => {
            return Err(CommandError {
                kind: "invalid_rules",
                message: reason,
            });
        }
        rules::Validation::Unavailable { reason } => {
            return Err(CommandError {
                kind: "unavailable",
                message: format!("Kairos can't check the rules, so it didn't save them: {reason}"),
            });
        }
    }
    rules::write_atomically(&rules::default_path()?, &text)?;
    read_rules_state().await
}

/// Puts the starter rules back.
#[tauri::command]
pub async fn reset_rules() -> Result<RulesState, CommandError> {
    save_rules(rules::STARTER_RULES.to_string()).await
}

// --- System page (M3 Step 2) ---------------------------------------------------------------

/// The latest reading, for the page to draw at once when it opens; `None` until the first sample.
#[tauri::command]
pub async fn system_snapshot(
    monitor: tauri::State<'_, Arc<SystemMonitor>>,
) -> Result<Option<SystemSnapshot>, CommandError> {
    Ok(monitor.latest())
}

/// The page says when it is on screen, so readings come every 2 s instead of every 15 s.
#[tauri::command]
pub async fn set_system_page_visible(
    monitor: tauri::State<'_, Arc<SystemMonitor>>,
    visible: bool,
) -> Result<(), CommandError> {
    monitor.set_visible(visible);
    Ok(())
}

/// Whether alerts are on, from `kairos.db`; on unless the user switched them off (M3 §7). A
/// database problem leaves them on: missing a low-battery warning is worse than an extra one.
pub fn stored_alerts_enabled(state: &AppState) -> bool {
    state
        .with_store(|store| store.setting(AlertSwitch::SETTING))
        .ok()
        .flatten()
        .is_none_or(|value| value != "false")
}

#[tauri::command]
pub async fn alerts_enabled(
    switch: tauri::State<'_, Arc<AlertSwitch>>,
) -> Result<bool, CommandError> {
    Ok(switch.enabled())
}

#[tauri::command]
pub async fn set_alerts_enabled(
    state: tauri::State<'_, AppState>,
    switch: tauri::State<'_, Arc<AlertSwitch>>,
    enabled: bool,
) -> Result<bool, CommandError> {
    state.with_store(|store| {
        store.set_setting(AlertSwitch::SETTING, if enabled { "true" } else { "false" })
    })?;
    switch
        .0
        .store(enabled, std::sync::atomic::Ordering::Relaxed);
    Ok(enabled)
}

static SPEED_TEST_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Measures latency, download and upload against speed.cloudflare.com (M3 §5.2, D-017), sending
/// `speed-test` progress events. One at a time.
#[tauri::command]
pub async fn run_speed_test(app: tauri::AppHandle) -> Result<SpeedTestResult, CommandError> {
    use std::sync::atomic::Ordering;
    if SPEED_TEST_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(CommandError {
            kind: "busy",
            message: "A speed test is already running.".into(),
        });
    }
    let outcome = speedtest::run(|progress| {
        let _ = app.emit("speed-test", progress);
    })
    .await;
    SPEED_TEST_RUNNING.store(false, Ordering::SeqCst);
    outcome.map_err(|message| CommandError {
        kind: "speed_test",
        message,
    })
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

    fn control(prompt_id: &str) -> PendingControl {
        serde_json::from_value(serde_json::json!({
            "kind": "approval", "prompt_id": prompt_id, "execution_generation": 1
        }))
        .expect("control")
    }

    #[test]
    fn only_an_approval_hermes_still_lists_is_live() {
        assert!(approval_is_live(&[control("p1")], "p1"));
        assert!(!approval_is_live(&[control("p2")], "p1"));
        assert!(!approval_is_live(&[], "p1"));
    }

    fn run_status(status: &str) -> Result<RunStatus, HermesError> {
        Ok(
            serde_json::from_value(serde_json::json!({ "run_id": "r", "status": status }))
                .expect("status"),
        )
    }

    #[test]
    fn after_a_restart_each_hermes_answer_maps_to_a_task_status() {
        assert_eq!(
            resumed_status(&run_status("completed")),
            TaskStatus::Completed
        );
        assert_eq!(
            resumed_status(&run_status("cancelled")),
            TaskStatus::Cancelled
        );
        assert_eq!(resumed_status(&run_status("running")), TaskStatus::Running);
        assert_eq!(
            resumed_status(&run_status("waiting_for_approval")),
            TaskStatus::WaitingForApproval
        );
        assert_eq!(resumed_status(&run_status("unknown")), TaskStatus::Unknown);
        assert_eq!(
            resumed_status(&run_status("brand_new")),
            TaskStatus::Unknown
        );
    }

    #[test]
    fn a_run_hermes_no_longer_knows_is_lost_not_resubmitted() {
        let gone = Err(HermesError::NotFound {
            code: "run_not_found".into(),
            message: "Run not found".into(),
        });
        assert_eq!(resumed_status(&gone), TaskStatus::Lost);
    }
}
