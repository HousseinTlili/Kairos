import { useEffect, useState } from "react";
import { drawDiagram, useScheme } from "./mermaid-draw";

/**
 * A Mermaid diagram in an answer (M13 Step 3). Drawn once its code block has closed; until then,
 * and if Mermaid can't read it, the code shows instead.
 */
export function Diagram({ code, closed }: { code: string; closed: boolean }) {
  const scheme = useScheme();
  const key = `${scheme}\n${code}`;
  // Each drawing remembers what it was drawn from, so a change of code or scheme redraws.
  const [drawn, setDrawn] = useState<{ key: string; svg: string | null } | null>(null);
  const [source, setSource] = useState(false);

  useEffect(() => {
    if (!closed) return;
    let live = true;
    drawDiagram(code, scheme)
      .then((svg) => live && setDrawn({ key, svg }))
      .catch(() => live && setDrawn({ key, svg: null }));
    return () => {
      live = false;
    };
  }, [code, closed, scheme, key]);

  const codeBlock = (
    <pre className="code-block" data-lang="mermaid">
      <code>{code}</code>
    </pre>
  );
  const current = drawn?.key === key ? drawn : null;
  if (!closed) return <p className="diagram-wait muted small">Drawing a diagram…</p>;
  if (current && current.svg === null) {
    return (
      <>
        {codeBlock}
        <p className="muted small">Couldn&apos;t draw this diagram.</p>
      </>
    );
  }
  return (
    <figure className="diagram">
      {source ? (
        codeBlock
      ) : current?.svg ? (
        // The one place the chat inserts markup (D-028): Mermaid's own SVG, drawn with
        // securityLevel "strict" and sanitised by Mermaid, never the model's text as such.
        <div className="diagram-svg" dangerouslySetInnerHTML={{ __html: current.svg }} />
      ) : (
        <p className="diagram-wait muted small">Drawing a diagram…</p>
      )}
      <figcaption>
        <button className="link small" onClick={() => setSource((s) => !s)}>
          {source ? "Show the diagram" : "Source"}
        </button>
      </figcaption>
    </figure>
  );
}
