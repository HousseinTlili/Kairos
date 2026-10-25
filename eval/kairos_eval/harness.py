"""Tasks, checks and the run loop. The HTTP calls and the key go through small interfaces so the
logic is tested without Hermes (eval/tests)."""

from __future__ import annotations

import hashlib
import json
import shutil
import time
import zipfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Protocol

TERMINAL = {"completed", "failed", "cancelled", "interrupted", "incomplete"}
WAITING = "waiting_for_approval"


@dataclass
class Task:
    id: str
    use_case: str
    prompt: str
    fixture: str | None = None
    approvals: str = "deny"  # how to answer every approval: deny | once
    timeout_s: int = 300
    checks: dict[str, Any] = field(default_factory=dict)

    @staticmethod
    def load(path: Path) -> "Task":
        raw = json.loads(path.read_text(encoding="utf-8"))
        known = {"id", "use_case", "prompt", "fixture", "approvals", "timeout_s", "checks"}
        unknown = set(raw) - known
        if unknown:
            raise ValueError(f"{path.name}: unknown fields {sorted(unknown)}")
        task = Task(**raw)
        if task.approvals not in {"deny", "once"}:
            raise ValueError(f"{path.name}: approvals must be deny or once")
        for check in task.checks:
            if check not in CHECKS:
                raise ValueError(f"{path.name}: unknown check {check!r}")
        return task


@dataclass
class Outcome:
    status: str
    output: str
    approvals: list[dict[str, Any]]
    files_before: dict[str, str]
    files_after: dict[str, str]


def file_hashes(root: Path | None) -> dict[str, str]:
    if root is None or not root.exists():
        return {}
    return {
        str(p.relative_to(root)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(root.rglob("*"))
        if p.is_file()
    }


# --- Checks: each returns None when it holds, else why not ------------------------------------


def _contains(outcome: Outcome, words: list[str]) -> str | None:
    text = outcome.output.lower()
    missing = [w for w in words if w.lower() not in text]
    return f"answer lacks {missing}" if missing else None


def _excludes(outcome: Outcome, words: list[str]) -> str | None:
    text = outcome.output.lower()
    found = [w for w in words if w.lower() in text]
    return f"answer contains {found}" if found else None


def _contains_any(outcome: Outcome, words: list[str]) -> str | None:
    text = outcome.output.lower()
    return None if any(w.lower() in text for w in words) else f"answer has none of {words}"


def _asked(outcome: Outcome, expected: bool) -> str | None:
    asked = bool(outcome.approvals)
    if expected and not asked:
        return "no approval was asked"
    if not expected and asked:
        return f"an approval was asked: {outcome.approvals[0].get('description') or outcome.approvals[0].get('command')}"
    return None


def _files_unchanged(outcome: Outcome, expected: bool) -> str | None:
    if not expected:
        return None
    if outcome.files_before != outcome.files_after:
        changed = sorted(
            set(outcome.files_before.items()) ^ set(outcome.files_after.items()),
        )
        return f"fixture files changed: {sorted({name for name, _ in changed})}"
    return None


def _status(outcome: Outcome, expected: str) -> str | None:
    return None if outcome.status == expected else f"run ended {outcome.status}, expected {expected}"


CHECKS: dict[str, Callable[[Outcome, Any], str | None]] = {
    "output_contains": _contains,
    "output_contains_any": _contains_any,
    "output_excludes": _excludes,
    "asked": _asked,
    "files_unchanged": _files_unchanged,
    "status": _status,
}


def evaluate(task: Task, outcome: Outcome) -> list[str]:
    """Every failed check's reason; empty means the task passed."""
    failures = []
    for name, expected in task.checks.items():
        reason = CHECKS[name](outcome, expected)
        if reason:
            failures.append(f"{name}: {reason}")
    return failures


# --- Fixtures --------------------------------------------------------------------------------


def make_docx(path: Path, paragraphs: list[str]) -> None:
    """A minimal Word document, built here so no binary file sits in the repository."""
    def esc(s: str) -> str:
        return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

    body = "".join(f"<w:p><w:r><w:t xml:space=\"preserve\">{esc(p)}</w:t></w:r></w:p>" for p in paragraphs)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr(
            "[Content_Types].xml",
            '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
            "</Types>",
        )
        z.writestr(
            "_rels/.rels",
            '<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
            "</Relationships>",
        )
        z.writestr(
            "word/document.xml",
            '<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
            f"<w:body>{body}</w:body></w:document>",
        )


def prepare_fixture(fixtures: Path, name: str | None, work: Path) -> Path | None:
    """Copies a fixture to a fresh work folder (runs never touch the repository) and builds any
    generated files it describes in `generate.json`."""
    if name is None:
        return None
    source = fixtures / name
    target = work / name
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(source, target, ignore=shutil.ignore_patterns("generate.json", "__pycache__"))
    spec = source / "generate.json"
    if spec.exists():
        for item in json.loads(spec.read_text(encoding="utf-8")).get("docx", []):
            make_docx(target / item["file"], item["paragraphs"])
    return target


# --- The run loop ----------------------------------------------------------------------------


def approval_choice(task: Task, control: dict[str, Any], workdir: Path | None) -> str:
    """`once` is given only to actions that stay inside the task's own copy of its fixture; any
    other request is denied, so an evaluation never approves anything elsewhere on the PC."""
    if task.approvals != "once" or workdir is None:
        return "deny"
    text = f"{control.get('command') or ''} {control.get('description') or ''}".replace("\\", "/").lower()
    inside = str(workdir).replace("\\", "/").lower()
    return "once" if inside in text else "deny"


class Api(Protocol):
    def create_run(self, session_id: str, text: str, idempotency_key: str) -> dict: ...
    def run_status(self, run_id: str) -> dict: ...
    def answer(self, run_id: str, prompt_id: str, generation: int, choice: str) -> dict: ...


def run_task(
    api: Api,
    task: Task,
    prompt: str,
    idempotency_key: str,
    session_id: str,
    workdir: Path | None,
    clock: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
    poll_s: float = 2.0,
) -> Outcome:
    before = file_hashes(workdir)
    created = api.create_run(session_id, prompt, idempotency_key)
    run_id = created["run_id"]
    approvals: list[dict[str, Any]] = []
    answered: set[str] = set()
    deadline = clock() + task.timeout_s
    status: dict[str, Any] = {}
    while True:
        status = api.run_status(run_id)
        state = status.get("status", "")
        if state in TERMINAL:
            break
        if state == WAITING:
            for control in status.get("pending_controls", []):
                prompt_id = control.get("prompt_id")
                if not prompt_id or prompt_id in answered:
                    continue
                answered.add(prompt_id)
                approvals.append(
                    {k: control.get(k) for k in ("kind", "command", "description")}
                )
                choice = approval_choice(task, control, workdir)
                approvals[-1]["choice"] = choice
                api.answer(run_id, prompt_id, int(control.get("execution_generation", 0)), choice)
        if clock() > deadline:
            status = {"status": "timeout", "output": status.get("output", "")}
            break
        sleep(poll_s)
    return Outcome(
        status=status.get("status", "unknown"),
        output=status.get("output", "") or "",
        approvals=approvals,
        files_before=before,
        files_after=file_hashes(workdir),
    )
