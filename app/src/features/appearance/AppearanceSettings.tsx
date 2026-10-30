import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { readAppearance, saveAppearance } from "./apply";
import {
  DEFAULT_APPEARANCE,
  PATTERNS,
  THEMES,
  effectiveMode,
  patternCss,
  type Appearance,
  type Mode,
  type PatternId,
  type Theme,
} from "./palettes";

const MODES: { id: Mode; label: string }[] = [
  { id: "system", label: "Follow Windows" },
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
];

/** A theme's card: a small room in its own colours, with its seal, a line of text and its gold. */
function ThemePreview({
  theme,
  mode,
  active,
  onPick,
}: {
  theme: Theme;
  mode: "dark" | "light";
  active: boolean;
  onPick: () => void;
}) {
  const p = theme[mode];
  return (
    <button
      className={`theme-card ${active ? "active" : ""}`}
      onClick={onPick}
      aria-pressed={active}
      style={{
        background: p.charcoal,
        backgroundImage: patternCss(theme.pattern, p.patternColor, p.patternAlpha * 1.6),
        color: p.vellum,
        borderColor: active ? p.gold : p.iron,
      }}
    >
      <span className="theme-card-side" style={{ background: p.sidebarBg }}>
        <span className="theme-card-seal" style={{ background: p.sealWax, color: p.sealLetter }}>
          K
        </span>
      </span>
      <span className="theme-card-body">
        <span className="theme-card-name" style={{ color: p.vellum }}>
          {theme.name}
        </span>
        <span className="theme-card-line" style={{ color: p.ash }}>
          {theme.blurb}
        </span>
        <span className="theme-card-gold" style={{ background: p.gold, color: p.onGilt }}>
          Grant once
        </span>
      </span>
    </button>
  );
}

/** Settings ▸ Kairos ▸ Appearance (M16): the theme, light or dark, and the pattern. */
export function AppearanceSettings() {
  const [appearance, setAppearance] = useState<Appearance>(DEFAULT_APPEARANCE);
  const [error, setError] = useState<string | null>(null);
  const prefersLight = matchMedia("(prefers-color-scheme: light)").matches;

  useEffect(() => {
    void invoke<unknown>("appearance_state").then((stored) =>
      setAppearance(readAppearance(stored)),
    );
    // A change made elsewhere (another window) shows here too.
    const stop = listen<unknown>("appearance-changed", (event) =>
      setAppearance(readAppearance(event.payload)),
    );
    return () => void stop.then((unlisten) => unlisten());
  }, []);

  function change(next: Partial<Appearance>) {
    const updated = { ...appearance, ...next };
    setAppearance(updated);
    void saveAppearance(updated)
      .then(() => setError(null))
      .catch((err: { message?: string }) => setError(err.message ?? String(err)));
  }

  const mode = effectiveMode(appearance, prefersLight);
  const theme = THEMES.find((t) => t.id === appearance.theme) ?? THEMES[0];
  return (
    <div className="settings-section">
      <h2>Appearance</h2>
      <p className="muted small">
        A theme colours all of Kairos at once: this window, the quick box, the island, the title
        bar, tables and diagrams. Each has a dark and a light version.
      </p>
      <div className="theme-grid">
        {THEMES.map((t) => (
          <ThemePreview
            key={t.id}
            theme={t}
            mode={mode}
            active={t.id === appearance.theme}
            onPick={() => change({ theme: t.id })}
          />
        ))}
      </div>
      <div className="briefing-fields">
        <label>
          <span>Light or dark</span>
          <select
            value={appearance.mode}
            onChange={(e) => change({ mode: e.target.value as Mode })}
          >
            {MODES.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Pattern</span>
          <select
            value={appearance.pattern ?? ""}
            onChange={(e) => change({ pattern: (e.target.value || null) as PatternId | null })}
          >
            <option value="">
              The theme&apos;s own ({PATTERNS.find((x) => x.id === theme.pattern)?.name})
            </option>
            {PATTERNS.map((x) => (
              <option key={x.id} value={x.id}>
                {x.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      {error && <p className="bad-text">{error}</p>}
    </div>
  );
}
