// Typed wrappers around Tauri `invoke` for the commands in M1 §8.
// Shapes mirror the Rust types in src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { KairosState } from "../features/island/model";
import type { RunEventPayload, RunStreamEnd } from "./hermes-events";

export type ConnectStatus =
  "connected" | "not_running" | "key_rejected" | "capability_missing" | "no_key";

export interface ConnectInfo {
  status: ConnectStatus;
  base_url: string;
  hermes_home: string | null;
  version: string | null;
  missing_capabilities: string[];
  approval_mode: string | null;
  key_source: "keyring" | "env" | null;
  detail: string | null;
}

export interface CommandError {
  kind: string;
  message: string;
}

// Tauri rejects with whatever the command returned as its error; normalise anything else.
function asCommandError(err: unknown): CommandError {
  if (typeof err === "object" && err !== null && "kind" in err && "message" in err) {
    return err as CommandError;
  }
  return { kind: "internal", message: String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw asCommandError(err);
  }
}

export interface TaskRef {
  task_id: string;
  session_id: string;
  run_id: string;
  replayed: boolean;
}

export const connect = () => call<ConnectInfo>("connect");
export const reloadKey = () => call<ConnectInfo>("reload_key");
/** `hermes gateway ensure` (D-035). `automatic`: only if Settings ▸ Startup allows it. False when
 *  it didn't try. Takes up to a minute. */
export const startHermes = (automatic: boolean) => call<boolean>("start_hermes", { automatic });
export const setStartHermes = (enabled: boolean) => call<boolean>("set_start_hermes", { enabled });

// Tauri maps camelCase argument names here to the snake_case Rust parameters.
/** A conversation's model (M7); both null = Hermes's own choice. */
export interface ModelChoice {
  provider: string | null;
  model: string | null;
}

export const createRun = (sessionId: string | null, input: string, choice: ModelChoice | null) =>
  call<TaskRef>("create_run", { sessionId, input, choice });
export const subscribeRun = (taskId: string, runId: string) =>
  call<void>("subscribe_run", { taskId, runId });

export type ApprovalChoice = "once" | "session" | "always" | "deny";

export const answerApproval = (
  runId: string,
  promptId: string,
  executionGeneration: number,
  choice: ApprovalChoice,
) => call<void>("answer_approval", { runId, promptId, executionGeneration, choice });

export const stopRun = (taskId: string, runId: string) => call<void>("stop_run", { taskId, runId });

/** A row of kairos.db's task table (store.rs `Task`). */
export interface StoredTask {
  id: string;
  session_id: string;
  run_id: string | null;
  goal: string;
  status:
    | "created"
    | "running"
    | "waiting_for_approval"
    | "stopping"
    | "completed"
    | "incomplete"
    | "failed"
    | "cancelled"
    | "interrupted"
    | "unknown"
    | "lost";
  created_at: number;
  finished_at: number | null;
  outcome: string | null;
  /** From run.completed (M7). cost_usd null = price unknown. */
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cost_usd: number | null;
  served_provider: string | null;
  served_model: string | null;
}

export interface Conversation {
  session_id: string;
  title: string;
  /** Unix seconds. */
  last_active: number;
  task_count: number;
}

export interface OtherConversation {
  id: string;
  title: string | null;
  source: string | null;
  model: string | null;
  last_active: number | null;
  message_count: number;
}

export interface ConversationList {
  kairos: Conversation[];
  other: OtherConversation[];
}

export const resumeOpenTasks = () => call<StoredTask[]>("resume_open_tasks");
export const listConversations = () => call<ConversationList>("list_conversations");
export const conversationTasks = (sessionId: string) =>
  call<StoredTask[]>("conversation_tasks", { sessionId });
/** Deletes a Kairos conversation in Hermes and in Kairos's list; refused while it is running. */
export const deleteConversation = (sessionId: string) =>
  call<null>("delete_conversation", { sessionId });

/** One tool call as the Kairos plugin recorded it in audit.db (audit.rs `AuditEvent`). */
export interface AuditEvent {
  /** Unix seconds. */
  at: number;
  tool_call_id: string | null;
  tool_name: string;
  /** JSON of the arguments, with secrets removed by the plugin. */
  args_redacted: string;
  decision: "allow" | "deny" | "ask" | "inherit" | "invalid_rules";
  rule_id: string | null;
  status: "ok" | "error" | "blocked" | null;
  error_type: string | null;
  duration_ms: number | null;
  approval_choice: string | null;
  /** "aux_llm" when Hermes's smart mode decided instead of a person. */
  approval_decided_by: string | null;
}

export const auditForSession = (sessionId: string) =>
  call<AuditEvent[]>("audit_for_session", { sessionId });

export function onRunEvent(handler: (payload: RunEventPayload) => void): Promise<UnlistenFn> {
  return listen<RunEventPayload>("run-event", (event) => handler(event.payload));
}

export function onRunStreamEnd(handler: (payload: RunStreamEnd) => void): Promise<UnlistenFn> {
  return listen<RunStreamEnd>("run-stream-end", (event) => handler(event.payload));
}

export type RulesValidation =
  | { result: "valid" }
  | { result: "invalid"; reason: string }
  | { result: "unavailable"; reason: string };

export interface RulesState {
  path: string;
  exists: boolean;
  text: string;
  validation: RulesValidation;
}

export const rulesState = () => call<RulesState>("rules_state");
export const validateRules = (text: string) => call<RulesValidation>("validate_rules", { text });
export const saveRules = (text: string) => call<RulesState>("save_rules", { text });
export const resetRules = () => call<RulesState>("reset_rules");

// --- System page (M3 §5). Shapes mirror src-tauri/src/system/. -------------------------------

export interface WifiInfo {
  ssid: string;
  signal_percent: number | null;
}

export interface GpuInfo {
  name: string;
  load_percent: number | null;
  memory_used: number | null;
  memory_total: number | null;
  temperature_c: number | null;
}

export interface DiskReading {
  mount: string;
  name: string;
  kind: "ssd" | "hdd" | "unknown";
  removable: boolean;
  total: number;
  available: number;
  read_per_s: number | null;
  write_per_s: number | null;
}

export interface BatteryReading {
  percent: number;
  state: "charging" | "discharging" | "full" | "unknown";
  time_to_empty_s: number | null;
  time_to_full_s: number | null;
  health_percent: number | null;
}

export interface ProcessGroup {
  name: string;
  count: number;
  /** Share of the whole CPU, as in Task Manager. */
  cpu_percent: number;
  memory: number;
}

export interface SystemSnapshot {
  /** Unix milliseconds. */
  taken_at: number;
  cpu: { name: string; frequency_mhz: number; total: number | null; per_core: number[] };
  memory: { used: number; total: number; swap_used: number; swap_total: number };
  disks: DiskReading[];
  network: { down_per_s: number | null; up_per_s: number | null };
  wifi: WifiInfo | null;
  gpus: GpuInfo[];
  battery: BatteryReading | null;
  uptime_s: number;
  top_cpu: ProcessGroup[];
  top_memory: ProcessGroup[];
  notes: string[];
}

export const systemSnapshot = () => call<SystemSnapshot | null>("system_snapshot");
export const setSystemPageVisible = (visible: boolean) =>
  call<void>("set_system_page_visible", { visible });

export function onSystemSnapshot(handler: (snapshot: SystemSnapshot) => void): Promise<UnlistenFn> {
  return listen<SystemSnapshot>("system-snapshot", (event) => handler(event.payload));
}

export interface SpeedTestResult {
  latency_ms: number;
  download_mbps: number;
  upload_mbps: number;
  /** Unix milliseconds. */
  finished_at: number;
}

export type SpeedTestProgress =
  | { phase: "latency" }
  | { phase: "download"; latency_ms: number }
  | { phase: "upload"; latency_ms: number; download_mbps: number }
  | { phase: "done"; result: SpeedTestResult }
  | { phase: "failed"; message: string };

export const runSpeedTest = () => call<SpeedTestResult>("run_speed_test");

export function onSpeedTest(handler: (progress: SpeedTestProgress) => void): Promise<UnlistenFn> {
  return listen<SpeedTestProgress>("speed-test", (event) => handler(event.payload));
}

export interface SystemAlert {
  /** Stable for the episode: "battery", "disk:C:\\", "cpu:chrome". */
  id: string;
  kind: "battery_low" | "battery_critical" | "disk_full" | "cpu_hog";
  title: string;
  message: string;
  process: string | null;
  cpu_percent: number | null;
}

export interface AlertEvaluation {
  fired: SystemAlert[];
  cleared: string[];
}

export const alertsEnabled = () => call<boolean>("alerts_enabled");
export const setAlertsEnabled = (enabled: boolean) =>
  call<boolean>("set_alerts_enabled", { enabled });

export function onSystemAlerts(
  handler: (evaluation: AlertEvaluation) => void,
): Promise<UnlistenFn> {
  return listen<AlertEvaluation>("system-alerts", (event) => handler(event.payload));
}

// --- Starting with Windows (M3 §8) ------------------------------------------------------------

export interface PresenceEvent {
  /** Unix seconds. */
  at: number;
  kind: "startup" | "resume" | "unlock";
  detail: string | null;
  /** Seconds away, when known (M5); absent on events read back from kairos.db. */
  away_s?: number | null;
}

export interface StartupState {
  kairos_at_login: boolean | null;
  hermes_at_login: { scheduled_task: boolean; startup_entry: boolean } | null;
  launched_at_login: boolean;
  /** Kairos starts Hermes itself when it isn't running (D-035). */
  start_hermes: boolean;
  recent: PresenceEvent[];
}

export const startupState = () => call<StartupState>("startup_state");
export const setKairosAtLogin = (enabled: boolean) =>
  call<boolean>("set_kairos_at_login", { enabled });

/** The tray menu's "System" item asks the app to show a view. */
export function onNavigate(handler: (view: string) => void): Promise<UnlistenFn> {
  return listen<string>("navigate", (event) => handler(event.payload));
}

export function onPresence(handler: (event: PresenceEvent) => void): Promise<UnlistenFn> {
  return listen<PresenceEvent>("presence", (event) => handler(event.payload));
}

// --- Notifications (M4 §6–§7). Shapes mirror src-tauri/src/notifications/. --------------------

export interface StoredNotification {
  id: number;
  app_id: string;
  app_name: string;
  /** Unix seconds. */
  created_at: number;
  title: string;
  body: string;
  read_at: number | null;
}

export interface AppCount {
  app_id: string;
  app_name: string;
  total: number;
  unread: number;
}

export type NotificationAccess = "allowed" | "denied" | "unspecified" | "unavailable";
export type CollectState = "off" | "waiting_for_windows" | "unavailable" | "on";

export interface NotificationsState {
  enabled: boolean;
  access: NotificationAccess;
  state: CollectState;
  unread: number;
  counts: AppCount[];
}

export const notificationsState = () => call<NotificationsState>("notifications_state");
export const setNotificationsEnabled = (enabled: boolean) =>
  call<NotificationsState>("set_notifications_enabled", { enabled });
export const listNotifications = (appId: string | null, unreadOnly: boolean) =>
  call<StoredNotification[]>("list_notifications", { appId, unreadOnly });
export const markNotificationsRead = (id: number | null, appId: string | null) =>
  call<number>("mark_notifications_read", { id, appId });
export const clearNotifications = (appId: string | null) =>
  call<number>("clear_notifications", { appId });

export function onNotificationsChanged(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("notifications-changed", () => handler());
}

// --- Welcome-back briefing (M5 §2). Shapes mirror src-tauri/src/briefing/. ---------------------

export interface BriefingFact {
  kind: "away" | "battery" | "network" | "disk" | "notifications" | "tasks";
  tone: "info" | "warn";
  text: string;
}

export interface Briefing {
  /** Unix seconds. */
  at: number;
  reason: "first_of_day" | "away" | "asked";
  away_s: number | null;
  facts: BriefingFact[];
}

export interface BriefingNotice {
  briefing: Briefing;
  ai_summary: boolean;
  projects: string[];
}

export interface BriefingSettings {
  enabled: boolean;
  min_away_min: number;
  /** Minutes after local midnight; null = no quiet hours. */
  quiet_start: number | null;
  quiet_end: number | null;
  ai_summary: boolean;
  projects: string[];
}

export const setUtcOffset = (minutes: number) => call<void>("set_utc_offset", { minutes });
export const briefingSettings = () => call<BriefingSettings>("briefing_settings");
export const saveBriefingSettings = (settings: BriefingSettings) =>
  call<BriefingSettings>("save_briefing_settings", { settings });
export const latestBriefing = () => call<Briefing | null>("latest_briefing");
export const briefNow = () => call<Briefing | null>("brief_now");

export function onBriefing(handler: (notice: BriefingNotice) => void): Promise<UnlistenFn> {
  return listen<BriefingNotice>("briefing", (event) => handler(event.payload));
}

// --- Accounts (M6 §3). Shapes mirror src-tauri/src/accounts.rs. -------------------------------

export type AccountKind = "google" | "github";

export type AccountState =
  | { kind: AccountKind; status: "connected" }
  | { kind: AccountKind; status: "not_connected" }
  | { kind: AccountKind; status: "unavailable"; reason: string };

export const accountsState = () => call<AccountState[]>("accounts_state");
export const connectAccount = (kind: AccountKind) => call<void>("connect_account", { kind });
export const disconnectAccount = (kind: AccountKind) =>
  call<AccountState[]>("disconnect_account", { kind });

// --- Models and sandbox (M7). ------------------------------------------------------------------

export const terminalBackend = () => call<string | null>("terminal_backend");

export interface Price {
  /** Dollars per million tokens. */
  input: number;
  output: number;
  cache: number | null;
}

export interface ProviderModels {
  slug: string;
  name: string;
  is_current: boolean;
  models: { id: string; price: Price | null }[];
}

export interface Catalog {
  current_provider: string | null;
  current_model: string | null;
  providers: ProviderModels[];
}

export const modelCatalog = () => call<Catalog>("model_catalog");
export const conversationModel = (sessionId: string) =>
  call<ModelChoice>("conversation_model", { sessionId });

/** A request's usage once stored (M7), sent by the core as `task-usage`. */
export interface TaskUsage {
  task_id: string;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cost_usd: number | null;
  served_provider: string | null;
  served_model: string | null;
}

export interface CostCapHit {
  task_id: string;
  cap: number;
  cost: number;
}

export const costCap = () => call<number | null>("cost_cap");
export const setCostCap = (cap: number | null) => call<number | null>("set_cost_cap", { cap });

export function onTaskUsage(handler: (usage: TaskUsage) => void): Promise<UnlistenFn> {
  return listen<TaskUsage>("task-usage", (event) => handler(event.payload));
}

export function onCostCap(handler: (hit: CostCapHit) => void): Promise<UnlistenFn> {
  return listen<CostCapHit>("cost-cap", (event) => handler(event.payload));
}

// --- Quick box (M8 §3). -----------------------------------------------------------------------

export const quickHotkey = () => call<string>("quick_hotkey");
export const setQuickHotkey = (hotkey: string) => call<string>("set_quick_hotkey", { hotkey });
export const hideQuick = () => call<void>("hide_quick");
export const showQuick = () => call<void>("show_quick");
export const openInMain = (sessionId: string) => call<void>("open_in_main", { sessionId });

export function onQuickShown(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("quick-shown", () => handler());
}

export function onOpenConversation(handler: (sessionId: string) => void): Promise<UnlistenFn> {
  return listen<string>("open-conversation", (event) => handler(event.payload));
}

// --- Scheduled jobs (M8 §5). Shapes mirror src-tauri/src/hermes/jobs.rs. ------------------------

export interface Job {
  id: string;
  name: string;
  prompt: string;
  /** Hermes's own wording: "every monday 9am", "0 9 * * *". */
  schedule_display: string;
  enabled: boolean;
  /** "scheduled" | "paused" | "completed" | "error" */
  state: string;
  next_run_at: string | null;
  last_run_at: string | null;
  /** "ok" | "error" | "delivery_failed" | … ; null before the first run. */
  last_status: string | null;
  last_error: string | null;
}

export type JobAction = "pause" | "resume" | "run";

export const listJobs = () => call<Job[]>("list_jobs");
export const createJob = (name: string, schedule: string, prompt: string) =>
  call<Job>("create_job", { name, schedule, prompt });
export const jobAction = (jobId: string, action: JobAction) =>
  call<Job>("job_action", { jobId, action });
export const deleteJob = (jobId: string) => call<null>("delete_job", { jobId });

// --- Voice (M9, D-022). Shapes mirror src-tauri/src/voice and the voice commands. ---------------

export type VoiceModelId = "small" | "base";

export interface VoiceModelInfo {
  id: VoiceModelId;
  label: string;
  size_bytes: number;
  ready: boolean;
}

export interface VoiceSettings {
  models: VoiceModelInfo[];
  model: VoiceModelId;
  /** "en" | "fr" | "ar"; null = detect per recording. */
  language: string | null;
  read_aloud: boolean;
  /** Answers to spoken requests are read aloud. */
  answer_spoken: boolean;
  ptt_hotkey: string;
  downloading: boolean;
  /** The natural voice (Kokoro, M9 Step 8). */
  natural_installed: boolean;
  natural_installing: boolean;
  natural_size_bytes: number;
  /** What answers are read with now. */
  engine: "natural" | "windows";
  speaker: Speaker;
  /** The wake word "Kairos" (M9 Step 9). */
  wake_installed: boolean;
  wake_installing: boolean;
  wake_enabled: boolean;
  wake_sensitivity: WakeSensitivity;
  wake_size_bytes: number;
}

export type WakeSensitivity = "strict" | "normal" | "eager";

export type Speaker =
  | "heart"
  | "bella"
  | "nicole"
  | "sarah"
  | "sky"
  | "adam"
  | "michael"
  | "emma"
  | "isabella"
  | "george"
  | "lewis";

export type VoiceInstallProgress =
  | { phase: "downloading"; received: number; total: number; what?: "wake" }
  | { phase: "done"; what?: "wake" }
  | { phase: "failed"; message: string; what?: "wake" };

export type VoiceModelProgress =
  | { phase: "downloading"; id: VoiceModelId; received: number; total: number }
  | { phase: "done"; id: VoiceModelId }
  | { phase: "failed"; id: VoiceModelId; message: string };

export interface Transcript {
  text: string;
  language: string | null;
  elapsed_ms: number;
}

export const voiceState = () => call<VoiceSettings>("voice_state");
export const saveVoiceSettings = (
  model: VoiceModelId,
  language: string | null,
  readAloud: boolean,
  answerSpoken: boolean,
) => call<VoiceSettings>("save_voice_settings", { model, language, readAloud, answerSpoken });
export const downloadVoiceModel = (id: VoiceModelId) => call<null>("download_voice_model", { id });
export const deleteVoiceModel = (id: VoiceModelId) =>
  call<VoiceSettings>("delete_voice_model", { id });
export const pttHotkey = () => call<string>("ptt_hotkey");
export const setPttHotkey = (hotkey: string) => call<string>("set_ptt_hotkey", { hotkey });

/** The raw 16 kHz mono 16-bit samples go as the request body, not as JSON (M9 §3). */
export async function transcribe(pcm: Int16Array, language: string | null): Promise<Transcript> {
  try {
    return await invoke<Transcript>(
      "transcribe",
      new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength),
      {
        headers: language ? { language } : {},
      },
    );
  } catch (err) {
    throw asCommandError(err);
  }
}

export function onVoiceModel(handler: (progress: VoiceModelProgress) => void): Promise<UnlistenFn> {
  return listen<VoiceModelProgress>("voice-model", (event) => handler(event.payload));
}

/** Push-to-talk key held (true) or let go (false), sent to the quick box. */
export function onPushToTalk(handler: (held: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("push-to-talk", (event) => handler(event.payload));
}

// --- Persona (M10, D-023). Shapes mirror src-tauri/src/persona.rs. ------------------------------

export type PersonaKind = "kairos" | "hermes_default" | "custom";

export interface PersonaState {
  /** What Hermes's SOUL.md holds now; empty when there is none. */
  text: string;
  kind: PersonaKind;
  kairos_text: string;
  has_backup: boolean;
}

export const personaState = () => call<PersonaState>("persona_state");
export const savePersona = (text: string) => call<PersonaState>("save_persona", { text });
export const restorePersona = () => call<PersonaState>("restore_persona");

export const saveSpeechSettings = (natural: boolean, speaker: Speaker) =>
  call<VoiceSettings>("save_speech_settings", { natural, speaker });
export const installNaturalVoice = () => call<null>("install_natural_voice");
export const removeNaturalVoice = () => call<VoiceSettings>("remove_natural_voice");
export const warmNaturalVoice = () => call<null>("warm_natural_voice");

/** One sentence in the natural voice, as WAV bytes. */
export async function speakNatural(text: string): Promise<ArrayBuffer> {
  try {
    return await invoke<ArrayBuffer>("speak_natural", { text });
  } catch (err) {
    throw asCommandError(err);
  }
}

export function onVoiceInstall(
  handler: (progress: VoiceInstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<VoiceInstallProgress>("voice-install", (event) => handler(event.payload));
}

// --- Memory (M11, D-026). Shapes mirror src-tauri/src/memory.rs. -------------------------------

export interface MemoryFile {
  entries: string[];
  /** Characters used, as Hermes counts them. */
  used: number;
  limit: number;
}

export interface Fact {
  id: number;
  content: string;
  category: string;
  /** 0–1. */
  trust: number;
  updated_at: string;
}

export interface MemoryState {
  user: MemoryFile;
  notes: MemoryFile;
  /** null when there is no fact store yet. */
  facts: Fact[] | null;
  provider: string | null;
}

export const memoryState = () => call<MemoryState>("memory_state");

// --- Wake word (M9 Step 9) ---------------------------------------------------------------------

export const saveWakeSettings = (enabled: boolean, sensitivity: WakeSensitivity) =>
  call<VoiceSettings>("save_wake_settings", { enabled, sensitivity });
export const installWakeWord = () => call<null>("install_wake_word");

/** A chunk of the always-on microphone, raw 16 kHz 16-bit; true when "Kairos" was heard. */
export async function wakeFeed(pcm: Int16Array): Promise<boolean> {
  try {
    return await invoke<boolean>(
      "wake_feed",
      new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength),
    );
  } catch (err) {
    throw asCommandError(err);
  }
}

/** A wake: `spoken` when words already followed the name ("Kairos, open…"). */
export interface Woken {
  spoken: boolean;
}

/** Sent to the quick box when Whisper confirmed the name. */
export function onWake(handler: (woken: Woken) => void): Promise<UnlistenFn> {
  return listen<Woken>("wake", (event) => handler(event.payload));
}

/**
 * The request after a wake, heard from the start of the sentence that held the name (Rust keeps
 * it), without the name.
 */
export const wakeTranscribe = (language: string | null) =>
  call<Transcript>("wake_transcribe", { language });
/** The quick box stopped listening after a wake without a request. */
export const wakeCancel = () => call<null>("wake_cancel");

// --- Pictures in answers (M13 Step 4) -----------------------------------------------------------

/** A `MEDIA:` picture from history; Rust reads it only inside Hermes's image folders. */
export async function mediaImage(path: string): Promise<ArrayBuffer> {
  try {
    return await invoke<ArrayBuffer>("media_image", { path });
  } catch (err) {
    throw asCommandError(err);
  }
}

/** Saves a picture to Downloads; returns where it went. */
export async function saveImage(bytes: Uint8Array, name: string): Promise<string> {
  try {
    return await invoke<string>("save_image", bytes, { headers: { name } });
  } catch (err) {
    throw asCommandError(err);
  }
}

// --- The island (M14, D-029) ---------------------------------------------------------------------

export interface IslandSettings {
  enabled: boolean;
  /** The key that hides and shows it ("Ctrl+Alt+L"). */
  hotkey: string;
  panel: boolean;
  /** Hidden while a window is maximized, until the mouse touches the top (D-034). */
  autohide: boolean;
}

export const islandState = () => call<IslandSettings>("island_state");
export const saveIslandSettings = (enabled: boolean, panel: boolean, autohide: boolean) =>
  call<IslandSettings>("save_island_settings", { enabled, panel, autohide });

/** The island window was shown again (from the top edge, or after a maximized window). */
export function onIslandShown(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("island-shown", () => handler());
}
/** The bar is about to slide up and hide (D-036). */
export function onIslandLeaving(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("island-leaving", () => handler());
}

// --- The top bar's modules (D-036). Shapes mirror src-tauri/src/desk.rs. ----------------------

export interface Desktops {
  count: number;
  /** 0-based. */
  current: number;
}

export interface Volume {
  /** 0–100. */
  level: number;
  muted: boolean;
}

export interface BarState {
  desktops: Desktops;
  volume: Volume | null;
  /** The Wi-Fi network in use and its bars (0–4). */
  wifi: [string, number] | null;
}

// --- The bar's own Wi-Fi and Bluetooth menus (D-037). Shapes mirror src-tauri/src/net.rs. -----

export interface WifiNetwork {
  ssid: string;
  bars: number;
  secured: boolean;
  saved: boolean;
  connected: boolean;
}

export interface WifiState {
  adapter: boolean;
  on: boolean | null;
  connected: [string, number] | null;
  saved: string[];
  /** Null while Windows hides nearby networks (location off). */
  nearby: WifiNetwork[] | null;
}

export interface BluetoothDevice {
  name: string;
  connected: boolean;
}

export interface BluetoothState {
  adapter: boolean;
  on: boolean | null;
  devices: BluetoothDevice[];
}

export const wifiState = (scan: boolean) => call<WifiState>("wifi_state", { scan });
export const wifiSet = (on: boolean) => call<WifiState>("wifi_set", { on });
/** The password goes to Windows's profile; Kairos keeps nothing. */
export const wifiConnect = (ssid: string, password: string | null) =>
  call<null>("wifi_connect", { ssid, password });
export const wifiDisconnect = () => call<null>("wifi_disconnect");
export const bluetoothState = () => call<BluetoothState>("bluetooth_state");
export const bluetoothSet = (on: boolean) => call<BluetoothState>("bluetooth_set", { on });

/** The Windows panels the modules open; names only (`desk::panel_uri`). */
export type WindowsPanel =
  "wifi" | "bluetooth" | "sound" | "mixer" | "power" | "battery" | "date" | "location";

export const barState = () => call<BarState>("bar_state");
export const barSwitchDesktop = (index: number) => call<Desktops>("bar_switch_desktop", { index });
export const barSetVolume = (level: number) => call<Volume>("bar_set_volume", { level });
export const barSetMuted = (muted: boolean) => call<Volume>("bar_set_muted", { muted });
export const barOpen = (which: WindowsPanel) => call<null>("bar_open", { which });
export function onBarState(handler: (state: BarState) => void): Promise<UnlistenFn> {
  return listen<BarState>("bar-state", (event) => handler(event.payload));
}
export const islandOpenQuick = () => call<null>("island_open_quick");
export const setIslandHotkey = (hotkey: string) =>
  call<IslandSettings>("set_island_hotkey", { hotkey });
/** The hotkey hid or showed the island. */
export function onIslandSettings(handler: (settings: IslandSettings) => void): Promise<UnlistenFn> {
  return listen<IslandSettings>("island-settings", (event) => handler(event.payload));
}
/** The pill expands into its panel, or folds back (M15). */
export const islandExpand = (expanded: boolean) => call<null>("island_expand", { expanded });
/** The width the top bar needs (CSS px); the window fits it (D-034). */
export const islandFit = (width: number) => call<null>("island_fit", { width });
/** Sent by Rust when the island expanded or folded. */
export function onIslandExpanded(handler: (expanded: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("island-expanded", (event) => handler(event.payload));
}
export const islandPushToTalk = (held: boolean) => call<null>("island_push_to_talk", { held });
export const islandSnooze = (minutes: number) => call<null>("island_snooze", { minutes });
export const islandCapturing = (capturing: boolean) =>
  call<null>("island_capturing", { capturing });
export const hidePresencePanel = () => call<null>("hide_presence_panel");

/** The main window and the quick box tell the island what they are doing. */
export const emitKairosState = (state: KairosState) => emit("kairos-state", state);
export function onKairosState(handler: (state: KairosState) => void): Promise<UnlistenFn> {
  return listen<KairosState>("kairos-state", (event) => handler(event.payload));
}

/** The wake panel was placed and shown (it then slides in). */
export function onPanelShown(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("panel-shown", () => handler());
}
/** The main window, optionally on a page ("settings:island"). */
export const showMainWindow = (view?: string) =>
  call<null>("show_main_window", { view: view ?? null });

// --- Automations (M17). Shapes mirror src-tauri/src/automations/. -------------------------------

export type Trigger =
  | { kind: "login" }
  | { kind: "back"; away_min: number }
  | { kind: "battery_low"; below: number }
  | { kind: "charger"; plugged: boolean }
  | { kind: "wifi_joined"; network: string }
  | { kind: "wifi_lost" }
  | { kind: "disk_full" }
  | { kind: "crash"; programs: string[] };

export interface Conditions {
  from_min: number | null;
  to_min: number | null;
  days: number[];
  power: "battery" | "plugged" | null;
  during_quiet_hours: boolean;
}

export type AutomationAction =
  | { kind: "notify"; text: string }
  | { kind: "say"; text: string }
  | { kind: "island"; text: string }
  | { kind: "open_app"; app_id: string; name: string }
  | { kind: "offer"; text: string }
  | { kind: "ask"; text: string };

export interface Automation {
  id: string;
  name: string;
  enabled: boolean;
  trigger: Trigger;
  only_if: Conditions;
  actions: AutomationAction[];
  cooldown_min: number;
}

export type RunOutcome = "done" | "waiting" | "skipped" | "failed";

export interface AutomationRun {
  id: number;
  automation_id: string;
  name: string;
  at: number;
  event: string;
  outcome: RunOutcome;
  reason: string | null;
  asked: boolean;
  test: boolean;
}

export interface AutomationsState {
  enabled: boolean;
  automations: Automation[];
  runs: AutomationRun[];
  crash_ignore: string[];
}

/** What the main window does for a run: voice, island, offers and "Ask Kairos". */
export interface AutomationFired {
  run_id: number;
  automation_id: string;
  name: string;
  event: string;
  test: boolean;
  actions: AutomationAction[];
}

export const automationsState = () => call<AutomationsState>("automations_state");
export const saveAutomation = (automation: Automation) =>
  call<AutomationsState>("save_automation", { automation });
export const deleteAutomation = (id: string) => call<AutomationsState>("delete_automation", { id });
export const setAutomationsEnabled = (enabled: boolean) =>
  call<AutomationsState>("set_automations_enabled", { enabled });
export const setCrashIgnore = (names: string[]) =>
  call<AutomationsState>("set_crash_ignore", { names });
export const testAutomation = (automation: Automation) =>
  call<null>("test_automation", { automation });
export const automationOutcome = (
  runId: number,
  outcome: RunOutcome,
  reason: string | null,
  asked: boolean,
) => call<null>("automation_outcome", { runId, outcome, reason, asked });

export function onAutomationFired(handler: (fired: AutomationFired) => void): Promise<UnlistenFn> {
  return listen<AutomationFired>("automation-fired", (event) => handler(event.payload));
}

export function onAutomationsChanged(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("automations-changed", () => handler());
}
