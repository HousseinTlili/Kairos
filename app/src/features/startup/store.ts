import { create } from "zustand";
import {
  onPresence,
  setKairosAtLogin,
  startupState,
  type CommandError,
  type StartupState,
} from "../../lib/tauri";

interface StartupStore {
  state: StartupState | null;
  busy: boolean;
  error: string | null;
  load: () => Promise<void>;
  setKairosAtLogin: (enabled: boolean) => Promise<void>;
  /** Keeps "recent" current while the app runs. */
  start: () => Promise<() => void>;
}

export const useStartup = create<StartupStore>((set, get) => ({
  state: null,
  busy: false,
  error: null,

  load: async () => {
    try {
      set({ state: await startupState(), error: null });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    }
  },

  setKairosAtLogin: async (enabled) => {
    set({ busy: true });
    try {
      const now = await setKairosAtLogin(enabled);
      const current = get().state;
      set({ state: current && { ...current, kairos_at_login: now }, error: null });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    } finally {
      set({ busy: false });
    }
  },

  start: async () =>
    onPresence((event) => {
      const current = get().state;
      if (current) set({ state: { ...current, recent: [event, ...current.recent].slice(0, 8) } });
    }),
}));
