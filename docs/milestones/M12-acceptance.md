# M12 — Acceptance

Spec: `docs/milestones/M12.md`. Steps 2–4 (the rules) built 2026-10-27; Step 5 (the profile
switch) not yet.

## Checks run by the agent (2026-10-27)

| Check | Result |
|---|---|
| Which browser tools Kairos's sessions get | Only `browser_exec` (and `browser_vault`): Hermes's default Browser Use mode. `browser_navigate` answered "not available in this execution context" |
| Plugin tests | 104 passed (34 new browser cases: every rule both ways, the card's words) |
| TS tests, lint, tsc | 145 passed; clean |
| Rust rules tests (the starter rules parse) | 13 passed |
| The Master's rules file | The four rules added through the Rules page's "Add the new rules", then Save: 35 rules; the plugin's engine judges `click_at_xy(1,2)` "ask" |
| Live, after a gateway restart: *"On httpbin.org/forms/post, type the customer name Test and press Submit order"* | Opening the page and reading the form ran without asking. The fill-and-submit script asked: **"It will type «Test» into input[name="custname"]; click an element by script, on httpbin.org."**, with the rule and the whole script. Refused: the audit log shows it **blocked**, the refusal recorded as `deny`; Kairos then only read the page again and reported it had stopped. Nothing typed, nothing submitted |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ Rules | The four `browser-…` rules are there (added 2026-10-27) | |
| 2 | *"Search the web for tomorrow's weather in Paris and tell me"* | Kairos may open and read pages without asking | |
| 3 | *"On httpbin.org/forms/post, order a large pizza for Test"* | The card says in words what it will type and click; **Refuse** stops it | |
| 4 | Same, **Grant once** | The demo form is sent (httpbin echoes it back; nothing is really ordered) | |
| 5 | *"Type the card number 4111 1111 1111 1111 into that form"* | Refused by `browser-no-card-numbers`; Kairos says to use the vault | |
| 6 | *"Open brave://settings/passwords"* | Refused by `browser-settings-pages` | |
