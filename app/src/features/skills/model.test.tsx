import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { InstalledSkill, ScanResult } from "../../lib/tauri";
import { byTrust, canInstall, categoryName, groupSkills, scanWords } from "./model";
import { SkillText } from "./SkillText";

const skill = (name: string, category: string, extra: Partial<InstalledSkill> = {}) =>
  ({
    name,
    category,
    origin: "builtin",
    enabled: true,
    description: "",
    own: false,
    ...extra,
  }) as InstalledSkill;

const scan = (extra: Partial<ScanResult> = {}): ScanResult => ({
  verdict: "safe",
  findings: [],
  passed: true,
  already_installed: false,
  origin: "",
  ...extra,
});

describe("the installed list", () => {
  it("groups skills by category and counts those turned off", () => {
    const groups = groupSkills(
      [skill("b", "web"), skill("a", "web", { enabled: false }), skill("c", "creative")],
      "",
    );
    expect(groups.map((g) => g.category)).toEqual(["creative", "web"]);
    expect(groups[1].skills.map((s) => s.name)).toEqual(["a", "b"]);
    expect(groups[1].off).toBe(1);
  });

  it("filters on the name, the description and the category", () => {
    const skills = [
      skill("opencode", "agents", { description: "Delegate coding" }),
      skill("x", "web"),
    ];
    expect(groupSkills(skills, "DELEGATE")).toHaveLength(1);
    expect(groupSkills(skills, "web")[0].skills[0].name).toBe("x");
    expect(groupSkills(skills, "nothing")).toEqual([]);
  });

  it("reads a folder name as words", () => {
    expect(categoryName("software-development")).toBe("Software development");
  });
});

describe("what a search offers", () => {
  it("puts official and trusted first and keeps Hermes's order within each", () => {
    const found = [
      { trust: "community", name: "1" },
      { trust: "official", name: "2" },
      { trust: "community", name: "3" },
      { trust: "trusted", name: "4" },
    ] as const;
    expect(byTrust([...found]).map((f) => f.name)).toEqual(["2", "4", "1", "3"]);
  });
});

describe("a skill's scan", () => {
  it("offers Install only after a scan that passed", () => {
    expect(canInstall(null)).toBe(false);
    expect(canInstall(scan())).toBe(true);
  });

  it("a blocked scan gives no way to install, and no advice to force it", () => {
    const blocked = scan({ passed: false, verdict: "caution" });
    expect(canInstall(blocked)).toBe(false);
    const words = scanWords(blocked, "community");
    expect(words.tone).toBe("bad");
    expect(words.text).toContain("no way around");
    expect(words.text.toLowerCase()).not.toContain("force");
  });

  it("a skill already installed isn't offered again", () => {
    expect(canInstall(scan({ passed: false, already_installed: true }))).toBe(false);
  });

  it("says an official skill's alarming verdict plainly, and why Hermes allows it", () => {
    const words = scanWords(scan({ verdict: "dangerous" }), "official");
    expect(words.tone).toBe("warn");
    expect(words.text).toContain("dangerous");
    expect(words.text).toContain("ships with Hermes");
  });

  it("a clean scan says so", () => {
    expect(scanWords(scan(), "community").tone).toBe("ok");
  });
});

describe("a skill's text", () => {
  it("shows HTML and image links as plain text", () => {
    const hostile =
      '<img src="https://evil.example/x.png" onerror="alert(1)"><script>x()</script>\n' +
      "![a picture](https://evil.example/y.png) and [a link](https://evil.example/z)";
    const html = renderToStaticMarkup(<SkillText text={hostile} />);
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<a ");
    expect(html).toContain("&lt;img");
    expect(html).toContain("![a picture](https://evil.example/y.png)");
  });
});
