# M1 acceptance record

**Date:** 2026-10-24
**Hermes:** 0.22.5 (commit `bf0203a`), Windows 11, native install, approval mode `manual`
**Model:** `upstage/solar-mini4:free` via Nous Portal
**Kairos:** `main` after M1 Steps 1–9

"Product owner" means the check was done by hand in the running app and confirmed by the product
owner. "Automated" means a test or command whose output was recorded in the session.

## ROADMAP M1 acceptance criteria

| Criterion | Result | How verified |
|---|---|---|
| Ask a question → streamed answer on the free model | Pass | Product owner: "Reply with exactly: Kairos OK" streamed and completed. Automated: live test `a_simple_run_streams_to_completion_and_a_retry_never_runs_twice` |
| A dangerous command → approval card → approve → result; deny → shown as blocked, not success | Pass | Product owner: deleting `kairos-approval-test` showed a card; Deny kept the folder and showed "1 action blocked"; Allow once deleted it (folder confirmed gone) |
| Kill the app mid-run, reopen → the run is found again | Pass | Product owner: a `ping -n 40` run was picked up after closing and reopening Kairos |

## Step checks (M1 §11)

| Step | Check | Result | How verified |
|---|---|---|---|
| 1 Scaffold | Window opens; `cargo test`, `pnpm test` run | Pass | Product owner screenshot; automated |
| 2 Setup script | Second run changes nothing, no restart | Pass | Automated: `.env` checksum, backup count and gateway PID unchanged over two runs; 8/8 offline tests |
| 3 Client + SSE | Every fixture stream parses event by event | Pass | Automated: 17 fixture tests |
| 4 Connect screen | Connected / not running / key rejected → re-read | Pass | Product owner, all three states |
| 5 Chat | Simple answer streams; a tool call shows a tool row | Pass | Product owner; `kairos.db` rows checked (status, run ID, outcome) |
| 6 Approval card | Deny blocks, Allow once runs | Pass | Product owner |
| 7 Stop + reconnect | Stop ends "Stopped"; run found after reopening | Pass | Product owner |
| 8 Session list | Kairos conversations reopen; other Hermes conversations read-only | Pass | Product owner |
| 9 Live tests | Health, capabilities, end-to-end run, idempotent retry, wrong key, unknown run | Pass | Automated: 4/4 `--ignored` tests |

## Definition of done (M1 §12)

| Item | Result |
|---|---|
| `cargo clippy --all-targets -- -D warnings` | 0 findings |
| `cargo fmt --check` | Clean |
| `cargo test` | 58 passed (41 unit, 17 fixture) |
| `cargo test -- --ignored` (live Hermes) | 4 passed |
| `pnpm -C app test` | 26 passed (3 files) |
| `pnpm -C app lint` (ESLint + Prettier), `tsc --noEmit` | Clean |
| `scripts/test-setup.ps1` | 8 passed |
| No secrets or personal data in the repo | Scan clean (the only match is the search pattern in `fixtures/README.md`). One earlier leak, a user name split across fixture deltas, was fixed on 2026-10-24 and remains in the history of commit `13dac20` by the product owner's decision |
| Approval mode | `manual` (D-009) |

## Not covered, or covered only partly

- **React components have no automated tests.** Their logic lives in tested pure modules
  (`turn.ts`, `markdown.ts`, `store.ts` helpers); the screens themselves were checked by hand.
  Component tests need Testing Library and jsdom, which are not approved dependencies yet.
- **Events never seen live:** `run.failed`, `run.interrupted`, `message.interim`,
  `subagent.*`. The code handles their documented shapes and is tested with made-up payloads.
- **Approval choices "Allow for this conversation" and "Always allow"** were not exercised live;
  only Deny and Allow once were.
- **`unknown` runs** (Hermes restarted mid-run) show "Outcome unknown, Kairos won't resend it" but
  have no Acknowledge button calling `resolve-unknown` yet (allowed by M1 §9 rule 7).
- **The clearer "nothing is accepting connections" wording** was added after the not-running check
  and is covered by unit tests only.
- **A tool row's "done"** means Hermes ran the command, not that it achieved the goal (seen with
  `C:\Windows\Temp`, where the command ran but access was denied).

## Hermes issues found during M1

- `GET /v1/skills` fails on 0.22.5: `find_all_skills() got an unexpected keyword argument
  'include_editorial'`. Kairos doesn't use it.
- `runtime.provider` / `runtime.model` are empty strings in run results.
- A gateway started from an agent's shell dies with that shell; Hermes logs it as an unclean exit.
  CLAUDE.md now says to start Hermes from a terminal the user keeps open.

## Sign-off

M1 is accepted when the product owner confirms this record.
