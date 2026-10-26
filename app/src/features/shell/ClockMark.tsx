import { useEffect, useState } from "react";
import { isFinished } from "../chat/turn";
import { useChat } from "../chat/store";
import { useVoice } from "../voice/store";

/** What Kairos is doing, as the Dial shows it (docs/design/BRAND.md, principle 1). */
export type DialState = "idle" | "listening" | "working" | "speaking";

/** Kairos's state in this window: listening beats speaking beats working. */
export function useDialState(): { state: DialState; level: number } {
  const phase = useVoice((s) => s.phase);
  const level = useVoice((s) => s.level);
  const speaking = useVoice((s) => s.speaking !== null);
  const working = useChat((s) => s.turns.some((t) => !isFinished(t.status)));
  if (phase === "listening") return { state: "listening", level };
  if (phase === "transcribing") return { state: "working", level: 0 };
  if (speaking) return { state: "speaking", level: 0 };
  return { state: working ? "working" : "idle", level: 0 };
}

export const DIAL_WORDS: Record<DialState, string> = {
  idle: "",
  listening: "Listening",
  working: "Working",
  speaking: "Speaking",
};

/**
 * The Dial: a gilt watch face whose hands show the real time ("the right moment"). It is the one
 * element that moves: a lume ring follows the voice while listening, a lume second hand sweeps
 * while Kairos works, and the ring breathes while it speaks.
 */
export function ClockMark({
  size = 28,
  state = "idle",
  level = 0,
}: {
  size?: number;
  state?: DialState;
  level?: number;
}) {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const minutes = now.getMinutes();
  const hours = (now.getHours() % 12) + minutes / 60;
  const minuteAngle = minutes * 6;
  const hourAngle = hours * 30;
  const time = now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const words = DIAL_WORDS[state];

  return (
    <svg
      className={`clock-mark ${state}`}
      width={size}
      height={size}
      viewBox="-2 -1 36 36"
      role="img"
      aria-label={words ? `Kairos: ${words.toLowerCase()}` : `Kairos, ${time}`}
    >
      <circle
        cx="16"
        cy="17"
        r="15.2"
        className="clock-ring"
        style={state === "listening" ? { opacity: 0.35 + Math.min(1, level) * 0.65 } : undefined}
      />
      <circle cx="16" cy="17" r="12.5" className="clock-case" />
      <circle cx="16" cy="17" r="10" className="clock-face" />
      <rect x="14.5" y="1.5" width="3" height="3" rx="1" className="clock-crown" />
      {[0, 90, 180, 270].map((angle) => (
        <line
          key={angle}
          x1="16"
          y1="8.6"
          x2="16"
          y2="10.2"
          className="clock-tick"
          transform={`rotate(${angle} 16 17)`}
        />
      ))}
      <line
        x1="16"
        y1="17"
        x2="16"
        y2="11.8"
        className="clock-hand hour"
        transform={`rotate(${hourAngle} 16 17)`}
      />
      <line
        x1="16"
        y1="17"
        x2="16"
        y2="9.4"
        className="clock-hand minute"
        transform={`rotate(${minuteAngle} 16 17)`}
      />
      <line x1="16" y1="18.5" x2="16" y2="8.2" className="clock-hand second" />
      <circle cx="16" cy="17" r="1.2" className="clock-pin" />
    </svg>
  );
}
