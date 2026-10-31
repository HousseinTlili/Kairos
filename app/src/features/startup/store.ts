import { create } from "zustand";
import {
  onPresence,
  setKairosAtLogin,
  setStartHermes,
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
  setStartHermes: (enabled: boolean) => Promise<void>;
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

  setStartHermes: async (enabled) => {
    try {
      const now = await setStartHermes(enabled);
      const current = get().state;
      set({ state: current && { ...current, start_hermes: now }, error: null });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    }
  },

  start: async () =>
    onPresence((event) => {
      const current = get().state;
      if (current) set({ state: { ...current, recent: [event, ...current.recent].slice(0, 8) } });
    }),
}));
