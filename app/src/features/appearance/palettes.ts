// Themes (M16, D-031): each is a set of the base colours global.css derives everything from, in a
// dark and a light version, plus a pattern. Kept as data here (not CSS blocks) so the contrast
// test reads exactly the colours the windows show.

export type ThemeId = "charter" | "carthage" | "midnight" | "ember";
export type Mode = "system" | "dark" | "light";
export type PatternId = "diaper" | "stars" | "lattice" | "none";

export interface Appearance {
  theme: ThemeId;
  mode: Mode;
  /** null: the theme's own pattern. */
  pattern: PatternId | null;
}

export const DEFAULT_APPEARANCE: Appearance = { theme: "charter", mode: "system", pattern: null };

export interface Palette {
  charcoal: string; // the room
  hearth: string; // panels
  ember: string; // raised panels
  iron: string; // borders
  gold: string; // what Kairos offers: accents, links, primary buttons
  gules: string; // what is refused or failed
  gulesDeep: string;
  vert: string; // what is live
  vellum: string; // text
  ash: string; // quieter text
  warn: string;
  warnBg: string;
  codeBg: string;
  onGilt: string; // text on a gold button
  goldSoft: string;
  gulesSoft: string;
  vertSoft: string;
  sidebarBg: string; // the sidebar (and the island) stay dark in every mode
  sidebarFg: string;
  sidebarMuted: string;
  sidebarHover: string;
  sidebarActive: string;
  sidebarGold: string;
  sidebarVert: string;
  sidebarGules: string;
  wordmark: string;
  sealWax: string;
  sealEdge: string;
  sealLetter: string;
  /** The pattern's colour and strength over the room. */
  patternColor: string;
  patternAlpha: number;
}

export interface Theme {
  id: ThemeId;
  name: string;
  blurb: string;
  pattern: Exclude<PatternId, "none">;
  dark: Palette;
  light: Palette;
}

const CHARTER: Theme = {
  id: "charter",
  name: "Charter",
  blurb: "Charcoal, gold leaf and a crimson seal: the illuminated charter.",
  pattern: "diaper",
  dark: {
    charcoal: "#121110",
    hearth: "#1b1917",
    ember: "#24211d",
    iron: "#35312b",
    gold: "#c9a043",
    gules: "#e5847f",
    gulesDeep: "#a8343a",
    vert: "#7db394",
    vellum: "#ece4d3",
    ash: "#a89f8e",
    warn: "#e3b866",
    warnBg: "rgba(227, 184, 102, 0.12)",
    codeBg: "#0c0b0a",
    onGilt: "#1d1406",
    goldSoft: "rgba(201, 160, 67, 0.14)",
    gulesSoft: "rgba(196, 71, 76, 0.16)",
    vertSoft: "rgba(125, 179, 148, 0.15)",
    sidebarBg: "#0c0b0a",
    sidebarFg: "#ece4d3",
    sidebarMuted: "#a89f8e",
    sidebarHover: "#191715",
    sidebarActive: "#23201c",
    sidebarGold: "#c9a043",
    sidebarVert: "#7db394",
    sidebarGules: "#e5847f",
    wordmark: "#e2c47a",
    sealWax: "#9c2e33",
    sealEdge: "#7a2227",
    sealLetter: "#ecd28f",
    patternColor: "#c9a043",
    patternAlpha: 0.08,
  },
  light: {
    charcoal: "#ede5d4",
    hearth: "#f7f1e4",
    ember: "#e5dcc8",
    iron: "#cdbfa3",
    // Darkened from #86661d: that gold on the vellum room read at 4.26:1, under AA (found by the
    // M16 contrast test, 2026-10-29).
    gold: "#7f601b",
    gules: "#9c2c30",
    gulesDeep: "#a8343a",
    vert: "#2e7a55",
    vellum: "#2a2420",
    ash: "#6b5f50",
    warn: "#8a5a00",
    warnBg: "#f6e8c8",
    codeBg: "#e5dcc8",
    onGilt: "#fffaf0",
    goldSoft: "rgba(134, 102, 29, 0.12)",
    gulesSoft: "rgba(156, 44, 48, 0.1)",
    vertSoft: "rgba(46, 122, 85, 0.12)",
    sidebarBg: "#121110",
    sidebarFg: "#ece4d3",
    sidebarMuted: "#a89f8e",
    sidebarHover: "#201d1a",
    sidebarActive: "#2a2622",
    sidebarGold: "#c9a043",
    sidebarVert: "#7db394",
    sidebarGules: "#e5847f",
    wordmark: "#e2c47a",
    sealWax: "#9c2e33",
    sealEdge: "#7a2227",
    sealLetter: "#ecd28f",
    patternColor: "#86661d",
    patternAlpha: 0.12,
  },
};

const CARTHAGE: Theme = {
  id: "carthage",
  name: "Carthage",
  blurb: "Tyrian purple and gold, as Elissa and the Carthaginian projects.",
  pattern: "stars",
  dark: {
    charcoal: "#130c13",
    hearth: "#1c121d",
    ember: "#271a29",
    iron: "#3d2b40",
    gold: "#d6b04a",
    gules: "#ef8fa0",
    gulesDeep: "#a12a45",
    vert: "#86c2a0",
    vellum: "#f0e6ef",
    ash: "#b3a1b4",
    warn: "#e8c06a",
    warnBg: "rgba(232, 192, 106, 0.12)",
    codeBg: "#0d080d",
    onGilt: "#1e1407",
    goldSoft: "rgba(214, 176, 74, 0.14)",
    gulesSoft: "rgba(161, 42, 69, 0.2)",
    vertSoft: "rgba(134, 194, 160, 0.15)",
    sidebarBg: "#0d080d",
    sidebarFg: "#f0e6ef",
    sidebarMuted: "#b3a1b4",
    sidebarHover: "#1c121d",
    sidebarActive: "#2a1a2d",
    sidebarGold: "#d6b04a",
    sidebarVert: "#86c2a0",
    sidebarGules: "#ef8fa0",
    wordmark: "#e6c86e",
    sealWax: "#66206b",
    sealEdge: "#4a1650",
    sealLetter: "#ecd28f",
    patternColor: "#b36cb8",
    patternAlpha: 0.09,
  },
  light: {
    charcoal: "#f1e9e4",
    hearth: "#faf5f1",
    ember: "#e9ddd7",
    iron: "#d3c1c9",
    gold: "#6e2366",
    gules: "#9f2236",
    gulesDeep: "#a12a45",
    vert: "#2b7550",
    vellum: "#2a1d29",
    ash: "#6c5868",
    warn: "#8a5a00",
    warnBg: "#f6e8c8",
    codeBg: "#e9ddd7",
    onGilt: "#fff8fd",
    goldSoft: "rgba(110, 35, 102, 0.1)",
    gulesSoft: "rgba(159, 34, 54, 0.1)",
    vertSoft: "rgba(43, 117, 80, 0.12)",
    sidebarBg: "#1c121d",
    sidebarFg: "#f0e6ef",
    sidebarMuted: "#b3a1b4",
    sidebarHover: "#271a29",
    sidebarActive: "#33213a",
    sidebarGold: "#d6b04a",
    sidebarVert: "#86c2a0",
    sidebarGules: "#ef8fa0",
    wordmark: "#e6c86e",
    sealWax: "#66206b",
    sealEdge: "#4a1650",
    sealLetter: "#ecd28f",
    patternColor: "#6e2366",
    patternAlpha: 0.1,
  },
};

const MIDNIGHT: Theme = {
  id: "midnight",
  name: "Midnight",
  blurb: "Deep navy, silver and ice-blue: calm and cool.",
  pattern: "lattice",
  dark: {
    charcoal: "#0b111d",
    hearth: "#111a2a",
    ember: "#182337",
    iron: "#263450",
    gold: "#a9cdf2",
    gules: "#f29494",
    gulesDeep: "#b0343c",
    vert: "#82d2ad",
    vellum: "#e6ebf2",
    ash: "#9aa7b9",
    warn: "#e9c46a",
    warnBg: "rgba(233, 196, 106, 0.12)",
    codeBg: "#070c16",
    onGilt: "#0b1626",
    goldSoft: "rgba(169, 205, 242, 0.13)",
    gulesSoft: "rgba(176, 52, 60, 0.18)",
    vertSoft: "rgba(130, 210, 173, 0.14)",
    sidebarBg: "#070c16",
    sidebarFg: "#e6ebf2",
    sidebarMuted: "#9aa7b9",
    sidebarHover: "#111a2a",
    sidebarActive: "#1a2740",
    sidebarGold: "#a9cdf2",
    sidebarVert: "#82d2ad",
    sidebarGules: "#f29494",
    wordmark: "#cfe0f5",
    sealWax: "#1f3d66",
    sealEdge: "#16304f",
    sealLetter: "#dbe8f7",
    patternColor: "#a9cdf2",
    patternAlpha: 0.07,
  },
  light: {
    charcoal: "#e7ecf2",
    hearth: "#f5f7fa",
    ember: "#dce3ec",
    iron: "#c1ccd9",
    gold: "#1d4b85",
    gules: "#a3242f",
    gulesDeep: "#b0343c",
    vert: "#23704e",
    vellum: "#152033",
    ash: "#536277",
    warn: "#8a5a00",
    warnBg: "#f6e8c8",
    codeBg: "#dce3ec",
    onGilt: "#f6faff",
    goldSoft: "rgba(29, 75, 133, 0.1)",
    gulesSoft: "rgba(163, 36, 47, 0.1)",
    vertSoft: "rgba(35, 112, 78, 0.12)",
    sidebarBg: "#0f1a2b",
    sidebarFg: "#e6ebf2",
    sidebarMuted: "#9aa7b9",
    sidebarHover: "#17243a",
    sidebarActive: "#1f2e48",
    sidebarGold: "#a9cdf2",
    sidebarVert: "#82d2ad",
    sidebarGules: "#f29494",
    wordmark: "#cfe0f5",
    sealWax: "#1f3d66",
    sealEdge: "#16304f",
    sealLetter: "#dbe8f7",
    patternColor: "#1d4b85",
    patternAlpha: 0.09,
  },
};

const EMBER: Theme = {
  id: "ember",
  name: "Ember",
  blurb: "Black, crimson and ember orange: strong and warm.",
  pattern: "diaper",
  dark: {
    charcoal: "#0e0b0b",
    hearth: "#171212",
    ember: "#221918",
    iron: "#382725",
    gold: "#f0924c",
    gules: "#ff8080",
    gulesDeep: "#b3261e",
    vert: "#7fbf98",
    vellum: "#f3e7e2",
    ash: "#b09d97",
    warn: "#f0c060",
    warnBg: "rgba(240, 192, 96, 0.12)",
    codeBg: "#090606",
    onGilt: "#200c03",
    goldSoft: "rgba(240, 146, 76, 0.14)",
    gulesSoft: "rgba(179, 38, 30, 0.2)",
    vertSoft: "rgba(127, 191, 152, 0.14)",
    sidebarBg: "#090606",
    sidebarFg: "#f3e7e2",
    sidebarMuted: "#b09d97",
    sidebarHover: "#171212",
    sidebarActive: "#241a19",
    sidebarGold: "#f0924c",
    sidebarVert: "#7fbf98",
    sidebarGules: "#ff8080",
    wordmark: "#f5a868",
    sealWax: "#b3261e",
    sealEdge: "#7d1a14",
    sealLetter: "#ffd2a6",
    patternColor: "#e0563c",
    patternAlpha: 0.08,
  },
  light: {
    charcoal: "#f2eae5",
    hearth: "#fbf6f3",
    ember: "#e9ded7",
    iron: "#d5c3b9",
    gold: "#a33a0d",
    gules: "#a0172a",
    gulesDeep: "#b3261e",
    vert: "#2b744f",
    vellum: "#2b1c18",
    ash: "#6e5a53",
    warn: "#8a5a00",
    warnBg: "#f6e8c8",
    codeBg: "#e9ded7",
    onGilt: "#fff8f3",
    goldSoft: "rgba(163, 58, 13, 0.1)",
    gulesSoft: "rgba(160, 23, 42, 0.1)",
    vertSoft: "rgba(43, 116, 79, 0.12)",
    sidebarBg: "#1a1111",
    sidebarFg: "#f3e7e2",
    sidebarMuted: "#b09d97",
    sidebarHover: "#241a19",
    sidebarActive: "#2e2120",
    sidebarGold: "#f0924c",
    sidebarVert: "#7fbf98",
    sidebarGules: "#ff8080",
    wordmark: "#f5a868",
    sealWax: "#b3261e",
    sealEdge: "#7d1a14",
    sealLetter: "#ffd2a6",
    patternColor: "#a33a0d",
    patternAlpha: 0.1,
  },
};

export const THEMES: Theme[] = [CHARTER, CARTHAGE, MIDNIGHT, EMBER];

export const PATTERNS: { id: PatternId; name: string }[] = [
  { id: "diaper", name: "Diaper (lozenges)" },
  { id: "stars", name: "Stars" },
  { id: "lattice", name: "Lattice" },
  { id: "none", name: "None" },
];

export function themeOf(id: string): Theme {
  return THEMES.find((t) => t.id === id) ?? CHARTER;
}

/** The pattern as a CSS url(): small SVG tiles in the theme's colour, faint. */
export function patternCss(id: PatternId, color: string, alpha: number): string {
  if (id === "none") return "none";
  const c = encodeURIComponent(color);
  const svg =
    id === "diaper"
      ? `<svg xmlns='http://www.w3.org/2000/svg' width='56' height='56' viewBox='0 0 56 56'><path d='M0 28L28 0L56 28L28 56Z' fill='none' stroke='${c}' stroke-opacity='${alpha * 0.94}' stroke-width='0.8'/><g fill='${c}' fill-opacity='${alpha * 1.12}'><circle cx='28' cy='25.2' r='2'/><circle cx='30.8' cy='28' r='2'/><circle cx='28' cy='30.8' r='2'/><circle cx='25.2' cy='28' r='2'/><circle cx='0' cy='28' r='1'/><circle cx='56' cy='28' r='1'/><circle cx='28' cy='0' r='1'/><circle cx='28' cy='56' r='1'/></g></svg>`
      : id === "stars"
        ? `<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64' viewBox='0 0 64 64'><g fill='${c}' fill-opacity='${alpha * 1.2}'><path d='M32 22l2.2 5.8L40 30l-5.8 2.2L32 38l-2.2-5.8L24 30l5.8-2.2z'/><circle cx='0' cy='0' r='1.2'/><circle cx='64' cy='0' r='1.2'/><circle cx='0' cy='64' r='1.2'/><circle cx='64' cy='64' r='1.2'/></g></svg>`
        : `<svg xmlns='http://www.w3.org/2000/svg' width='40' height='40' viewBox='0 0 40 40'><path d='M0 0L40 40M40 0L0 40' stroke='${c}' stroke-opacity='${alpha}' stroke-width='0.7'/></svg>`;
  return `url("data:image/svg+xml,${svg.replace(/</g, "%3C").replace(/>/g, "%3E")}")`;
}

/** The CSS variables a palette sets on <html>; global.css derives the rest from them. */
export function cssVars(p: Palette, pattern: PatternId): Record<string, string> {
  return {
    "--charcoal": p.charcoal,
    "--hearth": p.hearth,
    "--ember": p.ember,
    "--iron": p.iron,
    "--gold": p.gold,
    "--gold-soft": p.goldSoft,
    "--gules": p.gules,
    "--gules-deep": p.gulesDeep,
    "--gules-soft": p.gulesSoft,
    "--vert": p.vert,
    "--vert-soft": p.vertSoft,
    "--vellum": p.vellum,
    "--ash": p.ash,
    "--warn": p.warn,
    "--warn-bg": p.warnBg,
    "--code-bg": p.codeBg,
    "--on-gilt": p.onGilt,
    "--sidebar-bg": p.sidebarBg,
    "--sidebar-fg": p.sidebarFg,
    "--sidebar-muted": p.sidebarMuted,
    "--sidebar-hover": p.sidebarHover,
    "--sidebar-active": p.sidebarActive,
    "--sidebar-gold": p.sidebarGold,
    "--sidebar-vert": p.sidebarVert,
    "--sidebar-gules": p.sidebarGules,
    "--wordmark": p.wordmark,
    "--seal-wax": p.sealWax,
    "--seal-edge": p.sealEdge,
    "--seal-letter": p.sealLetter,
    "--pattern": patternCss(pattern, p.patternColor, p.patternAlpha),
  };
}

/** Dark or light for this appearance, given whether Windows is in light mode. */
export function effectiveMode(appearance: Appearance, prefersLight: boolean): "dark" | "light" {
  if (appearance.mode === "system") return prefersLight ? "light" : "dark";
  return appearance.mode;
}

// --- Contrast (WCAG 2.1) --------------------------------------------------------------------------

function channel(v: number): number {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  return (
    0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255)
  );
}

/** The contrast ratio of two #rrggbb colours, 1 to 21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
