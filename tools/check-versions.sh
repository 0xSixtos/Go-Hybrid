#!/usr/bin/env bash
# tools/check-versions.sh — version-drift guard (Lisette check-go-versions.sh pattern).
#
# tools/env.sh is the SINGLE SOURCE OF TRUTH for the toolchain pins
# (PLAN.md Decision Log: "pin go1.26.9 / rust 1.99.0; do not float").
# Every other place that names a version must agree with it:
#   1. README.md quickstart line
#   2. hyb's compiled-in pin defaults (compiler/crates/hyb/src/main.rs)
#   3. PLAN.md environment row
# and the INSTALLED toolchains (when present) must match the pins exactly —
# the same contract `hyb build` enforces at runtime as HYB-E9001.
#
# Usage: tools/check-versions.sh          (exit 0 = consistent, 1 = drift)
#   tools/check-versions.sh --no-toolchain   (skip the installed-version probe,
#                                             for CI runners without the pins)
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# --- 1. Source of truth -----------------------------------------------------
# shellcheck disable=SC1091
source tools/env.sh >/dev/null 2>&1 || { echo "FAIL: cannot source tools/env.sh"; exit 1; }
: "${PINNED_GO:?PINNED_GO missing from tools/env.sh}"
: "${PINNED_RUST:?PINNED_RUST missing from tools/env.sh}"
GO_VER="${PINNED_GO#go}"   # 1.26.9 (README spells it without the leading `go`)

DRIFT=0
fail() { echo "FAIL: $*"; DRIFT=1; }
pass() { echo "ok:   $*"; }

echo "pins: $PINNED_GO / rust $PINNED_RUST (tools/env.sh)"

# --- 2. Cross-reference every version-bearing artifact ----------------------
grep -q "Go $GO_VER" README.md \
    && pass "README.md names Go $GO_VER" \
    || fail "README.md does not name Go $GO_VER"
grep -q "Rust $PINNED_RUST" README.md \
    && pass "README.md names Rust $PINNED_RUST" \
    || fail "README.md does not name Rust $PINNED_RUST"

grep -q "\"$PINNED_GO\"" compiler/crates/hyb/src/main.rs \
    && pass "hyb default pin matches ($PINNED_GO)" \
    || fail "hyb default pin (compiler/crates/hyb/src/main.rs) != $PINNED_GO"
grep -q "\"$PINNED_RUST\"" compiler/crates/hyb/src/main.rs \
    && pass "hyb default pin matches (rust $PINNED_RUST)" \
    || fail "hyb default pin (compiler/crates/hyb/src/main.rs) != rust $PINNED_RUST"

grep -q "$PINNED_GO" PLAN.md \
    && pass "PLAN.md environment row names $PINNED_GO" \
    || fail "PLAN.md does not name $PINNED_GO"
grep -q "$PINNED_RUST" PLAN.md \
    && pass "PLAN.md environment row names rust $PINNED_RUST" \
    || fail "PLAN.md does not name rust $PINNED_RUST"

# --- 3. Installed toolchains match the pins (skippable on bare CI) ----------
if [ "${1:-}" != "--no-toolchain" ]; then
    if command -v go >/dev/null 2>&1; then
        INSTALLED_GO="$(go version 2>/dev/null | awk '{print $3}')"
        [ "$INSTALLED_GO" = "$PINNED_GO" ] \
            && pass "installed go == $PINNED_GO" \
            || fail "installed go ($INSTALLED_GO) != $PINNED_GO"
    else
        pass "go not installed — skipping (hyb build enforces at runtime)"
    fi
    if command -v rustc >/dev/null 2>&1; then
        INSTALLED_RUST="$(rustc --version 2>/dev/null | awk '{print $2}')"
        [ "$INSTALLED_RUST" = "$PINNED_RUST" ] \
            && pass "installed rustc == $PINNED_RUST" \
            || fail "installed rustc ($INSTALLED_RUST) != $PINNED_RUST"
    else
        pass "rustc not installed — skipping (hyb build enforces at runtime)"
    fi
fi

# --- verdict ----------------------------------------------------------------
if [ "$DRIFT" -eq 0 ]; then
    echo "versions: CONSISTENT"
    exit 0
else
    echo "versions: DRIFT DETECTED — update tools/env.sh FIRST, then the artifacts above, in one commit"
    exit 1
fi
