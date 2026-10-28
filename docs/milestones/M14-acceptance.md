# M14 — Acceptance

Spec: `docs/milestones/M14.md`. Built 2026-10-29 on `m14/island`. The product owner's choices:
top-centre, the butler clip for everything, on at start.

## Checks run by the agent (2026-10-29)

| Check | Result |
|---|---|
| The video | `Butler_standing_on_dark_floor_1080p_1.mp4` (1080 × 1920, 8 s) had black bands baked in (150 px top and bottom, ~96 px each side, by `cropdetect`): cropped to 984 × 1620 and re-encoded to 540 px wide H.264 without sound, 553 KB, with Hermes's ffmpeg. The originals stay in `Videos/` (not committed) |
| Rust tests, clippy | 207 + 19 passed (5 new: top-centre at 100 and 150 %, a monitor left of the main one, the quick box under the pill and inside a narrow screen — a real panic fixed there — and which Windows states hide it); clean |
| TS tests, lint, tsc | 165 passed (8 new: what the pill says, an approval first, "Done" fading, merging the windows); clean |
| In the dev app: the four windows | main, quick, island, presence-panel all load; the pill shows at (772, 10), 375 × 50 physical px (300 × 40 at 125 %), "At your service" |
| The wake word with the panel (George's "Hey Kairos, what time is it?" fed to `wake_feed`) | Woke at 1.8 s; the quick box opened under the pill; the panel slid in with the video playing; caption "Speaking…", *"what time is it?"*, and the answer ("Right now it is 12:50 in the morning…"); it slid away by itself after the answer |
| Hide for an hour (tried with 2 minutes) | Hidden at once; back by itself when the time was up |
| CPU, Kairos + every WebView2 process on the PC, 10 s samples at rest | Pill showing 15.5, 15.9, 19.8 %; pill hidden 20.5, 21.1 %: no difference the pill can be blamed for (most of it is the always-on wake word and other apps' WebView2). The pill alone could not be isolated to prove "under 1 % of a core" |
| Not checked by the agent | The see-through corners on the real desktop (the agent's screen access masks Kairos's dev build), full-screen hiding, a second screen and other scalings: the product owner's checklist |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Look at the pill on a light and a dark wallpaper | Round corners see-through, text readable | |
| 2 | Click it; hold its microphone | The quick box under it; push-to-talk | |
| 3 | Right-click it | Open Kairos, Hide for an hour, Island settings | |
| 4 | Ask something in the main window; then something that needs your leave | The pill shows the request, then "Done"; "Kairos asks your leave", a click opens the card | |
| 5 | Say "Kairos, what time is it?" | Your standing Kairos slides in on the right, then away | |
| 6 | Start a full-screen game or film | The pill steps aside, and returns after | |
| 7 | 150 % scaling, or a second screen | Still top-centre of the main screen | |
| 8 | Settings ▸ Kairos ▸ Island ▸ off | Hidden at once; on again shows it | |
