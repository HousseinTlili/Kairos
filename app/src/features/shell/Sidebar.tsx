import { useEffect, useState } from "react";
import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import type { Conversation, OtherConversation } from "../../lib/tauri";
import { PRESENCE_WORDS, usePresence } from "./presence";
import { SealMark } from "./SealMark";
import { useBriefing } from "../briefing/store";
import { badgeText } from "../notifications/model";
import { useNotifications } from "../notifications/store";
import {
  BellIcon,
  CalendarIcon,
  ChevronIcon,
  GaugeIcon,
  PlusIcon,
  SettingsIcon,
  SidebarIcon,
  TrashIcon,
  SunIcon,
} from "./icons";
import { isMemoryConversation } from "../memory/model";
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

/**
 * One conversation in the list. Its bin turns the row into a question first ("Delete this
 * conversation?"), so a stray click never deletes anything; Esc or Keep backs out.
 */
function ConversationRow({
  title,
  active,
  onOpen,
  onDelete,
}: {
  title: string;
  active: boolean;
  onOpen: () => void;
  onDelete: () => Promise<string | null>;
}) {
  const [asking, setAsking] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (asking) {
    return (
      <li
        className="conversation-confirm"
        role="group"
        aria-label={`Delete ${title}?`}
        onKeyDown={(e) => {
          if (e.key === "Escape") setAsking(false);
        }}
      >
        <span className="conversation-confirm-text">Delete this conversation?</span>
        <span className="conversation-confirm-actions">
          <button
            className="danger small"
            autoFocus
            disabled={deleting}
            onClick={() => {
              setDeleting(true);
              void onDelete().then((why) => {
                setDeleting(false);
                setError(why);
                if (!why) setAsking(false);
              });
            }}
          >
            {deleting ? "Deleting…" : "Delete"}
          </button>
          <button className="small" disabled={deleting} onClick={() => setAsking(false)}>
            Keep
          </button>
        </span>
        {error && <span className="conversation-confirm-error">{error}</span>}
      </li>
    );
  }
  return (
    <li className="conversation-row">
      <button className={`conversation ${active ? "active" : ""}`} onClick={onOpen} title={title}>
        <span className="conversation-title">{title}</span>
      </button>
      <button
        className="icon-button conversation-delete"
        onClick={() => {
          setError(null);
          setAsking(true);
        }}
        aria-label={`Delete ${title}`}
        title="Delete this conversation"
      >
        <TrashIcon />
      </button>
    </li>
  );
}

export function Sidebar() {
  const { conversations, sessionId, turns, newConversation, openConversation, deleteConversation } =
    useChat();
  const { view, peekId, show, sidebarCollapsed, toggleSidebar } = useShell();
  const info = useConnection((s) => s.info);
  const connected = info?.status === "connected";
  const busy = turns.some((t) =>
    ["sending", "running", "waiting_for_approval", "stopping"].includes(t.status),
  );
  // Forget requests (M11) have their own conversation, kept out of the list.
  const groups = groupByAge(
    conversations.kairos.filter((c) => !isMemoryConversation(c.session_id)),
  );
  const badge = badgeText(useNotifications((s) => s.state?.unread ?? 0));
  const reopenBriefing = useBriefing((s) => s.reopen);

  const presence = usePresence();

  // Ctrl+B folds the sidebar to its rail and back, as in most editors.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleSidebar]);

  const fold = sidebarCollapsed ? "Expand the sidebar (Ctrl+B)" : "Collapse the sidebar (Ctrl+B)";

  return (
    <nav className={`sidebar ${sidebarCollapsed ? "collapsed" : ""}`} aria-label="Conversations">
      <div className={`brand ${presence.state}`}>
        <SealMark size={40} state={presence.state} level={presence.level} />
        <span className="brand-words">
          <span className="wordmark">Kairos</span>
          <span className="brand-state" aria-live="polite">
            {PRESENCE_WORDS[presence.state] || "At your service"}
          </span>
        </span>
        <button
          className="icon-button fold-button"
          onClick={toggleSidebar}
          aria-label={fold}
          aria-expanded={!sidebarCollapsed}
          title={fold}
        >
          <SidebarIcon />
        </button>
      </div>

      <button
        className="new-conversation"
        onClick={() => {
          newConversation();
          show("chat");
        }}
        disabled={!connected || (busy && sessionId === null)}
        title="New conversation"
      >
        <PlusIcon /> <span className="label">New conversation</span>
      </button>

      <button
        className={`nav-item ${view === "system" ? "active" : ""}`}
        onClick={() => show("system")}
        aria-current={view === "system" ? "page" : undefined}
        title="System"
      >
        <GaugeIcon /> <span className="label">System</span>
      </button>

      <button
        className={`nav-item ${view === "notifications" ? "active" : ""}`}
        onClick={() => show("notifications")}
        aria-current={view === "notifications" ? "page" : undefined}
        title="Notifications"
      >
        <BellIcon /> <span className="label">Notifications</span>
        {badge && (
          <span className="nav-badge" aria-label={`${badge} unread`}>
            {badge}
          </span>
        )}
      </button>

      <button
        className={`nav-item ${view === "jobs" ? "active" : ""}`}
        onClick={() => show("jobs")}
        aria-current={view === "jobs" ? "page" : undefined}
        title="Scheduled"
      >
        <CalendarIcon /> <span className="label">Scheduled</span>
      </button>

      <button
        className="nav-item"
        onClick={() => {
          show("chat");
          void reopenBriefing();
        }}
        title="Show the latest welcome-back briefing"
      >
        <SunIcon /> <span className="label">Briefing</span>
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
                <ConversationRow
                  key={item.session_id}
                  title={item.title}
                  active={view === "chat" && !peekId && sessionId === item.session_id}
                  onOpen={() => {
                    show("chat");
                    void openConversation(item.session_id);
                  }}
                  onDelete={() => deleteConversation(item.session_id)}
                />
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
          <span className="label">
            {connected ? `Hermes ${info?.version ?? ""}` : "Not connected"}
          </span>
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
