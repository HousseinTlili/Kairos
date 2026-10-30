import { create } from "zustand";
import {
  automationOutcome,
  automationsState,
  deleteAutomation,
  emitKairosState,
  onAutomationFired,
  onAutomationsChanged,
  saveAutomation,
  setAutomationsEnabled,
  setCrashIgnore,
  testAutomation,
  type Automation,
  type AutomationFired,
  type AutomationsState,
  type CommandError,
} from "../../lib/tauri";
import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import { useShell } from "../shell/store";
import { useVoice } from "../voice/store";
import { ASK_WAIT_MS, askRequest, offerRequest } from "./model";

const BUSY = ["sending", "running", "waiting_for_approval", "stopping"];
const ISLAND_MS = 10_000;

/** An offer waiting for the Master's click, shown as a card above the chat. */
export interface Offer {
  fired: AutomationFired;
  text: string;
}

interface AutomationsStore {
  state: AutomationsState | null;
  offers: Offer[];
  error: string | null;
  start: () => Promise<() => void>;
  load: () => Promise<void>;
  save: (automation: Automation) => Promise<boolean>;
  remove: (id: string) => Promise<void>;
  setEnabled: (on: boolean) => Promise<void>;
  setIgnore: (names: string[]) => Promise<void>;
  test: (automation: Automation) => Promise<void>;
  acceptOffer: (runId: number) => Promise<void>;
  dismissOffer: (runId: number) => void;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

function chatBusy(): boolean {
  return useChat.getState().turns.some((t) => BUSY.includes(t.status));
}

const connected = () => useConnection.getState().info?.status === "connected";
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Starts a request in a new conversation, in the main window. */
async function startRequest(text: string) {
  const chat = useChat.getState();
  chat.newConversation();
  useShell.getState().show("chat");
  await chat.send(text);
}

/** "Ask Kairos" requests go one at a time: each waits for the chat to be free (M17 §3.4). */
let askQueue: Promise<void> = Promise.resolve();

/** Waits for a free chat; the reason it gave up, or null. */
async function waitForChat(): Promise<string | null> {
  const until = Date.now() + ASK_WAIT_MS;
  while (Date.now() < until) {
    if (!connected()) return "Hermes isn't connected";
    if (!chatBusy()) return null;
    await sleep(2000);
  }
  return "another request was running for 5 min";
}

async function carryOut(fired: AutomationFired, addOffer: (offer: Offer) => void) {
  // Set from inside the queued request, so an object rather than two variables.
  const result: { asked: boolean; skipped: string | null } = { asked: false, skipped: null };
  for (const action of fired.actions) {
    switch (action.kind) {
      case "say":
        useVoice.getState().speak(`automation-${fired.run_id}`, action.text);
        break;
      case "island":
        void emitKairosState({
          source: "automation",
          mood: "idle",
          text: action.text,
          approval: false,
          sessionId: null,
          heard: null,
          answer: null,
          doneAt: Date.now(),
          holdMs: ISLAND_MS,
        }).catch(() => {});
        break;
      case "offer":
        addOffer({ fired, text: action.text });
        break;
      case "ask": {
        const text = askRequest(fired, action.text);
        const turn = askQueue.then(async () => {
          const why = await waitForChat();
          if (why) {
            result.skipped = why;
            return;
          }
          result.asked = true;
          await startRequest(text);
        });
        askQueue = turn.catch(() => {});
        await turn;
        break;
      }
      default:
        break;
    }
  }
  await automationOutcome(
    fired.run_id,
    result.skipped ? "skipped" : "done",
    result.skipped,
    result.asked,
  ).catch(() => {});
}

export const useAutomations = create<AutomationsStore>((set, get) => ({
  state: null,
  offers: [],
  error: null,

  start: async () => {
    const stops = await Promise.all([
      onAutomationFired((fired) => {
        void carryOut(fired, (offer) =>
          set((s) => ({
            offers: [...s.offers.filter((o) => o.fired.name !== offer.fired.name), offer],
          })),
        ).catch((err) =>
          automationOutcome(fired.run_id, "failed", messageOf(err), false).catch(() => {}),
        );
      }),
      onAutomationsChanged(() => {
        if (get().state) void get().load();
      }),
    ]);
    return () => {
      for (const stop of stops) stop();
    };
  },

  load: async () => {
    try {
      set({ state: await automationsState(), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  save: async (automation) => {
    try {
      set({ state: await saveAutomation(automation), error: null });
      return true;
    } catch (err) {
      set({ error: messageOf(err) });
      return false;
    }
  },

  remove: async (id) => {
    try {
      set({ state: await deleteAutomation(id), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  setEnabled: async (on) => {
    try {
      set({ state: await setAutomationsEnabled(on), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  setIgnore: async (names) => {
    try {
      set({ state: await setCrashIgnore(names), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  test: async (automation) => {
    try {
      await testAutomation(automation);
      set({ error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  acceptOffer: async (runId) => {
    const offer = get().offers.find((o) => o.fired.run_id === runId);
    if (!offer) return;
    if (chatBusy()) {
      set({ error: "Kairos is busy with a request; try again when it's done." });
      return;
    }
    set((s) => ({ offers: s.offers.filter((o) => o.fired.run_id !== runId), error: null }));
    await startRequest(offerRequest(offer.fired, offer.text));
  },

  dismissOffer: (runId) =>
    set((s) => ({ offers: s.offers.filter((o) => o.fired.run_id !== runId) })),
}));
