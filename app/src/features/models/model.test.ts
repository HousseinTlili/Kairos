import { describe, expect, it } from "vitest";
import { sandboxText } from "./model";

describe("the sandbox line", () => {
  it("is reassuring only for the container", () => {
    expect(sandboxText("docker").tone).toBe("ok");
    expect(sandboxText("local").tone).toBe("warn");
    expect(sandboxText("local").text).toMatch(/nothing else contains it/);
    expect(sandboxText(null).tone).toBe("muted");
    expect(sandboxText("ssh").text).toContain("ssh");
  });
});
