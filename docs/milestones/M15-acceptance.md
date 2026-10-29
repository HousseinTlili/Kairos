# M15 — Acceptance

Spec: `docs/milestones/M15.md`. Built 2026-10-29 on `m15/island-panel`.

## Checks run by the agent (2026-10-29)

| Check | Result |
|---|---|
| The port | Elissa's `wallpaper`, `media` (without `viz` and `fake`), `openapps`, `pinned`, and her `settings`, `registry`, `shell_icon`, `png`: about 3,000 lines, in `src-tauri/src/island_panel/`. No new crate; the `windows` features of D-030 |
| Elissa's own tests, in Kairos | 54 passed |
| Rust tests, clippy | 263 + 19 passed (new: the panel shares the pill's centre at any scaling; a wallpaper folder typed with forward slashes); clean |
| TS tests, lint, tsc | 171 passed (new: the progress bar, the app filter, wallpaper paths, the player's name); clean |
| Home tab (dev app) | Clock and date, Kairos's state, "Ask Kairos…", the open apps' real icons |
| Wallpaper tab | The product owner's folder (`Desktop\Wallpaper`, 76 images): every thumbnail loads, the current one outlined. **Found and fixed:** a folder given as `C:/Users/...` made every thumbnail fail ("The parameter is incorrect"); paths now get Windows slashes |
| Changing the wallpaper (said beforehand) | "Next" changed it (`ASQQ (9)` → `berserker-armor…`), then it was set back: the same file again. The wallpaper was `ASQQ (10)` earlier in the session: Windows seems to rotate it on its own |
| Apps tab (said beforehand) | The picker listed 196 Start menu and Store apps; Calculator (a Store app) pinned with its icon, launched from the pin, seen in the open apps, closed from there, the pin removed (0 left) |
| Media tab | The product owner's paused video in Opera GX: cover, title (Arabic, right to left), 8:13 of 14:47, the controls the player allows. Nothing was played. The app's name now reads "Opera GX Web Browser", not its Store id |
| Not checked by the agent | Folding on a real click elsewhere (the agent's tools take the focus, so a development-only flag kept the panel open for screenshots), a second monitor's wallpaper, play/pause on a real player |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Click the pill; click elsewhere; Esc | It expands into the panel; it folds back | |
| 2 | Home ▸ Ask Kairos… | The quick box opens under the pill | |
| 3 | Media, with music playing | Cover, title, progress moving; play/pause, next, previous work | |
| 4 | Wallpaper | Next, previous, a click on a thumbnail; with two monitors, one or all | |
| 5 | Apps | Pin two apps, launch them; right-click an open app, then ×: it closes politely | |
| 6 | Leave the Media tab | Nothing keeps listening to the player (it unsubscribes) | |
