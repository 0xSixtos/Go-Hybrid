#!/usr/bin/env python3
"""check_conflict_markers.py — fail closed if a merge-conflict marker block
reached the tree.

A conflict marker that lands in a tracked file is invisible to the test
suites (nothing imports a YAML workflow or a Markdown doc) but it can take
the whole pipeline down: `.github/workflows/ci.yml` with a marker block is
invalid YAML, so the workflow never parses, no job is created, and every run
reports failure with no logs — while blocking every open pull request,
because a PR is tested against its base.

Only a real conflict BLOCK counts: a `<<<<<<<` line followed later by a
`>>>>>>>` line. A lone `=======` on its own is NOT reported, because a
setext heading underline or a horizontal rule is legitimate Markdown and
this repository uses them.

Stdlib only. Exit codes: 0 = clean, 1 = markers found, 2 = misuse.
"""
import argparse
import os
import re
import sys

START_RE = re.compile(r"^<<<<<<<(\s|$)")
END_RE = re.compile(r"^>>>>>>>(\s|$)")

# Directories that are never tracked content.
SKIP_DIRS = {".git", "target", "node_modules", ".venv", "__pycache__"}

# Extensions worth reading as text; anything else is skipped rather than
# guessed at, so a binary file cannot produce a spurious hit.
TEXT_EXTS = {
    ".md", ".yml", ".yaml", ".json", ".toml", ".rs", ".go", ".py", ".sh",
    ".hy", ".txt", ".cfg", ".ini", ".html", ".css", ".js", ".ts",
}

# Files with no extension that still matter.
TEXT_NAMES = {"Makefile", "Dockerfile", "LICENSE", "NOTICE"}


def is_text_candidate(path):
    name = os.path.basename(path)
    if name in TEXT_NAMES:
        return True
    return os.path.splitext(name)[1].lower() in TEXT_EXTS


def scan_file(path):
    """Return [(line_no, line)] for conflict blocks in one file.

    A block is a `<<<<<<<` line and the first following `>>>>>>>` line; the
    start line is reported (the block itself is the evidence).
    """
    hits = []
    open_start = None
    try:
        with open(path, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                stripped = line.rstrip("\n")
                if START_RE.match(stripped):
                    open_start = (n, stripped)
                elif END_RE.match(stripped) and open_start is not None:
                    hits.append(open_start)
                    open_start = None
        # An unterminated `<<<<<<<` is still a marker that must not ship.
        if open_start is not None:
            hits.append(open_start)
    except (UnicodeDecodeError, OSError):
        return []
    return hits


def scan(root):
    """Yield (relpath, line_no, line) for every conflict start line."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in filenames:
            full = os.path.join(dirpath, fn)
            if not is_text_candidate(full):
                continue
            for n, line in scan_file(full):
                yield os.path.relpath(full, root), n, line


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".", help="tree to scan (default: cwd)")
    args = ap.parse_args()
    if not os.path.isdir(args.root):
        print(f"usage error: {args.root} is not a directory", file=sys.stderr)
        return 2
    found = list(scan(args.root))
    if not found:
        print("conflict markers: none")
        return 0
    print(f"conflict markers: {len(found)} block(s) found", file=sys.stderr)
    for rel, n, line in found:
        print(f"  {rel}:{n}: {line}", file=sys.stderr)
    print(
        "A committed conflict marker means a merge was closed with the "
        "markers still in place. Remove the block before this can land.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
