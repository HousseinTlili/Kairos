# What to take from six outside projects

Asked for by the product owner on 2026-11-01: Edge Drop, CUA, GBrain, Skills Hub, Agent Reach and
Hermes Studio. This is a plan, not a spec: each milestone below still gets its own spec and its own
yes before anything is built. Nothing was installed or changed to write it.

## The short answer

| Project | What it is | Verdict | How |
|---|---|---|---|
| **Skills Hub** | Hermes's own skill registry and installer | **Add first** | A Kairos page over `hermes skills`. Small, high value |
| **CUA** | Background control of desktop apps | **Add second** | Already installed with Hermes. Kairos needs the rules, the cards and the switch |
| **Edge Drop** | Clipboard shelf on the screen edge | **Add the idea, not the app** | A native shelf in the island, off by default. Its code is Electron and can't be embedded |
| **Hermes Studio** | Other people's web and desktop clients for Hermes | **Take ideas only** | Command palette, MCP manager, usage charts. It competes with Kairos, so don't add it |
| **GBrain** | A memory layer you own, as Markdown | **Try it, don't adopt it yet** | A two-week trial over MCP, after the MCP manager. Overlaps M11's memory |
| **Agent Reach** | Reads login-walled sites with your logins | **Optional, your risk** | Not bundled. Hermes already covers YouTube, GitHub, arXiv and Reddit reading |

The proposed order is M19 Skills → M20 Computer use → M21 Palette and MCP manager → M22 Clipboard
shelf, with the GBrain trial possible after M21.

## What was checked, and how far

| Fact | Source |
|---|---|
| `cua-driver` 0.21.0 installed; `computer_use` **on** for the Hermes CLI and **off** for `api_server`, which is what Kairos uses | Run on this PC (`hermes computer-use status`, `hermes tools list --platform api_server`) |
| 51 built-in skills, 0 from the hub; no MCP servers; memory provider `holographic`; no `bun` or `uv` on this PC | Run on this PC |
| The Kairos rules have no `computer_use` rule | `plugin/kairos/policy.default.json` |
| Hermes's computer use and skills behaviour | Hermes 0.22.5's own docs, in its install |
| Agent Reach, GBrain, Hermes Studio (JPeetz, EKKO), Edge Drop, CUA | Their own READMEs and site, read 2026-11-01 |

**Not verified:** none of these was run. Agent Reach's and GBrain's READMEs don't state Windows
support. Popularity figures disagree between sources, so none is relied on here.

---

## 1. Skills Hub — add first

**What it is.** Part of Hermes, not an outside tool: `hermes skills browse | search | inspect |
install | list | check | update | audit | uninstall`, over several registries (official, skills.sh,
well-known URLs, GitHub, custom taps). An install goes to quarantine, is scanned, and its source,
hash and verdict are recorded.

**Why first.** You have 51 built-in skills and none from the hub, and Kairos shows none of this.
It is the cheapest large gain: new abilities with no new code.

**What Kairos adds: M19, Settings ▸ Skills (or its own page).**
1. **Installed:** name, category, source, on or off.
2. **Find:** search across the registries, with the source named on every result.
3. **Inspect before install:** the skill's text and the scanner's verdict, shown first.
4. **Install, uninstall, update, re-audit**, each from a click.

**How.** Through the `hermes skills` commands only, as M18 does for providers. `GET /v1/skills` can
list them read-only, but the recorded fixture for it is a server error, so re-test it first. Check
for machine-readable output; if there is none, parsing a table is the weak point of this milestone.

**Risks and rules.**
- A skill is instructions the model will follow, so a bad one is a prompt injection you installed.
- Kairos never passes `--force`. A blocked skill stays blocked; the override stays in a terminal.
- Third-party sources carry a visible warning; official ones say so.
- The Kairos rules keep applying to whatever a skill makes the model do.

**Size:** small to medium. **Decision needed:** may Kairos install skills through Hermes from your
click? (It changes `~/.hermes/skills`, so it needs your yes under CLAUDE.md.)

---

## 2. CUA — add second

**What it is.** `trycua/cua` holds several products, and only one matters here:

| Part | Licence | For Kairos |
|---|---|---|
| **Cua Driver** | MIT | **Yes.** Hermes ships and pins it. It drives apps through UIAutomation without moving your cursor or taking focus |
| Cua MCP server | see repo | No need: Hermes already speaks to the driver |
| Spaces | FSL-1.1-MIT | No. Shared VMs; Windows is a preview; the hosted relay can read traffic in transit |
| Lume | MIT | No. macOS VMs |

**Why it matters.** It is the step from "Kairos opens an app" to "Kairos uses the app", working
beside you, on any tool-capable model. It's the most JARVIS-like item on this list, and also the
riskiest.

**What Kairos adds: M20, Computer use.** Nothing to install. Five parts:
1. **A probe first (on the PC):** does an approval for `computer_use` reach Kairos over `/v1/runs` as
   a normal `approval.request`? Hermes's docs say that where nobody can answer the action is refused,
   so this must be seen before anything else. Record a fixture.
2. **Rules before the switch.** Add `computer_use` rules while it is still off: reading the screen
   allowed or asked (your choice), every click, key and typing action asks, and banking, password
   managers and Windows Security always refused. Add it to `KAIROS_TOOLS`, so only Kairos's own
   conversations can use it through Kairos (D-032).
3. **Cards in words:** "Click Send in Outlook", with the app and the window, the way `describe_script`
   does for browser scripts, instead of raw coordinates.
4. **Seeing it happen:** the top bar shows which app Kairos is working in, with a Stop button.
   Captures must also step aside for the bar (`hidesForCapture`).
5. **The switch:** a setting that turns the toolset on for `api_server`, off by default. It changes
   Hermes's configuration, so it needs your yes.

**Risks and rules.**
- A page or document on screen can carry instructions. Hermes tells the model not to follow them,
  which is not a guarantee, so every action still asks.
- Never with "Always grant" for typing. Hermes's grant keys are per action kind, so one "always"
  would cover all typing: Kairos should offer only once and this session here.
- Never `--yolo`, which switches the driver to its unrestricted mode (CLAUDE.md already forbids it).
- Known limits: it can't drive elevated (admin) windows, and it has no password entry by design.
- `computer_use` is on today for the Hermes command line, without a Kairos rule behind it. Rules
  will close that too, because the plugin's hook sees CLI calls.

**Size:** medium. **Decisions needed:** the switch; whether reading the screen also asks.

---

## 3. Edge Drop — add the idea

**What it is.** A clipboard shelf for Windows 10 and 11: hover a screen edge or press Alt+C, then
drag files and images out or click to copy. It holds text, links, images, files, colours and code.
History is encrypted with DPAPI, nothing leaves the machine, and it skips what password managers
mark as private. Apache-2.0. Its "AI" features are only on its roadmap. (Not Microsoft's retired
Edge Drop.)

**Two ways:**
- **Just install it** alongside Kairos: no work, and it's a good app, but it knows nothing of Kairos.
- **A native shelf in Kairos: M22.** It's Electron with `koffi`, so it can't be embedded in a Tauri
  app. The behaviour is simple to rebuild in Rust, and Apache-2.0 allows borrowing its ideas (with a
  notice for any code reused).

**Why build it, given Win+V exists?** Only for what Windows can't do: **"Ask Kairos about this"**
on any clip (explain this error, translate this, summarise this link, look at this screenshot). If
that isn't wanted, install Edge Drop instead and skip this milestone.

**What Kairos adds: M22, Clipboard shelf** as a tab in the island's panel, and optionally its own
screen edge (the right, since the top left now reveals the bar).
1. Capture with `WM_CLIPBOARDUPDATE`, no polling.
2. Skip `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory = 0`, which
   password managers set.
3. Encrypted at rest with DPAPI, with a size and age limit, an incognito switch and Clear all.
4. Click to copy; "Ask Kairos" starts a request holding that clip as data, as M17 quotes events.

**Risks and rules.**
- A clipboard holds passwords and private text, so it is **off by default**.
- **The model never reads the history by itself.** No tool for it: a clip reaches the model only from
  your click on that clip. This is the rule that makes the feature acceptable.
- Dragging out to other apps needs native drag support, probably a new dependency: ask at spec time,
  or ship click-to-copy first.

**Size:** medium. **Decision needed:** build it, or install Edge Drop?

---

## 4. Hermes Studio — ideas only

**What it is.** Three separate projects share the name, none official:

| Project | Kind | Licence | Note |
|---|---|---|---|
| `JPeetz/Hermes-Studio` | Web dashboard, self-hosted | MIT | Password off by default, with a full terminal in the browser |
| `EKKOLearnAI/hermes-studio` (Ekko Studio) | Electron and web | **BSL-1.1** | Not open source: **don't copy its code**, ideas only |
| `helciocosta/hermes-studio` | Desktop and web | Not confirmed | Same description and npm package as Ekko Studio's |

**Why not add one.** They are clients of Hermes, as Kairos is. Running one alongside means a second
door to Hermes that may not pass through Kairos's cards, and a browser terminal if it's exposed.

**Kairos already has:** chat with tool rows, approvals, scheduled jobs, memory, the audit log, cost
per request, models, themes, voice, Hermes's start.

**Worth taking:**

| Idea | Value | Goes in |
|---|---|---|
| **Command palette and search over conversations (Ctrl+K)** | High: Kairos has neither | M21 |
| **MCP server manager** (`hermes mcp`, none configured today) | High: opens up many tools, and GBrain | M21 |
| **Usage charts** (tokens, cost, model mix over 30 days) | Medium: Kairos already stores the numbers | M21 or later |
| **A "this session" approval scope** | Medium: needed for M20 | M20 |
| **Pending-approvals badge** on the bar and sidebar | Medium | M20 |
| Attachments and file previews in chat | Medium | Later |
| Backup and import of Kairos's data | Medium | Later |
| Phone access through a PWA and Tailscale | Answers your earlier mobile question without an app | Its own plan |
| Kanban, crews, workflow builder, group chat, coding-agent launcher | Low for one person, and large | No |

**Size of M21:** small to medium.

---

## 5. GBrain — a trial

**What it is.** Garry Tan's memory layer (MIT): a Markdown repo you own, indexed for search, with a
self-wiring graph, sources on facts, and corrections. PGLite by default, served over MCP. It can run
with no API key (`gbrain init --pglite --no-embedding`, keyword search). Better search needs an
embedding provider; Ollama is listed, so it could stay local.

**Why not adopt it now.**
- **Overlap.** M11 accepted Hermes's `holographic` memory on 2026-10-27, and two memories can
  disagree. It should replace or sit under that, not simply join it.
- **Windows is unproven.** The hosted installer is a macOS and Linux shell script. A related project's
  pull request records Bun install failures in Windows shells. Bun isn't on this PC.
- **Weight.** The README suggests a server with 8 GB or more and its own API usage for the always-on
  setup, with cron enrichment. That's more than a laptop assistant needs.
- **Memory is a door for injection:** something read from the web and stored comes back later as
  trusted.

**The trial, after M21 (MCP manager):**
1. Install Bun (a new dependency: your yes), then GBrain with no embeddings.
2. Add it with the `starter` tool surface, as an MCP server of the **Kairos profile only**.
3. Kairos rules: reading memory allowed, writing asks. No signal capture, no cron jobs.
4. Use it for two weeks beside `holographic`.
5. **Decide:** does it recall things `holographic` misses? If yes, write a spec and a decision that
   replaces M11's. If no, remove it. Worth borrowing either way: sources and corrections on the
   Memory page.

**Size:** small to try; large to adopt.

---

## 6. Agent Reach — optional, at your risk

**What it is.** MIT. A command line (`agent-reach <channel> <action>`, `agent-reach read <url>`) that
routes to other tools: web, GitHub, YouTube and Bilibili need no login; Reddit, X, Instagram, Facebook,
LinkedIn search, Xiaohongshu and others reuse **your Chrome logins** through a background browser. It
describes itself as read-only.

**What you already have.** Hermes's built-in `youtube-content`, `github`, `arxiv` and `maps` skills,
its web tools, and an optional `reddit-reading` skill, which credits Agent Reach for the idea. So the
no-login part adds little.

**What it adds, and the price.** X, Instagram, Facebook, LinkedIn and Chinese platforms, using real
logins.
- Its own README recommends a **secondary account**, because a copied login is usually your main one.
- Automated reading can break a site's terms and get an account limited.
- A social feed is the worst place for injection: anyone can write what your agent reads.
- A browser holding your sessions runs in the background.
- Windows isn't stated, and the install runs a downloaded PowerShell script (`irm … | iex`) for `uv`.
  The PyPI package of the same name is **not** this project.

**Recommendation.** Don't bundle it or add a page for it. If you want X or Reddit:
1. First try `hermes skills install` for `reddit-reading` (arrives naturally with M19).
2. If that isn't enough, install Agent Reach yourself with a secondary account, default channels only.
3. Kairos's part is one rule (`agent-reach` read commands allowed, anything touching logins asks) and
   a line in the docs. D-027, about using your real browser profile, should be settled first, since
   it's the same question.

**Size:** very small on Kairos's side. **Decision needed:** do you want these sites at all?

---

## The order, and what each needs from you

| # | Milestone | Size | Your yes is needed for |
|---|---|---|---|
| **M19** | Skills page | S–M | Kairos installing skills through Hermes |
| **M20** | Computer use: rules, cards, Stop, switch | M | Turning the toolset on for Kairos; whether reading the screen asks |
| **M21** | Ctrl+K palette and search, MCP manager, usage charts | S–M | Kairos changing Hermes's MCP list from your clicks |
| **M22** | Clipboard shelf | M | Building it instead of installing Edge Drop; a drag dependency if wanted |
| Trial | GBrain over MCP | S | Installing Bun; a later decision that could replace M11 |
| None | Agent Reach | XS | Whether you want those sites, and a secondary account |

**Why this order.** M19 is the fastest gain and also brings `reddit-reading`. M20 is the biggest
change, and its rules should exist before anything else widens what Kairos can do. M21 unlocks the
GBrain trial. M22 stands alone and can move up if you'd use it daily.

**Waiting already, and unaffected:** your pattern images (M16), the on-PC checks in `docs/TESTING.md`,
the Ollama test (M18), and merging the milestone branches into `main`, which now run to M18. Ten or
more stacked branches get harder to merge the longer they wait, so that may deserve to come first.

## Sources

- [trycua/cua](https://github.com/trycua/cua) and [cua.ai](https://cua.ai/); Hermes's `computer-use.md` (in its install)
- [garrytan/gbrain](https://github.com/garrytan/gbrain); [gstack PR #1487, GBrain install on Windows shells](https://github.com/garrytan/gstack/pull/1487)
- [Hermes skills documentation](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills)
- [Panniantong/Agent-Reach](https://github.com/Panniantong/agent-reach) and [its install guide](https://github.com/Panniantong/Agent-Reach/blob/main/docs/install.md)
- [JPeetz/Hermes-Studio](https://github.com/JPeetz/Hermes-Studio), [EKKOLearnAI/hermes-studio](https://github.com/EKKOLearnAI/hermes-studio), [helciocosta/hermes-studio](https://github.com/helciocosta/hermes-studio)
- [Deepender25/Edge-Drop](https://github.com/Deepender25/Edge-Drop) and [edgedrop.app](https://www.edgedrop.app/)
