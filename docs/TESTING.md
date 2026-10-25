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

Details: `docs/milestones/M5-acceptance.md` (written when M5's steps are done).

- [ ] (filled in as M5 is built)
