import { useEffect, useState } from "react";
import {
  barOpen,
  barSetMuted,
  barSetVolume,
  barSwitchDesktop,
  showMainWindow,
  type Desktops,
  type SystemSnapshot,
  type Volume,
} from "../../lib/tauri";
import {
  BatteryIcon,
  BellIcon,
  BluetoothIcon,
  CpuIcon,
  VolumeIcon,
  WifiIcon,
} from "../shell/icons";
import { batteryWords, clockText, monthWeeks, wheelVolume, wifiBars } from "./model";

// The top bar's modules, after Omarchy's (D-036): each shows one thing and does one thing on a
// click, as its Waybar module does there.

/** Omarchy's workspaces: one button per virtual desktop, the one in front lit; a click goes there. */
export function Workspaces({
  desktops,
  onSwitched,
}: {
  desktops: Desktops | null;
  onSwitched: (d: Desktops) => void;
}) {
  if (!desktops) return null;
  return (
    <nav className="bar-workspaces" aria-label="Desktops">
      {Array.from({ length: desktops.count }, (_, i) => (
        <button
          key={i}
          className={i === desktops.current ? "active" : ""}
          aria-current={i === desktops.current ? "true" : undefined}
          title={`Desktop ${i + 1}`}
          onClick={() =>
            void barSwitchDesktop(i)
              .then(onSwitched)
              .catch(() => {})
          }
        >
          {i + 1}
        </button>
      ))}
    </nav>
  );
}

/** The time, re-read every 15 s: one text change a minute, no animation. */
function useNow() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

/** Omarchy's centre: the clock ("Saturday 19:42"); a click opens the calendar. */
export function CentreClock({ onOpen }: { onOpen: () => void }) {
  const now = useNow();
  return (
    <button className="bar-clock" onClick={onOpen} title="Calendar">
      {clockText(now)}
    </button>
  );
}

function Module({
  title,
  onClick,
  onContextMenu,
  onWheel,
  active,
  children,
}: {
  title: string;
  onClick: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  onWheel?: (e: React.WheelEvent) => void;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      className={`bar-module ${active ? "active" : ""}`}
      title={title}
      aria-label={title}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onWheel={onWheel}
    >
      {children}
    </button>
  );
}

export type Pop = "volume" | "battery" | "calendar";

/**
 * Omarchy's right side: Bluetooth, the network, audio, the CPU and the battery, with Kairos's bell
 * before them. Audio: a click opens the slider, the wheel turns it 5 % a notch, a right-click
 * mutes. The CPU opens Kairos's System page, as Omarchy's opens btop.
 */
export function SystemModules({
  snapshot,
  unread,
  volume,
  onVolume,
  open,
  onPop,
}: {
  snapshot: SystemSnapshot | null;
  unread: number;
  volume: Volume | null;
  onVolume: (v: Volume) => void;
  open: Pop | null;
  onPop: (pop: Pop) => void;
}) {
  const wifi = snapshot?.wifi;
  const battery = snapshot?.battery;
  const cpu = snapshot?.cpu.total;
  return (
    <div className="bar-modules">
      <Module
        title={unread ? `${unread} unread notifications` : "Notifications"}
        onClick={() => void showMainWindow("notifications")}
      >
        <BellIcon />
        {unread > 0 && <span className="topbar-badge">{unread > 99 ? "99+" : unread}</span>}
      </Module>
      <Module title="Bluetooth" onClick={() => void barOpen("bluetooth").catch(() => {})}>
        <BluetoothIcon />
      </Module>
      <Module
        title={
          wifi
            ? `${wifi.ssid}${wifi.signal_percent !== null ? `, ${wifi.signal_percent} %` : ""}`
            : "Network"
        }
        onClick={() => void barOpen("wifi").catch(() => {})}
      >
        <WifiIcon bars={wifi ? wifiBars(wifi.signal_percent) : 0} />
      </Module>
      {volume && (
        <Module
          title={volume.muted ? "Muted" : `Volume ${volume.level} %`}
          active={open === "volume"}
          onClick={() => onPop("volume")}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            void barSetMuted(!volume.muted)
              .then(onVolume)
              .catch(() => {});
          }}
          onWheel={(e) =>
            void barSetVolume(wheelVolume(volume.level, e.deltaY))
              .then(onVolume)
              .catch(() => {})
          }
        >
          <VolumeIcon level={volume.level} muted={volume.muted} />
          <span className="bar-figure">{volume.muted ? "—" : `${volume.level}%`}</span>
        </Module>
      )}
      {cpu != null && (
        <Module title="CPU: open System" onClick={() => void showMainWindow("system")}>
          <CpuIcon />
          <span className="bar-figure">{Math.round(cpu)}%</span>
        </Module>
      )}
      {battery && (
        <Module
          title={batteryWords(battery.percent, battery.state, battery.time_to_empty_s)}
          active={open === "battery"}
          onClick={() => onPop("battery")}
        >
          <span
            className={`bar-battery ${battery.percent <= 20 && battery.state === "discharging" ? "low" : ""}`}
          >
            <BatteryIcon percent={battery.percent} charging={battery.state === "charging"} />
          </span>
          <span className="bar-figure">{Math.round(battery.percent)}%</span>
        </Module>
      )}
    </div>
  );
}

/** Omarchy's audio mixer, the Windows way: the speakers' level, mute, and the full mixer. */
export function VolumePopover({
  volume,
  onVolume,
}: {
  volume: Volume | null;
  onVolume: (v: Volume) => void;
}) {
  if (!volume) return null;
  return (
    <section className="bar-pop right" aria-label="Volume">
      <header>
        <VolumeIcon level={volume.level} muted={volume.muted} />
        <strong>Speakers</strong>
        <span className="muted">{volume.muted ? "Muted" : `${volume.level} %`}</span>
      </header>
      <input
        type="range"
        min={0}
        max={100}
        step={1}
        value={volume.level}
        aria-label="Volume"
        onChange={(e) =>
          void barSetVolume(Number(e.target.value))
            .then(onVolume)
            .catch(() => {})
        }
      />
      <div className="bar-pop-actions">
        <button
          onClick={() =>
            void barSetMuted(!volume.muted)
              .then(onVolume)
              .catch(() => {})
          }
        >
          {volume.muted ? "Unmute" : "Mute"}
        </button>
        <button className="link" onClick={() => void barOpen("mixer").catch(() => {})}>
          Volume mixer
        </button>
        <button className="link" onClick={() => void barOpen("sound").catch(() => {})}>
          Sound settings
        </button>
      </div>
    </section>
  );
}

/** Omarchy's power menu, the Windows way: the battery's state, and its power settings. */
export function BatteryPopover({ snapshot }: { snapshot: SystemSnapshot | null }) {
  const battery = snapshot?.battery;
  if (!battery) return null;
  const state =
    battery.state === "charging"
      ? "Charging"
      : battery.state === "full"
        ? "Plugged in, full"
        : battery.state === "discharging"
          ? "On battery"
          : "";
  return (
    <section className="bar-pop right" aria-label="Battery">
      <header>
        <BatteryIcon percent={battery.percent} charging={battery.state === "charging"} />
        <strong>{Math.round(battery.percent)} %</strong>
        <span className="muted">{state}</span>
      </header>
      <dl className="bar-facts">
        {battery.state === "discharging" && battery.time_to_empty_s !== null && (
          <>
            <dt>Time left</dt>
            <dd>
              {batteryWords(battery.percent, "discharging", battery.time_to_empty_s).split(", ")[1]}
            </dd>
          </>
        )}
        {battery.state === "charging" && battery.time_to_full_s !== null && (
          <>
            <dt>Full in</dt>
            <dd>{Math.round(battery.time_to_full_s / 60)} min</dd>
          </>
        )}
        {battery.health_percent !== null && (
          <>
            <dt>Health</dt>
            <dd>{Math.round(battery.health_percent)} % of new</dd>
          </>
        )}
      </dl>
      <div className="bar-pop-actions">
        <button onClick={() => void barOpen("power").catch(() => {})}>Power mode</button>
        <button className="link" onClick={() => void barOpen("battery").catch(() => {})}>
          Battery saver
        </button>
      </div>
    </section>
  );
}

const MONTH = { month: "long", year: "numeric" } as const;
const DAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

/** The calendar under the clock: this month, today lit; arrows for the others. */
export function CalendarPopover() {
  const today = new Date();
  const [shown, setShown] = useState({ year: today.getFullYear(), month: today.getMonth() });
  const step = (n: number) => {
    const d = new Date(shown.year, shown.month + n, 1);
    setShown({ year: d.getFullYear(), month: d.getMonth() });
  };
  const isToday = (day: number) =>
    day === today.getDate() &&
    shown.month === today.getMonth() &&
    shown.year === today.getFullYear();
  return (
    <section className="bar-pop centre" aria-label="Calendar">
      <header>
        <button className="bar-step" onClick={() => step(-1)} aria-label="Previous month">
          ‹
        </button>
        <strong>{new Date(shown.year, shown.month, 1).toLocaleDateString([], MONTH)}</strong>
        <button className="bar-step" onClick={() => step(1)} aria-label="Next month">
          ›
        </button>
      </header>
      <table className="bar-calendar">
        <thead>
          <tr>
            {DAYS.map((d) => (
              <th key={d}>{d}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {monthWeeks(shown.year, shown.month).map((week, i) => (
            <tr key={i}>
              {week.map((day, j) => (
                <td key={j} className={day && isToday(day) ? "today" : ""}>
                  {day ?? ""}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="bar-pop-actions">
        <button
          className="link"
          onClick={() => setShown({ year: today.getFullYear(), month: today.getMonth() })}
        >
          Today
        </button>
        <button className="link" onClick={() => void barOpen("date").catch(() => {})}>
          Date and time settings
        </button>
      </div>
    </section>
  );
}
