// Models, cost and sandbox wording (M7). Pure, for the tests.
import type { Catalog, ModelChoice, Price } from "../../lib/tauri";

/** What the terminal backend means for the user (D-019). */
export function sandboxText(backend: string | null): {
  tone: "ok" | "warn" | "muted";
  text: string;
} {
  switch (backend) {
    case "docker":
      return {
        tone: "ok",
        text: "In a container: each conversation gets a fresh Docker container that sees only your chosen project folders.",
      };
    case "local":
      return {
        tone: "warn",
        text: "Directly on this PC: the rules check every command, but nothing else contains it.",
      };
    case null:
      return { tone: "muted", text: "Kairos couldn't ask Hermes." };
    default:
      return { tone: "muted", text: `Hermes's "${backend}" backend.` };
  }
}

export const DEFAULT_CHOICE: ModelChoice = { provider: null, model: null };

/** The picker's option value: "" for Hermes's default, else "provider\u0000model". */
export function choiceKey(choice: ModelChoice | null): string {
  return choice?.provider && choice.model ? `${choice.provider}\u0000${choice.model}` : "";
}

export function parseChoiceKey(key: string): ModelChoice {
  const [provider, model] = key.split("\u0000");
  return provider && model ? { provider, model } : DEFAULT_CHOICE;
}

/** "free", "$3 / $15 per M tokens", or "" when unknown. */
export function priceLabel(price: Price | null): string {
  if (!price) return "";
  if (price.input === 0 && price.output === 0) return "free";
  const money = (v: number) => `$${v >= 1 ? v.toFixed(v % 1 === 0 ? 0 : 2) : v.toPrecision(2)}`;
  return `${money(price.input)} / ${money(price.output)} per M tokens`;
}

/** "≈ $0.0021", "free", or "price unknown" (D-020: always an estimate). */
export function costLabel(cost: number | null, served: string | null): string {
  if (cost == null) return served ? "price unknown" : "";
  if (cost === 0) return "free";
  return `≈ $${cost < 0.01 ? cost.toPrecision(2) : cost.toFixed(2)}`;
}

/**
 * The line under an answer: "13k tokens in, 190 out, free, upstage/solar-mini4:free". Plain
 * words joined with commas (docs/design/BRAND.md: no "A · B · C" strings).
 */
export function usageLine(usage: {
  input: number;
  output: number;
  cost: number | null;
  provider: string | null;
  model: string | null;
}): string {
  const parts = [`${tokens(usage.input)} tokens in`, `${tokens(usage.output)} out`];
  const cost = costLabel(usage.cost, usage.provider);
  if (cost) parts.push(cost);
  if (usage.model) parts.push(usage.model);
  return parts.join(", ");
}

/** "1.2k" for token counts. */
export function tokens(n: number | null): string {
  if (n == null) return "—";
  return n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}k` : String(n);
}

/** Whether the chosen model is still offered (a removed provider falls back to the default). */
export function isOffered(catalog: Catalog | null, choice: ModelChoice): boolean {
  if (!choice.provider || !choice.model) return true;
  return (
    catalog?.providers.some(
      (p) => p.slug === choice.provider && p.models.some((m) => m.id === choice.model),
    ) ?? false
  );
}

/** What a source is called in the model menu: its group's name. */
const GROUP_NAMES: Record<string, string> = {
  nous: "Nous Portal (Hermes's own)",
  gemini: "Google AI Studio (Gemini)",
  copilot: "GitHub Copilot",
  "copilot-acp": "GitHub Copilot (CLI)",
  "openai-api": "OpenAI",
  "openai-codex": "ChatGPT (Codex)",
  llamacpp: "On this PC",
  // The Providers page's ready-made local endpoints (M18), named as their slugs come out.
  ollama: "Ollama (this PC)",
  "lm-studio": "LM Studio (this PC)",
};

export function groupName(slug: string, name: string): string {
  return GROUP_NAMES[slug] ?? name;
}

export interface ModelGroup {
  slug: string;
  name: string;
  /** The models shown (all of them, or the ones matching the search). */
  models: { id: string; price: Price | null }[];
  /** How many the source has, whatever the search. */
  total: number;
  /** Hermes's own default comes from this source. */
  isDefault: boolean;
}

/**
 * The menu's groups, one per source, in Hermes's order; with a search, only the models whose name
 * holds every word of it (and sources whose name matches keep all their models), empty groups out.
 */
export function modelGroups(catalog: Catalog | null, query: string): ModelGroup[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const hit = (text: string) => words.every((w) => text.toLowerCase().includes(w));
  return (catalog?.providers ?? [])
    .map((p) => {
      const name = groupName(p.slug, p.name);
      const all = words.length === 0 || hit(`${name} ${p.slug}`);
      return {
        slug: p.slug,
        name,
        models: all ? p.models : p.models.filter((m) => hit(m.id)),
        total: p.models.length,
        isDefault: p.is_current,
      };
    })
    .filter((g) => g.models.length > 0 || (words.length === 0 && g.total === 0));
}

/** Which groups start open: the one the chosen model is in, else the one Hermes's default is in. */
export function openAtFirst(groups: ModelGroup[], choice: ModelChoice): string[] {
  const chosen = choice.provider ?? groups.find((g) => g.isDefault)?.slug;
  return chosen ? [chosen] : [];
}
