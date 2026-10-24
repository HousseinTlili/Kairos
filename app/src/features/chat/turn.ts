// One request and everything Hermes sent back for it, built up event by event. Pure, so it can
// be tested against the recorded streams in fixtures/.
import type { RunEventEnvelope } from "../../lib/hermes-events";

export interface ToolRow {
  kind: "tool";
  toolCallId: string | null;
  tool: string;
  preview: string;
  state: "running" | "ok" | "error";
}

export interface ApprovalRow {
  kind: "approval";
  promptId: string;
  command: string;
  description: string;
  state: "waiting" | "settled";
}

export type TurnItem = ToolRow | ApprovalRow;

export type TurnStatus =
  | "sending"
  | "running"
  | "waiting_for_approval"
  | "completed"
  | "incomplete"
  | "cancelled"
  | "failed"
  | "interrupted"
  | "error";

export interface Turn {
  taskId: string | null;
  runId: string | null;
  input: string;
  items: TurnItem[];
  /** Streamed text while running; the final `output` once completed (M1 §7). */
  text: string;
  status: TurnStatus;
  error: string | null;
  /** Highest `seq` applied. Re-subscribing replays from 0, so anything at or below is a repeat. */
  lastSeq: number;
}

export function newTurn(input: string): Turn {
  return {
    taskId: null,
    runId: null,
    input,
    items: [],
    text: "",
    status: "sending",
    error: null,
    lastSeq: -1,
  };
}

export function isFinished(status: TurnStatus): boolean {
  return !["sending", "running", "waiting_for_approval"].includes(status);
}

function errorText(error: unknown): string {
  if (error === null || error === undefined) return "Hermes reported a failure.";
  if (typeof error === "string") return error;
  if (typeof error === "object" && "message" in error) return String(error.message);
  return JSON.stringify(error);
}

/** Applies one event and returns the new turn; events already applied are ignored. */
export function applyEvent(turn: Turn, envelope: RunEventEnvelope): Turn {
  if (envelope.seq !== null && envelope.seq <= turn.lastSeq) return turn;
  const next: Turn = {
    ...turn,
    items: [...turn.items],
    lastSeq: envelope.seq ?? turn.lastSeq,
  };
  const event = envelope.event;

  switch (event.event) {
    case "message.delta":
      next.text += event.delta;
      break;
    case "tool.started":
      next.items.push({
        kind: "tool",
        toolCallId: event.tool_call_id,
        tool: event.tool,
        preview: event.preview,
        state: "running",
      });
      break;
    case "tool.completed": {
      const index = next.items.findIndex(
        (item) =>
          item.kind === "tool" &&
          item.state === "running" &&
          (event.tool_call_id ? item.toolCallId === event.tool_call_id : item.tool === event.tool),
      );
      if (index >= 0) {
        next.items[index] = {
          ...(next.items[index] as ToolRow),
          state: event.error ? "error" : "ok",
        };
      }
      break;
    }
    case "approval.request":
      next.items.push({
        kind: "approval",
        promptId: event.prompt_id,
        command: event.command,
        description: event.description,
        state: "waiting",
      });
      next.status = "waiting_for_approval";
      break;
    case "approval.settled":
      next.items = next.items.map((item) =>
        item.kind === "approval" && item.promptId === event.prompt_id
          ? { ...item, state: "settled" }
          : item,
      );
      next.status = "running";
      break;
    case "run.completed":
      // Streamed text after a tool call starts with "\n\n" that `output` doesn't have.
      next.text = event.output;
      next.status = event.completed && !event.partial ? "completed" : "incomplete";
      break;
    case "run.cancelled":
      next.status = "cancelled";
      break;
    case "run.failed":
      next.status = "failed";
      next.error = errorText(event.error);
      break;
    case "run.interrupted":
      next.status = "interrupted";
      next.error = errorText(event.error);
      break;
    default:
      // message.interim, subagent.*, unknown: nothing shown in M1.
      return { ...turn, lastSeq: next.lastSeq };
  }
  return next;
}
