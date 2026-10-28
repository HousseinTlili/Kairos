import { useMemo, useState } from "react";
import { Diagram } from "./Diagram";
import { parseMarkdown, tableText, type Block, type Inline } from "./markdown";
import { MediaPicture, Picture, SvgPicture } from "./Pictures";

function InlineView({ nodes }: { nodes: Inline[] }) {
  return (
    <>
      {nodes.map((node, i) => {
        switch (node.type) {
          case "text":
            return <span key={i}>{node.text}</span>;
          case "code":
            return <code key={i}>{node.text}</code>;
          case "strong":
            return (
              <strong key={i}>
                <InlineView nodes={node.children} />
              </strong>
            );
          case "em":
            return (
              <em key={i}>
                <InlineView nodes={node.children} />
              </em>
            );
          case "link":
            return (
              <a key={i} href={node.href} target="_blank" rel="noreferrer noopener">
                <InlineView nodes={node.children} />
              </a>
            );
          case "image":
            return <Picture key={i} src={node.src} alt={node.alt} />;
        }
      })}
    </>
  );
}

function TableView({ block }: { block: Extract<Block, { type: "table" }> }) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    void navigator.clipboard.writeText(tableText(block.header, block.rows)).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  return (
    <figure className="md-table">
      <div className="md-table-scroll">
        <table>
          <thead>
            <tr>
              {block.header.map((cell, c) => (
                <th key={c} style={{ textAlign: block.align[c] ?? undefined }}>
                  <InlineView nodes={cell} />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {block.rows.map((row, r) => (
              <tr key={r}>
                {row.map((cell, c) => (
                  <td key={c} style={{ textAlign: block.align[c] ?? undefined }}>
                    <InlineView nodes={cell} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <figcaption>
        <button className="link small" onClick={copy}>
          {copied ? "Copied" : "Copy"}
        </button>
      </figcaption>
    </figure>
  );
}

function BlockView({ block }: { block: Block }) {
  switch (block.type) {
    case "paragraph":
      return (
        <p>
          <InlineView nodes={block.children} />
        </p>
      );
    case "heading": {
      const Tag = (["h3", "h4", "h5"] as const)[block.level - 1];
      return (
        <Tag>
          <InlineView nodes={block.children} />
        </Tag>
      );
    }
    case "code":
      if (block.lang === "mermaid") return <Diagram code={block.text} closed={block.closed} />;
      if (block.lang === "svg" && block.closed) return <SvgPicture text={block.text} />;
      return (
        <pre className="code-block" data-lang={block.lang || undefined}>
          <code>{block.text}</code>
        </pre>
      );
    case "list": {
      const items = block.items.map((item, i) => (
        <li key={i}>
          <InlineView nodes={item} />
        </li>
      ));
      return block.ordered ? <ol>{items}</ol> : <ul>{items}</ul>;
    }
    case "quote":
      return (
        <blockquote>
          <InlineView nodes={block.children} />
        </blockquote>
      );
    case "rule":
      return <hr />;
    case "table":
      return <TableView block={block} />;
    case "media":
      return <MediaPicture path={block.path} />;
  }
}

export function MarkdownView({ text }: { text: string }) {
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  return (
    <div className="markdown">
      {blocks.map((block, i) => (
        <BlockView key={i} block={block} />
      ))}
    </div>
  );
}
