"""CLI regressions for optional exact diagnostic metadata (stdlib only)."""

from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

HARNESS = Path(__file__).with_name("corpus_harness.py")


class ExactDiagnosticCliTests(unittest.TestCase):
    def run_harness(self, header, code="HYB-E3002", verdict="REJECT A1c"):
        # A tiny deterministic checker exercises the complete harness CLI:
        # fixture metadata -> subprocess JSON -> rule/code comparison -> exit.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                f"// EXPECTED-VERDICT: {verdict}\n{header}\nfn main() {{}}\n",
                encoding="utf-8",
            )
            payload = {
                "summary": {"verdict": "reject"},
                "diagnostics": [{"severity": "error", "tier0_rule": "A1c", "code": code}],
            }
            checker = root / "hyb"
            checker.write_text(
                f"#!{sys.executable}\nimport json\n"
                f"print(json.dumps({payload!r}))\nraise SystemExit(1)\n",
                encoding="utf-8",
            )
            checker.chmod(0o700)
            return subprocess.run(
                [sys.executable, str(HARNESS), "--bin", str(checker), "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10,
            )

    def test_wrong_code_with_same_rule_fails(self):
        run = self.run_harness("// EXPECTED-DIAGNOSTIC: HYB-E3002", code="HYB-E3001")
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("expected first diagnostic HYB-E3002, got HYB-E3001", run.stdout)
        self.assertIn("verdict agreement: 0/1", run.stdout)

    def test_exact_code_match_passes(self):
        run = self.run_harness("// EXPECTED-DIAGNOSTIC: HYB-E3002")
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertIn("verdict agreement: 1/1", run.stdout)

    def test_old_fixture_without_header_keeps_rule_only_contract(self):
        run = self.run_harness("", code="HYB-E3001")
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertIn("verdict agreement: 1/1", run.stdout)

    def test_malformed_header_fails_closed(self):
        for header in (
            "// EXPECTED-DIAGNOSTIC: not-a-code",
            "// EXPECTED-DIAGNOSTIC HYB-E3002",
            "// EXPECTED-DIAGNOSTIC:",
        ):
            with self.subTest(header=header):
                run = self.run_harness(header)
                self.assertEqual(run.returncode, 2, run.stdout + run.stderr)
                self.assertIn("malformed EXPECTED-DIAGNOSTIC", run.stderr)

    def test_duplicate_headers_fail_closed(self):
        run = self.run_harness(
            "// EXPECTED-DIAGNOSTIC: HYB-E3002\n// EXPECTED-DIAGNOSTIC: HYB-E3002"
        )
        self.assertEqual(run.returncode, 2, run.stdout + run.stderr)
        self.assertIn("multiple EXPECTED-DIAGNOSTIC", run.stderr)

    def test_accept_fixture_cannot_declare_error_code(self):
        run = self.run_harness("// EXPECTED-DIAGNOSTIC: HYB-E3002", verdict="ACCEPT")
        self.assertEqual(run.returncode, 2, run.stdout + run.stderr)
        self.assertIn("without REJECT", run.stderr)


class CheckerEnvelopeCliTests(unittest.TestCase):
    def run_payload(self, payload, expected="ACCEPT", status=0):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                f"// EXPECTED-VERDICT: {expected} A1c\nfn main() {{}}\n", encoding="utf-8"
            )
            checker = root / "hyb"
            checker.write_text(
                f"#!{sys.executable}\nimport json\n"
                f"print(json.dumps({payload!r}))\nraise SystemExit({status})\n",
                encoding="utf-8",
            )
            checker.chmod(0o700)
            return subprocess.run(
                [sys.executable, str(HARNESS), "--bin", str(checker), "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10,
            )

    def assert_invalid(self, payload, expected="ACCEPT", status=0):
        run = self.run_payload(payload, expected, status)
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("harness-error", run.stdout)
        self.assertIn("verdict agreement: 0/1", run.stdout)
        self.assertNotIn("Traceback", run.stderr)

    def test_matching_counts_accept(self):
        payload = {"summary": {"verdict": "accept", "errors": 0, "warnings": 1},
                   "diagnostics": [{"severity": "warning", "code": "HYB-E8001"}]}
        run = self.run_payload(payload, expected="ACCEPT-MANAGED")
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_matching_counts_reject(self):
        payload = {"summary": {"verdict": "reject", "errors": 1, "warnings": 0},
                   "diagnostics": [{"severity": "error", "tier0_rule": "A1c"}]}
        run = self.run_payload(payload, expected="REJECT A1c", status=1)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_omitted_counts_keep_existing_contract(self):
        run = self.run_payload({"summary": {"verdict": "accept"}, "diagnostics": []})
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_reported_errors_must_match_diagnostics(self):
        self.assert_invalid({"summary": {"verdict": "accept", "errors": 1},
                             "diagnostics": []})
        self.assert_invalid({"summary": {"verdict": "reject", "errors": 0},
                             "diagnostics": [{"severity": "error", "tier0_rule": "A1c"}]},
                            expected="REJECT A1c", status=1)

    def test_reported_warnings_must_match_diagnostics(self):
        self.assert_invalid({"summary": {"verdict": "accept", "warnings": 1},
                             "diagnostics": []})

    def test_counts_require_nonnegative_integers(self):
        for count in (False, True, None, "0", 0.0, -1):
            with self.subTest(count=count):
                self.assert_invalid({"summary": {"verdict": "accept", "errors": count},
                                     "diagnostics": []})

    def test_invalid_envelope_shapes_fail_without_traceback(self):
        for payload in (
            None, [], {"summary": None, "diagnostics": []},
            {"summary": [], "diagnostics": []},
            {"summary": {"verdict": "accept"}, "diagnostics": None},
            {"summary": {"verdict": "accept"}, "diagnostics": {}},
            {"summary": {"verdict": "accept"}, "diagnostics": [None]},
            {"summary": {"verdict": "accept"}, "diagnostics": ["error"]},
        ):
            with self.subTest(payload=payload):
                self.assert_invalid(payload)

    def test_extra_fields_do_not_change_acceptance(self):
        run = self.run_payload({"summary": {"verdict": "accept", "errors": 0,
                                            "warnings": 0, "future": "kept"},
                                "diagnostics": [], "version": 1, "future": {}})
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)


@unittest.skipUnless(os.name == "posix",
                     "PATH-lookup substitution is POSIX exec semantics")
class CheckerPathCliTests(unittest.TestCase):
    """The validated --bin file must be the checker that runs (#477).

    `--bin` is verified with os.path.isfile and then handed to
    subprocess.run unchanged. On POSIX an unresolved bare name is
    re-resolved against PATH, so a same-named binary elsewhere on PATH
    can silently replace the validated local checker — a false verdict
    agreement or a false disagreement — and a PATH without such a binary
    raises FileNotFoundError even though the local file passed
    validation. POSIX shebang checkers stand in for hyb; the scenarios
    mirror the issue's local/PATH directory split.
    """

    REJECT_PAYLOAD = (
        "{'summary': {'verdict': 'reject'}, "
        "'diagnostics': [{'severity': 'error', 'tier0_rule': 'A1c'}]}"
    )
    ACCEPT_PAYLOAD = "{'summary': {'verdict': 'accept'}, 'diagnostics': []}"

    def make_checker(self, path, payload, status):
        path.write_text(
            f"#!{sys.executable}\nimport json\n"
            f"print(json.dumps({payload}))\nraise SystemExit({status})\n",
            encoding="utf-8",
        )
        path.chmod(0o700)

    def run_shadow(self, expected, local_payload, local_status, path_payload,
                   path_status, bin_name="hyb", path_value=None):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                f"// EXPECTED-VERDICT: {expected} A1c\nfn main() {{}}\n",
                encoding="utf-8",
            )
            local = root / "local"
            local.mkdir()
            self.make_checker(local / "hyb", local_payload, local_status)
            shadow = root / "shadow"
            shadow.mkdir()
            self.make_checker(shadow / "hyb", path_payload, path_status)
            env = dict(os.environ)
            env["PATH"] = str(shadow) if path_value is None else path_value
            return subprocess.run(
                [sys.executable, str(HARNESS), "--bin", bin_name,
                 "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10, env=env,
                cwd=str(local),
            )

    def test_path_checker_cannot_replace_the_validated_local_one(self):
        # Local rejects, PATH shadow accepts, fixture expects REJECT: the
        # shadow must not turn the verdict into a disagreement.
        run = self.run_shadow("REJECT", self.REJECT_PAYLOAD, 1,
                              self.ACCEPT_PAYLOAD, 0)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertIn("verdict agreement: 1/1", run.stdout)

    def test_path_checker_cannot_fake_agreement(self):
        # The dangerous direction: local rejects, the PATH shadow accepts,
        # and an ACCEPT fixture would report a false 1/1 agreement.
        run = self.run_shadow("ACCEPT", self.REJECT_PAYLOAD, 1,
                              self.ACCEPT_PAYLOAD, 0)
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("verdict agreement: 0/1", run.stdout)

    def test_absolute_bin_runs_the_validated_file(self):
        # An absolute --bin is unchanged by the resolution (idempotence):
        # the PATH shadow still must not win.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                "// EXPECTED-VERDICT: REJECT A1c\nfn main() {}\n",
                encoding="utf-8",
            )
            local = root / "local"
            local.mkdir()
            self.make_checker(local / "hyb", self.REJECT_PAYLOAD, 1)
            shadow = root / "shadow"
            shadow.mkdir()
            self.make_checker(shadow / "hyb", self.ACCEPT_PAYLOAD, 0)
            env = dict(os.environ)
            env["PATH"] = str(shadow)
            run = subprocess.run(
                [sys.executable, str(HARNESS), "--bin", str(local / "hyb"),
                 "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10, env=env,
            )
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            self.assertIn("verdict agreement: 1/1", run.stdout)

    def test_path_without_the_checker_runs_the_local_file(self):
        # A PATH that carries no such binary used to raise
        # FileNotFoundError after the local file passed validation. The
        # empty PATH dir is created inside this test's own root so the
        # scenario cannot silently stop testing the clean-PATH branch.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                "// EXPECTED-VERDICT: REJECT A1c\nfn main() {}\n",
                encoding="utf-8",
            )
            local = root / "local"
            local.mkdir()
            self.make_checker(local / "hyb", self.REJECT_PAYLOAD, 1)
            empty_path = root / "empty-path"
            empty_path.mkdir()
            env = dict(os.environ)
            env["PATH"] = str(empty_path)
            run = subprocess.run(
                [sys.executable, str(HARNESS), "--bin", "hyb",
                 "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10, env=env,
                cwd=str(local),
            )
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            self.assertIn("verdict agreement: 1/1", run.stdout)
            self.assertNotIn("Traceback", run.stderr)

    def test_non_executable_bin_reports_harness_error(self):
        # isfile proves existence, not executability: a validated but
        # non-executable --bin must surface as a harness-error row, not
        # an uncaught PermissionError traceback.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = root / "corpus"
            corpus.mkdir()
            (corpus / "01-example.hy").write_text(
                "// EXPECTED-VERDICT: REJECT A1c\nfn main() {}\n",
                encoding="utf-8",
            )
            local = root / "local"
            local.mkdir()
            checker = local / "hyb"
            self.make_checker(checker, self.REJECT_PAYLOAD, 1)
            checker.chmod(0o644)
            run = subprocess.run(
                [sys.executable, str(HARNESS), "--bin", str(checker),
                 "--corpus", str(corpus)],
                capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
            self.assertIn("harness-error", run.stdout)
            self.assertIn("verdict agreement: 0/1", run.stdout)
            self.assertNotIn("Traceback", run.stderr)

    def test_missing_bin_still_fails_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = subprocess.run(
                [sys.executable, str(HARNESS), "--bin",
                 str(Path(tmp) / "absent" / "hyb"), "--corpus", str(Path(tmp))],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(run.returncode, 2, run.stdout + run.stderr)
            self.assertIn("checker binary not found", run.stderr)


if __name__ == "__main__":
    unittest.main()
