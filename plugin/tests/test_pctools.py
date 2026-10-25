"""PC control tool tests (M3 Step 5), with a fake Windows layer: what each tool decides and
answers, not how Windows is called."""

import json
import unittest
from unittest import mock

import kairos
from kairos.pctools import BUILTIN_APPS, STEERING, PcTools, Window, match_name, steering


class FakeWindows:
    def __init__(self, apps=None, windows=None, stubborn=(), unkillable=()):
        self.apps = apps if apps is not None else []
        self.open = {w.hwnd: w for w in (windows or [])}
        self.running = {w.pid for w in (windows or [])}
        self.stubborn = set(stubborn)  # windows that ignore WM_CLOSE (a "save changes?" prompt)
        self.unkillable = set(unkillable)
        self.launched, self.closed, self.terminated = [], [], []
        self.start_apps_calls = 0
        self.fail_start_apps = False

    def start_apps(self):
        self.start_apps_calls += 1
        if self.fail_start_apps:
            raise OSError("powershell missing")
        return self.apps

    def launch(self, target):
        self.launched.append(target)

    def windows(self):
        return list(self.open.values())

    def post_close(self, hwnd):
        self.closed.append(hwnd)
        if hwnd not in self.stubborn:
            self.open.pop(hwnd, None)

    def window_open(self, hwnd):
        return hwnd in self.open

    def terminate(self, pid):
        self.terminated.append(pid)
        if pid not in self.unkillable:
            self.running.discard(pid)
            for hwnd in [h for h, w in self.open.items() if w.pid == pid]:
                del self.open[hwnd]

    def process_running(self, pid):
        return pid in self.running


class FakeClock:
    def __init__(self):
        self.now = 1000.0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


START_APPS = [
    ("Notepad", "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"),
    ("Visual Studio Code", "Microsoft.VisualStudioCode"),
    ("Visual Studio 2022", "VisualStudio.17"),
    ("Spotify", "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
    ("Uninstall Spotify", "{uninstall}"),
]

NOTEPAD = Window(hwnd=11, pid=100, exe="Notepad.exe", title="notes.txt - Notepad")
NOTEPAD_2 = Window(hwnd=12, pid=101, exe="notepad.exe", title="todo.txt - Notepad")
CHROME = Window(hwnd=21, pid=200, exe="chrome.exe", title="Inbox - Google Chrome")
DOWNLOADS = Window(hwnd=31, pid=300, exe="explorer.exe", title="Downloads")
CALC = Window(hwnd=41, pid=400, exe="ApplicationFrameHost.exe", title="Calculator")
HERMES = Window(hwnd=51, pid=999, exe="python.exe", title="Hermes gateway")


def tools(fake, clock=None):
    clock = clock or FakeClock()
    return PcTools(fake, clock=clock, sleep=clock.sleep, own_pid=999)


def answer(text):
    return json.loads(text)


class MatchNameTest(unittest.TestCase):
    NAMES = [name for name, _ in START_APPS]

    def test_exact_name_wins_even_when_it_is_also_a_prefix(self):
        self.assertEqual(match_name("visual studio code", self.NAMES)[0], "Visual Studio Code")

    def test_a_unique_prefix_or_substring_is_enough(self):
        self.assertEqual(match_name("spot", self.NAMES)[0], "Spotify")
        self.assertEqual(match_name("code", self.NAMES)[0], "Visual Studio Code")

    def test_several_matches_are_ambiguous(self):
        chosen, candidates = match_name("visual", self.NAMES)
        self.assertIsNone(chosen)
        self.assertEqual(candidates, ["Visual Studio 2022", "Visual Studio Code"])

    def test_no_match_and_blank_queries(self):
        self.assertEqual(match_name("photoshop", self.NAMES), (None, []))
        self.assertEqual(match_name("   ", self.NAMES), (None, []))


class OpenAppTest(unittest.TestCase):
    def test_an_app_in_the_start_menu_is_launched_through_its_app_id(self):
        fake = FakeWindows(apps=START_APPS)
        out = answer(tools(fake).open_app({"name": "notepad"}))
        self.assertEqual(out, {"opened": "Notepad"})
        self.assertEqual(fake.launched, ["shell:AppsFolder\\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"])

    def test_an_ambiguous_name_opens_nothing_and_lists_the_candidates(self):
        fake = FakeWindows(apps=START_APPS)
        out = answer(tools(fake).open_app({"name": "Visual Studio"}))
        self.assertIn("Several apps", out["error"])
        self.assertEqual(out["candidates"], ["Visual Studio 2022", "Visual Studio Code"])
        self.assertEqual(fake.launched, [])

    def test_an_unknown_name_opens_nothing_and_suggests_close_names(self):
        fake = FakeWindows(apps=START_APPS)
        out = answer(tools(fake).open_app({"name": "Spotify Premium Desktop"}))
        self.assertIn("No app called", out["error"])
        self.assertIn("Spotify", out["similar"])
        self.assertEqual(fake.launched, [])

    def test_a_path_or_command_is_just_a_name_that_matches_nothing(self):
        fake = FakeWindows(apps=START_APPS)
        for name in [r"C:\Windows\System32\cmd.exe", "notepad & calc", "powershell -enc AAAA"]:
            with self.subTest(name=name):
                out = answer(tools(fake).open_app({"name": name}))
                self.assertIn("error", out)
        self.assertEqual(fake.launched, [])

    def test_the_built_in_list_still_works_when_the_start_menu_cannot_be_read(self):
        fake = FakeWindows()
        fake.fail_start_apps = True
        out = answer(tools(fake).open_app({"name": "calculator"}))
        self.assertEqual(out, {"opened": "Calculator"})
        self.assertEqual(fake.launched, [BUILTIN_APPS["Calculator"]])

    def test_the_catalogue_is_read_once_per_five_minutes(self):
        fake = FakeWindows(apps=START_APPS)
        clock = FakeClock()
        t = tools(fake, clock)
        t.open_app({"name": "Notepad"})
        t.open_app({"name": "Spotify"})
        self.assertEqual(fake.start_apps_calls, 1)
        clock.now += 301
        t.open_app({"name": "Notepad"})
        self.assertEqual(fake.start_apps_calls, 2)

    def test_a_missing_name_is_explained(self):
        for args in [None, {}, {"name": ""}, {"name": 42}]:
            with self.subTest(args=args):
                self.assertIn("error", answer(tools(FakeWindows(apps=START_APPS)).open_app(args)))


class ListRunningAppsTest(unittest.TestCase):
    def test_apps_are_listed_once_per_process_without_system_processes_or_hermes(self):
        fake = FakeWindows(windows=[NOTEPAD, NOTEPAD_2, CHROME, DOWNLOADS, HERMES])
        out = answer(tools(fake).list_running_apps({}))
        self.assertEqual(
            [(a["name"], a["pid"]) for a in out["apps"]],
            [("chrome.exe", 200), ("Notepad.exe", 100), ("notepad.exe", 101)],
        )
        self.assertEqual(out["apps"][0]["windows"], 1)


class CloseAppTest(unittest.TestCase):
    def test_closing_by_program_name_asks_every_window_to_close(self):
        fake = FakeWindows(windows=[NOTEPAD, NOTEPAD_2, CHROME])
        out = answer(tools(fake).close_app({"name": "notepad.exe"}))
        self.assertEqual(out["status"], "closed")
        self.assertEqual(sorted(fake.closed), [11, 12])
        self.assertEqual(fake.terminated, [])
        self.assertIn(21, fake.open)

    def test_closing_by_window_title(self):
        fake = FakeWindows(windows=[NOTEPAD, CHROME])
        out = answer(tools(fake).close_app({"name": "Inbox"}))
        self.assertEqual((out["status"], out["apps"]), ("closed", ["chrome.exe"]))

    def test_an_app_that_stays_open_is_not_reported_as_closed(self):
        fake = FakeWindows(windows=[NOTEPAD], stubborn={11})
        clock = FakeClock()
        out = answer(tools(fake, clock).close_app({"name": "notepad"}))
        self.assertEqual(out["status"], "still_open")
        self.assertIn("asking to save", out["message"])
        self.assertEqual(fake.terminated, [], "never forces on its own")
        self.assertGreaterEqual(clock.now - 1000.0, 5.0)

    def test_force_ends_the_process_only_when_asked(self):
        fake = FakeWindows(windows=[NOTEPAD], stubborn={11})
        out = answer(tools(fake).close_app({"name": "notepad", "force": True}))
        self.assertEqual((out["status"], out.get("forced")), ("closed", True))
        self.assertEqual(fake.terminated, [100])

    def test_force_must_be_true_not_just_truthy(self):
        fake = FakeWindows(windows=[NOTEPAD], stubborn={11})
        out = answer(tools(fake).close_app({"name": "notepad", "force": "yes"}))
        self.assertEqual(out["status"], "still_open")
        self.assertEqual(fake.terminated, [])

    def test_a_window_title_that_belongs_to_explorer_is_refused(self):
        # The `system-processes` rule sees only "Downloads"; the tool sees explorer.exe.
        fake = FakeWindows(windows=[DOWNLOADS, NOTEPAD])
        out = answer(tools(fake).close_app({"name": "Downloads", "force": True}))
        self.assertIn("system process", out["error"])
        self.assertEqual((fake.closed, fake.terminated), ([], []))

    def test_system_processes_are_refused_by_name(self):
        fake = FakeWindows(windows=[DOWNLOADS])
        out = answer(tools(fake).close_app({"name": "EXPLORER"}))
        self.assertIn("system process", out["error"])
        self.assertEqual(fake.closed, [])

    def test_the_assistant_never_closes_its_own_process(self):
        fake = FakeWindows(windows=[HERMES])
        out = answer(tools(fake).close_app({"name": "python", "force": True}))
        self.assertIn("own process", out["error"])
        self.assertEqual(fake.terminated, [])

    def test_a_store_app_closes_normally_but_is_never_force_killed(self):
        fake = FakeWindows(windows=[CALC])
        out = answer(tools(fake).close_app({"name": "Calculator", "force": True}))
        self.assertIn("Store app", out["error"])
        self.assertEqual(fake.terminated, [])
        out = answer(tools(fake).close_app({"name": "Calculator"}))
        self.assertEqual(out["status"], "closed")

    def test_a_title_shared_by_two_apps_is_ambiguous(self):
        fake = FakeWindows(windows=[NOTEPAD, Window(hwnd=61, pid=600, exe="code.exe", title="notes.txt - VS Code")])
        out = answer(tools(fake).close_app({"name": "notes.txt"}))
        self.assertIn("several apps", out["error"])
        self.assertEqual(fake.closed, [])

    def test_nothing_open_by_that_name(self):
        out = answer(tools(FakeWindows(windows=[NOTEPAD])).close_app({"name": "spotify"}))
        self.assertIn("list_running_apps", out["error"])

    def test_a_process_windows_wont_end_is_reported(self):
        fake = FakeWindows(windows=[NOTEPAD], stubborn={11}, unkillable={100})
        out = answer(tools(fake).close_app({"name": "notepad", "force": True}))
        self.assertEqual(out["status"], "still_open")


class NeverRaiseTest(unittest.TestCase):
    class Broken:
        def __getattr__(self, name):
            def fail(*_a, **_k):
                raise RuntimeError("boom")

            return fail

    def test_every_handler_returns_json_even_when_windows_calls_fail(self):
        t = PcTools(self.Broken(), own_pid=1)
        for handler, args in [
            (t.list_running_apps, {}),
            (t.open_app, {"name": "Notepad"}),  # falls back to the built-in list, then launch fails
            (t.close_app, {"name": "notepad"}),
        ]:
            with self.subTest(handler=handler.__name__):
                out = answer(handler(args, task_id="t", session_id="s"))
                self.assertIn("error", out)

    def test_off_windows_the_tools_say_so(self):
        t = PcTools(None)
        for handler in (t.list_running_apps, t.open_app, t.close_app):
            self.assertIn("Windows only", answer(handler({"name": "x"}))["error"])


class RegisterToolsTest(unittest.TestCase):
    def test_the_three_tools_are_registered_in_the_kairos_toolset(self):
        registered = {}

        class FakeCtx:
            def register_hook(self, name, callback):
                pass

            def register_tool(self, name, toolset, schema, handler, check_fn=None):
                registered[name] = (toolset, schema, handler, check_fn)

        kairos.register_tools(FakeCtx())
        self.assertEqual(set(registered), {"list_running_apps", "open_app", "close_app"})
        for name, (toolset, schema, handler, check_fn) in registered.items():
            self.assertEqual(toolset, "kairos")
            self.assertEqual(schema["name"], name)
            self.assertIn("instead of terminal", schema["description"])
            self.assertIsInstance(json.loads(handler({})), dict)
            self.assertTrue(callable(check_fn))

    def test_the_steering_hook_is_registered_only_where_the_tools_exist(self):
        def hooks_registered(platform):
            hooks = []

            class FakeCtx:
                def register_hook(self, name, callback):
                    hooks.append((name, callback))

                def register_tool(self, **kwargs):
                    pass

            # winapps.py loads Windows DLLs at import; stub the layer so this runs on any OS.
            with mock.patch.object(kairos.sys, "platform", platform), mock.patch.object(
                kairos, "pc_tools", lambda: PcTools(None)
            ):
                kairos.register_tools(FakeCtx())
            return hooks

        self.assertEqual(hooks_registered("win32"), [("pre_llm_call", steering)])
        self.assertEqual(hooks_registered("linux"), [])

    def test_a_hermes_without_tool_registration_keeps_the_hooks(self):
        class OldCtx:
            def register_hook(self, name, callback):
                pass

        kairos.register_tools(OldCtx())  # no exception


class SteeringTest(unittest.TestCase):
    """The pre_llm_call note that points the model at the PC tools (M3 acceptance finding)."""

    def test_turns_about_apps_get_the_note(self):
        for message in ("open notepad", "Close it", "i closed it so reopen it", "what apps are open",
                        "Launch Spotify please", "quit chrome", "which programs are running?"):
            with self.subTest(message):
                self.assertEqual(steering(user_message=message), {"context": STEERING})

    def test_other_turns_get_nothing(self):
        for message in ("Reply with exactly: Kairos OK", "summarise this README", "what is 2+2?", "", None):
            with self.subTest(message):
                self.assertIsNone(steering(user_message=message))

    def test_a_multimodal_turn_is_read_from_its_text_parts(self):
        parts = [{"type": "image_url", "image_url": {"url": "data:,"}}, {"type": "text", "text": "close this app"}]
        self.assertEqual(steering(user_message=parts), {"context": STEERING})

    def test_the_note_names_the_tools_and_the_commands_to_avoid(self):
        for word in ("open_app", "close_app", "list_running_apps", "taskkill", "tasklist", "save"):
            self.assertIn(word, STEERING)

    def test_unexpected_keywords_are_tolerated(self):
        self.assertEqual(
            steering(user_message="open notepad", session_id="s", conversation_history=[], is_first_turn=True,
                     model="m", platform="api_server", future=1),
            {"context": STEERING},
        )


if __name__ == "__main__":
    unittest.main()
