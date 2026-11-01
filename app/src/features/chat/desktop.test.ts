import { describe, expect, it } from "vitest";
import clickTypeSse from "../../../../fixtures/hermes-0.22.5/computer-use/run-click-type.sse?raw";
import stopSse from "../../../../fixtures/hermes-0.22.5/computer-use/run-stop-at-card.sse?raw";
import type { RunEvent, RunEventEnvelope } from "../../lib/hermes-events";
import type { AuditEvent } from "../../lib/tauri";
import {
  desktopCardWords,
  desktopPhrase,
  desktopTarget,
  isDesktopApproval,
  offeredChoices,
  waitingApprovals,
} from "./desktop";
import {
  applyEvent,
  attachAudit,
  newTurn,
  readApproval,
  type ApprovalRow,
  type ToolRow,
  type Turn,
} from "./turn";

// As in turn.test.ts: recorded lines are flat, and the Rust side wraps them.
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

function replay(sse: string): Turn {
  return envelopes(sse).reduce(
    (turn, envelope) => applyEvent(turn, envelope, true),
    newTurn("probe"),
  );
}

const audit = (id: string | null, args: object): AuditEvent =>
  ({
    at: 1,
    tool_call_id: id,
    tool_name: "computer_use",
    args_redacted: JSON.stringify(args),
    decision: "inherit",
    rule_id: null,
    status: null,
    error_type: null,
    duration_ms: null,
    approval_choice: null,
    approval_decided_by: null,
  }) as AuditEvent;

const tool = (args: object): ToolRow => ({
  kind: "tool",
  toolCallId: "c",
  tool: "computer_use",
  preview: "",
  state: "running",
  audit: audit("c", args),
});

const card = (command: string, extra: Partial<ApprovalRow> = {}): ApprovalRow => ({
  kind: "approval",
  promptId: "p",
  executionGeneration: 1,
  ...readApproval(command, `Allow computer_use to perform something?`),
  choices: ["once", "deny", "session", "always"],
  live: true,
  state: "waiting",
  choice: null,
  ...extra,
});

describe("a desktop call in words", () => {
  const target = { kairos_target: "Character Map" };

  it("names the button and the app", () => {
    expect(desktopPhrase(tool({ action: "click", kairos_element: "Select", ...target }))).toBe(
      "Click Select in Character Map",
    );
  });

  it("shows what will be typed, whole", () => {
    const long = "x".repeat(200);
    expect(desktopPhrase(tool({ action: "type", text: long, ...target }))).toContain(long);
  });

  it("says a click with no named element is on a spot, rather than guess", () => {
    expect(desktopPhrase(tool({ action: "click", coordinate: [1, 2], ...target }))).toBe(
      "Click a spot in Character Map",
    );
  });

  it("is empty for a row the plugin hasn't described, so the card keeps Hermes's own text", () => {
    const row: ToolRow = { ...tool({}), audit: undefined };
    expect(desktopPhrase(row)).toBe("");
    expect(desktopPhrase({ ...tool({}), audit: audit("c", { nothing: 1 }) })).toBe("");
    expect(
      desktopPhrase({ ...tool({}), audit: { ...audit("c", {}), args_redacted: "not json" } }),
    ).toBe("");
  });
});

describe("what a desktop card offers", () => {
  it("never offers always, for any desktop action", () => {
    for (const command of ["computer_use: click element #1", "computer_use: type 'abc'"]) {
      expect(offeredChoices(card(command))).not.toContain("always");
    }
    expect(
      offeredChoices(card("x", { rule: "computer-use-see", source: "kairos", command: "x" })),
    ).toEqual(["once", "deny", "session"]);
  });

  it("offers a click for this conversation", () => {
    expect(offeredChoices(card("computer_use: click element #1"))).toEqual([
      "once",
      "deny",
      "session",
    ]);
  });

  it("offers typing and keys only once", () => {
    for (const command of [
      "computer_use: type 'abc'",
      "computer_use: key 'ctrl+s'",
      "computer_use: set_value",
    ]) {
      expect(offeredChoices(card(command))).toEqual(["once", "deny"]);
    }
  });

  it("leaves other tools' cards alone", () => {
    expect(offeredChoices(card("rm -rf x"))).toEqual(["once", "deny", "session", "always"]);
  });
});

describe("the recorded run", () => {
  it("gives one card per action, each read as a desktop card", () => {
    const turn = replay(clickTypeSse);
    const cards = turn.items.filter((i): i is ApprovalRow => i.kind === "approval");
    expect(cards.map((c) => c.command)).toEqual([
      "computer_use: capture",
      "computer_use: click element #1",
      "computer_use: type 'abc'",
    ]);
    expect(cards.every(isDesktopApproval)).toBe(true);
    // Seeing is the Kairos rule's card, and input Hermes's: never both for one action.
    expect(cards.map((c) => c.source)).toEqual(["kairos", "hermes", "hermes"]);
    expect(cards[0].rule).toBe("computer-use-see");
    expect(cards[0].will).toBe("see Character Map, and send the picture to your AI provider");
  });

  it("words a card from the audit row of the call before it", () => {
    const parsed = replay(clickTypeSse);
    const ids = parsed.items
      .filter((i): i is ToolRow => i.kind === "tool" && i.tool === "computer_use")
      .map((i) => i.toolCallId);
    const events = ids.map((id) =>
      audit(id, { action: "click", kairos_target: "Character Map", kairos_element: "Select" }),
    );
    const [turn] = attachAudit([parsed], events);
    const first = turn.items.find(
      (i): i is ApprovalRow => i.kind === "approval" && i.source === "hermes",
    );
    expect(first && desktopCardWords(turn, first)).toBe("Click Select in Character Map");
  });

  it("keeps Hermes's text when the audit row hasn't been read", () => {
    const turn = replay(clickTypeSse);
    const first = turn.items.find(
      (i): i is ApprovalRow => i.kind === "approval" && i.source === "hermes",
    );
    expect(first && desktopCardWords(turn, first)).toBe("");
  });

  it("ends as cancelled when stopped at a card", () => {
    expect(replay(stopSse).status).toBe("cancelled");
  });
});

describe("where it's working", () => {
  const working = (status: Turn["status"]): Turn => ({
    ...newTurn("x"),
    status,
    items: [tool({ action: "capture", kairos_target: "Character Map" })],
  });

  it("names the app while the request runs or waits", () => {
    expect(desktopTarget(working("running"))).toBe("Character Map");
    expect(desktopTarget(working("waiting_for_approval"))).toBe("Character Map");
  });

  it("says nothing once it's over, or when it isn't on the desktop", () => {
    expect(desktopTarget(working("completed"))).toBeNull();
    expect(desktopTarget(newTurn("x"))).toBeNull();
    expect(desktopTarget(undefined)).toBeNull();
  });

  it("counts only the approvals that can be answered", () => {
    const turn: Turn = {
      ...newTurn("x"),
      items: [
        card("computer_use: click element #1"),
        card("rm x", { live: false }),
        card("rm y", { state: "settled" }),
      ],
    };
    expect(waitingApprovals([turn])).toBe(1);
  });
});
