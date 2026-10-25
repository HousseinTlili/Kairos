# What the product owner needs to verify

Everything built since M2 that has **not** been seen working on the PC, in the order to test it.
Each milestone has its own detailed checklist in `docs/milestones/M*-acceptance.md`; this page is
the overview and the order. Tick items off there, not here.

**Branches are stacked:** `m3/review` → `m4/notifications` → `m5/briefing` → … Each starts from the
previous one, so testing a later branch also runs everything before it. Test in order, and sign
each milestone off before moving on: a fix found while testing M3 goes on `m3/review` and is
carried forward.

**Before each session:** `git checkout <branch>`, `rustup update` if Rust is older than 1.95,
restart the Hermes gateway (plugin code changed), and in Settings ▸ Rules press "Add the new
rules" if it is offered, then Save.

## Decisions to confirm (made on your behalf, 2026-10-25)

You asked me to continue with my best judgment. These were your calls; each is reversible:

| Decision | Where | If you say no |
|---|---|---|
| `windows` crate added (M4 notification listener) | `docs/milestones/M4.md` §4 | Step 4 of M4 is removed; no notification reading |
| D-018: "anything I missed?" sends notification senders and previews to the AI model when you ask | `docs/DECISIONS.md` | M4 Step 6 is removed |
| M5 briefing defaults: on, quiet hours 22:00–07:00, after 60 min away, AI summary on | `docs/milestones/M5.md` §2 | Changed in Settings ▸ Briefing, or new defaults |
| M8: quick box on Ctrl+Alt+Space (not Alt+Space, which Windows uses), one new dependency (`tauri-plugin-global-shortcut`); screenshots ask every time (D-021) | `docs/milestones/M8.md` §2 | Pick another hotkey in Settings, or I remove the quick box |
| M7: the agent's commands in Docker, one container per conversation, your project folders mounted (D-019); cost estimated from Hermes's prices, cap off by default (D-020) | `docs/milestones/M7.md` §2 | Skip `setup-container.ps1`; leave the cap off |
| M6: Google and GitHub through Hermes's skill and `gh`, no Kairos-side sign-in; Outlook left out (no passwordless route in Hermes 0.22.5) | `docs/milestones/M6.md` §1 | Tell me which account matters and I'll look for its official sign-in route |

## M3 — System dashboard and PC control (`m3/review`)

Details: `docs/milestones/M3-acceptance.md` ▸ "Final checklist". About 30 minutes.

- [ ] **Speed test** on the System page; figures close to speed.cloudflare.com.
- [ ] **Resume from sleep** (Modern Standby, on battery): Settings ▸ Startup shows
      "PC woke from sleep (slept N min)" and `kairos.db` has a `resume` row. If not, tell me: the
      fallback (Windows power notifications) is designed but not built.
- [ ] **Reboot with autostart:** release build, `scripts/install-gateway-autostart.ps1`, reboot;
      Kairos in the tray and Hermes answering `/health` with nobody starting them.
- [ ] Then sign M3 off.

## M4 — Notifications hub (`m4/notifications`)

Details: `docs/milestones/M4-acceptance.md`.

- [ ] **The probe first:** `powershell -NoProfile -ExecutionPolicy Bypass -File
      scripts/probe-notifications.ps1` in Windows PowerShell 5.1. Only `Readable` lets the rest
      work; anything else, send me the output.
- [ ] Settings ▸ Notifications on; Windows allows access.
- [ ] A Discord message or test toast appears in Kairos within 5 s; the badge counts it.
- [ ] Switch off: no new ones collected; switching on doesn't import old ones.
- [ ] Mark read, Clear (asks first).
- [ ] "Anything I missed?" uses `recent_notifications` and summarises.
- [ ] Then sign M4 off.

## M5 — Use cases and the welcome-back briefing (`m5/briefing`)

Details: `docs/milestones/M5-acceptance.md`. About 45 minutes, most of it the evaluation run.

- [ ] Settings ▸ Rules ▸ Add the new rules (`kairos-skills`); run `scripts/install-skills.ps1`.
- [ ] Settings ▸ Briefing ▸ **Brief me now**: card, toast, then a short "Welcome back" chat.
- [ ] Lock for longer than the minimum (set it to 1 min for the test), unlock: a briefing that
      says how long you were away; a second unlock within 10 minutes gives none.
- [ ] Quiet hours around now: no briefing.
- [ ] `/kairos-code-review` on a real branch: findings, nothing changed.
- [ ] **Evaluation run** (`eval/README.md`): 12 tasks; the four safety tasks must pass.
- [ ] Then sign M5 off.

## M6 — Accounts (`m6/accounts`)

Details: `docs/milestones/M6-acceptance.md`. About 45 minutes, half of it the one-time Google
Cloud setup.

- [ ] **You first, once:** a Google Cloud project with the Gmail and Calendar APIs, yourself as
      test user, and a "Desktop app" OAuth client (steps in the acceptance file). Install `gh` if
      missing.
- [ ] Settings ▸ Rules ▸ Add the new rules (three account rules).
- [ ] Settings ▸ Accounts ▸ Connect GitHub, then Google; both show Connected.
- [ ] Unread count: no prompt. "Reply to…": the approval card shows the reply; deny.
- [ ] The Google token file is blocked.
- [ ] Brief me now: today's events and unread count in the AI summary.
- [ ] Disconnect both (asks first); access is gone.
- [ ] Then sign M6 off.

## M7 — Isolation, models and cost (`m7/isolation-models`)

Details: `docs/milestones/M7-acceptance.md`. About 40 minutes. Run the M5 evaluation **before**
switching to the container.

- [ ] Settings ▸ Rules ▸ Add the new rules (`no-sandbox-changes`).
- [ ] The model menu lists your providers with prices; a chosen model shows as the served model
      and is remembered per conversation.
- [ ] Tokens and "≈ $…" under each answer; total in the header.
- [ ] Cost cap `0.0001` on a paid model: stopped mid-run? (If not, say so: it tells us how Hermes
      reports usage.)
- [ ] `scripts/setup-container.ps1`; restart the gateway; `uname -a` says Linux; only your chosen
      folder is visible; switching the backend back is blocked; `-Undo` works.
- [ ] Then sign M7 off.

## M8 — Always-there assistant (`m8/always-there`)

Details: `docs/milestones/M8-acceptance.md`. About 30 minutes. `pnpm -C app install` first (one
new dependency).

- [ ] Settings ▸ Rules ▸ Add the new rules (`screen-capture`, `scheduled-jobs`,
      `scheduled-jobs-cli`).
- [ ] Ctrl+Alt+Space over another app opens the quick box; a question gets an answer; Esc and
      clicking away hide it; "Open in Kairos" works; the hotkey changes in Settings ▸ Quick box.
- [ ] "What's on my screen?" asks first; allowed, the box hides and the answer describes the
      screen; denied, nothing is captured; only 20 screenshots are kept.
- [ ] Scheduled page: create "every 2m: say hello", see it run, pause, resume, run now, delete
      (asks first).
- [ ] In a chat, asking the agent to schedule something asks first (`scheduled-jobs`).
- [ ] Then sign M8 off.
