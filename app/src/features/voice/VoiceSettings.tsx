import { useEffect, useState } from "react";
import {
  deleteVoiceModel,
  downloadVoiceModel,
  onVoiceModel,
  pttHotkey,
  setPttHotkey,
  type CommandError,
  type VoiceModelId,
} from "../../lib/tauri";
import { hotkeyLabel } from "../quick/model";
import { LANGUAGE_NAMES, megabytes, missingVoices } from "./model";
import { MicButton, VoiceStatus } from "./VoiceControls";
import { NaturalVoice } from "./NaturalVoice";
import { WakeWord } from "./WakeWord";
import { useVoice } from "./store";

function useWindowsVoices(): SpeechSynthesisVoice[] {
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);
  useEffect(() => {
    if (typeof speechSynthesis === "undefined") return;
    const read = () => setVoices(speechSynthesis.getVoices());
    read();
    speechSynthesis.addEventListener("voiceschanged", read);
    return () => speechSynthesis.removeEventListener("voiceschanged", read);
  }, []);
  return voices;
}

export function VoiceSettings() {
  const { settings, error, loadSettings, saveSettings, setSettings, speak } = useVoice();
  const [progress, setProgress] = useState<{
    id: VoiceModelId;
    received: number;
    total: number;
  } | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<VoiceModelId | null>(null);
  const [hotkey, setHotkey] = useState("");
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [heardText, setHeardText] = useState<string | null>(null);
  const voices = useWindowsVoices();

  useEffect(() => {
    void loadSettings();
    void pttHotkey().then(setHotkey);
    const stop = onVoiceModel((event) => {
      if (event.phase === "downloading") setProgress(event);
      else {
        setProgress(null);
        setDownloadError(event.phase === "failed" ? event.message : null);
        void loadSettings();
      }
    });
    return () => void stop.then((unlisten) => unlisten());
  }, [loadSettings]);

  if (!settings) return <p className="muted">{error ?? "Reading voice settings…"}</p>;
  const ready = settings.models.some((m) => m.id === settings.model && m.ready);
  const downloading = settings.downloading || progress !== null;
  const missing = missingVoices(voices);

  const download = (id: VoiceModelId) => {
    setDownloadError(null);
    setProgress({
      id,
      received: 0,
      total: settings.models.find((m) => m.id === id)?.size_bytes ?? 0,
    });
    void downloadVoiceModel(id).catch((err: CommandError) => {
      setProgress(null);
      setDownloadError(err.message);
    });
  };

  const saveHotkey = () =>
    void setPttHotkey(hotkey)
      .then((h) => {
        setHotkey(h);
        setHotkeyError(null);
        void loadSettings();
      })
      .catch((err: CommandError) => setHotkeyError(err.message));

  return (
    <>
      <div className="settings-section">
        <h2>Speaking to Kairos</h2>
        <p className="muted small">
          Hold <strong>{hotkeyLabel(settings.ptt_hotkey)}</strong> anywhere, speak, and let go: the
          quick box sends what you said. In a chat, click the mic. Speech is turned into text on
          this PC; no audio is saved or sent anywhere.
        </p>
        <ul className="voice-models">
          {settings.models.map((model) => {
            const active = progress?.id === model.id;
            return (
              <li key={model.id}>
                <label className="switch-row">
                  <input
                    type="radio"
                    name="voice-model"
                    checked={settings.model === model.id}
                    onChange={() => void saveSettings({ model: model.id })}
                  />
                  <span>
                    {model.label}{" "}
                    <span className="muted small">({megabytes(model.size_bytes)})</span>
                  </span>
                </label>
                <span className="voice-model-state">
                  {model.ready ? (
                    confirmDelete === model.id ? (
                      <>
                        <span className="muted small">Delete this model?</span>
                        <button
                          className="deny"
                          onClick={() => {
                            setConfirmDelete(null);
                            void deleteVoiceModel(model.id).then(setSettings);
                          }}
                        >
                          Delete
                        </button>
                        <button onClick={() => setConfirmDelete(null)}>Keep</button>
                      </>
                    ) : (
                      <>
                        <span className="ok-text small">Ready</span>
                        <button className="link" onClick={() => setConfirmDelete(model.id)}>
                          Delete
                        </button>
                      </>
                    )
                  ) : active ? (
                    <span className="muted small">
                      Downloading… {megabytes(progress.received)} of {megabytes(progress.total)}
                    </span>
                  ) : (
                    <button onClick={() => download(model.id)} disabled={downloading}>
                      Download
                    </button>
                  )}
                </span>
              </li>
            );
          })}
        </ul>
        <p className="muted small">
          Models come from the whisper.cpp project on huggingface.co and are checked against their
          published checksum.
        </p>
        {downloadError && <p className="bad-text">{downloadError}</p>}
        <div className="briefing-fields">
          <label>
            <span>Language</span>
            <select
              value={settings.language ?? ""}
              onChange={(e) => void saveSettings({ language: e.target.value || null })}
            >
              <option value="">Detect each time</option>
              {Object.entries(LANGUAGE_NAMES).map(([code, name]) => (
                <option key={code} value={code}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="voice-test">
          <MicButton onText={setHeardText} disabled={!ready} />
          <span className="muted small">
            {ready
              ? "Test: click, say something, click again."
              : "Download the chosen model to test."}
          </span>
        </div>
        <VoiceStatus />
        {heardText && <p className="voice-heard">“{heardText}”</p>}
      </div>

      <WakeWord settings={settings} />

      <div className="settings-section">
        <h2>Push-to-talk key</h2>
        <div className="cap-row">
          <input
            value={hotkey}
            onChange={(e) => setHotkey(e.target.value)}
            aria-label="Push-to-talk key"
            spellCheck={false}
          />
          <button onClick={saveHotkey} disabled={!hotkey.trim() || hotkey === settings.ptt_hotkey}>
            Use this key
          </button>
        </div>
        <p className="muted small">
          Held, not pressed: for example Ctrl+Alt+J or Ctrl+Shift+Space.
        </p>
        {hotkeyError && <p className="bad-text">{hotkeyError}</p>}
      </div>

      <div className="settings-section">
        <h2>Reading answers aloud</h2>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={settings.answer_spoken}
            onChange={(e) => void saveSettings({ answer_spoken: e.target.checked })}
          />
          <span>Answer out loud when I speak</span>
        </label>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={settings.read_aloud}
            onChange={(e) => void saveSettings({ read_aloud: e.target.checked })}
          />
          <span>Read typed questions' answers aloud too</span>
        </label>
        <p className="muted small">
          Any answer can also be read with the speaker button under it. Esc or a new request stops
          it. Code is skipped.
        </p>
        <NaturalVoice settings={settings} />
        {settings.engine === "windows" && (
          <div className="voice-test">
            <button onClick={() => speak("test", "Hello, I'm Kairos. This is how I sound.")}>
              Test the Windows voice
            </button>
          </div>
        )}
        {settings.engine === "windows" && missing.length > 0 && (
          <p className="muted small">
            No Windows voice for {missing.map((code) => LANGUAGE_NAMES[code]).join(" or ")} yet, so
            those answers are read with an English voice. Add one in Windows Settings ▸ Time &amp;
            language ▸ Speech ▸ Manage voices.
          </p>
        )}
      </div>
    </>
  );
}
