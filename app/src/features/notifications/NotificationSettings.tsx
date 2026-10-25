import { useEffect } from "react";
import { stateText, totalOf } from "./model";
import { useNotifications } from "./store";

export function NotificationSettings() {
  const { state, error, refresh, setEnabled } = useNotifications();

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!state) return <p className="muted">{error ?? "Reading notification settings…"}</p>;
  const status = stateText(state.state, state.access);

  return (
    <>
      <div className="settings-section">
        <h2>Windows notifications</h2>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={state.enabled}
            onChange={(e) => void setEnabled(e.target.checked)}
          />
          <span>
            Read my Windows notifications (missed calls and texts through Phone Link, Discord,
            Teams, WhatsApp, Outlook…)
          </span>
        </label>
        <p
          className={
            status.tone === "warn" ? "warn-text" : status.tone === "ok" ? "ok-text" : "muted"
          }
        >
          {status.text}
        </p>
        <p className="muted small">
          Only notifications that arrive after you switch this on are kept, and switching it off
          stops collection at once. Kairos never logs into these apps; it sees what the notification
          shows (sender and a preview). Stored notifications stay on this PC until you clear them (
          {totalOf(state.counts)} now).
        </p>
        <p className="muted small">
          When you ask about your notifications in a chat (&quot;anything I missed?&quot;), their
          senders and previews are sent to the AI model like the rest of the conversation. Kairos
          tells the model they are other people&apos;s words, to summarise and never to follow.
        </p>
      </div>
      {error && <p className="bad-text">{error}</p>}
    </>
  );
}
