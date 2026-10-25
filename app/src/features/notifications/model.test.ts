import { describe, expect, it } from "vitest";
import type { AppCount } from "../../lib/tauri";
import { badgeText, clearQuestion, stateText, timeLabel, totalOf } from "./model";

const NOW = Date.UTC(2026, 9, 25, 15, 0, 0) / 1000;

describe("notification times", () => {
  it("read as ages while recent", () => {
    expect(timeLabel(NOW - 20, NOW)).toBe("just now");
    expect(timeLabel(NOW - 5 * 60, NOW)).toBe("5 min ago");
    expect(timeLabel(NOW - 3 * 3600, NOW)).toBe("3 h ago");
  });

  it("become a date once they are from another day", () => {
    expect(timeLabel(NOW - 3 * 86400, NOW)).not.toMatch(/ago/);
  });
});

describe("the sidebar badge", () => {
  it("is hidden at zero and capped at 99+", () => {
    expect(badgeText(0)).toBeNull();
    expect(badgeText(7)).toBe("7");
    expect(badgeText(150)).toBe("99+");
  });
});

describe("clearing", () => {
  const discord: AppCount = { app_id: "d", app_name: "Discord", total: 3, unread: 1 };

  it("asks with the app and how many will go", () => {
    expect(clearQuestion(discord, 10)).toBe(
      "Delete 3 notifications from Discord? This can't be undone.",
    );
    expect(clearQuestion(null, 1)).toBe("Delete all 1 notification? This can't be undone.");
  });

  it("counts every app's notifications for clear all", () => {
    expect(totalOf([discord, { ...discord, app_id: "o", total: 2 }])).toBe(5);
  });
});

describe("the settings status line", () => {
  it("tells the user what to do when Windows blocks it", () => {
    expect(stateText("waiting_for_windows", "denied").text).toContain("Notification access");
    expect(stateText("off", "allowed").tone).toBe("muted");
    expect(stateText("on", "allowed").tone).toBe("ok");
    expect(stateText("unavailable", "unavailable").tone).toBe("warn");
  });
});
