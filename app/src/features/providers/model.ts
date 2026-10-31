// Your own AI providers (M18): small pure helpers for the Providers page.

import type { Catalog } from "../../lib/tauri";

/** "gpt-4o, llama-3.3-70b" or one per line → the list Hermes is given. */
export function parseModels(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((m) => m.trim())
    .filter((m) => m.length > 0);
}

/** How many models Hermes lists for a provider, or null when it doesn't list the provider yet. */
export function modelsListed(catalog: Catalog | null, id: string): number | null {
  const found = catalog?.providers.find((p) => p.slug === id);
  return found ? found.models.length : null;
}

/** An endpoint's name as Hermes's provider id (the core's `slug`, for showing it). */
export function slugOf(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 32);
}

/** What the page says about a provider just added. */
export function addedWords(listed: number | null): string {
  if (listed === null) return "Hermes doesn't list it yet";
  if (listed === 0) return "Hermes lists it, with no models yet";
  return `${listed} model${listed === 1 ? "" : "s"}`;
}
