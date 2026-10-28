// What the island shows (M14 §1): one state and one line of words, from what the main window and the
// quick box each report. Pure, so the rules can be tested.

import type { Turn } from "../chat/turn";
import { speechText } from "../voice/model";
import type { VoicePhase } from "../voice/store";

export type IslandMood = "idle" | "listening" | "working" | "speaking";

/** What one window reports, sent on the `kairos-state` event. */
export interface KairosState {
  source: "main" | "quick";
  mood: IslandMood;
  text: string;
  /** An approval waits: a click on the pill opens it. */
  approval: boolean;
  sessionId: string | null;
  /** The request as asked, and the start of its answer, for the wake panel. */
  heard: string | null;
  answer: string | null;
  /** When the answer ended (ms), so "Done" can fade. */
  doneAt: number | null;
}

/** How long "Done — …" stays on the pill after an answer. */
export const DONE_MS = 4000;
const RUNNING = ["sending", "running", "stopping"];

function short(text: string, max: number): string {
  const one = text.replace(/\s+/g, " ").trim();
  return one.length > max ? `${one.slice(0, max - 1)}…` : one;
}

/** The first words of an answer as they would be heard, for the panel and the "Done" line. */
export function answerStart(markdown: string, max = 240): string {
  return short(speechText(markdown), max);
}

/** One window's state, from its latest turn and its voice. */
export function stateOf(
  source: KairosState["source"],
  turn: Turn | undefined,
  voice: { phase: VoicePhase; speaking: boolean },
  sessionId: string | null,
  now: number,
  doneAt: number | null,
): KairosState {
  const base = {
    source,
    sessionId,
    heard: turn ? short(turn.input, 120) : null,
    answer: turn?.text ? answerStart(turn.text) : null,
    approval: false,
    doneAt,
  };
  if (turn?.status === "waiting_for_approval") {
    return { ...base, mood: "working", text: "Kairos asks your leave", approval: true };
  }
  if (voice.phase === "listening") return { ...base, mood: "listening", text: "Listening…" };
  if (voice.phase === "transcribing") return { ...base, mood: "working", text: "Understanding…" };
  if (turn && RUNNING.includes(turn.status)) {
    return { ...base, mood: "working", text: short(turn.input, 34) };
  }
  if (voice.speaking) return { ...base, mood: "speaking", text: "Speaking…" };
  if (turn && doneAt !== null && now - doneAt < DONE_MS) {
    const ending = turn.status === "completed" ? "Done" : "Stopped";
    return { ...base, mood: "idle", text: ending };
  }
  return { ...base, mood: "idle", text: "At your service" };
}

const RANK: Record<IslandMood, number> = { idle: 0, working: 1, speaking: 2, listening: 3 };

/** The pill shows the most important of the windows' states: an approval first, then the voice. */
export function mergeStates(states: KairosState[], now: number): KairosState | null {
  const live = states.filter(
    (s) => s.mood !== "idle" || s.approval || (s.doneAt !== null && now - s.doneAt < DONE_MS),
  );
  const pool = live.length > 0 ? live : states;
  return (
    [...pool].sort(
      (a, b) =>
        Number(b.approval) - Number(a.approval) ||
        RANK[b.mood] - RANK[a.mood] ||
        (b.doneAt ?? 0) - (a.doneAt ?? 0),
    )[0] ?? null
  );
}
