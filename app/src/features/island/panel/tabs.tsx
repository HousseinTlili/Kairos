import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, type ReactNode } from "react";
import {
  GridIcon,
  ImageIcon,
  MusicIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PreviousIcon,
} from "../../shell/icons";
import {
  appLabel,
  callModule,
  clock,
  filterApps,
  positionNow,
  samePath,
  type AppEntry,
  type MediaState,
  type MediaUpdate,
  type OpenApp,
  type Pin,
  type WallpaperState,
} from "./api";

function messageOf(err: unknown): string {
  return typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
}

// --- Open apps ------------------------------------------------------------------------------------

/**
 * Icons of the apps with a window open. A click brings it to the front; a right-click arms a red ×
 * and a click on it asks the app to close (it can still ask to save): Elissa's deliberate two-step.
 */
export function OpenApps() {
  // null until the first scan: the list is empty before it, which isn't "no apps open".
  const [apps, setApps] = useState<OpenApp[] | null>(null);
  const [armed, setArmed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // The scan runs only while this row is on screen.
    void callModule("openapps", "set_visible", { visible: true });
    void callModule<OpenApp[]>("openapps", "list").then((list) => {
      if (list.length) setApps(list);
    });
    const stop = listen<OpenApp[]>("openapps://list", (e) => setApps(e.payload));
    return () => {
      void callModule("openapps", "set_visible", { visible: false });
      void stop.then((u) => u());
    };
  }, []);

  const act = (command: "focus" | "close", key: string) =>
    void callModule("openapps", command, { key })
      .then(() => setError(null))
      .catch((err) => setError(messageOf(err)));

  if (!apps) return <p className="muted small">Looking at your windows…</p>;
  if (apps.length === 0) return <p className="muted small">No app windows open.</p>;
  return (
    <div className="open-apps">
      {apps.map((app) => (
        <span key={app.key} className="open-app">
          <button
            className="app-icon"
            title={app.name}
            aria-label={`Bring ${app.name} to the front`}
            onClick={() => act("focus", app.key)}
            onContextMenu={(e) => {
              e.preventDefault();
              setArmed(armed === app.key ? null : app.key);
            }}
          >
            {app.icon ? <img src={app.icon} alt="" /> : <span>{app.name.slice(0, 1)}</span>}
          </button>
          {armed === app.key && (
            <button
              className="app-close"
              aria-label={`Close ${app.name}`}
              title={`Close ${app.name}`}
              onClick={() => {
                setArmed(null);
                act("close", app.key);
              }}
            >
              ×
            </button>
          )}
        </span>
      ))}
      {error && <p className="bad-text small">{error}</p>}
    </div>
  );
}

// --- Media ----------------------------------------------------------------------------------------

/** What is playing in any app; subscribed to Windows only while a media view is on screen. */
function useMedia() {
  const [update, setUpdate] = useState<{ value: MediaUpdate; at: number } | null>(null);
  const [cover, setCover] = useState<{ key: string; url: string | null } | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const take = (value: MediaUpdate) => setUpdate({ value, at: Date.now() });
    void callModule("media", "visible", { visible: true });
    void callModule<MediaUpdate>("media", "state").then(take);
    const stop = listen<MediaUpdate>("media://state", (e) => take(e.payload));
    return () => {
      void callModule("media", "visible", { visible: false });
      void stop.then((u) => u());
    };
  }, []);

  const state = update?.value.state;
  const key = state?.has_thumbnail ? state.track_key : null;
  useEffect(() => {
    if (!key) return;
    let live = true;
    void callModule<string | null>("media", "cover", { key }).then(
      (url) => live && setCover({ key, url }),
    );
    return () => {
      live = false;
    };
  }, [key]);

  // The progress bar moves once a second while playing; nothing runs when paused.
  const playing = state?.status === "playing";
  useEffect(() => {
    if (!playing) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [playing]);

  return {
    update,
    state: update?.value.state ?? null,
    coverUrl: cover?.key === key ? cover.url : null,
    at: state && update ? positionNow(state, update.at, now) : 0,
    playing,
  };
}

function MediaControls({ state, playing }: { state: MediaState; playing: boolean }) {
  const act = (command: string) => void callModule("media", command).catch(() => {});
  return (
    <div className="media-controls">
      <button
        disabled={!state.controls.previous}
        onClick={() => act("previous")}
        aria-label="Previous"
        title="Previous"
      >
        <PreviousIcon />
      </button>
      <button
        className="media-play"
        disabled={!state.controls.toggle}
        onClick={() => act("toggle")}
        aria-label={playing ? "Pause" : "Play"}
        title={playing ? "Pause" : "Play"}
      >
        {playing ? <PauseIcon /> : <PlayIcon />}
      </button>
      <button
        disabled={!state.controls.next}
        onClick={() => act("next")}
        aria-label="Next"
        title="Next"
      >
        <NextIcon />
      </button>
    </div>
  );
}

function Cover({ url }: { url: string | null }) {
  return (
    <div className="media-cover">
      {url ? (
        <img src={url} alt="" />
      ) : (
        <span>
          <MusicIcon />
        </span>
      )}
    </div>
  );
}

/** The Media tab: the cover large, over a blur of itself. */
export function MediaTab() {
  const { update, state, coverUrl, at, playing } = useMedia();
  if (!update) return <Empty icon={<MusicIcon />} text="Looking for something playing…" />;
  if (update.value.error) return <Empty icon={<MusicIcon />} text={update.value.error} />;
  if (!state?.has_session) {
    return (
      <Empty icon={<MusicIcon />} text="Nothing is playing. Start music or a video in any app." />
    );
  }
  const t = state.timeline;
  return (
    <div className="media-tab">
      {coverUrl && <img className="media-backdrop" src={coverUrl} alt="" />}
      <Cover url={coverUrl} />
      <div className="media-body">
        <p className="eyebrow">{playing ? "Now playing" : "Paused"}</p>
        <p className="media-title">{state.title || "Untitled"}</p>
        <p className="media-artist muted">
          {[state.artist, state.album].filter(Boolean).join(" · ")}
        </p>
        {state.app && <p className="media-app small muted">{appLabel(state.app)}</p>}
        {t && t.duration_ms > 0 && (
          <div className="media-progress" aria-label="Progress">
            <span className="small muted">{clock(at)}</span>
            <span className="media-bar">
              <span style={{ transform: `scaleX(${at / t.duration_ms})` }} />
            </span>
            <span className="small muted">{clock(t.duration_ms)}</span>
          </div>
        )}
        <MediaControls state={state} playing={playing} />
      </div>
    </div>
  );
}

/** Home's small player: the track and its controls, or a quiet note when nothing plays. */
export function NowPlayingCard() {
  const { state, coverUrl, at, playing } = useMedia();
  if (!state?.has_session) {
    return (
      <div className="card now-card idle">
        <p className="eyebrow">Music</p>
        <div className="now-idle">
          <MusicIcon />
          <span className="small muted">Nothing playing</span>
        </div>
      </div>
    );
  }
  const t = state.timeline;
  return (
    <div className="card now-card">
      {coverUrl && <img className="media-backdrop" src={coverUrl} alt="" />}
      <p className="eyebrow">{playing ? "Now playing" : "Paused"}</p>
      <div className="now-track">
        <Cover url={coverUrl} />
        <div className="now-words">
          <p className="now-title">{state.title || "Untitled"}</p>
          <p className="small muted">{state.artist || (state.app ? appLabel(state.app) : "")}</p>
        </div>
      </div>
      {t && t.duration_ms > 0 && (
        <span className="media-bar thin">
          <span style={{ transform: `scaleX(${at / t.duration_ms})` }} />
        </span>
      )}
      <MediaControls state={state} playing={playing} />
    </div>
  );
}

/** A tab with nothing to show yet: a large faint glyph and a line, not a lone sentence. */
export function Empty({ icon, text }: { icon: ReactNode; text: string }) {
  return (
    <div className="island-empty">
      <span className="island-empty-icon">{icon}</span>
      <p className="small muted">{text}</p>
    </div>
  );
}

// --- Wallpaper ------------------------------------------------------------------------------------

// Thumbnails are slow to make and small to keep: kept for the app's lifetime (as Elissa).
const thumbs = new Map<string, string | null>();

function Thumb({
  path,
  name,
  active,
  onPick,
}: {
  path: string;
  name: string;
  active: boolean;
  onPick: () => void;
}) {
  const [url, setUrl] = useState<string | null | undefined>(() => thumbs.get(path));
  useEffect(() => {
    if (thumbs.has(path)) return;
    let live = true;
    void callModule<string | null>("wallpaper", "thumbnail", { path })
      .catch(() => null)
      .then((u) => {
        thumbs.set(path, u);
        if (live) setUrl(u);
      });
    return () => {
      live = false;
    };
  }, [path]);
  return (
    <button className={`wall-thumb ${active ? "active" : ""}`} onClick={onPick} title={name}>
      {url ? <img src={url} alt={name} /> : <span className="small muted">{name}</span>}
    </button>
  );
}

/** The Master's wallpaper folder: next, previous, or a click on one; one monitor or all. */
export function WallpaperTab() {
  const [data, setData] = useState<WallpaperState | null>(null);
  const [monitor, setMonitor] = useState<string>("all");
  const [folder, setFolder] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = (state: WallpaperState) => {
    setData(state);
    setFolder(state.folder);
  };
  useEffect(() => {
    void callModule<WallpaperState>("wallpaper", "state")
      .then(load)
      .catch((err) => setError(messageOf(err)));
  }, []);

  const target = monitor === "all" ? null : monitor;
  const apply = (command: string, args: Record<string, unknown> = {}) =>
    void callModule<{ monitors: WallpaperState["monitors"] }>("wallpaper", command, {
      monitor: target,
      ...args,
    })
      .then((r) => {
        setError(null);
        setData((d) => (d ? { ...d, monitors: r.monitors } : d));
      })
      .catch((err) => setError(messageOf(err)));

  if (!data) return <p className="muted small">{error ?? "Reading the wallpaper folder…"}</p>;
  const current =
    (data.monitors.find((m) => m.id === target) ?? data.monitors.find((m) => m.primary))?.current ??
    "";
  return (
    <div className="wall-tab">
      <div className="wall-bar">
        <button onClick={() => apply("previous")} aria-label="Previous wallpaper">
          ‹
        </button>
        <button onClick={() => apply("next")} aria-label="Next wallpaper">
          ›
        </button>
        {data.monitors.length > 1 && (
          <select value={monitor} onChange={(e) => setMonitor(e.target.value)} aria-label="Monitor">
            <option value="all">All monitors</option>
            {data.monitors.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
                {m.primary ? " (main)" : ""}
              </option>
            ))}
          </select>
        )}
        <form
          className="wall-folder"
          onSubmit={(e) => {
            e.preventDefault();
            void callModule<WallpaperState>("wallpaper", "set_folder", { path: folder.trim() })
              .then((s) => {
                setError(null);
                load(s);
              })
              .catch((err) => setError(messageOf(err)));
          }}
        >
          <input
            value={folder}
            onChange={(e) => setFolder(e.target.value)}
            aria-label="Wallpaper folder"
          />
        </form>
      </div>
      {error && <p className="bad-text small">{error}</p>}
      {data.images.length === 0 ? (
        <Empty
          icon={<ImageIcon />}
          text={data.error ?? "No jpg, png or bmp images in this folder."}
        />
      ) : (
        <div className="wall-grid">
          {data.images.map((img) => (
            <Thumb
              key={img.path}
              path={img.path}
              name={img.name}
              active={samePath(img.path, current)}
              onPick={() => apply("set", { path: img.path })}
            />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Home's picture: the current wallpaper (of the main monitor) and a step either way. The thumbnail
 * comes from the same cache as the Wallpaper tab's.
 */
function useWallpaper() {
  const [current, setCurrent] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<{ path: string; url: string | null } | null>(null);

  const take = (monitors: WallpaperState["monitors"]) =>
    setCurrent((monitors.find((m) => m.primary) ?? monitors[0])?.current || null);
  useEffect(() => {
    void callModule<WallpaperState>("wallpaper", "state")
      .then((s) => take(s.monitors))
      .catch(() => {});
  }, []);
  useEffect(() => {
    if (!current || thumbs.has(current)) return;
    let live = true;
    void callModule<string | null>("wallpaper", "thumbnail", { path: current })
      .catch(() => null)
      .then((u) => {
        thumbs.set(current, u);
        if (live) setLoaded({ path: current, url: u });
      });
    return () => {
      live = false;
    };
  }, [current]);
  const url = !current
    ? null
    : thumbs.has(current)
      ? (thumbs.get(current) ?? null)
      : loaded?.path === current
        ? loaded.url
        : null;

  const step = (command: "next" | "previous") =>
    void callModule<{ monitors: WallpaperState["monitors"] }>("wallpaper", command, {
      monitor: null,
    })
      .then((r) => take(r.monitors))
      .catch(() => {});
  return { url, step };
}

/** The wallpaper behind Home's hour, with a step either way in the corner. */
export function WallpaperBackdrop() {
  const { url, step } = useWallpaper();
  return (
    <>
      {url && <img className="hero-picture" src={url} alt="" />}
      <span className="hero-steps">
        <button onClick={() => step("previous")} title="Previous wallpaper">
          ‹
        </button>
        <button onClick={() => step("next")} title="Next wallpaper">
          ›
        </button>
      </span>
    </>
  );
}

// --- Apps -----------------------------------------------------------------------------------------

/**
 * The pinned apps as icons, one click to start each. A new `refresh` reads them again, keeping the
 * icons on screen meanwhile: on the top bar an empty row for a moment narrowed the bar, and the
 * whole bar jumped sideways and back (the flicker seen 2026-11-01).
 */
export function PinnedStrip({ onEmpty, refresh }: { onEmpty: ReactNode; refresh?: number }) {
  const [pins, setPins] = useState<Pin[] | null>(null);
  useEffect(() => {
    void callModule<Pin[]>("pinned", "list")
      .then(setPins)
      .catch(() => setPins([]));
  }, [refresh]);
  if (!pins) return null;
  if (pins.length === 0) return <>{onEmpty}</>;
  return (
    <div className="open-apps">
      {pins.map((pin) => (
        <button
          key={pin.id}
          className="app-icon"
          title={`Start ${pin.name}`}
          aria-label={`Start ${pin.name}`}
          onClick={() => void callModule("pinned", "launch", { id: pin.id }).catch(() => {})}
        >
          {pin.icon ? <img src={pin.icon} alt="" /> : <span>{pin.name.slice(0, 1)}</span>}
        </button>
      ))}
    </div>
  );
}

/** Pinned apps launched with one click, and a picker over the Start menu and Store apps. */
export function AppsTab() {
  const [pins, setPins] = useState<Pin[]>([]);
  const [picking, setPicking] = useState(false);
  const [editing, setEditing] = useState(false);
  const [catalog, setCatalog] = useState<AppEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  const fail = (err: unknown) => setError(messageOf(err));
  const reload = () => void callModule<Pin[]>("pinned", "list").then(setPins).catch(fail);
  useEffect(reload, []);
  useEffect(() => {
    if (!picking || catalog) return;
    void callModule<AppEntry[]>("pinned", "catalog").then(setCatalog).catch(fail);
  }, [picking, catalog]);

  return (
    <div className="apps-tab">
      <p className="apps-label">Open now</p>
      <OpenApps />
      <div className="apps-head">
        <p className="apps-label">Pinned</p>
        <span className="spacer" />
        {pins.length > 0 && (
          <button className="link small" onClick={() => setEditing((e) => !e)}>
            {editing ? "Done" : "Edit"}
          </button>
        )}
        <button className="link small" onClick={() => setPicking((p) => !p)}>
          {picking ? "Close" : "Pin an app"}
        </button>
      </div>
      {error && <p className="bad-text small">{error}</p>}
      {picking ? (
        <div className="pin-picker">
          <input
            autoFocus
            placeholder="Find an app"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <ul>
            {(catalog ? filterApps(catalog, query).slice(0, 40) : []).map((app) => (
              <li key={app.id}>
                <button
                  disabled={app.pinned}
                  onClick={() =>
                    void callModule("pinned", "add", { id: app.id })
                      .then(() => {
                        setCatalog(null);
                        reload();
                      })
                      .catch(fail)
                  }
                >
                  {app.name}
                  {app.pinned ? " · pinned" : ""}
                </button>
              </li>
            ))}
          </ul>
          {!catalog && <p className="muted small">Reading the Start menu…</p>}
        </div>
      ) : pins.length === 0 ? (
        <Empty icon={<GridIcon />} text="Pin the apps you open most; one click starts them." />
      ) : (
        <div className="pin-grid">
          {pins.map((pin) => (
            <span key={pin.id} className="open-app">
              <button
                className="app-icon large"
                title={pin.name}
                onClick={() =>
                  void callModule("pinned", "launch", { id: pin.id })
                    .then(() => setError(null))
                    .catch(fail)
                }
              >
                {pin.icon ? <img src={pin.icon} alt="" /> : <span>{pin.name.slice(0, 1)}</span>}
                <span className="pin-name">{pin.name}</span>
              </button>
              {editing && (
                <button
                  className="app-close"
                  aria-label={`Unpin ${pin.name}`}
                  onClick={() =>
                    void callModule("pinned", "remove", { id: pin.id }).then(reload).catch(fail)
                  }
                >
                  ×
                </button>
              )}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
