# M17 — Acceptance

Spec: `docs/milestones/M17.md`. Steps 2–7 built 2026-10-30 on `m17/automations`; D-033 accepted the
same day with the recommended answers.

## Checks run by the agent (2026-10-30)

| Check | Result |
|---|---|
| Rust tests (automations) | 23 passed: triggers ("the battery fires only when the level is crossed", "a crash matches its programs with or without .exe"), placeholders, the decision ("an automation in its cooldown is skipped with the reason", "the seventh AI request in an hour is held back but the rest still runs", "quiet hours hold back an automation unless it is allowed through", window, days, power, UTC offset), the store (starters once, order, cooldown and limits counting), snapshots ("unplugging fires once, not every snapshot", Wi-Fi debounced over two snapshots), crashes (the two recorded event-log fixtures, ignore list, 60 s bursts) |
| Clippy (all targets) | clean |
| TS tests, lint, tsc | 200 passed (7 new: the request keeps the Master's words first and the event fenced as data, and can't be broken out of the fence; descriptions; what can be saved); clean |
| The crash source on the PC | Read without admin rights with `Get-WinEvent`: *Application Error* 1000 and *Application Hang* 1002 present (RuntimeBroker, nvcontainer, nasm, claude.exe). Two of them, sanitised, are the fixtures |
| **Not run** | The app with the new code: the dev app and the gateway were stopped before the live check, so no screenshot of Settings ▸ Automations and no live firing yet. Everything below is the product owner's |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ Kairos ▸ Automations | The three starters; "A program crashed" on, the other two off | |
| 2 | New automation: When the charger is unplugged → Notify "Battery {battery}"; save; unplug | One notification with the level, not one every 15 s; Recent runs shows it | |
| 3 | Test now on an automation with "Say it" and "Show it on the island" | Kairos speaks the text; the pill shows it for about 10 s; the run is marked test | |
| 4 | A program that crashes on purpose (or one that hangs and is closed) | "A program crashed" toast, and a card above the chat; **Ask Kairos** starts a request that explains, without changing anything | |
| 5 | An automation with "Ask Kairos" whose request needs an asking rule (e.g. "close Notepad") | The approval card appears and waits; nothing is approved by itself | |
| 6 | Send a chat request, then fire an "Ask Kairos" automation | It waits for the chat to finish, then starts in a new conversation | |
| 7 | During quiet hours (Settings ▸ Briefing) | An automation is skipped with "quiet hours", unless "Also during quiet hours" is ticked | |
| 8 | Log in with Kairos at startup (an installed build) | A "When I log in" automation runs once | |
| 9 | Switch "Run my automations" off | Nothing runs; Test now still works | |
