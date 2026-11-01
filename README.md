# Kairos

*Kairos* (Greek καιρός): the right, opportune moment.

Kairos is a personal AI assistant for Windows 11: a desktop app and a set of safety rules on top of
[Hermes Agent](https://github.com/NousResearch/hermes-agent) by Nous Research. Hermes does the thinking,
tools, skills and memory, and runs unmodified as its own process. Kairos adds its own interface, its own
rules for what the agent may do, and a record of everything it did.

> **Status: pre-release, in active development.** Not all of it has been signed off on a real PC: see
> [docs/TESTING.md](https://github.com/HousseinTlili/Kairos/blob/m20/computer-use/docs/TESTING.md). Windows only. Built against Hermes Agent 0.22.5.

## What it does

**Talk to it**
- Chat with streaming answers, tables, diagrams and images
- A quick box over any app (Ctrl+Alt+K by default)
- Ctrl+K: a command palette and search across conversations
- Push-to-talk voice with local speech recognition (Whisper) and a local voice (Kokoro). Nothing listens
  unless you hold the key
- A persona of its own, and memory you can see and undo
- A welcome-back briefing when you return to the PC

**Use your PC**
- A top bar that hides behind maximized windows: desktops, clock, volume, battery, and its own Wi-Fi and
  Bluetooth menus
- A panel with wallpaper, media, open and pinned apps
- A system dashboard and alerts
- Opening and closing apps, and capturing the screen when you allow it
- **Computer use:** working in an app in the background without taking your mouse or keyboard. Off by default
- Your notifications in one place, including calls and texts through Phone Link
- Automations on events, such as a program crashing or the charger being unplugged
- A clipboard shelf: text, links, images and files at the edge of the screen
- Themes and patterns

**Reach further**
- Browser control, asking before anything with consequences, such as paying or sending
- Google and GitHub accounts through their official sign-in, read-only by default
- Any AI model: Hermes's providers, your own API keys, OpenAI-compatible endpoints, local Ollama and LM Studio
- Skills from Hermes's registries: read one and see its security scan before you install it
- An MCP server manager: add outside tools from Settings
- Usage charts: tokens, cost and models over time

## How it stays safe

Kairos can act on your PC, so what it may do is decided outside the model:

- **Rules before every tool call.** A plugin checks each call against rules you can read and edit in the app:
  allow, ask or refuse. If the rules can't be read, everything is refused.
- **You see what you approve.** Cards say what will run, in words where Kairos can tell.
- **Some things are always refused.** For example, Kairos can't turn off its own rules, read saved passwords, or
  use computer use on its own window, terminals, password managers or banking.
- **An audit log** on your PC records every decision and tool call.
- **Commands can run in a container** (Docker), with only the folders you choose.
- **Keys stay out of the code.** Kairos's key to Hermes is in the Windows credential store, and provider keys
  stay in Hermes's own settings.

No safety layer is perfect: text on a page or in a window can carry instructions to the AI, so read the cards.
What you ask, and what it sees, goes to the AI provider you choose, unless you use a local model. See
[docs/SECURITY.md](docs/SECURITY.md).

## Getting started

You need Windows 11, [Hermes Agent](https://github.com/NousResearch/hermes-agent), Node with
[pnpm](https://pnpm.io), and Rust. Voice also needs Visual Studio Build Tools 2022, CMake and LLVM, at the paths
in [.cargo/config.toml](https://github.com/HousseinTlili/Kairos/blob/m20/computer-use/.cargo/config.toml). Docker Desktop is optional, for running commands in a container.

```powershell
# The current code is on the latest milestone branch, not on main
git checkout m20/computer-use

# Make Hermes's API ready for Kairos
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1

# Install the Kairos plugin into Hermes (asks first)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-plugin.ps1

# Install the app's dependencies, then run it
pnpm -C app install
pnpm -C app tauri dev
```

There is no installer yet.

## Development

| What | Command |
|---|---|
| Rust tests | `cargo test --manifest-path app/src-tauri/Cargo.toml` |
| TypeScript tests | `pnpm -C app test` |
| Lint | `cargo clippy --manifest-path app/src-tauri/Cargo.toml -- -D warnings` and `pnpm -C app lint` |
| Plugin tests | `& (Get-Item "$env:LOCALAPPDATA\hermes\tools\python-*\python.exe")[0] -I -m unittest discover -s plugin/tests -t plugin` |

While the dev app runs, set `CARGO_TARGET_DIR=target/test-build` for Rust tests: Windows locks the running
`kairos.exe`.

Work goes milestone by milestone, one branch each, stacked on the one before. The newest work is on the latest
`m*/` branch, not on `main`.

```
Kairos/
├── app/        Tauri 2, React and TypeScript desktop app (Rust core in src-tauri/)
├── plugin/     Hermes plugin (Python, standard library only): rules, audit log, PC tools
├── skills/     Kairos's own Hermes skills
├── fixtures/   Recorded Hermes responses, the test suite's truth about Hermes
├── eval/       Evaluation tasks
├── scripts/    Setup and install scripts
└── docs/       Architecture, decisions, security, milestones
```

## Documents

| Document | What it covers |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Components, how Kairos talks to Hermes, who owns what data |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Each design decision, with alternatives and reasons |
| [docs/SECURITY.md](docs/SECURITY.md) | Safety layers, rules, isolation, secrets, prompt injection |
| [docs/ROADMAP.md](docs/ROADMAP.md) | The original milestones and their acceptance criteria |
| [docs/TESTING.md](https://github.com/HousseinTlili/Kairos/blob/m20/computer-use/docs/TESTING.md) | What's still to verify on a real PC |
| [docs/milestones/](https://github.com/HousseinTlili/Kairos/tree/m20/computer-use/docs/milestones) | Each milestone's spec and acceptance record |
| [docs/plans/](https://github.com/HousseinTlili/Kairos/tree/m20/computer-use/docs/plans) | Plans, including what comes next |

## Credits

Built on Hermes Agent, © 2025 Nous Research, MIT License. Computer use goes through Hermes's use of
[Cua Driver](https://github.com/trycua/cua). Voice uses [whisper.cpp](https://github.com/ggml-org/whisper.cpp) and
[Kokoro](https://github.com/hexgrad/kokoro).
