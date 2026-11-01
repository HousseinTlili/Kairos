// What the island shows (M14 §1): one state and one line of words, from what the main window and the
// quick box each report. Pure, so the rules can be tested.

import { desktopTarget } from "../chat/desktop";
import type { Turn } from "../chat/turn";
import { speechText } from "../voice/model";
import type { VoicePhase } from "../voice/store";

export type IslandMood = "idle" | "listening" | "working" | "speaking";

/** What one window reports, sent on the `kairos-state` event. */
export interface KairosState {
  /** "automation": a line an automation shows on the pill (M17). */
  source: "main" | "quick" | "automation";
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
  /** How long a line stays after `doneAt` (default DONE_MS). */
  holdMs?: number;
  /** Kairos is working in another app (M20): what the bar's Stop stops. */
  desktop?: { taskId: string; runId: string } | null;
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
  const target = desktopTarget(turn);
  const base = {
    source,
    sessionId,
    heard: turn ? short(turn.input, 120) : null,
    answer: turn?.text ? answerStart(turn.text) : null,
    approval: false,
    doneAt,
    desktop:
      target && turn?.taskId && turn.runId ? { taskId: turn.taskId, runId: turn.runId } : null,
  };
  if (turn?.status === "waiting_for_approval") {
    return { ...base, mood: "working", text: "Kairos asks your leave", approval: true };
  }
  if (voice.phase === "listening") return { ...base, mood: "listening", text: "Listening…" };
  if (voice.phase === "transcribing") return { ...base, mood: "working", text: "Understanding…" };
  if (target) return { ...base, mood: "working", text: short(`Working in ${target}`, 34) };
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
    (s) =>
      s.mood !== "idle" ||
      s.approval ||
      (s.doneAt !== null && now - s.doneAt < (s.holdMs ?? DONE_MS)),
  );
  // An automation's line is only shown while it lasts; at rest, a window speaks.
  const pool = live.length > 0 ? live : states.filter((s) => s.source !== "automation");
  return (
    [...pool].sort(
      (a, b) =>
        Number(b.approval) - Number(a.approval) ||
        RANK[b.mood] - RANK[a.mood] ||
        (b.doneAt ?? 0) - (a.doneAt ?? 0),
    )[0] ?? null
  );
}

/** Wi-Fi bars on the top bar from the signal (D-034): Windows's own steps, roughly. */
export function wifiBars(signal: number | null): 0 | 1 | 2 | 3 {
  if (signal === null) return 3;
  if (signal >= 70) return 3;
  if (signal >= 40) return 2;
  if (signal >= 15) return 1;
  return 0;
}

/** "64 %, 2 h 10 min left" for the battery's tooltip. */
export function batteryWords(percent: number, state: string, emptyS: number | null): string {
  const level = `${Math.round(percent)} %`;
  if (state === "charging") return `${level}, charging`;
  if (state === "full") return `${level}, plugged in`;
  if (emptyS === null || state !== "discharging") return level;
  const minutes = Math.round(emptyS / 60);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return `${level}, ${h > 0 ? `${h} h ` : ""}${m} min left`;
}

/** Omarchy's clock: "Saturday 19:42" (its `%A %H:%M`), in the Master's own language and hours. */
export function clockText(d: Date): string {
  const day = d.toLocaleDateString([], { weekday: "long" });
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return `${day} ${time}`;
}

/**
 * A month as weeks of day numbers, Monday first, `null` outside the month: the calendar under the
 * clock.
 */
export function monthWeeks(year: number, month: number): (number | null)[][] {
  const first = new Date(year, month, 1);
  const days = new Date(year, month + 1, 0).getDate();
  const lead = (first.getDay() + 6) % 7; // Monday = 0
  const cells: (number | null)[] = [
    ...Array<null>(lead).fill(null),
    ...Array.from({ length: days }, (_, i) => i + 1),
  ];
  while (cells.length % 7) cells.push(null);
  const weeks: (number | null)[][] = [];
  for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));
  return weeks;
}

/** The volume after one turn of the mouse wheel on the audio module: 5 % a notch, as Omarchy. */
export function wheelVolume(level: number, deltaY: number): number {
  const step = deltaY < 0 ? 5 : -5;
  return Math.max(0, Math.min(100, Math.round((level + step) / 5) * 5));
}
