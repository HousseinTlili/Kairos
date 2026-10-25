# M4 acceptance record

**Status: built (2026-10-25), nothing run on Windows yet.** Steps 1–6 are on `m4/notifications`
(which starts from `m3/review`). The product owner left the open choices to the engineer's
recommendation: the `windows` crate was added, D-018 accepted, and Step 4 built before the probe.
Each can still be reversed at testing.

## Testing order (one or two sittings)

1. **M3 first:** the final checklist in `M3-acceptance.md` (speed test, sleep and wake, reboot with
   autostart), on `m3/review`. M3 is signed off on its own.
2. **Switch to M4:** `git checkout m4/notifications`, then **restart the Hermes gateway** (plugin
   0.4.0 adds `recent_notifications`). `kairos.db` moves to schema 3 on first start.
3. **The probe** (below). If the verdict isn't Readable, stop and report it: the listener can't
   work, and the rest of this list can't pass.
4. The **on the PC** table, top to bottom.

## Step 1 — the probe (product owner, 2 minutes)

In **Windows PowerShell 5.1** (`powershell.exe`, not `pwsh`), from the Kairos folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/probe-notifications.ps1
```

It shows one test toast ("Kairos probe NNNN") and prints counts and app names only, never message
text, so its output is safe to paste back. **Look for the last line:**

| Verdict | Meaning | Next |
|---|---|---|
| `Readable` (exit 0) | An unpackaged app can read notifications here | Go on with the table below |
| `Denied` (exit 1) | Windows refused | Check Settings ▸ Privacy & security ▸ Notifications ▸ notification access, run it again; if still denied, you choose between packaging Kairos and changing D-011 |
| `Unsupported` (exit 2) | The listener isn't available to this process | As for Denied |
| `Inconclusive` (exit 3) | Allowed, but the test toast wasn't found | Turn off Do not disturb, allow PowerShell notifications, run again |

**Not run anywhere yet**: the cloud session has no Windows or PowerShell, so the script hasn't even
been parsed by PowerShell. A syntax error on first run is possible; paste it back if so.

## Checks run in the cloud session (2026-10-25, Linux)

| Check | Result |
|---|---|
| `cargo test` | 114 unit and 19 fixture tests passed (new: store, v2 → v3 migration, collector, export, switch) |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | Clean |
| `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | Clean, including the listener in `notifications/windows.rs` and the poller (MinGW; compiled, not run) |
| `pnpm -C app test`, lint, `tsc --noEmit` | 71 passed; clean |
| Plugin tests (Python 3.13, not Hermes's 3.14.9) | 86 passed (new: `recent_notifications`, its steering, registration) |
| `scripts/probe-notifications.ps1` | **Not run** (no PowerShell here) |

## On the PC

| Step | Check | Result |
|---|---|---|
| 1 | Probe verdict | |
| 4 | Settings ▸ Notifications ▸ switch on: Windows may ask for access (allow it); the status line then says "On" | |
| 4 | Send yourself a Discord message (or run the probe again for a test toast): it appears in the Kairos panel within 5 s and the sidebar badge counts it | |
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

## Known limits

- **Toasts from `pnpm tauri dev`** carry Windows PowerShell's identity, not Kairos's, so in a dev
  build Kairos's own alert toasts can be collected as "Windows PowerShell" notifications (the
  installed build is recognised and skipped). The probe's test toasts are collected the same way,
  which is handy for testing.
- **Polling, not push:** a notification appears up to 3 s after Windows shows it.
- **Only what the toast says:** sender and preview, not the conversation (D-011).

## Sign-off

Pending: after M3's sign-off and the checks above.
