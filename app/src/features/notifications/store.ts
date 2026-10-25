import { create } from "zustand";
import {
  clearNotifications,
  listNotifications,
  markNotificationsRead,
  notificationsState,
  onNotificationsChanged,
  setNotificationsEnabled,
  type CommandError,
  type NotificationsState,
  type StoredNotification,
} from "../../lib/tauri";

interface NotificationsStore {
  state: NotificationsState | null;
  items: StoredNotification[];
  /** The app filter; null = all apps. */
  appId: string | null;
  unreadOnly: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  setFilter: (appId: string | null, unreadOnly?: boolean) => Promise<void>;
  setEnabled: (enabled: boolean) => Promise<void>;
  markRead: (id: number | null, appId: string | null) => Promise<void>;
  clear: (appId: string | null) => Promise<void>;
  /** Called once at startup: keeps the badge current. */
  start: () => Promise<() => void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

export const useNotifications = create<NotificationsStore>((set, get) => ({
  state: null,
  items: [],
  appId: null,
  unreadOnly: false,
  error: null,

  refresh: async () => {
    try {
      const { appId, unreadOnly } = get();
      const [state, items] = await Promise.all([
        notificationsState(),
        listNotifications(appId, unreadOnly),
      ]);
      set({ state, items, error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  setFilter: async (appId, unreadOnly = get().unreadOnly) => {
    set({ appId, unreadOnly });
    await get().refresh();
  },

  setEnabled: async (enabled) => {
    try {
      set({ state: await setNotificationsEnabled(enabled), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  markRead: async (id, appId) => {
    try {
      await markNotificationsRead(id, appId);
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  clear: async (appId) => {
    try {
      await clearNotifications(appId);
      if (get().appId === appId) set({ appId: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  start: async () => {
    // Every change (a poll, mark read, clear) arrives as one event; re-read once per event.
    const unlisten = await onNotificationsChanged(() => void get().refresh());
    await get().refresh();
    return unlisten;
  },
}));
