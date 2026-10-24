import { create } from "zustand";
import { connect, reloadKey, type CommandError, type ConnectInfo } from "../../lib/tauri";

interface ConnectionState {
  info: ConnectInfo | null;
  error: CommandError | null;
  checking: boolean;
  check: () => Promise<void>;
  reloadKey: () => Promise<void>;
}

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

export const useConnection = create<ConnectionState>((set) => ({
  info: null,
  error: null,
  checking: false,
  check: () => run(set, connect),
  reloadKey: () => run(set, reloadKey),
}));

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
