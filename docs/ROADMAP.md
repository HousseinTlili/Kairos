# Kairos Roadmap

Each milestone is small, testable, and leaves Kairos usable. A milestone is done only when its
acceptance criteria pass **with evidence** (test output or a recorded demo), not when the code
compiles.

---

## M0 — Phase 0: architecture (this document set)

- **Deliverables:** `ARCHITECTURE.md`, `DECISIONS.md`, `SECURITY.md`, `ROADMAP.md`.
- **Done when:** the product owner approves the Proposed decisions and answers Q-1 to Q-3.

## M1 — Walking skeleton: talk to Hermes

The thinnest end-to-end path. No Kairos policy yet; Hermes approval mode stays `smart`.

- **Deliverables**
  - Repo layout: `app/` (Tauri 2 + React + TS), `plugin/` (Python), `docs/`, `scripts/`.
  - `scripts/setup`: enables the Hermes API server on `127.0.0.1`, generates `API_SERVER_KEY`,
    stores it in the OS keyring, checks `GET /health` and `GET /v1/capabilities`.
  - App: connect screen (shows Hermes version, model, health), one chat view using `/v1/runs`
    with live tokens and tool events, a stop button, a session list from `/api/sessions`.
  - Approval card: renders `approval.request`, answers via `/v1/runs/{id}/approval`.
- **Acceptance**
  - Ask a question → streamed answer on the free model.
  - Ask for a command that `smart` mode escalates to a human (e.g. recursively deleting a scratch
    folder) → approval card appears → approve → result shown; deny → shown as blocked, not as
    success.
  - Kill the app mid-run, reopen → the run is found again via `GET /v1/runs/{id}`.
- **Tests:** Rust client unit tests against a recorded SSE fixture; one integration test against a
  real local Hermes on a temporary `HERMES_HOME`.
- **Risks:** SSE parsing edge cases (keepalive comments, named events); Windows keyring access
  from Tauri.

## M2 — Kairos plugin: policy and audit

- **Deliverables**
  - `plugin/kairos/` (`plugin.yaml`, `__init__.py`): `pre_tool_call` policy, `post_tool_call`,
    approval and session observers.
  - `policy.yaml` format v1 with schema validation; starter rules from `SECURITY.md` §3.
  - `kairos.db` with `task`, `tool_event`, `approval` (`ARCHITECTURE.md` §4) and migrations.
  - App: task timeline per conversation (every tool call, decision, rule, result); policy editor.
- **Acceptance**
  - Asking Kairos to read `~/.ssh/id_rsa` is blocked by rule `no-credentials`, visible in the
    timeline with the rule ID.
  - A malformed `policy.yaml` blocks every tool call (fail closed) and the app says why.
  - An injected instruction inside a fetched web page cannot cause an `ask` tool to run without
    a click.
- **Tests:** policy matcher unit tests (table-driven); a plugin test that raises → call blocked;
  redaction tests with fake secrets.
- **Risks:** argument shapes differ per tool, so path matching must be written per tool family.

## M3 — Isolation and workspaces

- Hermes terminal backend in a container (Docker Desktop on Windows, Docker/Podman on Linux).
- Named workspaces in the app, mapped to `HERMES_WRITE_SAFE_ROOT` / container mounts.
- **Acceptance:** a command that tries to write outside the workspace fails inside the container,
  and the timeline shows it.

## M4 — Models and cost

- Model picker (`/api/model/options`), per-conversation model, Hermes fallback providers.
- Local model via Ollama for private or offline tasks; a cloud model for multi-step work.
- Usage and cost shown per task from run `usage` / `runtime`; a per-task cost cap.
- **Acceptance:** a task that exceeds its cap stops and reports it.

## M5 — Use cases and evaluation

- Implement the 3–5 use cases from Q-2 as skills plus policy rules.
- An eval set: 10–20 repeatable tasks with checkable outcomes, run by a script, results stored,
  to compare models and prompt changes objectively.

## M6 — Scheduled and proactive work

- Scheduled jobs from the app via `/api/jobs`; notifications from the Tauri tray.
- Every scheduled job runs under the same policy; `ask` actions wait for the user.

## M7 — Voice

- Speech in and out using Hermes's voice / TTS features or local Whisper + Piper.
- Test English, French and Arabic before choosing components.

## M8 — Character and avatar (last)

- The Kairos persona via Hermes's personality / `SOUL.md`.
- The animated avatar driven by run events (`idle`, `listening`, `thinking`, `speaking`).
- Only after M1–M6 are reliable.

---

## First implementation task (after approval)

M1, step 1: `scripts/setup` plus the connect screen, proving that Kairos can find, authenticate to,
and identify the local Hermes. Nothing else depends on guesswork after that.
