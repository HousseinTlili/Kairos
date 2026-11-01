# M19 — Acceptance

Spec: `docs/milestones/M19.md`. Built 2026-11-01 on `m19/skills`; D-039 accepted the same day with the
recommended answers.

## Checks run by the agent (2026-11-01)

| Check | Result |
|---|---|
| Probe: scan without installing | An official and a community skill, stdin closed: the report printed, "Installation cancelled", exit 0, nothing installed, no quarantine left. **Works as read from the source** |
| Probe: one community skill | `skills-sh/resciencelab/opc-skills/reddit`: Safe, allowed; quarantined, scanned, removed, never installed or run |
| Probe: install, uninstall, check, audit | With `official/social-media/reddit-reading`; recorded in `fixtures/hermes-0.22.5/skills/` |
| Probe: `skills.disabled` | Written with `config set` (which warns it's unrecognised); `skills list` then says `disabled`; unset afterwards, as it was |
| Rust tests | 338 + 19 passed, 25 of them new: "`--force` can't be built by any command", "an identifier starting with a dash is refused", "a table with long names reads back whole", "a blocked scan gives no way to install", "a skill's name from a registry can't reach an option", "a skill's text can't pose as the decision" |
| TS tests, lint, tsc | 222 passed (10 new, including "shows HTML and image links as plain text"); clean |
| Clippy | clean |
| Plugin tests | 111 passed, with the two new rules: `hermes skills … --force` and `update` denied; install, remove and switching ask |
| Live, the page | Driven through the debug port: 51 skills in 9 folding groups; a search for "reddit" with Official first; `reddit-reading`'s page with no Install before the scan; the scan's Dangerous verdict with its 2 findings and why Hermes allows it; Install; the row with Remove and its switch; off (Hermes says `disabled`) and on (setting gone); Check for updates; Re-scan, with no decision shown; Remove |
| Live, update | `skills_update` called directly: removed and installed again through the scan. No skill had a real update to offer, so the Update button itself wasn't clicked |
| Live, refusals | `--force`, `-x`, a URL and `a/../b` as identifiers: refused before Hermes runs. Turning off a Kairos skill: refused. Removing a built-in: refused by Hermes, in Kairos's words |
| Screenshots | Dark (`midnight`) and light (`charter`, applied in the page only, not saved) |
| State afterwards | 0 hub skills, 51 built-in, `skills.disabled` unset, quarantine empty |
| **Found and fixed** | `run_cli` read a command's output only after it ended, so one longer than the pipe's buffer (the 51-row list) would have hung until the time limit. It now reads as it runs |
| Probe and live, a refusal | With the product owner's go-ahead, up to three more community skills, stopping at the first refused: the first was. `…/analyzing-persistence-mechanisms-in-linux`: Dangerous, 48 findings, exit 1, nothing installed, no quarantine left. Through the app's `skills_scan`: `passed` false, 48 findings, no advice to force it. Recorded as `preview-blocked.txt` |
| **Not seen** | That refusal drawn on the page: the product owner was using the window, so the agent stopped driving it (checklist 5). A Caution verdict refused: its fixture is still written from the source |
| **Not seen** | A conversation using an installed skill; whether an open conversation keeps its old skill list (read from the source). Both spend your model, so they're yours (checklists 3 and 4) |
| **Caveat on these checks** | They ran in the agent's test setup, where the dev app and Hermes were started from the Claude app, whose AppData overlay may hold what they write. Repeat them from an ordinary terminal |

## What Hermes turned out to do, which you may want to know

- **You can read only the first 50 lines** of a skill before installing it: that's all Hermes shows.
- **What you read can be missing text.** Hermes prints the skill through a formatting library that
  removes `[anything like this]`: `before [run this hidden command] after` printed as `before  after`.
  The page says so. The scan reads the real files, and the source's address is shown to read in full.
- **The scan goes by keywords.** The skill it refused is a defensive one, for finding persistence on
  Linux; it was flagged for describing what it looks for (48 findings, some in its description). So
  a refusal doesn't prove a skill is malicious, nor does a pass prove it's safe.
- **`reddit-reading`, an official skill, scans Dangerous,** because it reads its own key from `.env`.
  Hermes allows it because it ships with Hermes. The page shows both rather than hiding either.
- **Hermes's Re-scan judges every hub skill as community,** so it says BLOCKED about skills it just
  let you install. Kairos shows the findings without that decision.
- **The model's terminal could already run `hermes skills install --force`.** Two new rules stop that.
  They reach your rules through Settings ▸ Rules, "Add the new rules".

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ Rules: "Add the new rules", Save | `no-forced-skills` and `skills-changes` are there | |
| 2 | Settings ▸ Kairos ▸ Skills ▸ Find new: search "reddit", open `reddit-reading`, Scan it, Install | No Install before the scan; "Installed." It's listed under Social media, as Hub | |
| 3 | In a new conversation: "What's on r/LocalLLaMA?" | It uses the skill; any command still follows your rules | |
| 4 | Turn it off with its switch; new conversation, same question. Then, with a conversation left open, turn it on or off and ask there | Off: it doesn't use the skill. Note what the open one does: the wording on the page says it may not notice | |
| 5 | Find new: search "persistence mechanisms linux", open `analyzing-persistence-mechanisms-in-linux`, Scan it | The reason in Kairos's words, 48 findings, no Install, no mention of `--force` | |
| 6 | Remove `reddit-reading` | Asks first, then gone from the list | |
| 7 | With a Hub skill installed: Check for updates, Re-scan | "Up to date"; the findings, no decision | |
