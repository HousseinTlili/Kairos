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
  usageLine,
  groupName,
  isOffered,
  modelGroups,
  openAtFirst,
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

describe("the line under an answer", () => {
  it("reads as plain words", () => {
    expect(
      usageLine({
        input: 13_000,
        output: 190,
        cost: 0,
        provider: "nous",
        model: "upstage/solar-mini4:free",
      }),
    ).toBe("13k tokens in, 190 out, free, upstage/solar-mini4:free");
    expect(usageLine({ input: 900, output: 40, cost: null, provider: null, model: null })).toBe(
      "900 tokens in, 40 out",
    );
  });
});

describe("the model menu's groups", () => {
  const catalog: Catalog = {
    current_provider: "nous",
    current_model: "solar",
    providers: [
      {
        slug: "nous",
        name: "Nous Portal",
        is_current: true,
        models: [
          { id: "upstage/solar-mini4:free", price: null },
          { id: "anthropic/claude-sonnet-5-5", price: null },
        ],
      },
      {
        slug: "gemini",
        name: "gemini",
        is_current: false,
        models: [{ id: "gemini-3-pro", price: null }],
      },
      {
        slug: "copilot-acp",
        name: "GitHub Copilot ACP",
        is_current: false,
        models: [{ id: "gpt-5.4", price: null }],
      },
    ],
  };

  it("makes one group per source, named for where the key came from", () => {
    expect(modelGroups(catalog, "").map((g) => g.name)).toEqual([
      "Nous Portal (Hermes's own)",
      "Google AI Studio (Gemini)",
      "GitHub Copilot (CLI)",
    ]);
  });

  it("a search keeps the matching models, and a whole source when its name matches", () => {
    const sonnet = modelGroups(catalog, "sonnet");
    expect(sonnet.map((g) => g.slug)).toEqual(["nous"]);
    expect(sonnet[0].models.map((m) => m.id)).toEqual(["anthropic/claude-sonnet-5-5"]);
    expect(sonnet[0].total).toBe(2);
    expect(modelGroups(catalog, "google")[0].models).toHaveLength(1);
    expect(modelGroups(catalog, "nothing like this")).toEqual([]);
  });

  it("opens the chosen model's group first, else the default's", () => {
    const groups = modelGroups(catalog, "");
    expect(openAtFirst(groups, { provider: "gemini", model: "gemini-3-pro" })).toEqual(["gemini"]);
    expect(openAtFirst(groups, { provider: null, model: null })).toEqual(["nous"]);
  });

  it("names the local endpoints added from the Providers page for this PC", () => {
    expect(groupName("ollama", "ollama")).toBe("Ollama (this PC)");
    expect(groupName("lm-studio", "lm-studio")).toBe("LM Studio (this PC)");
    expect(groupName("my-gateway", "my-gateway")).toBe("my-gateway");
  });
});
