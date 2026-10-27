import { create } from "zustand";

export type View = "chat" | "system" | "notifications" | "jobs" | "settings";
export type SettingsTab =
  | "connection"
  | "rules"
  | "system"
  | "notifications"
  | "briefing"
  | "accounts"
  | "models"
  | "quick"
  | "voice"
  | "persona"
  | "memory"
  | "startup";

interface ShellState {
  view: View;
  /** An "other Hermes conversation" opened read-only from the sidebar. */
  peekId: string | null;
  settingsTab: SettingsTab;
  show: (view: View) => void;
  showSettings: (tab: SettingsTab) => void;
  peek: (id: string | null) => void;
  /** The sidebar folded to a rail of icons (Ctrl+B); remembered on this PC. */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
}

const COLLAPSED_KEY = "kairos-sidebar-collapsed";

function rememberedCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}

export const useShell = create<ShellState>((set, get) => ({
  view: "chat",
  peekId: null,
  settingsTab: "connection",
  show: (view) => set({ view, peekId: null }),
  showSettings: (settingsTab) => set({ view: "settings", settingsTab, peekId: null }),
  peek: (peekId) => set({ peekId, view: "chat" }),
  sidebarCollapsed: rememberedCollapsed(),
  toggleSidebar() {
    const sidebarCollapsed = !get().sidebarCollapsed;
    set({ sidebarCollapsed });
    try {
      localStorage.setItem(COLLAPSED_KEY, sidebarCollapsed ? "1" : "0");
    } catch {
      // Without storage it just opens expanded next time.
    }
  },
}));
