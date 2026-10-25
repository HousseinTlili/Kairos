import { create } from "zustand";

export type View = "chat" | "system" | "settings";
export type SettingsTab = "connection" | "rules";

interface ShellState {
  view: View;
  /** An "other Hermes conversation" opened read-only from the sidebar. */
  peekId: string | null;
  settingsTab: SettingsTab;
  show: (view: View) => void;
  showSettings: (tab: SettingsTab) => void;
  peek: (id: string | null) => void;
}

export const useShell = create<ShellState>((set) => ({
  view: "chat",
  peekId: null,
  settingsTab: "connection",
  show: (view) => set({ view, peekId: null }),
  showSettings: (settingsTab) => set({ view: "settings", settingsTab, peekId: null }),
  peek: (peekId) => set({ peekId, view: "chat" }),
}));
