import { describe, expect, it } from "vitest";
import starterText from "../../../../plugin/kairos/policy.default.json?raw";
import {
  NEWER_STARTER_RULES,
  addNewerRules,
  addRule,
  isEnabled,
  missingNewerRules,
  moveRule,
  parsePolicy,
  removeRule,
  serializePolicy,
  setMatch,
  summarizeRule,
  toggleRule,
  type PolicyDoc,
} from "./model";

const starter = (): PolicyDoc => {
  const doc = parsePolicy(starterText);
  if (!doc) throw new Error("starter rules don't parse");
  return doc;
};

describe("the rules model", () => {
  it("reads the plugin's starter rules in order", () => {
    const doc = starter();
    expect(doc.rules[0].id).toBe("no-credentials");
    expect(doc.rules).toHaveLength(27);
  });

  it("saves the starter rules unchanged when nothing was edited", () => {
    expect(parsePolicy(serializePolicy(starter()))).toEqual(starter());
  });

  it("turns a rule off and back on without leaving an enabled field behind", () => {
    const off = toggleRule(starter(), 0);
    expect(isEnabled(off.rules[0])).toBe(false);
    const on = toggleRule(off, 0);
    expect(isEnabled(on.rules[0])).toBe(true);
    expect("enabled" in on.rules[0]).toBe(false);
  });

  it("moves rules, and ignores moves past either end", () => {
    const doc = starter();
    const moved = moveRule(doc, 1, -1);
    expect(moved.rules.slice(0, 2).map((r) => r.id)).toEqual([
      "no-credential-store",
      "no-credentials",
    ]);
    expect(moveRule(doc, 0, -1)).toBe(doc);
    expect(moveRule(doc, doc.rules.length - 1, 1)).toBe(doc);
  });

  it("sets match patterns, trimming blanks, and drops an emptied kind", () => {
    const doc = setMatch(starter(), 0, "paths", ["  **/secrets/**  ", ""]);
    expect(doc.rules[0].match?.paths).toEqual(["**/secrets/**"]);
    const cleared = setMatch(doc, 0, "paths", []);
    expect(cleared.rules[0].match?.paths).toBeUndefined();
    expect(cleared.rules[0].match?.any_text).toBeDefined();
  });

  it("removing the last pattern removes the match block, so the rule covers every call", () => {
    const one = parsePolicy(
      JSON.stringify({
        version: 1,
        rules: [{ id: "a", tools: ["terminal"], action: "ask", match: { commands: ["x"] } }],
      }),
    )!;
    expect(setMatch(one, 0, "commands", []).rules[0].match).toBeUndefined();
  });

  it("adds a new rule at the top with a free id, asking by default", () => {
    const once = addRule(starter());
    const twice = addRule(once);
    expect(twice.rules.slice(0, 2).map((r) => r.id)).toEqual(["my-rule-2", "my-rule-1"]);
    expect(once.rules[0].action).toBe("ask");
    expect(removeRule(once, 0)).toEqual(starter());
  });

  it("keeps fields it doesn't know", () => {
    const doc = parsePolicy(
      JSON.stringify({
        version: 1,
        note: "keep me",
        rules: [{ id: "a", tools: ["*"], action: "deny", future: 1 }],
      }),
    )!;
    const text = serializePolicy(toggleRule(doc, 0));
    expect(JSON.parse(text)).toMatchObject({
      note: "keep me",
      rules: [{ future: 1, enabled: false }],
    });
  });

  it("refuses text that isn't a rules document", () => {
    expect(parsePolicy("{ nope")).toBeNull();
    expect(parsePolicy("[]")).toBeNull();
    expect(parsePolicy('{"version": 1}')).toBeNull();
  });

  it("summarises a rule in one line", () => {
    const [noCredentials] = starter().rules;
    expect(summarizeRule(noCredentials)).toMatch(
      /^read_file, .* · text: \*\*\/\.ssh\/\*\*, \*\*\/hermes\/\.env and 10 more$/,
    );
    const closeApps = starter().rules.find((r) => r.id === "close-apps")!;
    expect(summarizeRule(closeApps)).toBe("close_app · every call");
  });
});

describe("rules added to the starter set later (M3)", () => {
  const m2File = (): PolicyDoc => {
    const doc = starter();
    return {
      ...doc,
      rules: doc.rules.filter((r) => !["open-installers", "open-apps"].includes(r.id)),
    };
  };

  it("are the same as in the plugin's starter rules", () => {
    const byId = new Map(starter().rules.map((r) => [r.id, r]));
    for (const group of NEWER_STARTER_RULES) {
      for (const rule of group.rules) expect(byId.get(rule.id)).toEqual(rule);
    }
  });

  it("are offered to a file written before them, and not to the current starter rules", () => {
    expect(missingNewerRules(m2File()).map((r) => r.id)).toEqual(["open-installers", "open-apps"]);
    expect(missingNewerRules(starter())).toEqual([]);
  });

  it("bring a file written at M2 sign-off up to the current starter set", () => {
    // The product owner's policy.json: the 18 rules from M2, before either M3 group.
    const newer = NEWER_STARTER_RULES.flatMap((g) => g.rules.map((r) => r.id));
    const doc = starter();
    const m2SignOff: PolicyDoc = { ...doc, rules: doc.rules.filter((r) => !newer.includes(r.id)) };
    expect(m2SignOff.rules).toHaveLength(18);
    expect(missingNewerRules(m2SignOff).map((r) => r.id)).toEqual([
      "open-installers",
      "open-apps",
      "kill-system-processes",
      "kill-processes",
      "run-installers",
      "kairos-skills",
      "no-account-tokens",
      "account-writes",
      "account-disconnect",
    ]);
    expect(addNewerRules(m2SignOff)).toEqual(starter());
  });

  it("land where the starter set has them, so an older file ends up the same", () => {
    expect(addNewerRules(m2File())).toEqual(starter());
  });

  it("go at the end when the rule they follow was removed, and only the missing ones", () => {
    const doc = m2File();
    const withoutAnchor: PolicyDoc = {
      ...doc,
      rules: [...doc.rules.filter((r) => r.id !== "close-apps"), NEWER_STARTER_RULES[0].rules[1]],
    };
    const ids = addNewerRules(withoutAnchor).rules.map((r) => r.id);
    expect(ids.slice(-2)).toEqual(["open-apps", "open-installers"]);
    expect(ids.filter((id) => id === "open-apps")).toHaveLength(1);
  });
});
