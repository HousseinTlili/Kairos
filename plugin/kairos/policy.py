"""Kairos rules: loading, validating and evaluating `policy.json` (M2 spec §5).

Pure Python, standard library only, no Hermes imports, so it can be tested on its own with
Hermes's bundled interpreter. The plugin (`__init__.py`) turns a `Decision` into a hook result.

Matching: a rule applies to a call when the tool is in `tools` and every condition kind present in
`match` holds; a condition kind holds when any one of its patterns matches. Rules are checked in
order and the first enabled match wins. Anything unmatched is `inherit`: Hermes's own rules decide.
"""

from __future__ import annotations

import json
import os
import re
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Mapping

ACTIONS = ("allow", "deny", "ask")
MATCH_KINDS = ("paths", "commands", "urls", "any_text")
RULE_FIELDS = {"id", "description", "tools", "match", "action", "enabled"}
TOP_FIELDS = {"version", "default", "rules"}
RULE_ID = re.compile(r"^[a-z0-9][a-z0-9-]*$")

# Argument names that hold a path, across Hermes's file-like tools.
PATH_KEYS = ("path", "file_path", "target_path", "source", "destination", "dest", "cwd", "workdir")
# Tools whose text is a shell command or a script. `browser_exec` runs arbitrary Python on this
# machine through the browser-use helper (Hermes's tools/browser_use_cli.py) and isn't covered by
# Hermes's own command check, so it is read like a script (M2 Step 9).
COMMAND_TOOLS = {"terminal": "command", "execute_code": "code", "browser_exec": "code"}
URL_KEYS = ("url", "urls")
_URL = re.compile(r"https?://[^\s\"'<>]+", re.IGNORECASE)
# Splits a command into candidate path tokens: whitespace, quotes, and shell separators.
_TOKEN_SPLIT = re.compile(r"[\s\"'`;|&<>()=,]+")


class PolicyError(ValueError):
    """`policy.json` can't be used. Every tool call is blocked until it is fixed (fail closed)."""


@dataclass(frozen=True)
class Decision:
    """What Kairos decided for one tool call."""

    action: str  # allow | deny | ask | inherit | invalid_rules
    rule_id: str | None = None
    message: str | None = None

    @property
    def blocks(self) -> bool:
        return self.action in ("deny", "invalid_rules")


INHERIT = Decision("inherit")


# --- Normalisation -------------------------------------------------------------------------------


def _expand(text: str, env: Mapping[str, str]) -> str:
    """Expands `~`, `%VAR%`, `$VAR` and `${VAR}` the way cmd, PowerShell and Git Bash would, so a
    rule written with `%APPDATA%` also catches a Git Bash command using `$APPDATA`."""
    home = env.get("USERPROFILE") or env.get("HOME") or ""

    def lookup(match: re.Match[str]) -> str:
        name = match.group(1).upper()
        if name == "HOME" and home:
            return home
        return env.get(name, match.group(0))

    text = re.sub(r"%([A-Za-z_][A-Za-z0-9_]*)%", lookup, text)
    text = re.sub(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}", lookup, text)
    text = re.sub(r"\$env:([A-Za-z_][A-Za-z0-9_]*)", lookup, text, flags=re.IGNORECASE)
    text = re.sub(r"\$([A-Za-z_][A-Za-z0-9_]*)", lookup, text)
    if home:
        text = re.sub(r"(?<![\w.])~(?=[/\\]|$)", lambda _: home, text)
    return text


def normalise_path(text: str, env: Mapping[str, str]) -> str:
    """Windows paths compare case-insensitively and with either slash; Git Bash writes `/c/...`."""
    text = _expand(text.strip().strip("\"'"), env).replace("\\", "/").lower()
    text = re.sub(r"^/([a-z])/", r"\1:/", text)  # /c/Users -> c:/users
    return re.sub(r"/{2,}", "/", text)


def _glob_to_regex(pattern: str) -> re.Pattern[str]:
    """`**` crosses directories, `*` and `?` don't. Matched against the whole normalised text."""
    out = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("/**", i) and i + 3 == len(pattern):
            out.append("(?:/.*)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out), re.DOTALL)


# --- Text extraction per tool family (spec §5 "Where text comes from") --------------------------


def _strings(value: object) -> Iterable[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, Mapping):
        for item in value.values():
            yield from _strings(item)
    elif isinstance(value, (list, tuple)):
        for item in value:
            yield from _strings(item)


def _tokens(text: str) -> list[str]:
    return [t for t in _TOKEN_SPLIT.split(text) if t]


def _command_text(tool: str, args: Mapping[str, object]) -> list[str]:
    key = COMMAND_TOOLS.get(tool)
    value = args.get(key) if key else None
    return [value] if isinstance(value, str) else []


def _path_candidates(tool: str, args: Mapping[str, object]) -> list[str]:
    found = [v for k in PATH_KEYS for v in _strings(args.get(k)) if v]
    for command in _command_text(tool, args):
        found.extend(_tokens(command))
    return found


def _url_candidates(args: Mapping[str, object]) -> list[str]:
    found = [v for k in URL_KEYS for v in _strings(args.get(k))]
    for text in _strings(args):
        found.extend(_URL.findall(text))
    return found


def _any_text_candidates(args: Mapping[str, object]) -> list[str]:
    found = []
    for text in _strings(args):
        found.append(text)
        found.extend(_tokens(text))
    return found


# --- Rules ---------------------------------------------------------------------------------------


@dataclass(frozen=True)
class Rule:
    id: str
    description: str
    tools: frozenset[str] | None  # None means every tool
    action: str
    enabled: bool
    paths: tuple[re.Pattern[str], ...]
    commands: tuple[re.Pattern[str], ...]
    urls: tuple[re.Pattern[str], ...]
    any_text: tuple[re.Pattern[str], ...]

    def applies_to(self, tool: str, args: Mapping[str, object], env: Mapping[str, str]) -> bool:
        if not self.enabled or (self.tools is not None and tool not in self.tools):
            return False
        checks = (
            (self.paths, lambda: [normalise_path(p, env) for p in _path_candidates(tool, args)], "full"),
            (self.commands, lambda: _command_text(tool, args), "search"),
            (self.urls, lambda: [u.lower() for u in _url_candidates(args)], "full"),
            (self.any_text, lambda: [normalise_path(t, env) for t in _any_text_candidates(args)], "full"),
        )
        for patterns, candidates, mode in checks:
            if not patterns:
                continue
            texts = candidates()
            if mode == "search":
                hit = any(p.search(t) for p in patterns for t in texts)
            else:
                hit = any(p.fullmatch(t) for p in patterns for t in texts)
            if not hit:
                return False
        return True


@dataclass(frozen=True)
class Policy:
    rules: tuple[Rule, ...]
    env: Mapping[str, str]

    def evaluate(self, tool: str, args: Mapping[str, object] | None) -> Decision:
        args = args if isinstance(args, Mapping) else {}
        for rule in self.rules:
            if rule.applies_to(tool, args, self.env):
                return Decision(rule.action, rule.id, rule.description)
        return INHERIT


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise PolicyError(message)


def _string_list(value: object, where: str) -> list[str]:
    _require(
        isinstance(value, list) and all(isinstance(v, str) and v for v in value),
        f"{where} must be a list of non-empty strings",
    )
    return value  # type: ignore[return-value]


def _parse_rule(raw: object, index: int, env: Mapping[str, str]) -> Rule:
    where = f"rules[{index}]"
    _require(isinstance(raw, dict), f"{where} must be an object")
    assert isinstance(raw, dict)
    unknown = set(raw) - RULE_FIELDS
    _require(not unknown, f"{where} has unknown field(s): {', '.join(sorted(unknown))}")

    rule_id = raw.get("id")
    _require(isinstance(rule_id, str) and bool(RULE_ID.match(rule_id)), f"{where}.id must match [a-z0-9-]+")
    where = f"rule {rule_id!r}"
    description = raw.get("description", "")
    _require(isinstance(description, str), f"{where}: description must be a string")
    action = raw.get("action")
    _require(action in ACTIONS, f"{where}: action must be one of {', '.join(ACTIONS)}")
    enabled = raw.get("enabled", True)
    _require(isinstance(enabled, bool), f"{where}: enabled must be true or false")

    tools = _string_list(raw.get("tools"), f"{where}: tools")
    _require(bool(tools), f"{where}: tools must not be empty")
    tool_set = None if tools == ["*"] else frozenset(tools)
    _require("*" not in tools or tools == ["*"], f'{where}: "*" must be the only tool')

    match = raw.get("match", {})
    _require(isinstance(match, dict), f"{where}: match must be an object")
    unknown = set(match) - set(MATCH_KINDS)
    _require(not unknown, f"{where}: unknown match kind(s): {', '.join(sorted(unknown))}")

    compiled: dict[str, tuple[re.Pattern[str], ...]] = {}
    for kind in MATCH_KINDS:
        patterns = _string_list(match[kind], f"{where}: match.{kind}") if kind in match else []
        if kind == "commands":
            try:
                compiled[kind] = tuple(re.compile(p, re.IGNORECASE) for p in patterns)
            except re.error as err:
                raise PolicyError(f"{where}: invalid regex in match.commands: {err}") from None
        elif kind == "urls":
            compiled[kind] = tuple(_glob_to_regex(p.lower()) for p in patterns)
        else:
            compiled[kind] = tuple(_glob_to_regex(normalise_path(p, env)) for p in patterns)

    return Rule(
        id=rule_id,
        description=description or f"Blocked by Kairos rule {rule_id}.",
        tools=tool_set,
        action=action,
        enabled=enabled,
        **compiled,
    )


def load_policy(text: str, env: Mapping[str, str] | None = None) -> Policy:
    """Parses and validates `policy.json`. Raises `PolicyError` with a reason a person can act on."""
    env = {k.upper(): v for k, v in (env if env is not None else os.environ).items()}
    try:
        raw = json.loads(text)
    except json.JSONDecodeError as err:
        raise PolicyError(f"not valid JSON (line {err.lineno}, column {err.colno}): {err.msg}") from None
    _require(isinstance(raw, dict), "the file must contain a JSON object")
    unknown = set(raw) - TOP_FIELDS
    _require(not unknown, f"unknown top-level field(s): {', '.join(sorted(unknown))}")
    _require(raw.get("version") == 1, "version must be 1")
    _require(raw.get("default", "inherit") == "inherit", 'default must be "inherit"')
    rules_raw = raw.get("rules")
    _require(isinstance(rules_raw, list), "rules must be a list")

    rules = tuple(_parse_rule(r, i, env) for i, r in enumerate(rules_raw))
    ids = [r.id for r in rules]
    duplicates = sorted({i for i in ids if ids.count(i) > 1})
    _require(not duplicates, f"duplicate rule id(s): {', '.join(duplicates)}")
    return Policy(rules=rules, env=env)


def decide(policy: Policy | PolicyError, tool: str, args: Mapping[str, object] | None) -> Decision:
    """The decision for one call. A broken policy blocks everything (spec §5, fail closed)."""
    if isinstance(policy, PolicyError):
        return Decision("invalid_rules", None, f"Kairos rules are invalid: {policy}")
    return policy.evaluate(tool, args)


class PolicyFile:
    """`policy.json`, re-read when its modification time changes (checked at most once a second)."""

    def __init__(self, path: Path, env: Mapping[str, str] | None = None, check_interval: float = 1.0):
        self.path = path
        self.env = env
        self.check_interval = check_interval
        self._lock = threading.Lock()
        self._checked = 0.0
        self._mtime: float | None = None
        self._current: Policy | PolicyError = PolicyError("not loaded yet")

    def current(self) -> Policy | PolicyError:
        with self._lock:
            now = time.monotonic()
            if now - self._checked < self.check_interval and self._mtime is not None:
                return self._current
            self._checked = now
            try:
                mtime = self.path.stat().st_mtime
            except FileNotFoundError:
                self._mtime = None
                self._current = PolicyError(f"{self.path} is missing; open Kairos to restore the rules")
                return self._current
            except OSError as err:
                self._mtime = None
                self._current = PolicyError(f"can't read {self.path}: {err}")
                return self._current
            if mtime != self._mtime:
                self._mtime = mtime
                try:
                    self._current = load_policy(self.path.read_text(encoding="utf-8"), self.env)
                except PolicyError as err:
                    self._current = err
                except OSError as err:
                    self._current = PolicyError(f"can't read {self.path}: {err}")
            return self._current
