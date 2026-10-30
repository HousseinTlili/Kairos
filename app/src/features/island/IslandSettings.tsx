import { useEffect, useState } from "react";
import {
  islandState,
  onIslandSettings,
  saveIslandSettings,
  setIslandHotkey,
  type CommandError,
  type IslandSettings as Settings,
} from "../../lib/tauri";

/** Settings ▸ Kairos ▸ Island (M14): the pill at the top of the screen, and the wake panel. */
export function IslandSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hotkey, setHotkey] = useState("");
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);

  useEffect(() => {
    const take = (next: Settings) => {
      setSettings(next);
      setHotkey(next.hotkey);
    };
    void islandState()
      .then(take)
      .catch((err: CommandError) => setError(err.message ?? String(err)));
    // The hotkey hides and shows it from anywhere: the switch below follows.
    const stop = onIslandSettings(take);
    return () => void stop.then((unlisten) => unlisten());
  }, []);

  const saveHotkey = () =>
    void setIslandHotkey(hotkey)
      .then((next) => {
        setSettings(next);
        setHotkey(next.hotkey);
        setHotkeyError(null);
      })
      .catch((err: CommandError) => setHotkeyError(err.message ?? String(err)));

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
          <h3>Hide and show</h3>
          <div className="cap-row">
            <input
              value={hotkey}
              onChange={(e) => setHotkey(e.target.value)}
              aria-label="Key that hides and shows the island"
              spellCheck={false}
            />
            <button onClick={saveHotkey} disabled={!hotkey.trim() || hotkey === settings.hotkey}>
              Use this key
            </button>
          </div>
          <p className="muted small">
            Press it anywhere to hide the island, and again to bring it back. Next to the quick box
            (Ctrl+Alt+K) and push-to-talk (Ctrl+Alt+J).
          </p>
          {hotkeyError && <p className="bad-text">{hotkeyError}</p>}
        </>
      )}
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
