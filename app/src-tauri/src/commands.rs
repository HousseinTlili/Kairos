//! Tauri commands exposed to the UI (M1 §8).
//!
//! Nothing that crosses this boundary contains the API key.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::accounts::{self, AccountKind, AccountState};
use crate::audit::{self, AuditEvent};
use crate::briefing::facts::{self as briefing_facts, Briefing};
use crate::briefing::rules::{self as briefing_rules, BriefingSettings, Reason};
use crate::briefing::{self, BriefingClock, BriefingNotice};
use crate::hermes::client::{
    ApiKey, ApprovalChoice, Capabilities, CreateRunRequest, CreateRunResponse, DEFAULT_BASE_URL,
    Health, HermesClient, JobAction, PendingControl, RunStatus,
};
use crate::hermes::errors::HermesError;
use crate::hermes::events::{RunEvent, RunEventEnvelope};
use crate::hermes::jobs::{Job, NewJob};
use crate::hermes::local::{self, LocalError};
use crate::keyring::{self, KeyStoreError};
use crate::models::{self, Catalog};
use crate::notifications::collect::{self as collect, Access, CollectState};
use crate::notifications::store::{AppCount, StoredNotification};
use crate::notifications::{self, ENABLED_SETTING, NotificationSwitch, SINCE_SETTING};
use crate::presence::{self, PresenceEvent};
use crate::rules;
use crate::store::{self, Conversation, ModelChoice, StoreError, Task, TaskStatus, TaskStore};
use crate::system::speedtest::{self, SpeedTestResult};
use crate::system::{AlertSwitch, SystemMonitor, SystemSnapshot};

/// Shared between commands.
#[derive(Default)]
pub struct AppState {
    /// Set by `connect` when Hermes answers with every required capability.
    pub client: Mutex<Option<HermesClient>>,
    /// Opened on first use, so a database problem shows as an error instead of a crash at startup.
    pub store: Mutex<Option<TaskStore>>,
    /// The latest model catalogue (M7), kept for pricing finished runs.
    pub catalog: Mutex<Option<Catalog>>,
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
    choice: &ModelChoice,
    idempotency_key: &str,
) -> Result<CreateRunResponse, HermesError> {
    let request = CreateRunRequest {
        input,
        session_id,
        model: choice.model.as_deref(),
        provider: choice.provider.as_deref(),
    };
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: Option<String>,
    input: String,
    choice: Option<ModelChoice>,
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
    // The page sends the conversation's model with every request (M7 §4); kept for reopening.
    let choice = choice.unwrap_or_default();
    state.with_store(|store| {
        store.set_conversation_model(&session_id, &choice)?;
        store.create(&task_id, &session_id, &input)
    })?;

    match create_with_retry(&client, &input, &session_id, &choice, &task_id).await {
        Ok(created) => {
            state.with_store(|store| store.set_run(&task_id, &created.run_id))?;
            if let Some(cap) = stored_cost_cap(&state) {
                watch_cost(
                    app.clone(),
                    client.clone(),
                    task_id.clone(),
                    created.run_id.clone(),
                    choice.clone(),
                    cap,
                );
            }
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
                    if let RunEvent::RunCompleted { usage, runtime, .. } = &envelope.event {
                        record_run_usage(&app, &task.id, usage, runtime.as_ref());
                    }
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

/// A request's usage as the chat shows it (M7 §5), sent as `task-usage` once it is stored.
#[derive(Debug, Clone, Serialize)]
pub struct TaskUsage {
    pub task_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    /// Estimate; `None` = price unknown.
    pub cost_usd: Option<f64>,
    pub served_provider: Option<String>,
    pub served_model: Option<String>,
}

/// The estimated cost of `usage` on the served model, from the cached catalogue (D-020); `None`
/// when there is no price, never zero.
fn estimate(
    state: &AppState,
    usage: &crate::hermes::events::Usage,
    served: Option<&crate::hermes::events::Runtime>,
) -> Option<f64> {
    let served = served?;
    let catalog = state.catalog.lock().ok()?;
    let price = catalog.as_ref()?.price(&served.provider, &served.model)?;
    Some(models::estimate_cost(usage, &price))
}

/// Stores a finished run's tokens, the served model and the estimated cost, and tells the chat.
fn record_run_usage(
    app: &tauri::AppHandle,
    task_id: &str,
    usage: &crate::hermes::events::Usage,
    served: Option<&crate::hermes::events::Runtime>,
) {
    let state = app.state::<AppState>();
    let cost = estimate(&state, usage, served);
    if let Err(err) = state.with_store(|store| store.record_usage(task_id, usage, cost, served)) {
        eprintln!(
            "[kairos] couldn't store the usage of {task_id}: {}",
            err.message
        );
    }
    let named = |s: Option<&str>| s.filter(|s| !s.is_empty()).map(str::to_string);
    let _ = app.emit(
        "task-usage",
        TaskUsage {
            task_id: task_id.to_string(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cost_usd: cost,
            served_provider: named(served.map(|r| r.provider.as_str())),
            served_model: named(served.map(|r| r.model.as_str())),
        },
    );
}

const COST_CAP: &str = "cost_cap_usd";
const CAP_POLL: Duration = Duration::from_secs(10);

/// The per-request cost cap in dollars (M7 §5), `None` when off.
fn stored_cost_cap(state: &AppState) -> Option<f64> {
    state
        .with_store(|store| store.setting(COST_CAP))
        .ok()
        .flatten()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

#[tauri::command]
pub async fn cost_cap(state: tauri::State<'_, AppState>) -> Result<Option<f64>, CommandError> {
    Ok(stored_cost_cap(&state))
}

/// Sets or clears (`None`, zero) the per-request cap.
#[tauri::command]
pub async fn set_cost_cap(
    state: tauri::State<'_, AppState>,
    cap: Option<f64>,
) -> Result<Option<f64>, CommandError> {
    let cap = cap.filter(|v| v.is_finite() && *v > 0.0);
    state.with_store(|store| {
        store.set_setting(COST_CAP, &cap.map(|v| v.to_string()).unwrap_or_default())
    })?;
    Ok(cap)
}

/// Sent as `cost-cap` when Kairos stops a request for going over the cap.
#[derive(Debug, Clone, Serialize)]
pub struct CostCapHit {
    pub task_id: String,
    pub cap: f64,
    pub cost: f64,
}

/// Watches a capped run (D-020): every 10 s, its usage so far priced on the served model (or the
/// conversation's chosen one while Hermes hasn't named it); over the cap, Kairos stops it. If
/// Hermes reports usage only at the end, this never fires mid-run; testing records which.
fn watch_cost(
    app: tauri::AppHandle,
    client: HermesClient,
    task_id: String,
    run_id: String,
    chosen: ModelChoice,
    cap: f64,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(CAP_POLL)).await;
            let Ok(status) = client.run_status(&run_id).await else {
                continue;
            };
            if TaskStatus::from_hermes(&status.status).is_none_or(TaskStatus::is_terminal) {
                break;
            }
            let served = status.runtime.clone().or_else(|| {
                Some(crate::hermes::events::Runtime {
                    provider: chosen.provider.clone()?,
                    model: chosen.model.clone()?,
                })
            });
            let cost = estimate(&app.state::<AppState>(), &status.usage, served.as_ref());
            if models::over_cap(cost, Some(cap)) {
                let _ = client.stop_run(&run_id).await;
                let _ = app.emit(
                    "cost-cap",
                    CostCapHit {
                        task_id: task_id.clone(),
                        cap,
                        cost: cost.unwrap_or_default(),
                    },
                );
                break;
            }
        }
    });
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

// --- Starting with Windows (M3 Step 7) -------------------------------------------------------

/// Set when Kairos was started by its login entry (`--autostart`), for Settings ▸ Startup.
pub static LAUNCHED_AT_LOGIN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
pub struct StartupState {
    /// Kairos's own login entry; `None` if it couldn't be read.
    pub kairos_at_login: Option<bool>,
    /// Hermes's login start; `None` off Windows or when Task Scheduler didn't answer.
    pub hermes_at_login: Option<local::GatewayAutostart>,
    pub launched_at_login: bool,
    /// Newest first.
    pub recent: Vec<PresenceEvent>,
}

#[tauri::command]
pub async fn startup_state(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<StartupState, CommandError> {
    use tauri_plugin_autostart::ManagerExt;
    let hermes_at_login = tauri::async_runtime::spawn_blocking(local::gateway_autostart)
        .await
        .ok()
        .flatten();
    Ok(StartupState {
        kairos_at_login: app.autolaunch().is_enabled().ok(),
        hermes_at_login,
        launched_at_login: LAUNCHED_AT_LOGIN.load(std::sync::atomic::Ordering::Relaxed),
        recent: state.with_store(|store| store.recent_presence(8))?,
    })
}

#[tauri::command]
pub async fn set_kairos_at_login(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<bool, CommandError> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let outcome = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    outcome.map_err(|err| CommandError {
        kind: "autostart",
        message: format!("Windows didn't accept the change: {err}"),
    })?;
    manager.is_enabled().map_err(|err| CommandError {
        kind: "autostart",
        message: format!("couldn't read the login entry back: {err}"),
    })
}

const LAST_SEEN: &str = "last_seen";

/// When the PC was last seen in use (Unix seconds), for the time away at startup (M5 §2.1).
pub fn last_seen(state: &AppState) -> Option<i64> {
    state
        .with_store(|store| store.setting(LAST_SEEN))
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
}

pub fn set_last_seen(state: &AppState, now: i64) {
    let _ = state.with_store(|store| store.set_setting(LAST_SEEN, &now.to_string()));
}

/// Records a presence event; failures are logged, never shown (it's a background record).
pub fn record_presence(state: &AppState, event: &PresenceEvent) {
    if let Err(err) = state.with_store(|store| store.add_presence(event)) {
        eprintln!("kairos: couldn't record {:?}: {}", event.kind, err.message);
    }
}

/// On the first launch of a release build, Kairos registers itself to start at login (D-013).
/// Done once: if the user later switches it off, it stays off. Dev builds never do it, so the
/// login entry never points at a debug build.
pub fn first_launch_autostart(app: &tauri::AppHandle, state: &AppState) {
    use tauri_plugin_autostart::ManagerExt;
    const DONE: &str = "autostart_initialized";
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(None) = state.with_store(|store| store.setting(DONE)) else {
        return;
    };
    if app.autolaunch().enable().is_ok() {
        let _ = state.with_store(|store| store.set_setting(DONE, "true"));
    }
}

// --- Notifications (M4 Step 5) ----------------------------------------------------------------

const NOTIFICATIONS_LIST_MAX: u32 = 500;

#[derive(Debug, Clone, Serialize)]
pub struct NotificationsState {
    /// The Kairos switch (off by default).
    pub enabled: bool,
    pub access: Access,
    pub state: CollectState,
    pub unread: i64,
    pub counts: Vec<AppCount>,
}

fn notifications_state_now(state: &AppState) -> Result<NotificationsState, CommandError> {
    let access = notifications::access();
    state.with_store(|store| {
        let enabled = store.setting(ENABLED_SETTING)?.as_deref() == Some("true");
        Ok(NotificationsState {
            enabled,
            access,
            state: collect::collect_state(enabled, access),
            unread: store.unread_notifications()?,
            counts: store.notification_counts()?,
        })
    })
}

/// Brings `notifications.json` in line with the store (D-018) and tells the panel and the badge to
/// re-read. A failed file write is logged, never shown: the store is still right.
pub fn notifications_changed(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let synced = state.with_store(|store| {
        let enabled = store.setting(ENABLED_SETTING)?.as_deref() == Some("true");
        notifications::export::sync(
            store,
            &notifications::export::default_path()?,
            enabled,
            presence::unix_now(),
        )
    });
    if let Err(err) = synced {
        eprintln!(
            "kairos: couldn't update notifications.json: {}",
            err.message
        );
    }
    let _ = app.emit("notifications-changed", ());
}

#[tauri::command]
pub async fn notifications_state(
    state: tauri::State<'_, AppState>,
) -> Result<NotificationsState, CommandError> {
    notifications_state_now(&state)
}

/// The Kairos switch. Turning it on starts the "collect from now" clock (M4 §5): what is already
/// in Action Center arrived before permission and is left alone.
#[tauri::command]
pub async fn set_notifications_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    switch: tauri::State<'_, std::sync::Arc<NotificationSwitch>>,
    enabled: bool,
) -> Result<NotificationsState, CommandError> {
    let now = presence::unix_now();
    let since = state.with_store(|store| {
        if enabled {
            store.set_setting(SINCE_SETTING, &now.to_string())?;
        }
        store.set_setting(ENABLED_SETTING, if enabled { "true" } else { "false" })?;
        Ok(store
            .setting(SINCE_SETTING)?
            .and_then(|s| s.parse().ok())
            .unwrap_or(now))
    })?;
    // Off takes effect at once: the poller checks this before every read.
    switch.set(enabled, since);
    #[cfg(windows)]
    if enabled && notifications::access() == Access::Unspecified {
        // Windows hasn't been asked yet; it may show its prompt now.
        let handle = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            notifications::windows::request_access(&handle)
        })
        .await;
    }
    notifications_changed(&app);
    notifications_state_now(&state)
}

/// The switch as stored, for the poller at startup.
pub fn stored_notification_switch(state: &AppState) -> NotificationSwitch {
    state
        .with_store(|store| {
            let enabled = store.setting(ENABLED_SETTING)?.as_deref() == Some("true");
            let since = store
                .setting(SINCE_SETTING)?
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            Ok(NotificationSwitch::new(enabled, since))
        })
        .unwrap_or_default()
}

/// Stores what a poll found; tells the panel only if something was new.
#[cfg(windows)]
pub fn store_notifications(
    app: &tauri::AppHandle,
    fresh: &[notifications::store::NewNotification],
) {
    let state = app.state::<AppState>();
    let added = state.with_store(|store| {
        let mut added = 0;
        for n in fresh {
            if store.insert_notification(n)? {
                added += 1;
            }
        }
        Ok(added)
    });
    match added {
        Ok(0) => {}
        Ok(_) => notifications_changed(app),
        Err(err) => eprintln!("kairos: couldn't store notifications: {}", err.message),
    }
}

#[tauri::command]
pub async fn list_notifications(
    state: tauri::State<'_, AppState>,
    app_id: Option<String>,
    unread_only: bool,
) -> Result<Vec<StoredNotification>, CommandError> {
    state.with_store(|store| {
        store.notifications(app_id.as_deref(), unread_only, NOTIFICATIONS_LIST_MAX)
    })
}

/// Marks one notification (`id`), one app's (`app_id`), or all (neither) as read.
#[tauri::command]
pub async fn mark_notifications_read(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: Option<i64>,
    app_id: Option<String>,
) -> Result<usize, CommandError> {
    let changed = state.with_store(|store| {
        store.mark_notifications_read(id, app_id.as_deref(), presence::unix_now())
    })?;
    notifications_changed(&app);
    Ok(changed)
}

/// Deletes one app's notifications, or all of them. The panel asks before calling this.
#[tauri::command]
pub async fn clear_notifications(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    app_id: Option<String>,
) -> Result<usize, CommandError> {
    let removed = state.with_store(|store| store.clear_notifications(app_id.as_deref()))?;
    notifications_changed(&app);
    Ok(removed)
}

// --- Welcome-back briefing (M5 Step 5) --------------------------------------------------------

/// Decides and, if it's time, builds, stores and announces a briefing (M5 §2). `away_s` is how
/// long the user was away; `asked` skips the rules ("Brief me now").
pub fn brief(app: &tauri::AppHandle, away_s: Option<i64>, asked: bool) -> Option<Briefing> {
    let state = app.state::<AppState>();
    let clock = app.state::<Arc<BriefingClock>>();
    let now = presence::unix_now();
    let outcome = state.with_store(|store| {
        let settings = briefing::load_settings(store)?;
        let last = briefing::load_last(store)?;
        let reason = if asked {
            Some(Reason::Asked)
        } else {
            briefing_rules::decide(
                &settings,
                &briefing_rules::Moment {
                    now,
                    utc_offset_min: clock.offset().unwrap_or(0),
                    away_s,
                    last_briefing_at: last.as_ref().map(|b| b.at),
                },
            )
        };
        let Some(reason) = reason else {
            return Ok(None);
        };
        // "While you were away": since the user left, or since the last briefing when unknown.
        let since = away_s
            .map(|a| now - a)
            .or(last.as_ref().map(|b| b.at))
            .unwrap_or(now - 24 * 3600);
        let snapshot = app.state::<Arc<SystemMonitor>>().latest();
        let counts = store.notification_counts()?;
        let facts = briefing_facts::build(&briefing_facts::Inputs {
            away_s,
            snapshot: snapshot.as_ref(),
            notifications: &counts,
            tasks: store.task_summary_since(since)?,
        });
        let briefing = Briefing {
            at: now,
            reason,
            away_s,
            facts,
        };
        briefing::save_last(store, &briefing)?;
        Ok(Some((briefing, settings)))
    });
    let (briefing, settings) = match outcome {
        Ok(Some(found)) => found,
        Ok(None) => return None,
        Err(err) => {
            eprintln!("kairos: couldn't prepare the briefing: {}", err.message);
            return None;
        }
    };
    let _ = app.emit(
        "briefing",
        BriefingNotice {
            briefing: briefing.clone(),
            ai_summary: settings.ai_summary,
            projects: settings.projects.clone(),
        },
    );
    {
        use tauri_plugin_notification::NotificationExt;
        let body = briefing
            .facts
            .iter()
            .find(|f| f.tone == briefing_facts::Tone::Warn)
            .or(briefing.facts.first())
            .map_or_else(
                || "Nothing needs your attention.".to_string(),
                |f| f.text.clone(),
            );
        let _ = app
            .notification()
            .builder()
            .title("Welcome back")
            .body(body)
            .show();
    }
    Some(briefing)
}

/// A return to the PC (M3 presence event): startup waits for the page's clock (quiet hours are
/// local), the others are judged at once.
pub fn on_presence(app: &tauri::AppHandle, event: &PresenceEvent) {
    let clock = app.state::<Arc<BriefingClock>>();
    if event.kind == presence::PresenceKind::Startup && clock.offset().is_none() {
        clock.hold_startup(event.away_s);
        return;
    }
    brief(app, event.away_s, false);
}

/// The page's UTC offset in minutes (east of UTC positive), at startup and every hour.
#[tauri::command]
pub async fn set_utc_offset(app: tauri::AppHandle, minutes: i32) -> Result<(), CommandError> {
    let pending = app.state::<Arc<BriefingClock>>().set_offset(minutes);
    if let Some(away_s) = pending {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || brief(&handle, away_s, false));
    }
    Ok(())
}

#[tauri::command]
pub async fn briefing_settings(
    state: tauri::State<'_, AppState>,
) -> Result<BriefingSettings, CommandError> {
    state.with_store(briefing::load_settings)
}

#[tauri::command]
pub async fn save_briefing_settings(
    state: tauri::State<'_, AppState>,
    settings: BriefingSettings,
) -> Result<BriefingSettings, CommandError> {
    state.with_store(|store| {
        briefing::save_settings(store, &settings)?;
        briefing::load_settings(store)
    })
}

#[tauri::command]
pub async fn latest_briefing(
    state: tauri::State<'_, AppState>,
) -> Result<Option<Briefing>, CommandError> {
    state.with_store(briefing::load_last)
}

/// "Brief me now": a briefing whatever the rules say, for testing and on request.
#[tauri::command]
pub async fn brief_now(app: tauri::AppHandle) -> Result<Option<Briefing>, CommandError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || brief(&handle, None, true))
        .await
        .map_err(|err| CommandError {
            kind: "internal",
            message: format!("briefing failed: {err}"),
        })
}

// --- Accounts (M6 Step 3) ---------------------------------------------------------------------

impl From<accounts::AccountError> for CommandError {
    fn from(err: accounts::AccountError) -> Self {
        Self {
            kind: "account",
            message: err.0,
        }
    }
}

#[tauri::command]
pub async fn accounts_state() -> Result<Vec<AccountState>, CommandError> {
    tauri::async_runtime::spawn_blocking(|| {
        let home = local::hermes_home().ok();
        accounts::states(home.as_deref())
    })
    .await
    .map_err(|err| CommandError {
        kind: "internal",
        message: format!("couldn't read the accounts: {err}"),
    })
}

/// GitHub: opens the sign-in window. Google's sign-in is a conversation with Hermes, started by
/// the page, so it has nothing to do here.
#[tauri::command]
pub async fn connect_account(kind: AccountKind) -> Result<(), CommandError> {
    match kind {
        AccountKind::Github => Ok(accounts::github_connect()?),
        AccountKind::Google => Err(CommandError {
            kind: "account",
            message: "Google is connected through a conversation with Hermes.".into(),
        }),
    }
}

/// Disconnects directly, not through the model, so it works whatever the model does. The page
/// asks before calling this.
#[tauri::command]
pub async fn disconnect_account(kind: AccountKind) -> Result<Vec<AccountState>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<AccountState>, CommandError> {
        let home = local::hermes_home().ok();
        match kind {
            AccountKind::Github => accounts::github_disconnect()?,
            AccountKind::Google => {
                let home = home.as_deref().ok_or(LocalError::NoHome)?;
                accounts::google_disconnect(home, rules::hermes_python(home))?
            }
        }
        Ok(accounts::states(home.as_deref()))
    })
    .await
    .map_err(|err| CommandError {
        kind: "internal",
        message: format!("disconnect failed: {err}"),
    })?
}

// --- Models and sandbox (M7) -----------------------------------------------------------------

/// The models Hermes can route to, with prices (M7 §4). Also kept to price finished runs.
#[tauri::command]
pub async fn model_catalog(state: tauri::State<'_, AppState>) -> Result<Catalog, CommandError> {
    let client = state.connected_client()?;
    let catalog = models::parse_catalog(&client.model_options().await?);
    if let Ok(mut cached) = state.catalog.lock() {
        *cached = Some(catalog.clone());
    }
    Ok(catalog)
}

#[tauri::command]
pub async fn conversation_model(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<ModelChoice, CommandError> {
    state.with_store(|store| store.conversation_model(&session_id))
}

/// Where the agent's commands run (`local`, `docker`, …), or `None` when Hermes can't say.
#[tauri::command]
pub async fn terminal_backend() -> Result<Option<String>, CommandError> {
    let home = local::hermes_home()?;
    tauri::async_runtime::spawn_blocking(move || local::terminal_backend(&home))
        .await
        .map_err(|err| CommandError {
            kind: "internal",
            message: format!("couldn't ask Hermes: {err}"),
        })
}

// --- Quick box (M8 Step 2) ---------------------------------------------------------------------

/// The quick box's hotkey as stored, or the default.
pub fn stored_hotkey(state: &AppState) -> String {
    state
        .with_store(|store| store.setting(crate::quick::HOTKEY_SETTING))
        .ok()
        .flatten()
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| crate::quick::DEFAULT_HOTKEY.to_string())
}

#[tauri::command]
pub async fn quick_hotkey(state: tauri::State<'_, AppState>) -> Result<String, CommandError> {
    Ok(stored_hotkey(&state))
}

/// Registers a new hotkey and keeps it only if Windows accepts it.
#[tauri::command]
pub async fn set_quick_hotkey(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<String, CommandError> {
    let old = stored_hotkey(&state);
    crate::quick::register(&app, Some(&old), &hotkey).map_err(|message| CommandError {
        kind: "hotkey",
        message,
    })?;
    let hotkey = hotkey.trim().to_string();
    state.with_store(|store| store.set_setting(crate::quick::HOTKEY_SETTING, &hotkey))?;
    Ok(hotkey)
}

#[tauri::command]
pub async fn hide_quick(app: tauri::AppHandle) -> Result<(), CommandError> {
    crate::quick::hide(&app);
    Ok(())
}

#[tauri::command]
pub async fn show_quick(app: tauri::AppHandle) -> Result<(), CommandError> {
    crate::quick::show(&app);
    Ok(())
}

/// "Open in Kairos" from the quick box: the main window shows that conversation.
#[tauri::command]
pub async fn open_in_main(app: tauri::AppHandle, session_id: String) -> Result<(), CommandError> {
    crate::quick::hide(&app);
    crate::tray::show_main(&app);
    let _ = app.emit_to(crate::tray::MAIN_WINDOW, "open-conversation", session_id);
    Ok(())
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

// --- Scheduled jobs (M8 Step 4) -----------------------------------------------------------------

#[tauri::command]
pub async fn list_jobs(state: tauri::State<'_, AppState>) -> Result<Vec<Job>, CommandError> {
    Ok(state.connected_client()?.list_jobs().await?)
}

/// A job made here goes straight to Hermes, not through the agent, so the `scheduled-jobs` rule
/// doesn't ask: the user is the one scheduling it. Its runs still go through every rule.
#[tauri::command]
pub async fn create_job(
    state: tauri::State<'_, AppState>,
    name: String,
    schedule: String,
    prompt: String,
) -> Result<Job, CommandError> {
    let (name, schedule, prompt) = (name.trim(), schedule.trim(), prompt.trim());
    if name.is_empty() || schedule.is_empty() || prompt.is_empty() {
        return Err(CommandError {
            kind: "invalid",
            message: "A job needs a name, a schedule and what to do.".into(),
        });
    }
    let client = state.connected_client()?;
    Ok(client
        .create_job(&NewJob::local(name, schedule, prompt))
        .await?)
}

#[tauri::command]
pub async fn job_action(
    state: tauri::State<'_, AppState>,
    job_id: String,
    action: JobAction,
) -> Result<Job, CommandError> {
    Ok(state
        .connected_client()?
        .job_action(&job_id, action)
        .await?)
}

/// The page asks the user before calling this.
#[tauri::command]
pub async fn delete_job(
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<(), CommandError> {
    Ok(state.connected_client()?.delete_job(&job_id).await?)
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
            runtime: None,
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

// --- Voice (M9, D-022) --------------------------------------------------------------------------

use crate::voice::models::{self as voice_models, DownloadProgress, ModelId, ModelInfo};
use crate::voice::tts::{self as voice_tts, Speaker};
use crate::voice::{self, VoiceError, VoiceState};

impl From<VoiceError> for CommandError {
    fn from(err: VoiceError) -> Self {
        Self {
            kind: "voice",
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceSettings {
    pub models: Vec<ModelInfo>,
    pub model: ModelId,
    /// `None` = detect per recording.
    pub language: Option<String>,
    pub read_aloud: bool,
    /// Answers to spoken requests are read aloud (on by default).
    pub answer_spoken: bool,
    pub ptt_hotkey: String,
    pub downloading: bool,
    /// The natural voice (Kokoro, M9 Step 8).
    pub natural_installed: bool,
    pub natural_installing: bool,
    pub natural_size_bytes: u64,
    /// "natural" or "windows": what answers are read with now.
    pub engine: &'static str,
    pub speaker: Speaker,
    /// The wake word "Kairos" (M9 Step 9).
    pub wake_installed: bool,
    pub wake_installing: bool,
    pub wake_enabled: bool,
    /// "strict" | "normal" | "eager".
    pub wake_sensitivity: String,
    /// What a wake-word download still needs (the runtime is shared with the natural voice).
    pub wake_size_bytes: u64,
}

fn setting_or(state: &AppState, key: &str) -> Option<String> {
    state
        .with_store(|store| store.setting(key))
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
}

pub fn stored_ptt_hotkey(state: &AppState) -> String {
    setting_or(state, voice::PTT_SETTING).unwrap_or_else(|| voice::DEFAULT_PTT.to_string())
}

fn stored_voice_model(state: &AppState) -> ModelId {
    setting_or(state, voice::MODEL_SETTING)
        .and_then(|m| ModelId::parse(&m))
        .unwrap_or(ModelId::Small)
}

fn voice_settings(state: &AppState, voice: &VoiceState) -> Result<VoiceSettings, CommandError> {
    let dir = voice_models::default_dir()?;
    let voices = crate::voice::tts::default_dir()?;
    let natural_installed = crate::voice::tts::is_installed(&voices);
    let wake_installed =
        crate::voice::tts::is_installed_all(&voices, &crate::voice::wake::PACKAGES);
    Ok(VoiceSettings {
        models: voice_models::list(&dir),
        model: stored_voice_model(state),
        language: setting_or(state, voice::LANGUAGE_SETTING)
            .filter(|l| crate::voice::stt::language_setting(Some(l)).is_some()),
        read_aloud: setting_or(state, voice::READ_ALOUD_SETTING).as_deref() == Some("1"),
        answer_spoken: setting_or(state, voice::ANSWER_SPOKEN_SETTING).as_deref() != Some("0"),
        ptt_hotkey: stored_ptt_hotkey(state),
        downloading: voice.downloading.load(std::sync::atomic::Ordering::SeqCst),
        natural_installed,
        natural_installing: voice
            .installing_voice
            .load(std::sync::atomic::Ordering::SeqCst),
        natural_size_bytes: crate::voice::tts::total_size(),
        engine: if natural_installed
            && setting_or(state, voice::ENGINE_SETTING).as_deref() != Some("windows")
        {
            "natural"
        } else {
            "windows"
        },
        speaker: stored_speaker(state),
        wake_installed,
        wake_installing: voice
            .installing_wake
            .load(std::sync::atomic::Ordering::SeqCst),
        wake_enabled: wake_installed
            && setting_or(state, voice::WAKE_SETTING).as_deref() == Some("1"),
        wake_sensitivity: stored_wake_sensitivity(state),
        wake_size_bytes: crate::voice::wake::PACKAGES
            .iter()
            .filter(|p| !voices.join(p.folder).join(p.marker).is_file())
            .map(|p| (p.expected)().size)
            .sum(),
    })
}

#[tauri::command]
pub async fn voice_state(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
) -> Result<VoiceSettings, CommandError> {
    voice_settings(&state, &voice)
}

/// Saves the model, language and the two read-aloud choices (not the hotkey: `set_ptt_hotkey`).
#[tauri::command]
pub async fn save_voice_settings(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    model: ModelId,
    language: Option<String>,
    read_aloud: bool,
    answer_spoken: bool,
) -> Result<VoiceSettings, CommandError> {
    let language = language
        .as_deref()
        .and_then(|l| crate::voice::stt::language_setting(Some(l)))
        .unwrap_or("");
    state.with_store(|store| {
        store.set_setting(voice::MODEL_SETTING, model.as_str())?;
        store.set_setting(voice::LANGUAGE_SETTING, language)?;
        store.set_setting(
            voice::READ_ALOUD_SETTING,
            if read_aloud { "1" } else { "0" },
        )?;
        store.set_setting(
            voice::ANSWER_SPOKEN_SETTING,
            if answer_spoken { "1" } else { "0" },
        )
    })?;
    voice_settings(&state, &voice)
}

/// Downloads a model on the user's click, sending `voice-model` progress events.
#[tauri::command]
pub async fn download_voice_model(
    app: tauri::AppHandle,
    voice: tauri::State<'_, Arc<VoiceState>>,
    id: ModelId,
) -> Result<(), CommandError> {
    use std::sync::atomic::Ordering;
    if voice.downloading.swap(true, Ordering::SeqCst) {
        return Err(CommandError {
            kind: "busy",
            message: "A voice model is already downloading.".into(),
        });
    }
    let dir = voice_models::default_dir();
    let outcome = match dir {
        Ok(dir) => {
            voice_models::download(&dir, id, |progress| {
                let _ = app.emit("voice-model", progress);
            })
            .await
        }
        Err(err) => Err(err),
    };
    voice.downloading.store(false, Ordering::SeqCst);
    let _ = app.emit(
        "voice-model",
        match &outcome {
            Ok(()) => DownloadProgress::Done { id },
            Err(err) => DownloadProgress::Failed {
                id,
                message: err.to_string(),
            },
        },
    );
    Ok(outcome?)
}

#[tauri::command]
pub async fn delete_voice_model(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    id: ModelId,
) -> Result<VoiceSettings, CommandError> {
    voice.engine.unload();
    voice_models::delete(&voice_models::default_dir()?, id)?;
    voice_settings(&state, &voice)
}

/// Transcribes one recording. The body is the raw audio, 16 kHz mono signed 16-bit little-endian
/// (half the size of floats, and no JSON for a minute of samples); the `language` header is "en",
/// "fr", "ar" or absent (detect).
#[tauri::command]
pub async fn transcribe(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    request: tauri::ipc::Request<'_>,
) -> Result<crate::voice::stt::Transcript, CommandError> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(VoiceError::Audio("expected raw audio".into()).into());
    };
    let samples = crate::voice::stt::pcm16_to_f32(bytes);
    let language = request
        .headers()
        .get("language")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let id = stored_voice_model(&state);
    let dir = voice_models::default_dir()?;
    if !voice_models::is_ready(&dir, id) {
        return Err(CommandError {
            kind: "voice_model_missing",
            message: "Download a voice model first: Settings ▸ Voice.".into(),
        });
    }
    let model = voice_models::path(&dir, id);
    let voice = Arc::clone(&voice);
    tauri::async_runtime::spawn_blocking(move || {
        voice
            .engine
            .transcribe(&model, &samples, language.as_deref())
    })
    .await
    .map_err(|err| CommandError {
        kind: "internal",
        message: format!("transcription stopped: {err}"),
    })?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn ptt_hotkey(state: tauri::State<'_, AppState>) -> Result<String, CommandError> {
    Ok(stored_ptt_hotkey(&state))
}

/// Registers a new push-to-talk key and keeps it only if Windows accepts it.
#[tauri::command]
pub async fn set_ptt_hotkey(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<String, CommandError> {
    if crate::quick::parse_hotkey(&hotkey).ok()
        == crate::quick::parse_hotkey(&stored_hotkey(&state)).ok()
    {
        return Err(CommandError {
            kind: "hotkey",
            message: "That's the quick box's hotkey; push-to-talk needs its own.".into(),
        });
    }
    let old = stored_ptt_hotkey(&state);
    crate::quick::register_ptt(&app, Some(&old), &hotkey).map_err(|message| CommandError {
        kind: "hotkey",
        message,
    })?;
    let hotkey = hotkey.trim().to_string();
    state.with_store(|store| store.set_setting(voice::PTT_SETTING, &hotkey))?;
    Ok(hotkey)
}

// --- Persona (M10, D-023) ----------------------------------------------------------------------

use crate::persona::{PersonaError, PersonaFiles, PersonaState};

impl From<PersonaError> for CommandError {
    fn from(err: PersonaError) -> Self {
        Self {
            kind: "persona",
            message: err.to_string(),
        }
    }
}

fn persona_files() -> Result<PersonaFiles, CommandError> {
    let kairos_dir = store::default_path()?
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| CommandError {
            kind: "persona",
            message: "couldn't find the Kairos folder".into(),
        })?;
    Ok(PersonaFiles::new(&local::hermes_home()?, &kairos_dir))
}

#[tauri::command]
pub async fn persona_state() -> Result<PersonaState, CommandError> {
    Ok(persona_files()?.state()?)
}

/// Writes Hermes's SOUL.md, only on the user's Save (D-023).
#[tauri::command]
pub async fn save_persona(text: String) -> Result<PersonaState, CommandError> {
    Ok(persona_files()?.save(&text)?)
}

#[tauri::command]
pub async fn restore_persona() -> Result<PersonaState, CommandError> {
    Ok(persona_files()?.restore()?)
}

fn stored_speaker(state: &AppState) -> Speaker {
    setting_or(state, voice::SPEAKER_SETTING)
        .and_then(|s| Speaker::parse(&s))
        .unwrap_or(Speaker::George)
}

/// The natural voice's engine and speaker (M9 Step 8).
#[tauri::command]
pub async fn save_speech_settings(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    natural: bool,
    speaker: Speaker,
) -> Result<VoiceSettings, CommandError> {
    state.with_store(|store| {
        store.set_setting(
            voice::ENGINE_SETTING,
            if natural { "natural" } else { "windows" },
        )?;
        store.set_setting(voice::SPEAKER_SETTING, speaker.as_str())
    })?;
    voice_settings(&state, &voice)
}

/// Downloads and unpacks the natural voice on the user's click, sending `voice-install` events
/// ({received, total}, then {done} or {failed}).
#[tauri::command]
pub async fn install_natural_voice(
    app: tauri::AppHandle,
    voice: tauri::State<'_, Arc<VoiceState>>,
) -> Result<(), CommandError> {
    use std::sync::atomic::Ordering;
    if voice.installing_voice.swap(true, Ordering::SeqCst) {
        return Err(CommandError {
            kind: "busy",
            message: "The natural voice is already downloading.".into(),
        });
    }
    let outcome = match voice_tts::default_dir() {
        Ok(dir) => {
            voice_tts::install(&dir, |received, total| {
                let _ = app.emit(
                "voice-install",
                serde_json::json!({ "phase": "downloading", "received": received, "total": total }),
            );
            })
            .await
        }
        Err(err) => Err(err),
    };
    voice.installing_voice.store(false, Ordering::SeqCst);
    let _ = app.emit(
        "voice-install",
        match &outcome {
            Ok(()) => serde_json::json!({ "phase": "done" }),
            Err(err) => serde_json::json!({ "phase": "failed", "message": err.to_string() }),
        },
    );
    Ok(outcome?)
}

#[tauri::command]
pub async fn remove_natural_voice(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
) -> Result<VoiceSettings, CommandError> {
    voice.voice.unload();
    voice_tts::remove(&voice_tts::default_dir()?)?;
    voice_settings(&state, &voice)
}

/// One sentence in the natural voice, as WAV bytes (the page queues the sentences of an answer).
#[tauri::command]
pub async fn speak_natural(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    text: String,
) -> Result<tauri::ipc::Response, CommandError> {
    let speaker = stored_speaker(&state);
    let dir = voice_tts::default_dir()?;
    let voice = Arc::clone(&voice);
    let (samples, rate) =
        tauri::async_runtime::spawn_blocking(move || voice.voice.speak(&dir, &text, speaker, 1.0))
            .await
            .map_err(|err| CommandError {
                kind: "internal",
                message: format!("speaking stopped: {err}"),
            })??;
    Ok(tauri::ipc::Response::new(voice_tts::wav(&samples, rate)))
}

/// Loads the natural voice in the background (when the user starts speaking), so the answer
/// doesn't wait for it. Errors are ignored here: speaking reports them.
#[tauri::command]
pub async fn warm_natural_voice(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
) -> Result<(), CommandError> {
    if voice_settings(&state, &voice)?.engine != "natural" {
        return Ok(());
    }
    let dir = voice_tts::default_dir()?;
    let voice = Arc::clone(&voice);
    tauri::async_runtime::spawn_blocking(move || {
        let _ = voice.voice.warm(&dir);
    });
    Ok(())
}

// --- Memory (M11, D-026) ------------------------------------------------------------------------

/// What Kairos remembers: Hermes's memory files and the fact store, read-only.
#[tauri::command]
pub async fn memory_state() -> Result<crate::memory::MemoryState, CommandError> {
    let home = local::hermes_home()?;
    tauri::async_runtime::spawn_blocking(move || {
        let provider = local::memory_provider(&home);
        crate::memory::read_all(&home, provider)
    })
    .await
    .map_err(|err| CommandError {
        kind: "internal",
        message: format!("couldn't read memory: {err}"),
    })?
    .map_err(|err| CommandError {
        kind: "memory",
        message: err.to_string(),
    })
}

// --- Wake word (M9 Step 9) ----------------------------------------------------------------------

fn stored_wake_sensitivity(state: &AppState) -> String {
    setting_or(state, voice::WAKE_SENSITIVITY_SETTING)
        .filter(|s| ["strict", "normal", "eager"].contains(&s.as_str()))
        .unwrap_or_else(|| "normal".to_string())
}

#[tauri::command]
pub async fn save_wake_settings(
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    enabled: bool,
    sensitivity: String,
) -> Result<VoiceSettings, CommandError> {
    let sensitivity = if ["strict", "normal", "eager"].contains(&sensitivity.as_str()) {
        sensitivity
    } else {
        "normal".to_string()
    };
    state.with_store(|store| {
        store.set_setting(voice::WAKE_SETTING, if enabled { "1" } else { "0" })?;
        store.set_setting(voice::WAKE_SENSITIVITY_SETTING, &sensitivity)
    })?;
    if !enabled {
        voice.wake.unload();
    }
    voice_settings(&state, &voice)
}

/// Downloads the wake-word model (and the runtime, if the natural voice isn't installed), on the
/// user's click, sending `voice-install` events like the natural voice.
#[tauri::command]
pub async fn install_wake_word(
    app: tauri::AppHandle,
    voice: tauri::State<'_, Arc<VoiceState>>,
) -> Result<(), CommandError> {
    use std::sync::atomic::Ordering;
    if voice.installing_wake.swap(true, Ordering::SeqCst) {
        return Err(CommandError {
            kind: "busy",
            message: "The wake word is already downloading.".into(),
        });
    }
    let outcome = match voice_tts::default_dir() {
        Ok(dir) => {
            voice_tts::install_packages(&dir, &crate::voice::wake::PACKAGES, |received, total| {
                let _ = app.emit(
                    "voice-install",
                    serde_json::json!({ "phase": "downloading", "received": received, "total": total, "what": "wake" }),
                );
            })
            .await
        }
        Err(err) => Err(err),
    };
    voice.installing_wake.store(false, Ordering::SeqCst);
    let _ = app.emit(
        "voice-install",
        match &outcome {
            Ok(()) => serde_json::json!({ "phase": "done", "what": "wake" }),
            Err(err) => {
                serde_json::json!({ "phase": "failed", "message": err.to_string(), "what": "wake" })
            }
        },
    );
    Ok(outcome?)
}

/// One chunk of the always-on microphone (raw 16 kHz 16-bit mono, as for `transcribe`). When
/// "Kairos" is heard, the quick box opens and is told to listen.
#[tauri::command]
pub async fn wake_feed(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    voice: tauri::State<'_, Arc<VoiceState>>,
    request: tauri::ipc::Request<'_>,
) -> Result<bool, CommandError> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(VoiceError::Audio("expected raw audio".into()).into());
    };
    let samples = crate::voice::stt::pcm16_to_f32(bytes);
    let sensitivity = stored_wake_sensitivity(&state);
    let dir = voice_tts::default_dir()?;
    let voice = Arc::clone(&voice);
    let heard =
        tauri::async_runtime::spawn_blocking(move || voice.wake.feed(&dir, &samples, &sensitivity))
            .await
            .map_err(|err| CommandError {
                kind: "internal",
                message: format!("the wake word stopped: {err}"),
            })??;
    if heard {
        crate::quick::show(&app);
        let _ = app.emit_to(crate::quick::QUICK_WINDOW, "wake", ());
    }
    Ok(heard)
}
