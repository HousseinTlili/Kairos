"""Computer use under the Kairos rules (M20): where a call acts, and what's refused there."""

import json
from pathlib import Path

from kairos.desktop import DesktopTracker
from kairos.hooks import KairosHooks
from kairos.pctools import Window
from kairos.policy import PolicyFile

from .test_hooks import HooksTestBase

ROOT = Path(__file__).resolve().parents[2]
CAPTURE = (ROOT / "fixtures/hermes-0.22.5/computer-use/capture-ax.json").read_text(encoding="utf-8")
CLICK = (ROOT / "fixtures/hermes-0.22.5/computer-use/click-result.json").read_text(encoding="utf-8")
SESSION = "kairos-test"

CHARMAP = Window(1, 10, "charmap.exe", "Character Map")
KAIROS = Window(2, 20, "kairos.exe", "Kairos")
TERMINAL = Window(3, 30, "WindowsTerminal.exe", "PowerShell")
BROWSER = Window(4, 40, "opera.exe", "Some page - Opera")
BANKING = Window(5, 50, "mybanktool.exe", "My Bank - accounts")
EXPLORER = Window(6, 60, "explorer.exe", "Downloads")


class DesktopTestBase(HooksTestBase):
    def setUp(self):
        super().setUp()
        self.windows = [CHARMAP, KAIROS, TERMINAL, BROWSER, BANKING, EXPLORER]
        self.front = KAIROS
        tracker = DesktopTracker(lambda: self.windows, lambda: self.front)
        self.hooks = KairosHooks(PolicyFile(self.home / "policy.json", check_interval=0), self.audit, tracker)
        self.calls = 0

    def call(self, session=SESSION, **args):
        self.calls += 1
        return self.hooks.pre_tool_call(
            tool_name="computer_use", args=args, session_id=session, tool_call_id=f"c{self.calls}"
        )

    def finish(self, result, session=SESSION):
        self.hooks.post_tool_call(
            tool_name="computer_use", tool_call_id=f"c{self.calls}", session_id=session, result=result, status="ok"
        )

    def captured(self, app="charmap.exe"):
        self.call(action="capture", app=app, mode="ax")
        self.finish(CAPTURE)

    def assertRefused(self, result, rule):
        self.assertIsNotNone(result)
        self.assertEqual(result["action"], "block", result)
        self.assertIn(rule, result["message"])


class WhereItMayNotGoTest(DesktopTestBase):
    def test_a_click_on_kaiross_own_window_is_refused(self):
        self.assertRefused(self.call(action="capture", app="kairos.exe"), "computer-use-off-limits")
        self.assertRefused(self.call(action="click", app="Kairos", element=1), "computer-use-off-limits")

    def test_capturing_with_no_app_while_the_master_is_in_kairos_is_refused(self):
        self.assertRefused(self.call(action="capture"), "computer-use-off-limits")

    def test_typing_into_a_terminal_is_refused(self):
        result = self.call(action="type", app="WindowsTerminal.exe", text="hermes config set approvals.mode off")
        self.assertRefused(result, "computer-use-off-limits")

    def test_a_window_named_by_its_title_or_path_is_checked_all_the_same(self):
        self.assertRefused(self.call(action="capture", app="powershell"), "computer-use-off-limits")
        self.assertRefused(self.call(action="capture", app=r"C:\Program\opera.exe"), "computer-use-off-limits")

    def test_browsers_and_file_explorer_are_refused(self):
        self.assertRefused(self.call(action="capture", app="opera.exe"), "computer-use-off-limits")
        self.assertRefused(self.call(action="capture", app="explorer.exe"), "computer-use-off-limits")

    def test_a_window_whose_title_names_a_bank_is_refused_whatever_the_process(self):
        self.assertRefused(self.call(action="capture", app="mybanktool"), "computer-use-off-limits")

    def test_an_off_limits_app_with_no_open_window_is_refused_by_name(self):
        self.windows.remove(KAIROS)
        self.assertRefused(self.call(action="capture", app="kairos.exe"), "computer-use-off-limits")
        self.assertRefused(self.call(action="capture", app="kairos"), "computer-use-off-limits")

    def test_a_window_found_by_its_number_is_checked_too(self):
        self.assertRefused(self.call(action="capture", window_id=2), "computer-use-off-limits")
        self.assertRefused(self.call(action="capture", pid=30), "computer-use-off-limits")

    def test_the_whole_screen_and_the_taskbar_are_refused(self):
        self.assertRefused(self.call(action="capture", app="screen"), "computer-use-whole-screen")
        self.assertRefused(self.call(action="click", app="desktop", element=1), "computer-use-whole-screen")

    def test_win_r_is_refused(self):
        self.captured()
        for keys in ("win+r", "WIN + R", "ctrl+win+d", "win", "ctrl+shift+esc", "ctrl+alt+del"):
            self.assertRefused(self.call(action="key", keys=keys), "computer-use-keys")

    def test_ordinary_keys_go_on_to_hermess_gate(self):
        self.captured()
        for keys in ("ctrl+s", "return", "tab", "window"):
            self.assertIsNone(self.call(action="key", keys=keys), keys)

    def test_text_that_looks_like_a_key_isnt_typed(self):
        self.captured()
        result = self.call(action="type", text="sk-abcdefghijklmnopqrstuvwxyz0123456789")
        self.assertRefused(result, "computer-use-typed-secret")

    def test_the_tool_is_refused_outside_kaiross_conversations(self):
        self.assertRefused(self.call(session="telegram-1", action="capture", app="charmap.exe"), "kairos-own-tools")
        self.assertRefused(self.call(session="", action="click", element=1), "kairos-own-tools")

    def test_with_no_way_to_check_windows_everything_is_refused(self):
        self.hooks.desktop = None
        self.assertRefused(self.call(action="capture", app="charmap.exe"), "computer-use-no-target")

    def test_a_window_list_that_fails_blocks_the_call(self):
        def broken():
            raise OSError("no")

        self.hooks.desktop = DesktopTracker(broken, lambda: None)
        result = self.call(action="capture", app="charmap.exe")
        self.assertEqual(result["action"], "block")


class KnowingTheTargetTest(DesktopTestBase):
    def test_input_with_no_known_target_is_refused(self):
        self.assertRefused(self.call(action="click", element=1), "computer-use-no-target")

    def test_a_capture_that_found_nothing_sets_no_target(self):
        self.call(action="capture", app="charmap.exe")
        self.finish(json.dumps({"mode": "ax", "width": 0, "height": 0, "app": "", "window_title": "<no window>"}))
        self.assertRefused(self.call(action="click", element=1), "computer-use-no-target")

    def test_input_after_a_capture_goes_on_to_hermess_gate_with_one_card(self):
        self.captured()
        self.assertIsNone(self.call(action="click", element=1))

    def test_an_app_that_isnt_open_is_refused(self):
        self.assertRefused(self.call(action="capture", app="nothing-like-this"), "computer-use-no-target")

    def test_a_target_that_closed_is_no_longer_a_target(self):
        self.captured()
        self.windows.remove(CHARMAP)
        self.assertRefused(self.call(action="click", element=1), "computer-use-no-target")

    def test_a_remembered_target_that_became_off_limits_is_refused(self):
        self.captured()
        self.windows[0] = Window(1, 10, "charmap.exe", "PayPal receipt")
        self.assertRefused(self.call(action="click", element=1), "computer-use-off-limits")

    def test_one_conversations_target_isnt_anothers(self):
        self.captured()
        self.assertRefused(self.call(session="kairos-other", action="click", element=1), "computer-use-no-target")

    def test_an_input_naming_a_new_app_is_checked_against_that_app(self):
        self.captured()
        self.assertRefused(self.call(action="click", app="opera", element=1), "computer-use-off-limits")

    def test_the_audit_row_says_where_and_what_for_the_card(self):
        self.captured()
        self.call(action="click", element=1)
        args = json.loads(self.rows("tool_event")[-1]["args_redacted"])
        self.assertEqual(args["kairos_target"], "Character Map")
        self.assertEqual(args["kairos_element"], "Select")

    def test_element_names_follow_the_last_capture(self):
        self.captured()
        later = json.loads(CAPTURE)
        later["elements"][1]["label"] = "Copy"
        self.call(action="capture", app="charmap.exe")
        self.finish(json.dumps(later))
        self.call(action="click", element=1)
        self.assertEqual(json.loads(self.rows("tool_event")[-1]["args_redacted"])["kairos_element"], "Copy")

    def test_a_result_with_no_elements_leaves_the_names_alone(self):
        self.captured()
        self.call(action="click", element=1)
        self.finish(CLICK)
        self.call(action="click", element=1)
        self.assertEqual(json.loads(self.rows("tool_event")[-1]["args_redacted"])["kairos_element"], "Select")


class SeeingTest(DesktopTestBase):
    def test_a_click_with_app_capture_isnt_treated_as_a_capture(self):
        self.windows.append(Window(7, 70, "capture.exe", "Capture"))
        self.assertIsNone(self.call(action="click", app="capture", element=1))

    def test_seeing_a_window_asks_and_says_where_the_picture_goes(self):
        result = self.call(action="capture", app="charmap.exe")
        self.assertEqual(result["action"], "approve")
        self.assertIn("computer-use-see", result["message"])
        self.assertIn("Will: see Character Map, and send the picture to your AI provider", result["message"])

    def test_a_grant_to_see_one_app_doesnt_cover_another(self):
        self.windows.append(Window(8, 80, "calc.exe", "Calculator"))
        first = self.call(action="capture", app="charmap.exe", mode="ax")
        other_mode = self.call(action="capture", app="CharMap", mode="som")
        other_app = self.call(action="capture", app="calc.exe")
        self.assertEqual(first["rule_key"], other_mode["rule_key"])
        self.assertNotEqual(first["rule_key"], other_app["rule_key"])

    def test_listing_windows_asks_and_says_what_goes_out(self):
        for action, words in (("list_windows", "titles"), ("list_apps", "names of your apps")):
            result = self.call(action=action)
            self.assertEqual(result["action"], "approve")
            self.assertIn("computer-use-list", result["message"])
            self.assertIn(words, result["message"])

    def test_waiting_is_free(self):
        self.assertIsNone(self.call(action="wait", seconds=1))
