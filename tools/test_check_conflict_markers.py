"""Tests for tools/check_conflict_markers.py (stdlib only, no network)."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check_conflict_markers.py")
SPEC = importlib.util.spec_from_file_location("conflict_markers", SCRIPT)
MOD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MOD)


class MarkerDetectionTests(unittest.TestCase):
    def _tree(self, files):
        tmp = tempfile.TemporaryDirectory(prefix="markers-")
        root = Path(tmp.name)
        for rel, content in files.items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content, encoding="utf-8")
        self.addCleanup(tmp.cleanup)
        return root

    def test_clean_tree_reports_nothing(self):
        root = self._tree({
            "README.md": "hello\nworld\n",
            "ci.yml": "on: push\njobs:\n  a:\n    steps: []\n",
        })
        self.assertEqual(list(MOD.scan(str(root))), [])

    def test_full_conflict_block_is_found(self):
        root = self._tree({"ci.yml": "a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> main\nd\n"})
        hits = list(MOD.scan(str(root)))
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0][0], "ci.yml")
        self.assertEqual(hits[0][1], 2)

    def test_unterminated_start_is_still_reported(self):
        root = self._tree({"x.md": "a\n<<<<<<< HEAD\nb\n"})
        hits = list(MOD.scan(str(root)))
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0][1], 2)

    def test_lone_end_marker_is_not_reported(self):
        root = self._tree({"x.md": "a\n>>>>>>> main\n"})
        self.assertEqual(list(MOD.scan(str(root))), [])

    def test_setext_underline_is_not_a_marker(self):
        # `=======` under a heading is legitimate Markdown, not a conflict.
        root = self._tree({"doc.md": "Title\n=======\n\ntext\n"})
        self.assertEqual(list(MOD.scan(str(root))), [])

    def test_indented_marker_is_not_reported(self):
        root = self._tree({"code.py": "    <<<<<<< HEAD\n    >>>>>>> x\n"})
        self.assertEqual(list(MOD.scan(str(root))), [])

    def test_two_blocks_in_one_file_are_both_found(self):
        root = self._tree({
            "y.md": "<<<<<<< a\n1\n>>>>>>> b\nok\n<<<<<<< c\n2\n>>>>>>> d\n"
        })
        hits = list(MOD.scan(str(root)))
        self.assertEqual(sorted(h[1] for h in hits), [1, 5])

    def test_unknown_extension_is_skipped(self):
        root = self._tree({"weird.xyz": "<<<<<<< HEAD\n>>>>>>> main\n"})
        self.assertEqual(list(MOD.scan(str(root))), [])

    def test_git_directory_is_skipped(self):
        root = self._tree({".git/COMMIT_EDITMSG": "<<<<<<< HEAD\n>>>>>>> main\n"})
        self.assertEqual(list(MOD.scan(str(root))), [])


class CliTests(unittest.TestCase):
    def _run(self, root):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(root)],
            capture_output=True, text=True, timeout=20,
        )

    def test_clean_exits_zero(self):
        with tempfile.TemporaryDirectory() as td:
            Path(td, "a.md").write_text("ok\n", encoding="utf-8")
            res = self._run(td)
            self.assertEqual(res.returncode, 0, res.stderr)
            self.assertIn("none", res.stdout)

    def test_marker_exits_one_and_names_the_file(self):
        with tempfile.TemporaryDirectory() as td:
            Path(td, "broken.yml").write_text(
                "a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> main\n", encoding="utf-8")
            res = self._run(td)
            self.assertEqual(res.returncode, 1)
            self.assertIn("broken.yml", res.stderr)
            self.assertIn("conflict marker", res.stderr.lower())

    def test_missing_root_is_usage_error(self):
        res = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", "no-such-dir-xyz"],
            capture_output=True, text=True, timeout=20,
        )
        self.assertEqual(res.returncode, 2)


if __name__ == "__main__":
    unittest.main()
