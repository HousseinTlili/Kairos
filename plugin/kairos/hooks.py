"""The three Hermes hooks of the Kairos plugin (M2 spec §4), kept apart from `register(ctx)` so
they can be tested with recorded hook calls and no Hermes.

`pre_tool_call` must always return an answer: if anything inside it fails, it blocks.
"""

from __future__ import annotations

import hashlib
import json
import logging
import re
from collections import OrderedDict
from typing import Any, Mapping
from urllib.parse import urlsplit

from .audit import AuditStore
from .policy import Decision, PolicyFile, decide
from .redact import redact, redact_text

log = logging.getLogger("kairos.hooks")

SUMMARY_MAX = 300
# A browser script is shown whole when it can be: its last line may be the click that matters.
SCRIPT_MAX = 1500

# --- Browser scripts in words (M12 Step 4) -------------------------------------------------------
#
# In Hermes's Browser Use mode every page action is Python for browser_exec, which the card shows
# as code; these say what it will do. Typed text is shown except for fields that look secret.

_ARG = r"""\s*(?:\w+\s*=\s*)?[rbuf]{0,2}(['"])(.*?)\1"""
_NUM = r"\s*(?:\w+\s*=\s*)?(-?\d+(?:\.\d+)?)"
_SECRET_FIELD = re.compile(r"password|passwd|pwd|passcode|cvc|cvv|csc|card|iban|secret|token|otp", re.I)
_SCRIPT_STEPS: tuple[tuple[re.Pattern[str], Any], ...] = (
    (re.compile(r"\b(?:goto_url|new_tab)\s*\(" + _ARG), lambda m: f"open {_host(m.group(2))}"),
    (re.compile(r"\b(?:click_at_xy|click)\s*\(" + _NUM + r"\s*," + _NUM), lambda m: f"click at {m.group(1)}, {m.group(2)}"),
    (
        re.compile(r"\bfill_input\s*\(" + _ARG + r"\s*," + _ARG),
        lambda m: f"type {_typed(m.group(4), m.group(2))} into {m.group(2)}",
    ),
    (re.compile(r"\btype_text\s*\(" + _ARG), lambda m: f"type {_typed(m.group(2), '')}"),
    (re.compile(r"\bpress_key\s*\(" + _ARG), lambda m: f"press {m.group(2)}"),
    (re.compile(r"\bdispatch_key\s*\(" + _ARG + r"(?:\s*," + _ARG + r")?"), lambda m: f"press {m.group(4) or 'Enter'} in {m.group(2)}"),
    (re.compile(r"\bupload_file\s*\(" + _ARG + r"\s*," + _ARG), lambda m: f"upload {m.group(4)}"),
    (re.compile(r"\.(?:submit|requestSubmit)\s*\("), lambda m: "submit a form by script"),
    (re.compile(r"\.click\s*\("), lambda m: "click an element by script"),
    (re.compile(r"\.(?:value|checked|selected|selectedIndex)\s*=(?!=)"), lambda m: "change a field by script"),
    (re.compile(r"\bdocument\.cookie\b|\b(?:localStorage|sessionStorage|indexedDB)\b"), lambda m: "read cookies or site storage"),
    (re.compile(r"\b(?:fetch|sendBeacon)\s*\(|\b(?:XMLHttpRequest|WebSocket)\b"), lambda m: "send a request from the page"),
    (re.compile(r"\b(?:cdp|_send)\s*\("), lambda m: "send a raw browser command"),
)


def _host(url: str) -> str:
    return urlsplit(url).hostname or url[:60]


def _typed(text: str, field: str) -> str:
    if _SECRET_FIELD.search(field):
        return "a hidden value"
    shown = redact_text(text)
    return f"«{shown[:40]}…»" if len(shown) > 40 else f"«{shown}»"


def describe_script(code: str, last_host: str | None = None) -> str:
    """What a browser script will do, in order: "open httpbin.org; click at 412, 380; press Enter".
    `last_host` is where the conversation's browser was left, for a script that opens nothing."""
    found: list[tuple[int, str]] = []
    for pattern, say in _SCRIPT_STEPS:
        for match in pattern.finditer(code):
            found.append((match.start(), say(match)))
    found.sort(key=lambda item: item[0])
    steps: list[str] = []
    for _, step in found:
        if step not in steps:
            steps.append(step)
    if not steps:
        return ""
    words = "; ".join(steps[:6]) + ("; …" if len(steps) > 6 else "")
    if last_host and not steps[0].startswith("open "):
        words += f", on {last_host}"
    # " — " separates the parts of the plugin's message (turn.ts reads it).
    return words.replace(" — ", " - ")


def script_host(code: str) -> str | None:
    """The last site a browser script opens, if any."""
    urls = re.findall(r"\b(?:goto_url|new_tab)\s*\(" + _ARG, code)
    return _host(urls[-1][1]) if urls else None


def call_summary(tool_name: str, args: Mapping[str, Any] | None) -> str:
    """What the call does, for the approval card. Hermes shows a placeholder instead of the
    command for plugin approvals, so the real command travels in the message (redacted)."""
    args = args if isinstance(args, Mapping) else {}
    if tool_name == "browser_exec" and isinstance(args.get("code"), str):
        return redact_text(args["code"])[:SCRIPT_MAX]
    for key in ("command", "code", "path", "file_path", "url"):
        value = args.get(key)
        if isinstance(value, str) and value:
            return redact_text(value)[:SUMMARY_MAX]
    urls = args.get("urls")
    if isinstance(urls, list) and urls:
        return redact_text(", ".join(str(u) for u in urls))[:SUMMARY_MAX]
    return json.dumps(redact(args), ensure_ascii=False, default=str)[:SUMMARY_MAX]


def approval_rule_key(rule_id: str, tool_name: str, args: Mapping[str, Any] | None) -> str:
    """Hermes's "Always allow" remembers approvals by this key. One key per rule would let a single
    "always" approve every future match of the rule (every rm -rf), so the key also covers the
    exact tool and arguments: "always" then means "always for this exact call"."""
    canonical = json.dumps({"tool": tool_name, "args": args or {}}, sort_keys=True, default=str)
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()[:16]
    return f"kairos:{rule_id}:{digest}"


def hook_result(
    decision: Decision, tool_name: str, args: Mapping[str, Any] | None, last_host: str | None = None
) -> dict | None:
    """A Kairos decision as Hermes's pre_tool_call directive; None lets Hermes's own rules decide.
    A browser script's ask also says, on a "Will:" line, what it will do (turn.ts reads it)."""
    if decision.action == "invalid_rules":
        return {"action": "block", "message": f"{decision.message} Fix them in Kairos ▸ Settings ▸ Rules."}
    if decision.action == "deny":
        return {
            "action": "block",
            "message": (
                f"Blocked by Kairos rule '{decision.rule_id}': {decision.message} "
                "Do not retry this or try to reach the same result another way; tell the user instead."
            ),
        }
    if decision.action == "ask":
        reason = decision.message
        code = args.get("code") if isinstance(args, Mapping) else None
        if tool_name == "browser_exec" and isinstance(code, str):
            words = describe_script(code, last_host)
            if words:
                reason = f"{reason}\nWill: {words}"
        return {
            "action": "approve",
            "message": f"Kairos rule '{decision.rule_id}': {reason} — {call_summary(tool_name, args)}",
            "rule_key": approval_rule_key(decision.rule_id or "unnamed", tool_name, args),
        }
    return None  # allow, inherit


class KairosHooks:
    def __init__(self, policy_file: PolicyFile, audit: AuditStore):
        self.policy_file = policy_file
        self.audit = audit
        # Where each conversation's browser was last sent (M12 Step 4), for the card's "on …".
        self.browser_hosts: OrderedDict[str, str] = OrderedDict()

    def _last_host(self, session: str, args: Mapping[str, Any] | None) -> str | None:
        known = self.browser_hosts.get(session)
        code = args.get("code") if isinstance(args, Mapping) else None
        opened = script_host(code) if isinstance(code, str) else None
        if opened:
            self.browser_hosts[session] = opened
            self.browser_hosts.move_to_end(session)
            while len(self.browser_hosts) > 64:
                self.browser_hosts.popitem(last=False)
        return known

    def pre_tool_call(
        self,
        tool_name: str = "",
        args: Mapping[str, Any] | None = None,
        task_id: str = "",
        session_id: str = "",
        tool_call_id: str = "",
        **_: Any,
    ) -> dict | None:
        try:
            decision = decide(self.policy_file.current(), tool_name, args)
        except Exception as err:  # noqa: BLE001 — fail closed: a broken check never lets a call through
            log.exception("kairos: rules check failed for %s", tool_name)
            decision = Decision("invalid_rules", None, f"Kairos couldn't check its rules ({type(err).__name__}).")
        try:
            last_host = self._last_host(session_id or task_id, args) if tool_name == "browser_exec" else None
            result = hook_result(decision, tool_name, args, last_host)
        except Exception:  # noqa: BLE001
            log.exception("kairos: couldn't build the hook result for %s", tool_name)
            result = {"action": "block", "message": "Kairos couldn't check this action, so it was blocked."}
        # Recorded after deciding: an audit failure can change the record, never the decision.
        self.audit.record_decision(
            session_id=session_id or task_id or None,
            tool_call_id=tool_call_id or None,
            tool_name=tool_name,
            args=args,
            decision=decision,
        )
        return result

    def post_tool_call(
        self,
        tool_call_id: str = "",
        result: Any = None,
        duration_ms: int | None = None,
        status: str | None = None,
        error_type: str | None = None,
        **_: Any,
    ) -> None:
        self.audit.record_result(
            tool_call_id=tool_call_id or None,
            status=status,
            duration_ms=duration_ms,
            result=result,
            error_type=error_type,
        )

    def post_approval_response(
        self,
        command: str | None = None,
        description: str | None = None,
        pattern_key: str | None = None,
        session_key: str | None = None,
        choice: str = "",
        decided_by: str | None = None,
        **_: Any,
    ) -> None:
        self.audit.record_approval(
            session_key=session_key,
            command=command,
            description=description,
            pattern_key=pattern_key,
            choice=choice or "unknown",
            decided_by=decided_by,
        )
