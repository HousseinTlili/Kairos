import { create } from "zustand";
import {
  createRun,
  memoryState,
  onRunStreamEnd,
  subscribeRun,
  type CommandError,
  type MemoryState,
} from "../../lib/tauri";
import { FORGET_SESSION, forgetRequest, type Forgettable } from "./model";

/** Longer than this and Kairos is probably waiting for an answer in the Forget conversation. */
const FORGET_TIMEOUT_MS = 90_000;

interface MemoryStore {
  state: MemoryState | null;
  error: string | null;
  /** The item being forgotten, by its text or fact id. */
  forgetting: string | null;
  /** Kairos didn't finish in time: the page offers to open the conversation. */
  stalled: boolean;
  load: () => Promise<void>;
  forget: (item: Forgettable) => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

export function keyOf(item: Forgettable): string {
  return item.kind === "fact" ? `fact-${item.fact.id}` : `${item.kind}-${item.text}`;
}

export const useMemory = create<MemoryStore>((set, get) => ({
  state: null,
  error: null,
  forgetting: null,
  stalled: false,

  async load() {
    try {
      set({ state: await memoryState(), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  async forget(item) {
    if (get().forgetting) return;
    set({ forgetting: keyOf(item), stalled: false, error: null });
    try {
      const task = await createRun(FORGET_SESSION, forgetRequest(item), null);
      let finish: () => void = () => {};
      const ended = new Promise<void>((resolve) => (finish = resolve));
      // Listening first, so a quick run can't end before anyone hears it.
      const unlisten = await onRunStreamEnd((end) => {
        if (end.task_id !== task.task_id) return;
        if (end.error) set({ error: end.error.message });
        finish();
      });
      const timer = setTimeout(() => {
        set({ stalled: true });
        finish();
      }, FORGET_TIMEOUT_MS);
      void subscribeRun(task.task_id, task.run_id).catch((err: unknown) => {
        set({ error: messageOf(err) });
        finish();
      });
      await ended;
      clearTimeout(timer);
      unlisten();
    } catch (err) {
      set({ error: messageOf(err) });
    }
    set({ forgetting: null });
    await get().load();
  },
}));
