import { describe, expect, it } from "vitest";
import type { AuditEvent } from "../../lib/tauri";
import { newTurn, type Turn } from "../chat/turn";
import {
  DONE_MS,
  batteryWords,
  mergeStates,
  monthWeeks,
  stateOf,
  wheelVolume,
  wifiBars,
  type KairosState,
} from "./model";

const quiet = { phase: "idle" as const, speaking: false };

function turn(status: Turn["status"], text = ""): Turn {
  return { ...newTurn("Open Notepad and write a shopping list for the weekend"), status, text };
}

describe("working in another app (M20)", () => {
  const working = (status: Turn["status"]): Turn => ({
    ...newTurn("use it"),
    status,
    taskId: "t",
    runId: "r",
    items: [
      {
        kind: "tool",
        toolCallId: "c",
        tool: "computer_use",
        preview: "",
        state: "running",
        audit: {
          args_redacted: JSON.stringify({ action: "capture", kairos_target: "Character Map" }),
        } as AuditEvent,
      },
    ],
  });
  const quiet = { phase: "idle", speaking: false } as const;

  it("names the app, and gives the bar what Stop needs", () => {
    const state = stateOf("main", working("running"), quiet, "kairos-1", 0, null);
    expect(state.text).toBe("Working in Character Map");
    expect(state.desktop).toEqual({ taskId: "t", runId: "r" });
  });

  it("keeps Stop while a card waits, and drops it when the request ends", () => {
    const waiting = stateOf("main", working("waiting_for_approval"), quiet, "kairos-1", 0, null);
    expect(waiting.approval).toBe(true);
    expect(waiting.desktop).toEqual({ taskId: "t", runId: "r" });
    expect(stateOf("main", working("completed"), quiet, "kairos-1", 0, null).desktop).toBeNull();
  });
});

describe("what the island says", () => {
  it("is at your service when nothing happens", () => {
    expect(stateOf("main", undefined, quiet, null, 0, null)).toMatchObject({
      mood: "idle",
      text: "At your service",
    });
  });

  it("shows the request being worked on, shortened", () => {
    const state = stateOf("quick", turn("running"), quiet, "kairos-1", 0, null);
    expect(state.mood).toBe("working");
    expect(state.text).toBe("Open Notepad and write a shopping…");
  });

  it("an approval outranks everything and says so", () => {
    const state = stateOf("main", turn("waiting_for_approval"), quiet, "kairos-1", 0, null);
    expect(state).toMatchObject({ approval: true, text: "Kairos asks your leave" });
  });

  it("listening and speaking come from the voice", () => {
    expect(
      stateOf("quick", undefined, { phase: "listening", speaking: false }, null, 0, null).text,
    ).toBe("Listening…");
    expect(
      stateOf("quick", turn("completed"), { phase: "idle", speaking: true }, null, 0, null).mood,
    ).toBe("speaking");
  });

  it("says Done for a few seconds after an answer, then rests", () => {
    const done = turn("completed", "Notepad is **open**.");
    expect(stateOf("quick", done, quiet, null, 1000, 0).text).toBe("Done");
    expect(stateOf("quick", done, quiet, null, DONE_MS + 1, 0).text).toBe("At your service");
    expect(stateOf("quick", done, quiet, null, 1000, 0).answer).toBe("Notepad is open.");
  });
});

describe("merging the windows", () => {
  const idle = stateOf("main", undefined, quiet, null, 0, null);
  const busy: KairosState = stateOf("quick", turn("running"), quiet, "kairos-2", 0, null);
  const asking: KairosState = stateOf(
    "main",
    turn("waiting_for_approval"),
    quiet,
    "kairos-1",
    0,
    null,
  );

  it("shows the busy window rather than the idle one", () => {
    expect(mergeStates([idle, busy], 0)?.source).toBe("quick");
  });

  it("shows a waiting approval before anything else", () => {
    expect(mergeStates([busy, asking], 0)?.approval).toBe(true);
  });

  it("has nothing to show before any window reported", () => {
    expect(mergeStates([], 0)).toBeNull();
  });

  it("shows an automation's line for its time, then goes back to the window", () => {
    const line: KairosState = {
      ...idle,
      source: "automation",
      text: "Joined Home",
      doneAt: 1000,
      holdMs: 10_000,
    };
    expect(mergeStates([idle, line], 5000)?.text).toBe("Joined Home");
    expect(mergeStates([idle, line], 12_000)?.text).toBe("At your service");
  });
});

describe("the top bar's status", () => {
  it("shows Wi-Fi in three steps, and full bars when the signal isn't known", () => {
    expect([90, 55, 20, 5, null].map(wifiBars)).toEqual([3, 2, 1, 0, 3]);
  });

  it("says the battery's time left only while it runs on battery", () => {
    expect(batteryWords(64.4, "discharging", 7800)).toBe("64 %, 2 h 10 min left");
    expect(batteryWords(64, "charging", null)).toBe("64 %, charging");
    expect(batteryWords(100, "full", null)).toBe("100 %, plugged in");
  });
});

describe("Omarchy's modules", () => {
  it("lays a month out in weeks from Monday", () => {
    // October 2026 starts on a Thursday and has 31 days.
    const weeks = monthWeeks(2026, 9);
    expect(weeks[0]).toEqual([null, null, null, 1, 2, 3, 4]);
    expect(weeks[weeks.length - 1]).toEqual([26, 27, 28, 29, 30, 31, null]);
    expect(weeks.every((w) => w.length === 7)).toBe(true);
  });

  it("turns the volume 5 % a notch of the wheel, within 0–100", () => {
    expect(wheelVolume(42, -100)).toBe(45);
    expect(wheelVolume(42, 100)).toBe(35);
    expect(wheelVolume(98, -100)).toBe(100);
    expect(wheelVolume(3, 100)).toBe(0);
  });
});
