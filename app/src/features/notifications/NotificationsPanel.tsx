import { useEffect, useState } from "react";
import { useShell } from "../shell/store";
import { clearQuestion, timeLabel, totalOf } from "./model";
import { useNotifications } from "./store";

export function NotificationsPanel() {
  const { state, items, appId, unreadOnly, error, refresh, setFilter, markRead, clear } =
    useNotifications();
  const showSettings = useShell((s) => s.showSettings);
  const [confirmClear, setConfirmClear] = useState(false);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const counts = state?.counts ?? [];
  const selected = counts.find((c) => c.app_id === appId) ?? null;
  const total = totalOf(counts);

  return (
    <section className="notifications" aria-labelledby="notifications-title">
      <div className="system-head">
        <h1 id="notifications-title">Notifications</h1>
        {state && <span className="muted">{state.unread} unread</span>}
      </div>

      {state && state.state !== "on" && (
        <div className="status-card" role="status">
          <p>
            {state.state === "off"
              ? "Kairos isn't reading your Windows notifications."
              : "Kairos can't read notifications right now."}{" "}
            <button className="link-button" onClick={() => showSettings("notifications")}>
              Notification settings
            </button>
          </p>
        </div>
      )}
      {error && (
        <div className="status-card bad" role="alert">
          <p>{error}</p>
        </div>
      )}

      <div className="notif-toolbar">
        <div className="notif-apps" role="tablist" aria-label="Apps">
          <button
            role="tab"
            aria-selected={appId === null}
            className={appId === null ? "chip active" : "chip"}
            onClick={() => void setFilter(null)}
          >
            All {state && state.unread > 0 && <span className="chip-count">{state.unread}</span>}
          </button>
          {counts.map((c) => (
            <button
              key={c.app_id}
              role="tab"
              aria-selected={appId === c.app_id}
              className={appId === c.app_id ? "chip active" : "chip"}
              onClick={() => void setFilter(c.app_id)}
            >
              {c.app_name} {c.unread > 0 && <span className="chip-count">{c.unread}</span>}
            </button>
          ))}
        </div>
        <label className="switch-row compact">
          <input
            type="checkbox"
            checked={unreadOnly}
            onChange={(e) => void setFilter(appId, e.target.checked)}
          />
          <span>Unread only</span>
        </label>
        <span className="spacer" />
        <button
          onClick={() => void markRead(null, appId)}
          disabled={(selected ? selected.unread : (state?.unread ?? 0)) === 0}
        >
          Mark {selected ? `${selected.app_name} ` : "all "}read
        </button>
        {confirmClear ? (
          <>
            <span className="muted small">{clearQuestion(selected, total)}</span>
            <button
              className="deny"
              onClick={() => {
                setConfirmClear(false);
                void clear(appId);
              }}
            >
              Delete
            </button>
            <button onClick={() => setConfirmClear(false)}>Keep</button>
          </>
        ) : (
          <button onClick={() => setConfirmClear(true)} disabled={total === 0}>
            {selected ? `Clear ${selected.app_name}` : "Clear all"}
          </button>
        )}
      </div>

      {items.length === 0 ? (
        <p className="muted">
          {state?.state === "on" ? "Nothing here yet." : "No notifications stored."}
        </p>
      ) : (
        <ul className="notif-list">
          {items.map((n) => (
            <li key={n.id}>
              <button
                className={`notif-item ${n.read_at === null ? "unread" : ""}`}
                onClick={() => n.read_at === null && void markRead(n.id, null)}
                title={n.read_at === null ? "Mark as read" : undefined}
              >
                <span className="notif-meta">
                  <span className="notif-app">{n.app_name}</span>
                  <span className="muted">{timeLabel(n.created_at)}</span>
                </span>
                <span className="notif-title">{n.title}</span>
                {n.body && <span className="notif-body">{n.body}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="muted small">
        Read from Windows notifications, with your permission. Message text stays on this PC unless
        you ask Kairos about your notifications in a chat.
      </p>
    </section>
  );
}
