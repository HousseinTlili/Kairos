# Kairos Roadmap

Each milestone is small, testable, and leaves Kairos usable. A milestone is done only when its
acceptance criteria pass **with evidence** (test output or a recorded demo), not when the code
compiles.

---

## M0 — Phase 0: architecture (this document set)

- **Deliverables:** `ARCHITECTURE.md`, `DECISIONS.md`, `SECURITY.md`, `ROADMAP.md`.
- **Done when:** the product owner approves the Proposed decisions and answers Q-1 to Q-3.

## M1 — Walking skeleton: talk to Hermes

**Status:** accepted on 2026-10-24 (`docs/milestones/M1-acceptance.md`). It grew into a full desktop layout (sidebar, settings,
Markdown answers) at the product owner's request.

The thinnest end-to-end path. No Kairos policy yet; Hermes approval mode is `manual` (D-009).

- **Deliverables**
  - Repo layout: `app/` (Tauri 2 + React + TS), `plugin/` (Python), `docs/`, `scripts/`.
  - `scripts/setup`: enables the Hermes API server on `127.0.0.1`, generates `API_SERVER_KEY`,
    stores it in the OS keyring, checks `GET /health` and `GET /v1/capabilities`.
  - App: connect screen (shows Hermes version, model, health), one chat view using `/v1/runs`
    with live tokens and tool events, a stop button, a session list from `/api/sessions`.
  - Approval card: renders `approval.request`, answers via `/v1/runs/{id}/approval`.
- **Acceptance**
  - Ask a question → streamed answer on the free model.
  - Ask for a command Hermes classes as dangerous (e.g. recursively deleting a scratch folder)
    → approval card appears → approve → result shown; deny → shown as blocked, not as
    success.
  - Kill the app mid-run, reopen → the run is found again via `GET /v1/runs/{id}`.
- **Tests:** Rust client unit tests against a recorded SSE fixture; one integration test against a
  real local Hermes on a temporary `HERMES_HOME`.
- **Risks:** SSE parsing edge cases (keepalive comments, named events); Windows keyring access
  from Tauri.

## M2 — Kairos rules and audit timeline

**Status:** accepted on 2026-10-25 (`docs/milestones/M2-acceptance.md`).

Kairos's own safety layer, before Kairos is given any more power (D-004). Spec:
`docs/milestones/M2.md`.

- **Deliverables**
  - The `kairos` Hermes plugin (Python, `plugin/kairos/`): a `pre_tool_call` policy check on every
    tool call, `post_tool_call` and approval observers writing an audit log.
  - `policy.yaml` v1 with validation; starter rules for the use cases below, including the rule
    families M3–M6 will need (app control, account actions, outbound messages).
  - `kairos.db` gains `tool_event` and `approval` tables (ARCHITECTURE.md §4).
  - App: a timeline per request (every action, the rule that decided it, the result) and a rules
    page in Settings.
- **Acceptance**
  - Reading `~/.ssh/id_rsa` is blocked by rule `no-credentials`, visible in the timeline.
  - A malformed `policy.yaml` blocks every tool call (fail closed) and the app says why.
  - An instruction injected into a fetched web page can't make an `ask` action run without a click.
  - With approval mode briefly set to `smart`, the recorded `rm -rf`
    (`fixtures/hermes-0.22.5/run-smart-autoapprove/`) asks first because of a Kairos rule. Only
    then does the product owner decide whether to return to `smart` (D-009).

## M3 — System dashboard and PC control

The Jarvis layer, part one (D-012, D-013, D-014).

- **System page**, native and with no AI: CPU (total, per core), RAM, disks (usage, read/write
  speed, free space per drive), GPU (load, VRAM, temperature via `nvidia-smi`), network up/down,
  Wi-Fi network and signal, battery (level, charging, time left, wear), uptime, top processes by
  CPU and RAM, and an on-demand internet speed test.
- **PC control tools** registered by the Kairos plugin: `open_app`, `close_app`,
  `list_running_apps`; later window focus, volume, brightness, media, lock and sleep. Opening an
  allowed app runs; closing an app asks; system processes are always denied.
- **Proactive alerts:** low battery, a nearly full disk, a process hogging CPU (with a "close it?"
  that goes through the same rule).
- **Starts with Windows:** Kairos in the system tray; Hermes's login auto-start
  (`hermes gateway install`). Kairos notices startup, wake from sleep and unlock.
- **Acceptance:** the system page matches Task Manager within a few percent; "open Notepad" runs,
  "close Notepad" asks first; after a reboot both Kairos and Hermes are running without anyone
  starting them.

## M4 — Notifications hub

- Reads Windows notifications through the notification-listener API, with the user's permission
  (D-011): missed calls and texts via Phone Link, Discord, Teams, WhatsApp desktop, Outlook and
  anything else that shows a toast. No account passwords.
- Unread counts per app, a notifications panel, and "anything I missed?" answered from it.
- **Acceptance:** a test toast and a real Discord message appear in Kairos within seconds; nothing
  is collected before permission is granted, and turning it off stops collection.

## M5 — Use cases and the welcome-back briefing

The product owner's first use cases (Q-2), each as skills plus rules, with an evaluation set.

1. **Welcome-back briefing** (the headline feature). On startup, wake or unlock, after a
   configurable time away and outside quiet hours: instant facts with no AI (battery, Wi-Fi,
   missed calls and messages, system warnings, finished or waiting tasks), then a short AI summary
   of what matters (repos, calendar and email once connected) with suggestions.
2. **Debug-and-fix:** reproduce an error, find the root cause, propose or apply a fix; delegate
   large changes to Claude Code or Codex only with approval.
3. **Code review:** review a branch or PR for bugs, security problems and missing tests;
   read-only, asks before posting to GitHub.
4. **Documents to actions:** summarise PDFs, Word and Excel files and extract action items; writes
   only to a Kairos output folder.

- **Evaluation:** 10–20 repeatable tasks with checkable outcomes (a planted bug the review must
  find, a failing test the briefing must mention), run by a script, results stored.

## M6 — Accounts

- Accounts connected one at a time through their official sign-in (OAuth, or the provider's own
  CLI login), never with passwords given to Kairos (D-010): Gmail or Outlook, Google Calendar,
  GitHub, then others on request.
- Read-only by default. Sending, deleting, paying or posting always asks. Every account can be
  disconnected from Settings.
- **Acceptance:** the briefing lists today's calendar and unread email counts; "reply to X" drafts
  and asks before sending; disconnecting an account removes Kairos's access to it.

## M7 — Isolation, models and cost

- Commands run inside a container (Docker Desktop with WSL2) with the workspace mounted.
- Model choice per conversation, a local Ollama model for private or offline work, Hermes fallback
  providers, usage and cost per task, and a per-task cost cap.

## M8 — Always-there assistant

- A global hotkey (for example Alt+Space) opens a small Kairos box over any app.
- "What's on my screen?": a screenshot plus vision to explain an error dialog or a page.
- Scheduled jobs from the app via `/api/jobs`, all under the same rules.

## M9 — Voice

- Speech in and out (Hermes voice/TTS, or local Whisper and Piper); a wake word ("Kairos…");
  the welcome-back briefing read aloud on request. Test English, French and Arabic first.

## M10 — Persona and avatar (last)

- The Kairos persona via Hermes's personality / `SOUL.md`.
- The animated avatar driven by run events (`idle`, `listening`, `thinking`, `speaking`).
