import { useEffect, useRef, useState } from "react";
import standing from "../../assets/kairos/standing.mp4";
import { hidePresencePanel, onKairosState, onPanelShown, onWake } from "../../lib/tauri";
import { DONE_MS, type KairosState } from "./model";

/** After the answer ends, the panel stays this long; a wake that hears nothing closes sooner. */
const LINGER_MS = DONE_MS;
const NOTHING_HEARD_MS = 9000;
/** The slide out (global.css .presence-panel), before the window hides. */
const SLIDE_MS = 240;

/**
 * The wake panel (M14 §1): the Master's standing Kairos slides in from the right on the wake word,
 * with what he heard and the start of the answer, and slides away after. The video exists only
 * while the panel is open, so a hidden panel costs nothing.
 */
export function PresencePanel() {
  const [open, setOpen] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [state, setState] = useState<KairosState | null>(null);
  const active = useRef(false);
  const shownAt = useRef(0);
  // The request the quick box already showed when the panel opened: not this wake's, so not shown.
  const [earlier, setEarlier] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    document.documentElement.classList.add("panel-window");
    const show = () => {
      active.current = false;
      shownAt.current = Date.now();
      setEarlier(undefined);
      setState(null);
      setLeaving(false);
      setOpen(true);
    };
    const stops = [
      onPanelShown(show),
      onWake(show),
      // The wake word's conversation runs in the quick box.
      onKairosState((next) => {
        if (next.source !== "quick") return;
        setEarlier((known) => (known === undefined ? next.heard : known));
        setState(next);
      }),
    ];
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten());
    };
  }, []);

  function close() {
    setLeaving(true);
    setTimeout(() => {
      setOpen(false);
      setLeaving(false);
      void hidePresencePanel();
    }, SLIDE_MS);
  }

  // Stays while Kairos listens, works or speaks; slides out once it rests again.
  useEffect(() => {
    if (!open || leaving) return;
    const busy = state !== null && (state.mood !== "idle" || state.approval);
    if (busy) {
      active.current = true;
      return;
    }
    const wait = active.current
      ? LINGER_MS
      : Math.max(0, NOTHING_HEARD_MS - (Date.now() - shownAt.current));
    const timer = setTimeout(close, wait);
    return () => clearTimeout(timer);
  }, [open, leaving, state]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  if (!open) return null;
  const fresh = state !== null && state.heard !== earlier;
  return (
    <aside className={`presence-panel ${leaving ? "leaving" : ""}`} aria-label="Kairos">
      <video className="presence-video" src={standing} autoPlay loop muted playsInline />
      <div className="presence-caption">
        <p className="presence-state">{state?.text ?? "Listening…"}</p>
        {fresh && state.heard && <p className="presence-heard">&ldquo;{state.heard}&rdquo;</p>}
        {fresh && state.answer && <p className="presence-answer">{state.answer}</p>}
      </div>
      <button className="presence-close icon-button" onClick={close} aria-label="Close">
        ×
      </button>
    </aside>
  );
}
