import { create } from "zustand";

export type View = "chat" | "settings";

interface ShellState {
  view: View;
  /** An "other Hermes conversation" opened read-only from the sidebar. */
  peekId: string | null;
  show: (view: View) => void;
  peek: (id: string | null) => void;
}

export const useShell = create<ShellState>((set) => ({
  view: "chat",
  peekId: null,
  show: (view) => set({ view, peekId: null }),
  peek: (peekId) => set({ peekId, view: "chat" }),
}));
