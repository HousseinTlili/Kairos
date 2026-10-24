// The Kairos rules as the Rules page edits them (M2 spec §5). Pure, so it can be tested; the
// plugin's validator (through Rust) has the final word on what is valid.

export const MATCH_KINDS = ["paths", "commands", "urls", "any_text"] as const;
export type MatchKind = (typeof MATCH_KINDS)[number];
export type RuleAction = "allow" | "deny" | "ask";

export interface RuleDoc {
  id: string;
  description?: string;
  tools: string[];
  match?: Partial<Record<MatchKind, string[]>>;
  action: RuleAction;
  enabled?: boolean;
  // Fields this page doesn't know are kept as they are.
  [other: string]: unknown;
}

export interface PolicyDoc {
  version: number;
  default?: string;
  rules: RuleDoc[];
  [other: string]: unknown;
}

export const MATCH_LABEL: Record<MatchKind, string> = {
  paths: "Paths (globs)",
  commands: "Commands (regular expressions)",
  urls: "Web addresses (globs)",
  any_text: "Any text in the call (globs)",
};

export const ACTION_LABEL: Record<RuleAction, string> = {
  deny: "Block",
  ask: "Ask me",
  allow: "Allow",
};

/** The rules from the file's text, or null when it isn't a rules document this page can show. */
export function parsePolicy(text: string): PolicyDoc | null {
  try {
    const value = JSON.parse(text) as unknown;
    if (typeof value !== "object" || value === null || !Array.isArray((value as PolicyDoc).rules)) {
      return null;
    }
    return value as PolicyDoc;
  } catch {
    return null;
  }
}

export function serializePolicy(doc: PolicyDoc): string {
  return JSON.stringify(doc, null, 2) + "\n";
}

export function isEnabled(rule: RuleDoc): boolean {
  return rule.enabled !== false;
}

function withRules(doc: PolicyDoc, rules: RuleDoc[]): PolicyDoc {
  return { ...doc, rules };
}

export function toggleRule(doc: PolicyDoc, index: number): PolicyDoc {
  return withRules(
    doc,
    doc.rules.map((rule, i) => {
      if (i !== index) return rule;
      if (isEnabled(rule)) return { ...rule, enabled: false };
      // Back on: drop the field rather than writing the default.
      const rest: RuleDoc = { ...rule };
      delete rest.enabled;
      return rest;
    }),
  );
}

/** Moves a rule up (-1) or down (+1). Order matters: the first matching rule wins. */
export function moveRule(doc: PolicyDoc, index: number, by: -1 | 1): PolicyDoc {
  const target = index + by;
  if (target < 0 || target >= doc.rules.length) return doc;
  const rules = [...doc.rules];
  [rules[index], rules[target]] = [rules[target], rules[index]];
  return withRules(doc, rules);
}

export function updateRule(doc: PolicyDoc, index: number, patch: Partial<RuleDoc>): PolicyDoc {
  return withRules(
    doc,
    doc.rules.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)),
  );
}

/** Replaces one match kind's patterns; an empty list removes the kind. */
export function setMatch(
  doc: PolicyDoc,
  index: number,
  kind: MatchKind,
  patterns: string[],
): PolicyDoc {
  const rule = doc.rules[index];
  const match = { ...(rule.match ?? {}) };
  const cleaned = patterns.map((p) => p.trim()).filter(Boolean);
  if (cleaned.length) match[kind] = cleaned;
  else delete match[kind];
  const next: RuleDoc = { ...rule, match };
  if (Object.keys(match).length === 0) delete next.match;
  // Replace the rule outright: merging would bring a deleted `match` back from the old copy.
  return withRules(
    doc,
    doc.rules.map((r, i) => (i === index ? next : r)),
  );
}

export function removeRule(doc: PolicyDoc, index: number): PolicyDoc {
  return withRules(
    doc,
    doc.rules.filter((_, i) => i !== index),
  );
}

/** A new rule at the top, asking by default: the safe direction for something half-written. */
export function addRule(doc: PolicyDoc): PolicyDoc {
  const taken = new Set(doc.rules.map((r) => r.id));
  let n = 1;
  while (taken.has(`my-rule-${n}`)) n++;
  const rule: RuleDoc = {
    id: `my-rule-${n}`,
    description: "Describe what this rule protects.",
    tools: ["terminal"],
    match: { commands: ["\\bexample-command\\b"] },
    action: "ask",
  };
  return withRules(doc, [rule, ...doc.rules]);
}

/** One line describing what a rule matches, for the collapsed list. */
export function summarizeRule(rule: RuleDoc): string {
  const tools = rule.tools.includes("*") ? "any tool" : rule.tools.join(", ");
  const parts = MATCH_KINDS.flatMap((kind) => {
    const patterns = rule.match?.[kind] ?? [];
    if (!patterns.length) return [];
    const shown = patterns.slice(0, 2).join(", ");
    const more = patterns.length > 2 ? ` and ${patterns.length - 2} more` : "";
    return [`${kind === "any_text" ? "text" : kind}: ${shown}${more}`];
  });
  return parts.length ? `${tools} · ${parts.join("; ")}` : `${tools} · every call`;
}
