# M11 — Memory: acceptance

**Status (2026-10-27):** built on `m10/persona`; checks run by the agent below. Not signed off.

## Checks run on the PC by the agent (2026-10-27)

| Check | Result |
|---|---|
| holographic installed and enabled | `hermes plugins install holographic --force --yes-deps --enable`: 0.18.3, numpy and snowballstemmer added to Hermes's environment, `memory.provider: holographic` set; `config.yaml` differs from the backup by that one line; gateway restarted |
| Rust tests (`memory`) | 4 passed: entries split on `§`, missing files and store, use counted as Hermes counts, facts read newest first without writing; clippy clean |
| TS tests, lint, tsc | 135 passed (5 new); clean |
| Persona line saved through Settings ▸ Persona | "Saved"; `SOUL.md` has the line; the backup still holds Hermes's original |
| Live: "Please remember this about me…" (new conversation) | "Done, Master. I've saved two things for next time…"; `USER.md` got two entries ("Prefers short answers", "Main project is a desktop assistant called Kairos"). Hermes used its built-in profile, not the fact store, for these |
| Settings ▸ Memory | Shows both under About you, "73 of 1,375 characters (5 %)", provider "holographic", no facts yet |
| Forget (two clicks, through the real page) | "Prefers short answers" gone after 8 s, the other entry kept; no error, no stall; the Forget conversation stays out of the sidebar |

The two entries were the agent's test; the remaining one is true and left in place.

**Also fixed:** `SealMark.tsx` exported a hook next to the component, which stops Vite's fast refresh
for it (a likely cause of the stale pages seen on 2026-10-26); the presence hook moved to
`shell/presence.ts`.

## Product owner's checklist

See `docs/TESTING.md` ▸ M11.
