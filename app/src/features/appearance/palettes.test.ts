import { describe, expect, it } from "vitest";
import { THEMES, contrast, cssVars, effectiveMode, patternCss } from "./palettes";

// WCAG 2.1 AA: 4.5:1 for text, 3:1 for large text and for the borders and edges that matter.
const TEXT = 4.5;
const LARGE = 3;

describe("every theme stays readable (M16, D-031)", () => {
  for (const theme of THEMES) {
    for (const mode of ["dark", "light"] as const) {
      const p = theme[mode];
      it(`${theme.name} ${mode}: text, quiet text and accents`, () => {
        expect(contrast(p.vellum, p.charcoal)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.vellum, p.hearth)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.vellum, p.ember)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.ash, p.charcoal)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.ash, p.hearth)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.gold, p.charcoal)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.gold, p.hearth)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.gules, p.hearth)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.vert, p.hearth)).toBeGreaterThanOrEqual(LARGE);
        expect(contrast(p.onGilt, p.gold)).toBeGreaterThanOrEqual(TEXT);
      });
      it(`${theme.name} ${mode}: the sidebar and the island`, () => {
        expect(contrast(p.sidebarFg, p.sidebarBg)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.sidebarMuted, p.sidebarBg)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.sidebarFg, p.sidebarActive)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.sidebarGold, p.sidebarBg)).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(p.wordmark, p.sidebarBg)).toBeGreaterThanOrEqual(LARGE);
        expect(contrast(p.sealLetter, p.sealWax)).toBeGreaterThanOrEqual(LARGE);
      });
    }
  }
});

describe("applying a theme", () => {
  it("follows Windows unless told dark or light", () => {
    const a = { theme: "charter" as const, pattern: null };
    expect(effectiveMode({ ...a, mode: "system" }, true)).toBe("light");
    expect(effectiveMode({ ...a, mode: "system" }, false)).toBe("dark");
    expect(effectiveMode({ ...a, mode: "dark" }, true)).toBe("dark");
  });

  it("sets every base colour and the pattern, and no pattern is none", () => {
    const vars = cssVars(THEMES[0].dark, "diaper");
    expect(vars["--charcoal"]).toBe("#121110");
    expect(vars["--pattern"]).toMatch(/^url\("data:image\/svg\+xml,/);
    expect(patternCss("none", "#000000", 0.1)).toBe("none");
  });

  it("measures contrast like WCAG", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrast("#777777", "#777777")).toBe(1);
  });
});
