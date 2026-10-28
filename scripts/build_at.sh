#!/usr/bin/env bash
# Builds the game at a commit, once, and prints where the binary is.
#   scripts/build_at.sh <sha>   ->   target/vdiff/bin/<sha8>/extraction
# The build is in a worktree at target/vdiff/wt (always the same path, so
# the shared target/vdiff/build stays incremental) and is kept as
# target/vdiff/bin/<sha8>/ with a copy of its assets. That copy links the
# generated textures in from this checkout, because they are gitignored.
# Used by scripts/visual_diff.sh and scripts/before_after.py. Build output
# goes to stderr; only the path goes to stdout.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ROOT="$(pwd)"
VD="$ROOT/target/vdiff"
SHA="$(git rev-parse --short=8 "$1")"
OUT="$VD/bin/$SHA"
if [ ! -x "$OUT/extraction" ]; then
    {
        echo "building $SHA"
        mkdir -p "$VD/bin"
        git worktree remove --force "$VD/wt" 2>/dev/null || rm -rf "$VD/wt"
        git worktree prune
        git worktree add -q --detach "$VD/wt" "$SHA"
        for d in textures/foliage textures/materials textures/sky textures/terrain .cache; do
            if [ -e "$ROOT/assets/$d" ] && [ ! -e "$VD/wt/assets/$d" ]; then
                ln -s "$ROOT/assets/$d" "$VD/wt/assets/$d"
            fi
        done
        (cd "$VD/wt" && CARGO_TARGET_DIR="$VD/build" cargo build --profile fast 2>&1 | tail -1)
        mkdir -p "$OUT"
        cp "$VD/build/fast/extraction" "$OUT/extraction"
        # A binary finds assets/ next to itself first (a packaged build).
        cp -a "$VD/wt/assets" "$OUT/assets"
        git worktree remove --force "$VD/wt"
    } >&2
fi
echo "$OUT/extraction"
