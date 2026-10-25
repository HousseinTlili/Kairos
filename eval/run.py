"""Runs the Kairos evaluation set (M5 §4). From the repository root, with Hermes running:

    & (Get-Item "$env:LOCALAPPDATA\\hermes\\tools\\python-*\\python.exe")[0] -I eval/run.py

`--list` shows the tasks, `--task <id>` runs one, `--resume <results.json>` continues a run.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from kairos_eval.cli import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
