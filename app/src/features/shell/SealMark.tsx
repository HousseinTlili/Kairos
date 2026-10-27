import { PRESENCE_WORDS, type Presence } from "./presence";

// The wax's edge: a circle pressed unevenly, sixteen soft lobes.
const WAX = (() => {
  const points: string[] = [];
  const lobes = 16;
  for (let i = 0; i <= lobes * 2; i++) {
    const angle = (i / (lobes * 2)) * Math.PI * 2;
    const r = i % 2 === 0 ? 13.4 : 12.5;
    points.push(
      `${(16 + r * Math.cos(angle)).toFixed(2)} ${(16 + r * Math.sin(angle)).toFixed(2)}`,
    );
  }
  return `M${points.join(" L")}Z`;
})();

/**
 * The seal: crimson wax, a gold ring, a blackletter K. It is the one element that moves: listening,
 * a vert ring follows the voice; working, a gold ring turns; speaking, the ring breathes.
 */
export function SealMark({
  size = 30,
  state = "idle",
  level = 0,
}: {
  size?: number;
  state?: Presence;
  level?: number;
}) {
  const words = PRESENCE_WORDS[state];
  return (
    <svg
      className={`seal-mark ${state}`}
      width={size}
      height={size}
      viewBox="-2 -2 36 36"
      role="img"
      aria-label={words ? `Kairos: ${words.toLowerCase()}` : "Kairos"}
    >
      <circle
        cx="16"
        cy="16"
        r="16.4"
        className="seal-ring"
        style={state === "listening" ? { opacity: 0.35 + Math.min(1, level) * 0.65 } : undefined}
      />
      <path d={WAX} className="seal-wax" />
      <circle cx="16" cy="16" r="10.2" className="seal-inner" />
      <text x="16" y="21.2" textAnchor="middle" className="seal-letter">
        K
      </text>
    </svg>
  );
}
