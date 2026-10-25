import { useEffect, useRef } from "react";
import { isFinished, type Turn } from "../chat/turn";
import { useVoice } from "./store";

/**
 * Voice in a chat window (M9 §3): settings loaded, Esc cancels a recording or stops reading, and
 * with "Read answers aloud" on, each answer that finishes while the window is open is read once.
 * Answers already finished when a conversation opens are not.
 */
export function useVoiceInChat(turns: Turn[]) {
  const { loadSettings, cancelListening, stopSpeaking, speak } = useVoice();
  const readAloud = useVoice((s) => s.settings?.read_aloud ?? false);
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
      } else if (running.current.delete(turn.taskId) && readAloud && turn.text) {
        speak(turn.taskId, turn.text);
      }
    }
  }, [turns, readAloud, speak]);
}
