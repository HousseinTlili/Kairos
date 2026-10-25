import { useEffect } from "react";
import { describePresence, hermesAtLogin } from "./format";
import { useStartup } from "./store";

export function StartupSettings() {
  const { state, busy, error, load, setKairosAtLogin } = useStartup();

  useEffect(() => {
    void load();
  }, [load]);

  if (!state) return <p className="muted">{error ?? "Reading startup settings…"}</p>;
  const hermes = hermesAtLogin(state);

  return (
    <>
      <div className="settings-section">
        <h2>Kairos</h2>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={state.kairos_at_login ?? false}
            disabled={busy || state.kairos_at_login === null}
            onChange={(e) => void setKairosAtLogin(e.target.checked)}
          />
          <span>Start Kairos when I log in to Windows (it waits in the tray)</span>
        </label>
        <p className="muted small">
          Closing the window keeps Kairos in the tray, so alerts keep working; use Quit in the tray
          menu to end it.
          {state.launched_at_login && " This time it was started at login."}
        </p>
      </div>

      <div className="settings-section">
        <h2>Hermes</h2>
        <p className={`setting-value ${hermes.ok === false ? "warn-text" : ""}`}>{hermes.text}</p>
        {hermes.ok === false && (
          <>
            <p className="muted small">
              Kairos needs Hermes running. To have Windows start it at login, run this in a terminal
              from the Kairos folder (it asks before changing anything):
            </p>
            <code className="command">
              powershell -NoProfile -ExecutionPolicy Bypass -File
              scripts/install-gateway-autostart.ps1
            </code>
          </>
        )}
      </div>

      <div className="settings-section">
        <h2>Recently</h2>
        {state.recent.length === 0 ? (
          <p className="muted small">Nothing recorded yet.</p>
        ) : (
          <ul className="presence-list">
            {state.recent.map((event) => (
              <li key={`${event.at}-${event.kind}`}>
                <span>{describePresence(event)}</span>
                <span className="muted">{new Date(event.at * 1000).toLocaleString()}</span>
              </li>
            ))}
          </ul>
        )}
        <p className="muted small">
          Kairos notes when it starts, when the PC wakes from sleep and when you unlock it. From M5,
          that is when it will offer a welcome-back briefing.
        </p>
      </div>
      {error && <p className="bad-text">{error}</p>}
    </>
  );
}
