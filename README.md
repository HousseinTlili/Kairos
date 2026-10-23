# Kairos

*Kairos* (Greek καιρός): the right, opportune moment.

Kairos is a personal AI assistant: a desktop app and a policy layer built on top of
[Hermes Agent](https://github.com/NousResearch/hermes-agent) by Nous Research. Hermes does the
thinking, tools, skills and memory. Kairos adds its own interface, its own rules for what the
agent may do, and a clear record of everything it did.

**Status:** Phase 0, architecture only. No application code yet.

## Documents

| Document | What it covers |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Components, the Hermes integration contract, data ownership, task lifecycle |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Each design decision with alternatives and reasons; open questions |
| [docs/SECURITY.md](docs/SECURITY.md) | Safety layers, policy file, isolation, secrets, prompt injection |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones with deliverables and acceptance criteria |

## Planned layout

```
Kairos/
├── app/       # Tauri 2 + React + TypeScript desktop app
├── plugin/    # Hermes plugin (Python): policy + audit
├── scripts/   # setup and dev tooling
└── docs/
```

## Credits

Built on Hermes Agent, © 2025 Nous Research, MIT License.
