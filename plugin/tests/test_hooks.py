"""Hook tests (M2 Step 4): the recorded hook calls from Step 1, replayed through the real hooks."""

import json
import shutil
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import kairos
from kairos import hooks as hooks_module
from kairos.audit import AuditStore
from kairos.hooks import KairosHooks, approval_rule_key, call_summary
from kairos.policy import PolicyFile

ROOT = Path(__file__).resolve().parents[2]
RECORDING = ROOT / "fixtures/hermes-0.22.5/plugin-hooks/hooks.jsonl"
DEFAULT_POLICY = ROOT / "plugin/kairos/policy.default.json"


def unshape(value):
    """The probe stored {"type", "len", "sample"} per value; turn it back into the value."""
    if isinstance(value, dict) and "type" in value and "sample" in value:
        return value["sample"]
    if isinstance(value, dict):
        return {k: unshape(v) for k, v in value.items()}
    if isinstance(value, list):
        return [unshape(v) for v in value]
    return value


def recorded_calls():
    for line in RECORDING.read_text(encoding="utf-8").splitlines():
        entry = json.loads(line)
        yield entry["hook"], unshape(entry["kwargs"])


class HooksTestBase(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.home = Path(self.dir.name)
        shutil.copy(DEFAULT_POLICY, self.home / "policy.json")
        self.audit = AuditStore(self.home / "audit.db")
        self.hooks = KairosHooks(PolicyFile(self.home / "policy.json", check_interval=0), self.audit)

    def tearDown(self):
        self.audit.close()
        self.dir.cleanup()

    def rows(self, table):
        conn = sqlite3.connect(self.home / "audit.db")
        conn.row_factory = sqlite3.Row
        try:
            return [dict(r) for r in conn.execute(f"SELECT * FROM {table} ORDER BY id")]
        finally:
            conn.close()


class ReplayTest(HooksTestBase):
    def replay(self):
        results = []
        for hook, kwargs in recorded_calls():
            handler = getattr(self.hooks, hook, None)
            if handler is None:  # on_session_start / on_session_end: not used by Kairos
                continue
            out = handler(**kwargs)
            if hook == "pre_tool_call":
                results.append((kwargs["tool_name"], out))
        return results

    def test_only_the_recursive_delete_is_escalated(self):
        results = self.replay()
        self.assertEqual([tool for tool, _ in results], ["read_file", "terminal", "search_files", "web_extract", "terminal"])
        self.assertEqual([out for _, out in results[:4]], [None, None, None, None])
        directive = results[4][1]
        self.assertEqual(directive["action"], "approve")
        self.assertIn("destructive-commands", directive["message"])
        # Hermes shows a placeholder for plugin approvals, so the command must be in the message.
        self.assertIn("rm -rf C:/Users/USER/kairos-probe-m2", directive["message"])
        self.assertTrue(directive["rule_key"].startswith("kairos:destructive-commands:"))

    def test_the_replay_leaves_a_complete_audit_trail(self):
        self.replay()
        events = self.rows("tool_event")
        self.assertEqual([(e["tool_name"], e["decision"], e["status"]) for e in events], [
            ("read_file", "inherit", "ok"),
            ("terminal", "inherit", "ok"),
            ("search_files", "inherit", "ok"),
            ("web_extract", "inherit", "ok"),
            ("terminal", "ask", "error"),
        ])
        self.assertEqual(events[4]["rule_id"], "destructive-commands")
        self.assertEqual(events[4]["error_type"], "tool_error")
        self.assertTrue(all(e["session_id"].startswith("kairos-probe-m2-") for e in events))
        self.assertTrue(all(e["tool_call_id"].startswith("call_") for e in events))
        [approval] = self.rows("approval")
        self.assertEqual(approval["choice"], "deny")
        self.assertEqual(approval["session_id"], "kairos-probe-m2-5")
        self.assertEqual(approval["tool_call_id"], events[4]["tool_call_id"])


class FailClosedTest(HooksTestBase):
    CALL = {"tool_name": "read_file", "args": {"path": "README.md"}, "session_id": "kairos-s", "tool_call_id": "call_1"}

    def assertBlocks(self, out):
        self.assertIsInstance(out, dict)
        self.assertEqual(out["action"], "block")
        self.assertTrue(out["message"])

    def test_a_deny_rule_blocks_and_tells_the_model_not_to_work_around_it(self):
        out = self.hooks.pre_tool_call(tool_name="read_file", args={"path": "~/.ssh/id_rsa"},
                                       session_id="kairos-s", tool_call_id="call_1")
        self.assertBlocks(out)
        self.assertIn("no-credentials", out["message"])
        self.assertIn("Do not retry", out["message"])

    def test_a_missing_policy_file_blocks(self):
        (self.home / "policy.json").unlink()
        self.assertBlocks(self.hooks.pre_tool_call(**self.CALL))

    def test_a_broken_policy_file_blocks_and_says_where_to_fix_it(self):
        (self.home / "policy.json").write_text("{ broken", encoding="utf-8")
        out = self.hooks.pre_tool_call(**self.CALL)
        self.assertBlocks(out)
        self.assertIn("Kairos rules are invalid", out["message"])
        self.assertIn("Settings", out["message"])

    def test_an_internal_error_in_the_rules_check_blocks(self):
        with mock.patch.object(hooks_module, "decide", side_effect=RuntimeError("boom")),                 self.assertLogs("kairos.hooks", level="ERROR"):
            self.assertBlocks(self.hooks.pre_tool_call(**self.CALL))

    def test_an_internal_error_building_the_answer_blocks(self):
        with mock.patch.object(hooks_module, "hook_result", side_effect=RuntimeError("boom")),                 self.assertLogs("kairos.hooks", level="ERROR"):
            self.assertBlocks(self.hooks.pre_tool_call(**self.CALL))

    def test_a_broken_audit_store_doesnt_change_the_decision(self):
        blocker = self.home / "not-a-dir"
        blocker.write_text("x")
        hooks = KairosHooks(PolicyFile(self.home / "policy.json", check_interval=0), AuditStore(blocker / "audit.db"))
        with self.assertLogs("kairos.audit"):
            self.assertIsNone(hooks.pre_tool_call(**self.CALL))
            out = hooks.pre_tool_call(tool_name="terminal", args={"command": "rm -rf x"}, session_id="s", tool_call_id="c")
        self.assertEqual(out["action"], "approve")

    def test_unexpected_or_missing_keywords_are_tolerated(self):
        self.assertIsNone(self.hooks.pre_tool_call(tool_name="read_file", args=None, future_field=1))
        self.hooks.post_tool_call(future_field=1)
        self.hooks.post_approval_response(future_field=1)


class ApprovalKeyTest(unittest.TestCase):
    def test_always_allow_covers_only_the_exact_call(self):
        a = approval_rule_key("destructive-commands", "terminal", {"command": "rm -rf build"})
        b = approval_rule_key("destructive-commands", "terminal", {"command": "rm -rf C:/Users"})
        self.assertNotEqual(a, b)
        self.assertEqual(a, approval_rule_key("destructive-commands", "terminal", {"command": "rm -rf build"}))

    def test_the_summary_shows_the_command_without_secrets(self):
        summary = call_summary("terminal", {"command": "curl -H 'Authorization: Bearer abc.def.ghi' https://x"})
        self.assertNotIn("abc.def.ghi", summary)
        self.assertIn("curl", summary)
        self.assertEqual(call_summary("web_extract", {"urls": ["https://a", "https://b"]}), "https://a, https://b")


class BrowserScriptCardTest(HooksTestBase):
    """M12 Step 4: the approval card says what a browser script will do, in words."""

    def ask(self, code, session="s1"):
        return self.hooks.pre_tool_call(tool_name="browser_exec", args={"code": code}, session_id=session)

    def test_a_click_says_where_it_clicks_and_on_which_site(self):
        self.ask('goto_url("https://httpbin.org/forms/post")\nwait_for_load()\npage_info()')
        result = self.ask("click_at_xy(412, 380)")
        self.assertEqual(result["action"], "approve")
        self.assertIn("\nWill: click at 412, 380, on httpbin.org — click_at_xy(412, 380)", result["message"])

    def test_steps_are_listed_in_the_order_the_script_does_them(self):
        words = hooks_module.describe_script(
            "goto_url('https://www.google.com')\nfill_input('textarea[name=q]', 'pizza near me')\npress_key('Enter')"
        )
        self.assertEqual(words, "open www.google.com; type «pizza near me» into textarea[name=q]; press Enter")

    def test_a_secret_field_never_shows_what_is_typed(self):
        words = hooks_module.describe_script("fill_input('#otp-code', '482913')")
        self.assertEqual(words, "type a hidden value into #otp-code")

    def test_script_actions_are_named(self):
        words = hooks_module.describe_script("js(\"document.querySelector('form').requestSubmit()\")")
        self.assertEqual(words, "submit a form by script")
        self.assertEqual(hooks_module.describe_script("print(js('document.cookie'))"), "read cookies or site storage")

    def test_the_whole_script_is_still_shown(self):
        code = "goto_url('https://example.com')\n" + "wait(0.1)\n" * 60 + "click_at_xy(1, 2)"
        result = self.ask(code)
        self.assertTrue(result["message"].endswith("click_at_xy(1, 2)"))

    def test_reading_a_page_asks_nothing(self):
        self.assertIsNone(self.ask('goto_url("https://httpbin.org/forms/post")\npage_info()'))

    def test_each_conversation_keeps_its_own_site(self):
        self.ask("goto_url('https://a.example')\npage_info()", session="one")
        self.ask("goto_url('https://b.example')\npage_info()", session="two")
        self.assertIn("on a.example", self.ask("press_key('Enter')", session="one")["message"])


class RegisterTest(unittest.TestCase):
    def test_register_wires_the_three_hooks_to_the_kairos_folder(self):
        registered = {}

        class FakeCtx:
            def register_hook(self, name, callback):
                registered[name] = callback

        with tempfile.TemporaryDirectory() as home, mock.patch.dict("os.environ", {"KAIROS_HOME": home}):
            hooks = kairos.register(FakeCtx())
            self.assertEqual(set(registered), {"pre_tool_call", "post_tool_call", "post_approval_response"})
            self.assertEqual(hooks.policy_file.path, Path(home) / "policy.json")
            hooks.audit.close()


if __name__ == "__main__":
    unittest.main()
