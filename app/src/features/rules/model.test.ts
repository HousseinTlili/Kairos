import { describe, expect, it } from "vitest";
import starterText from "../../../../plugin/kairos/policy.default.json?raw";
import {
  addRule,
  isEnabled,
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
    expect(doc.rules).toHaveLength(17);
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
