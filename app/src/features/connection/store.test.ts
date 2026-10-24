import { describe, expect, it } from "vitest";
import { approvalWarning } from "./store";

describe("approvalWarning (D-009)", () => {
  it("is silent for the two accepted modes, smart (with Kairos rules) and manual", () => {
    expect(approvalWarning("smart")).toBeNull();
    expect(approvalWarning("manual")).toBeNull();
  });

  it("warns for modes that run commands without any check", () => {
    expect(approvalWarning("off")).toMatch(/'off'/);
    expect(approvalWarning("yolo")).toMatch(/'yolo'/);
  });

  it("warns when the mode couldn't be read", () => {
    expect(approvalWarning(null)).toMatch(/Couldn't read/);
  });
});
