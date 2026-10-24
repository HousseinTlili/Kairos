import { describe, expect, it } from "vitest";
import type { RunEvent, RunEventEnvelope } from "../../lib/hermes-events";
import type { AuditEvent } from "../../lib/tauri";
import {
  applyEvent,
  attachAudit,
  blockedCount,
  decisionText,
  isFinished,
  liveApproval,
  markAnswered,
  newTurn,
  readApproval,
  toolLabel,
  toolRowFromAudit,
  turnFromTask,
  turnsFromHistory,
  type Turn,
} from "./turn";
import simpleSse from "../../../../fixtures/hermes-0.22.5/run-simple/02-events.sse?raw";
import autoApproveSse from "../../../../fixtures/hermes-0.22.5/run-smart-autoapprove/02-events.sse?raw";
import denySse from "../../../../fixtures/hermes-0.22.5/run-approval-deny/05-events.sse?raw";
import stopSse from "../../../../fixtures/hermes-0.22.5/run-stop/03-events.sse?raw";
import kairosDenySse from "../../../../fixtures/hermes-0.22.5/run-kairos-rule-deny/events.sse?raw";
import doublePromptSse from "../../../../fixtures/hermes-0.22.5/run-kairos-rule-double-prompt/events.sse?raw";

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
  return envelopes(sse).reduce((turn, e) => applyEvent(turn, e, true), newTurn("request"));
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
    const apply = (turn: Turn, e: RunEventEnvelope) => applyEvent(turn, e, true);
    const once = events.reduce(apply, newTurn("request"));
    const twice = events.reduce(apply, once);
    expect(twice).toEqual(once);
  });

  it("an approval request waits until it is settled", () => {
    const events = envelopes(denySse);
    const waiting = events
      .slice(0, 2)
      .reduce((turn, e) => applyEvent(turn, e, true), newTurn("request"));
    expect(waiting.status).toBe("waiting_for_approval");
    expect(waiting.items[1]).toEqual(expect.objectContaining({ state: "waiting" }));
  });
});

describe("events not recorded live yet", () => {
  const base = { run_id: "run_1", timestamp: null };

  it("a failed run shows Hermes's error message", () => {
    const turn = applyEvent(
      newTurn("x"),
      {
        ...base,
        seq: 0,
        event: { event: "run.failed", error: { message: "model unavailable" } },
      },
      false,
    );
    expect(turn.status).toBe("failed");
    expect(turn.error).toBe("model unavailable");
  });

  it("an unknown event only advances the sequence", () => {
    const before = newTurn("x");
    const after = applyEvent(
      before,
      {
        ...base,
        seq: 0,
        event: { event: "unknown", name: "brand.new", raw: {} },
      },
      false,
    );
    expect(after).toEqual({ ...before, lastSeq: 0 });
  });
});

describe("approvals, stop and history (M1 §9 rules 2, 4, 5)", () => {
  const events = envelopes(denySse);

  it("a replayed approval request is shown but can't be answered and doesn't block the turn", () => {
    const running: Turn = { ...newTurn("x"), status: "running" };
    const turn = events.slice(0, 2).reduce((t, e) => applyEvent(t, e, false), running);
    expect(turn.status).toBe("running");
    expect(liveApproval(turn)).toBeNull();
  });

  it("a live approval request can be answered until it settles", () => {
    const waiting = events.slice(0, 2).reduce((t, e) => applyEvent(t, e, true), newTurn("x"));
    expect(liveApproval(waiting)).toMatchObject({
      promptId: "5ca707fdc79b4076b0d1dddbcc756c64",
      executionGeneration: 1,
    });
    const answered = markAnswered(waiting, "5ca707fdc79b4076b0d1dddbcc756c64", "deny");
    const settled = applyEvent(answered, events[2], true);
    expect(liveApproval(settled)).toBeNull();
  });

  it("a denied approval marks the tool row blocked and counts it", () => {
    let turn = events.slice(0, 2).reduce((t, e) => applyEvent(t, e, true), newTurn("x"));
    turn = markAnswered(turn, "5ca707fdc79b4076b0d1dddbcc756c64", "deny");
    turn = events.slice(2).reduce((t, e) => applyEvent(t, e, true), turn);
    const tool = turn.items[0];
    expect(tool.kind === "tool" && toolLabel(turn, tool)).toBe("blocked");
    expect(blockedCount(turn)).toBe(1);
    expect(turn.status).toBe("completed");
  });

  it("the tool interrupted by Stop is labelled stopped, not failed", () => {
    const turn = play(stopSse);
    const tool = turn.items[0];
    expect(tool.kind === "tool" && toolLabel(turn, tool)).toBe("stopped");
  });

  it("a stored task becomes a finished turn with its answer", () => {
    const turn = turnFromTask({
      id: "t1",
      session_id: "kairos-s",
      run_id: "run_1",
      goal: "Reply with exactly: Kairos OK",
      status: "completed",
      created_at: 1,
      finished_at: 2,
      outcome: "Kairos OK",
    });
    expect(turn).toMatchObject({ input: "Reply with exactly: Kairos OK", text: "Kairos OK" });
    expect(isFinished(turn.status)).toBe(true);
  });

  it("a lost task shows why instead of an answer", () => {
    const turn = turnFromTask({
      id: "t1",
      session_id: "kairos-s",
      run_id: "run_1",
      goal: "x",
      status: "lost",
      created_at: 1,
      finished_at: 2,
      outcome: "Hermes no longer knows this run",
    });
    expect(turn).toMatchObject({ text: "", error: "Hermes no longer knows this run" });
  });
});

describe("the Kairos audit on tool rows (M2 Step 6)", () => {
  function audit(overrides: Partial<AuditEvent>): AuditEvent {
    return {
      at: 100,
      tool_call_id: "call_1",
      tool_name: "read_file",
      args_redacted: '{"path": "C:\\Users\\USER\\.ssh\\id_rsa"}',
      decision: "inherit",
      rule_id: null,
      status: "ok",
      error_type: null,
      duration_ms: 3,
      approval_choice: null,
      approval_decided_by: null,
      ...overrides,
    };
  }

  function turnWithTool(state: "ok" | "error"): Turn {
    return {
      ...newTurn("x"),
      status: "completed",
      items: [{ kind: "tool", toolCallId: "call_1", tool: "read_file", preview: "", state }],
    };
  }

  it("a call blocked by a Kairos rule is labelled blocked, with the rule", () => {
    const [turn] = attachAudit(
      [turnWithTool("error")],
      [audit({ decision: "deny", rule_id: "no-credentials", status: "blocked" })],
    );
    const row = turn.items[0];
    expect(row.kind === "tool" && toolLabel(turn, row)).toBe("blocked");
    expect(row.kind === "tool" && decisionText(row)).toBe("blocked by no-credentials");
  });

  it("an asked call says who decided", () => {
    const denied = audit({
      decision: "ask",
      rule_id: "destructive-commands",
      approval_choice: "deny",
    });
    expect(decisionText(toolRowFromAudit(denied))).toBe(
      "asked by destructive-commands, you denied it",
    );
    const smart = audit({
      decision: "inherit",
      approval_choice: "once",
      approval_decided_by: "aux_llm",
    });
    expect(decisionText(toolRowFromAudit(smart))).toBe(
      "Hermes asked, Hermes's helper model allowed it",
    );
  });

  it("a call no rule matched has no decision text", () => {
    expect(decisionText(toolRowFromAudit(audit({})))).toBeNull();
  });

  it("attaching the same audit twice changes nothing", () => {
    const events = [audit({ decision: "deny", rule_id: "no-credentials" })];
    const once = attachAudit([turnWithTool("error")], events);
    expect(attachAudit(once, events)[0]).toBe(once[0]);
  });

  it("a reopened conversation gets each task's tool calls back", () => {
    const task = (id: string, created_at: number, goal: string) => ({
      id,
      session_id: "kairos-s",
      run_id: `run_${id}`,
      goal,
      status: "completed" as const,
      created_at,
      finished_at: created_at + 5,
      outcome: "done",
    });
    const turns = turnsFromHistory(
      [task("a", 100, "read the readme"), task("b", 200, "read my key")],
      [
        audit({ at: 101.5, tool_call_id: "call_a", args_redacted: '{"path": "README.md"}' }),
        audit({
          at: 202,
          tool_call_id: "call_b",
          decision: "deny",
          rule_id: "no-credentials",
          status: "blocked",
        }),
      ],
    );
    expect(turns.map((t) => t.items.length)).toEqual([1, 1]);
    const [first, second] = turns.map((t) => t.items[0]);
    expect(first).toMatchObject({ kind: "tool", preview: "README.md", state: "ok" });
    expect(second.kind === "tool" && toolLabel(turns[1], second)).toBe("blocked");
  });
});

describe("approvals from a Kairos rule (M2 Step 8 recordings)", () => {
  it("a Kairos-rule approval names the rule and shows the real command, not Hermes's placeholder", () => {
    expect(
      readApproval(
        "<terminal> (plugin approval rule)",
        "Kairos rule 'destructive-commands': This command deletes data. — rm -rf C:/x",
      ),
    ).toEqual({
      source: "kairos",
      rule: "destructive-commands",
      description: "This command deletes data.",
      command: "rm -rf C:/x",
    });
    expect(readApproval("rm -rf C:/x", "recursive delete")).toEqual({
      source: "hermes",
      rule: null,
      description: "recursive delete",
      command: "rm -rf C:/x",
    });
  });

  it("a call denied at a Kairos rule counts as blocked though it never became a tool row", () => {
    const events = envelopes(kairosDenySse);
    let turn: Turn = { ...newTurn("x"), status: "running" };
    turn = applyEvent(turn, events[0], true);
    const [approval] = turn.items;
    expect(approval).toMatchObject({
      kind: "approval",
      source: "kairos",
      rule: "destructive-commands",
      command: "rm -rf C:/Users/USER/kairos-smart-test",
    });
    turn = markAnswered(turn, (approval as { promptId: string }).promptId, "deny");
    turn = events.slice(1).reduce((t, e) => applyEvent(t, e, true), turn);
    expect(turn.items.filter((i) => i.kind === "tool")).toHaveLength(0);
    expect(turn.status).toBe("completed");
    expect(blockedCount(turn)).toBe(1);
  });

  it("manual mode shows the Kairos card first, then Hermes's own", () => {
    const turn = play(doublePromptSse);
    const approvals = turn.items.filter((i) => i.kind === "approval");
    expect(approvals.map((a) => a.kind === "approval" && a.source)).toEqual(["kairos", "hermes"]);
    expect(turn.items.map((i) => i.kind)).toEqual(["approval", "tool", "approval"]);
  });
});
