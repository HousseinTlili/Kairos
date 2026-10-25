import { create } from "zustand";
import type { RunEventPayload, RunStreamEnd } from "../../lib/hermes-events";
import {
  answerApproval,
  auditForSession,
  conversationTasks,
  createRun,
  listConversations,
  onRunEvent,
  onRunStreamEnd,
  resumeOpenTasks,
  stopRun,
  subscribeRun,
  type ApprovalChoice,
  type CommandError,
  type ConversationList,
} from "../../lib/tauri";
import {
  applyEvent,
  attachAudit,
  isFinished,
  markAnswered,
  newTurn,
  turnsFromHistory,
  type Turn,
} from "./turn";
import { useModels } from "../models/store";

// Hermes writes audit rows from its own process a moment after the events arrive here, so the
// audit is re-read shortly after a tool call or run ends, not on every event.
const AUDIT_REFRESH_DELAY_MS = 400;
let auditTimer: ReturnType<typeof setTimeout> | null = null;

interface ChatState {
  /** Null for a new conversation until its first request creates a Kairos session. */
  sessionId: string | null;
  turns: Turn[];
  conversations: ConversationList;
  /** Set when an action fails outside any turn (loading a conversation, stopping). */
  notice: string | null;

  send: (input: string) => Promise<void>;
  newConversation: () => void;
  openConversation: (sessionId: string) => Promise<void>;
  refreshConversations: () => Promise<void>;
  /** Re-reads the Kairos plugin's audit of the open conversation onto its tool rows. */
  refreshAudit: () => Promise<void>;
  stop: (taskId: string) => Promise<void>;
  answer: (taskId: string, promptId: string, choice: ApprovalChoice) => Promise<void>;
  resume: () => Promise<void>;
  receive: (payload: RunEventPayload) => void;
  streamEnded: (payload: RunStreamEnd) => void;
}

function messageOf(err: unknown): string {
  return (err as CommandError)?.message ?? String(err);
}

export const useChat = create<ChatState>((set, get) => {
  /** Applies `change` to the turn of `taskId`, if it's in the open conversation. */
  const updateTask = (taskId: string, change: (turn: Turn) => Turn) => {
    const { turns } = get();
    const index = turns.findIndex((turn) => turn.taskId === taskId);
    if (index < 0) return;
    set({ turns: turns.map((turn, i) => (i === index ? change(turn) : turn)) });
  };

  const subscribeUnfinished = async (turns: Turn[]) => {
    for (const turn of turns) {
      if (!isFinished(turn.status) && turn.taskId && turn.runId) {
        await subscribeRun(turn.taskId, turn.runId).catch((err) =>
          set({ notice: `Couldn't follow a running request: ${messageOf(err)}` }),
        );
      }
    }
  };

  return {
    sessionId: null,
    turns: [],
    conversations: { kairos: [], other: [] },
    notice: null,

    async send(input) {
      const text = input.trim();
      if (!text) return;
      const index = get().turns.length;
      set({ turns: [...get().turns, newTurn(text)], notice: null });
      const patch = (change: (turn: Turn) => Turn) =>
        set({ turns: get().turns.map((turn, i) => (i === index ? change(turn) : turn)) });
      try {
        const task = await createRun(get().sessionId, text, useModels.getState().choice);
        set({ sessionId: task.session_id });
        patch((turn) => ({
          ...turn,
          taskId: task.task_id,
          runId: task.run_id,
          status: "running",
        }));
        void get().refreshConversations();
        await subscribeRun(task.task_id, task.run_id);
      } catch (err) {
        patch((turn) => ({ ...turn, status: "error", error: messageOf(err) }));
      }
    },

    newConversation() {
      set({ sessionId: null, turns: [], notice: null });
      void useModels.getState().loadFor(null);
    },

    async openConversation(sessionId) {
      try {
        const [tasks, events] = await Promise.all([
          conversationTasks(sessionId),
          // No audit (plugin not installed, file unreadable) still shows the conversation.
          auditForSession(sessionId).catch(() => []),
        ]);
        const turns = turnsFromHistory(tasks, events);
        set({ sessionId, turns, notice: null });
        void useModels.getState().loadFor(sessionId);
        await subscribeUnfinished(turns);
      } catch (err) {
        set({ notice: `Couldn't open that conversation: ${messageOf(err)}` });
      }
    },

    async refreshAudit() {
      const sessionId = get().sessionId;
      if (!sessionId) return;
      try {
        const events = await auditForSession(sessionId);
        if (get().sessionId === sessionId) set({ turns: attachAudit(get().turns, events) });
      } catch {
        // The timeline is extra detail; a failed read leaves the rows as they are.
      }
    },

    async refreshConversations() {
      try {
        set({ conversations: await listConversations() });
      } catch (err) {
        set({ notice: `Couldn't load conversations: ${messageOf(err)}` });
      }
    },

    async stop(taskId) {
      const turn = get().turns.find((t) => t.taskId === taskId);
      if (!turn?.runId) return;
      updateTask(taskId, (t) => ({ ...t, status: "stopping" }));
      try {
        await stopRun(taskId, turn.runId);
      } catch (err) {
        updateTask(taskId, (t) => ({ ...t, status: "running" }));
        set({ notice: `Couldn't stop: ${messageOf(err)}` });
      }
    },

    async answer(taskId, promptId, choice) {
      const turn = get().turns.find((t) => t.taskId === taskId);
      const row = turn?.items.find((i) => i.kind === "approval" && i.promptId === promptId);
      if (!turn?.runId || row?.kind !== "approval") return;
      updateTask(taskId, (t) => markAnswered(t, promptId, choice));
      try {
        await answerApproval(turn.runId, promptId, row.executionGeneration, choice);
      } catch (err) {
        const stale = (err as CommandError).kind === "stale";
        updateTask(taskId, (t) => ({
          ...t,
          items: t.items.map((item) =>
            item.kind === "approval" && item.promptId === promptId
              ? stale
                ? { ...item, live: false, state: "settled" }
                : { ...item, state: "waiting", choice: null }
              : item,
          ),
        }));
        set({
          notice: stale
            ? "That approval was already answered or is no longer current."
            : `Couldn't send your answer: ${messageOf(err)}`,
        });
      }
    },

    /** After start-up: settle what finished while Kairos was closed, follow what still runs. */
    async resume() {
      try {
        const open = await resumeOpenTasks();
        await get().refreshConversations();
        const latest = open[open.length - 1];
        if (latest && get().turns.length === 0) {
          await get().openConversation(latest.session_id);
        }
      } catch (err) {
        set({ notice: `Couldn't check earlier requests: ${messageOf(err)}` });
      }
    },

    receive({ task_id, envelope, approval_live }) {
      updateTask(task_id, (turn) => applyEvent(turn, envelope, approval_live === true));
      const name = envelope.event.event;
      if (name.startsWith("run.")) void get().refreshConversations();
      if (name === "tool.completed" || name === "approval.settled" || name.startsWith("run.")) {
        if (auditTimer) clearTimeout(auditTimer);
        auditTimer = setTimeout(() => void get().refreshAudit(), AUDIT_REFRESH_DELAY_MS);
      }
    },

    streamEnded({ task_id, error }) {
      updateTask(task_id, (turn) =>
        isFinished(turn.status)
          ? turn
          : {
              ...turn,
              status: "error",
              error: error
                ? `Lost the connection to this request: ${error.message}`
                : "The request's event stream ended without a result.",
            },
      );
    },
  };
});

/** Subscribes the store to the Rust run events. Call once, before any request is sent. */
export async function startChatEvents(): Promise<() => void> {
  const offEvent = await onRunEvent((payload) => useChat.getState().receive(payload));
  const offEnd = await onRunStreamEnd((payload) => useChat.getState().streamEnded(payload));
  return () => {
    offEvent();
    offEnd();
  };
}
