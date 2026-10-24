import { useMemo } from "react";
import { parseMarkdown, type Block, type Inline } from "./markdown";

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
        }
      })}
    </>
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
