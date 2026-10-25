# M10 — Persona: acceptance

**Status (2026-10-26):** Part 1 (persona) built on `m10/persona`; live checks below. Not signed off.

## Checks run on the PC by the agent (2026-10-26)

| Check | Result |
|---|---|
| Rust tests (persona: backup once, restore, no file, starter text) | 5 passed; clippy and fmt clean |
| TS tests, lint, tsc | 122 passed; clean |
| Real folders, read-only | `C:\Users\Houssein\AppData\Local\hermes\SOUL.md` read as Hermes's default; backup planned in `%APPDATA%\kairos\backups\` |
| Save from Settings ▸ Persona (driven through WebView2's debugging port, after the product owner's own Save left nothing written and showed nothing) | Works: label "Hermes's default" → "Kairos persona", "Saved…", `SOUL.md` written, original backed up; a second save kept the backup. The product owner's failed attempt was not reproduced on a freshly started app (likely a page left stale by many hot reloads while building) |
| Live, first wording (free model `upstage/solar-mini4:free`) | "Who are you?" → introduces itself as Kairos; French question answered **in French**; never says "Master" |
| Live, second wording (the two hard rules first, with an example) | French → "Twelve times twelve is 144, Master." Arabic → "The capital of France is Paris." (English, no "Master"). "Who are you?" → Kairos, but also says it is Solar Mini, no "Master" |

**Finding:** English-only now holds; "Master" is used about one reply in three, and the model
still talks about the underlying model. The free model follows the system prompt loosely. Two
ways to make it reliable, the product owner's call: a one-line per-turn reminder from the Kairos
plugin (changes D-023, which said no per-turn injection) or a stronger model (M7's model menu).

**Also seen:** with Voice and Persona added, Settings' tab row is wider than the window, so the
whole page scrolls sideways and its left edge is cut off. To fix.
