import { useEffect, useState } from "react";
import { Menu } from "@tauri-apps/api/menu";
import {
  islandCapturing,
  islandExpand,
  islandOpenQuick,
  islandPushToTalk,
  islandSnooze,
  onIslandExpanded,
  onKairosState,
  onRunEvent,
  openInMain,
  quickHotkey,
  showMainWindow,
} from "../../lib/tauri";
import { hidesForCapture } from "../quick/model";
import { SealMark } from "../shell/SealMark";
import { GridIcon, HomeIcon, ImageIcon, MicIcon, MusicIcon } from "../shell/icons";
import { DONE_MS, mergeStates, type KairosState } from "./model";
import {
  AppsTab,
  MediaTab,
  NowPlayingCard,
  OpenApps,
  PinnedStrip,
  WallpaperBackdrop,
  WallpaperTab,
} from "./panel/tabs";

type Tab = "home" | "media" | "wallpaper" | "apps";
const TABS: { id: Tab; label: string; icon: () => React.ReactElement }[] = [
  { id: "home", label: "Home", icon: HomeIcon },
  { id: "media", label: "Media", icon: MusicIcon },
  { id: "wallpaper", label: "Wallpaper", icon: ImageIcon },
  { id: "apps", label: "Apps", icon: GridIcon },
];
const TAB_KEY = "kairos.island.tab";

function savedTab(): Tab {
  try {
    const tab = localStorage.getItem(TAB_KEY);
    return TABS.some((t) => t.id === tab) ? (tab as Tab) : "home";
  } catch {
    return "home";
  }
}

/**
 * The island pill (M14 §1): Kairos's state at a glance, at the top of the screen. A click opens the
 * quick box under it (or the waiting approval); the microphone is push-to-talk; right-click for
 * the menu. Still unless the state changes: an always-on-top transparent window that animates all
 * the time costs a third of a core (Elissa's measurement).
 */
export function IslandApp() {
  const [states, setStates] = useState<Partial<Record<KairosState["source"], KairosState>>>({});
  const [now, setNow] = useState(() => Date.now());
  // M15: a click expands the pill into a panel with tabs.
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<Tab>(savedTab);

  useEffect(() => {
    document.documentElement.classList.add("island-window");
    // The page starts folded; the window must too, or an invisible expanded window would sit over
    // the top of the screen and take its clicks (seen 2026-10-30 after a reload).
    void islandExpand(false);
    const stops = [
      onIslandExpanded(setExpanded),
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

  // "Done" (and an automation's line) fades by itself: one re-check when it should be gone.
  const fadeAt = Math.max(
    0,
    ...Object.values(states).map((s) => (s?.doneAt ? s.doneAt + (s.holdMs ?? DONE_MS) : 0)),
  );
  useEffect(() => {
    if (!fadeAt || now >= fadeAt) return;
    const timer = setTimeout(() => setNow(Date.now()), fadeAt - now + 50);
    return () => clearTimeout(timer);
  }, [fadeAt, now]);

  const shown = mergeStates(
    Object.values(states).filter((s): s is KairosState => !!s),
    now,
  );
  const mood = shown?.mood ?? "idle";
  const text = shown?.text ?? "At your service";

  // Folds back on Esc and when another window is clicked (the panel took the focus when opened).
  useEffect(() => {
    if (!expanded) return;
    const fold = () => {
      // Development only (Elissa's ELISSA_NO_BLUR_COLLAPSE): test tools take the focus away.
      if (import.meta.env.DEV && (window as { kairosKeepOpen?: boolean }).kairosKeepOpen) return;
      void islandExpand(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") fold();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", fold);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", fold);
    };
  }, [expanded]);

  function open() {
    if (shown?.approval && shown.sessionId) void openInMain(shown.sessionId);
    else void islandExpand(!expanded);
  }

  function pickTab(next: Tab) {
    setTab(next);
    try {
      localStorage.setItem(TAB_KEY, next);
    } catch {
      // A private profile just starts on Home next time.
    }
  }

  function ask() {
    void islandExpand(false);
    void islandOpenQuick();
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

  const bar = (
    <div className="island-bar">
      <button
        className="island-main"
        onClick={open}
        title={
          shown?.approval ? "Open the approval" : expanded ? "Fold the island" : "Open the island"
        }
      >
        <SealMark size={26} state={mood} level={0} />
        <span className="island-text" aria-live="polite">
          {text}
        </span>
      </button>
      {expanded ? (
        <nav className="island-tabs" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              className={tab === t.id ? "active" : ""}
              onClick={() => pickTab(t.id)}
              title={t.label}
            >
              <t.icon />
              <span>{t.label}</span>
            </button>
          ))}
        </nav>
      ) : (
        <Clock className="island-clock" />
      )}
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

  if (!expanded) {
    return (
      <div
        className={`island ${mood} ${shown?.approval ? "asking" : ""}`}
        onContextMenu={(e) => void menu(e)}
      >
        {bar}
      </div>
    );
  }
  return (
    <div
      className={`island expanded ${mood} ${shown?.approval ? "asking" : ""}`}
      onContextMenu={(e) => void menu(e)}
    >
      {bar}
      <section className={`island-body ${tab}`}>
        {tab === "home" ? (
          <HomeTab status={shown?.text} onAsk={ask} />
        ) : tab === "media" ? (
          <MediaTab />
        ) : tab === "wallpaper" ? (
          <WallpaperTab />
        ) : (
          <AppsTab />
        )}
      </section>
    </div>
  );
}

/** The time now, re-read every 15 s: one text change a minute, no animation. */
function useNow() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

const time = (d: Date) => d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

function Clock({ className }: { className: string }) {
  return <span className={className}>{time(useNow())}</span>;
}

function greeting(hour: number): string {
  if (hour < 5) return "You keep late hours, Master";
  if (hour < 12) return "Good morning, Master";
  if (hour < 18) return "Good afternoon, Master";
  return "Good evening, Master";
}

/**
 * Home: the hour over the Master's wallpaper, what's playing, a way to ask, and a dock of the open
 * and pinned apps. Everything on it is live; each piece also has its own tab.
 */
function HomeTab({ status, onAsk }: { status?: string; onAsk: () => void }) {
  const now = useNow();
  const [key, setKey] = useState<string | null>(null);
  useEffect(() => {
    void quickHotkey()
      .then(setKey)
      .catch(() => {});
  }, []);
  return (
    <div className="home-tab">
      <div className="card hero-card">
        <WallpaperBackdrop />
        <p className="eyebrow">{greeting(now.getHours())}</p>
        <p className="home-clock">{time(now)}</p>
        <p className="home-date">
          {now.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" })}
        </p>
      </div>
      <NowPlayingCard />
      <button className="home-ask" onClick={onAsk}>
        <SealMark size={22} state="idle" level={0} />
        <span className="home-ask-words">
          <span>Ask Kairos…</span>
          {status && <span className="small muted">{status}</span>}
        </span>
        {key && <kbd>{key.replace(/\+/g, " ")}</kbd>}
      </button>
      <div className="card dock">
        <div className="dock-part">
          <p className="eyebrow">Open now</p>
          <OpenApps />
        </div>
        <span className="dock-rule" aria-hidden="true" />
        <div className="dock-part">
          <p className="eyebrow">Pinned</p>
          <PinnedStrip onEmpty={<p className="small muted">Pin apps in the Apps tab.</p>} />
        </div>
      </div>
    </div>
  );
}
