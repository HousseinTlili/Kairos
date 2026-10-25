"""The Kairos PC control tools (M3 §6, D-014): `list_running_apps`, `open_app`, `close_app`.

Everything that decides *what* happens lives here and is tested with a fake Windows layer; the
calls into Windows are in `winapps.py`. The M2 rules run before any of these handlers (Hermes
calls `pre_tool_call` first), and the handlers keep their own protections anyway: a window title
can hide a system process from a rule that only sees the name.

Handlers follow Hermes's tool contract: `(args, **kwargs) -> str` (JSON), and they never raise.
"""

from __future__ import annotations

import json
import logging
import os
import re
import time
from dataclasses import dataclass
from typing import Any, Callable, Mapping, Protocol, Sequence

log = logging.getLogger("kairos.pctools")

CATALOGUE_TTL = 300.0  # seconds; Get-StartApps takes about a second
CLOSE_WAIT = 5.0
FORCE_WAIT = 2.0
POLL = 0.2
TITLE_MAX = 120

# The same names as the `system-processes` rule (M2 §6), plus Kairos itself.
PROTECTED = frozenset(
    {"explorer", "winlogon", "csrss", "lsass", "services", "svchost", "smss", "wininit", "dwm",
     "system", "registry", "kairos"}
)

# Hosts the windows of Store apps (Calculator, Settings, …): closing one window is fine, killing
# the host closes every Store app at once.
STORE_APP_HOST = "applicationframehost"

# Used when the Start-menu catalogue can't be read. Targets are fixed strings, never user input.
BUILTIN_APPS: dict[str, str] = {
    "Notepad": "notepad.exe",
    "Calculator": "calc.exe",
    "Paint": "mspaint.exe",
    "File Explorer": "explorer.exe",
    "Task Manager": "taskmgr.exe",
    "Settings": "ms-settings:",
    "Snipping Tool": "snippingtool.exe",
}


@dataclass(frozen=True)
class Window:
    hwnd: int
    pid: int
    exe: str  # file name, e.g. "notepad.exe"
    title: str


class WindowsLayer(Protocol):
    def start_apps(self) -> list[tuple[str, str]]: ...  # (name, AppID), from Get-StartApps
    def launch(self, target: str) -> None: ...  # os.startfile
    def windows(self) -> list[Window]: ...  # visible top-level app windows
    def post_close(self, hwnd: int) -> None: ...  # WM_CLOSE
    def window_open(self, hwnd: int) -> bool: ...
    def terminate(self, pid: int) -> None: ...
    def process_running(self, pid: int) -> bool: ...


def base_name(name: str) -> str:
    """`Notepad.EXE` → `notepad`."""
    lower = name.strip().lower()
    return lower[:-4] if lower.endswith(".exe") else lower


def is_protected(exe: str) -> bool:
    return base_name(exe) in PROTECTED


def _norm(text: str) -> str:
    return re.sub(r"\s+", " ", text.strip().lower())


def match_name(query: str, names: Sequence[str]) -> tuple[str | None, list[str]]:
    """The one name `query` means, or None and the candidates: exact (ignoring case and spacing),
    then a unique prefix, then a unique substring. Several matches at a level are ambiguous."""
    q = _norm(query)
    if not q:
        return None, []
    normalised = [(name, _norm(name)) for name in names]
    for test in (
        lambda n: n == q,
        lambda n: n.startswith(q),
        lambda n: q in n,
    ):
        hits = sorted({name for name, n in normalised if test(n)}, key=str.lower)
        if len(hits) == 1:
            return hits[0], hits
        if len(hits) > 1:
            return None, hits
    return None, []


def close_names(query: str, names: Sequence[str], limit: int = 5) -> list[str]:
    """Names sharing a word with `query`, for a "did you mean" answer."""
    words = {w for w in re.split(r"\W+", _norm(query)) if len(w) >= 3}
    found = [n for n in names if words & set(re.split(r"\W+", _norm(n)))]
    return sorted(found, key=str.lower)[:limit]


def _result(**fields: Any) -> str:
    return json.dumps(fields, ensure_ascii=False)


def _error(message: str, **fields: Any) -> str:
    return _result(error=message, **fields)


class PcTools:
    def __init__(
        self,
        layer: WindowsLayer | None,
        clock: Callable[[], float] = time.monotonic,
        sleep: Callable[[float], None] = time.sleep,
        own_pid: int | None = None,
    ):
        self.layer = layer
        self.clock = clock
        self.sleep = sleep
        self.own_pid = os.getpid() if own_pid is None else own_pid
        self._catalogue: tuple[float, dict[str, str]] | None = None

    # --- open_app ---------------------------------------------------------------------------

    def catalogue(self) -> dict[str, str]:
        """App name → launch target. Start-menu apps (desktop and Store), else the built-in list."""
        now = self.clock()
        if self._catalogue and now - self._catalogue[0] < CATALOGUE_TTL:
            return self._catalogue[1]
        apps: dict[str, str] = {}
        try:
            for name, app_id in self.layer.start_apps():
                if name and app_id:
                    apps.setdefault(name.strip(), "shell:AppsFolder\\" + app_id.strip())
        except Exception:  # noqa: BLE001 — fall back to the built-in list
            log.exception("kairos: couldn't read the Start menu apps")
        for name, target in BUILTIN_APPS.items():
            if not any(_norm(n) == _norm(name) for n in apps):
                apps[name] = target
        self._catalogue = (now, apps)
        return apps

    def open_app(self, args: Mapping[str, Any] | None = None, **_: Any) -> str:
        try:
            name = (args or {}).get("name")
            if not isinstance(name, str) or not name.strip():
                return _error("Say which app to open, e.g. {\"name\": \"Notepad\"}.")
            if self.layer is None:
                return _error("Kairos PC control works on Windows only.")
            apps = self.catalogue()
            chosen, candidates = match_name(name, list(apps))
            if chosen is None and candidates:
                return _error(
                    f"Several apps match '{name}'. Ask the user which one, then call open_app with its exact name.",
                    candidates=candidates[:10],
                )
            if chosen is None:
                return _error(
                    f"No app called '{name}' is in the Start menu.",
                    similar=close_names(name, list(apps)),
                )
            self.layer.launch(apps[chosen])
            return _result(opened=chosen)
        except Exception as err:  # noqa: BLE001 — a tool never raises into Hermes
            log.exception("kairos: open_app failed")
            return _error(f"Couldn't open the app ({type(err).__name__}).")

    # --- list_running_apps ------------------------------------------------------------------

    def _app_windows(self) -> list[Window]:
        return [w for w in self.layer.windows() if not is_protected(w.exe) and w.pid != self.own_pid]

    def list_running_apps(self, args: Mapping[str, Any] | None = None, **_: Any) -> str:
        try:
            if self.layer is None:
                return _error("Kairos PC control works on Windows only.")
            apps: dict[int, dict[str, Any]] = {}
            for window in self._app_windows():
                entry = apps.setdefault(
                    window.pid,
                    {"name": window.exe, "title": window.title[:TITLE_MAX], "pid": window.pid, "windows": 0},
                )
                entry["windows"] += 1
            listed = sorted(apps.values(), key=lambda a: (a["name"].lower(), a["pid"]))
            return _result(apps=listed)
        except Exception as err:  # noqa: BLE001
            log.exception("kairos: list_running_apps failed")
            return _error(f"Couldn't list the running apps ({type(err).__name__}).")

    # --- close_app --------------------------------------------------------------------------

    def resolve(self, name: str) -> tuple[list[Window], str | None]:
        """The windows `name` refers to, or an explanation. By executable first, then by a window
        title containing the text, which must belong to a single executable."""
        everything = self.layer.windows()
        wanted = base_name(name)
        by_exe = [w for w in everything if base_name(w.exe) == wanted]
        if by_exe:
            return by_exe, None
        needle = _norm(name)
        by_title = [w for w in everything if needle and needle in _norm(w.title)]
        exes = sorted({base_name(w.exe) for w in by_title})
        if len(exes) > 1:
            return [], f"'{name}' matches windows of several apps ({', '.join(exes)}). Ask the user which one."
        if not by_title:
            return [], f"No open app matches '{name}'. Use list_running_apps to see what is open."
        return by_title, None

    def close_app(self, args: Mapping[str, Any] | None = None, **_: Any) -> str:
        try:
            args = args or {}
            name = args.get("name")
            force = args.get("force") is True
            if not isinstance(name, str) or not name.strip():
                return _error("Say which app to close, e.g. {\"name\": \"notepad\"}.")
            if self.layer is None:
                return _error("Kairos PC control works on Windows only.")
            targets, problem = self.resolve(name)
            if problem:
                return _error(problem)
            exes = sorted({w.exe for w in targets}, key=str.lower)
            if any(is_protected(w.exe) for w in targets):
                return _error(
                    f"'{name}' is a Windows system process ({', '.join(exes)}); Kairos never closes those. "
                    "Tell the user instead of trying another way."
                )
            if any(w.pid == self.own_pid for w in targets):
                return _error("That is the assistant's own process; it can't close itself.")
            if force and any(base_name(w.exe) == STORE_APP_HOST for w in targets):
                return _error(
                    "That is a Store app; force-closing it would close every Store app. Try again without force."
                )
            return self._force_close(targets, exes) if force else self._close(targets, exes)
        except Exception as err:  # noqa: BLE001
            log.exception("kairos: close_app failed")
            return _error(f"Couldn't close the app ({type(err).__name__}).")

    def _wait(self, still_there: Callable[[], list[Any]], seconds: float) -> list[Any]:
        deadline = self.clock() + seconds
        remaining = still_there()
        while remaining and self.clock() < deadline:
            self.sleep(POLL)
            remaining = still_there()
        return remaining

    def _close(self, targets: list[Window], exes: list[str]) -> str:
        for window in targets:
            self.layer.post_close(window.hwnd)
        open_windows = self._wait(lambda: [w for w in targets if self.layer.window_open(w.hwnd)], CLOSE_WAIT)
        if open_windows:
            return _result(
                status="still_open",
                apps=exes,
                message=(
                    "Asked the app to close, but it is still open; it may be asking to save. "
                    "Tell the user. Only use force if the user says so: unsaved work would be lost."
                ),
                titles=sorted({w.title[:TITLE_MAX] for w in open_windows}),
            )
        return _result(status="closed", apps=exes, windows=len(targets))

    def _force_close(self, targets: list[Window], exes: list[str]) -> str:
        pids = sorted({w.pid for w in targets})
        for pid in pids:
            self.layer.terminate(pid)
        running = self._wait(lambda: [p for p in pids if self.layer.process_running(p)], FORCE_WAIT)
        if running:
            return _result(status="still_open", apps=exes, message="Windows didn't end the app.", pids=running)
        return _result(status="closed", apps=exes, forced=True, processes=len(pids))


# --- Tool schemas (what the model sees) ----------------------------------------------------------

STEER = "Use this instead of terminal or PowerShell commands to"

LIST_RUNNING_APPS = {
    "name": "list_running_apps",
    "description": f"List the apps that have a window open on this PC. {STEER} see what is open.",
    "parameters": {"type": "object", "properties": {}},
}

OPEN_APP = {
    "name": "open_app",
    "description": (
        f"Open an app installed on this Windows PC by its Start-menu name (for example 'Notepad', "
        f"'Calculator', 'Spotify'). {STEER} open apps. Takes only a name: no paths or arguments."
    ),
    "parameters": {
        "type": "object",
        "properties": {"name": {"type": "string", "description": "The app's name as in the Start menu."}},
        "required": ["name"],
    },
}

CLOSE_APP = {
    "name": "close_app",
    "description": (
        f"Close an open app, as clicking its X would. {STEER} close apps. The user is asked first. "
        "Windows system processes can't be closed. If the app is still open afterwards (often a "
        "'save changes?' prompt), tell the user; use force only if the user asks for it."
    ),
    "parameters": {
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "The app's program name (for example 'notepad' or 'chrome.exe') or text from its window title.",
            },
            "force": {
                "type": "boolean",
                "description": "End the app without letting it save. Only when the user explicitly asks.",
            },
        },
        "required": ["name"],
    },
}
