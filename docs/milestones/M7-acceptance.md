# M7 acceptance record

**Status: built (2026-10-25), nothing run on Windows or against Hermes yet.** Steps 1–5 are on
`m7/isolation-models` (which starts from `m6/accounts`). Test after M6 is signed off.

## Before testing

1. `git checkout m7/isolation-models`; restart the Hermes gateway. `kairos.db` moves to schema 4.
2. Settings ▸ Rules ▸ **Add the new rules** (`no-sandbox-changes`), then Save.
3. **Run the M5 evaluation before switching to the container** (its fixtures live in `%TEMP%`,
   which the container won't see).

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 152 unit and 19 fixture tests passed (new: catalogue parsing, prices, cost, cap decision, schema 4, usage storage, the run request body) |
| `cargo clippy -D warnings` (Linux and `x86_64-pc-windows-gnu`), `cargo fmt --check` | Clean |
| `pnpm -C app test`, lint, `tsc --noEmit` | 89 passed; clean (new: picker values and labels, cost labels, conversation total, sandbox wording) |
| Plugin tests | 89 passed (`no-sandbox-changes` cases) |
| The fixture tests with the new `usage` and `runtime` fields | Still pass (the fields default) |
| Docker, `/api/model/options`, a paid model, the cap | **Not run** (no Windows, Docker or Hermes here) |

## On the PC

| Step | Check | Result |
|---|---|---|
| 3 | **Record the shape:** with Hermes running, `curl -H "Authorization: Bearer <key>" http://127.0.0.1:8642/api/model/options` and tell me if `providers[].slug/name/models/pricing` look as described in M7 §4 (don't paste the output: it's your real profile) | |
| 4 | The model menu above the message box lists your providers and models with prices; "Hermes default" is first | |
| 4 | Pick a model, send a request: under the answer, the served model is the one picked; reopen the conversation later: the menu remembers it | |
| 5 | Under each answer: tokens in/out and "≈ $…" (or "free"/"price unknown"); the chat header shows the conversation total | |
| 5 | Settings ▸ Models & sandbox ▸ cost cap `0.0001`, then a long request on a paid model: Kairos stops it and says the cap stopped it. **If it isn't stopped mid-run**, note it: Hermes only reports usage at the end, and the cap can only act on the next request | |
| 2 | `scripts/setup-container.ps1` (Docker Desktop running): pick a project folder; restart the gateway | |
| 2 | Settings ▸ Models & sandbox says "In a container" | |
| 2 | Ask "run `uname -a` and `ls /workspace/projects`": Linux, and only your chosen folder | |
| 2 | Ask it to list `C:\Users\<you>\Documents`: not visible from the container. **Also note** whether `read_file` on a Windows path still works (it would mean Hermes's file tools don't follow the container; the M2 path rules remain their guard) | |
| 2 | Ask "set Hermes's terminal backend back to local": blocked by `no-sandbox-changes` | |
| 2 | `scripts/setup-container.ps1 -Undo`; restart the gateway; Settings says "Directly on this PC" | |

## Acceptance criteria (ROADMAP M7)

| Criterion | Result |
|---|---|
| Commands run inside a container with the workspace mounted | |
| Model choice per conversation; a local model selectable; fallback visible | |
| Usage and cost per task, and a per-task cost cap | |

## Known limits

- **Costs are estimates** from Hermes's price list; cached input is priced at the cache price when
  one exists. Providers may bill differently.
- **The cap may only act between requests** if Hermes fills `usage` only at the end of a run.
- **File tools may not follow the container** (Hermes's docs only promise it for shell commands).
- **Local models** must be downloaded in Hermes Desktop's Local Models first; Kairos doesn't manage
  them.
- **The container setting is global to Hermes**, so Hermes Desktop conversations run in it too.

## Sign-off

Pending.
