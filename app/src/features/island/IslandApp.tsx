import { useEffect, useRef, useState } from "react";
import { Menu } from "@tauri-apps/api/menu";
import {
  islandCapturing,
  islandExpand,
  islandFit,
  islandOpenQuick,
  islandPushToTalk,
  islandSnooze,
  onIslandExpanded,
  onIslandShown,
  onKairosState,
  onRunEvent,
  openInMain,
  quickHotkey,
  showMainWindow,
} from "../../lib/tauri";
import { hidesForCapture } from "../quick/model";
import { SealMark } from "../shell/SealMark";
import { GridIcon, HomeIcon, ImageIcon, MusicIcon, SparkleIcon, WaveIcon } from "../shell/icons";
import { BarStatus } from "./BarStatus";
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
 * The top bar (M14, D-034): the Master's pinned apps, a field to ask Kairos that also shows what
 * Kairos is doing (a click opens the quick box, or the waiting approval), push-to-talk, and the
 * PC's status and time. The seal or the clock opens the panel (M15); right-click for the menu.
 * Still unless something changes: an always-on-top transparent window that animates all the time
 * costs a third of a core (Elissa's measurement).
 */
export function IslandApp() {
  const [states, setStates] = useState<Partial<Record<KairosState["source"], KairosState>>>({});
  const [now, setNow] = useState(() => Date.now());
  // M15: a click expands the pill into a panel with tabs.
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<Tab>(savedTab);
  // A short slide down each time the bar comes back (from the top edge, or after a maximized window).
  const [arriving, setArriving] = useState(false);
  // Bumped when the panel folds, so the bar re-reads the pins changed in its Apps tab.
  const [pinsSeen, setPinsSeen] = useState(0);
  const [quickKey, setQuickKey] = useState<string | null>(null);
  const barRef = useRef<HTMLElement>(null);

  // The window is as wide as the bar needs (D-034): measured here, whenever what it holds changes
  // (pinned apps arriving, the battery appearing), plus the flares that curve into the top edge.
  useEffect(() => {
    const bar = barRef.current;
    if (!bar) return;
    const flares = 2 * 30 + 4;
    const report = () =>
      void islandFit(Math.ceil(bar.getBoundingClientRect().width) + flares).catch(() => {});
    report();
    const observer = new ResizeObserver(report);
    observer.observe(bar);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    document.documentElement.classList.add("island-window");
    // The page starts folded; the window must too, or an invisible expanded window would sit over
    // the top of the screen and take its clicks (seen 2026-10-30 after a reload).
    void islandExpand(false);
    void quickHotkey()
      .then(setQuickKey)
      .catch(() => {});
    const stops = [
      onIslandExpanded((open) => {
        setExpanded(open);
        if (!open) setPinsSeen((n) => n + 1);
      }),
      onIslandShown(() => setArriving(true)),
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

  useEffect(() => {
    if (!arriving) return;
    const timer = setTimeout(() => setArriving(false), 300);
    return () => clearTimeout(timer);
  }, [arriving]);

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

  // Kairos is doing something worth showing in the ask field instead of its invitation.
  const busy = mood !== "idle" || !!shown?.approval || text !== "At your service";

  return (
    <div
      className={`topbar ${mood} ${shown?.approval ? "asking" : ""} ${expanded ? "expanded" : ""} ${arriving ? "arriving" : ""}`}
      onContextMenu={(e) => void menu(e)}
      // Opened, the window is full width: a click on its clear part folds the panel.
      onMouseDown={(e) => {
        if (expanded && e.target === e.currentTarget) void islandExpand(false);
      }}
    >
      <header className="topbar-bar" ref={barRef}>
        <div className="topbar-left">
          <button
            className="topbar-seal"
            onClick={() => void islandExpand(!expanded)}
            title={expanded ? "Fold the panel" : "Open the panel"}
            aria-label={expanded ? "Fold the panel" : "Open the panel"}
          >
            <SealMark size={30} state={mood} level={0} />
          </button>
          <span className="topbar-rule" aria-hidden="true" />
          <div className="topbar-apps">
            <PinnedStrip
              key={pinsSeen}
              onEmpty={
                <button
                  className="topbar-pin-hint"
                  onClick={() => {
                    pickTab("apps");
                    void islandExpand(true);
                  }}
                >
                  Pin your apps
                </button>
              }
            />
          </div>
        </div>

        <button
          className={`topbar-ask ${busy ? "busy" : ""}`}
          onClick={() => {
            if (shown?.approval && shown.sessionId) void openInMain(shown.sessionId);
            else ask();
          }}
          title={shown?.approval ? "Open the approval" : "Ask Kairos"}
        >
          <span className="topbar-spark">
            <SparkleIcon />
          </span>
          <span className="topbar-ask-text" aria-live="polite">
            {busy ? text : "Ask Kairos…"}
          </span>
          {quickKey && !busy && <kbd>{quickKey.replace(/\+/g, " + ")}</kbd>}
        </button>

        <div className="topbar-right">
          <button
            className="topbar-voice"
            title="Hold to speak"
            aria-label="Hold to speak"
            onPointerDown={() => void islandPushToTalk(true)}
            onPointerUp={() => void islandPushToTalk(false)}
            onPointerLeave={(e) => {
              if (e.buttons) void islandPushToTalk(false);
            }}
          >
            <WaveIcon />
          </button>
          <span className="topbar-rule" aria-hidden="true" />
          <BarStatus onClock={() => void islandExpand(!expanded)} />
        </div>
      </header>

      {expanded && (
        <section className="topbar-panel">
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
          <div className={`island-body ${tab}`}>
            {tab === "home" ? (
              <HomeTab status={busy ? text : undefined} onAsk={ask} />
            ) : tab === "media" ? (
              <MediaTab />
            ) : tab === "wallpaper" ? (
              <WallpaperTab />
            ) : (
              <AppsTab />
            )}
          </div>
        </section>
      )}
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
