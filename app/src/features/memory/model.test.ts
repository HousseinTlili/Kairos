import { describe, expect, it } from "vitest";
import { categoryLabel, forgetRequest, fullness, isMemoryConversation } from "./model";

describe("forgetting", () => {
  it("asks Kairos to remove exactly one entry with its own tool", () => {
    const text = forgetRequest({ kind: "user", text: "Prefers short answers." });
    expect(text).toContain("memory tool");
    expect(text).toContain('target "user"');
    expect(text).toContain("«Prefers short answers.»");
    expect(text).toContain("Do nothing else");
    expect(forgetRequest({ kind: "notes", text: "x" })).toContain('target "memory"');
  });

  it("forgets a fact by its id, history and all", () => {
    const text = forgetRequest({
      kind: "fact",
      fact: { id: 12, content: "Likes tea", category: "user_pref", trust: 0.5, updated_at: "" },
    });
    expect(text).toContain('action "forget" and fact_id 12');
    expect(text).toContain("«Likes tea»");
  });

  it("keeps its own conversation out of the sidebar, and only that one", () => {
    expect(isMemoryConversation("kairos-memory")).toBe(true);
    expect(isMemoryConversation("kairos-memory-2")).toBe(true);
    expect(isMemoryConversation("kairos-memoryless")).toBe(false);
    expect(isMemoryConversation("20261027_abc")).toBe(false);
  });
});

describe("the memory page", () => {
  it("says how full a file is", () => {
    expect(fullness(312, 1375)).toBe("312 of 1,375 characters (23 %)");
    expect(fullness(0, 0)).toBe("0 of 0 characters (0 %)");
  });

  it("names categories in words", () => {
    expect(categoryLabel("user_pref")).toBe("Preference");
    expect(categoryLabel("custom")).toBe("custom");
  });
});
