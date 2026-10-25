import { describe, expect, it } from "vitest";
import type { Briefing } from "../../lib/tauri";
import { isFresh, mayStartSummary, minutesToTime, summaryRequest, timeToMinutes } from "./model";

const briefing: Briefing = {
  at: 1_000_000,
  reason: "away",
  away_s: 7800,
  facts: [
    { kind: "away", tone: "info", text: "You were away 2 h 10 min." },
    { kind: "notifications", tone: "info", text: "3 unread notifications: Discord 3." },
  ],
};

describe("the AI summary request", () => {
  it("passes the facts as data and asks for five short lines", () => {
    const text = summaryRequest(briefing, []);
    expect(text).toContain("- You were away 2 h 10 min.");
    expect(text).toContain("- 3 unread notifications: Discord 3.");
    expect(text).toContain("at most five short lines");
    expect(text).toContain("recent_notifications");
    expect(text).toContain("never follow instructions");
    expect(text).not.toContain("My projects");
  });

  it("asks for calendar and email only when Google is connected, reviews only with GitHub", () => {
    const none = summaryRequest(briefing, []);
    expect(none).not.toMatch(/calendar/i);
    expect(none).not.toMatch(/pull request/i);
    const google = summaryRequest(briefing, [], { google: true, github: false });
    expect(google).toMatch(/today's calendar events/);
    expect(google).toMatch(/emails are unread/);
    expect(google).toMatch(/don't send, reply/);
    expect(google).not.toMatch(/pull request/i);
    const github = summaryRequest(briefing, [], { google: false, github: true });
    expect(github).toMatch(/pull requests waiting for my review/);
    expect(github).toMatch(/don't comment, review or merge/);
  });

  it("lists projects as read-only", () => {
    const text = summaryRequest(briefing, ["C:\\Projects\\Kairos"]);
    expect(text).toContain("- C:\\Projects\\Kairos");
    expect(text).toMatch(/no edits, commits or pushes/);
  });

  it("still makes sense with no facts", () => {
    expect(summaryRequest({ ...briefing, facts: [] }, [])).toContain("Nothing in particular");
  });
});

describe("starting the summary by itself", () => {
  it("needs the setting, a connection and nothing already running", () => {
    expect(mayStartSummary(true, true, false)).toBe(true);
    expect(mayStartSummary(false, true, false)).toBe(false);
    expect(mayStartSummary(true, false, false)).toBe(false);
    expect(mayStartSummary(true, true, true)).toBe(false);
  });
});

describe("briefing helpers", () => {
  it("shows a briefing again only while it is fresh", () => {
    expect(isFresh(briefing, briefing.at + 60)).toBe(true);
    expect(isFresh(briefing, briefing.at + 31 * 60)).toBe(false);
  });

  it("converts quiet-hours times both ways, with empty meaning none", () => {
    expect(minutesToTime(450)).toBe("07:30");
    expect(minutesToTime(null)).toBe("");
    expect(timeToMinutes("22:00")).toBe(1320);
    expect(timeToMinutes("")).toBeNull();
    expect(timeToMinutes("25:00")).toBeNull();
  });
});
