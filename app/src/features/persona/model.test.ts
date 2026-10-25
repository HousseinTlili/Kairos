import { describe, expect, it } from "vitest";
import { personaLabel, sameText } from "./model";

describe("the persona page", () => {
  it("names what the saved file is", () => {
    expect(personaLabel("kairos", "You are Kairos.", "You are Kairos.")).toBe("Kairos persona");
    expect(personaLabel("hermes_default", "", "")).toBe("Hermes's default");
  });

  it("says when the editor holds changes not saved yet", () => {
    expect(personaLabel("custom", "A", "B")).toBe("Your own text (unsaved changes)");
  });

  it("doesn't count line ends or trailing space as changes", () => {
    expect(sameText("a\r\nb\n", "a\nb")).toBe(true);
    expect(sameText("a", "b")).toBe(false);
  });
});
