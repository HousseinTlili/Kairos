# M4 acceptance record

**Status: partly built (2026-10-25); waiting for the product owner.** Steps 1, 2, 3, 5 and 6 are
built on `m4/notifications` (which starts from `m3/review`). **Step 4, the Windows listener, is not
written yet.** It waits for three things from the product owner:

1. **Run the Step 1 probe** on the PC and report its verdict (below).
2. **Approve the `windows` crate** (M4 §4; already in `Cargo.lock` through Tauri and sysinfo, but a
   new direct dependency of Kairos).
3. **Say yes or no to D-018** ("anything I missed?" through a plugin tool reading
   `notifications.json`; the model provider sees notification text when you ask about it). Step 6
   is built on it; a no means removing Step 6 before merging.

Until Step 4 exists, Settings ▸ Notifications says Kairos can't read notifications on this PC, and
the panel stays empty. That is expected, not a bug.

## Step 1 — the probe (product owner, 2 minutes)

In **Windows PowerShell 5.1** (`powershell.exe`, not `pwsh`), from the Kairos folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/probe-notifications.ps1
```

It shows one test toast ("Kairos probe NNNN") and prints counts and app names only, never message
text, so its output is safe to paste back. **Look for the last line:**

| Verdict | Meaning | Next |
|---|---|---|
| `Readable` (exit 0) | An unpackaged app can read notifications here | Approve the crate; Step 4 goes ahead as written |
| `Denied` (exit 1) | Windows refused | Check Settings ▸ Privacy & security ▸ Notifications ▸ notification access, run it again; if still denied, you choose between packaging Kairos and changing D-011 |
| `Unsupported` (exit 2) | The listener isn't available to this process | As for Denied |
| `Inconclusive` (exit 3) | Allowed, but the test toast wasn't found | Turn off Do not disturb, allow PowerShell notifications, run again |

**Not run anywhere yet**: the cloud session has no Windows or PowerShell, so the script hasn't even
been parsed by PowerShell. A syntax error on first run is possible; paste it back if so.

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 112 unit and 19 fixture tests passed (new: store, v2 → v3 migration, collector, export) |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | Clean |
| `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | Clean (MinGW; compiled, not run) |
| `pnpm -C app test`, lint, `tsc --noEmit` | 71 passed; clean |
| Plugin tests (Python 3.13, not Hermes's 3.14.9) | 86 passed (new: `recent_notifications`, its steering, registration) |
| `scripts/probe-notifications.ps1` | **Not run** (no PowerShell here) |

## On the PC (after Step 4)

| Step | Check | Result |
|---|---|---|
| 1 | Probe verdict | |
| 4 | A test toast appears in the Kairos panel within 5 s; the sidebar badge counts it | |
| 4 | With the switch off, a new toast does **not** appear; switching on doesn't import what was already in Action Center | |
| 5 | Mark read (one, one app, all); Clear asks first, then deletes; the badge follows | |
| 6 | Gateway restarted; "anything I missed?" → `recent_notifications` on the tool row, and a summary of the test toasts | |
| 6 | Switch off → `%APPDATA%\kairos\notifications.json` is gone; "anything I missed?" says reading is off | |

## Acceptance criteria (ROADMAP M4)

| Criterion | Result |
|---|---|
| A test toast and a real Discord message appear in Kairos within seconds | |
| Nothing is collected before permission is granted, and turning it off stops collection | |
| "Anything I missed?" is answered from them | |

## Sign-off

Pending: after M3's sign-off, and after Step 4 and the checks above.
