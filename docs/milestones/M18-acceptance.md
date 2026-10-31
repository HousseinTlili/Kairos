# M18 — Acceptance

Spec: `docs/milestones/M18.md`. Steps 2–3 built 2026-10-31 on `m18/providers`; D-038 accepted the
same day.

## Checks run by the agent (2026-10-31)

| Check | Result |
|---|---|
| Rust tests (providers) | 10 passed: "a Kairos key goes at the end under its marker and everything else stays", "setting a key again replaces it and its marker once", Windows line endings kept, "removing takes only Kairos's own line" (a key set up by hand is never removed), markers read back, keys a `.env` reader could cut refused, Copilot's token kinds, endpoint names, "the endpoint entry names its key and never holds it", http(s) addresses only |
| TS tests, lint, tsc | 208 passed (3 new); clean |
| Clippy | clean |
| Live, own endpoint | `Kairos Test` at a dead local address with model `test-model`: `providers.kairos-test` in Hermes's config, Hermes's model list showing it with `test-model` **at once, no restart**; Remove: gone from the list and the config, no Kairos line left in `.env`. Left behind: an empty `providers: {}` in `config.yaml` (it had no `providers` section before) |
| Live, known provider | NovitaAI with an obviously fake key: listed as having a key, by Kairos; removed: no `NOVITA_API_KEY` or marker line left. A key with spaces and a classic `ghp_` token for Copilot refused with plain messages; no error message holds a key |
| The page | Settings ▸ Accounts and models ▸ AI providers: Hermes's providers (Nous Portal 64 models, Anthropic 17, GitHub Copilot ACP 17), the three ways to add one (screenshot) |
| **Found and fixed** | Vite served a stale `Settings.tsx` after a scripted edit: the page went blank (`ProvidersSettings is not defined`) until the file was touched with an editor |
| **Caveat on these checks** | They ran in the agent's test setup, where the dev app and Hermes were started from the Claude app: Windows gives such processes their own copy of AppData files they change (checked: a file made there isn't visible from outside). Kairos and Hermes started normally don't have that layer, so the product owner's own run below is the real check |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ AI providers ▸ A known provider: pick one you have a key for, paste it, Save | "Saved", then its models in the model menu (or "Restart Hermes", then they appear) | |
| 2 | A conversation on one of those models | It answers; the line under the answer names that provider and model | |
| 3 | Your own endpoint (e.g. Groq from "Start from", with a Groq key) | Its models in the menu | |
| 4 | Remove each | Gone from the menu; `.env` keeps everything you set up yourself | |
| 5 | (If you choose) GitHub Copilot with a fine-grained token (Copilot Requests permission) | Copilot's models in the menu; mind the student plan's allowance | |
