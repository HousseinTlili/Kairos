import { useEffect, useState } from "react";
import {
  islandState,
  saveIslandSettings,
  type CommandError,
  type IslandSettings as Settings,
} from "../../lib/tauri";

/** Settings ▸ Kairos ▸ Island (M14): the pill at the top of the screen, and the wake panel. */
export function IslandSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void islandState()
      .then(setSettings)
      .catch((err: CommandError) => setError(err.message ?? String(err)));
  }, []);

  const save = (next: Settings) =>
    void saveIslandSettings(next.enabled, next.panel)
      .then(setSettings)
      .catch((err: CommandError) => setError(err.message ?? String(err)));

  return (
    <div className="settings-section">
      <h2>Island</h2>
      <p className="muted small">
        A small pill at the top of the screen shows what Kairos is doing: at rest, listening, at
        work, speaking, or waiting for your leave. Click it to ask something, hold its microphone to
        speak, right-click it for more. It steps aside for full-screen games, films and
        presentations.
      </p>
      {settings && (
        <>
          <label className="switch-row">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={(e) => save({ ...settings, enabled: e.target.checked })}
            />
            <span>Show the island</span>
          </label>
          <label className="switch-row">
            <input
              type="checkbox"
              checked={settings.panel}
              onChange={(e) => save({ ...settings, panel: e.target.checked })}
            />
            <span>When you say &ldquo;Kairos&rdquo;, show him at the side of the screen</span>
          </label>
        </>
      )}
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
