import { isFinished } from "../chat/turn";
import { useChat } from "../chat/store";
import { useVoice } from "../voice/store";

/** What Kairos is doing, as the seal shows it (docs/design/BRAND.md, Ornament 1). */
export type Presence = "idle" | "listening" | "working" | "speaking";

/** Kairos's state in this window: listening beats speaking beats working. */
export function usePresence(): { state: Presence; level: number } {
  const phase = useVoice((s) => s.phase);
  const level = useVoice((s) => s.level);
  const speaking = useVoice((s) => s.speaking !== null);
  const working = useChat((s) => s.turns.some((t) => !isFinished(t.status)));
  if (phase === "listening") return { state: "listening", level };
  if (phase === "transcribing") return { state: "working", level: 0 };
  if (speaking) return { state: "speaking", level: 0 };
  return { state: working ? "working" : "idle", level: 0 };
}

export const PRESENCE_WORDS: Record<Presence, string> = {
  idle: "",
  listening: "Listening",
  working: "At work",
  speaking: "Speaking",
};
