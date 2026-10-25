// Typed wrappers around Tauri `invoke` for the commands in M1 §8.
// Shapes mirror the Rust types in src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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

// Tauri maps camelCase argument names here to the snake_case Rust parameters.
export const createRun = (sessionId: string | null, input: string) =>
  call<TaskRef>("create_run", { sessionId, input });
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
}

export interface StartupState {
  kairos_at_login: boolean | null;
  hermes_at_login: { scheduled_task: boolean; startup_entry: boolean } | null;
  launched_at_login: boolean;
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
