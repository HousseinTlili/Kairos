import { describe, expect, it } from "vitest";
import { approvalWarning } from "./store";

describe("approvalWarning (D-009)", () => {
  it("is silent only for manual mode", () => {
    expect(approvalWarning("manual")).toBeNull();
  });

  it("warns that smart mode can auto-approve destructive commands", () => {
    expect(approvalWarning("smart")).toMatch(/auto-approve destructive commands/);
  });

  it("warns for off and yolo", () => {
    expect(approvalWarning("off")).toMatch(/'off'/);
    expect(approvalWarning("yolo")).toMatch(/'yolo'/);
  });

  it("warns when the mode couldn't be read", () => {
    expect(approvalWarning(null)).toMatch(/Couldn't read/);
  });
});
