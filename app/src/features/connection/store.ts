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
export function approvalWarning(mode: string | null): string | null {
  if (mode === "manual") return null;
  if (mode === null) {
    return "Couldn't read Hermes's approval mode. Kairos needs it to be 'manual' until M2.";
  }
  if (mode === "smart") {
    return "Approval mode is 'smart': Hermes's helper model can auto-approve destructive commands (it approved rm -rf in testing). Kairos needs 'manual' until M2.";
  }
  return `Approval mode is '${mode}', which can run commands without asking you. Kairos needs 'manual' until M2.`;
}
