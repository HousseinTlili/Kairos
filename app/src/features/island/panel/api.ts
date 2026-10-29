// The island panel's modules (M15), ported from Elissa: one Rust command routes to a module by id.

import { invoke } from "@tauri-apps/api/core";

export function callModule<T>(module: string, command: string, args?: unknown): Promise<T> {
  return invoke<T>("island_module", { module, command, args: args ?? null });
}

export type Playback = "playing" | "paused" | "stopped" | "busy" | "closed";

export interface MediaState {
  has_session: boolean;
  app: string | null;
  title: string;
  artist: string;
  album: string;
  status: Playback;
  controls: {
    play: boolean;
    pause: boolean;
    toggle: boolean;
    next: boolean;
    previous: boolean;
    seek: boolean;
  };
  track_key: string;
  has_thumbnail: boolean;
  timeline: { position_ms: number; duration_ms: number; rate: number } | null;
  sessions_count: number;
}

export interface MediaUpdate {
  state: MediaState;
  error: string | null;
}

export interface WallpaperImage {
  path: string;
  name: string;
}

export interface Monitor {
  id: string;
  label: string;
  width: number;
  height: number;
  primary: boolean;
  current: string;
}

export interface WallpaperState {
  folder: string;
  images: WallpaperImage[];
  monitors: Monitor[];
  error: string | null;
}

export interface OpenApp {
  key: string;
  name: string;
  icon: string | null;
}

export interface AppEntry {
  id: string;
  name: string;
  kind: "classic" | "store";
  pinned: boolean;
}

export interface Pin {
  id: number;
  name: string;
  kind: "classic" | "store";
  position: number;
  icon: string | null;
}

/** "3:07" from milliseconds. */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Where the track is now: the last reported position, moved on while it plays. */
export function positionNow(state: MediaState, receivedAt: number, now: number): number {
  const t = state.timeline;
  if (!t) return 0;
  const moved = state.status === "playing" ? (now - receivedAt) * t.rate : 0;
  return Math.min(t.duration_ms, Math.max(0, t.position_ms + moved));
}

/** Every word of the query in the name, any case, any order (Elissa's picker filter). */
export function filterApps(apps: AppEntry[], query: string): AppEntry[] {
  const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return apps.filter((app) => {
    const name = app.name.toLocaleLowerCase();
    return words.every((w) => name.includes(w));
  });
}

/** Two Windows paths name the same file whatever the slashes or case. */
export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.split("/").join("\\").toLowerCase();
  return norm(a) === norm(b);
}

/**
 * A player's name for people: Windows gives Store apps as "Publisher.AppName.12345" and desktop
 * apps as "Spotify.exe"; this keeps the app's own name and spaces out its words.
 */
export function appLabel(app: string): string {
  const parts = app
    .replace(/\.exe$/i, "")
    .split(/[.!]/)
    .filter((p) => p && !/^\d+$/.test(p));
  // A Store package family ends in "_" and the publisher's hash.
  const name = (parts.length > 1 ? parts[1] : (parts[0] ?? app)).replace(/_[a-z0-9]+$/i, "");
  return name.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2");
}
