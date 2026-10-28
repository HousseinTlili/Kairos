// Mermaid diagrams in answers (M13 Step 3, D-028). Mermaid is large, so it is loaded the first time
// an answer holds a diagram; it runs with securityLevel "strict" (no HTML labels, no click
// handlers, its SVG sanitised), and each drawing is kept so a re-render doesn't redraw it.

import { useSyncExternalStore } from "react";
import type { Mermaid } from "mermaid";

export type Scheme = "dark" | "light";

const LIGHT = "(prefers-color-scheme: light)";

function subscribe(onChange: () => void) {
  const media = matchMedia(LIGHT);
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}

/** The colour scheme, so diagrams are redrawn in the charter's light or dark colours. */
export function useScheme(): Scheme {
  return useSyncExternalStore(subscribe, () => (matchMedia(LIGHT).matches ? "light" : "dark"));
}

let mermaid: Promise<Mermaid> | null = null;
const drawn = new Map<string, string>();
let count = 0;

function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** The charter's colours as Mermaid theme variables, read from the CSS tokens. */
function themeVariables(): Record<string, unknown> {
  const fg = token("--fg");
  const gilt = token("--gilt");
  const surface = token("--surface");
  const raised = token("--raised");
  return {
    background: surface,
    mainBkg: raised,
    primaryColor: raised,
    primaryTextColor: fg,
    primaryBorderColor: gilt,
    secondaryColor: surface,
    tertiaryColor: token("--bg"),
    nodeBorder: gilt,
    lineColor: gilt,
    textColor: fg,
    titleColor: gilt,
    clusterBkg: surface,
    clusterBorder: token("--border"),
    edgeLabelBackground: surface,
    actorBkg: raised,
    actorBorder: gilt,
    actorTextColor: fg,
    signalColor: fg,
    signalTextColor: fg,
    noteBkgColor: surface,
    noteTextColor: fg,
    pie1: gilt,
    pie2: token("--lume"),
    pie3: token("--garnet"),
    pie4: token("--slate"),
    pieTitleTextColor: fg,
    pieSectionTextColor: token("--on-gilt"),
    pieLegendTextColor: fg,
    pieStrokeColor: surface,
    xyChart: {
      backgroundColor: surface,
      titleColor: fg,
      xAxisLabelColor: fg,
      yAxisLabelColor: fg,
      xAxisTitleColor: fg,
      yAxisTitleColor: fg,
      xAxisLineColor: token("--border"),
      yAxisLineColor: token("--border"),
      xAxisTickColor: token("--border"),
      yAxisTickColor: token("--border"),
      plotColorPalette: [gilt, token("--lume"), token("--garnet"), token("--slate")].join(", "),
    },
  };
}

/** The SVG for a diagram, or an error when Mermaid can't read it. */
export async function drawDiagram(code: string, scheme: Scheme): Promise<string> {
  const key = `${scheme}\n${code}`;
  const known = drawn.get(key);
  if (known) return known;
  mermaid ??= import("mermaid").then((module) => module.default);
  const m = await mermaid;
  m.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    fontFamily: token("--font-ui") || "Segoe UI, sans-serif",
    themeVariables: themeVariables(),
    flowchart: { htmlLabels: false },
  });
  // Parse first: an invalid diagram throws here instead of leaving Mermaid's error box behind.
  await m.parse(code);
  const id = `kairos-diagram-${++count}`;
  try {
    const { svg } = await m.render(id, code);
    drawn.set(key, svg);
    return svg;
  } finally {
    document.getElementById(`d${id}`)?.remove();
  }
}
