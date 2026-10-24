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

export function onRunEvent(handler: (payload: RunEventPayload) => void): Promise<UnlistenFn> {
  return listen<RunEventPayload>("run-event", (event) => handler(event.payload));
}

export function onRunStreamEnd(handler: (payload: RunStreamEnd) => void): Promise<UnlistenFn> {
  return listen<RunStreamEnd>("run-stream-end", (event) => handler(event.payload));
}
