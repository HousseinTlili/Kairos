# M16 — Acceptance

Spec: `docs/milestones/M16.md`. Steps 2–4 built 2026-10-30 on `m16/themes`; Step 5 (the product
owner's pattern images) waits for the images.

## Checks run by the agent (2026-10-30)

| Check | Result |
|---|---|
| The contrast test (WCAG AA) | Every theme, dark and light: text, quiet text, gold, red and green on the room and panels; text on gold buttons; the sidebar, the wordmark, the seal. **Found:** today's Charter light gold on the vellum room was 4.26:1, under 4.5:1; darkened `#86661d` → `#7f601b` (4.67:1) |
| TS tests, lint, tsc | 193 passed (new: the contrast test for all 8 palettes, reading a stored appearance, the title bar's edge colour); clean |
| Rust tests, clippy | 264 + 19 passed (new: a theme colour read from its hex); clean |
| In the dev app | All four windows apply the theme before they paint; the eight themes screenshotted on the Appearance page (room, sidebar, wordmark, seal, pattern, light versions) |
| **Found and fixed** | In a light theme the island's text was dark on its dark pill: the island and the wake panel now take the sidebar's dark-panel colours. The Appearance page didn't follow a change made elsewhere: it listens now. **The island window could stay at its expanded size behind a folded pill** (an invisible window over the top of the screen, taking clicks): the page now folds it on load and Rust sizes it every time it places it |
| Left as found | The appearance put back to Charter, following Windows |

## Product owner's checklist

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ Kairos ▸ Appearance: try each theme | The whole window, the title bar, the island and the quick box change at once | |
| 2 | Light or dark: Light, Dark, Follow Windows (switch Windows's own mode) | Each theme readable in both | |
| 3 | Pattern: each one, and None | Faint, never in the way of text | |
| 4 | An answer with a table and a diagram in another theme | Drawn in that theme's colours | |
