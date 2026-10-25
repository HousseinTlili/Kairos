import { create } from "zustand";
import {
  accountsState,
  connectAccount,
  disconnectAccount,
  type AccountKind,
  type AccountState,
  type CommandError,
} from "../../lib/tauri";
import { useChat } from "../chat/store";
import { useShell } from "../shell/store";
import { googleSetupRequest } from "./model";

interface AccountsStore {
  accounts: AccountState[] | null;
  busy: AccountKind | null;
  error: string | null;
  load: () => Promise<void>;
  connect: (kind: AccountKind) => Promise<void>;
  disconnect: (kind: AccountKind) => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

export const useAccounts = create<AccountsStore>((set) => ({
  accounts: null,
  busy: null,
  error: null,

  load: async () => {
    try {
      set({ accounts: await accountsState(), error: null });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  connect: async (kind) => {
    set({ busy: kind, error: null });
    try {
      if (kind === "google") {
        // Hermes's skill drives Google's sign-in in a conversation.
        const chat = useChat.getState();
        chat.newConversation();
        useShell.getState().show("chat");
        await chat.send(googleSetupRequest());
      } else {
        await connectAccount(kind);
      }
    } catch (err) {
      set({ error: messageOf(err) });
    } finally {
      set({ busy: null });
    }
  },

  disconnect: async (kind) => {
    set({ busy: kind, error: null });
    try {
      set({ accounts: await disconnectAccount(kind) });
    } catch (err) {
      set({ error: messageOf(err) });
    } finally {
      set({ busy: null });
    }
  },
}));
