"""The Kairos audit log, `audit.db` (M2 spec §7, D-015). The plugin is its only writer.

Nothing here may break a tool call: every public method catches its own errors, logs them, and
returns None. The policy decision is made before any audit write, so a full disk or a locked
database changes what is recorded, never what is allowed.
"""

from __future__ import annotations

import json
import logging
import sqlite3
import threading
import time
from pathlib import Path
from typing import Any, Mapping

from .policy import Decision
from .redact import redact, redact_text

log = logging.getLogger("kairos.audit")

SCHEMA_VERSION = 1
ARGS_MAX = 2000
RESULT_MAX = 500
# Hermes's approval hooks identify the session only by this key (fixtures/…/plugin-hooks).
SESSION_KEY_PREFIX = "agent:main:api_server:dm:"

SCHEMA = """
CREATE TABLE IF NOT EXISTS tool_event (
  id             INTEGER PRIMARY KEY,
  at             REAL NOT NULL,
  session_id     TEXT,
  tool_call_id   TEXT,
  tool_name      TEXT NOT NULL,
  args_redacted  TEXT NOT NULL,
  decision       TEXT NOT NULL,
  rule_id        TEXT,
  status         TEXT,
  duration_ms    INTEGER,
  result_preview TEXT,
  error_type     TEXT
);
CREATE INDEX IF NOT EXISTS tool_event_call ON tool_event(tool_call_id);
CREATE INDEX IF NOT EXISTS tool_event_session ON tool_event(session_id, at);
CREATE TABLE IF NOT EXISTS approval (
  id           INTEGER PRIMARY KEY,
  at           REAL NOT NULL,
  session_key  TEXT,
  session_id   TEXT,
  tool_call_id TEXT,
  command      TEXT,
  description  TEXT,
  pattern_key  TEXT,
  choice       TEXT NOT NULL,
  decided_by   TEXT
);
CREATE INDEX IF NOT EXISTS approval_session ON approval(session_id, at);
"""


def session_id_from_key(session_key: str | None) -> str | None:
    """`agent:main:api_server:dm:kairos-abc` → `kairos-abc`; other shapes give None."""
    if session_key and session_key.startswith(SESSION_KEY_PREFIX):
        return session_key[len(SESSION_KEY_PREFIX):] or None
    return None


def _json(value: Any) -> str:
    return json.dumps(redact(value, max_string=ARGS_MAX), ensure_ascii=False, default=str)


class AuditStore:
    """One connection, guarded by a lock: Hermes may run hooks for parallel tool calls on several
    threads at once."""

    def __init__(self, path: Path):
        self.path = path
        self._lock = threading.Lock()
        self._conn: sqlite3.Connection | None = None

    def _connection(self) -> sqlite3.Connection:
        if self._conn is None:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            conn = sqlite3.connect(self.path, timeout=2.0, check_same_thread=False, isolation_level=None)
            conn.execute("PRAGMA journal_mode=WAL")
            version = conn.execute("PRAGMA user_version").fetchone()[0]
            if version > SCHEMA_VERSION:
                conn.close()
                raise RuntimeError(f"audit.db is schema {version}, newer than this plugin ({SCHEMA_VERSION})")
            conn.executescript(SCHEMA)
            conn.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
            self._conn = conn
        return self._conn

    def _write(self, what: str, sql: str, params: tuple) -> int | None:
        try:
            with self._lock:
                cursor = self._connection().execute(sql, params)
                return cursor.lastrowid
        except Exception as err:  # noqa: BLE001 — an audit failure must never reach the tool call
            log.warning("kairos audit: couldn't record %s: %s", what, err)
            return None

    def record_decision(
        self,
        *,
        session_id: str | None,
        tool_call_id: str | None,
        tool_name: str,
        args: Mapping[str, Any] | None,
        decision: Decision,
    ) -> int | None:
        """Called from `pre_tool_call`, after the decision is made."""
        return self._write(
            "a decision",
            "INSERT INTO tool_event (at, session_id, tool_call_id, tool_name, args_redacted, decision, rule_id)"
            " VALUES (?, ?, ?, ?, ?, ?, ?)",
            (time.time(), session_id, tool_call_id, tool_name, _json(args or {}), decision.action, decision.rule_id),
        )

    def record_result(
        self,
        *,
        tool_call_id: str | None,
        status: str | None,
        duration_ms: int | None,
        result: Any,
        error_type: str | None = None,
    ) -> int | None:
        """Called from `post_tool_call`: fills in the most recent open row for this call."""
        if not tool_call_id:
            return None
        preview = redact_text(result if isinstance(result, str) else json.dumps(result, default=str))[:RESULT_MAX]
        return self._write(
            "a result",
            "UPDATE tool_event SET status = ?, duration_ms = ?, result_preview = ?, error_type = ?"
            " WHERE id = (SELECT id FROM tool_event WHERE tool_call_id = ? AND status IS NULL"
            "             ORDER BY id DESC LIMIT 1)",
            (status, duration_ms, preview, error_type, tool_call_id),
        )

    def record_approval(
        self,
        *,
        session_key: str | None,
        command: str | None,
        description: str | None,
        pattern_key: str | None,
        choice: str,
        decided_by: str | None,
    ) -> int | None:
        """Called from `post_approval_response`. Hermes runs its approval inside the tool call
        (pre_tool_call → approval → post_tool_call), so the approval belongs to the call that is
        still open in that session."""
        session_id = session_id_from_key(session_key)
        return self._write(
            "an approval",
            "INSERT INTO approval (at, session_key, session_id, tool_call_id, command, description,"
            " pattern_key, choice, decided_by)"
            " VALUES (?, ?, ?, (SELECT tool_call_id FROM tool_event WHERE session_id = ? AND status IS NULL"
            "                   ORDER BY id DESC LIMIT 1), ?, ?, ?, ?, ?)",
            (
                time.time(),
                session_key,
                session_id,
                session_id,
                redact_text(command or "")[:ARGS_MAX],
                description,
                pattern_key,
                choice,
                decided_by,
            ),
        )

    def close(self) -> None:
        with self._lock:
            if self._conn is not None:
                self._conn.close()
                self._conn = None
