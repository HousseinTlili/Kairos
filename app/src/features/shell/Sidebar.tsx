import { useState } from "react";
import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import type { Conversation, OtherConversation } from "../../lib/tauri";
import { ClockMark } from "./ClockMark";
import { ChevronIcon, GaugeIcon, PlusIcon, SettingsIcon } from "./icons";
import { useShell } from "./store";

const DAY = 24 * 60 * 60;

function groupByAge(items: Conversation[]) {
  const startOfToday = new Date();
  startOfToday.setHours(0, 0, 0, 0);
  const today = startOfToday.getTime() / 1000;
  const groups: { label: string; items: Conversation[] }[] = [
    { label: "Today", items: [] },
    { label: "Yesterday", items: [] },
    { label: "Previous 7 days", items: [] },
    { label: "Older", items: [] },
  ];
  for (const item of items) {
    const at = item.last_active;
    const group = at >= today ? 0 : at >= today - DAY ? 1 : at >= today - 7 * DAY ? 2 : 3;
    groups[group].items.push(item);
  }
  return groups.filter((group) => group.items.length > 0);
}

function sourceLabel(source: string | null): string {
  if (!source) return "Hermes";
  const names: Record<string, string> = {
    local: "Hermes Desktop",
    cli: "Hermes CLI",
    tui: "Hermes TUI",
  };
  return names[source] ?? `Hermes (${source})`;
}

function OtherList({ items }: { items: OtherConversation[] }) {
  const [open, setOpen] = useState(false);
  const peek = useShell((s) => s.peek);
  const peekId = useShell((s) => s.peekId);
  if (items.length === 0) return null;
  return (
    <div className="sidebar-group">
      <button className="group-toggle" onClick={() => setOpen(!open)} aria-expanded={open}>
        <ChevronIcon open={open} />
        Other Hermes conversations
        <span className="count">{items.length}</span>
      </button>
      {open && (
        <ul className="conversation-list">
          {items.map((item) => (
            <li key={item.id}>
              <button
                className={`conversation other ${peekId === item.id ? "active" : ""}`}
                onClick={() => peek(item.id)}
                title={`${item.title ?? "Untitled"} (${sourceLabel(item.source)}, read-only)`}
              >
                <span className="conversation-title">{item.title ?? "Untitled"}</span>
                <span className="conversation-meta">{sourceLabel(item.source)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function Sidebar() {
  const { conversations, sessionId, turns, newConversation, openConversation } = useChat();
  const { view, peekId, show } = useShell();
  const info = useConnection((s) => s.info);
  const connected = info?.status === "connected";
  const busy = turns.some((t) =>
    ["sending", "running", "waiting_for_approval", "stopping"].includes(t.status),
  );
  const groups = groupByAge(conversations.kairos);

  return (
    <nav className="sidebar" aria-label="Conversations">
      <div className="brand">
        <ClockMark />
        <span className="wordmark">Kairos</span>
      </div>

      <button
        className="new-conversation"
        onClick={() => {
          newConversation();
          show("chat");
        }}
        disabled={!connected || (busy && sessionId === null)}
      >
        <PlusIcon /> New conversation
      </button>

      <button
        className={`nav-item ${view === "system" ? "active" : ""}`}
        onClick={() => show("system")}
        aria-current={view === "system" ? "page" : undefined}
      >
        <GaugeIcon /> System
      </button>

      <div className="sidebar-scroll">
        {groups.length === 0 && connected && (
          <p className="sidebar-empty">Your conversations will appear here.</p>
        )}
        {groups.map((group) => (
          <div className="sidebar-group" key={group.label}>
            <h2 className="group-label">{group.label}</h2>
            <ul className="conversation-list">
              {group.items.map((item) => (
                <li key={item.session_id}>
                  <button
                    className={`conversation ${
                      view === "chat" && !peekId && sessionId === item.session_id ? "active" : ""
                    }`}
                    onClick={() => {
                      show("chat");
                      void openConversation(item.session_id);
                    }}
                    title={item.title}
                  >
                    <span className="conversation-title">{item.title}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ))}
        <OtherList items={conversations.other} />
      </div>

      <div className="sidebar-footer">
        <button
          className={`status-pill ${connected ? "ok" : "bad"}`}
          onClick={() => show("settings")}
          title="Connection details"
        >
          <span className="dot" aria-hidden="true" />
          {connected ? `Hermes ${info?.version ?? ""}` : "Not connected"}
        </button>
        <button
          className={`icon-button ${view === "settings" ? "active" : ""}`}
          onClick={() => show("settings")}
          aria-label="Settings"
          title="Settings"
        >
          <SettingsIcon />
        </button>
      </div>
    </nav>
  );
}
