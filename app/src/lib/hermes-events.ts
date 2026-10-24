// TypeScript mirror of `RunEventEnvelope` / `RunEvent` in src-tauri/src/hermes/events.rs
// (M1 §7). Keep the two in step.

export interface Usage {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

export type RunEvent =
  | { event: "message.delta"; delta: string }
  | { event: "message.interim"; text: string; already_streamed: boolean }
  | { event: "tool.started"; tool: string; preview: string; tool_call_id: string | null }
  | {
      event: "tool.completed";
      tool: string;
      duration: number;
      error: boolean;
      tool_call_id: string | null;
      preview: string;
    }
  | {
      event: "approval.request";
      prompt_id: string;
      execution_generation: number;
      command: string;
      description: string;
      choices: string[];
    }
  | { event: "approval.settled"; prompt_id: string; execution_generation: number }
  | {
      event: "run.completed";
      output: string;
      usage: Usage;
      completed: boolean;
      partial: boolean;
      interrupted: boolean;
    }
  | { event: "run.cancelled"; turn_exit_reason: string | null }
  | { event: "run.failed"; error: unknown }
  | { event: "run.interrupted"; error: unknown }
  | { event: "subagent.start"; [key: string]: unknown }
  | { event: "subagent.complete"; [key: string]: unknown }
  | { event: "unknown"; name: string; raw: unknown };

export interface RunEventEnvelope {
  run_id: string;
  seq: number | null;
  timestamp: number | null;
  event: RunEvent;
}

/** The `run-event` Tauri event (commands.rs `RunEventPayload`). */
export interface RunEventPayload {
  task_id: string;
  envelope: RunEventEnvelope;
  /** For approval.request only: whether Hermes still lists it as pending (M1 §9 rule 2). */
  approval_live: boolean | null;
}

/** The `run-stream-end` Tauri event (commands.rs `RunStreamEnd`). */
export interface RunStreamEnd {
  task_id: string;
  error: { kind: string; message: string } | null;
}
