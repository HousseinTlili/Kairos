import { useEffect, useState } from "react";
import { Menu } from "@tauri-apps/api/menu";
import {
  islandCapturing,
  islandOpenQuick,
  islandPushToTalk,
  islandSnooze,
  onKairosState,
  onRunEvent,
  openInMain,
  showMainWindow,
} from "../../lib/tauri";
import { hidesForCapture } from "../quick/model";
import { SealMark } from "../shell/SealMark";
import { MicIcon } from "../shell/icons";
import { DONE_MS, mergeStates, type KairosState } from "./model";

/**
 * The island pill (M14 §1): Kairos's state at a glance, at the top of the screen. A click opens the
 * quick box under it (or the waiting approval); the microphone is push-to-talk; right-click for
 * the menu. Still unless the state changes: an always-on-top transparent window that animates all
 * the time costs a third of a core (Elissa's measurement).
 */
export function IslandApp() {
  const [states, setStates] = useState<Partial<Record<KairosState["source"], KairosState>>>({});
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    document.documentElement.classList.add("island-window");
    const stops = [
      onKairosState((state) => {
        setNow(Date.now());
        setStates((prev) => ({ ...prev, [state.source]: state }));
      }),
      // Out of the picture while the screen is captured, as the quick box (M8 §4).
      onRunEvent(({ envelope }) => {
        const event = envelope.event;
        if (event.event === "tool.started" && hidesForCapture(event.tool))
          void islandCapturing(true);
        if (event.event === "tool.completed" && hidesForCapture(event.tool))
          void islandCapturing(false);
      }),
    ];
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten());
    };
  }, []);

  // "Done" fades by itself: one re-check when it should be gone.
  const doneAt = Math.max(0, ...Object.values(states).map((s) => s?.doneAt ?? 0));
  useEffect(() => {
    if (!doneAt || now - doneAt >= DONE_MS) return;
    const timer = setTimeout(() => setNow(Date.now()), DONE_MS - (now - doneAt) + 50);
    return () => clearTimeout(timer);
  }, [doneAt, now]);

  const shown = mergeStates(
    Object.values(states).filter((s): s is KairosState => !!s),
    now,
  );
  const mood = shown?.mood ?? "idle";
  const text = shown?.text ?? "At your service";

  function open() {
    if (shown?.approval && shown.sessionId) void openInMain(shown.sessionId);
    else void islandOpenQuick();
  }

  async function menu(event: React.MouseEvent) {
    event.preventDefault();
    const items = await Menu.new({
      items: [
        { id: "open", text: "Open Kairos", action: () => void showMainWindow() },
        { id: "hour", text: "Hide for an hour", action: () => void islandSnooze(60) },
        {
          id: "settings",
          text: "Island settings",
          action: () => void showMainWindow("settings:island"),
        },
      ],
    });
    await items.popup();
  }

  return (
    <div
      className={`island ${mood} ${shown?.approval ? "asking" : ""}`}
      onContextMenu={(e) => void menu(e)}
    >
      <button
        className="island-main"
        onClick={open}
        title={shown?.approval ? "Open the approval" : "Ask Kairos"}
      >
        <SealMark size={26} state={mood} level={0} />
        <span className="island-text" aria-live="polite">
          {text}
        </span>
      </button>
      <button
        className="island-mic"
        title="Hold to speak"
        aria-label="Hold to speak"
        onPointerDown={() => void islandPushToTalk(true)}
        onPointerUp={() => void islandPushToTalk(false)}
        onPointerLeave={(e) => {
          if (e.buttons) void islandPushToTalk(false);
        }}
      >
        <MicIcon />
      </button>
    </div>
  );
}
