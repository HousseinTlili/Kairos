import { create } from "zustand";
import {
  resetRules,
  rulesState,
  saveRules,
  validateRules,
  type CommandError,
  type RulesState,
  type RulesValidation,
} from "../../lib/tauri";
import { parsePolicy, serializePolicy, type PolicyDoc } from "./model";

const VALIDATE_DELAY_MS = 400;
let validateTimer: ReturnType<typeof setTimeout> | null = null;

interface RulesStore {
  /** The file on disk, as the plugin sees it. */
  saved: RulesState | null;
  /** The edited rules; null while the raw text isn't a rules document. */
  draft: PolicyDoc | null;
  draftText: string;
  /** The plugin validator's verdict on the draft. */
  draftValidation: RulesValidation | null;
  checking: boolean;
  saving: boolean;
  error: string | null;
  load: () => Promise<void>;
  edit: (change: (doc: PolicyDoc) => PolicyDoc) => void;
  setRaw: (text: string) => void;
  save: () => Promise<void>;
  discard: () => void;
  restoreStarter: () => Promise<void>;
}

export const useRules = create<RulesStore>((set, get) => {
  const validateSoon = () => {
    if (validateTimer) clearTimeout(validateTimer);
    set({ checking: true });
    validateTimer = setTimeout(async () => {
      const text = get().draftText;
      try {
        const verdict = await validateRules(text);
        // Ignore an answer for text the user has since changed.
        if (get().draftText === text) set({ draftValidation: verdict, checking: false });
      } catch (err) {
        set({ checking: false, error: (err as CommandError).message ?? String(err) });
      }
    }, VALIDATE_DELAY_MS);
  };

  const adopt = (state: RulesState) =>
    set({
      saved: state,
      draft: parsePolicy(state.text),
      draftText: state.text,
      draftValidation: state.validation,
      checking: false,
      error: null,
    });

  return {
    saved: null,
    draft: null,
    draftText: "",
    draftValidation: null,
    checking: false,
    saving: false,
    error: null,

    async load() {
      try {
        adopt(await rulesState());
      } catch (err) {
        set({ error: (err as CommandError).message ?? String(err) });
      }
    },

    edit(change) {
      const draft = get().draft;
      if (!draft) return;
      const next = change(draft);
      set({ draft: next, draftText: serializePolicy(next) });
      validateSoon();
    },

    setRaw(text) {
      set({ draftText: text, draft: parsePolicy(text) });
      validateSoon();
    },

    async save() {
      set({ saving: true, error: null });
      try {
        adopt(await saveRules(get().draftText));
      } catch (err) {
        set({ error: (err as CommandError).message ?? String(err) });
      } finally {
        set({ saving: false });
      }
    },

    discard() {
      const saved = get().saved;
      if (saved) adopt(saved);
    },

    async restoreStarter() {
      set({ saving: true, error: null });
      try {
        adopt(await resetRules());
      } catch (err) {
        set({ error: (err as CommandError).message ?? String(err) });
      } finally {
        set({ saving: false });
      }
    },
  };
});

export function isDirty(store: Pick<RulesStore, "saved" | "draftText">): boolean {
  return store.saved !== null && (store.draftText !== store.saved.text || !store.saved.exists);
}
