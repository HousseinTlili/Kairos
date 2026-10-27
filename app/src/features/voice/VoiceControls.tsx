import { useEffect } from "react";
import { MicIcon, SpeakerIcon, StopIcon } from "../shell/icons";
import { useShell } from "../shell/store";
import { LANGUAGE_NAMES } from "./model";
import { useVoice, whenRecordingLimitReached } from "./store";

/**
 * The mic button in a chat box (M9 §3): click to start, click again to stop and send what was
 * said. `onText` gets the transcript.
 */
export function MicButton({
  onText,
  disabled,
}: {
  onText: (text: string) => void;
  disabled?: boolean;
}) {
  const { phase, level, startListening, finishListening } = useVoice();

  useEffect(() => {
    if (phase !== "listening") return;
    whenRecordingLimitReached(() => void finishListening().then((text) => text && onText(text)));
    return () => whenRecordingLimitReached(null);
  }, [phase, finishListening, onText]);

  const click = () => {
    if (phase === "listening") void finishListening().then((text) => text && onText(text));
    else if (phase === "idle") void startListening();
  };

  const label =
    phase === "listening" ? "Stop and send" : phase === "transcribing" ? "Transcribing…" : "Speak";
  return (
    <button
      type="button"
      className={`composer-button mic ${phase}`}
      onClick={click}
      disabled={disabled || phase === "transcribing"}
      aria-label={label}
      title={label}
      style={
        phase === "listening"
          ? { boxShadow: `0 0 0 ${2 + level * 6}px var(--mic-ring)` }
          : undefined
      }
    >
      <MicIcon />
    </button>
  );
}

/** What the voice is doing, under a chat box: listening, transcribing, or what went wrong. */
export function VoiceStatus() {
  const { phase, error, heard, auto, cancelListening } = useVoice();
  const { showSettings } = useShell();
  if (phase === "listening") {
    return (
      <p className="voice-status" role="status">
        {auto
          ? "Listening… it sends when you pause. Esc to cancel."
          : "Listening… click the mic again to send, Esc to cancel."}{" "}
        <button className="link" onClick={() => void cancelListening()}>
          Cancel
        </button>
      </p>
    );
  }
  if (phase === "transcribing") {
    return (
      <p className="voice-status" role="status">
        Transcribing…
      </p>
    );
  }
  if (error) {
    const missing = error.includes("Download a voice model");
    return (
      <p className="voice-status bad-text" role="status">
        {error}{" "}
        {missing && (
          <button className="link" onClick={() => showSettings("voice")}>
            Open Settings ▸ Voice
          </button>
        )}
      </p>
    );
  }
  return heard && LANGUAGE_NAMES[heard] ? (
    <p className="voice-status muted">Heard in {LANGUAGE_NAMES[heard]}</p>
  ) : null;
}

/** Reads one answer aloud, or stops it (M9 §3: only when asked). */
export function SpeakButton({ id, text }: { id: string; text: string }) {
  const { speaking, speak, stopSpeaking } = useVoice();
  const active = speaking === id;
  return (
    <button
      type="button"
      className={`speak-button ${active ? "active" : ""}`}
      onClick={() => (active ? stopSpeaking() : speak(id, text))}
      aria-label={active ? "Stop reading" : "Read aloud"}
      title={active ? "Stop reading" : "Read aloud"}
    >
      {active ? <StopIcon /> : <SpeakerIcon />}
    </button>
  );
}
