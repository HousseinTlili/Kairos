"""`eval/run.py`: runs the tasks against the real Hermes and stores the results (M5 §4).

The API key comes from Windows Credential Manager, the entry the Kairos app uses
(`hermes-api-key.kairos`), never from `.env`, and is never printed. Each task's Idempotency-Key is
written to the results file *before* its run is created, so rerunning with `--resume` never starts
the same task twice under a new key (M1 §9 rule 1).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

from .harness import Task, evaluate, prepare_fixture, run_task

EVAL_DIR = Path(__file__).resolve().parents[1]
CREDENTIAL_TARGET = "hermes-api-key.kairos"
DEFAULT_BASE = "http://127.0.0.1:8642"


def read_key_from_credential_manager() -> str:
    """The Hermes API key as the Kairos app stored it (Windows only)."""
    if sys.platform != "win32":
        raise SystemExit("The evaluation reads the key from Windows Credential Manager; run it on Windows.")
    import ctypes
    from ctypes import wintypes

    class CREDENTIAL(ctypes.Structure):
        _fields_ = [
            ("Flags", wintypes.DWORD),
            ("Type", wintypes.DWORD),
            ("TargetName", wintypes.LPWSTR),
            ("Comment", wintypes.LPWSTR),
            ("LastWritten", wintypes.FILETIME),
            ("CredentialBlobSize", wintypes.DWORD),
            ("CredentialBlob", ctypes.POINTER(ctypes.c_ubyte)),
            ("Persist", wintypes.DWORD),
            ("AttributeCount", wintypes.DWORD),
            ("Attributes", ctypes.c_void_p),
            ("TargetAlias", wintypes.LPWSTR),
            ("UserName", wintypes.LPWSTR),
        ]

    advapi32 = ctypes.WinDLL("advapi32", use_last_error=True)
    advapi32.CredReadW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.POINTER(ctypes.POINTER(CREDENTIAL))]
    advapi32.CredReadW.restype = wintypes.BOOL
    advapi32.CredFree.argtypes = [ctypes.c_void_p]
    pointer = ctypes.POINTER(CREDENTIAL)()
    if not advapi32.CredReadW(CREDENTIAL_TARGET, 1, 0, ctypes.byref(pointer)):  # CRED_TYPE_GENERIC
        raise SystemExit("No Hermes key in Credential Manager. Open Kairos once so it stores it.")
    try:
        cred = pointer.contents
        blob = bytes(cred.CredentialBlob[: cred.CredentialBlobSize])
    finally:
        advapi32.CredFree(pointer)
    return decode_secret(blob)


def decode_secret(blob: bytes) -> str:
    """The keyring crate stores the password as UTF-16; fall back to UTF-8 for other writers.
    Hermes keys are ASCII, so a UTF-16 reading that isn't plain ASCII was really UTF-8 bytes."""
    if len(blob) % 2 == 0:
        try:
            text = blob.decode("utf-16-le")
            if text.isascii() and text.isprintable():
                return text
        except UnicodeDecodeError:
            pass
    return blob.decode("utf-8")


class HttpApi:
    def __init__(self, base: str, key: str):
        self.base = base.rstrip("/")
        self._key = key

    def _call(self, method: str, path: str, body: dict | None = None, headers: dict | None = None) -> dict:
        data = json.dumps(body).encode("utf-8") if body is not None else None
        request = urllib.request.Request(self.base + path, data=data, method=method)
        request.add_header("Authorization", f"Bearer {self._key}")
        request.add_header("Content-Type", "application/json")
        for name, value in (headers or {}).items():
            request.add_header(name, value)
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.loads(response.read().decode("utf-8") or "{}")
        except urllib.error.HTTPError as err:
            # The body may echo the request; never the Authorization header, which isn't echoed.
            raise RuntimeError(f"{method} {path} -> HTTP {err.code}") from None

    def create_run(self, session_id: str, text: str, idempotency_key: str) -> dict:
        return self._call("POST", "/v1/runs", {"input": text, "session_id": session_id}, {"Idempotency-Key": idempotency_key})

    def run_status(self, run_id: str) -> dict:
        return self._call("GET", f"/v1/runs/{run_id}")

    def answer(self, run_id: str, prompt_id: str, generation: int, choice: str) -> dict:
        return self._call(
            "POST",
            f"/v1/runs/{run_id}/approval",
            {"request_id": prompt_id, "execution_generation": generation, "choice": choice},
        )


def results_dir() -> Path:
    base = os.environ.get("KAIROS_HOME") or Path(os.environ.get("APPDATA") or Path.home()) / "kairos"
    return Path(base) / "eval"


def load_tasks(selected: list[str] | None) -> list[Task]:
    tasks = [Task.load(p) for p in sorted((EVAL_DIR / "tasks").glob("*.json"))]
    if selected:
        tasks = [t for t in tasks if t.id in selected]
    return tasks


def save(path: Path, results: dict) -> None:
    temp = path.with_suffix(".tmp")
    temp.write_text(json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    temp.replace(path)


def run_all(api, tasks: list[Task], results_path: Path, results: dict, work: Path, log=print) -> dict:
    for task in tasks:
        entry = results["tasks"].setdefault(task.id, {})
        if "verdict" in entry:
            log(f"  {task.id:<32} {entry['verdict']} (already run)")
            continue
        workdir = prepare_fixture(EVAL_DIR / "fixtures", task.fixture, work)
        prompt = task.prompt.replace("{workdir}", str(workdir) if workdir else "")
        # Stored before the run is created: a rerun reuses it, never a new key (M1 §9 rule 1).
        entry.setdefault("idempotency_key", str(uuid.uuid4()))
        entry.setdefault("session_id", f"kairos-eval-{uuid.uuid4().hex[:12]}")
        save(results_path, results)
        try:
            outcome = run_task(api, task, prompt, entry["idempotency_key"], entry["session_id"], workdir)
        except Exception as err:  # noqa: BLE001 — record and go on with the next task
            entry.update(verdict="ERROR", failures=[str(err)])
            save(results_path, results)
            log(f"  {task.id:<32} ERROR {err}")
            continue
        failures = evaluate(task, outcome)
        entry.update(
            verdict="PASS" if not failures else "FAIL",
            failures=failures,
            status=outcome.status,
            approvals=outcome.approvals,
            output=outcome.output[:4000],
        )
        save(results_path, results)
        log(f"  {task.id:<32} {entry['verdict']}" + (f"  ({'; '.join(failures)})" if failures else ""))
    return results


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Kairos evaluation set against Hermes.")
    parser.add_argument("--base-url", default=DEFAULT_BASE)
    parser.add_argument("--task", action="append", help="run only this task id (repeatable)")
    parser.add_argument("--resume", type=Path, help="a results file to continue (reuses its keys)")
    parser.add_argument("--list", action="store_true", help="list the tasks and exit")
    args = parser.parse_args(argv)

    tasks = load_tasks(args.task)
    if args.list:
        for t in tasks:
            print(f"{t.id:<32} {t.use_case}")
        return 0

    if args.resume:
        results_path = args.resume
        results = json.loads(results_path.read_text(encoding="utf-8"))
    else:
        results_dir().mkdir(parents=True, exist_ok=True)
        results_path = results_dir() / f"{time.strftime('%Y%m%d-%H%M%S')}.json"
        results = {"started_at": time.strftime("%Y-%m-%d %H:%M:%S"), "base_url": args.base_url, "tasks": {}}

    api = HttpApi(args.base_url, read_key_from_credential_manager())
    print(f"Kairos evaluation: {len(tasks)} task(s); results in {results_path}")
    with tempfile.TemporaryDirectory(prefix="kairos-eval-") as work:
        run_all(api, tasks, results_path, results, Path(work))
    verdicts = [results["tasks"][t.id].get("verdict") for t in tasks]
    print(f"Passed {verdicts.count('PASS')} of {len(tasks)}.")
    return 0 if verdicts.count("PASS") == len(tasks) else 1
