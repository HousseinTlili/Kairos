# M3 acceptance record

**Status: PC tools and unlock pass on the PC (2026-10-25); three checks left before sign-off:**
speed test, resume from sleep, and reboot with autostart. They are in "Final checklist" below.

Steps 1–7 were built in a Linux cloud session, reviewed and fixed on the Windows PC, and re-run
there. Each table says where a result comes from.

## Before testing on the PC

1. `rustup update` if `rustc --version` is older than 1.95 (`sysinfo` needs it).
2. No new JavaScript packages; `pnpm -C app install` is not needed.
3. **Restart the Hermes gateway** in a Terminal panel tab: the plugin's PC tools are new Python
   code, and Hermes imports the plugin once (M2 §4).
4. Settings ▸ Rules shows "New starter rules came with this version": press **Add the new rules**,
   then **Save rules**. Your `policy.json` predates five of them: `open-installers` and `open-apps`,
   and the three added in the Windows review (`kill-system-processes`, `kill-processes`,
   `run-installers`). Until they are added, ending a program from the terminal isn't covered.
5. Optional, for criterion 3: `powershell -NoProfile -ExecutionPolicy Bypass -File
   scripts/install-gateway-autostart.ps1` (asks first; answer yes to "start on login").

## Checks run in the cloud session (2026-10-25)

| Check | Result |
|---|---|
| `cargo test` | 92 unit and 19 fixture tests passed |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (Linux) | Clean |
| `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | Clean: the `cfg(windows)` code (lock state, Task Scheduler check, `CREATE_NO_WINDOW`) compiles for Windows. Built with MinGW, not MSVC, and not run |
| `pnpm -C app test`, `pnpm -C app lint`, `tsc --noEmit` | 64 passed; clean |
| Plugin tests (Python 3.13 on Linux, not Hermes's 3.14.9) | 70 passed |
| `winapps.py` | Byte-compiles; **never run** (Windows only) |
| App smoke run under a virtual display (Linux) | Starts without errors; records `startup` in `kairos.db`; the System page shows live CPU, memory, network and disks and updates |
| Speed test | **Not run**: the cloud session's network doesn't allow `speed.cloudflare.com` |

## Checks run on the PC by the agent (2026-10-25)

Reviewed and re-run on Windows 11 in the desktop session, against the real Hermes 0.22.5.

| Check | Result |
|---|---|
| Plugin tests on Hermes's own Python 3.14.9 | 70 passed |
| `winapps.py`, first run on Windows | Works: enumerated the open app windows and read 208 Start-menu apps (Notepad among them) |
| `cargo clippy --all-targets -- -D warnings` (MSVC), `cargo fmt --check` | Clean |
| `cargo test` (MSVC) | 92 unit and 19 fixture tests passed |
| `cargo test -- --ignored` (live Hermes) | 4 passed |
| `pnpm -C app test`, lint, `tsc --noEmit` | 65 passed; clean |
| Hermes loads the plugin | `kairos` 0.3.0 enabled; `/v1/toolsets` lists the `kairos` toolset with `close_app`, `list_running_apps`, `open_app` |
| "Open Notepad" through the API | Notepad opened with no prompt, **but through the terminal, not `open_app`**: the free model ignored the tool |

**Found and fixed:** with the model free to use the terminal, nothing covered ending programs
from the command line, so `close_app`'s protections could be skipped. Three new starter rules
(`kill-system-processes` deny, `kill-processes` ask, `run-installers` ask; M3 §6), 149 policy
cases across 23 rules, and the Rules page offers them to existing files.

## On the PC (product owner, 2026-10-25)

The product owner ran the checklist and reported every item as passed. The checks that leave a
record were then compared with `audit.db` and `kairos.db`; where the record differs, it wins
("Don't claim what you didn't see").

| Step | Check | Result | Evidence |
|---|---|---|---|
| 2 | System page figures plausible: CPU, cores, RAM, drives with read/write, Wi-Fi, GPU, battery | Pass | Product owner |
| 3 | System page vs Task Manager side by side | Pass | Product owner |
| 4 | Speed test finishes in under 40 s, close to speed.cloudflare.com | Not run | Reported as passed, but nothing records it; re-run in the final checklist |
| — | Settings ▸ Rules "Add the new rules", then Save | Pass | `policy.json` now has 23 rules, valid, identical to the starter set |
| 5 | "Open Notepad" opens with no prompt, through `open_app` | Pass | After 04ed2db: `open_app`, "allowed by open-apps", no prompt. (Before it: opened through `terminal`) |
| 5 | "Close Notepad" asks, then closes gently and lets it ask to save | Pass | After 04ed2db: `close_app`, asked by `close-apps`, approved once, result `closed`, gentle (not forced). (Before it: `taskkill /f` in the terminal) |
| 5 | "Close explorer" blocked by `system-processes` | Pass | After 04ed2db: "Close File Explorer" → `close_app` blocked by `system-processes` |
| 5 | "What apps are open?" uses `list_running_apps` | Pass | After 04ed2db: "Which apps are running right now?" → `list_running_apps`. (Before it: `tasklist`, `Get-Process`) |
| 6 | CPU alert, toast, banner, "Close it?" → approval card | Pass | Product owner |
| 6 | Alerts off: no more toasts; back on: no replay | Pass | Product owner |
| 7 | Closing the window keeps Kairos in the tray; tray menu works | Pass | Product owner |
| 7 | Starting Kairos a second time focuses the running one | Pass | Product owner |
| 7 | Lock and unlock: "PC unlocked" in Settings ▸ Startup | Pass | Locked at 10:14:43, unlocked at 10:14:47; `kairos.db` got a `presence_event` `unlock` at 10:14:50 (one 5 s tick later). `presence.rs` unchanged. (An earlier run showed none: the app had not been running across a lock) |
| 7 | Sleep and wake: "PC woke from sleep (slept N min)" in Settings ▸ Startup | Not run | This laptop uses Modern Standby; see "Resume under Modern Standby" |
| 7 | Settings ▸ Startup switch changes the login entry | Pass | Product owner |

**What the PC checks found, and how it was fixed.** In the first run none of the three PC tools
was called: the free model (`upstage/solar-mini4:free`) opened, closed and listed apps with
terminal commands, so "close Notepad" became `taskkill /f` and the app lost its chance to save.
Two fixes:

1. **Rules for the terminal route** (f7fd9f7): `kill-system-processes` (deny), `kill-processes`
   (ask) and `run-installers` (ask) cover `taskkill`, `Stop-Process` and installers run from
   `terminal`, `execute_code` and `browser_exec`, so `close_app`'s protections can't be skipped
   by going around it. 149 policy cases across 23 rules.
2. **Steering** (04ed2db, plugin 0.3.1): a `pre_llm_call` note, added only to turns that mention
   apps, tells the model to use `open_app`, `close_app` and `list_running_apps` instead of terminal
   commands. It is guidance; the rules still enforce.

After the gateway restart, live API runs used the tools for all four requests (open, list, close,
close File Explorer), with the expected rule on each. Plugin tests: 76 passed on Hermes's
Python 3.14.9.

**Note on the login entry.** Only a **release** build switches "start at login" on by itself on
first launch; `pnpm tauri dev` never does, so the entry can't point at a debug build. To test
criterion 3, run `pnpm -C app tauri build`, start the built Kairos once, then reboot.

## Acceptance criteria (ROADMAP M3)

| Criterion | Result |
|---|---|
| The System page matches Task Manager within a few percent | Pass (product owner) |
| "Open Notepad" runs; "close Notepad" asks first | Pass: `open_app` with no prompt; `close_app` asked by `close-apps`, then closed gently (after 04ed2db) |
| After a reboot, Kairos (in the tray) and Hermes are both running with nobody starting them | Not run: needs a release build and `install-gateway-autostart.ps1` |

## Known limits

- **GPU figures are NVIDIA only** (`nvidia-smi`); Intel and AMD GPUs show nothing yet.
- **Store-app windows belong to `ApplicationFrameHost.exe`**: `list_running_apps` lists Calculator
  or Settings under that name. Closing them by title works; force-closing them is refused, because
  it would close every Store app.
- **Windows toasts from `pnpm tauri dev`** may show under PowerShell's name instead of Kairos's;
  an installed build shows its own.
- **Wi-Fi reads `netsh`'s English/French/German labels** (`SSID`, `Signal`); other Windows
  languages may show "Not on Wi-Fi".
- **`audit.db` still has no retention limit** (from M2).

## Sign-off

Pending.
