import { useEffect, useState } from "react";
import {
  installWakeWord,
  onVoiceInstall,
  saveWakeSettings,
  type CommandError,
  type VoiceSettings,
  type WakeSensitivity,
} from "../../lib/tauri";
import { megabytes } from "./model";
import { useVoice } from "./store";

const SENSITIVITY: { id: WakeSensitivity; label: string }[] = [
  { id: "strict", label: "Strict: fewer false wakes, may need repeating" },
  { id: "normal", label: "Normal" },
  { id: "eager", label: "Eager: hears you from further, wakes by mistake more" },
];

/** Settings ▸ Voice ▸ the wake word "Kairos" (M9 Step 9): offline, off until you switch it on. */
export function WakeWord({ settings }: { settings: VoiceSettings }) {
  const { setSettings, loadSettings } = useVoice();
  const [progress, setProgress] = useState<{ received: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const stop = onVoiceInstall((event) => {
      if (event.what !== "wake") return;
      if (event.phase === "downloading") setProgress(event);
      else {
        setProgress(null);
        setError(event.phase === "failed" ? event.message : null);
        void loadSettings();
      }
    });
    return () => void stop.then((unlisten) => unlisten());
  }, [loadSettings]);

  const fail = (err: CommandError) => setError(err.message ?? String(err));
  const save = (enabled: boolean, sensitivity: WakeSensitivity) =>
    void saveWakeSettings(enabled, sensitivity).then(setSettings).catch(fail);

  return (
    <div className="settings-section">
      <h2>Wake word</h2>
      <p className="muted small">
        Say <strong>&ldquo;Kairos&rdquo;</strong> (or &ldquo;Hey Kairos&rdquo;) and the quick box
        opens and listens; it sends what you say when you pause. It runs on this PC: nothing is
        recorded or sent while it waits for its name. When you start speaking, the speech model
        checks the first two seconds for the name, so it works with your accent and costs a little
        processor time while people talk.
      </p>
      {!settings.wake_installed ? (
        settings.wake_installing || progress ? (
          <p className="muted small">
            {progress
              ? `Downloading… ${megabytes(progress.received)} of ${megabytes(progress.total)}`
              : "Downloading…"}
          </p>
        ) : (
          <button
            onClick={() => {
              setError(null);
              void installWakeWord().catch(fail);
            }}
          >
            Download the wake word ({megabytes(settings.wake_size_bytes)})
          </button>
        )
      ) : (
        <>
          <label className="switch-row">
            <input
              type="checkbox"
              checked={settings.wake_enabled}
              onChange={(e) => save(e.target.checked, settings.wake_sensitivity)}
            />
            <span>Listen for &ldquo;Kairos&rdquo;</span>
          </label>
          <div className="briefing-fields">
            <label>
              <span>Sensitivity</span>
              <select
                value={settings.wake_sensitivity}
                onChange={(e) => save(settings.wake_enabled, e.target.value as WakeSensitivity)}
              >
                {SENSITIVITY.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="muted small">
            While it listens, Windows shows the microphone as in use. Switch it off whenever you
            like; Kairos then hears nothing until you speak to it with the key or the mic button.
          </p>
        </>
      )}
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
