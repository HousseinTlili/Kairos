# M5 acceptance record

**Status: built (2026-10-25), nothing run on Windows or against Hermes yet.** Steps 1–7 are on
`m5/briefing` (which starts from `m4/notifications`). Test after M3 and M4 are signed off.

## Before testing

1. `git checkout m5/briefing`; restart the Hermes gateway.
2. Settings ▸ Rules ▸ **Add the new rules** (`kairos-skills`), then Save.
3. Install the skills (asks first; adds a link in Hermes's skills folder, no configuration change):
   `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-skills.ps1`

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 137 unit and 19 fixture tests passed (new: time away, briefing rules, facts, settings, clock, task summary, drive label) |
| `cargo clippy -D warnings` (Linux, and `x86_64-pc-windows-gnu`), `cargo fmt --check` | Clean |
| `pnpm -C app test`, lint, `tsc --noEmit` | 77 passed; clean (new: the summary request, when it may start, quiet-hours inputs) |
| Plugin tests (Python 3.13) | 89 passed (new: `kairos-skills` cases, the skills' format) |
| Evaluation harness tests (`eval/tests`) | 14 passed, with a fake Hermes |
| App under a virtual display | On first start, the startup trigger waited for the page's clock, then briefed ("first return of the day"); the card showed above the chat. Found and fixed: a root-mounted drive was named as an empty string (Linux only) |
| `scripts/install-skills.ps1`, `eval/run.py` against Hermes | **Not run** (no Windows, no Hermes) |

## On the PC

| Step | Check | Result |
|---|---|---|
| 5 | Settings ▸ Briefing ▸ **Brief me now**: the card appears above the chat with the facts (battery, Wi-Fi, unread counts…) and a "Welcome back" toast | |
| 5 | With AI summary on and Hermes connected, a "Welcome back" conversation starts by itself: at most five lines, no action taken | |
| 5 | Set "after at least" to 1 minute, lock (Win+L) for 2 minutes, unlock: a briefing with "You were away 2 min" | |
| 5 | Lock and unlock again within 10 minutes: no second briefing | |
| 5 | Set quiet hours around the current time, lock 2 minutes, unlock: no briefing | |
| 5 | Restart Kairos after more than the minimum time away: one briefing at startup (after the window has loaded) | |
| 5 | Sidebar ▸ Briefing reopens the latest card after Dismiss | |
| 6 | `/kairos-code-review` on a real branch: findings with file, line, failing case and fix; `git status` unchanged | |
| 6 | Ask Kairos to edit `skills\kairos\kairos-code-review\SKILL.md`: `kairos-skills` asks | |
| 7 | `eval/run.py` (see `eval/README.md`): record each task's verdict below | |

### Evaluation results

| Task | Verdict | Notes |
|---|---|---|
| review-off-by-one | | |
| review-hardcoded-secret | | |
| review-asks-before-posting | | |
| debug-names-root-cause | | |
| debug-fixes-when-allowed | | |
| debug-hand-off-asks | | |
| documents-actions-markdown | | |
| documents-word-file | | |
| injection-in-readme | | |
| credentials-blocked | | |
| destructive-asks | | |
| system-change-asks | | |

A FAIL is information, not necessarily a bug: the free model may miss a planted bug. A FAIL on a
**safety** task (the last four) is a rules problem and blocks sign-off.

## Acceptance criteria (ROADMAP M5)

| Criterion | Result |
|---|---|
| A real return to the PC gives a correct briefing (facts right, quiet hours and time away respected) | |
| The three skills work on the evaluation fixtures | |
| The evaluation set runs by script and its results are stored | |

## Known limits

- **Rules are global, not per skill.** "Read-only" (code review) and "writes only to the output
  folder" (documents) are instructions to the model; the global rules make the risky actions ask.
- **Calendar and email** join the briefing in M6.
- **The "Welcome back" toast** from a dev build is collected by M4 as a "Windows PowerShell"
  notification (an installed build is recognised as Kairos's own and skipped).
- **Startup briefing** waits for the window's clock, so with Kairos starting hidden in the tray it
  comes a second or two after login, not before.

## Sign-off

Pending.
