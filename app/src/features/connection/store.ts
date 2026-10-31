import { create } from "zustand";
import {
  connect,
  reloadKey,
  startHermes,
  type CommandError,
  type ConnectInfo,
} from "../../lib/tauri";

interface ConnectionState {
  info: ConnectInfo | null;
  error: CommandError | null;
  checking: boolean;
  /** Kairos is starting Hermes's gateway (D-035). */
  starting: boolean;
  /** Why the last start failed, in words. */
  startError: string | null;
  check: () => Promise<void>;
  reloadKey: () => Promise<void>;
  /** The Master's "Start Hermes". */
  startHermes: () => Promise<void>;
}

// Kairos starts Hermes by itself once per run of the app; after that, only the Master's button.
let triedByItself = false;
const RECHECK_MS = 20_000;

async function run(
  set: (partial: Partial<ConnectionState>) => void,
  action: () => Promise<ConnectInfo>,
) {
  set({ checking: true, error: null });
  try {
    set({ info: await action(), checking: false });
  } catch (err) {
    set({ error: err as CommandError, checking: false });
  }
}

export const useConnection = create<ConnectionState>((set, get) => {
  /** Starts Hermes, then checks again: the API answers once `ensure` says it is ready. */
  async function start(automatic: boolean) {
    set({ starting: true, startError: null });
    try {
      const tried = await startHermes(automatic);
      if (tried) {
        // "Ready" from `ensure` can come a moment before the API opens its port (seen
        // 2026-10-31): check again until it answers, for a little while.
        const until = Date.now() + RECHECK_MS;
        await run(set, connect);
        while (get().info?.status === "not_running" && Date.now() < until) {
          await new Promise((resolve) => setTimeout(resolve, 1500));
          await run(set, connect);
        }
      }
      set({ starting: false });
    } catch (err) {
      set({ starting: false, startError: (err as CommandError).message ?? String(err) });
    }
  }

  return {
    info: null,
    error: null,
    checking: false,
    starting: false,
    startError: null,
    check: async () => {
      await run(set, connect);
      if (get().info?.status === "not_running" && !triedByItself) {
        triedByItself = true;
        await start(true);
      }
    },
    reloadKey: () => run(set, reloadKey),
    startHermes: () => start(false),
  };
});

/** D-009: only `manual` is acceptable until M2 is accepted. */
/** D-009 (2026-10-25): `smart` with the Kairos rules, or `manual`. Anything that can run commands
 *  without any check (`off`, YOLO) is never acceptable. */
export const ACCEPTED_APPROVAL_MODES = ["smart", "manual"];

export function approvalWarning(mode: string | null): string | null {
  if (mode !== null && ACCEPTED_APPROVAL_MODES.includes(mode)) return null;
  if (mode === null) {
    return "Couldn't read Hermes's approval mode. Kairos expects 'smart' (with its rules) or 'manual'.";
  }
  return `Approval mode is '${mode}', which can run commands without asking you. Kairos expects 'smart' (with its rules) or 'manual'.`;
}
