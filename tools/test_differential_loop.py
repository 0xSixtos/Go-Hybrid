"""Offline CLI regressions for the differential harness, using a fake builder.

These exercise harness decisions, not Hybrid compiler correctness.
Run: python3 -m unittest discover -s tools -p test_differential_loop.py -v
"""
import pathlib
import subprocess
import sys
import tempfile
import unittest


HARNESS = pathlib.Path(__file__).with_name("differential_loop.py")


class RuntimeHazardTests(unittest.TestCase):
    def run_case(self, stderr="", stdout="", exit_code=2,
                 hazard="concurrent map writes (Go fatal, pre-registered)"):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            corpus = root / "corpus"
            corpus.mkdir()
            source = "// EXPECTED-VERDICT: ACCEPT\nfn main() {}\n"
            if hazard is not None:
                source += f"// RUNTIME-HAZARD: {hazard}\n"
            (corpus / "01-case.hy").write_text(source, encoding="utf-8")
            program = (f"#!{sys.executable}\nimport sys\n"
                       f"sys.stderr.write({stderr!r})\n"
                       f"sys.stdout.write({stdout!r})\n"
                       f"sys.exit({exit_code})\n")
            builder = root / "hyb"
            builder.write_text(
                f"#!{sys.executable}\nimport pathlib\n"
                f"p = pathlib.Path('a.out')\np.write_text({program!r})\n"
                "p.chmod(0o755)\n", encoding="utf-8")
            builder.chmod(0o755)
            return subprocess.run(
                [sys.executable, str(HARNESS), "--bin", str(builder),
                 "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10,
            )

    def assert_result(self, result, code):
        self.assertEqual(result.returncode, code, result.stdout + result.stderr)
        self.assertEqual("FULL AGREEMENT" in result.stdout, code == 0)

    def test_registered_fatal_is_allowed(self):
        self.assert_result(self.run_case(stderr="fatal error: concurrent map writes\n"), 0)

    def test_registered_fatal_before_long_stack_is_allowed(self):
        self.assert_result(self.run_case(
            stderr="fatal error: concurrent map writes\n" + "stack frame\n" * 100), 0)

    def test_unrelated_panic_fails(self):
        self.assert_result(self.run_case(stderr="panic: runtime error: index out of range\n"), 1)

    def test_unrelated_fatal_fails(self):
        self.assert_result(self.run_case(stderr="fatal error: all goroutines are asleep - deadlock!\n"), 1)

    def test_hazard_words_in_stdout_do_not_allow_unrelated_failure(self):
        self.assert_result(self.run_case(
            stdout="concurrent map writes", stderr="panic: unexpected nil pointer\n"), 1)

    def test_unregistered_fatal_fails(self):
        self.assert_result(self.run_case(
            stderr="fatal error: concurrent map writes\n", hazard=None), 1)

    def test_empty_normalized_hazard_does_not_match(self):
        self.assert_result(self.run_case(
            stderr="panic: unexpected nil pointer\n", hazard="(explanation only)"), 1)

    def test_successful_program_is_allowed(self):
        self.assert_result(self.run_case(exit_code=0), 0)


if __name__ == "__main__":
    unittest.main()
