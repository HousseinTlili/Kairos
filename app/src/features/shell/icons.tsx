// Stroke icons drawn inline, so the app needs no icon library.
import type { SVGProps } from "react";

function Icon({ children, ...props }: SVGProps<SVGSVGElement>) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  );
}

export const PlusIcon = () => (
  <Icon>
    <path d="M12 5v14M5 12h14" />
  </Icon>
);

export const TrashIcon = () => (
  <Icon>
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" />
  </Icon>
);

export const SettingsIcon = () => (
  <Icon>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </Icon>
);

export const SendIcon = () => (
  <Icon>
    <path d="M12 19V5M5 12l7-7 7 7" />
  </Icon>
);

export const StopIcon = () => (
  <Icon>
    <rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" />
  </Icon>
);

export const TerminalIcon = () => (
  <Icon>
    <path d="M4 17l6-5-6-5M12 19h8" />
  </Icon>
);

export const ShieldIcon = () => (
  <Icon>
    <path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6l8-3z" />
  </Icon>
);

export const ChevronIcon = ({ open }: { open: boolean }) => (
  <Icon style={{ transform: open ? "rotate(90deg)" : undefined, transition: "transform 120ms" }}>
    <path d="M9 6l6 6-6 6" />
  </Icon>
);

export const LockIcon = () => (
  <Icon>
    <rect x="5" y="11" width="14" height="10" rx="2" />
    <path d="M8 11V8a4 4 0 0 1 8 0v3" />
  </Icon>
);

export const GaugeIcon = () => (
  <Icon>
    <path d="M4 15a8 8 0 1 1 16 0" />
    <path d="M12 15l4-5" />
    <circle cx="12" cy="15" r="1" />
  </Icon>
);

export const BellIcon = () => (
  <Icon>
    <path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z" />
    <path d="M10 20a2 2 0 0 0 4 0" />
  </Icon>
);

export const SunIcon = () => (
  <Icon>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  </Icon>
);

export const CalendarIcon = () => (
  <Icon>
    <rect x="4" y="5" width="16" height="15" rx="2" />
    <path d="M4 10h16M9 3v4M15 3v4M12 13v3l2 1" />
  </Icon>
);

export const MicIcon = () => (
  <Icon>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
  </Icon>
);

export const SpeakerIcon = () => (
  <Icon>
    <path d="M4 9v6h4l5 4V5L8 9H4z" />
    <path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12" />
  </Icon>
);

export const SidebarIcon = () => (
  <Icon>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M9 4v16" />
  </Icon>
);

// The island's tabs and media controls (M15).

export const HomeIcon = () => (
  <Icon>
    <path d="M4 11l8-7 8 7v9H14v-6h-4v6H4z" />
  </Icon>
);

export const MusicIcon = () => (
  <Icon>
    <path d="M9 18V6l11-2v12" />
    <circle cx="6.5" cy="18" r="2.5" />
    <circle cx="17.5" cy="16" r="2.5" />
  </Icon>
);

export const ImageIcon = () => (
  <Icon>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="9" cy="10" r="2" />
    <path d="M21 16l-5-5-9 9" />
  </Icon>
);

export const GridIcon = () => (
  <Icon>
    <rect x="4" y="4" width="6.5" height="6.5" rx="1.5" />
    <rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5" />
    <rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5" />
    <rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5" />
  </Icon>
);

export const PlayIcon = () => (
  <Icon fill="currentColor" stroke="none">
    <path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" />
  </Icon>
);

export const PauseIcon = () => (
  <Icon fill="currentColor" stroke="none">
    <rect x="6" y="5" width="4" height="14" rx="1.2" />
    <rect x="14" y="5" width="4" height="14" rx="1.2" />
  </Icon>
);

export const NextIcon = () => (
  <Icon fill="currentColor" stroke="none">
    <path d="M5 6.2v11.6a.8.8 0 0 0 1.25.66L14.5 12.7a.8.8 0 0 0 0-1.4L6.25 5.54A.8.8 0 0 0 5 6.2z" />
    <rect x="16.5" y="5" width="2.5" height="14" rx="1" />
  </Icon>
);

export const PreviousIcon = () => (
  <Icon fill="currentColor" stroke="none" style={{ transform: "scaleX(-1)" }}>
    <path d="M5 6.2v11.6a.8.8 0 0 0 1.25.66L14.5 12.7a.8.8 0 0 0 0-1.4L6.25 5.54A.8.8 0 0 0 5 6.2z" />
    <rect x="16.5" y="5" width="2.5" height="14" rx="1" />
  </Icon>
);

// The top bar (D-034).

export const SparkleIcon = () => (
  <Icon fill="currentColor" stroke="none">
    <path d="M11 3l1.9 5.1L18 10l-5.1 1.9L11 17l-1.9-5.1L4 10l5.1-1.9z" />
    <path d="M18.5 14l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z" />
  </Icon>
);

export const WaveIcon = () => (
  <Icon>
    <path d="M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4" />
  </Icon>
);

/** Wi-Fi by signal: three arcs, the ones above the strength drawn faint. */
export const WifiIcon = ({ bars }: { bars: 0 | 1 | 2 | 3 }) => (
  <Icon>
    <path d="M2.5 9a14 14 0 0 1 19 0" opacity={bars >= 3 ? 1 : 0.3} />
    <path d="M5.5 12.5a9.5 9.5 0 0 1 13 0" opacity={bars >= 2 ? 1 : 0.3} />
    <path d="M8.5 16a5 5 0 0 1 7 0" opacity={bars >= 1 ? 1 : 0.3} />
    <circle cx="12" cy="19.2" r="1" fill="currentColor" stroke="none" />
  </Icon>
);

/** The battery filled to its level, with a bolt while charging. */
export const BatteryIcon = ({ percent, charging }: { percent: number; charging: boolean }) => (
  <Icon width="20" height="16">
    <rect x="2" y="7" width="17" height="10" rx="2.2" />
    <path d="M21.5 10.5v3" />
    <rect
      x="4"
      y="9"
      width={Math.max(1, Math.round((13 * Math.min(100, Math.max(0, percent))) / 100))}
      height="6"
      rx="1"
      fill="currentColor"
      stroke="none"
    />
    {charging && <path d="M11.5 8.5l-2 4h3l-2 3.5" strokeWidth="1.4" stroke="var(--bg, #111)" />}
  </Icon>
);
