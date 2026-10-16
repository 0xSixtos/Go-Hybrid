#!/usr/bin/env python3
"""check_test_registration.py — every tools/**/test_*.py must be run by CI.

A merged regression suite that nothing invokes cannot fail the build: it
reads as a passing test while pinning nothing. Four suites had drifted out
of .github/workflows/ci.yml that way (test_job_cli, test_job_payload,
test_corpus_error_diagnostics, test_differential_verdict_markers) — each
landed by a PR that did not register it, so the green check meant less than
it looked.

This guard cross-checks the `unittest discover -p <pattern>` lines in the
workflow against the suites actually present under tools/, and fails closed
on any suite no pattern matches. A pattern may be a glob (`test_job*.py`),
so registering a family is one line.

Scope is tools/ only: tests/ holds standalone scripts invoked directly by
the workflow, not discovery-driven suites.

Usage: python3 tools/check_test_registration.py [--workflow PATH] [--root DIR]
Exit:  0 = every suite is registered; 1 = orphaned suite(s); 2 = guard misuse.
"""

import argparse
import fnmatch
import os
import re
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# One discovery invocation, quoted or bare pattern: `... discover -s X -p Y`.
DISCOVER_RE = re.compile(r"unittest\s+discover\b[^\n]*?-p\s+'?([^'\s]+)")


def registered_patterns(workflow_text):
    """Every `-p` pattern the workflow passes to `unittest discover`."""
    return DISCOVER_RE.findall(workflow_text)


def is_registered(name, patterns):
    return any(fnmatch.fnmatch(name, pattern) for pattern in patterns)


def find_tests(root):
    """Repo-relative paths of every tools/**/test_*.py under root."""
    found = []
    for base, _dirs, files in os.walk(os.path.join(root, "tools")):
        for filename in files:
            if filename.startswith("test_") and filename.endswith(".py"):
                rel = os.path.relpath(os.path.join(base, filename), root)
                found.append(rel.replace(os.sep, "/"))
    return sorted(found)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--workflow", default=os.path.join(REPO_ROOT, ".github", "workflows", "ci.yml"),
                    help="workflow whose discovery lines define registration")
    ap.add_argument("--root", default=REPO_ROOT, help="repository root holding tools/")
    args = ap.parse_args()

    with open(args.workflow, encoding="utf-8") as handle:
        workflow_text = handle.read()

    patterns = registered_patterns(workflow_text)
    if not patterns:
        print("REGISTRATION CHECK ERROR: no `unittest discover -p` pattern in "
              f"{args.workflow}; the guard cannot verify anything", file=sys.stderr)
        return 2

    tests = find_tests(args.root)
    if not tests:
        print(f"REGISTRATION CHECK ERROR: no tools/**/test_*.py under {args.root}",
              file=sys.stderr)
        return 2

    orphaned = [t for t in tests if not is_registered(os.path.basename(t), patterns)]
    if orphaned:
        print("UNREGISTERED TEST SUITE(S) — nothing runs these, so they cannot fail CI:")
        for test in orphaned:
            print(f"  {test}")
        print(f"Register each in {args.workflow} as a `python3 -m unittest discover -p …` "
              "line (a glob like `test_job*.py` covers a family), or delete it.")
        return 1

    print(f"test registration: OK ({len(tests)} suite(s), "
          f"{len(patterns)} discovery pattern(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
