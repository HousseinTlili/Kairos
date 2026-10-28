// A small Markdown parser for assistant answers. It produces plain data that MarkdownView renders
// as React elements, so text from the model is never inserted as HTML.

export type Inline =
  | { type: "text"; text: string }
  | { type: "code"; text: string }
  | { type: "strong"; children: Inline[] }
  | { type: "em"; children: Inline[] }
  | { type: "link"; href: string; children: Inline[] }
  | { type: "image"; src: string; alt: string };

export type Align = "left" | "center" | "right" | null;

export type Block =
  | { type: "paragraph"; children: Inline[] }
  | { type: "heading"; level: 1 | 2 | 3; children: Inline[] }
  /** `closed` is false while the answer is still streaming the block (M13: draw diagrams once). */
  | { type: "code"; lang: string; text: string; closed: boolean }
  | { type: "list"; ordered: boolean; items: Inline[][] }
  | { type: "quote"; children: Inline[] }
  | { type: "rule" }
  | { type: "table"; align: Align[]; header: Inline[][]; rows: Inline[][][] }
  /** Hermes's `MEDIA:<path>` attachment line (M13); the picture is read through Kairos. */
  | { type: "media"; path: string };

// Only web links are made clickable; anything else (javascript:, file:) stays as text.
const SAFE_LINK = /^https?:\/\//i;
// The only pictures shown (D-028): the raster images Hermes inlines. No SVG data URLs (scripts) and
// no web addresses (the model could make Kairos fetch a tracking URL); those become links or text.
const SAFE_IMAGE = /^data:image\/(png|jpeg|gif|webp);base64,[A-Za-z0-9+/=\s]+$/i;
const MEDIA_LINE = /^\s*MEDIA:\s*(\S.*?)\s*$/;
const ALIGN_ROW = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  let rest = text;
  const pushText = (value: string) => {
    if (!value) return;
    const last = out[out.length - 1];
    if (last?.type === "text") last.text += value;
    else out.push({ type: "text", text: value });
  };

  while (rest.length > 0) {
    const code = /^`([^`]+)`/.exec(rest);
    if (code) {
      out.push({ type: "code", text: code[1] });
      rest = rest.slice(code[0].length);
      continue;
    }
    const strong = /^\*\*(.+?)\*\*/.exec(rest) ?? /^__(.+?)__/.exec(rest);
    if (strong) {
      out.push({ type: "strong", children: parseInline(strong[1]) });
      rest = rest.slice(strong[0].length);
      continue;
    }
    const em = /^\*(?!\s)(.+?)\*/.exec(rest) ?? /^_(?!\s)(.+?)_(?![A-Za-z0-9])/.exec(rest);
    if (em) {
      out.push({ type: "em", children: parseInline(em[1]) });
      rest = rest.slice(em[0].length);
      continue;
    }
    const image = /^!\[([^\]]*)\]\(([^)\s]+)\)/.exec(rest);
    if (image) {
      const [, alt, src] = image;
      if (SAFE_IMAGE.test(src)) out.push({ type: "image", src, alt });
      else if (SAFE_LINK.test(src)) {
        out.push({ type: "link", href: src, children: [{ type: "text", text: alt || src }] });
      } else pushText(alt ? `[${alt}]` : "[image]");
      rest = rest.slice(image[0].length);
      continue;
    }
    const link = /^\[([^\]]+)\]\(([^)\s]+)\)/.exec(rest);
    if (link) {
      if (SAFE_LINK.test(link[2])) {
        out.push({ type: "link", href: link[2], children: parseInline(link[1]) });
      } else {
        pushText(link[0]);
      }
      rest = rest.slice(link[0].length);
      continue;
    }
    // Plain text up to the next character that could start markup.
    const plain = /^[^`*_[!]+/.exec(rest);
    const take = plain ? plain[0] : rest[0];
    pushText(take);
    rest = rest.slice(take.length);
  }
  return out;
}

/** A cell's words, for copying a table as tab-separated text. */
export function plainText(nodes: Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
        case "code":
          return node.text;
        case "image":
          return node.alt;
        default:
          return plainText(node.children);
      }
    })
    .join("");
}

/** A table as tab-separated lines, which paste into Excel as cells (M13). */
export function tableText(header: Inline[][], rows: Inline[][][]): string {
  return [header, ...rows]
    .map((row) => row.map((cell) => plainText(cell).replace(/[\t\n]/g, " ")).join("\t"))
    .join("\n");
}

/** A table row's cells: split on `|`, except inside `code` or after a backslash. */
export function splitRow(line: string): string[] {
  const cells: string[] = [];
  let cell = "";
  let inCode = false;
  const text = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === "\\" && text[i + 1] === "|") {
      cell += "|";
      i++;
    } else if (ch === "`") {
      inCode = !inCode;
      cell += ch;
    } else if (ch === "|" && !inCode) {
      cells.push(cell.trim());
      cell = "";
    } else cell += ch;
  }
  cells.push(cell.trim());
  return cells;
}

function alignOf(cell: string): Align {
  const left = cell.startsWith(":");
  const right = cell.endsWith(":");
  return left && right ? "center" : right ? "right" : left ? "left" : null;
}

/** A table starts at a row with a pipe that is followed by its `|---|:---:|` alignment row. */
function isTableStart(lines: string[], i: number): boolean {
  return lines[i].includes("|") && i + 1 < lines.length && ALIGN_ROW.test(lines[i + 1]);
}

export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;

  const isBlockStart = (line: string) =>
    /^```/.test(line) ||
    /^#{1,3}\s/.test(line) ||
    /^\s*([-*+]|\d+[.)])\s+/.test(line) ||
    /^>\s?/.test(line) ||
    /^(-{3,}|\*{3,})\s*$/.test(line);

  while (i < lines.length) {
    const line = lines[i];

    if (line.trim() === "") {
      i++;
      continue;
    }

    const fence = /^```\s*([\w+-]*)/.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) body.push(lines[i++]);
      const closed = i < lines.length;
      i++; // closing fence (or end of text while still streaming)
      blocks.push({ type: "code", lang: fence[1].toLowerCase(), text: body.join("\n"), closed });
      continue;
    }

    const media = MEDIA_LINE.exec(line);
    if (media) {
      blocks.push({ type: "media", path: media[1].replace(/^["'`]+|["'`.,]+$/g, "") });
      i++;
      continue;
    }

    if (isTableStart(lines, i)) {
      const header = splitRow(line);
      const align = splitRow(lines[i + 1]).map(alignOf);
      const rows: Inline[][][] = [];
      i += 2;
      while (i < lines.length && lines[i].includes("|") && lines[i].trim() !== "") {
        const cells = splitRow(lines[i]);
        // Ragged rows are padded or cut to the header's width.
        rows.push(header.map((_, c) => parseInline(cells[c] ?? "")));
        i++;
      }
      blocks.push({
        type: "table",
        align: header.map((_, c) => align[c] ?? null),
        header: header.map((cell) => parseInline(cell)),
        rows,
      });
      continue;
    }

    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      blocks.push({
        type: "heading",
        level: heading[1].length as 1 | 2 | 3,
        children: parseInline(heading[2].trim()),
      });
      i++;
      continue;
    }

    if (/^(-{3,}|\*{3,})\s*$/.test(line)) {
      blocks.push({ type: "rule" });
      i++;
      continue;
    }

    const listItem = /^\s*([-*+]|\d+[.)])\s+(.*)$/;
    const first = listItem.exec(line);
    if (first) {
      const ordered = /\d/.test(first[1]);
      const items: Inline[][] = [];
      while (i < lines.length) {
        const match = listItem.exec(lines[i]);
        if (!match || /\d/.test(match[1]) !== ordered) break;
        let text = match[2];
        i++;
        // Indented continuation lines belong to the same item.
        while (i < lines.length && /^\s{2,}\S/.test(lines[i]) && !listItem.test(lines[i])) {
          text += " " + lines[i].trim();
          i++;
        }
        items.push(parseInline(text));
      }
      blocks.push({ type: "list", ordered, items });
      continue;
    }

    if (/^>\s?/.test(line)) {
      const quoted: string[] = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) {
        quoted.push(lines[i].replace(/^>\s?/, ""));
        i++;
      }
      blocks.push({ type: "quote", children: parseInline(quoted.join(" ")) });
      continue;
    }

    const paragraph: string[] = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !isBlockStart(lines[i]) &&
      !MEDIA_LINE.test(lines[i]) &&
      !isTableStart(lines, i)
    ) {
      paragraph.push(lines[i]);
      i++;
    }
    blocks.push({ type: "paragraph", children: parseInline(paragraph.join("\n")) });
  }
  return blocks;
}
