import { describe, expect, it } from "vitest";
import type { Job } from "../../lib/tauri";
import {
  deleteQuestion,
  formProblem,
  lastRunLabel,
  nextRunLabel,
  statusOf,
  toggleOf,
} from "./model";

const job = (over: Partial<Job> = {}): Job => ({
  id: "a1b2c3d4e5f6",
  name: "Weekly summary",
  prompt: "Summarise my week",
  schedule_display: "every monday 9am",
  enabled: true,
  state: "scheduled",
  next_run_at: null,
  last_run_at: null,
  last_status: null,
  last_error: null,
  ...over,
});

// A fixed local "now": 25 Oct 2026, 10:00.
const now = new Date(2026, 9, 25, 10, 0, 0).getTime() / 1000;
const at = (d: Date) => d.toISOString();

describe("next run wording", () => {
  it("counts minutes in the next hour", () => {
    expect(nextRunLabel(at(new Date(2026, 9, 25, 10, 20)), now)).toBe("in 20 min");
    expect(nextRunLabel(at(new Date(2026, 9, 25, 10, 0, 30)), now)).toBe("any moment");
  });

  it("names today and tomorrow, then the date", () => {
    // The clock part follows the PC's locale (15:00 or 03:00 PM), so only the day is checked.
    const today = nextRunLabel(at(new Date(2026, 9, 25, 15, 0)), now);
    const tomorrow = nextRunLabel(at(new Date(2026, 9, 26, 9, 0)), now);
    const later = nextRunLabel(at(new Date(2026, 10, 3, 9, 0)), now);
    expect(today).toMatch(/^today \d/);
    expect(tomorrow).toMatch(/^tomorrow \d/);
    expect(later).toMatch(/3/);
    expect(later).not.toMatch(/^(today|tomorrow)/);
  });

  it("shows Hermes's text as is when it isn't a date", () => {
    expect(nextRunLabel("soon", now)).toBe("soon");
  });

  it("says how long ago the last run was", () => {
    expect(lastRunLabel(at(new Date(2026, 9, 25, 9, 55)), now)).toBe("5 min ago");
  });
});

describe("job status wording", () => {
  it("says a new job hasn't run", () => {
    expect(statusOf(job())).toEqual({ text: "Hasn't run yet", tone: "muted" });
  });

  it("reports the last result with its error", () => {
    expect(statusOf(job({ last_status: "ok" })).tone).toBe("ok");
    expect(statusOf(job({ last_status: "error", last_error: "Provider returned 429" }))).toEqual({
      text: "Last run failed: Provider returned 429",
      tone: "bad",
    });
    expect(statusOf(job({ last_status: "delivery_failed" })).tone).toBe("warn");
    expect(statusOf(job({ last_status: "blocked_config" })).text).toBe("Last run: blocked config");
  });

  it("puts paused, finished and unschedulable before the last result", () => {
    expect(statusOf(job({ enabled: false, state: "paused", last_status: "ok" })).text).toBe(
      "Paused",
    );
    expect(statusOf(job({ state: "completed", last_status: "ok" })).text).toBe("Finished");
    expect(statusOf(job({ state: "error", last_error: "bad cron" })).text).toBe(
      "Can't be scheduled: bad cron",
    );
  });

  it("offers Pause or Resume only where it means something", () => {
    expect(toggleOf(job())).toBe("pause");
    expect(toggleOf(job({ enabled: false, state: "paused" }))).toBe("resume");
    expect(toggleOf(job({ state: "completed" }))).toBeNull();
  });

  it("names the job when asking before deleting it", () => {
    expect(deleteQuestion(job())).toContain('"Weekly summary"');
    expect(deleteQuestion(job({ name: "" }))).toContain('"every monday 9am"');
  });
});

describe("new job form", () => {
  it("needs a name, a schedule and instructions", () => {
    expect(formProblem("", "every 2m", "say hello")).toMatch(/name/);
    expect(formProblem("Hello", " ", "say hello")).toMatch(/when/);
    expect(formProblem("Hello", "every 2m", "")).toMatch(/what/);
    expect(formProblem("Hello", "every 2m", "x".repeat(5001))).toMatch(/too long/);
    expect(formProblem("Hello", "every 2m", "say hello")).toBeNull();
  });
});
