import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown, splitRow } from "./markdown";

describe("parseMarkdown", () => {
  it("splits paragraphs on blank lines and keeps soft line breaks", () => {
    expect(parseMarkdown("one\ntwo\n\nthree")).toEqual([
      { type: "paragraph", children: [{ type: "text", text: "one\ntwo" }] },
      { type: "paragraph", children: [{ type: "text", text: "three" }] },
    ]);
  });

  it("reads a fenced code block verbatim, including markup characters", () => {
    expect(parseMarkdown("```powershell\nGet-ChildItem *_*.txt\n```")).toEqual([
      { type: "code", lang: "powershell", text: "Get-ChildItem *_*.txt", closed: true },
    ]);
  });

  it("shows an unclosed code fence while an answer is still streaming", () => {
    expect(parseMarkdown("```\nline one")).toEqual([
      { type: "code", lang: "", text: "line one", closed: false },
    ]);
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

describe("tables, attachments and diagrams (M13)", () => {
  it("reads a table with its alignment and inline formatting", () => {
    const [table] = parseMarkdown(
      "| Laptop | Price | Battery |\n|:--|--:|:-:|\n| **A** | 900 € | 10 h |\n| B | 1 200 € | 8 h |",
    );
    expect(table).toMatchObject({ type: "table", align: ["left", "right", "center"] });
    if (table.type !== "table") throw new Error("not a table");
    expect(table.header).toHaveLength(3);
    expect(table.rows[0][0]).toEqual([{ type: "strong", children: [{ type: "text", text: "A" }] }]);
    expect(table.rows[1][1]).toEqual([{ type: "text", text: "1 200 €" }]);
  });

  it("doesn't split a cell on a pipe inside code or after a backslash", () => {
    expect(splitRow("| `a | b` | c \\| d |")).toEqual(["`a | b`", "c | d"]);
  });

  it("pads a short row and cuts a long one to the header's width", () => {
    const [table] = parseMarkdown("| a | b |\n|---|---|\n| 1 |\n| 1 | 2 | 3 |");
    if (table.type !== "table") throw new Error("not a table");
    expect(table.rows.map((r) => r.length)).toEqual([2, 2]);
    expect(table.rows[0][1]).toEqual([]);
  });

  it("keeps pipes without an alignment row as a paragraph, as while a table still streams", () => {
    expect(parseMarkdown("| a | b |")[0].type).toBe("paragraph");
  });

  it("starts a table right after a sentence", () => {
    expect(
      parseMarkdown("Here they are:\n| a | b |\n|---|---|\n| 1 | 2 |").map((b) => b.type),
    ).toEqual(["paragraph", "table"]);
  });

  it("reads Hermes's MEDIA: attachment line on its own", () => {
    expect(parseMarkdown("Here it is.\nMEDIA: C:\\Users\\USER\\shot.png")).toEqual([
      { type: "paragraph", children: [{ type: "text", text: "Here it is." }] },
      { type: "media", path: "C:\\Users\\USER\\shot.png" },
    ]);
  });

  it("marks a diagram closed only once its fence has ended", () => {
    expect(parseMarkdown("```mermaid\nflowchart LR\nA-->B")[0]).toMatchObject({
      lang: "mermaid",
      closed: false,
    });
    expect(parseMarkdown("```Mermaid\nflowchart LR\nA-->B\n```")[0]).toMatchObject({
      lang: "mermaid",
      closed: true,
    });
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

  it("shows only the raster images Hermes inlines (M13, D-028)", () => {
    expect(parseInline("![apple](data:image/png;base64,iVBORw0KGgo=)")).toEqual([
      { type: "image", src: "data:image/png;base64,iVBORw0KGgo=", alt: "apple" },
    ]);
  });

  it("turns a web image into a link instead of fetching it", () => {
    expect(parseInline("![chart](https://tracker.example/p.png)")).toEqual([
      {
        type: "link",
        href: "https://tracker.example/p.png",
        children: [{ type: "text", text: "chart" }],
      },
    ]);
  });

  it("never shows an SVG data image or a script address as a picture", () => {
    expect(parseInline("![x](data:image/svg+xml;base64,PHN2Zz4=)")).toEqual([
      { type: "text", text: "[x]" },
    ]);
    expect(parseInline("![](file:///C:/Users/USER/secret.png)")).toEqual([
      { type: "text", text: "[image]" },
    ]);
  });

  it("keeps an exclamation mark that starts no image", () => {
    expect(parseInline("Done! [x] stays")).toEqual([{ type: "text", text: "Done! [x] stays" }]);
  });

  it("leaves snake_case words and lone asterisks alone", () => {
    expect(parseInline("API_SERVER_KEY and 2 * 3")).toEqual([
      { type: "text", text: "API_SERVER_KEY and 2 * 3" },
    ]);
  });
});
