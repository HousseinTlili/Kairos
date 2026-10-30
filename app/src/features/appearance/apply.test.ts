import { describe, expect, it } from "vitest";
import { mix, readAppearance } from "./apply";

describe("the stored appearance", () => {
  it("reads back what was saved", () => {
    expect(readAppearance({ theme: "carthage", mode: "light", pattern: "stars" })).toEqual({
      theme: "carthage",
      mode: "light",
      pattern: "stars",
    });
  });

  it("falls back to Charter, following Windows, with the theme's pattern", () => {
    expect(readAppearance(null)).toEqual({ theme: "charter", mode: "system", pattern: null });
    expect(readAppearance({ theme: "neon", mode: "dim", pattern: "polka" })).toEqual({
      theme: "charter",
      mode: "system",
      pattern: null,
    });
  });
});

describe("the title bar's edge", () => {
  it("mixes two colours part of the way", () => {
    expect(mix("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(mix("#c9a043", "#0c0b0a", 0)).toBe("#c9a043");
    expect(mix("#c9a043", "#0c0b0a", 1)).toBe("#0c0b0a");
  });
});
