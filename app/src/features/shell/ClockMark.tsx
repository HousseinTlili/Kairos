import { useEffect, useState } from "react";

/**
 * The Kairos mark: a brass pocket-watch face whose hands show the real time ("the right
 * moment"). It is the one decorative element in the app.
 */
export function ClockMark({ size = 28 }: { size?: number }) {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const minutes = now.getMinutes();
  const hours = (now.getHours() % 12) + minutes / 60;
  const minuteAngle = minutes * 6;
  const hourAngle = hours * 30;

  return (
    <svg
      className="clock-mark"
      width={size}
      height={size}
      viewBox="0 0 32 32"
      role="img"
      aria-label={`Kairos, ${now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`}
    >
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
      <circle cx="16" cy="17" r="1.2" className="clock-pin" />
    </svg>
  );
}
