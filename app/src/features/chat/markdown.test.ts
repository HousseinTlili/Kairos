import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown } from "./markdown";

describe("parseMarkdown", () => {
  it("splits paragraphs on blank lines and keeps soft line breaks", () => {
    expect(parseMarkdown("one\ntwo\n\nthree")).toEqual([
      { type: "paragraph", children: [{ type: "text", text: "one\ntwo" }] },
      { type: "paragraph", children: [{ type: "text", text: "three" }] },
    ]);
  });

  it("reads a fenced code block verbatim, including markup characters", () => {
    expect(parseMarkdown("```powershell\nGet-ChildItem *_*.txt\n```")).toEqual([
      { type: "code", lang: "powershell", text: "Get-ChildItem *_*.txt" },
    ]);
  });

  it("shows an unclosed code fence while an answer is still streaming", () => {
    expect(parseMarkdown("```\nline one")).toEqual([{ type: "code", lang: "", text: "line one" }]);
  });

  it("reads headings, bullet and numbered lists", () => {
    const blocks = parseMarkdown("## Plan\n- first\n- second\n\n1. a\n2. b");
    expect(blocks.map((b) => b.type)).toEqual(["heading", "list", "list"]);
    expect(blocks[1]).toMatchObject({
      ordered: false,
      items: [[{ text: "first" }], [{ text: "second" }]],
    });
    expect(blocks[2]).toMatchObject({ ordered: true });
  });

  it("joins an indented continuation line into its list item", () => {
    const [list] = parseMarkdown("- a long\n  item\n- next");
    expect(list).toMatchObject({ items: [[{ text: "a long item" }], [{ text: "next" }]] });
  });
});

describe("parseInline", () => {
  it("reads code, bold, italic and links", () => {
    expect(parseInline("Run `ls` **now**, *please*: [docs](https://example.com)")).toEqual([
      { type: "text", text: "Run " },
      { type: "code", text: "ls" },
      { type: "text", text: " " },
      { type: "strong", children: [{ type: "text", text: "now" }] },
      { type: "text", text: ", " },
      { type: "em", children: [{ type: "text", text: "please" }] },
      { type: "text", text: ": " },
      { type: "link", href: "https://example.com", children: [{ type: "text", text: "docs" }] },
    ]);
  });

  it("never turns a non-web link into a link", () => {
    expect(parseInline("[x](javascript:alert(1))")).toEqual([
      { type: "text", text: "[x](javascript:alert(1))" },
    ]);
  });

  it("leaves snake_case words and lone asterisks alone", () => {
    expect(parseInline("API_SERVER_KEY and 2 * 3")).toEqual([
      { type: "text", text: "API_SERVER_KEY and 2 * 3" },
    ]);
  });
});
