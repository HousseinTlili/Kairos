import { create } from "zustand";
import {
  onSystemSnapshot,
  setSystemPageVisible,
  systemSnapshot,
  type CommandError,
  type SystemSnapshot,
} from "../../lib/tauri";
import { pushHistory, usedShare } from "./format";

export interface SystemHistory {
  cpu: number[];
  memory: number[];
  down: number[];
  up: number[];
}

interface SystemStore {
  snapshot: SystemSnapshot | null;
  history: SystemHistory;
  error: string | null;
  /** Called once at startup: readings keep coming (every 15 s) while the page is closed. */
  start: () => Promise<() => void>;
  /** The page is on screen: readings every 2 s. */
  setVisible: (visible: boolean) => void;
}

const EMPTY: SystemHistory = { cpu: [], memory: [], down: [], up: [] };

export function addToHistory(history: SystemHistory, snapshot: SystemSnapshot): SystemHistory {
  return {
    cpu: pushHistory(history.cpu, snapshot.cpu.total),
    memory: pushHistory(history.memory, usedShare(snapshot.memory.used, snapshot.memory.total)),
    down: pushHistory(history.down, snapshot.network.down_per_s),
    up: pushHistory(history.up, snapshot.network.up_per_s),
  };
}

export const useSystem = create<SystemStore>((set, get) => ({
  snapshot: null,
  history: EMPTY,
  error: null,

  start: async () => {
    const unlisten = await onSystemSnapshot((snapshot) =>
      set({ snapshot, history: addToHistory(get().history, snapshot), error: null }),
    );
    try {
      const first = await systemSnapshot();
      if (first && !get().snapshot) set({ snapshot: first });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    }
    return unlisten;
  },

  setVisible: (visible) => {
    setSystemPageVisible(visible).catch((err: CommandError) =>
      set({ error: err.message ?? String(err) }),
    );
  },
}));
