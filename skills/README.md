# Kairos skills

Hermes skills for the M5 use cases (`docs/milestones/M5.md` §3), in Hermes's `SKILL.md` format.
`scripts/install-skills.ps1` links `%HERMES_HOME%\skills\kairos` to `skills/kairos/` here (a
directory junction, so edits apply to the next conversation). Each skill is also a slash command
in Hermes (`/kairos-code-review`).

The starter rule `kairos-skills` asks before the agent edits these files, and the global rules
(pushes, GitHub posts, hand-offs, deletions) apply while a skill runs: Kairos rules are global,
not per skill.
