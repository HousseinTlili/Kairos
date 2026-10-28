import { useEffect, useState } from "react";
import { mediaImage, saveImage, type CommandError } from "../../lib/tauri";

// Pictures in answers (M13 Step 4, D-028). Every picture is an <img>: an SVG shown that way can't
// run scripts or load anything, and only data: and blob: sources reach here (markdown.ts).

/** SVG sketches the model writes in a ```svg block are shown only below this size. */
const SVG_MAX_CHARS = 100_000;

const MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
};

function extensionOf(src: string): string {
  const mime = /^data:image\/([a-z+]+)/i.exec(src)?.[1]?.toLowerCase();
  if (mime === "svg+xml") return "svg";
  if (mime === "jpeg") return "jpg";
  return mime ?? "png";
}

/** A picture: a thumbnail in the answer, full size on click, Esc or a click outside to close. */
export function Picture({ src, alt, name }: { src: string; alt: string; name?: string }) {
  const [open, setOpen] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open]);

  async function save() {
    try {
      const bytes = new Uint8Array(await (await fetch(src)).arrayBuffer());
      const path = await saveImage(bytes, name ?? `kairos-image.${extensionOf(src)}`);
      setSaved(`Saved to ${path}`);
    } catch (err) {
      setSaved((err as CommandError).message ?? "Couldn't save the picture.");
    }
  }

  return (
    <>
      <button className="md-picture" onClick={() => setOpen(true)} title="Show full size">
        <img src={src} alt={alt || "Picture"} />
      </button>
      {open && (
        <div
          className="lightbox"
          role="dialog"
          aria-label={alt || "Picture"}
          onClick={() => setOpen(false)}
        >
          <img src={src} alt={alt || "Picture"} onClick={(e) => e.stopPropagation()} />
          <div className="lightbox-bar" onClick={(e) => e.stopPropagation()}>
            {saved && <span className="muted small">{saved}</span>}
            <button onClick={() => void save()}>Save</button>
            <button className="primary" onClick={() => setOpen(false)}>
              Close
            </button>
          </div>
        </div>
      )}
    </>
  );
}

/** A `MEDIA:` path from an older conversation, read through Kairos (Hermes's folders only). */
export function MediaPicture({ path }: { path: string }) {
  // Each result remembers its path: while an answer streams, the path grows a few characters at
  // a time, and a result for an earlier, shorter path no longer counts.
  const [loaded, setLoaded] = useState<{ path: string; src: string | null } | null>(null);

  useEffect(() => {
    let url: string | null = null;
    let live = true;
    const extension = path.split(".").pop()?.toLowerCase() ?? "";
    mediaImage(path)
      .then((bytes) => {
        if (!live) return;
        url = URL.createObjectURL(new Blob([bytes], { type: MIME[extension] ?? "image/png" }));
        setLoaded({ path, src: url });
      })
      .catch(() => live && setLoaded({ path, src: null }));
    return () => {
      live = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  if (loaded?.path !== path) return <p className="muted small">Picture…</p>;
  const src = loaded.src;
  if (!src) return <p className="muted small">Picture not available.</p>;
  const name = path.split(/[\\/]/).pop();
  return <Picture src={src} alt={name ?? "Picture"} name={name} />;
}

/** The model's ```svg sketch, as an image; anything that isn't a plain SVG stays code. */
export function SvgPicture({ text }: { text: string }) {
  if (text.length > SVG_MAX_CHARS || !/^\s*(<\?xml[^>]*>\s*)?<svg[\s>]/i.test(text)) {
    return (
      <pre className="code-block" data-lang="svg">
        <code>{text}</code>
      </pre>
    );
  }
  const src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(text)}`;
  return <Picture src={src} alt="Drawing" name="kairos-drawing.svg" />;
}
