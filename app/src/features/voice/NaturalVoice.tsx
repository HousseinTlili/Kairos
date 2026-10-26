import { useEffect, useState } from "react";
import {
  installNaturalVoice,
  onVoiceInstall,
  removeNaturalVoice,
  saveSpeechSettings,
  type CommandError,
  type Speaker,
  type VoiceSettings,
} from "../../lib/tauri";
import { SPEAKERS, megabytes } from "./model";
import { useVoice } from "./store";

/** Settings ▸ Voice ▸ the natural voice (M9 Step 8, D-025): Kokoro, offline, downloaded on click. */
export function NaturalVoice({ settings }: { settings: VoiceSettings }) {
  const { setSettings, loadSettings, speak } = useVoice();
  const [progress, setProgress] = useState<{ received: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  useEffect(() => {
    const stop = onVoiceInstall((event) => {
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
  const save = (natural: boolean, speaker: Speaker) =>
    void saveSpeechSettings(natural, speaker).then(setSettings).catch(fail);

  if (!settings.natural_installed) {
    const busy = settings.natural_installing || progress !== null;
    return (
      <div className="natural-voice">
        <p className="small">
          <strong>Natural voice (Kokoro).</strong> Sounds far more human than the Windows voices and
          works offline: nothing you hear is made online.
        </p>
        {busy ? (
          <p className="muted small">
            {progress
              ? `Downloading… ${megabytes(progress.received)} of ${megabytes(progress.total)}`
              : "Downloading…"}
          </p>
        ) : (
          <button
            onClick={() => {
              setError(null);
              void installNaturalVoice().catch(fail);
            }}
          >
            Download the natural voice ({megabytes(settings.natural_size_bytes)})
          </button>
        )}
        <p className="muted small">
          From k2-fsa&apos;s sherpa-onnx releases on GitHub, checked against their published
          checksums.
        </p>
        {error && <p className="bad-text">{error}</p>}
      </div>
    );
  }

  const natural = settings.engine === "natural";
  return (
    <div className="natural-voice">
      <label className="switch-row">
        <input
          type="radio"
          name="voice-engine"
          checked={natural}
          onChange={() => save(true, settings.speaker)}
        />
        <span>Natural voice (Kokoro, offline)</span>
      </label>
      <label className="switch-row">
        <input
          type="radio"
          name="voice-engine"
          checked={!natural}
          onChange={() => save(false, settings.speaker)}
        />
        <span>Windows voice</span>
      </label>
      {natural && (
        <div className="briefing-fields">
          <label>
            <span>Voice</span>
            <select
              value={settings.speaker}
              onChange={(e) => save(true, e.target.value as Speaker)}
            >
              {SPEAKERS.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      <div className="voice-test">
        <button
          onClick={() =>
            speak(
              "test",
              "Good evening, Master. The build finished without errors, and your notes are ready.",
            )
          }
        >
          Test the voice
        </button>
        {confirmRemove ? (
          <>
            <span className="muted small">Remove the natural voice?</span>
            <button
              className="deny"
              onClick={() => {
                setConfirmRemove(false);
                void removeNaturalVoice().then(setSettings).catch(fail);
              }}
            >
              Remove
            </button>
            <button onClick={() => setConfirmRemove(false)}>Keep</button>
          </>
        ) : (
          <button className="link" onClick={() => setConfirmRemove(true)}>
            Remove the natural voice
          </button>
        )}
      </div>
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
