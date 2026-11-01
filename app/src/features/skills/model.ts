import type { InstalledSkill, ScanResult, SkillOrigin, SkillTrust } from "../../lib/tauri";

export const ORIGIN_LABEL: Record<SkillOrigin, string> = {
  builtin: "Built-in",
  hub: "Hub",
  yours: "Yours",
};

export const TRUST_LABEL: Record<SkillTrust, string> = {
  official: "Official",
  trusted: "Trusted",
  community: "Community",
};

/** Step 1 of a skill's page: who it comes from, and what that's worth. */
export const TRUST_WORDS: Record<SkillTrust, string> = {
  official: "Maintained by Nous Research, who make Hermes.",
  trusted: "From an organisation Hermes trusts. It still scans everything from them.",
  community: "Anyone can publish here. A skill is instructions Kairos will follow.",
};

/** `hermes skills search --source`, in the Master's words. */
export const SKILL_SOURCES: { id: string; label: string }[] = [
  { id: "all", label: "All registries" },
  { id: "official", label: "Official (Hermes)" },
  { id: "anthropic", label: "Anthropic" },
  { id: "openai", label: "OpenAI" },
  { id: "huggingface", label: "Hugging Face" },
  { id: "nvidia", label: "NVIDIA" },
  { id: "skills-sh", label: "skills.sh" },
  { id: "clawhub", label: "ClawHub" },
  { id: "lobehub", label: "LobeHub" },
  { id: "github", label: "GitHub" },
  { id: "well-known", label: "Well-known sites" },
  { id: "browse-sh", label: "browse.sh" },
  { id: "voltagent", label: "VoltAgent" },
  { id: "gstack", label: "gstack" },
  { id: "minimax", label: "MiniMax" },
];

export interface SkillGroup {
  category: string;
  skills: InstalledSkill[];
  off: number;
}

/** The installed skills by category, alphabetically, narrowed to those matching `filter`. */
export function groupSkills(skills: InstalledSkill[], filter: string): SkillGroup[] {
  const words = filter.trim().toLowerCase();
  const groups = new Map<string, InstalledSkill[]>();
  for (const skill of skills) {
    const haystack = `${skill.name} ${skill.description} ${skill.category}`.toLowerCase();
    if (words && !haystack.includes(words)) continue;
    const category = skill.category || "other";
    groups.set(category, [...(groups.get(category) ?? []), skill]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([category, list]) => ({
      category,
      skills: [...list].sort((a, b) => a.name.localeCompare(b.name)),
      off: list.filter((s) => !s.enabled).length,
    }));
}

/** A category's folder name, for reading: `software-development` → `Software development`. */
export function categoryName(category: string): string {
  const words = category.replace(/[-_]+/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

const TRUST_ORDER: SkillTrust[] = ["official", "trusted", "community"];

/** Official and trusted first; otherwise Hermes's order, which is by relevance. */
export function byTrust<T extends { trust: SkillTrust }>(found: T[]): T[] {
  return [...found].sort((a, b) => TRUST_ORDER.indexOf(a.trust) - TRUST_ORDER.indexOf(b.trust));
}

/** Install is offered only after a scan that passed, on that same skill. */
export function canInstall(scan: ScanResult | null): boolean {
  return scan !== null && scan.passed && !scan.already_installed;
}

export type ScanTone = "ok" | "warn" | "bad";

/** A scan in Kairos's own words. Hermes's sentences advise `--force`, so they never reach here. */
export function scanWords(scan: ScanResult, trust: SkillTrust): { tone: ScanTone; text: string } {
  if (scan.already_installed) {
    return { tone: "ok", text: "It's already installed." };
  }
  if (!scan.passed) {
    return {
      tone: "bad",
      text: `The scan refused this skill (${verdictName(scan.verdict)}). It can't be installed from Kairos, and there's no way around that.`,
    };
  }
  if (scan.verdict === "safe" && scan.findings.length === 0) {
    return { tone: "ok", text: "The scan found nothing to worry about." };
  }
  return {
    tone: "warn",
    text:
      trust === "official"
        ? `The scan rates this ${verdictName(scan.verdict)} for what's listed below. Hermes allows it anyway, because it ships with Hermes. Read what it found first.`
        : `The scan rates this ${verdictName(scan.verdict)} for what's listed below, and Hermes's policy for this source allows it. Read what it found first.`,
  };
}

export function verdictName(verdict: ScanResult["verdict"]): string {
  return verdict === "unknown" ? "no clear verdict" : verdict;
}
