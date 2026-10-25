import { create } from "zustand";
import {
  accountsState,
  briefNow,
  briefingSettings,
  latestBriefing,
  onBriefing,
  saveBriefingSettings,
  setUtcOffset,
  type Briefing,
  type BriefingSettings,
  type CommandError,
} from "../../lib/tauri";
import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import { useShell } from "../shell/store";
import {
  NO_ACCOUNTS,
  isFresh,
  mayStartSummary,
  summaryRequest,
  type ConnectedAccounts,
} from "./model";

const HOUR_MS = 60 * 60 * 1000;
const BUSY = ["sending", "running", "waiting_for_approval", "stopping"];

interface BriefingStore {
  briefing: Briefing | null;
  /** The card is on screen. */
  open: boolean;
  /** The AI summary was started for this briefing. */
  summaryStarted: boolean;
  projects: string[];
  settings: BriefingSettings | null;
  error: string | null;
  start: () => Promise<() => void>;
  dismiss: () => void;
  reopen: () => Promise<void>;
  startSummary: () => Promise<void>;
  loadSettings: () => Promise<void>;
  saveSettings: (settings: BriefingSettings) => Promise<void>;
  briefNow: () => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

/** Connected accounts (M6) widen the summary; if they can't be read, it goes without them. */
async function connectedAccounts(): Promise<ConnectedAccounts> {
  try {
    const states = await accountsState();
    const on = (kind: string) => states.some((a) => a.kind === kind && a.status === "connected");
    return { google: on("google"), github: on("github") };
  } catch {
    return NO_ACCOUNTS;
  }
}

function chatBusy(): boolean {
  return useChat.getState().turns.some((t) => BUSY.includes(t.status));
}

export const useBriefing = create<BriefingStore>((set, get) => ({
  briefing: null,
  open: false,
  summaryStarted: false,
  projects: [],
  settings: null,
  error: null,

  start: async () => {
    // Quiet hours are local: the core learns the offset from here (D-008: no time-zone crate).
    const report = () => void setUtcOffset(-new Date().getTimezoneOffset()).catch(() => {});
    const unlisten = await onBriefing((notice) => {
      set({
        briefing: notice.briefing,
        open: true,
        summaryStarted: false,
        projects: notice.projects,
      });
      const connected = useConnection.getState().info?.status === "connected";
      if (mayStartSummary(notice.ai_summary, connected, chatBusy())) void get().startSummary();
    });
    report();
    const timer = setInterval(report, HOUR_MS);
    try {
      const latest = await latestBriefing();
      if (latest && isFresh(latest) && !get().briefing) set({ briefing: latest, open: true });
    } catch (err) {
      set({ error: messageOf(err) });
    }
    return () => {
      unlisten();
      clearInterval(timer);
    };
  },

  dismiss: () => set({ open: false }),

  reopen: async () => {
    try {
      const latest = get().briefing ?? (await latestBriefing());
      set({ briefing: latest, open: latest != null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  startSummary: async () => {
    const { briefing, projects } = get();
    if (!briefing || chatBusy()) return;
    set({ summaryStarted: true });
    const accounts = await connectedAccounts();
    const chat = useChat.getState();
    chat.newConversation();
    useShell.getState().show("chat");
    await chat.send(summaryRequest(briefing, projects, accounts));
  },

  loadSettings: async () => {
    try {
      set({ settings: await briefingSettings(), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  saveSettings: async (settings) => {
    try {
      set({ settings: await saveBriefingSettings(settings), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  briefNow: async () => {
    try {
      await briefNow();
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },
}));
