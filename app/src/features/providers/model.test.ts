import { describe, expect, it } from "vitest";
import type { Catalog } from "../../lib/tauri";
import { addedWords, modelsListed, parseModels, slugOf } from "./model";

const catalog: Catalog = {
  current_provider: "openrouter",
  current_model: "x",
  providers: [
    {
      slug: "openrouter",
      name: "OpenRouter",
      is_current: true,
      models: [{ id: "a", price: null }],
    },
    { slug: "groq", name: "groq", is_current: false, models: [] },
  ],
};

describe("the Providers page", () => {
  it("reads model names given with commas or one per line", () => {
    expect(parseModels("gpt-4o, llama-3.3-70b\n\n  mistral-large ")).toEqual([
      "gpt-4o",
      "llama-3.3-70b",
      "mistral-large",
    ]);
    expect(parseModels("  ")).toEqual([]);
  });

  it("says whether Hermes lists a provider yet, and how many models", () => {
    expect(addedWords(modelsListed(catalog, "openrouter"))).toBe("1 model");
    expect(addedWords(modelsListed(catalog, "groq"))).toBe("Hermes lists it, with no models yet");
    expect(addedWords(modelsListed(catalog, "deepseek"))).toBe("Hermes doesn't list it yet");
  });

  it("shows an endpoint's name as Hermes will know it", () => {
    expect(slugOf("My Gateway!")).toBe("my-gateway");
    expect(slugOf("LM Studio (this PC)")).toBe("lm-studio-this-pc");
  });
});
