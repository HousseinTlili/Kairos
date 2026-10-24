"""Audit store and redaction tests (M2 Step 3)."""

import json
import logging
import sqlite3
import tempfile
import threading
import unittest
from pathlib import Path

from kairos.audit import AuditStore, session_id_from_key
from kairos.policy import Decision
from kairos.redact import REDACTED, redact, redact_text

# Fake secrets in the real formats. None of them is valid anywhere.
FAKE = {
    "hermes_key": "3f9a" * 16,  # 64 hex, like API_SERVER_KEY
    "openai": "sk-proj-" + "A1b2C3d4E5f6G7h8" * 2,
    "anthropic": "sk-ant-" + "x" * 40,
    "github": "ghp_" + "Z9" * 18,
    "github_pat": "github_pat_" + "11AB" * 8,
    "slack": "xoxb-" + "1234567890-abcdef",
    "aws": "AKIA" + "ABCDEFGHIJKLMNOP",
    "google": "AIza" + "S" * 35,
    "jwt": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJlLXZhbHVl",
    "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjE=\n-----END OPENSSH PRIVATE KEY-----",
}
GIT_SHA = "1232822" + "a" * 33  # 40 hex: a commit id, which must survive


class RedactionTest(unittest.TestCase):
    def test_every_secret_format_is_removed_from_text(self):
        for name, secret in FAKE.items():
            with self.subTest(name):
                cleaned = redact_text(f"before {secret} after")
                self.assertNotIn(secret, cleaned)
                self.assertIn(REDACTED, cleaned)
                self.assertTrue(cleaned.startswith("before "))

    def test_secret_assignments_keep_the_name_and_lose_the_value(self):
        self.assertEqual(redact_text("API_SERVER_KEY=hunter2hunter2"), f"API_SERVER_KEY={REDACTED}")
        self.assertEqual(redact_text('$env:GITHUB_TOKEN = "abc def"'), f"$env:GITHUB_TOKEN = {REDACTED}")
        self.assertEqual(redact_text("password: letmein"), f"password: {REDACTED}")
        self.assertEqual(redact_text("GITHUB_TOKEN=12345678"), f"GITHUB_TOKEN={REDACTED}")

    def test_bearer_headers_are_removed(self):
        self.assertNotIn("abc.def", redact_text("curl -H 'Authorization: Bearer abc.def'"))

    def test_values_under_secret_keys_are_dropped_whatever_they_look_like(self):
        cleaned = redact({"path": "README.md", "api_key": "short", "nested": {"Password": 12345, "ok": "fine"}})
        self.assertEqual(cleaned, {"path": "README.md", "api_key": REDACTED, "nested": {"Password": REDACTED, "ok": "fine"}})

    def test_a_git_commit_id_is_kept(self):
        self.assertEqual(redact_text(f"git show {GIT_SHA}"), f"git show {GIT_SHA}")

    def test_ordinary_text_is_untouched(self):
        for text in ("rm -rf C:/Users/USER/scratch", "git log --oneline -3", "keyboard shortcuts", "tokens: 12",
                     '"input_tokens": 12368', "max_tokens=400"):
            with self.subTest(text):
                self.assertEqual(redact_text(text), text)

    def test_long_strings_are_cut_after_redaction_so_no_secret_half_survives(self):
        text = "x" * 1990 + FAKE["hermes_key"]
        cleaned = redact({"command": text}, max_string=2000)["command"]
        self.assertNotIn(FAKE["hermes_key"][:10], cleaned)
        self.assertLessEqual(len(cleaned.split("…")[0]), 2000)


class AuditStoreTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "kairos" / "audit.db"
        self.store = AuditStore(self.path)

    def tearDown(self):
        self.store.close()
        self.dir.cleanup()

    def rows(self, table):
        conn = sqlite3.connect(self.path)
        conn.row_factory = sqlite3.Row
        try:
            return [dict(r) for r in conn.execute(f"SELECT * FROM {table} ORDER BY id")]
        finally:
            conn.close()

    def call(self, tool_call_id="call_1", session="kairos-s1", tool="terminal", args=None, decision=None):
        return self.store.record_decision(
            session_id=session,
            tool_call_id=tool_call_id,
            tool_name=tool,
            args=args if args is not None else {"command": "ls"},
            decision=decision or Decision("inherit"),
        )

    def test_a_decision_and_its_result_end_up_in_one_row(self):
        self.call(decision=Decision("ask", "destructive-commands", "…"), args={"command": "rm -rf x"})
        self.store.record_result(tool_call_id="call_1", status="error", duration_ms=1702,
                                 result='{"error": "BLOCKED: Command denied by user."}', error_type="tool_error")
        [row] = self.rows("tool_event")
        self.assertEqual(row["decision"], "ask")
        self.assertEqual(row["rule_id"], "destructive-commands")
        self.assertEqual(json.loads(row["args_redacted"]), {"command": "rm -rf x"})
        self.assertEqual((row["status"], row["duration_ms"], row["error_type"]), ("error", 1702, "tool_error"))
        self.assertIn("BLOCKED", row["result_preview"])

    def test_the_schema_is_versioned(self):
        self.call()
        conn = sqlite3.connect(self.path)
        try:
            self.assertEqual(conn.execute("PRAGMA user_version").fetchone()[0], 1)
        finally:
            conn.close()

    def test_an_approval_links_to_the_open_call_in_its_session(self):
        self.call(tool_call_id="call_done")
        self.store.record_result(tool_call_id="call_done", status="ok", duration_ms=5, result="{}")
        self.call(tool_call_id="call_open", args={"command": "rm -rf x"})
        self.call(tool_call_id="call_other_session", session="kairos-s2")
        self.store.record_approval(
            session_key="agent:main:api_server:dm:kairos-s1",
            command="rm -rf x", description="recursive delete", pattern_key="recursive delete",
            choice="deny", decided_by=None,
        )
        [approval] = self.rows("approval")
        self.assertEqual((approval["session_id"], approval["tool_call_id"], approval["choice"]),
                         ("kairos-s1", "call_open", "deny"))

    def test_a_result_fills_the_latest_open_row_only(self):
        self.call(tool_call_id="call_1")
        self.store.record_result(tool_call_id="call_1", status="ok", duration_ms=1, result="{}")
        self.call(tool_call_id="call_1")  # Hermes retried the same call id
        self.store.record_result(tool_call_id="call_1", status="error", duration_ms=2, result="{}")
        self.assertEqual([r["status"] for r in self.rows("tool_event")], ["ok", "error"])

    def test_no_fake_secret_reaches_the_database(self):
        everything = " ".join(FAKE.values())
        self.call(args={"command": f"curl -H 'Authorization: Bearer {FAKE['jwt']}' {everything}",
                        "api_key": "plain-text-value", "env": {"GITHUB_TOKEN": FAKE["github"]}})
        self.store.record_result(tool_call_id="call_1", status="ok", duration_ms=3,
                                 result=json.dumps({"output": everything}))
        self.store.record_approval(session_key="agent:main:api_server:dm:kairos-s1", command=everything,
                                   description="x", pattern_key="x", choice="once", decided_by=None)
        self.store.close()
        # Check the raw bytes of the database and its WAL, not just what a query returns.
        raw = b"".join(p.read_bytes() for p in self.path.parent.glob("audit.db*"))
        for name, secret in list(FAKE.items()) + [("plain", "plain-text-value")]:
            for line in secret.splitlines():
                if len(line) >= 12:
                    with self.subTest(name):
                        self.assertNotIn(line.encode(), raw)

    def test_a_write_that_fails_returns_none_and_never_raises(self):
        blocker = Path(self.dir.name) / "not-a-dir"
        blocker.write_text("a file where the database folder should be")
        broken = AuditStore(blocker / "audit.db")
        with self.assertLogs("kairos.audit", level=logging.WARNING):
            self.assertIsNone(broken.record_decision(session_id="s", tool_call_id="c", tool_name="t",
                                                     args={}, decision=Decision("inherit")))
            self.assertIsNone(broken.record_result(tool_call_id="c", status="ok", duration_ms=1, result="{}"))
            self.assertIsNone(broken.record_approval(session_key=None, command=None, description=None,
                                                     pattern_key=None, choice="deny", decided_by=None))

    def test_parallel_tool_calls_are_all_recorded(self):
        threads = [threading.Thread(target=self.call, kwargs={"tool_call_id": f"call_{i}"}) for i in range(20)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(len(self.rows("tool_event")), 20)

    def test_a_newer_schema_is_refused_without_raising(self):
        self.store.close()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        conn = sqlite3.connect(self.path)
        conn.execute("PRAGMA user_version = 99")
        conn.close()
        with self.assertLogs("kairos.audit", level=logging.WARNING):
            self.assertIsNone(AuditStore(self.path).record_decision(
                session_id="s", tool_call_id="c", tool_name="t", args={}, decision=Decision("inherit")))

    def test_session_keys_map_to_kairos_session_ids(self):
        self.assertEqual(session_id_from_key("agent:main:api_server:dm:kairos-abc"), "kairos-abc")
        self.assertIsNone(session_id_from_key("agent:main:telegram:dm:123"))
        self.assertIsNone(session_id_from_key(None))


if __name__ == "__main__":
    unittest.main()
