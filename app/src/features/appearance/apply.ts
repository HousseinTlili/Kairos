// Applies the Master's theme in this window (M16, D-031): the palette's colours on <html>, the
// pattern, light or dark, and in the main window the title bar. Every window runs this at start
// and again when the appearance changes anywhere.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  DEFAULT_APPEARANCE,
  cssVars,
  effectiveMode,
  themeOf,
  type Appearance,
  type Mode,
  type PatternId,
  type ThemeId,
} from "./palettes";

const CACHE_KEY = "kairos.appearance";
const LIGHT = "(prefers-color-scheme: light)";

/** A stored value back into an Appearance, anything unknown replaced by the default. */
export function readAppearance(raw: unknown): Appearance {
  const value = (raw ?? {}) as Partial<Record<keyof Appearance, unknown>>;
  const themes: ThemeId[] = ["charter", "carthage", "midnight", "ember"];
  const modes: Mode[] = ["system", "dark", "light"];
  const patterns: PatternId[] = ["diaper", "stars", "lattice", "none"];
  return {
    theme: themes.includes(value.theme as ThemeId)
      ? (value.theme as ThemeId)
      : DEFAULT_APPEARANCE.theme,
    mode: modes.includes(value.mode as Mode) ? (value.mode as Mode) : DEFAULT_APPEARANCE.mode,
    pattern: patterns.includes(value.pattern as PatternId) ? (value.pattern as PatternId) : null,
  };
}

/** "#rrggbb" a part of the way from one colour to another. */
export function mix(from: string, to: string, amount: number): string {
  const a = parseInt(from.slice(1), 16);
  const b = parseInt(to.slice(1), 16);
  const part = (shift: number) => {
    const x = (a >> shift) & 255;
    const y = (b >> shift) & 255;
    return Math.round(x + (y - x) * amount);
  };
  return `#${[16, 8, 0].map((s) => part(s).toString(16).padStart(2, "0")).join("")}`;
}

let current: Appearance = DEFAULT_APPEARANCE;
let label = "main";

function apply(appearance: Appearance) {
  current = appearance;
  const theme = themeOf(appearance.theme);
  const mode = effectiveMode(appearance, matchMedia(LIGHT).matches);
  const palette = theme[mode];
  const root = document.documentElement;
  for (const [name, value] of Object.entries(
    cssVars(palette, appearance.pattern ?? theme.pattern),
  )) {
    root.style.setProperty(name, value);
  }
  root.style.colorScheme = mode;
  root.dataset.theme = theme.id;
  root.dataset.mode = mode;
  if (label === "main") {
    // The title bar is panelling like the sidebar; its edge is the gold, darkened to a line.
    void invoke("paint_titlebar", {
      caption: palette.sidebarBg,
      text: palette.sidebarFg,
      border: mix(palette.sidebarGold, palette.sidebarBg, 0.55),
    }).catch(() => {});
  }
}

function cache(appearance: Appearance) {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(appearance));
  } catch {
    // A private profile just reads it from Kairos each time.
  }
}

/** Called once per window, before it renders, so the first paint is already in the right theme. */
export function startAppearance(windowLabel: string) {
  label = windowLabel;
  let cached: unknown;
  try {
    cached = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
  } catch {
    cached = null;
  }
  apply(readAppearance(cached));
  void invoke<unknown>("appearance_state")
    .then((stored) => {
      const appearance = readAppearance(stored);
      cache(appearance);
      apply(appearance);
    })
    .catch(() => {});
  void listen<unknown>("appearance-changed", (event) => {
    const appearance = readAppearance(event.payload);
    cache(appearance);
    apply(appearance);
  });
  // "Follow Windows": switching Windows to light or dark switches Kairos too.
  matchMedia(LIGHT).addEventListener("change", () => apply(current));
}

/** Saves a new appearance; every window applies it through `appearance-changed`. */
export function saveAppearance(appearance: Appearance): Promise<void> {
  return invoke("save_appearance", { appearance });
}
