# M13 — Acceptance

Spec: `docs/milestones/M13.md`. Built 2026-10-28 on `m13/rich-answers`.

## Checks run by the agent (2026-10-28)

| Check | Result |
|---|---|
| Step 1 probe: *"use image_generate to make a red apple"* | Not available yet: Hermes asks for its image app to be connected on the Nous Portal (`CONNECTION_REQUIRED`). Not connected: the product owner's call |
| Step 1 probe: a screenshot with `MEDIA:` | The model reached for file-system code; each try asked (`browser-code-system-access`) and was refused by the agent. No fixture recorded |
| `mermaid` | 11.17.2 (25 Aug 2026, MIT), pinned exactly; loaded only when an answer holds a diagram |
| TS tests, lint, tsc | 157 passed (19 Markdown reader, including tables, safe images, `MEDIA:` lines, diagrams drawn only once closed; voice announces tables, diagrams and pictures); clean |
| Rust tests, clippy | 201 + 19 passed (4 new: pictures only from Hermes's folders, `..` refused, not over 5 MB, saving never overwrites); clean |
| Plugin tests | 106 passed (the note goes only to `kairos-` conversations) |
| A sample answer in the dev app (dark and light) | The table, a flowchart, a pie chart and an SVG sketch drawn in the charter's colours; light theme readable; the pie capped at 420 px |
| **The ten questions**, live, each in a new conversation | See below: 8 of 10 the right form at first; after sharpening the note, the 2 misses re-run right and drawn on screen |
| A diagram in a conversation reopened from history | Drawn (the population chart) |

### The ten questions

| Question | Wanted | Got |
|---|---|---|
| What's the capital of Australia? | plain | plain, one sentence |
| How do I take a screenshot of one window on Windows 11? | a list | numbered steps |
| Compare Python, Rust and Go for a desktop app | a table | a table |
| What happens when I type a web address and press Enter? | a diagram | a flowchart |
| World population 2000–2025: show how it changed | a chart | an `xychart` line chart |
| Thanks, that's all for now. | plain | plain |
| Draw me a simple sketch of a house. | an SVG | **first: saved a file to the Desktop** (`house-sketch.svg`, left in place); **after the fix: an SVG in the answer, shown as a picture** |
| What's 17 times 23? | plain | "Three hundred ninety-one." |
| Battery at 100 % or 20–80 %? | plain | plain paragraphs |
| How is a small team organised (lead, 2 devs, 1 designer)? | a diagram | **first: words only; after the fix: a diagram** |

The fix (2026-10-28): the note now says a drawing goes in the answer itself, never to a file, and
that *how people or parts are organised* is a diagram.

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | *"Compare three laptops under 1 000 € on price, battery and weight"* | A table; Copy pastes into Excel as cells | |
| 2 | *"How does Kairos's wake word work?"* | One diagram; Source shows the code | |
| 3 | A plain question | A plain answer: no table, diagram or picture | |
| 4 | *"Draw me a simple sketch of a cat"* | A small drawing in the answer; click enlarges; Save puts it in Downloads | |
| 5 | A screenshot through Kairos (grant it) | The picture in the answer, and again after reopening the conversation | |
| 6 | Question 1 in the quick box, and once spoken | The table fits (scrolls sideways); spoken: "There's a table on screen" | |
| 7 | Light theme | Tables and diagrams readable | |
| 8 | (Optional) connect Hermes's image app on the Nous Portal, then *"make a picture of a red apple"* | The picture in the answer | |
