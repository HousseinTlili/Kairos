// Words for Settings ▸ Startup (M3 §8). Pure, for the tests.
import type { PresenceEvent, StartupState } from "../../lib/tauri";

const KIND: Record<PresenceEvent["kind"], string> = {
  startup: "Kairos started",
  resume: "PC woke from sleep",
  unlock: "PC unlocked",
};

export function describePresence(event: PresenceEvent): string {
  if (event.kind === "startup" && event.detail === "autostart") return "Kairos started at login";
  return event.detail ? `${KIND[event.kind]} (${event.detail})` : KIND[event.kind];
}

export function hermesAtLogin(state: StartupState): { ok: boolean | null; text: string } {
  const at = state.hermes_at_login;
  if (!at) return { ok: null, text: "Kairos can't tell on this PC." };
  if (at.scheduled_task)
    return { ok: true, text: "Starts at login (Scheduled Task Hermes_Gateway)." };
  if (at.startup_entry) return { ok: true, text: "Starts at login (Startup folder)." };
  return { ok: false, text: "Doesn't start by itself yet." };
}
