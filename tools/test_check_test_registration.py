"""Regression tests for the test-registration guard (offline, stdlib only).

Each case builds a throwaway repo tree plus a throwaway workflow, so the
guard is exercised against fixtures rather than the live repository.
"""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "check_test_registration", Path(__file__).with_name("check_test_registration.py"))
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class PatternExtractionTests(unittest.TestCase):
    def test_quoted_and_bare_patterns_are_both_read(self):
        workflow = (
            "          python3 -m unittest discover -s tools/fuzz -p 'test_hyfuzz.py'\n"
            "          python3 -m unittest discover -s tools -p test_managed_gate.py -v\n"
        )
        self.assertEqual(guard.registered_patterns(workflow),
                         ["test_hyfuzz.py", "test_managed_gate.py"])

    def test_non_discovery_python_lines_are_ignored(self):
        workflow = (
            "          python3 -m py_compile tools/job.py tools/corpus_harness.py\n"
            "          python3 tests/test_install_checksums.py\n"
            "          python3 tools/runner/runner.py job.json\n"
        )
        self.assertEqual(guard.registered_patterns(workflow), [])


class RegistrationTests(unittest.TestCase):
    def build(self, workflow, suites):
        """Write a temp root with `suites` under tools/ and return its paths."""
        temporary = tempfile.TemporaryDirectory(prefix="hybrid-registration-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for rel in suites:
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# suite\n", encoding="utf-8")
        workflow_path = root / "ci.yml"
        workflow_path.write_text(workflow, encoding="utf-8")
        return root, workflow_path

    def test_glob_pattern_registers_a_whole_family(self):
        root, workflow = self.build(
            "          python3 -m unittest discover -s tools -p 'test_job*.py'\n",
            ["tools/test_job_cli.py", "tools/test_job_payload.py", "tools/test_job_artifacts.py"])
        self.assertEqual(guard.find_tests(root), [
            "tools/test_job_artifacts.py", "tools/test_job_cli.py", "tools/test_job_payload.py"])
        self.assertTrue(all(guard.is_registered(Path(t).name, guard.registered_patterns(
            workflow.read_text(encoding="utf-8"))) for t in guard.find_tests(root)))

    def test_unregistered_suite_is_detected(self):
        root, _ = self.build(
            "          python3 -m unittest discover -s tools -p test_job_cli.py -v\n",
            ["tools/test_job_cli.py", "tools/test_job_payload.py"])
        patterns = guard.registered_patterns(
            "          python3 -m unittest discover -s tools -p test_job_cli.py -v\n")
        orphaned = [t for t in guard.find_tests(root)
                    if not guard.is_registered(Path(t).name, patterns)]
        self.assertEqual(orphaned, ["tools/test_job_payload.py"])

    def test_nested_suite_is_found_and_checked(self):
        root, _ = self.build(
            "          python3 -m unittest discover -s tools -p test_none.py\n",
            ["tools/fuzz/test_hyfuzz.py", "tools/runner/test_runner.py"])
        self.assertEqual(guard.find_tests(root),
                         ["tools/fuzz/test_hyfuzz.py", "tools/runner/test_runner.py"])


class GuardMisuseTests(unittest.TestCase):
    def run_main(self, workflow, with_suites=False):
        """Run the real main() against a throwaway workflow/root pair."""
        with tempfile.TemporaryDirectory(prefix="hybrid-guard-") as temporary:
            root = Path(temporary)
            path = root / "ci.yml"
            path.write_text(workflow, encoding="utf-8")
            tools = root / "tools"
            tools.mkdir()
            if with_suites:
                (tools / "test_x.py").write_text("# suite\n", encoding="utf-8")
            original = guard.sys.argv
            guard.sys.argv = ["check_test_registration.py",
                              "--workflow", str(path), "--root", str(root)]
            try:
                with contextlib.redirect_stderr(io.StringIO()):
                    return guard.main()
            finally:
                guard.sys.argv = original

    def test_workflow_without_discovery_pattern_fails_closed(self):
        self.assertEqual(self.run_main("python3 -m py_compile tools/job.py\n",
                                       with_suites=True), 2)

    def test_root_without_suites_fails_closed(self):
        self.assertEqual(
            self.run_main("python3 -m unittest discover -s tools -p test_x.py\n"), 2)

    def test_fully_registered_tree_passes(self):
        self.assertEqual(
            self.run_main("python3 -m unittest discover -s tools -p test_x.py\n",
                          with_suites=True), 0)


if __name__ == "__main__":
    unittest.main()
