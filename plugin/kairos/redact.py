"""Removes secrets from tool arguments and results before they are written anywhere (M2 spec §7).

Two layers: values stored under secret-sounding keys are dropped whole, and secret-shaped text is
replaced wherever it appears. It errs towards redacting: a lost detail in the timeline is
acceptable, a leaked key is not.
"""

from __future__ import annotations

import re
from typing import Any

REDACTED = "[redacted]"

# A dict key whose value is a secret, whatever the value looks like.
SECRET_KEY = re.compile(
    r"(api[_-]?key|access[_-]?key|private[_-]?key|secret|token|passw(or)?d|passphrase|"
    r"authorization|cookie|credential|^key$|^pwd$)",
    re.IGNORECASE,
)

# Text that is a secret wherever it appears. Order matters: whole blocks before their parts.
SECRET_TEXT = [
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(-----END [A-Z ]*PRIVATE KEY-----|\Z)", re.DOTALL),
    re.compile(r"\bBearer\s+[A-Za-z0-9._~+/-]+=*", re.IGNORECASE),
    re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"),  # JWT
    re.compile(r"\bsk-(ant-|proj-|or-v1-)?[A-Za-z0-9_-]{16,}"),  # OpenAI, Anthropic, OpenRouter
    re.compile(r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}"),  # GitHub tokens
    re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"),
    re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),  # Slack
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),  # AWS access key id
    re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b"),  # Google API key
    re.compile(r"\bhf_[A-Za-z0-9]{30,}\b"),  # Hugging Face
    # 32+ hex characters (Hermes's API_SERVER_KEY is 64), except exactly 40: a git commit id.
    re.compile(r"(?<![0-9A-Fa-f])(?:[0-9A-Fa-f]{32,39}|[0-9A-Fa-f]{41,})(?![0-9A-Fa-f])"),
]

# NAME=value or NAME: value where NAME sounds secret (.env lines, PowerShell $env:X = "...").
SECRET_ASSIGNMENT = re.compile(
    r"(?P<name>\b[A-Za-z0-9_]*(KEY|TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL)[A-Za-z0-9_]*)"
    r"(?P<sep>\s*[:=]\s*)(?P<value>\"[^\"]*\"|'[^']*'|[^\s\"',;]+)",
    re.IGNORECASE,
)


# Counts, not secrets: `input_tokens: 12368`, `max_tokens=400`, `token_count: 5`.
TOKEN_COUNT = re.compile(r"(tokens|token_count)$", re.IGNORECASE)


def _assignment(match: re.Match[str]) -> str:
    if TOKEN_COUNT.search(match.group("name")) and match.group("value").isdigit():
        return match.group(0)
    return f"{match.group('name')}{match.group('sep')}{REDACTED}"


def redact_text(text: str) -> str:
    text = SECRET_ASSIGNMENT.sub(_assignment, text)
    for pattern in SECRET_TEXT:
        text = pattern.sub(REDACTED, text)
    return text


def redact(value: Any, *, max_string: int | None = None) -> Any:
    """A copy of `value` with secrets removed and, if given, every string cut to `max_string`."""
    if isinstance(value, dict):
        return {
            k: REDACTED if isinstance(k, str) and SECRET_KEY.search(k) else redact(v, max_string=max_string)
            for k, v in value.items()
        }
    if isinstance(value, (list, tuple)):
        return [redact(v, max_string=max_string) for v in value]
    if isinstance(value, str):
        cleaned = redact_text(value)
        if max_string is not None and len(cleaned) > max_string:
            cleaned = cleaned[:max_string] + f"… [{len(cleaned) - max_string} more characters]"
        return cleaned
    return value
