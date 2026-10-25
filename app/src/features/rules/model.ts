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

/**
 * Rules added to the starter set after a user's file was written (M3 §6). A valid rules file is
 * never rewritten behind the user's back, so the Rules page offers to add these instead. Kept
 * identical to plugin/kairos/policy.default.json (a test compares them).
 */
export const NEWER_STARTER_RULES: { after: string; rules: RuleDoc[] }[] = [
  {
    after: "close-apps",
    rules: [
      {
        id: "open-installers",
        description: "Installers and uninstallers change what is on the PC.",
        tools: ["open_app"],
        match: { any_text: ["*install*", "*setup*"] },
        action: "ask",
      },
      {
        id: "open-apps",
        description: "Opening an app from the Start menu is allowed.",
        tools: ["open_app"],
        action: "allow",
      },
    ],
  },
  {
    after: "permission-changes",
    rules: [
      {
        id: "kill-system-processes",
        description:
          "Ending a Windows system process can crash or log out the computer. Kairos never does it.",
        tools: ["terminal", "execute_code", "browser_exec"],
        match: {
          commands: [
            "\\b(taskkill(\\.exe)?|stop-process|spps|kill|pkill|killall|tskill|wmic(\\.exe)?)\\b[^;&|\\n]*\\b(explorer|winlogon|csrss|lsass|services|svchost|smss|wininit|dwm)(\\.exe)?\\b",
            "\\bget-process\\b[^;&\\n]*\\b(explorer|winlogon|csrss|lsass|services|svchost|smss|wininit|dwm)\\b[^;&\\n]*\\|\\s*(stop-process|spps|kill)\\b",
          ],
        },
        action: "deny",
      },
      {
        id: "kill-processes",
        description:
          "Ending a program from the command line can lose unsaved work. Use the close_app tool instead: it asks the user and lets the app save first.",
        tools: ["terminal", "execute_code", "browser_exec"],
        match: {
          commands: [
            "\\btaskkill(\\.exe)?\\b",
            "\\btskill\\b",
            "\\b(stop-process|spps)\\b",
            "\\|\\s*kill\\b",
            "(^|[;&|]\\s*)kill\\s+(-\\S+\\s+)*(%?\\d+|\\$\\S+)",
            "\\b(pkill|killall)\\b",
            "\\bwmic(\\.exe)?\\b[^;&|\\n]*\\bprocess\\b[^;&|\\n]*\\b(delete|terminate)\\b",
            "\\.(Kill|terminate)\\s*\\(",
          ],
        },
        action: "ask",
      },
      {
        id: "run-installers",
        description: "This starts an installer, which can change the computer.",
        tools: ["terminal", "execute_code", "browser_exec"],
        match: {
          commands: [
            "\\b(start|start-process|saps|invoke-item|ii|cmd\\s+/c\\s+start)\\b[^;&|\\n]*\\S*(setup|install)\\S*\\.exe\\b",
            "(^|[;&|]\\s*)\\.?[\\\\/]?\\S*(setup|install)\\S*\\.exe\\b",
            "\\S\\.msi\\b",
          ],
        },
        action: "ask",
      },
    ],
  },
  {
    after: "kairos-plugin-source",
    rules: [
      {
        id: "kairos-skills",
        description: "These are Kairos's own skills: changing them changes how Kairos works.",
        tools: ["write_file", "patch", "skill_manage"],
        match: {
          any_text: [
            "**/hermes/skills/kairos/**",
            "**/kairos/skills/kairos/**",
            "kairos-debug-and-fix",
            "kairos-code-review",
            "kairos-documents-to-actions",
          ],
        },
        action: "ask",
      },
    ],
  },
  {
    after: "no-credential-store",
    rules: [
      {
        id: "no-account-tokens",
        description:
          "Account sign-in tokens stay with Hermes and the GitHub CLI; Kairos never reads them (D-010).",
        tools: ["read_file", "write_file", "patch", "search_files", "terminal", "execute_code"],
        match: {
          any_text: [
            "**/hermes/google_token.json",
            "**/hermes/google_oauth_pending.json",
            "**/hermes/google_client_secret*.json",
            "**/gh/hosts.yml",
          ],
        },
        action: "deny",
      },
    ],
  },
  {
    after: "outbound-messages",
    rules: [
      {
        id: "account-writes",
        description:
          "Sending, replying, deleting or changing anything in an account always asks (D-010).",
        tools: ["terminal", "execute_code", "browser_exec"],
        match: {
          commands: [
            "\\bgmail\\s+(send|reply|forward|modify|trash|delete)\\b",
            "\\bcalendar\\s+(create|update|delete)\\b",
            "\\bdrive\\s+(upload|share|delete|create-folder)\\b",
            "\\bsheets\\s+(update|append|create)\\b",
            "\\bdocs\\s+(create|append)\\b",
            "\\bgws\\b[^;&|\\n]*\\b(send|insert|import|modify|batchModify|trash|delete|batchDelete|update|patch|create|copy|move)\\b",
            "\\bhimalaya\\b[^;&|\\n]*\\b(send|reply|forward|write|delete|move|copy)\\b",
          ],
        },
        action: "ask",
      },
      {
        id: "account-disconnect",
        description:
          "Disconnecting or re-authorising an account is the user's call; Settings > Accounts does it.",
        tools: ["terminal", "execute_code"],
        match: {
          commands: [
            "\\bsetup\\.py\\b[^;&|\\n]*--revoke\\b",
            "\\$\\{?GSETUP\\}?[^;&|\\n]*--revoke\\b",
            "\\bgh\\s+auth\\s+(logout|refresh|switch)\\b",
          ],
        },
        action: "ask",
      },
    ],
  },
  {
    after: "no-safety-changes",
    rules: [
      {
        id: "no-sandbox-changes",
        description:
          "The agent must not move its own commands out of the container (D-019); that is the user's setting.",
        tools: ["terminal", "execute_code", "browser_exec"],
        match: {
          commands: [
            "\\bhermes(\\.exe)?\\s+config\\s+(set|unset)\\s+terminal\\.",
            "\\bhermes(\\.exe)?\\s+config\\s+edit\\b",
          ],
        },
        action: "deny",
      },
    ],
  },
];

/** The newer starter rules this document doesn't have yet (matched by id). */
export function missingNewerRules(doc: PolicyDoc): RuleDoc[] {
  const ids = new Set(doc.rules.map((r) => r.id));
  return NEWER_STARTER_RULES.flatMap((group) => group.rules).filter((r) => !ids.has(r.id));
}

/**
 * Adds the missing newer rules, each group straight after the rule it follows in the starter set
 * (or at the end when the user removed that rule), keeping their order.
 */
export function addNewerRules(doc: PolicyDoc): PolicyDoc {
  let rules = [...doc.rules];
  for (const group of NEWER_STARTER_RULES) {
    const ids = new Set(rules.map((r) => r.id));
    const missing = group.rules.filter((r) => !ids.has(r.id));
    if (missing.length === 0) continue;
    const anchor = rules.findIndex((r) => r.id === group.after);
    const at = anchor === -1 ? rules.length : anchor + 1;
    rules = [...rules.slice(0, at), ...missing.map((r) => structuredClone(r)), ...rules.slice(at)];
  }
  return withRules(doc, rules);
}
