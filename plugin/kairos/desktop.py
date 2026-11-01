"""Where a `computer_use` call is about to act (M20, D-040).

Clicking and typing go round every rule that reads a tool's arguments, so the rules need to know
the target window, and the call often doesn't say: `click(element=12)` goes to whichever app Hermes's
backend last captured. This works it out, per conversation, from the calls the plugin sees, and says
so when it can't: input it can't place is refused (hooks.py).

It sees the calls, not the driver. A dialog that opens after a click can differ from what's
remembered, and that limit is stated to the Master on the switch's page.

Standard library only, and no Windows calls here: the window list comes in, so it's tested without
Windows.
"""

from __future__ import annotations

import json
import re
from collections import OrderedDict
from dataclasses import dataclass, field
from typing import Any, Callable, Mapping, Sequence

from .pctools import Window

# Actions with no window of their own: `list_*` read the whole desktop and have their own rule.
NO_TARGET_ACTIONS = frozenset({"list_apps", "list_windows", "wait"})
# Actions that choose the target, rather than use the one chosen.
CHOOSING_ACTIONS = frozenset({"capture", "focus_app"})
# Hermes's words for the whole screen and the shell: rules refuse them by these names.
WHOLE_SCREEN = frozenset({"screen", "desktop"})

_REMEMBERED = 64
_MIN_QUERY = 2
_LABEL_MAX = 80


@dataclass(frozen=True)
class Placed:
    """Where one call acts. `known` False means it can't be told."""

    known: bool
    targets: tuple[str, ...] = ()  # lower-case process names and titles, for the `targets` match kind
    label: str = ""  # for the card: the window's title, or the process
    element: str = ""  # what the app calls the element being acted on, if known
    key: str = ""  # one per app, for a grant to see it


@dataclass
class _Conversation:
    query: str | None = None
    elements: dict[int, str] = field(default_factory=dict)


def _action(args: Mapping[str, Any]) -> str:
    return str(args.get("action") or "").strip().lower()


def _stem(name: str) -> str:
    """`%windir%\\system32\\charmap.exe` and `CharMap.exe` both mean `charmap`."""
    base = re.split(r"[\\/]", name.strip().strip("\"'"))[-1].lower()
    return base[:-4] if base.endswith(".exe") else base


def _clean(text: str) -> str:
    return " ".join(text.split())[:_LABEL_MAX]


def _result_data(result: Any) -> Mapping[str, Any] | None:
    """A tool result as an object: JSON text, or Hermes's multimodal form with the JSON in a text part."""
    if isinstance(result, str):
        try:
            data = json.loads(result)
        except ValueError:
            return None
        return data if isinstance(data, Mapping) else None
    if isinstance(result, Mapping):
        if "elements" in result or "ok" in result:
            return result
        for part in result.get("content") or ():
            found = _result_data(part.get("text")) if isinstance(part, Mapping) else None
            if found is not None:
                return found
    return None


def _succeeded(action: str, data: Mapping[str, Any]) -> bool:
    if action == "capture":
        # A capture that found nothing has width 0 and a message where the title would be (fixture).
        return bool(data.get("width")) or bool(data.get("elements"))
    return data.get("ok") is True


class DesktopTracker:
    def __init__(self, windows: Callable[[], Sequence[Window]], foreground: Callable[[], Window | None]):
        self._windows = windows
        self._foreground = foreground
        self._conversations: OrderedDict[str, _Conversation] = OrderedDict()
        self._pending: OrderedDict[str, tuple[str, str, str | None]] = OrderedDict()

    def _conversation(self, session: str) -> _Conversation:
        found = self._conversations.setdefault(session, _Conversation())
        self._conversations.move_to_end(session)
        while len(self._conversations) > _REMEMBERED:
            self._conversations.popitem(last=False)
        return found

    def _matching(self, query: str) -> list[Window]:
        stem = _stem(query)
        if len(stem) < _MIN_QUERY:
            return []
        return [
            w
            for w in self._windows()
            if (w.exe and (stem in _stem(w.exe) or _stem(w.exe) in stem)) or stem in w.title.lower()
        ]

    @staticmethod
    def _place(found: Sequence[Window], element: str = "") -> Placed:
        if not found:
            return Placed(False)
        names: list[str] = []
        for window in found:
            names += [window.exe.lower(), window.title.lower()]
        first = found[0]
        return Placed(
            True,
            tuple(dict.fromkeys(n for n in names if n)),
            _clean(first.title or first.exe),
            element,
            _stem(first.exe),
        )

    def _by_id(self, args: Mapping[str, Any]) -> list[Window] | None:
        pid, window_id = args.get("pid"), args.get("window_id")
        if pid is None and window_id is None:
            return None
        return [
            w
            for w in self._windows()
            if (window_id is not None and w.hwnd == window_id) or (window_id is None and w.pid == pid)
        ]

    def before(self, session: str, tool_call_id: str, args: Mapping[str, Any] | None) -> Placed:
        """Where this call acts, remembering what it would choose until `after` says it worked."""
        args = args if isinstance(args, Mapping) else {}
        action = _action(args)
        if action in NO_TARGET_ACTIONS:
            return Placed(True)
        conversation = self._conversation(session)
        app = args.get("app")
        query = app.strip() if isinstance(app, str) and app.strip() else None
        chooses = action in CHOOSING_ACTIONS

        if query is not None and query.lower() in WHOLE_SCREEN:
            return Placed(True, (query.lower(),), query.lower(), key=query.lower())

        by_id = self._by_id(args)
        element = ""
        if by_id is not None:
            found = by_id
            query = found[0].exe if found else None
        elif query is not None:
            found = self._matching(query)
        elif action == "capture":
            # No app named means the frontmost window, which is whatever the Master is doing.
            front = self._foreground()
            found = [front] if front is not None else []
            query = front.exe if front is not None else None
        elif chooses or conversation.query is None:
            found = []
        else:
            found = self._matching(conversation.query)

        if not chooses:
            index = args.get("element")
            if isinstance(index, int):
                element = conversation.elements.get(index, "")

        if chooses and query is not None and found:
            self._pending[tool_call_id or f"{session}:{action}"] = (session, action, query)
            while len(self._pending) > _REMEMBERED:
                self._pending.popitem(last=False)
        if not found and query is not None:
            # Nothing open by that name (a hidden window isn't listed): still refused by the name it was
            # asked for, if that's off limits, and otherwise because it can't be placed.
            return Placed(False, (query.lower(), f"{_stem(query)}.exe"))
        return self._place(found, element)

    def after(self, tool_call_id: str, session: str, result: Any) -> None:
        """Learns from a finished call: a capture or focus that worked sets the target, and any result
        with elements replaces the names of the last ones (their numbers change between captures)."""
        pending = self._pending.pop(tool_call_id, None) if tool_call_id else None
        data = _result_data(result)
        if data is None:
            return
        if pending is not None:
            session, action, query = pending
            if _succeeded(action, data):
                self._conversation(session).query = query
        if session and isinstance(data.get("elements"), list):
            names: dict[int, str] = {}
            for item in data["elements"]:
                if isinstance(item, Mapping) and isinstance(item.get("index"), int):
                    names[item["index"]] = _clean(str(item.get("label") or item.get("role") or ""))
            self._conversation(session).elements = names
