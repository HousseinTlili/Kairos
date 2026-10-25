import { useEffect, useState } from "react";
import { quickHotkey, setQuickHotkey, type CommandError } from "../../lib/tauri";
import { hotkeyLabel } from "./model";

export function QuickSettings() {
  const [hotkey, setHotkey] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    quickHotkey()
      .then((h) => {
        setHotkey(h);
        setDraft(h);
      })
      .catch((err: CommandError) => setError(err.message ?? String(err)));
  }, []);

  const save = () =>
    void setQuickHotkey(draft)
      .then((h) => {
        setHotkey(h);
        setError(null);
      })
      .catch((err: CommandError) => setError(err.message ?? String(err)));

  return (
    <div className="settings-section">
      <h2>Quick box</h2>
      <p className="muted small">
        Press {hotkey ? <strong>{hotkeyLabel(hotkey)}</strong> : "the hotkey"} anywhere to ask
        Kairos something without leaving what you&apos;re doing. Esc hides it.
      </p>
      <div className="cap-row">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          aria-label="Hotkey"
          spellCheck={false}
        />
        <button onClick={save} disabled={!draft.trim() || draft === hotkey}>
          Use this hotkey
        </button>
      </div>
      <p className="muted small">
        For example Ctrl+Alt+K, Ctrl+Shift+K or Super+J. Alt+Space is Windows&apos;s own window-menu
        key.
      </p>
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
