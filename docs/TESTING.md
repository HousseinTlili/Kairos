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
| M8: quick box on Ctrl+Alt+K (not Alt+Space, which Windows uses, nor Ctrl+Alt+Space, taken on your PC), one new dependency (`tauri-plugin-global-shortcut`); screenshots ask every time (D-021) | `docs/milestones/M8.md` §2 | Pick another hotkey in Settings, or I remove the quick box |
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
- [ ] Ctrl+Alt+K over another app opens the quick box; a question gets an answer; Esc and
      clicking away hide it; "Open in Kairos" works; the hotkey changes in Settings ▸ Quick box.
- [ ] "What's on my screen?" asks first; allowed, the box hides and the answer describes the
      screen; denied, nothing is captured; only 20 screenshots are kept.
- [ ] Scheduled page: create "every 2m: say hello", see it run, pause, resume, run now, delete
      (asks first).
- [ ] In a chat, asking the agent to schedule something asks first (`scheduled-jobs`).
- [ ] Then sign M8 off.

## M9 — Voice (`m9/voice`)

Spec: `docs/milestones/M9.md`. D-022 and `whisper-rs` approved 2026-10-25. The build needs CMake and
libclang (`.cargo/config.toml` points at VS Build Tools' CMake and `C:\Program Files\LLVM`).

Details: `docs/milestones/M9-acceptance.md`. About 15 minutes. The Small model is already
downloaded (the agent's real-download check).

- [ ] Settings ▸ Voice: Small shows Ready.
- [ ] Mic button: an English, a French and an Arabic sentence each appear correctly and are sent.
- [ ] Hold Ctrl+Alt+J over another app, speak, release: the quick box answers.
- [ ] Esc cancels a recording; silence sends nothing.
- [ ] Speaker button reads an answer aloud; Esc stops it; "Read it to me" on the briefing.
- [ ] Offline (Wi-Fi off), transcription still works.
- [ ] Natural voice (Kokoro, George): answers sound human; Esc stops; removing it falls back to
      the Windows voice (M9 Step 8, rows 14–18 of the acceptance file).
- [ ] Wake word (Step 9, rows 19–24): switch it on, say "Kairos, …" from your seat; the quick box
      answers; "Kairos, open Notepad" in one breath sends all of it. Leave it on for 30 min of
      normal noise: nothing is sent without the name.
- [ ] Then sign M9 off.

## M10 — Persona (`m10/persona`)

Spec: `docs/milestones/M10.md`. About 5 minutes.

- [ ] Settings ▸ Persona ▸ **Use the Kairos persona** ▸ **Save**.
- [ ] New conversation: "Who are you?" — Kairos introduces itself, calls you Master, in English.
- [ ] Ask in French (typed or spoken): the answer is still in English.
- [ ] **Restore the original** (asks first) brings Hermes's default back; then save Kairos again.
- [ ] Then sign the persona off.

## M11 — Memory (`m10/persona`)

Spec: `docs/milestones/M11.md`. About 5 minutes.

- [ ] Tell Kairos something lasting ("I prefer short answers", "my project is called Kairos").
- [ ] Settings ▸ Memory shows it (under About you, Kairos's notes or Facts).
- [ ] Forget it (asks first): it disappears after a moment, and Kairos no longer knows it.
- [ ] The "Forget a memory" conversation doesn't clutter the sidebar.
- [ ] Then sign memory off.


## M12 — Browser rules (`m12/browser`)

Spec: `docs/milestones/M12.md`; details: `docs/milestones/M12-acceptance.md`. About 10 minutes.
Only the rules are built; Kairos still uses its own fresh browser, not your Brave profile.

- [ ] Settings ▸ Rules: the four `browser-…` rules are there.
- [ ] Ask Kairos to look something up on the web: it reads pages without asking.
- [ ] *"On httpbin.org/forms/post, order a large pizza for Test"*: the card says in words what it
      will type and click. Refuse stops it; Grant once sends the demo form.
- [ ] Ask it to type a card number into a page: refused. *"Open brave://settings"*: refused.

## M13 — Rich answers (`m13/rich-answers`)

Spec: `docs/milestones/M13.md`; details: `docs/milestones/M13-acceptance.md`. About 10 minutes.

- [ ] *"Compare three laptops under 1 000 € on price, battery and weight"*: a table; Copy pastes into Excel.
- [ ] *"How does Kairos's wake word work?"*: one diagram; Source shows its code.
- [ ] A plain question: a plain answer, no table or diagram.
- [ ] *"Draw me a simple sketch of a cat"*: a small drawing; click enlarges, Save puts it in Downloads.
- [ ] A screenshot through Kairos (grant it): the picture shows in the answer, and again after reopening the conversation.
- [ ] The same table question in the quick box, and once spoken: it says "There's a table on screen".
- [ ] Light theme: tables and diagrams stay readable.
