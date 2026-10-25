# M3 acceptance record

**Status: waiting for the product owner's checks on the PC.** Steps 1–7 are built and committed;
everything that could run without Windows, Hermes or a person was run on 2026-10-25 in a Linux
cloud session (results below). Nothing marked "On the PC" has been seen yet.

## Before testing on the PC

1. `rustup update` if `rustc --version` is older than 1.95 (`sysinfo` needs it).
2. No new JavaScript packages; `pnpm -C app install` is not needed.
3. **Restart the Hermes gateway** in a Terminal panel tab: the plugin's PC tools are new Python
   code, and Hermes imports the plugin once (M2 §4).
4. Settings ▸ Rules shows "New starter rules came with this version": press **Add the new rules**,
   then **Save rules**. (Your `policy.json` predates `open-installers` and `open-apps`; without
   them `open_app` still works through Hermes's own rules, but the tool row won't say
   "allowed by open-apps" and installers won't ask.)
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

## On the PC (product owner)

| Step | Check | Result |
|---|---|---|
| 2 | System page figures look plausible: CPU, cores, RAM, each drive with read/write, Wi-Fi name and signal, GPU (NVIDIA), battery on a laptop | |
| 3 | System page vs Task Manager, side by side for a minute: CPU, RAM, disk, network, GPU within a few percent (CPU may read lower; M3 §5.3) | |
| 4 | "Run speed test" finishes in under 40 s, close to speed.cloudflare.com in a browser | |
| 5 | Chat "open Notepad": Notepad opens with no prompt; tool row says "allowed by open-apps" | |
| 5 | Chat "close Notepad" (with unsaved text): approval card from `close-apps`; after allowing, Kairos reports Notepad is still open because it is asking to save, and does **not** force it | |
| 5 | Chat "close explorer": blocked by `system-processes` | |
| 5 | Chat "what apps are open?": uses `list_running_apps`, no terminal | |
| 6 | A CPU-burning app raises the alert within about a minute; Windows toast and banner; "Close it?" opens a conversation and leads to a `close-apps` approval card. The threshold is 80 % of the **whole** CPU, summed over an app's processes, so one busy loop isn't enough: start `powershell -NoProfile -Command "while(1){}"` once per logical core (Task Manager ▸ Performance ▸ CPU shows the count) | |
| 6 | Settings ▸ System ▸ alerts off: no more toasts; back on: no replay of old ones | |
| 7 | Closing the window leaves Kairos in the tray; tray "System" opens the System page; "Quit Kairos" exits | |
| 7 | Starting Kairos a second time focuses the running one | |
| 7 | Lock (Win+L) and unlock: "PC unlocked" in Settings ▸ Startup ▸ Recently. Sleep and wake: "PC woke from sleep (slept N min)" | |
| 7 | Settings ▸ Startup switch on/off changes the login entry (Task Manager ▸ Startup apps shows Kairos) | |

**Note on the login entry.** Only a **release** build switches "start at login" on by itself on
first launch; `pnpm tauri dev` never does, so the entry can't point at a debug build. To test
criterion 3, run `pnpm -C app tauri build`, start the built Kairos once, then reboot.

## Acceptance criteria (ROADMAP M3)

| Criterion | Result |
|---|---|
| The System page matches Task Manager within a few percent | |
| "Open Notepad" runs; "close Notepad" asks first | |
| After a reboot, Kairos (in the tray) and Hermes are both running with nobody starting them | |

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
