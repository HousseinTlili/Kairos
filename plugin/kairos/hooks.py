"""The three Hermes hooks of the Kairos plugin (M2 spec §4), kept apart from `register(ctx)` so
they can be tested with recorded hook calls and no Hermes.

`pre_tool_call` must always return an answer: if anything inside it fails, it blocks.
"""

from __future__ import annotations

import hashlib
import json
import logging
from typing import Any, Mapping

from .audit import AuditStore
from .policy import Decision, PolicyFile, decide
from .redact import redact, redact_text

log = logging.getLogger("kairos.hooks")

SUMMARY_MAX = 300


def call_summary(tool_name: str, args: Mapping[str, Any] | None) -> str:
    """What the call does, for the approval card. Hermes shows a placeholder instead of the
    command for plugin approvals, so the real command travels in the message (redacted)."""
    args = args if isinstance(args, Mapping) else {}
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


def hook_result(decision: Decision, tool_name: str, args: Mapping[str, Any] | None) -> dict | None:
    """A Kairos decision as Hermes's pre_tool_call directive; None lets Hermes's own rules decide."""
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
        return {
            "action": "approve",
            "message": f"Kairos rule '{decision.rule_id}': {decision.message} — {call_summary(tool_name, args)}",
            "rule_key": approval_rule_key(decision.rule_id or "unnamed", tool_name, args),
        }
    return None  # allow, inherit


class KairosHooks:
    def __init__(self, policy_file: PolicyFile, audit: AuditStore):
        self.policy_file = policy_file
        self.audit = audit

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
            result = hook_result(decision, tool_name, args)
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
