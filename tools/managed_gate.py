#!/usr/bin/env python3
"""managed_gate.py — the "new managed block appeared" CI gate (row 14,
discussion #4; sits on top of --deny-managed/--report-managed the way the
reviewer asked: a managed block that is NOT in the committed baseline is a
review event and fails the build; REMOVING a block is always allowed).

Usage:
  python3 tools/managed_gate.py <hyb> <baseline.json> <prog.hy> [more.hy ...]
  python3 tools/managed_gate.py --rebaseline <hyb> <baseline.json> <prog...>

Baseline shape: {"version": 1, "blocks": {"<file>:<line>": {"reason": ...,
"category": ..., "tickets": [...]}}} — keyed by file:line so moved blocks
count as new (deliberate: moving a hatch is a review event too).
"""
import json
import subprocess
import sys

VALID = {"mutex", "atomic", "actor", "finalizer", "interop", "other"}


def audit(hyb, prog):
    r = subprocess.run(
        [hyb, "check", prog, "--json", "--report-managed"],
        capture_output=True, text=True, timeout=120,
    )
    try:
        out = json.loads(r.stdout)
    except json.JSONDecodeError:
        print(f"GATE ERROR: {prog}: hyb check did not emit JSON\n{r.stdout}\n{r.stderr}")
        sys.exit(2)
    rep = (out.get("summary") or {}).get("managed") or {}
    blocks = []
    for b in rep.get("blocks", []):
        if b.get("category") not in VALID:
            print(f"GATE ERROR: {prog}: category {b.get('category')!r} outside the closed set")
            sys.exit(2)
        blocks.append(b)
    # line numbers: re-scan the source for the reason literals (the JSON
    # report is source-ordered; pairing by order is stable for a single file)
    lines = []
    with open(prog, encoding="utf-8") as f:
        for ln, text in enumerate(f, 1):
            if "managed(" in text and "//" not in text.split("managed(")[0][-2:]:
                lines.append(ln)
    keyed = {}
    for i, b in enumerate(blocks):
        key = f"{prog}:{lines[i] if i < len(lines) else '?'}"
        keyed[key] = b
    return keyed


def main():
    args = sys.argv[1:]
    rebaseline = False
    if args and args[0] == "--rebaseline":
        rebaseline = True
        args = args[1:]
    hyb, baseline_path, progs = args[0], args[1], args[2:]

    try:
        baseline = json.load(open(baseline_path))
    except FileNotFoundError:
        baseline = {"version": 1, "blocks": {}}
    base = baseline.setdefault("blocks", {})

    current = {}
    for prog in progs:
        current.update(audit(hyb, prog))

    new = {k: v for k, v in current.items() if k not in base}
    removed = {k for k in base if k not in current}
    changed = {
        k
        for k in current
        if k in base and base[k] != current[k]
    }

    if rebaseline:
        baseline["blocks"] = current
        json.dump(baseline, open(baseline_path, "w"), indent=1, sort_keys=True)
        open(baseline_path, "a").write("\n")
        print(f"rebaseline: {len(current)} block(s) recorded in {baseline_path}")
        return

    fail = False
    for k, v in sorted(new.items()):
        print(f"NEW MANAGED BLOCK — review required: {k} [{v['category']}] {v['reason']!r}")
        fail = True
    for k in sorted(changed):
        print(f"MANAGED BLOCK CHANGED — review required: {k}")
        fail = True
    for k in sorted(removed):
        print(f"managed block removed (allowed): {k}")

    if fail:
        print(
            "managed gate: FAILED — new/changed managed blocks must land in "
            f"{baseline_path} via `tools/managed_gate.py --rebaseline` WITH review "
            "(the hatch is unsafe-by-default, discussion #4)"
        )
        sys.exit(1)
    print(f"managed gate: PASS ({len(current)} block(s), baseline {len(base)})")


if __name__ == "__main__":
    main()
