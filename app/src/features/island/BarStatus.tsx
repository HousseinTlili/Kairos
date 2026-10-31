import { useEffect, useState } from "react";
import {
  notificationsState,
  onNotificationsChanged,
  onSystemSnapshot,
  showMainWindow,
  systemSnapshot,
  type SystemSnapshot,
} from "../../lib/tauri";
import { BatteryIcon, BellIcon, SpeakerIcon, WifiIcon } from "../shell/icons";
import { batteryWords, wifiBars } from "./model";

/** The time, re-read every 15 s: one text change a minute, no animation. */
function useNow() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

const time = (d: Date) => d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/**
 * The right end of the top bar (D-034): notifications, Wi-Fi, sound, battery and the time. Read
 * from what Kairos already has (the system snapshot every 15 s, the notification store); nothing
 * here polls by itself.
 */
export function BarStatus({ onClock }: { onClock: () => void }) {
  const now = useNow();
  const [snapshot, setSnapshot] = useState<SystemSnapshot | null>(null);
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    const readUnread = () =>
      void notificationsState()
        .then((s) => setUnread(s.enabled ? s.unread : 0))
        .catch(() => {});
    void systemSnapshot()
      .then((s) => s && setSnapshot(s))
      .catch(() => {});
    readUnread();
    const stops = [onSystemSnapshot(setSnapshot), onNotificationsChanged(readUnread)];
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten());
    };
  }, []);

  const wifi = snapshot?.wifi;
  const battery = snapshot?.battery;
  return (
    <div className="topbar-status">
      <button
        className="topbar-icon bell"
        title={unread ? `${unread} unread notifications` : "Notifications"}
        aria-label="Notifications"
        onClick={() => void showMainWindow("notifications")}
      >
        <BellIcon />
        {unread > 0 && <span className="topbar-badge">{unread > 99 ? "99+" : unread}</span>}
      </button>
      {wifi && (
        <span
          className="topbar-icon"
          title={`${wifi.ssid}${wifi.signal_percent !== null ? `, ${wifi.signal_percent} %` : ""}`}
        >
          <WifiIcon bars={wifiBars(wifi.signal_percent)} />
        </span>
      )}
      <span className="topbar-icon" title="Sound">
        <SpeakerIcon />
      </span>
      {battery && (
        <span
          className={`topbar-icon battery ${battery.percent <= 20 && battery.state === "discharging" ? "low" : ""}`}
          title={batteryWords(battery.percent, battery.state, battery.time_to_empty_s)}
        >
          <BatteryIcon percent={battery.percent} charging={battery.state === "charging"} />
        </span>
      )}
      <button className="topbar-clock" onClick={onClock} title="Open the panel">
        <span className="topbar-time">{time(now)}</span>
        <span className="topbar-date">
          {now.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" })}
        </span>
      </button>
    </div>
  );
}
