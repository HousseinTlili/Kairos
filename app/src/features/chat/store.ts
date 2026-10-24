import { create } from "zustand";
import type { RunEventPayload, RunStreamEnd } from "../../lib/hermes-events";
import {
  createRun,
  onRunEvent,
  onRunStreamEnd,
  subscribeRun,
  type CommandError,
} from "../../lib/tauri";
import { applyEvent, isFinished, newTurn, type Turn } from "./turn";

interface ChatState {
  /** Null until the first request of a new conversation creates a Kairos session. */
  sessionId: string | null;
  turns: Turn[];
  send: (input: string) => Promise<void>;
  newConversation: () => void;
  receive: (payload: RunEventPayload) => void;
  streamEnded: (payload: RunStreamEnd) => void;
}

function updateTurn(turns: Turn[], index: number, change: (turn: Turn) => Turn): Turn[] {
  return turns.map((turn, i) => (i === index ? change(turn) : turn));
}

export const useChat = create<ChatState>((set, get) => ({
  sessionId: null,
  turns: [],

  async send(input) {
    const text = input.trim();
    if (!text) return;
    const index = get().turns.length;
    set({ turns: [...get().turns, newTurn(text)] });
    try {
      const task = await createRun(get().sessionId, text);
      set({
        sessionId: task.session_id,
        turns: updateTurn(get().turns, index, (turn) => ({
          ...turn,
          taskId: task.task_id,
          runId: task.run_id,
          status: "running",
        })),
      });
      await subscribeRun(task.task_id, task.run_id);
    } catch (err) {
      const message = (err as CommandError).message ?? String(err);
      set({
        turns: updateTurn(get().turns, index, (turn) => ({
          ...turn,
          status: "error",
          error: message,
        })),
      });
    }
  },

  newConversation() {
    set({ sessionId: null, turns: [] });
  },

  receive({ task_id, envelope }) {
    const index = get().turns.findIndex((turn) => turn.taskId === task_id);
    if (index < 0) return;
    set({ turns: updateTurn(get().turns, index, (turn) => applyEvent(turn, envelope)) });
  },

  streamEnded({ task_id, error }) {
    const index = get().turns.findIndex((turn) => turn.taskId === task_id);
    if (index < 0) return;
    const turn = get().turns[index];
    if (isFinished(turn.status)) return;
    // The stream closed before a final event: say so rather than leave the turn spinning.
    set({
      turns: updateTurn(get().turns, index, (t) => ({
        ...t,
        status: "error",
        error: error
          ? `Lost the connection to this run: ${error.message}`
          : "The run's event stream ended without a result.",
      })),
    });
  },
}));

/** Subscribes the store to the Rust run events. Call once, before any request is sent. */
export async function startChatEvents(): Promise<() => void> {
  const offEvent = await onRunEvent((payload) => useChat.getState().receive(payload));
  const offEnd = await onRunStreamEnd((payload) => useChat.getState().streamEnded(payload));
  return () => {
    offEvent();
    offEnd();
  };
}
