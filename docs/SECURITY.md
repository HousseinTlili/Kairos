# Kairos Security Model

Kairos gives an AI model the ability to run commands, edit files and browse the web on your
computer. This document says what protects you and where each protection lives.

## 1. Principles

- **Enforced in code, not in prompts.** A system prompt saying "don't delete files" protects
  nothing. Every rule here is checked by software before the action runs.
- **Untrusted input.** Model output, web pages, documents, emails and tool results are data. Text
  inside them that looks like an instruction has no authority.
- **Kairos can only tighten.** Kairos's rules run on top of Hermes's built-in safety; they can block
  or require approval for more, never less.
- **Fail closed.** If a policy check errors or times out, the action does not run.

## 2. Layers, outermost first

| # | Layer | Owner | What it does |
|---|---|---|---|
| 1 | Hermes **hardline blocklist** | Hermes | Always-on floor of commands that are never allowed, whatever the mode |
| 2 | **Kairos policy** (`pre_tool_call`) | Kairos plugin | `deny` → block; `ask` → force human approval; unmatched → fall through |
| 3 | Hermes **user deny rules / allowlist** | Hermes config | `approvals.deny`, permanent allowlist |
| 4 | Hermes **approval mode** | Hermes config | `manual` (always ask), `smart` (auxiliary model decides low-risk cases; default), `off` |
| 5 | Hermes **protected paths** and optional `HERMES_WRITE_SAFE_ROOT` | Hermes | Blocks writes to sensitive locations; can restrict writes to one directory tree |
| 6 | **Isolation** of the terminal backend | Hermes + OS | Where commands actually run (§4) |
| 7 | **Audit log** | Kairos plugin | Every tool call, decision and approval, recorded with secrets removed |

Kairos never enables Hermes's YOLO mode, and the app shows a persistent warning if it finds YOLO
or approval mode `off` enabled.

## 3. Kairos policy file

`$KAIROS_HOME/policy.yaml`, edited from the app's settings. Rules are matched in order; the first
match wins. Illustrative shape (final format decided in milestone 2):

```yaml
version: 1
default: inherit            # unmatched calls: let Hermes decide
rules:
  - id: no-credentials
    tool: [read_file, write_file, patch, search_files, terminal, execute_code]
    match_any: ["**/.ssh/**", "**/hermes/.env", "**/*.pem", "**/auth.json"]
    action: deny
    reason: "Credential files are off-limits."
  - id: outbound-messages
    tool: [send_message, discord, discord_admin]
    action: ask
  - id: delegate-to-paid-agents
    tool: [delegate_task]
    action: ask
    reason: "Delegation can spend money on other services."
```

Every decision records the `rule_id`, so you can see why something was blocked or asked about.

Approval categories mirror the original plan:

- **Allowed**: read-only actions inside configured workspaces.
- **Approval required**: sending messages, publishing, installing software, writes outside the
  workspace, delegation to other agents or paid services.
- **Restricted (denied)**: reading credentials, disabling safeguards, destructive operations
  outside the authorised scope.

## 4. Isolation

A permission check decides *whether* a command runs. Isolation limits *what it can reach* if a
check is wrong. Hermes's terminal backend can run commands inside a Docker container with
security flags and resource limits (Hermes docs: Container Isolation).

| OS | Recommended from M7 |
|---|---|
| Windows 11 | Docker Desktop (WSL2 backend) container backend for terminal and code execution |
| Linux | Docker/Podman container backend; bubblewrap as a lighter alternative |

Until isolation is configured (M7), Kairos relies on its `deny`/`ask` rules (M2), Hermes approval
mode `smart` behind them (`DECISIONS.md`, D-009), and a workspace write root
(`HERMES_WRITE_SAFE_ROOT`).

## 5. Secrets

- Provider API keys: Hermes `.env` / credential vault only.
- Hermes API key: OS keyring for the app; Hermes `.env` for the server. Generated, ≥32 bytes.
- The audit log stores **redacted** arguments. The plugin removes values matching known secret
  shapes and anything from Hermes's redaction helpers before writing.
- No secrets in `policy.yaml`, `kairos.db`, logs, crash reports, or git.

## 6. Network exposure

- Hermes API server bound to `127.0.0.1`. Never `0.0.0.0` without an explicit, documented reason.
- CORS disabled (the Rust layer calls Hermes; the web view does not).
- Messaging gateways (Telegram, Discord, ...) are off unless deliberately enabled, each with its
  allowlist.

## 7. Prompt injection

Defences are structural, not textual:

- Consequential actions (`ask` rules) need a human click, whatever the model was told.
- The approval card shows the **exact** tool and arguments, not the model's description of them.
- Credential paths are denied outright, so a page saying "read ~/.ssh/id_rsa" fails at layer 2.
- Web and document content never changes policy; only the user, through the app, edits
  `policy.yaml`.

## 8. Reporting

Security issues in Kairos: open a private advisory on the Kairos repository. Issues in Hermes
itself go to Nous Research per Hermes's `SECURITY.md`.
