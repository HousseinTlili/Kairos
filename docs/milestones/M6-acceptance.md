# M6 acceptance record

**Status: built (2026-10-25), nothing run on Windows or with a real account yet.** Steps 1–4 are
on `m6/accounts` (which starts from `m5/briefing`). Test after M5 is signed off.

## Before testing

1. `git checkout m6/accounts`; restart the Hermes gateway.
2. Settings ▸ Rules ▸ **Add the new rules** (`no-account-tokens`, `account-writes`,
   `account-disconnect`), then Save.
3. **GitHub CLI:** `gh --version` in a terminal. If missing: `winget install GitHub.cli` (that
   command itself will ask through `install-software` if Kairos runs it; run it yourself instead).
4. **Google, once, in your own Google account** (Hermes's skill walks you through it too):
   1. <https://console.cloud.google.com/> ▸ create a project (any name, e.g. "Kairos").
   2. APIs & Services ▸ Library: enable **Gmail API** and **Google Calendar API**.
   3. Google Auth Platform ▸ Branding/Audience: user type **External**, publishing status
      **Testing**, add **your own address as a test user**.
   4. Clients ▸ Create client ▸ **Desktop app**; download its JSON file.
   Kairos never sees this file's contents or your password; Hermes's skill uses the file to start
   Google's own sign-in.

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 142 unit and 19 fixture tests passed (new: account commands, Google status, flat state) |
| `cargo clippy -D warnings` (Linux and `x86_64-pc-windows-gnu`), `cargo fmt --check` | Clean |
| `pnpm -C app test`, lint, `tsc --noEmit` | 81 passed; clean (new: account wording, Google setup request, briefing with accounts) |
| Plugin tests | 89 passed; the policy table gained 18 account cases (writes through `google_api.py`, `$GAPI`, `gws`, `himalaya`; reads; tokens; disconnects) |
| Real accounts, `gh`, Google's sign-in | **Not run** (no Windows, no accounts here) |

## On the PC

| Step | Check | Result |
|---|---|---|
| 3 | Settings ▸ Accounts lists Google and GitHub with their state ("not installed" reasons if a route is missing) | |
| 3 | GitHub ▸ **Connect**: a terminal window opens with `gh auth login`; finish in the browser; **Check again** shows Connected | |
| 3 | Google ▸ **Connect** (Hermes connected): a conversation where Hermes's skill gives a sign-in link; after signing in and pasting the redirect URL, **Check again** shows Connected | |
| 2 | "How many unread emails do I have?" → a `gmail search` in the terminal, no prompt | |
| 2 | "Reply to the last email from <someone> saying I'll call tomorrow" → the approval card from `account-writes` shows the full reply; **Deny**: nothing sent | |
| 2 | "Add a meeting tomorrow at 10" → asks (`account-writes`); deny | |
| 2 | "Show me the Google token file" → blocked by `no-account-tokens` | |
| 4 | Settings ▸ Briefing ▸ **Brief me now** with Google connected: the AI summary lists today's events and the unread count | |
| 3 | Google ▸ **Disconnect** (asks first) ▸ Not connected; "how many unread emails?" now fails to reach Gmail | |
| 3 | GitHub ▸ **Disconnect** ▸ Not connected; `gh auth status` in a terminal says not logged in | |

## Acceptance criteria (ROADMAP M6)

| Criterion | Result |
|---|---|
| The briefing lists today's calendar and unread email counts | |
| "Reply to X" drafts and asks before sending | |
| Disconnecting an account removes Kairos's access to it | |

## Known limits

- **Outlook isn't covered.** Hermes 0.22.5 has no Microsoft sign-in skill, and its IMAP route needs
  an app password, which D-010 rules out. It needs a Microsoft OAuth route first.
- **Google needs the product owner's own Google Cloud client**, in "Testing" mode: Google may ask
  to sign in again every 7 days until the client is published (Google's rule for test clients).
- **The scopes come from Hermes's skill**, which asks for read *and* write scopes for the services
  chosen. Kairos keeps writes behind `account-writes`; read-only at Google's level would need a
  change to Hermes's skill, which Kairos doesn't make (CLAUDE.md: never modify Hermes).
- **Account rules match command text.** A write expressed some other way (for example raw Python
  calling Google's API) isn't recognised; `browser-code-system-access` and the M7 container are the
  wider guards.
- **The briefing's calendar and email** come from the AI summary (the model runs the read
  commands), not the instant facts: Kairos doesn't read account data itself.

## Sign-off

Pending.
