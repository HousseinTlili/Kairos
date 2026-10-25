import type { Job } from "../../lib/tauri";
import { timeLabel } from "../notifications/model";

const MINUTE = 60;
const HOUR = 60 * MINUTE;

const clock = (d: Date) => d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/** When a job runs next: "in 5 min", "today 14:00", "tomorrow 09:00", "3 Nov 09:00". */
export function nextRunLabel(iso: string, now: number = Date.now() / 1000): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const ahead = date.getTime() / 1000 - now;
  if (ahead < MINUTE) return "any moment";
  if (ahead < HOUR) return `in ${Math.round(ahead / MINUTE)} min`;
  const today = new Date(now * 1000);
  const tomorrow = new Date(today);
  tomorrow.setDate(today.getDate() + 1);
  if (date.toDateString() === today.toDateString()) return `today ${clock(date)}`;
  if (date.toDateString() === tomorrow.toDateString()) return `tomorrow ${clock(date)}`;
  return `${date.toLocaleDateString([], { day: "numeric", month: "short" })} ${clock(date)}`;
}

/** When it last ran, as the notifications page says it. */
export function lastRunLabel(iso: string, now: number = Date.now() / 1000): string {
  const at = new Date(iso).getTime() / 1000;
  return Number.isNaN(at) ? iso : timeLabel(at, now);
}

export type Tone = "ok" | "warn" | "bad" | "muted";

/** The job's state in words. A paused or finished job says so before its last result. */
export function statusOf(job: Job): { text: string; tone: Tone } {
  if (job.state === "error") {
    return {
      text: `Can't be scheduled${job.last_error ? `: ${job.last_error}` : ""}`,
      tone: "bad",
    };
  }
  if (job.state === "completed") return { text: "Finished", tone: "muted" };
  if (!job.enabled || job.state === "paused") return { text: "Paused", tone: "muted" };
  switch (job.last_status) {
    case null:
      return { text: "Hasn't run yet", tone: "muted" };
    case "ok":
      return { text: "Last run OK", tone: "ok" };
    case "error":
      return { text: `Last run failed${job.last_error ? `: ${job.last_error}` : ""}`, tone: "bad" };
    case "delivery_failed":
      return { text: "Ran, but its result couldn't be delivered", tone: "warn" };
    default:
      return { text: `Last run: ${job.last_status.replace(/_/g, " ")}`, tone: "warn" };
  }
}

/** Pause for a running job, Resume for a paused one, nothing for a finished or broken one. */
export function toggleOf(job: Job): "pause" | "resume" | null {
  if (job.state === "completed" || job.state === "error") return null;
  return job.enabled && job.state !== "paused" ? "pause" : "resume";
}

/** What Delete asks first (it removes the job from Hermes; CLAUDE.md asks first). */
export function deleteQuestion(job: Job): string {
  return `Delete "${job.name || job.schedule_display || job.id}"? Hermes stops running it, and this can't be undone.`;
}

/** Why the new-job form can't be sent yet, or null when it can. */
export function formProblem(name: string, schedule: string, prompt: string): string | null {
  if (!name.trim()) return "Give the job a name.";
  if (!schedule.trim()) return "Say when it runs.";
  if (!prompt.trim()) return "Say what it should do.";
  if (prompt.length > 5000) return "The instructions are too long (5,000 characters at most).";
  return null;
}

/** Schedules Hermes understands, offered under the field. */
export const SCHEDULE_EXAMPLES = [
  "every 2h",
  "every day at 9am",
  "every monday 9am",
  "in 30m",
  "0 9 * * 1-5",
];
