"""The Kairos plugin for Hermes: checks every tool call against Kairos's rules and records it.

Installed by `scripts/install-plugin.ps1` as `%LOCALAPPDATA%\\hermes\\plugins\\kairos` and enabled with
`hermes plugins enable kairos`. Reads `%APPDATA%\\kairos\\policy.json`, writes
`%APPDATA%\\kairos\\audit.db` (D-015, D-016). `KAIROS_HOME` overrides the folder.
"""

from __future__ import annotations

import os
from pathlib import Path

from .audit import AuditStore
from .hooks import KairosHooks
from .policy import PolicyFile


def kairos_home() -> Path:
    override = os.environ.get("KAIROS_HOME")
    if override:
        return Path(override)
    return Path(os.environ.get("APPDATA") or Path.home() / "AppData" / "Roaming") / "kairos"


def register(ctx) -> KairosHooks:
    home = kairos_home()
    hooks = KairosHooks(PolicyFile(home / "policy.json"), AuditStore(home / "audit.db"))
    ctx.register_hook("pre_tool_call", hooks.pre_tool_call)
    ctx.register_hook("post_tool_call", hooks.post_tool_call)
    ctx.register_hook("post_approval_response", hooks.post_approval_response)
    return hooks
