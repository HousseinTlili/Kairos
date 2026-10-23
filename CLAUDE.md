# Kairos — instructions for AI coding assistants

Kairos is a desktop app (Tauri 2 + React + TS) and a Hermes plugin (Python) on top of
**Hermes Agent**, which runs unmodified as a separate process. Read `docs/ARCHITECTURE.md` once;
for the current milestone, read its spec in `docs/milestones/`.

## Current milestone

**M1** — `docs/milestones/M1.md`. Do the steps in order, one step per session. Each step ends
with a check; run it and show its output before saying the step is done.

## Roles

The user is the product owner. You are the engineer. Stop and ask before:

- adding a dependency not listed in the milestone spec;
- changing anything in Hermes's configuration or `.env` beyond what the spec says;
- changing an Accepted decision in `docs/DECISIONS.md`;
- anything that deletes data, pushes to git, or touches files outside this repository.

## Hard rules

- **Never modify Hermes.** No edits under `%LOCALAPPDATA%\hermes\hermes-agent\` or any Hermes
  checkout. Kairos uses only the API and plugin contracts in `docs/ARCHITECTURE.md` §3. If you need
  something Hermes doesn't offer, write it down as a question.
- **Secrets.** Never print, log, commit, or put in an error message the `API_SERVER_KEY` or any
  provider key. Read the key from the keyring (or `.env` once, at first launch). Redact
  `Authorization` headers in debug output.
- **No personal data in the repo.** Fixtures are sanitised (`fixtures/README.md`). Never commit
  `/api/sessions` or `/api/model/options` output from the real profile.
- **Fixtures are the truth about Hermes.** If code and a fixture disagree, the code is wrong. If
  live Hermes and a fixture disagree, record a new fixture and tell the user; don't edit the old
  one.
- **Don't claim what you didn't see.** "Tests pass" requires the test output in the same message.
  If a check could not be run, say so plainly.
- **Don't resend work.** Every Hermes run is created with an `Idempotency-Key` stored first; never
  retry a request under a new key (M1 §9 rule 1).
- **Approval mode.** Hermes runs in `smart` (D-009). Any test that switches it to `manual` must
  switch it back to `smart` and say so.

## Commands

| What | Command |
|---|---|
| Dev app | `pnpm -C app tauri dev` |
| Rust tests | `cargo test --manifest-path app/src-tauri/Cargo.toml` |
| Live Hermes tests | `cargo test --manifest-path app/src-tauri/Cargo.toml -- --ignored` (needs Hermes running) |
| TS tests | `pnpm -C app test` |
| Lint | `cargo clippy --manifest-path app/src-tauri/Cargo.toml -- -D warnings`; `pnpm -C app lint` |
| Hermes status | `hermes gateway status`; `curl http://127.0.0.1:8642/health` |
| Hermes setup | `powershell -File scripts/setup.ps1` |

## Code style

- Rust: `thiserror` errors, no `unwrap()` outside tests, one module per concern (`hermes/`,
  `store.rs`, `keyring.rs`).
- TS: feature folders under `app/src/features/`, one Zustand store per feature, types for Hermes
  events in `app/src/lib/hermes-events.ts` kept in step with `events.rs`.
- Tests describe behaviour ("a denied approval marks the tool row blocked"), not implementation.
- Comments explain *why*. Match the surrounding code.

## Commits

Small commits, one per step or sub-step, imperative subject line. Don't push unless asked.
