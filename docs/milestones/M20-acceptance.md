# M20 — Acceptance

Spec: `docs/milestones/M20.md`. Built 2026-11-01 on `m20/computer-use`; D-040 accepted the same day with
the recommended answers.

## Checks run by the agent (2026-11-01)

All in Windows' Character Map, shown without taking focus, on the default model. The switch was on only for these
checks, and the rules were added only for them: `config.yaml` and `policy.json` are back to their exact bytes.

| Check | Result |
|---|---|
| **The key probe** | A click's approval reaches Kairos over `/v1/runs` as an ordinary `approval.request`. One card per action; `smart` approved none by itself |
| Live, the normal flow | From a `kairos-…` session: a Kairos card to see Character Map ("It will see Character Map, and send the picture…"), then Hermes's cards for the click and the typing. The audit row names the app and the element: `kairos_target` Character Map, `kairos_element` Select |
| Live, a conversation grant | After "this conversation" on seeing the app, a second capture asked nothing |
| Live, refusals | `key win+r`: `computer-use-keys`. `capture screen`: `computer-use-whole-screen`. A capture from a session that isn't Kairos's: `kairos-own-tools` |
| Live, Stop | On a click's waiting card: `run.cancelled` |
| Live, the switch | Refused before the rules are there, naming "Add the new rules". On and off through the app: the tool appears and goes, a new run has no such tool with no restart |
| The page and cards | Seen in dark, with test data for the cards: "Click Select in Character Map", "Type “abc” in Character Map" offering only Grant once and Refuse, and no card offering Always |
| Rust tests | 346 + 19 passed. New: "it won't turn on without the rules", "the guard rule must be there and on" |
| TS tests, lint, tsc | Passed. New: "never offers always, for any desktop action", "offers typing and keys only once", "words a card from the audit row of the call before it", "keeps Hermes's text when the audit row hasn't been read" |
| Clippy | clean |
| Plugin tests | Passed, 31 new: "a click on Kairos's own window is refused", "typing into a terminal is refused", "Win+R is refused", "input with no known target is refused", "a click with app capture isn't treated as a capture", "a grant to see one app doesn't cover another", and the rest |
| **Found and fixed** | `kairos.exe` was refused as "no target" while Kairos's window was hidden, not as off limits. It's now refused by name |
| **Seen once, not explained** | One capture of Character Map was refused as "no target" in a run where the same capture had just worked, and it didn't happen again. It failed safe. The window list skips windows on another virtual desktop, so that's a possible cause; not confirmed |
| **Not seen** | The bar's line and Stop drawn on the bar; the sidebar's "Asks your leave"; the cards in a real conversation, since these runs went straight to the API; the page in light themes; a capture with pictures (`som`), which the plugin reads leniently; a real app with a menu or a dialog (checklist) |
| **Caveat on these checks** | They ran in the agent's test setup, where the dev app and Hermes were started from the Claude app, whose AppData overlay may hold what they write. Repeat them from an ordinary terminal |

## Worth knowing

- **The first probe sent your open windows' titles to the model.** `list_windows` returns every title, and the
  spec had it free. That included one Opera tab's title. It now asks. The conversation was deleted from Hermes; two rows of the
  local audit log (`audit.db`) still hold it, as that log keeps what tools returned.
- **The first switch off removes 236 lines of comments from `config.yaml`:** the commented-out "Gateway Platform
  Settings" block, which Hermes's writer ties to that list. No setting changes, and the same text stays in Hermes's
  `cli-config.yaml.example`. Hermes's own `hermes tools` does the same. A list of toolsets also stays in the file, so
  a toolset that a later Hermes adds won't appear by itself on this API.
- **The cost:** a five-step task took 130,000 to 200,000 input tokens on the default model, about 22,000 a step.
- **It isn't airtight.** Off limits goes by process names and window titles, and the plugin sees the calls, not
  the driver. A dialog that opens after a click can differ from what it remembers. Every action still asks.
- **Probe conversations:** all eight deleted from Hermes on 2026-11-01, at the product owner's word. Their rows
  remain in the audit log, like any other tool call's.

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Restart Hermes; then Settings ▸ Rules: "Add the new rules", Save. In that order: a plugin older than these rules refuses every tool | The rules are there; other tools still work | |
| 2 | Settings ▸ Safety ▸ Computer use: turn it on | It turns on | |
| 3 | Open Character Map. In a new conversation: "Use computer use to select the letter A in Character Map" | A card to see it, a card per click, in words | |
| 4 | Let it type something | The exact text, only Grant once and Refuse | |
| 5 | While it works, the top bar | "Working in Character Map" and Stop; Stop ends it | |
| 6 | Open another settings page while a card waits | "Asks your leave" in the sidebar | |
| 7 | "Use computer use on Firefox", then on Kairos | Refused, and it tells you why | |
| 8 | Add your bank's app to `computer-use-off-limits` | Asking for it is refused | |
| 9 | Turn it off, new conversation | It has no such tool; `hermes tools list --platform api_server` agrees | |
