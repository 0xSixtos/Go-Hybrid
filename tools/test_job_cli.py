"""Regression tests for fail-closed CLI argument validation (no network)."""
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ENTRY = Path(__file__).with_name("job.py")

JOB_SPEC = importlib.util.spec_from_file_location("job_module", ENTRY)
JOB = importlib.util.module_from_spec(JOB_SPEC)
JOB_SPEC.loader.exec_module(JOB)


class ReadTargetArgumentTests(unittest.TestCase):
    """The CLI must pass the requested run/kernel, without contacting services."""

    def invoke(self, *arguments):
        import contextlib
        import importlib.util
        import io
        from unittest.mock import patch

        spec = importlib.util.spec_from_file_location("job_target_fixture", ENTRY)
        job = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(job)
        output = io.StringIO()
        with patch.object(sys, "argv", [str(ENTRY), *arguments]), \
                patch.object(job, "kaggle_status") as status, \
                patch.object(job, "actions_watch") as watch, \
                contextlib.redirect_stdout(output), \
                contextlib.redirect_stderr(output):
            try:
                job.main()
                code = 0
            except SystemExit as exc:
                code = exc.code
        return code, status, watch, output.getvalue()

    def test_explicit_kaggle_slug_reaches_status(self):
        code, status, watch, _ = self.invoke("kaggle", "status", "owner/kernel")
        self.assertEqual(code, 0)
        status.assert_called_once_with("owner/kernel")
        watch.assert_not_called()

    def test_explicit_actions_run_reaches_watch(self):
        code, status, watch, _ = self.invoke("actions", "watch", "123456789")
        self.assertEqual(code, 0)
        watch.assert_called_once_with(123456789)
        status.assert_not_called()

    def test_omitted_targets_preserve_default_selection(self):
        for tier, action in (("kaggle", "status"), ("actions", "watch")):
            with self.subTest(tier=tier):
                code, status, watch, _ = self.invoke(tier, action)
                self.assertEqual(code, 0)
                selected = status if tier == "kaggle" else watch
                selected.assert_called_once_with(None)

    def test_surplus_arguments_fail_before_any_service_call(self):
        for tier, action, target in (("kaggle", "status", "owner/kernel"),
                                     ("actions", "watch", "123456789")):
            with self.subTest(tier=tier):
                code, status, watch, _ = self.invoke(tier, action, target, "unexpected")
                self.assertEqual(code, 2)
                status.assert_not_called()
                watch.assert_not_called()

    def test_invalid_actions_run_reports_usage_before_service_call(self):
        for target in ("invalid", "0", "-1"):
            with self.subTest(target=target):
                code, status, watch, output = self.invoke("actions", "watch", target)
                self.assertEqual(code, 2)
                self.assertIn("positive integer", output)
                status.assert_not_called()
                watch.assert_not_called()


class ActionsDispatchArgumentTests(unittest.TestCase):
    def call_dispatch(self, *args):
        return subprocess.run(
            [sys.executable, str(ENTRY), "actions", "dispatch", *args],
            capture_output=True,
            text=True,
            timeout=5,
        )

    def test_missing_parallelism_reports_usage_not_traceback(self):
        result = self.call_dispatch("dummy.yml", "0123456", "demo", "3")
        self.assertEqual(result.returncode, 2)
        self.assertIn("<maxparallel>", result.stdout)
        self.assertNotIn("Traceback", result.stdout + result.stderr)

    def test_invalid_parallelism_reports_error_not_traceback(self):
        result = self.call_dispatch("dummy.yml", "0123456", "demo", "3", "bad")
        self.assertEqual(result.returncode, 2)
        self.assertIn("positive integers", result.stderr)
        self.assertNotIn("Traceback", result.stdout + result.stderr)

    def test_nonpositive_values_are_rejected(self):
        for shards, parallel in [("0", "2"), ("3", "-1"), ("-2", "1")]:
            with self.subTest(shards=shards, parallel=parallel):
                result = self.call_dispatch("dummy.yml", "0123456", "demo", shards, parallel)
                self.assertEqual(result.returncode, 2)
                self.assertIn("positive integers", result.stderr)
                self.assertNotIn("Traceback", result.stdout + result.stderr)

    def test_invalid_shards_reports_error_not_traceback(self):
        result = self.call_dispatch("dummy.yml", "0123456", "demo", "bad", "2")
        self.assertEqual(result.returncode, 2)
        self.assertIn("positive integers", result.stderr)
        self.assertNotIn("Traceback", result.stdout + result.stderr)


class FetchFailureTests(unittest.TestCase):
    """A fetch that retrieves nothing must exit non-zero, never 0."""

    def test_kaggle_fetch_failed_download_is_nonzero(self):
        with tempfile.TemporaryDirectory() as td:
            with patch.object(JOB, "sh", return_value=(1, "kaggle: not authorized")):
                rc = JOB.kaggle_fetch(td, slug="demo/kernel")
            self.assertEqual(rc, 1)

    def test_kaggle_fetch_missing_summary_is_nonzero(self):
        with tempfile.TemporaryDirectory() as td:
            with patch.object(JOB, "sh", return_value=(0, "downloaded nothing")):
                rc = JOB.kaggle_fetch(td, slug="demo/kernel")
            self.assertEqual(rc, 1)

    def test_kaggle_fetch_present_summary_is_zero(self):
        with tempfile.TemporaryDirectory() as td:
            raw = Path(td) / "result" / "raw" / "nested"
            raw.mkdir(parents=True)
            (raw / "summary.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
            with patch.object(JOB, "sh", return_value=(0, "ok")):
                rc = JOB.kaggle_fetch(td, slug="demo/kernel")
            self.assertEqual(rc, 0)
            self.assertTrue((Path(td) / "result" / "summary.json").exists())

    def test_actions_fetch_empty_artifact_list_is_nonzero(self):
        with tempfile.TemporaryDirectory() as td:
            with patch.object(JOB, "gh_api", return_value={"total_count": 0, "artifacts": []}):
                rc = JOB.actions_fetch(123, td)
            self.assertEqual(rc, 1)

    def test_actions_fetch_error_payload_is_nonzero(self):
        with tempfile.TemporaryDirectory() as td:
            with patch.object(JOB, "gh_api", return_value={"message": "Not Found", "status": "404"}):
                rc = JOB.actions_fetch(123, td)
            self.assertEqual(rc, 1)

class ActionsArtifactPaginationTests(unittest.TestCase):
    """Exercise complete ZIP retrieval against the documented page defaults."""
    def exercise_fetch(self, count, failing_page=None, failing_download=False):
        import contextlib
        import importlib.util
        import io
        import tempfile
        import zipfile
        from urllib.parse import parse_qs, urlparse
        from unittest.mock import patch

        spec = importlib.util.spec_from_file_location("job_pagination_fixture", ENTRY)
        job = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(job)
        api_pages, downloaded = [], []
        artifacts, bodies = [], {}
        for index in range(count):
            body = io.BytesIO()
            with zipfile.ZipFile(body, "w") as archive:
                archive.writestr("evidence.txt", "artifact-%d" % index)
            bodies[index] = body.getvalue()
            artifacts.append({"id": index, "name": "shard-%03d" % index,
                              "size_in_bytes": len(bodies[index]),
                              "archive_download_url": "https://artifacts.invalid/%d" % index})

        def api(path):
            query = parse_qs(urlparse(path).query)
            size = int(query.get("per_page", ["30"])[0])
            page = int(query.get("page", ["1"])[0])
            api_pages.append(page)
            if page == failing_page:
                raise RuntimeError("synthetic listing failure")
            start = (page - 1) * size
            return {"total_count": count, "artifacts": artifacts[start:start + size]}

        def download(req, timeout):
            self.assertEqual(timeout, 120)
            if failing_download:
                raise OSError("synthetic download failure")
            index = int(urlparse(req.full_url).path.rsplit("/", 1)[1])
            downloaded.append(index)
            return io.BytesIO(bodies[index])

        with tempfile.TemporaryDirectory() as directory:
            with patch.object(job, "gh_api", side_effect=api), \
                    patch.object(job, "gh_token", return_value="synthetic-fixture"), \
                    patch.object(job.urllib.request, "urlopen", side_effect=download), \
                    contextlib.redirect_stdout(io.StringIO()):
                if failing_page is not None:
                    with self.assertRaisesRegex(RuntimeError, "listing failure"):
                        job.actions_fetch(123, directory)
                    self.assertEqual(downloaded, [])
                    self.assertFalse((Path(directory) / "result").exists())
                    return
                if failing_download:
                    with self.assertRaisesRegex(OSError, "download failure"):
                        job.actions_fetch(123, directory)
                    return
                job.actions_fetch(123, directory)
            files = sorted((Path(directory) / "result").glob("*.zip"))
            self.assertEqual(len(files), count)
            self.assertEqual(downloaded, list(range(count)))
            for index, file in enumerate(files):
                self.assertEqual(file.read_bytes(), bodies[index])
                with zipfile.ZipFile(file) as archive:
                    self.assertEqual(archive.read("evidence.txt").decode(), "artifact-%d" % index)
            self.assertEqual(api_pages, list(range(1, count // 100 + 2)))

    def test_retrieves_more_than_default_thirty_artifacts(self):
        self.exercise_fetch(31)

    def test_retrieves_later_pages(self):
        self.exercise_fetch(101)

    def test_empty_small_and_exact_page_boundaries(self):
        for count in (0, 1, 30, 100, 200):
            with self.subTest(count=count):
                self.exercise_fetch(count)

    def test_later_page_error_does_not_start_partial_download(self):
        self.exercise_fetch(101, failing_page=2)

    def test_download_errors_still_propagate(self):
        self.exercise_fetch(1, failing_download=True)


if __name__ == "__main__":
    unittest.main()
