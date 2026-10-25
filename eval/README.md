# Kairos evaluation set (M5 §4)

Twelve repeatable tasks with checkable outcomes, run against the real Hermes through the same API
the app uses. They cover the M5 use cases (code review, debug-and-fix, documents to actions) and
the rules (credentials, destructive commands, system changes, prompt injection).

```powershell
# from the repository root, with Hermes running and the Kairos skills installed
$py = (Get-Item "$env:LOCALAPPDATA\hermes\tools\python-*\python.exe")[0]
& $py -I eval/run.py --list                 # the tasks
& $py -I eval/run.py                        # all of them (several minutes on the free model)
& $py -I eval/run.py --task credentials-blocked
& $py -I eval/run.py --resume <results.json> # continue after a crash; reuses the stored keys
```

- **Key:** read from Windows Credential Manager (the entry the Kairos app stores), never `.env`.
- **Approvals:** answered as the task says, almost always **deny**: the point is to prove the ask
  happened, without letting anything run. The one task that approves (`debug-fixes-when-allowed`)
  approves only commands inside its own temporary copy of the fixture; anything else is denied.
- **Fixtures** are copied to a temporary folder for each task; the repository is never touched.
  The Word file is generated at run time, so no binary sits in the repository.
- **Results:** `%APPDATA%\kairos\eval\<timestamp>.json` (outside the repository: answers can
  mention personal paths), one line per task on screen.

The harness's own tests need no Hermes:
`python -I -m unittest discover -s eval/tests -t eval`.
