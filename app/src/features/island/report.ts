import { useEffect, useRef, useState } from "react";
import { emitKairosState } from "../../lib/tauri";
import { useChat } from "../chat/store";
import { useVoice } from "../voice/store";
import { DONE_MS, stateOf, type KairosState } from "./model";

const ENDED = ["completed", "incomplete", "cancelled", "failed", "interrupted"];

/**
 * The main window and the quick box each tell the island what they are doing (M14). Sent only when
 * it changes, and once more when the "Done" line should fade.
 */
export function useReportState(source: KairosState["source"]) {
  const turns = useChat((s) => s.turns);
  const sessionId = useChat((s) => s.sessionId);
  const phase = useVoice((s) => s.phase);
  const speaking = useVoice((s) => s.speaking !== null);
  const last = turns[turns.length - 1];
  const status = last?.status;
  const [doneAt, setDoneAt] = useState<number | null>(null);
  const [fadeTick, setFadeTick] = useState(0);
  const previous = useRef(status);
  const sent = useRef("");

  // A turn that just ended starts the "Done" line.
  useEffect(() => {
    const wasRunning = previous.current !== undefined && !ENDED.includes(previous.current);
    previous.current = status;
    if (status && ENDED.includes(status) && wasRunning) {
      const at = Date.now();
      setDoneAt(at);
      const timer = setTimeout(() => setFadeTick(at), DONE_MS + 50);
      return () => clearTimeout(timer);
    }
  }, [status]);

  useEffect(() => {
    const state = stateOf(source, last, { phase, speaking }, sessionId, Date.now(), doneAt);
    const key = JSON.stringify({ ...state, doneAt: null, fadeTick });
    if (key === sent.current) return;
    sent.current = key;
    void emitKairosState(state).catch(() => {});
  }, [source, last, phase, speaking, sessionId, doneAt, fadeTick]);
}
