#!/bin/sh
# package_release.sh — reproducible release packaging (v0.1.0's tarball was
# assembled ad-hoc; this script is the lesson, not a repeat).
#
# Produces: dist/hybrid-<ver>-<os>-<arch>.tar.gz (+ .sha256)
#
# Layout (the contract install.sh relies on):
#   hybrid-<ver>/hyb                          the CLI binary (prebuilt)
#   hybrid-<ver>/INSTALL.md                   install notes (Rust-FREE path)
#   hybrid-<ver>/runtime/go/...               the Go binding packages
#   hybrid-<ver>/runtime/rust/hyrt/...        bridge sources (contrib path)
#   hybrid-<ver>/runtime/rust/hyrt/target/release/libhyrt.a
#          ^^^ PREBUILT staticlib, mtime >= every source — the freshness
#              check in hyb's ensure_staticlib() then reuses it and the
#              user NEVER needs a Rust toolchain. (Discussion #1, ask 2:
#              "If I have to build Rust first, I'll wait.")
#
# Usage: tools/package_release.sh v0.1.0
set -eu

VER="${1:?usage: package_release.sh <version-tag, e.g. v0.1.0>}"
VERNUM="${VER#v}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"   # linux / darwin
ARCH="$(uname -m)"                              # x86_64 / aarch64 / arm64
case "$ARCH" in
    x86_64) ARCH=amd64 ;;
    aarch64) ARCH=arm64 ;;
esac
DIST="$REPO_ROOT/dist"
NAME="hybrid-${VER}-${OS}-${ARCH}"
DIR="$DIST/hybrid-${VER}"

echo "==> packaging $NAME"

# --- 0. toolchain sanity (the pins are the contract; HYB-E9001 enforces) ---
command -v cargo >/dev/null || { echo "cargo not on PATH" >&2; exit 1; }
command -v go >/dev/null || { echo "go not on PATH" >&2; exit 1; }

# --- 1. build the staticlib CRATE-LOCALLY (env.sh's shared CARGO_TARGET_DIR
#        redirects manual builds — the stale-archive trap, worklog session 8).
#        The lib must land at runtime/rust/hyrt/target/release/. ---
echo "==> building hyrt staticlib (crate-local)"
( cd "$REPO_ROOT/runtime/rust/hyrt" && CARGO_BUILD_JOBS=2 cargo build --release --quiet )

# --- 2. build the CLI ---
echo "==> building hyb (release)"
( cd "$REPO_ROOT/compiler" && cargo build --release -p hyb --quiet )

# --- 3. assemble the tree ---
rm -rf "$DIR"
mkdir -p "$DIR"
cp "$REPO_ROOT/compiler/target/release/hyb" "$DIR/hyb"
cp "$REPO_ROOT/tools/RELEASE-INSTALL.md" "$DIR/INSTALL.md"
mkdir -p "$DIR/runtime"
cp -R "$REPO_ROOT/runtime/go" "$DIR/runtime/go"
cp -R "$REPO_ROOT/runtime/rust" "$DIR/runtime/rust"
# strip the crate's build dir except the prebuilt staticlib, then re-lay it
find "$DIR/runtime/rust" -type d -name target -exec rm -rf {} + 2>/dev/null || true
mkdir -p "$DIR/runtime/rust/hyrt/target/release"
cp "$REPO_ROOT/runtime/rust/hyrt/target/release/libhyrt.a" \
   "$DIR/runtime/rust/hyrt/target/release/libhyrt.a"
# THE mtime contract: the shipped lib must be >= every source in the crate,
# or ensure_staticlib() rebuilds and a Rust-less install dies on first build.
find "$DIR/runtime/rust/hyrt" -name 'target' -prune -o -print | while read -r f; do
    [ -f "$f" ] && touch "$f"
done
touch "$DIR/runtime/rust/hyrt/target/release/libhyrt.a"

# --- 4. tar + checksum ---
mkdir -p "$DIST"
( cd "$DIST" && tar czf "$NAME.tar.gz" "hybrid-${VER}" )
if command -v sha256sum >/dev/null; then
    ( cd "$DIST" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256" )
else
    ( cd "$DIST" && shasum -a 256 "$NAME.tar.gz" > "$NAME.tar.gz.sha256" )
fi

# --- 5. smoke test from the ACTUAL tarball with cargo REMOVED from PATH ---
# (the reply's bar: no Rust toolchain, ever, for users. The shipped
#  staticlib's mtime must win ensure_staticlib()'s freshness check.)
echo "==> smoke test from the tarball (PATH without cargo)"
SMOKE="$(mktemp -d)"
( cd "$SMOKE" && tar xzf "$DIST/$NAME.tar.gz" )
RUSTLESS_PATH="$(echo "$PATH" | tr ':' '\n' | grep -v '.cargo/bin' | paste -sd:)"
if ( cd "$SMOKE" && PATH="$RUSTLESS_PATH" "$SMOKE/hybrid-${VER}/hyb" build \
        "$REPO_ROOT/examples/01-hello/program.hy" -o "$SMOKE/hello" ) \
    && "$SMOKE/hello" > "$SMOKE/out.txt" 2>&1 \
    && grep -q 'hello, Hybrid!' "$SMOKE/out.txt"; then
    echo "    smoke PASS (rustless): $(head -1 "$SMOKE/out.txt")"
else
    echo "SMOKE FAILED (rustless path)" >&2
    cat "$SMOKE/out.txt" 2>/dev/null >&2
    exit 1
fi
rm -rf "$SMOKE"

echo "==> done:"
ls -la "$DIST/$NAME.tar.gz" "$DIST/$NAME.tar.gz.sha256"
