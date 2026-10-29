import { describe, expect, it } from "vitest";
import {
  appLabel,
  clock,
  filterApps,
  positionNow,
  samePath,
  type AppEntry,
  type MediaState,
} from "./api";

const song: MediaState = {
  has_session: true,
  app: "Spotify",
  title: "Song",
  artist: "Artist",
  album: "",
  status: "playing",
  controls: { play: true, pause: true, toggle: true, next: true, previous: true, seek: true },
  track_key: "k",
  has_thumbnail: false,
  timeline: { position_ms: 60_000, duration_ms: 180_000, rate: 1 },
  sessions_count: 1,
};

describe("the island panel", () => {
  it("moves the progress on while a track plays, and not while paused", () => {
    expect(positionNow(song, 1000, 6000)).toBe(65_000);
    expect(positionNow({ ...song, status: "paused" }, 1000, 6000)).toBe(60_000);
  });

  it("never runs the progress past the end of the track", () => {
    expect(positionNow(song, 0, 10_000_000)).toBe(180_000);
  });

  it("writes times as minutes and seconds", () => {
    expect(clock(187_000)).toBe("3:07");
    expect(clock(-5)).toBe("0:00");
  });

  it("finds apps by every word typed, in any case or order", () => {
    const apps: AppEntry[] = ["Notepad", "Brave Browser", "Calculator"].map((name) => ({
      id: name,
      name,
      kind: "classic",
      pinned: false,
    }));
    expect(filterApps(apps, "BROWSER brave").map((a) => a.name)).toEqual(["Brave Browser"]);
    expect(filterApps(apps, "")).toHaveLength(3);
  });

  it("names the player the way people would", () => {
    expect(appLabel("OperaSoftware.OperaGXWebBrowser.1787159879")).toBe("Opera GX Web Browser");
    expect(appLabel("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify")).toBe("Spotify Music");
    expect(appLabel("Spotify.exe")).toBe("Spotify");
    expect(appLabel("vlc.exe")).toBe("vlc");
  });

  it("treats a wallpaper path the same whatever its slashes or case", () => {
    expect(samePath("C:/Users/U/Pictures/A.jpg", "c:\\users\\u\\pictures\\a.JPG")).toBe(true);
    expect(samePath("C:/a.jpg", "C:/b.jpg")).toBe(false);
  });
});
