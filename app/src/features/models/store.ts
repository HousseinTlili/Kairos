import { create } from "zustand";
import {
  conversationModel,
  modelCatalog,
  type Catalog,
  type CommandError,
  type ModelChoice,
} from "../../lib/tauri";
import { DEFAULT_CHOICE, isOffered } from "./model";

interface ModelsStore {
  catalog: Catalog | null;
  /** The open conversation's model; sent with each request (M7 §4). */
  choice: ModelChoice;
  error: string | null;
  loadCatalog: () => Promise<void>;
  /** Called when a conversation opens; a new conversation starts on Hermes's default. */
  loadFor: (sessionId: string | null) => Promise<void>;
  choose: (choice: ModelChoice) => void;
}

export const useModels = create<ModelsStore>((set, get) => ({
  catalog: null,
  choice: DEFAULT_CHOICE,
  error: null,

  loadCatalog: async () => {
    try {
      set({ catalog: await modelCatalog(), error: null });
    } catch (err) {
      set({ error: (err as CommandError).message ?? String(err) });
    }
  },

  loadFor: async (sessionId) => {
    if (!sessionId) {
      set({ choice: DEFAULT_CHOICE });
      return;
    }
    try {
      const choice = await conversationModel(sessionId);
      set({ choice: isOffered(get().catalog, choice) ? choice : DEFAULT_CHOICE });
    } catch {
      set({ choice: DEFAULT_CHOICE });
    }
  },

  choose: (choice) => set({ choice }),
}));
