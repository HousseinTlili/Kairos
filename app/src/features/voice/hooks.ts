import { useEffect, useRef } from "react";
import { isFinished, type Turn } from "../chat/turn";
import { wakeFeed } from "../../lib/tauri";
import { WakeListener, micError } from "./recorder";
import { useVoice, wasSpoken } from "./store";

/**
 * Voice in a chat window (M9 §3): settings loaded, Esc cancels a recording or stops reading, and
 * an answer that finishes while the window is open is read once when the request was spoken
 * ("answer out loud when I speak", on by default) or when "Read every answer aloud" is on.
 * Answers already finished when a conversation opens are not.
 */
export function useVoiceInChat(turns: Turn[]) {
  const { loadSettings, cancelListening, stopSpeaking, speak } = useVoice();
  const readAloud = useVoice((s) => s.settings?.read_aloud ?? false);
  const answerSpoken = useVoice((s) => s.settings?.answer_spoken ?? true);
  const running = useRef(new Set<string>());

  useEffect(() => {
    void loadSettings();
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const { phase, speaking } = useVoice.getState();
      if (phase === "listening") void cancelListening();
      else if (speaking) stopSpeaking();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [loadSettings, cancelListening, stopSpeaking]);

  useEffect(() => {
    for (const turn of turns) {
      if (!turn.taskId) continue;
      if (!isFinished(turn.status)) {
        running.current.add(turn.taskId);
      } else if (running.current.delete(turn.taskId)) {
        const spoken = wasSpoken(turn.input);
        if (turn.text && (readAloud || (answerSpoken && spoken))) speak(turn.taskId, turn.text);
      }
    }
  }, [turns, readAloud, answerSpoken, speak]);
}

/**
 * The wake word (M9 Step 9), in the main window: while it is on, the microphone streams to
 * Kairos's spotter; "Kairos" opens the quick box, which does the listening. Off, nothing listens.
 */
export function useWakeWord() {
  const enabled = useVoice((s) => s.settings?.wake_enabled ?? false);
  const loadSettings = useVoice((s) => s.loadSettings);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    if (!enabled) return;
    const listener = new WakeListener((pcm) => wakeFeed(pcm).catch(() => false));
    let stopped = false;
    listener.start().catch((err: unknown) => {
      if (!stopped) useVoice.setState({ error: micError(err) });
    });
    return () => {
      stopped = true;
      void listener.stop();
    };
  }, [enabled]);
}
