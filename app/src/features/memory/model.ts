// The memory page's small rules (M11, D-026). Pure, for the tests.
import type { Fact } from "../../lib/tauri";

/** Forget requests run in their own conversation, which the sidebar leaves out. */
export const FORGET_SESSION = "kairos-memory";

export function isMemoryConversation(sessionId: string): boolean {
  return sessionId === FORGET_SESSION || sessionId.startsWith(`${FORGET_SESSION}-`);
}

export type Forgettable =
  { kind: "user"; text: string } | { kind: "notes"; text: string } | { kind: "fact"; fact: Fact };

/**
 * What Kairos is asked to do: remove exactly one item with its own tool, nothing else. Hermes and
 * the provider stay in charge of their files (D-026).
 */
export function forgetRequest(item: Forgettable): string {
  const close = 'Do nothing else, then reply with the single word "Forgotten."';
  switch (item.kind) {
    case "user":
      return `Forget this about me. Use your memory tool to remove exactly this entry from your profile of me (target "user"), and nothing else: «${item.text}». ${close}`;
    case "notes":
      return `Forget this note. Use your memory tool to remove exactly this entry from your own notes (target "memory"), and nothing else: «${item.text}». ${close}`;
    case "fact":
      return `Forget this fact for good. Use fact_store with action "forget" and fact_id ${item.fact.id}: «${item.fact.content}». If the same thing is also in your memory notes or your profile of me, remove it there too. ${close}`;
  }
}

/** "312 of 1,375 characters (23 %)". */
export function fullness(used: number, limit: number): string {
  const percent = limit > 0 ? Math.round((used / limit) * 100) : 0;
  return `${used.toLocaleString("en")} of ${limit.toLocaleString("en")} characters (${percent} %)`;
}

const CATEGORY: Record<string, string> = {
  user_pref: "Preference",
  project: "Project",
  tool: "Tool",
  general: "General",
};

export function categoryLabel(category: string): string {
  return CATEGORY[category] ?? category;
}
