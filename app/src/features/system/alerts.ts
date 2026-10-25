// The alert banners' list and the "Close it?" request (M3 §7). Pure, for the tests.
import type { AlertEvaluation, SystemAlert } from "../../lib/tauri";

/** New alerts replace an older one with the same id; cleared ones go away. Newest first. */
export function applyAlerts(current: SystemAlert[], evaluation: AlertEvaluation): SystemAlert[] {
  const gone = new Set([...evaluation.cleared, ...evaluation.fired.map((a) => a.id)]);
  return [...evaluation.fired, ...current.filter((alert) => !gone.has(alert.id))];
}

/**
 * What "Close it?" sends to Hermes. Kairos doesn't close anything itself: Hermes calls
 * `close_app`, and the `close-apps` rule asks the user on an approval card first.
 */
export function closeRequest(alert: SystemAlert): string | null {
  if (alert.kind !== "cpu_hog" || !alert.process) return null;
  const share = alert.cpu_percent != null ? `about ${alert.cpu_percent.toFixed(0)} %` : "most";
  return `Close ${alert.process}. It has been using ${share} of the CPU for over a minute.`;
}
