"""The Kairos skills (M5 Step 6) are in Hermes's SKILL.md format: front matter with a name that
matches the folder and a description of at most 60 characters, and the four standard sections."""

import re
import unittest
from pathlib import Path

SKILLS = Path(__file__).resolve().parents[2] / "skills" / "kairos"
SECTIONS = ["## When to Use", "## Procedure", "## Pitfalls", "## Verification"]


def front_matter(text):
    match = re.match(r"---\n(.*?)\n---\n", text, re.DOTALL)
    if not match:
        return None
    fields = {}
    for line in match.group(1).splitlines():
        if re.match(r"^[a-z_]+:", line):
            key, _, value = line.partition(":")
            fields[key] = value.strip()
    return fields


class SkillFormatTest(unittest.TestCase):
    def skill_files(self):
        files = sorted(SKILLS.glob("*/SKILL.md"))
        self.assertEqual(
            [f.parent.name for f in files],
            ["kairos-code-review", "kairos-debug-and-fix", "kairos-documents-to-actions"],
        )
        return files

    def test_each_skill_has_valid_front_matter(self):
        for path in self.skill_files():
            with self.subTest(skill=path.parent.name):
                fields = front_matter(path.read_text(encoding="utf-8"))
                self.assertIsNotNone(fields, "no front matter")
                self.assertEqual(fields.get("name"), path.parent.name)
                self.assertTrue(fields.get("description"))
                self.assertLessEqual(len(fields["description"]), 60)
                self.assertTrue(fields.get("version"))

    def test_each_skill_has_the_four_sections_in_order(self):
        for path in self.skill_files():
            with self.subTest(skill=path.parent.name):
                text = path.read_text(encoding="utf-8")
                positions = [text.find(s) for s in SECTIONS]
                self.assertNotIn(-1, positions, f"missing a section in {path.parent.name}")
                self.assertEqual(positions, sorted(positions))

    def test_the_read_only_and_output_folder_promises_are_written_down(self):
        review = (SKILLS / "kairos-code-review" / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("read-only", review.lower())
        documents = (SKILLS / "kairos-documents-to-actions" / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn(r"Documents\Kairos\output", documents)
        for path in self.skill_files():
            with self.subTest(skill=path.parent.name):
                self.assertRegex(path.read_text(encoding="utf-8"), r"never follow|never [^.]*instructions")


if __name__ == "__main__":
    unittest.main()
