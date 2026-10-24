// A small Markdown parser for assistant answers. It produces plain data that MarkdownView renders
// as React elements, so text from the model is never inserted as HTML.

export type Inline =
  | { type: "text"; text: string }
  | { type: "code"; text: string }
  | { type: "strong"; children: Inline[] }
  | { type: "em"; children: Inline[] }
  | { type: "link"; href: string; children: Inline[] };

export type Block =
  | { type: "paragraph"; children: Inline[] }
  | { type: "heading"; level: 1 | 2 | 3; children: Inline[] }
  | { type: "code"; lang: string; text: string }
  | { type: "list"; ordered: boolean; items: Inline[][] }
  | { type: "quote"; children: Inline[] }
  | { type: "rule" };

// Only web links are made clickable; anything else (javascript:, file:) stays as text.
const SAFE_LINK = /^https?:\/\//i;

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
    const plain = /^[^`*_[]+/.exec(rest);
    const take = plain ? plain[0] : rest[0];
    pushText(take);
    rest = rest.slice(take.length);
  }
  return out;
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
      i++; // closing fence (or end of text while still streaming)
      blocks.push({ type: "code", lang: fence[1], text: body.join("\n") });
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
    while (i < lines.length && lines[i].trim() !== "" && !isBlockStart(lines[i])) {
      paragraph.push(lines[i]);
      i++;
    }
    blocks.push({ type: "paragraph", children: parseInline(paragraph.join("\n")) });
  }
  return blocks;
}
