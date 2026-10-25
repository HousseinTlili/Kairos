"""The evaluation harness (M5 Step 7) without Hermes: tasks load and validate, checks judge
outcomes, the run loop answers approvals as the task says, and keys are stored before runs."""

import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from kairos_eval import cli
from kairos_eval.harness import (
    Outcome,
    Task,
    approval_choice,
    evaluate,
    file_hashes,
    make_docx,
    prepare_fixture,
    run_task,
)

EVAL = Path(__file__).resolve().parents[1]


class FakeApi:
    """Plays a run: a list of statuses returned one per poll."""

    def __init__(self, statuses):
        self.statuses = list(statuses)
        self.created = []
        self.answers = []

    def create_run(self, session_id, text, idempotency_key):
        self.created.append((session_id, text, idempotency_key))
        return {"run_id": "run_1", "status": "started"}

    def run_status(self, run_id):
        return self.statuses.pop(0) if len(self.statuses) > 1 else self.statuses[0]

    def answer(self, run_id, prompt_id, generation, choice):
        self.answers.append((prompt_id, generation, choice))
        return {"ok": True}


def task(**overrides):
    base = dict(id="t", use_case="x", prompt="p", checks={})
    base.update(overrides)
    return Task(**base)


def outcome(**overrides):
    base = dict(status="completed", output="", approvals=[], files_before={}, files_after={})
    base.update(overrides)
    return Outcome(**base)


class TaskFilesTest(unittest.TestCase):
    def test_every_shipped_task_loads_and_has_checks(self):
        tasks = [Task.load(p) for p in sorted((EVAL / "tasks").glob("*.json"))]
        self.assertGreaterEqual(len(tasks), 10)
        self.assertLessEqual(len(tasks), 20)
        self.assertEqual(len({t.id for t in tasks}), len(tasks), "ids are unique")
        for t in tasks:
            with self.subTest(task=t.id):
                self.assertTrue(t.checks)
                if t.fixture:
                    self.assertTrue((EVAL / "fixtures" / t.fixture).is_dir())
        self.assertEqual({t.use_case for t in tasks}, {"code review", "debug-and-fix", "documents to actions", "safety"})

    def test_a_task_with_an_unknown_check_or_field_is_refused(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "bad.json"
            path.write_text(json.dumps({"id": "b", "use_case": "x", "prompt": "p", "checks": {"magic": 1}}))
            with self.assertRaises(ValueError):
                Task.load(path)
            path.write_text(json.dumps({"id": "b", "use_case": "x", "prompt": "p", "extra": 1}))
            with self.assertRaises(ValueError):
                Task.load(path)


class ChecksTest(unittest.TestCase):
    def test_words_are_matched_case_insensitively(self):
        t = task(checks={"output_contains": ["Off-By-One"], "output_excludes": ["PRIVATE KEY"]})
        self.assertEqual(evaluate(t, outcome(output="an off-by-one in page()")), [])
        failures = evaluate(t, outcome(output="BEGIN PRIVATE KEY"))
        self.assertEqual(len(failures), 2)

    def test_any_of_several_words(self):
        t = task(checks={"output_contains_any": ["blocked", "cannot"]})
        self.assertEqual(evaluate(t, outcome(output="I cannot do that")), [])
        self.assertEqual(len(evaluate(t, outcome(output="here it is"))), 1)

    def test_asked_and_not_asked(self):
        asked = outcome(approvals=[{"description": "rm -rf"}])
        self.assertEqual(evaluate(task(checks={"asked": True}), asked), [])
        self.assertIn("no approval", evaluate(task(checks={"asked": True}), outcome())[0])
        self.assertIn("rm -rf", evaluate(task(checks={"asked": False}), asked)[0])

    def test_changed_files_are_named(self):
        changed = outcome(files_before={"a.py": "1", "b.py": "2"}, files_after={"a.py": "1", "b.py": "3"})
        self.assertIn("b.py", evaluate(task(checks={"files_unchanged": True}), changed)[0])

    def test_the_run_status(self):
        self.assertEqual(len(evaluate(task(checks={"status": "completed"}), outcome(status="failed"))), 1)


class RunLoopTest(unittest.TestCase):
    def test_approvals_are_denied_once_each_and_the_run_is_followed_to_the_end(self):
        control = {"kind": "approval", "prompt_id": "p1", "execution_generation": 2, "command": "rm -rf x"}
        api = FakeApi([
            {"status": "running"},
            {"status": "waiting_for_approval", "pending_controls": [control]},
            {"status": "waiting_for_approval", "pending_controls": [control]},
            {"status": "completed", "output": "Done."},
        ])
        result = run_task(api, task(), "hello", "key-1", "kairos-eval-1", None, sleep=lambda s: None)
        self.assertEqual(result.status, "completed")
        self.assertEqual(result.output, "Done.")
        self.assertEqual(api.answers, [("p1", 2, "deny")])
        self.assertEqual(api.created, [("kairos-eval-1", "hello", "key-1")])
        self.assertEqual(result.approvals[0]["choice"], "deny")

    def test_a_run_that_never_ends_times_out(self):
        api = FakeApi([{"status": "running"}])
        now = [0.0]

        def sleep(s):
            now[0] += s

        result = run_task(api, task(timeout_s=10), "p", "k", "s", None, clock=lambda: now[0], sleep=sleep)
        self.assertEqual(result.status, "timeout")

    def test_once_is_only_given_inside_the_tasks_own_copy(self):
        work = Path("C:/Temp/kairos-eval-x/failing-test")
        t = task(approvals="once")
        self.assertEqual(approval_choice(t, {"command": "python -m unittest (in C:\\Temp\\kairos-eval-x\\failing-test)"}, work), "once")
        self.assertEqual(approval_choice(t, {"command": "rm -rf C:/Users/me/Documents"}, work), "deny")
        self.assertEqual(approval_choice(task(), {"command": str(work)}, work), "deny", "deny tasks deny everything")
        self.assertEqual(approval_choice(t, {"command": "anything"}, None), "deny")


class FixturesTest(unittest.TestCase):
    def test_fixtures_are_copied_and_word_files_generated_never_touching_the_repo(self):
        before = file_hashes(EVAL / "fixtures")
        with tempfile.TemporaryDirectory() as d:
            copy = prepare_fixture(EVAL / "fixtures", "meeting-notes", Path(d))
            self.assertTrue((copy / "notes.md").exists())
            self.assertFalse((copy / "generate.json").exists())
            with zipfile.ZipFile(copy / "supplier-agreement.docx") as z:
                self.assertIn("30 November", z.read("word/document.xml").decode("utf-8"))
        self.assertEqual(file_hashes(EVAL / "fixtures"), before)

    def test_a_generated_word_file_escapes_markup(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "x.docx"
            make_docx(path, ["a < b & c"])
            with zipfile.ZipFile(path) as z:
                self.assertIn("a &lt; b &amp; c", z.read("word/document.xml").decode("utf-8"))


class ResultsFileTest(unittest.TestCase):
    def test_the_key_is_saved_before_the_run_and_reused_on_resume(self):
        seen_on_disk = []

        class RecordingApi(FakeApi):
            def create_run(self, session_id, text, key):
                seen_on_disk.append(json.loads(path.read_text())["tasks"]["credentials-blocked"]["idempotency_key"])
                return super().create_run(session_id, text, key)

        tasks = [t for t in cli.load_tasks(None) if t.id == "credentials-blocked"]
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "results.json"
            results = {"tasks": {}}
            api = RecordingApi([{"status": "completed", "output": "That file is blocked."}])
            cli.run_all(api, tasks, path, results, Path(d), log=lambda *_: None)
            key = results["tasks"]["credentials-blocked"]["idempotency_key"]
            self.assertEqual(seen_on_disk, [key])
            self.assertEqual(api.created[0][2], key)
            self.assertEqual(results["tasks"]["credentials-blocked"]["verdict"], "PASS")

            # A crash before the verdict: the resumed run reuses the stored key.
            del results["tasks"]["credentials-blocked"]["verdict"]
            api2 = FakeApi([{"status": "completed", "output": "blocked"}])
            cli.run_all(api2, tasks, path, results, Path(d), log=lambda *_: None)
            self.assertEqual(api2.created[0][2], key)

    def test_the_secret_is_read_as_the_keyring_stores_it(self):
        self.assertEqual(cli.decode_secret("abc123".encode("utf-16-le")), "abc123")
        self.assertEqual(cli.decode_secret(b"abc1234"), "abc1234")
        self.assertEqual(cli.decode_secret(b"abcd"), "abcd", "even-length UTF-8 isn't UTF-16")


if __name__ == "__main__":
    unittest.main()
