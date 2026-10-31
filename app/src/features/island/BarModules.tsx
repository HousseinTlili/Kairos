import { useEffect, useState } from "react";
import {
  barOpen,
  barSetMuted,
  barSetVolume,
  barSwitchDesktop,
  bluetoothSet,
  bluetoothState,
  showMainWindow,
  wifiConnect,
  wifiDisconnect,
  wifiSet,
  wifiState,
  type BluetoothState,
  type Desktops,
  type SystemSnapshot,
  type Volume,
  type WifiState,
} from "../../lib/tauri";
import {
  BatteryIcon,
  BellIcon,
  BluetoothIcon,
  CpuIcon,
  VolumeIcon,
  WifiIcon,
} from "../shell/icons";
import { batteryWords, clockText, monthWeeks, wheelVolume } from "./model";

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

export type Pop = "volume" | "battery" | "calendar" | "wifi" | "bluetooth";

/**
 * Omarchy's right side: Bluetooth, the network, audio, the CPU and the battery, with Kairos's bell
 * before them. Bluetooth and Wi-Fi open Kairos's own menus (D-037). Audio: a click opens the
 * slider, the wheel turns it 5 % a notch, a right-click mutes. The CPU opens Kairos's System page,
 * as Omarchy's opens btop.
 */
export function SystemModules({
  snapshot,
  wifi,
  unread,
  volume,
  onVolume,
  open,
  onPop,
}: {
  snapshot: SystemSnapshot | null;
  /** The network in use and its bars, from the bar's state (it works with location off). */
  wifi: [string, number] | null;
  unread: number;
  volume: Volume | null;
  onVolume: (v: Volume) => void;
  open: Pop | null;
  onPop: (pop: Pop) => void;
}) {
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
      <Module title="Bluetooth" active={open === "bluetooth"} onClick={() => onPop("bluetooth")}>
        <BluetoothIcon />
      </Module>
      <Module
        title={wifi ? wifi[0] : "Wi-Fi"}
        active={open === "wifi"}
        onClick={() => onPop("wifi")}
      >
        <WifiIcon bars={wifi ? (Math.min(3, Math.max(1, wifi[1] - 1)) as 1 | 2 | 3) : 0} />
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

const messageOf = (err: unknown) =>
  (err as { message?: string })?.message ?? (typeof err === "string" ? err : String(err));

/** A switch in a menu's header, for a radio. */
function RadioSwitch({
  on,
  label,
  onChange,
}: {
  on: boolean | null;
  label: string;
  onChange: (on: boolean) => void;
}) {
  if (on === null) return null;
  return (
    <label className="bar-switch" title={`${label} ${on ? "on" : "off"}`}>
      <input type="checkbox" checked={on} onChange={(e) => onChange(e.target.checked)} />
      <span aria-hidden="true" />
      <span className="sr-only">{label}</span>
    </label>
  );
}

/**
 * Kairos's own Wi-Fi menu (D-037): the radio, the network in use, the saved networks and, when
 * Windows shows them, the nearby ones; a click joins, with a password for a new secured network.
 */
export function WifiPopover({ onChanged }: { onChanged: () => void }) {
  const [state, setState] = useState<WifiState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [asking, setAsking] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [joining, setJoining] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    const read = (scan: boolean) =>
      void wifiState(scan)
        .then((s) => live && setState(s))
        .catch((err: unknown) => live && setError(messageOf(err)));
    read(true);
    // A scan's results arrive a few seconds after it is asked for.
    const timer = setInterval(() => read(false), 3000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, []);

  // Joining: Windows answers later; the menu watches for the network to come up.
  useEffect(() => {
    if (!joining) return;
    const until = Date.now() + 20_000;
    const timer = setInterval(() => {
      void wifiState(false).then((s) => {
        setState(s);
        if (s.connected?.[0] === joining) {
          setJoining(null);
          onChanged();
        } else if (Date.now() > until) {
          setJoining(null);
          setError(`Couldn't join ${joining}. Check the password, or try again.`);
        }
      });
    }, 1500);
    return () => clearInterval(timer);
  }, [joining, onChanged]);

  const join = (ssid: string, pass: string | null) => {
    setError(null);
    setAsking(null);
    setPassword("");
    void wifiConnect(ssid, pass)
      .then(() => setJoining(ssid))
      .catch((err: unknown) => setError(messageOf(err)));
  };

  if (!state) {
    return (
      <section className="bar-pop right wide" aria-label="Wi-Fi">
        <p className="muted">{error ?? "Looking for networks…"}</p>
      </section>
    );
  }
  if (!state.adapter) {
    return (
      <section className="bar-pop right wide" aria-label="Wi-Fi">
        <p className="muted">This PC has no Wi-Fi adapter.</p>
      </section>
    );
  }

  const current = state.connected?.[0] ?? null;
  const nearby = state.nearby?.filter((n) => n.ssid !== current) ?? null;
  // With location off, the saved networks stand in for the nearby ones.
  const saved = state.saved.filter((s) => s !== current);

  return (
    <section className="bar-pop right wide" aria-label="Wi-Fi">
      <header>
        <WifiIcon bars={state.on === false ? 0 : 3} />
        <strong>Wi-Fi</strong>
        <RadioSwitch
          on={state.on}
          label="Wi-Fi"
          onChange={(on) =>
            void wifiSet(on)
              .then((s) => {
                setState(s);
                onChanged();
              })
              .catch((err: unknown) => setError(messageOf(err)))
          }
        />
      </header>

      {state.on !== false && (
        <>
          {current && (
            <div className="net-row connected">
              <WifiIcon
                bars={
                  state.connected
                    ? (Math.min(3, Math.max(1, state.connected[1] - 1)) as 1 | 2 | 3)
                    : 0
                }
              />
              <span className="net-name">{current}</span>
              <span className="net-tag">Connected</span>
              <button
                className="link"
                onClick={() =>
                  void wifiDisconnect()
                    .then(() => {
                      setState({ ...state, connected: null });
                      onChanged();
                    })
                    .catch((err: unknown) => setError(messageOf(err)))
                }
              >
                Disconnect
              </button>
            </div>
          )}

          <div className="net-list">
            {nearby
              ? nearby.map((n) => (
                  <div key={n.ssid} className="net-item">
                    <button
                      className="net-row"
                      disabled={joining !== null}
                      onClick={() =>
                        n.saved || !n.secured ? join(n.ssid, null) : setAsking(n.ssid)
                      }
                    >
                      <WifiIcon bars={Math.min(3, Math.max(1, n.bars - 1)) as 1 | 2 | 3} />
                      <span className="net-name">{n.ssid}</span>
                      {n.secured && (
                        <span className="net-lock" title="Secured">
                          🔒︎
                        </span>
                      )}
                      {joining === n.ssid ? (
                        <span className="net-tag">Joining…</span>
                      ) : n.saved ? (
                        <span className="net-tag muted">Saved</span>
                      ) : null}
                    </button>
                    {asking === n.ssid && (
                      <form
                        className="net-password"
                        onSubmit={(e) => {
                          e.preventDefault();
                          join(n.ssid, password);
                        }}
                      >
                        <input
                          autoFocus
                          type="password"
                          placeholder="Password"
                          value={password}
                          onChange={(e) => setPassword(e.target.value)}
                          aria-label={`Password for ${n.ssid}`}
                        />
                        <button type="submit" className="primary" disabled={password.length < 8}>
                          Join
                        </button>
                      </form>
                    )}
                  </div>
                ))
              : saved.map((ssid) => (
                  <button
                    key={ssid}
                    className="net-row"
                    disabled={joining !== null}
                    onClick={() => join(ssid, null)}
                    title="Saved: join without a password if it's in range"
                  >
                    <WifiIcon bars={1} />
                    <span className="net-name">{ssid}</span>
                    {joining === ssid ? (
                      <span className="net-tag">Joining…</span>
                    ) : (
                      <span className="net-tag muted">Saved</span>
                    )}
                  </button>
                ))}
          </div>

          {nearby === null && (
            <p className="net-note small muted">
              Windows shows nearby networks to apps only while location is on. Your saved networks
              are listed instead.{" "}
              <button className="link" onClick={() => void barOpen("location").catch(() => {})}>
                Location settings
              </button>
            </p>
          )}
        </>
      )}
      {error && <p className="bad-text small">{error}</p>}
      <div className="bar-pop-actions">
        <button className="link" onClick={() => void barOpen("wifi").catch(() => {})}>
          Windows network list
        </button>
      </div>
    </section>
  );
}

/**
 * Kairos's own Bluetooth menu (D-037): the radio and the paired devices. Connecting a device and
 * pairing a new one stay in Windows's settings (no public API for an app to connect one).
 */
export function BluetoothPopover() {
  const [state, setState] = useState<BluetoothState | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    const read = () =>
      void bluetoothState()
        .then((s) => live && setState(s))
        .catch((err: unknown) => live && setError(messageOf(err)));
    read();
    const timer = setInterval(read, 4000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, []);

  if (!state) {
    return (
      <section className="bar-pop right wide" aria-label="Bluetooth">
        <p className="muted">{error ?? "Reading Bluetooth…"}</p>
      </section>
    );
  }
  if (!state.adapter) {
    return (
      <section className="bar-pop right wide" aria-label="Bluetooth">
        <p className="muted">This PC has no Bluetooth.</p>
      </section>
    );
  }
  return (
    <section className="bar-pop right wide" aria-label="Bluetooth">
      <header>
        <BluetoothIcon />
        <strong>Bluetooth</strong>
        <RadioSwitch
          on={state.on}
          label="Bluetooth"
          onChange={(on) =>
            void bluetoothSet(on)
              .then(setState)
              .catch((err: unknown) => setError(messageOf(err)))
          }
        />
      </header>
      {state.on === false && (
        <p className="muted small">
          Bluetooth is off
          {state.devices.length > 0 &&
            ` (${state.devices.length} paired device${state.devices.length > 1 ? "s" : ""})`}
          .
        </p>
      )}
      {state.on !== false &&
        (state.devices.length === 0 ? (
          <p className="muted small">No paired devices.</p>
        ) : (
          <div className="net-list">
            {state.devices.map((d) => (
              <div key={d.name} className={`net-row ${d.connected ? "connected" : ""}`}>
                <BluetoothIcon />
                <span className="net-name">{d.name}</span>
                <span className={`net-tag ${d.connected ? "" : "muted"}`}>
                  {d.connected ? "Connected" : "Paired"}
                </span>
              </div>
            ))}
          </div>
        ))}
      {error && <p className="bad-text small">{error}</p>}
      <div className="bar-pop-actions">
        <button onClick={() => void barOpen("bluetooth").catch(() => {})}>Add a device</button>
        <button className="link" onClick={() => void barOpen("bluetooth").catch(() => {})}>
          Connect in Windows
        </button>
      </div>
    </section>
  );
}
