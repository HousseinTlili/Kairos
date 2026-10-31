import { create } from "zustand";
import {
  addProviderEndpoint,
  addProviderKey,
  providersState,
  removeProvider,
  restartHermes,
  type CommandError,
  type ProvidersState,
} from "../../lib/tauri";
import { useConnection } from "../connection/store";
import { useModels } from "../models/store";
import { slugOf } from "./model";

interface ProvidersStore {
  state: ProvidersState | null;
  /** Something is being saved, removed or restarted. */
  busy: boolean;
  error: string | null;
  /** What was just added, to say whether Hermes lists it yet. */
  justAdded: string | null;
  load: () => Promise<void>;
  addKey: (id: string, key: string) => Promise<boolean>;
  addEndpoint: (
    name: string,
    api: string,
    key: string | null,
    models: string[],
  ) => Promise<boolean>;
  remove: (id: string, endpoint: boolean) => Promise<void>;
  restart: () => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

/** The model menu re-reads Hermes's list, so a new provider's models show there too. */
const refreshCatalog = () => useModels.getState().loadCatalog();

export const useProviders = create<ProvidersStore>((set, get) => {
  async function act(work: () => Promise<ProvidersState>, added: string | null): Promise<boolean> {
    set({ busy: true, error: null });
    try {
      const state = await work();
      set({ state, busy: false, justAdded: added });
      await refreshCatalog();
      return true;
    } catch (err) {
      set({ busy: false, error: messageOf(err) });
      return false;
    }
  }

  return {
    state: null,
    busy: false,
    error: null,
    justAdded: null,

    load: async () => {
      try {
        set({ state: await providersState(), error: null });
        await refreshCatalog();
      } catch (err) {
        set({ error: messageOf(err) });
      }
    },

    addKey: (id, key) => act(() => addProviderKey(id, key), id),

    addEndpoint: (name, api, key, models) =>
      act(() => addProviderEndpoint(name, api, key, models), null).then((ok) => {
        if (ok) set({ justAdded: slugOf(name) });
        return ok;
      }),

    remove: async (id, endpoint) => {
      await act(() => removeProvider(id, endpoint), null);
    },

    restart: async () => {
      set({ busy: true, error: null });
      try {
        await restartHermes();
        // The gateway answers a moment after it says it's ready (D-035).
        for (let i = 0; i < 10; i++) {
          await new Promise((resolve) => setTimeout(resolve, 1500));
          await useConnection.getState().check();
          if (useConnection.getState().info?.status === "connected") break;
        }
        await refreshCatalog();
        set({ busy: false });
      } catch (err) {
        set({ busy: false, error: messageOf(err) });
      }
      void get().load();
    },
  };
});
