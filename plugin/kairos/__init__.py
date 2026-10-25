"""The Kairos plugin for Hermes: checks every tool call against Kairos's rules and records it.

Installed by `scripts/install-plugin.ps1` as `%LOCALAPPDATA%\\hermes\\plugins\\kairos` and enabled with
`hermes plugins enable kairos`. Reads `%APPDATA%\\kairos\\policy.json`, writes
`%APPDATA%\\kairos\\audit.db` (D-015, D-016). `KAIROS_HOME` overrides the folder.

From M3 it also registers the PC control tools (`pctools.py`, D-014) in its own `kairos` toolset,
and from M4 `recent_notifications` (`notifications.py`, D-018).
"""

from __future__ import annotations

import logging
import os
import sys
from pathlib import Path

from .audit import AuditStore
from . import notifications
from .hooks import KairosHooks
from .pctools import CLOSE_APP, LIST_RUNNING_APPS, OPEN_APP, PcTools, steering
from .policy import PolicyFile

log = logging.getLogger("kairos")


def kairos_home() -> Path:
    override = os.environ.get("KAIROS_HOME")
    if override:
        return Path(override)
    return Path(os.environ.get("APPDATA") or Path.home() / "AppData" / "Roaming") / "kairos"


def pc_tools() -> PcTools:
    if sys.platform != "win32":
        return PcTools(None)
    from .winapps import Win32Layer

    return PcTools(Win32Layer())


def register_tools(ctx) -> None:
    """The PC control tools. Hidden from the model off Windows; a failure here never costs the
    hooks, which are what keep every other tool call in check."""
    register_tool = getattr(ctx, "register_tool", None)
    if register_tool is None:
        log.warning("kairos: this Hermes can't register plugin tools; PC control is off")
        return
    try:
        tools = pc_tools()
        on_windows = lambda: sys.platform == "win32"  # noqa: E731
        for schema, handler in (
            (LIST_RUNNING_APPS, tools.list_running_apps),
            (OPEN_APP, tools.open_app),
            (CLOSE_APP, tools.close_app),
        ):
            register_tool(name=schema["name"], toolset="kairos", schema=schema, handler=handler, check_fn=on_windows)
        # Only where the tools exist: pointing the model at tools it can't see would mislead it.
        if on_windows():
            ctx.register_hook("pre_llm_call", steering)
    except Exception:  # noqa: BLE001
        log.exception("kairos: couldn't register the PC control tools")


def register_notifications(ctx, home: Path) -> None:
    """`recent_notifications` and its steering (M4 §8, D-018). Windows only, like the
    notifications themselves; a failure here never costs the hooks."""
    register_tool = getattr(ctx, "register_tool", None)
    if register_tool is None:
        return
    try:
        reader = notifications.Notifications(home / notifications.FILE_NAME)
        on_windows = lambda: sys.platform == "win32"  # noqa: E731
        schema = notifications.RECENT_NOTIFICATIONS
        register_tool(
            name=schema["name"], toolset="kairos", schema=schema, handler=reader.recent_notifications, check_fn=on_windows
        )
        if on_windows():
            ctx.register_hook("pre_llm_call", notifications.steering)
    except Exception:  # noqa: BLE001
        log.exception("kairos: couldn't register recent_notifications")


def register(ctx) -> KairosHooks:
    home = kairos_home()
    hooks = KairosHooks(PolicyFile(home / "policy.json"), AuditStore(home / "audit.db"))
    ctx.register_hook("pre_tool_call", hooks.pre_tool_call)
    ctx.register_hook("post_tool_call", hooks.post_tool_call)
    ctx.register_hook("post_approval_response", hooks.post_approval_response)
    register_tools(ctx)
    register_notifications(ctx, home)
    return hooks
