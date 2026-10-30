# M17 — Acceptance

Spec: `docs/milestones/M17.md`. Steps 2–7 built 2026-10-30 on `m17/automations`; D-033 accepted the
same day with the recommended answers.

## Checks run by the agent (2026-10-30)

| Check | Result |
|---|---|
| Rust tests (automations) | 24 passed: triggers ("the battery fires only when the level is crossed", "a crash matches its programs with or without .exe"), placeholders, the decision ("an automation in its cooldown is skipped with the reason", "the seventh AI request in an hour is held back but the rest still runs", "quiet hours hold back an automation unless it is allowed through", window, days, power, UTC offset), the store (starters once, order, cooldown and limits counting), snapshots ("unplugging fires once, not every snapshot", Wi-Fi debounced over two snapshots), crashes (the two recorded event-log fixtures, ignore list, 60 s bursts) |
| Clippy (all targets) | clean |
| TS tests, lint, tsc | 201 passed (8 new, with the island line fading: the request keeps the Master's words first and the event quoted as data, and can't be broken out of the quote; descriptions; what can be saved); clean |
| The crash source on the PC | Read without admin rights with `Get-WinEvent`: *Application Error* 1000 and *Application Hang* 1002 present (RuntimeBroker, nvcontainer, nasm, claude.exe). Two of them, sanitised, are the fixtures |
| Live in the dev app (2026-10-30, Hermes 0.22.5) | Settings ▸ Automations shows the three starters (screenshot). **Test now** with notify + say + island: the toast, the voice, the run recorded as a test. A program built to crash (`Environment.FailFast`) → *Application Error* 1000 in the log → within 10 s a toast and the offer card "crashtest.exe 0.0.0.0 crashed (exception 0x80131623). Shall I look into why?" → **Ask Kairos** started a request in a new conversation: two web searches, the cause named (.NET's `FailFast`), nothing changed |
| **Found and fixed live** | An automation's line stayed on the pill after its 10 s (the pill fell back to the newest state): it now goes back to the window's own line (test added). Windows writes the module as `unknown` for a fail-fast exit: it is left out, the exception code kept (test added). The offer card repeated the event under a text that already held it. The event went into the request inside a code fence that the chat showed literally: it is quoted with « » now, which the event can't close (test changed) |
| Not run | Charger, battery, Wi-Fi and login firing for real, and an "Ask Kairos" meeting an asking rule: the product owner's checks below |

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
