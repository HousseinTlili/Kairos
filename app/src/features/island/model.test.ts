import { describe, expect, it } from "vitest";
import { newTurn, type Turn } from "../chat/turn";
import { DONE_MS, mergeStates, stateOf, type KairosState } from "./model";

const quiet = { phase: "idle" as const, speaking: false };

function turn(status: Turn["status"], text = ""): Turn {
  return { ...newTurn("Open Notepad and write a shopping list for the weekend"), status, text };
}

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
