import { create } from "zustand";
import {
  createJob,
  deleteJob,
  jobAction,
  listJobs,
  type CommandError,
  type Job,
  type JobAction,
} from "../../lib/tauri";

interface JobsStore {
  jobs: Job[];
  loaded: boolean;
  error: string | null;
  /** The job a button is working on, so it can't be pressed twice. */
  busyId: string | null;
  refresh: () => Promise<void>;
  create: (name: string, schedule: string, prompt: string) => Promise<boolean>;
  act: (id: string, action: JobAction) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

export const useJobs = create<JobsStore>((set, get) => ({
  jobs: [],
  loaded: false,
  error: null,
  busyId: null,

  refresh: async () => {
    try {
      set({ jobs: await listJobs(), loaded: true, error: null });
    } catch (err) {
      set({ loaded: true, error: messageOf(err) });
    }
  },

  create: async (name, schedule, prompt) => {
    try {
      await createJob(name, schedule, prompt);
      await get().refresh();
      return true;
    } catch (err) {
      set({ error: messageOf(err) });
      return false;
    }
  },

  act: async (id, action) => {
    set({ busyId: id });
    try {
      await jobAction(id, action);
      await get().refresh();
    } catch (err) {
      set({ error: messageOf(err) });
    } finally {
      set({ busyId: null });
    }
  },

  remove: async (id) => {
    set({ busyId: id });
    try {
      await deleteJob(id);
      await get().refresh();
    } catch (err) {
      set({ error: messageOf(err) });
    } finally {
      set({ busyId: null });
    }
  },
}));
