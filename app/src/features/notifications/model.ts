// Pure helpers for the notifications panel (M4 §7), kept out of the components for the tests.
import type { AppCount, CollectState, NotificationAccess } from "../../lib/tauri";

const MINUTE = 60;
const HOUR = 60 * MINUTE;

/** "just now", "5 min ago", "3 h ago", then the time for today's older ones or the date. */
export function timeLabel(at: number, now: number = Date.now() / 1000): string {
  const age = now - at;
  if (age < MINUTE) return "just now";
  if (age < HOUR) return `${Math.floor(age / MINUTE)} min ago`;
  if (age < 6 * HOUR) return `${Math.floor(age / HOUR)} h ago`;
  const date = new Date(at * 1000);
  const today = new Date(now * 1000);
  const sameDay = date.toDateString() === today.toDateString();
  return sameDay
    ? date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : date.toLocaleDateString([], { day: "numeric", month: "short" });
}

/** The sidebar badge: nothing at zero, "99+" past 99. */
export function badgeText(unread: number): string | null {
  if (unread <= 0) return null;
  return unread > 99 ? "99+" : String(unread);
}

/** What "Clear" asks before deleting (it removes data; CLAUDE.md asks first). */
export function clearQuestion(app: AppCount | null, totalAll: number): string {
  if (app) {
    const n = app.total;
    return `Delete ${n} notification${n === 1 ? "" : "s"} from ${app.app_name}? This can't be undone.`;
  }
  return `Delete all ${totalAll} notification${totalAll === 1 ? "" : "s"}? This can't be undone.`;
}

export function totalOf(counts: AppCount[]): number {
  return counts.reduce((sum, c) => sum + c.total, 0);
}

/** Settings ▸ Notifications: what is happening and what the user can do about it. */
export function stateText(
  state: CollectState,
  access: NotificationAccess,
): { tone: "ok" | "warn" | "muted"; text: string } {
  switch (state) {
    case "off":
      return { tone: "muted", text: "Off. Kairos isn't reading your notifications." };
    case "on":
      return { tone: "ok", text: "On. New notifications appear in Kairos within a few seconds." };
    case "unavailable":
      return {
        tone: "warn",
        text: "Switched on, but Kairos can't read notifications on this PC yet.",
      };
    case "waiting_for_windows":
      return {
        tone: "warn",
        text:
          access === "denied"
            ? "Windows is blocking it. Turn on Settings ▸ Privacy & security ▸ Notifications ▸ Notification access."
            : "Waiting for Windows to allow it.",
      };
  }
}
