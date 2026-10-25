import { useChat } from "../chat/store";
import { useConnection } from "../connection/store";
import { useShell } from "../shell/store";
import { closeRequest } from "./alerts";
import { useSystem } from "./store";

/** The system alerts above the chat and the System page (M3 §7). */
export function AlertBanners() {
  const { alerts, dismissAlert } = useSystem();
  const connected = useConnection((s) => s.info?.status === "connected");
  const { newConversation, send } = useChat();
  const show = useShell((s) => s.show);
  if (alerts.length === 0) return null;

  return (
    <div className="alert-banners" role="region" aria-label="System alerts">
      {alerts.map((alert) => {
        const request = closeRequest(alert);
        return (
          <div
            key={alert.id}
            className={`alert-banner ${alert.kind === "battery_critical" ? "bad" : "warn"}`}
            role="alert"
          >
            <p>
              <strong>{alert.title}.</strong> {alert.message}
            </p>
            {request && (
              <button
                onClick={() => {
                  dismissAlert(alert.id);
                  newConversation();
                  show("chat");
                  void send(request);
                }}
                disabled={!connected}
                title={connected ? "Kairos asks you before closing it" : "Hermes isn't connected"}
              >
                Close it?
              </button>
            )}
            <button className="link" onClick={() => dismissAlert(alert.id)}>
              Dismiss
            </button>
          </div>
        );
      })}
    </div>
  );
}
