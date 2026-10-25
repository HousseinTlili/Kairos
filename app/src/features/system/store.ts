import { create } from "zustand";
import {
  alertsEnabled,
  onSpeedTest,
  onSystemAlerts,
  onSystemSnapshot,
  runSpeedTest,
  setAlertsEnabled,
  setSystemPageVisible,
  systemSnapshot,
  type CommandError,
  type SpeedTestProgress,
  type SystemAlert,
  type SystemSnapshot,
} from "../../lib/tauri";
import { applyAlerts } from "./alerts";
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
  /** The latest speed-test progress or outcome; null before the first run. */
  speed: SpeedTestProgress | null;
  runSpeedTest: () => Promise<void>;
  /** Alerts on screen, newest first (M3 §7). */
  alerts: SystemAlert[];
  /** Settings ▸ System's switch; null until read. */
  alertsOn: boolean | null;
  dismissAlert: (id: string) => void;
  setAlertsOn: (on: boolean) => Promise<void>;
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
  speed: null,
  alerts: [],
  alertsOn: null,

  dismissAlert: (id) => set({ alerts: get().alerts.filter((a) => a.id !== id) }),

  setAlertsOn: async (on) => {
    try {
      set({ alertsOn: await setAlertsEnabled(on), alerts: on ? get().alerts : [] });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    }
  },

  runSpeedTest: async () => {
    set({ speed: { phase: "latency" } });
    try {
      await runSpeedTest();
    } catch (err) {
      // The command's own "failed" event usually got here first; this covers "busy" and the like.
      const message = (err as CommandError).message ?? String(err);
      if (get().speed?.phase !== "failed") set({ speed: { phase: "failed", message } });
    }
  },

  start: async () => {
    const stopSnapshots = await onSystemSnapshot((snapshot) =>
      set({ snapshot, history: addToHistory(get().history, snapshot), error: null }),
    );
    const stopSpeed = await onSpeedTest((speed) => set({ speed }));
    const stopAlerts = await onSystemAlerts((evaluation) =>
      set({ alerts: applyAlerts(get().alerts, evaluation) }),
    );
    const unlisten = () => {
      stopSnapshots();
      stopSpeed();
      stopAlerts();
    };
    try {
      set({ alertsOn: await alertsEnabled() });
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
