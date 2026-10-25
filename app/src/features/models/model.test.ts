import { describe, expect, it } from "vitest";
import { sandboxText } from "./model";

describe("the sandbox line", () => {
  it("is reassuring only for the container", () => {
    expect(sandboxText("docker").tone).toBe("ok");
    expect(sandboxText("local").tone).toBe("warn");
    expect(sandboxText("local").text).toMatch(/nothing else contains it/);
    expect(sandboxText(null).tone).toBe("muted");
    expect(sandboxText("ssh").text).toContain("ssh");
  });
});

import type { Catalog } from "../../lib/tauri";
import {
  DEFAULT_CHOICE,
  choiceKey,
  costLabel,
  isOffered,
  parseChoiceKey,
  priceLabel,
  tokens,
} from "./model";

describe("the model picker", () => {
  it("round-trips a choice through its option value; empty is Hermes's default", () => {
    const choice = { provider: "openrouter", model: "example/large" };
    expect(parseChoiceKey(choiceKey(choice))).toEqual(choice);
    expect(choiceKey(DEFAULT_CHOICE)).toBe("");
    expect(parseChoiceKey("")).toEqual(DEFAULT_CHOICE);
  });

  it("labels prices per million tokens, free models as free", () => {
    expect(priceLabel({ input: 3, output: 15, cache: 0.3 })).toBe("$3 / $15 per M tokens");
    expect(priceLabel({ input: 0.15, output: 0.6, cache: null })).toBe(
      "$0.15 / $0.60 per M tokens",
    );
    expect(priceLabel({ input: 0, output: 0, cache: null })).toBe("free");
    expect(priceLabel(null)).toBe("");
  });

  it("notices a model that is no longer offered", () => {
    const catalog: Catalog = {
      current_provider: "openrouter",
      current_model: "a",
      providers: [
        {
          slug: "openrouter",
          name: "OpenRouter",
          is_current: true,
          models: [{ id: "a", price: null }],
        },
      ],
    };
    expect(isOffered(catalog, { provider: "openrouter", model: "a" })).toBe(true);
    expect(isOffered(catalog, { provider: "openrouter", model: "gone" })).toBe(false);
    expect(isOffered(catalog, DEFAULT_CHOICE)).toBe(true);
  });
});

describe("cost and tokens under an answer", () => {
  it("is always an estimate, free, or unknown", () => {
    expect(costLabel(0.0021, "openrouter")).toBe("≈ $0.0021");
    expect(costLabel(1.234, "openrouter")).toBe("≈ $1.23");
    expect(costLabel(0, "llamacpp")).toBe("free");
    expect(costLabel(null, "llamacpp")).toBe("price unknown");
    expect(costLabel(null, null)).toBe("");
  });

  it("shortens token counts", () => {
    expect(tokens(950)).toBe("950");
    expect(tokens(1234)).toBe("1.2k");
    expect(tokens(45_000)).toBe("45k");
    expect(tokens(null)).toBe("—");
  });
});
