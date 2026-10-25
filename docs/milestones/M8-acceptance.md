# M8 acceptance record

**Status: built (2026-10-25), nothing run on Windows or against Hermes yet.** Steps 1–4 are on
`m8/always-there` (which starts from `m7/isolation-models`). Test after M7 is signed off.

## Before testing

1. `git checkout m8/always-there`; `pnpm -C app install` (one new dependency:
   `tauri-plugin-global-shortcut`); restart the Hermes gateway (plugin 0.5.0: new `screen_capture`
   tool).
2. Settings ▸ Rules ▸ **Add the new rules** (`screen-capture`, `scheduled-jobs`,
   `scheduled-jobs-cli`), then Save.
3. Hermes needs a model that can see images, or Hermes's helper vision model set up, for the screen
   check (`vision_analyze`).

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 160 unit and 19 fixture tests passed (new: hotkey parsing, job list parsing, the six `/api/jobs` requests against a local test server — route, method, key in the header only, `deliver: "local"` — and a bad job id refused before sending) |
| `cargo clippy -D warnings` (Linux and `x86_64-pc-windows-gnu`), `cargo fmt --check` | Clean |
| `pnpm -C app test`, lint, `tsc --noEmit` | 102 passed; clean (new: hotkey text, the day's quick conversation, hiding for a capture, next/last run wording, job status wording, Pause/Resume choice, the delete question, the new-job form) |
| Plugin tests | 97 passed (new: the PNG is valid — signature, IHDR, CRCs, pixels decode with `zlib`; only the newest 20 kept; never raises; Windows only; the steering; `screen-capture`, `scheduled-jobs`, `scheduled-jobs-cli` cases) |
| Eval tests | 14 passed |
| Smoke run (Xvfb, no Hermes) | The app starts with the quick window registered; the **Scheduled** page opens from the sidebar, says Kairos isn't connected, and the New job form opens |
| The global hotkey under Xvfb | **Not seen working** (no window manager or key grab in the cloud); a PC check |
| Screen capture, `vision_analyze`, jobs against Hermes | **Not run** (no Windows or Hermes here) |

## On the PC

| Step | Check | Result |
|---|---|---|
| 2 | Over another app, **Ctrl+Alt+Space** opens the quick box; it has the focus | |
| 2 | Ask a question: the answer appears in the box, with the model menu, approval cards and cost line like the main window | |
| 2 | **Esc** hides it; clicking elsewhere hides it; Ctrl+Alt+Space again shows it with the same conversation | |
| 2 | "Open in Kairos" shows that conversation in the main window (titled by its first request) | |
| 2 | Settings ▸ Quick box: change the hotkey (e.g. Ctrl+Shift+K); the old one stops working, the new one works. Pick one Windows already uses (e.g. one taken by another app): the settings say it couldn't be registered | |
| 3 | In the quick box, "what's on my screen?": an approval card (`screen-capture`); allow once; the box hides, comes back, and the answer describes your screen | |
| 3 | Deny it the next time: nothing is captured (no new file in `%APPDATA%\kairos\screens`) | |
| 3 | After 21+ captures, `%APPDATA%\kairos\screens` holds 20 | |
| 3 | On a two-monitor PC, "screenshot both my screens": the picture has both | |
| 4 | Sidebar ▸ **Scheduled** ▸ New job: name "Hello", when "every 2m", what "say hello"; it is listed with "Hasn't run yet" and a next run | |
| 4 | Within ~2 minutes it says "Last run OK" and a last-run time (Refresh, or wait 30 s) | |
| 4 | Pause: "Paused", no next run, no new runs. Resume: a next run again | |
| 4 | Run now: it runs straight away | |
| 4 | Delete…: asks first; Keep leaves it; Delete removes it, and `hermes cron list` no longer shows it | |
| 4 | In a chat, "remind me every day at 9am to stretch": an approval card (`scheduled-jobs`). Deny: no job is created | |
| 4 | **Note for me:** where did the "Hello" job's answers go? (`deliver: "local"` should keep them in Hermes, not post them to a chat platform) | |

## Acceptance criteria (ROADMAP M8)

| Criterion | Result |
|---|---|
| A global hotkey opens a quick box over any app | |
| "What's on my screen?" works, and only after you allow it | |
| Scheduled jobs can be made, paused, resumed, run and deleted from the app, under the rules | |

## Known limits

- **A job step that would ask doesn't run**: nobody is there to answer. Jobs that need such steps
  need a rule set to `allow` for them, which is the user's choice.
- **Job output stays in Hermes** (`deliver: "local"`); Kairos shows the status, not the answer
  text, in this milestone.
- **The screenshot is of the whole display**, not one window, and it is sent to the model's
  provider to be read (like any image in a chat). The rule asks every time.
- **The hotkey can't be one Windows reserves** (Win+…, Alt+Space); Tauri reports those as taken.

## Sign-off

Pending.
