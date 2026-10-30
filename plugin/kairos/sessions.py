"""Kairos's own conversations. The plugin's PC tools act on this PC (open and close apps, read its
notifications, capture its screen), so they work only in conversations started in Kairos, where the
Master sees the approval cards; a Telegram chat, the Hermes CLI or a scheduled job doesn't get
them. Herald OS, another project on Hermes, scopes its tools the same way (2026-10-30).

Kairos names every conversation it starts "kairos-…" (commands.rs checks it).
"""

from __future__ import annotations

import functools
from typing import Any, Callable

KAIROS_SESSION = "kairos-"

# The tools this plugin registers (pctools, notifications, screen).
KAIROS_TOOLS = frozenset(
    {"list_running_apps", "open_app", "close_app", "recent_notifications", "screen_capture"}
)


def is_kairos(session_id: object) -> bool:
    return isinstance(session_id, str) and session_id.startswith(KAIROS_SESSION)


def only_in_kairos(hook: Callable[..., Any]) -> Callable[..., Any]:
    """A pre_llm_call note that points the model at Kairos's tools: said only where they work."""

    @functools.wraps(hook)
    def wrapped(*args: Any, session_id: str = "", **kwargs: Any) -> Any:
        if not is_kairos(session_id):
            return None
        return hook(*args, session_id=session_id, **kwargs)

    return wrapped
