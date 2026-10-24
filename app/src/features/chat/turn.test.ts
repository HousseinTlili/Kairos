import { describe, expect, it } from "vitest";
import type { RunEvent, RunEventEnvelope } from "../../lib/hermes-events";
import { applyEvent, newTurn, type Turn } from "./turn";
import simpleSse from "../../../../fixtures/hermes-0.22.5/run-simple/02-events.sse?raw";
import autoApproveSse from "../../../../fixtures/hermes-0.22.5/run-smart-autoapprove/02-events.sse?raw";
import denySse from "../../../../fixtures/hermes-0.22.5/run-approval-deny/05-events.sse?raw";
import stopSse from "../../../../fixtures/hermes-0.22.5/run-stop/03-events.sse?raw";

// The Rust side sends {run_id, seq, timestamp, event: {event, ...fields}}; recorded Hermes data
// lines are flat, so wrap them the same way.
function envelopes(sse: string): RunEventEnvelope[] {
  return sse
    .split("\n")
    .filter((line) => line.startsWith("data: "))
    .map((line) => {
      const raw = JSON.parse(line.slice("data: ".length));
      return {
        run_id: raw.run_id,
        seq: raw.seq,
        timestamp: raw.timestamp,
        event: raw as RunEvent,
      };
    });
}

function play(sse: string): Turn {
  return envelopes(sse).reduce(applyEvent, newTurn("request"));
}

describe("a turn built from a recorded stream", () => {
  it("a simple run completes with Hermes's final answer", () => {
    const turn = play(simpleSse);
    expect(turn.status).toBe("completed");
    expect(turn.text).toBe("Kairos probe OK");
    expect(turn.items).toEqual([]);
  });

  it("after a tool call the leading blank line is replaced by the final output", () => {
    const turn = play(autoApproveSse);
    expect(turn.text.startsWith("\n")).toBe(false);
    expect(turn.items).toEqual([
      expect.objectContaining({ kind: "tool", tool: "terminal", state: "ok" }),
    ]);
  });

  it("a denied approval leaves a settled approval row and a failed tool row", () => {
    const turn = play(denySse);
    expect(turn.status).toBe("completed");
    expect(turn.items).toEqual([
      expect.objectContaining({ kind: "tool", tool: "terminal", state: "error" }),
      expect.objectContaining({
        kind: "approval",
        state: "settled",
        description: "recursive delete",
        command: expect.stringContaining('echo "EXIT_CODE=$?"'),
      }),
    ]);
  });

  it("a stopped run ends cancelled with its tool marked as failed", () => {
    const turn = play(stopSse);
    expect(turn.status).toBe("cancelled");
    expect(turn.items).toEqual([expect.objectContaining({ kind: "tool", state: "error" })]);
  });

  it("a replayed stream changes nothing", () => {
    const events = envelopes(denySse);
    const once = events.reduce(applyEvent, newTurn("request"));
    const twice = events.reduce(applyEvent, once);
    expect(twice).toEqual(once);
  });

  it("an approval request waits until it is settled", () => {
    const events = envelopes(denySse);
    const waiting = events.slice(0, 2).reduce(applyEvent, newTurn("request"));
    expect(waiting.status).toBe("waiting_for_approval");
    expect(waiting.items[1]).toEqual(expect.objectContaining({ state: "waiting" }));
  });
});

describe("events not recorded live yet", () => {
  const base = { run_id: "run_1", timestamp: null };

  it("a failed run shows Hermes's error message", () => {
    const turn = applyEvent(newTurn("x"), {
      ...base,
      seq: 0,
      event: { event: "run.failed", error: { message: "model unavailable" } },
    });
    expect(turn.status).toBe("failed");
    expect(turn.error).toBe("model unavailable");
  });

  it("an unknown event only advances the sequence", () => {
    const before = newTurn("x");
    const after = applyEvent(before, {
      ...base,
      seq: 0,
      event: { event: "unknown", name: "brand.new", raw: {} },
    });
    expect(after).toEqual({ ...before, lastSeq: 0 });
  });
});
