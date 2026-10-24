// One request and everything Hermes sent back for it, built up event by event. Pure, so it can
// be tested against the recorded streams in fixtures/.
import type { RunEventEnvelope } from "../../lib/hermes-events";
import type { ApprovalChoice, AuditEvent, StoredTask } from "../../lib/tauri";

export interface ToolRow {
  kind: "tool";
  toolCallId: string | null;
  tool: string;
  preview: string;
  state: "running" | "ok" | "error";
  /** What the Kairos plugin decided for this call (audit.db), once it has been read. */
  audit?: AuditEvent;
}

export interface ApprovalRow {
  kind: "approval";
  promptId: string;
  executionGeneration: number;
  command: string;
  description: string;
  choices: string[];
  /** Hermes still lists it as pending, so it can be answered. Replayed requests are not live. */
  live: boolean;
  state: "waiting" | "answering" | "settled";
  /** What the user chose here; Hermes's approval.settled doesn't say. */
  choice: ApprovalChoice | null;
}

export type TurnItem = ToolRow | ApprovalRow;

export type TurnStatus =
  | "sending"
  | "running"
  | "waiting_for_approval"
  | "stopping"
  | "completed"
  | "incomplete"
  | "cancelled"
  | "failed"
  | "interrupted"
  | "unknown"
  | "lost"
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
  /** Rebuilt from kairos.db: only the request and its outcome are known, not the tool calls. */
  fromHistory: boolean;
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
    fromHistory: false,
  };
}

const STORED_STATUS: Record<StoredTask["status"], TurnStatus> = {
  created: "sending",
  running: "running",
  waiting_for_approval: "waiting_for_approval",
  stopping: "stopping",
  completed: "completed",
  incomplete: "incomplete",
  failed: "failed",
  cancelled: "cancelled",
  interrupted: "interrupted",
  unknown: "unknown",
  lost: "lost",
};

/** A turn for a task stored in kairos.db, as shown when a conversation is reopened. */
export function turnFromTask(task: StoredTask): Turn {
  const status = STORED_STATUS[task.status];
  const failed = ["failed", "interrupted", "unknown", "lost"].includes(status);
  return {
    ...newTurn(task.goal),
    taskId: task.id,
    runId: task.run_id,
    status,
    text: failed ? "" : (task.outcome ?? ""),
    error: failed ? task.outcome : null,
    fromHistory: true,
  };
}

export function isFinished(status: TurnStatus): boolean {
  return !["sending", "running", "waiting_for_approval", "stopping", "unknown"].includes(status);
}

/** The approval the user can answer now, if any. */
export function liveApproval(turn: Turn): ApprovalRow | null {
  for (const item of turn.items) {
    if (item.kind === "approval" && item.live && item.state !== "settled") return item;
  }
  return null;
}

/** How a tool row should be labelled, given what happened around it (M1 §9 rules 4–5). */
export function toolLabel(
  turn: Turn,
  row: ToolRow,
): "running" | "done" | "failed" | "blocked" | "stopped" {
  if (row.state === "running") return turn.status === "stopping" ? "stopped" : "running";
  if (row.state === "ok") return "done";
  const audit = row.audit;
  if (audit && (audit.decision === "deny" || audit.decision === "invalid_rules")) return "blocked";
  if (audit?.approval_choice === "deny" || audit?.status === "blocked") return "blocked";
  const denied = turn.items.some((item) => item.kind === "approval" && item.choice === "deny");
  if (denied) return "blocked";
  if (turn.status === "cancelled" || turn.status === "stopping") return "stopped";
  return "failed";
}

const CHOICE_TEXT: Record<string, string> = {
  once: "you allowed it once",
  session: "you allowed it for this conversation",
  always: "you always allow it",
  deny: "you denied it",
  timeout: "nobody answered in time",
  cancelled: "the request ended first",
};

/**
 * The Kairos rule's part in a tool call, for its row: "blocked by no-credentials",
 * "asked: destructive-commands, you denied it". Null when no Kairos rule matched.
 */
export function decisionText(row: ToolRow): string | null {
  const audit = row.audit;
  if (!audit) return null;
  const rule = audit.rule_id ?? "a Kairos rule";
  const choice = audit.approval_choice
    ? audit.approval_decided_by === "aux_llm"
      ? `Hermes's helper model ${audit.approval_choice === "deny" ? "denied" : "allowed"} it`
      : (CHOICE_TEXT[audit.approval_choice] ?? audit.approval_choice)
    : null;
  switch (audit.decision) {
    case "deny":
      return `blocked by ${rule}`;
    case "invalid_rules":
      return "blocked: Kairos rules are invalid";
    case "ask":
      return choice ? `asked by ${rule}, ${choice}` : `needs approval: ${rule}`;
    case "allow":
      return `allowed by ${rule}`;
    case "inherit":
      // No Kairos rule matched; Hermes's own approval may still have asked.
      return choice ? `Hermes asked, ${choice}` : null;
  }
}

/** Joins audit events onto the turns' tool rows by tool call id. */
export function attachAudit(turns: Turn[], events: AuditEvent[]): Turn[] {
  const byCall = new Map(events.filter((e) => e.tool_call_id).map((e) => [e.tool_call_id, e]));
  return turns.map((turn) => {
    let changed = false;
    const items = turn.items.map((item) => {
      if (item.kind !== "tool" || !item.toolCallId) return item;
      const audit = byCall.get(item.toolCallId);
      if (!audit || audit === item.audit) return item;
      changed = true;
      return { ...item, audit };
    });
    return changed ? { ...turn, items } : turn;
  });
}

function previewOf(event: AuditEvent): string {
  try {
    const args = JSON.parse(event.args_redacted) as Record<string, unknown>;
    for (const key of ["command", "code", "path", "file_path", "url"]) {
      if (typeof args[key] === "string") return args[key] as string;
    }
    if (Array.isArray(args.urls)) return args.urls.join(", ");
  } catch {
    // Not JSON: show nothing rather than raw text.
  }
  return "";
}

/** A tool row rebuilt from the audit log, for a conversation reopened from history. */
export function toolRowFromAudit(event: AuditEvent): ToolRow {
  return {
    kind: "tool",
    toolCallId: event.tool_call_id,
    tool: event.tool_name,
    preview: previewOf(event),
    state: event.status === null ? "running" : event.status === "ok" ? "ok" : "error",
    audit: event,
  };
}

/**
 * Turns for a reopened conversation: each stored task, with the audited tool calls that happened
 * between its start and the next task's start (kairos.db keeps only requests and answers).
 */
export function turnsFromHistory(tasks: StoredTask[], events: AuditEvent[]): Turn[] {
  return tasks.map((task, i) => {
    const start = task.created_at;
    const end = tasks[i + 1]?.created_at ?? Infinity;
    const rows = events.filter((e) => e.at >= start && e.at < end).map(toolRowFromAudit);
    return { ...turnFromTask(task), items: rows };
  });
}

export function blockedCount(turn: Turn): number {
  return turn.items.filter((item) => item.kind === "tool" && toolLabel(turn, item) === "blocked")
    .length;
}

function errorText(error: unknown): string {
  if (error === null || error === undefined) return "Hermes reported a failure.";
  if (typeof error === "string") return error;
  if (typeof error === "object" && "message" in error) return String(error.message);
  return JSON.stringify(error);
}

/**
 * Applies one event and returns the new turn; events already applied are ignored.
 * `approvalLive` comes from Rust with every approval.request (M1 §9 rule 2).
 */
export function applyEvent(turn: Turn, envelope: RunEventEnvelope, approvalLive: boolean): Turn {
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
        executionGeneration: event.execution_generation,
        command: event.command,
        description: event.description,
        choices: event.choices,
        live: approvalLive,
        state: "waiting",
        choice: null,
      });
      if (approvalLive) next.status = "waiting_for_approval";
      break;
    case "approval.settled":
      next.items = next.items.map((item) =>
        item.kind === "approval" && item.promptId === event.prompt_id
          ? { ...item, state: "settled" }
          : item,
      );
      if (next.status === "waiting_for_approval") next.status = "running";
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

/** Records the user's answer on the approval row before Hermes confirms it. */
export function markAnswered(turn: Turn, promptId: string, choice: ApprovalChoice): Turn {
  return {
    ...turn,
    items: turn.items.map((item) =>
      item.kind === "approval" && item.promptId === promptId
        ? { ...item, state: "answering", choice }
        : item,
    ),
  };
}
