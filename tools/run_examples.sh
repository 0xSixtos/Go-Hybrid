#!/usr/bin/env bash
# tools/run_examples.sh — MVP-0 walking-skeleton acceptance.
#
# Builds the Go-emittable examples through the FULL pipeline
# (parse → Tier-0 check → emit → go build) and diffs their runtime output
# against each example's expected.txt. Example 02 exercises both the happy
# path (stdout = bytes verbatim) and the E6 `?`-exit contract (the binary is
# built as `cat` so the <prog>: prefix matches the pinned expected output).
#
# Usage: tools/run_examples.sh [01 02 03]   (default: all emittable)
# Exit:  0 = all green, 1 = any mismatch.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-/home/z/go-hybrid-target}"
HYB="${HYB_BIN:-$TARGET_DIR/debug/hyb}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

FAIL=0
PASS=""

note() { printf '%s\n' "$*"; }

run_example() {
    local dir="$1"
    local name
    name="$(basename "$dir")"
    local prog="$dir/program.hy"
    local bin="$WORK/$name"

    note "--- $name"
    if ! "$HYB" build "$prog" -o "$bin" >"$WORK/$name.build.log" 2>&1; then
        note "FAIL $name (build)"
        sed 's/^/    /' "$WORK/$name.build.log"
        FAIL=1
        return
    fi

    case "$name" in
    01-hello|03-worker-pool|04-pipeline-backpressure)
        if "$bin" >"$WORK/$name.out" 2>&1 && diff -u "$dir/expected.txt" "$WORK/$name.out" >"$WORK/$name.diff"; then
            note "PASS $name (stdout == expected.txt)"
            PASS="$PASS $name"
        else
            note "FAIL $name (output diff)"
            sed 's/^/    /' "$WORK/$name.diff"
            FAIL=1
        fi
        ;;
    02-cli-cat)
        # E6 exit contract needs <prog> = basename(argv[0]) — build as `cat`.
        local catbin="$WORK/catbin/cat"
        mkdir -p "$WORK/catbin"
        if ! "$HYB" build "$prog" -o "$catbin" >"$WORK/$name.build.log" 2>&1; then
            note "FAIL $name (cat build)"
            sed 's/^/    /' "$WORK/$name.build.log"
            FAIL=1
            return
        fi
        printf 'hello from the file\nsecond line\n' >"$WORK/in.txt"
        local ok=1
        # happy path: stdout == input bytes verbatim, exit 0
        if "$catbin" "$WORK/in.txt" >"$WORK/$name.out" 2>"$WORK/$name.err" && \
           diff -u "$WORK/in.txt" "$WORK/$name.out" >/dev/null; then
            note "PASS $name happy path (bytes verbatim, exit 0)"
        else
            note "FAIL $name happy path"
            ok=0
        fi
        # error path: stderr "<prog>: open <missing>: no such file...", exit 1
        local missing="$WORK/definitely-not-here.txt"
        "$catbin" "$missing" >"$WORK/$name.out2" 2>"$WORK/$name.err2"
        local rc=$?
        local want="cat: open $missing: no such file or directory"
        if [ "$rc" -eq 1 ] && [ "$(cat "$WORK/$name.err2")" = "$want" ]; then
            note "PASS $name error path (E6: stderr + exit 1)"
        else
            note "FAIL $name error path (rc=$rc, stderr: $(cat "$WORK/$name.err2"))"
            ok=0
        fi
        if [ "$ok" -eq 1 ]; then PASS="$PASS $name"; else FAIL=1; fi
        ;;
    *)
        note "SKIP $name (not in the MVP-0 emittable set)"
        ;;
    esac
}

if [ $# -eq 0 ]; then
    set -- 01-hello 02-cli-cat 03-worker-pool 04-pipeline-backpressure
fi

for name in "$@"; do
    dir="$ROOT/examples/$name"
    if [ ! -f "$dir/program.hy" ]; then
        note "FAIL $name (no program.hy at $dir)"
        FAIL=1
        continue
    fi
    run_example "$dir"
done

note ""
if [ "$FAIL" -eq 0 ]; then
    note "examples: ALL PASS ($PASS)"
    exit 0
else
    note "examples: FAILURES (passed:$PASS)"
    exit 1
fi
