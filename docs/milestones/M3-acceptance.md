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

## Resume under Modern Standby

This laptop sleeps with Modern Standby (S0 low power idle, network connected), not classic S3.
Read from `app/src-tauri/src/presence.rs` (2026-10-25); not yet checked on the PC.

**How the detector works.** A thread wakes every 5 s, compares the wall clock with its previous
tick, and records `resume` when more than 60 s have passed. It doesn't ask Windows about power
at all: it relies on the thread **not running** while the PC sleeps.

**Under Modern Standby the PC never stops running**, so the result depends on whether Kairos is
paused:

- **Expected to work** when Windows suspends desktop apps during standby (its Desktop Activity
  Moderator does this to apps that aren't exempt). Kairos's thread then stops, and the first tick
  after waking sees the whole gap. When it does run, a resume event is recorded at the same 5 s
  latency the unlock showed.
- **Can miss a resume** if Kairos keeps getting CPU time during standby: on the charger, during
  Windows's maintenance wake-ups, or because timers fire late but still under 60 s apart. Ticks
  keep arriving less than 60 s apart and no gap is seen. A screen that only turns off (no sleep)
  correctly isn't a resume either.
- **Partly covered today:** when Windows asks for sign-in after sleep, waking also produces an
  `unlock` event (proven on the PC). M5's welcome-back briefing listens for both, so a missed
  `resume` costs the "slept N min" detail more than the briefing itself.

**No definite bug was found**, so the code is unchanged. The logic has unit tests for gaps, slow
ticks, a clock set backwards, and lock transitions; what can't be tested here is whether Windows
pauses the thread.

**If check B fails, the fallback is to ask Windows directly** instead of inferring from the
clock. Both options use the Windows API crate already approved in M3 §4: one more feature flag
(`Win32_System_Power`), no new dependency.

1. `PowerRegisterSuspendResumeNotification` with a callback (no window needed) for
   `PBT_APMRESUMEAUTOMATIC`, the documented resume signal.
2. `PowerSettingRegisterNotification` for `GUID_CONSOLE_DISPLAY_STATE` (display off → on). On
   Modern Standby that is the moment the user comes back, and it also fires when the screen only
   turned off.

Which of the two Windows sends on this laptop has to be recorded live before choosing. Keep the
clock-gap detector as the cross-platform fallback.

## Final checklist (product owner, one sitting, about 30 minutes)

The three checks left before sign-off. Do them in this order; write down what you see at each
"Look for", including failures and their exact messages. Run commands in PowerShell from the
Kairos folder. Hermes must be running (`curl http://127.0.0.1:8642/health` answers) for parts A
and B; part C restarts the PC.

### A. Speed test (2 minutes)

1. Open Kairos (`pnpm -C app tauri dev` is fine) ▸ **System** ▸ **Internet speed** ▸
   **Run speed test**.
2. **Look for:** the card goes "Checking latency…" → "Measuring download…" → "Measuring
   upload…" and shows Download, Upload (Mbps) and Latency (ms) within about 40 s.
3. In a browser, open <https://speed.cloudflare.com> and let it finish.
4. **Look for:** Kairos's figures in the same range as the browser's (speed tests vary run to
   run; a factor of two on a busy line is not a failure, ten times is).

### B. Resume from sleep (5 minutes)

1. Kairos running (dev or release). Unplug the charger if you can: Modern Standby behaves
   differently on battery (see "Resume under Modern Standby"), and battery is the case that
   matters most.
2. Start ▸ Power ▸ **Sleep**. Wait **at least 2 minutes** (watch a phone clock). Wake the laptop
   and sign in.
3. Wait 10 seconds, then open Kairos ▸ Settings ▸ **Startup** ▸ **Recently**.
4. **Look for:** a line **"PC woke from sleep (slept N min)"** with N about the time you waited,
   and a "PC unlocked" line just before or after it.
5. Check the database directly (uses Hermes's own Python, read-only):

   ```powershell
   @'
   import os, sqlite3
   db = os.path.join(os.environ["APPDATA"], "kairos", "kairos.db")
   for row in sqlite3.connect(db).execute(
       "SELECT datetime(at, 'unixepoch', 'localtime'), kind, detail "
       "FROM presence_event ORDER BY id DESC LIMIT 5"):
       print(*row)
   '@ | & (Get-Item "$env:LOCALAPPDATA\hermes\tools\python-*\python.exe")[0] -
   ```

6. **Look for:** a `resume` row with detail `slept N min`. If there is an `unlock` row but **no**
   `resume` row, the check fails: record it, and if you have time repeat it once on the charger, so
   we know whether battery vs charger makes the difference.

### C. Reboot with nobody starting anything (20 minutes, most of it the build)

1. **Quit every running Kairos** (tray icon ▸ Quit Kairos). The release build and the dev app share
   one identity, so a running dev app would swallow the release launch.
2. Build the release app:

   ```powershell
   pnpm -C app tauri build
   ```

   **Look for:** it ends with the bundle paths. The app itself is
   `app\src-tauri\target\release\kairos.exe`. (If only the installer step fails, for example
   while fetching WiX, the `.exe` is still built and is enough for this check.)
3. Start it once: `.\app\src-tauri\target\release\kairos.exe`. A release build switches
   "start at login" on by itself on first launch (D-013).
   **Look for:** Settings ▸ Startup ▸ "Start Kairos when I log in to Windows" is ticked, and Task
   Manager ▸ Startup apps lists Kairos as Enabled.
4. Set up Hermes's own login start:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-gateway-autostart.ps1
   ```

   Answer **y** to the script, then **yes** to Hermes's "Start the gateway automatically on
   Windows login". **Look for:** `[ok] Scheduled Task 'Hermes_Gateway' exists` (or "Startup-folder
   entry exists"). Settings ▸ Startup ▸ Hermes then says "Starts at login".
5. Close any terminal running `hermes gateway run` and Hermes Desktop, then **restart Windows**
   and sign in. Don't open anything.
6. Wait **one minute** (Hermes starts 30 s after logon), then:
   - **Look for:** the Kairos icon in the tray (it starts hidden there; no window).
   - `curl http://127.0.0.1:8642/health` answers with Hermes's version.
   - Open Kairos from the tray: it says Connected; Settings ▸ Startup ▸ Recently shows
     "Kairos started at login".
7. Optional: `hermes gateway status` reports the gateway running.

### Undo afterwards (if you don't want login start yet)

- Kairos: Settings ▸ Startup ▸ untick "Start Kairos when I log in to Windows". It stays off: the
  first-launch default is applied only once.
- Hermes:

  ```powershell
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-gateway-autostart.ps1 -Uninstall
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-gateway-autostart.ps1 -Status
  ```

  **Look for:** "The gateway does not start by itself at login yet." Start Hermes again the usual
  way (`hermes gateway run` in a terminal you keep open).
- Kairos's login entry points at `target\release\kairos.exe`; a later `cargo clean` would leave
  it pointing at nothing, so untick it first if you clean.

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
