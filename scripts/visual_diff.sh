#!/usr/bin/env bash
# Visual regression sweep: the same cameras in every valley, shot at day,
# dusk, night and fog by two builds, compared for what changed.
#   scripts/visual_diff.sh [old_sha new_sha]
# Without shas: main's tip against the end of the round before (the last
# first-parent commit before main's latest burst of merges; MAIN_REF,
# default origin/claude/serene-clarke-rcgj62).
#
# Each sha is built once, in a worktree at target/vdiff/wt (always the same
# path, so the shared target/vdiff/build stays incremental). It is kept as
# target/vdiff/bin/<sha>/ with a copy of its assets, which links the
# generated textures in from this checkout because they are gitignored.
# Its shots go to target/vdiff/shots/<sha>/<map>/<mood>/<view>.png at
# 960x540. Both are reused next round and pruned to the newest KEEP
# (default 3). The cameras are scripts/visual_diff/<map>.stills,
# committed. Remake them with scripts/visual_diff_cameras.sh when a valley
# changes under them.
#
# Output: target/vdiff/diff/<old>_<new>/sheet.jpg (changed shots side by
# side, most changed first), index.html and report.json. Env: MAPS,
# MOODS, KEEP, THRESHOLD (see scripts/visual_diff.py).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ROOT="$(pwd)"
VD="$ROOT/target/vdiff"
MAIN_REF="${MAIN_REF:-origin/claude/serene-clarke-rcgj62}"
MAPS=(${MAPS:-marrow_creek kettle_pass cinder_works sallow_reservoir rime_fell})
MOODS=(${MOODS:-day dusk night fog})
KEEP="${KEEP:-3}"
mkdir -p "$VD/shots" "$VD/bin" "$VD/diff"

if [ $# -ge 2 ]; then
    OLD="$(git rev-parse --short=8 "$1")"
    NEW="$(git rev-parse --short=8 "$2")"
else
    git fetch -q origin "${MAIN_REF#origin/}" || true
    NEW="$(git rev-parse --short=8 "$MAIN_REF")"
    # The round before: walk main's first parents back past the latest
    # burst of merges (commits within 45 min of each other) to the first
    # commit before it.
    OLD="$(git log --first-parent --format='%h %ct %s' "$MAIN_REF" | python3 -c '
import sys
rows = [l.split(" ", 2) for l in sys.stdin.read().splitlines()]
seen_merge = False
prev_t = None
for h, t, s in rows:
    t = int(t)
    if seen_merge and prev_t is not None and prev_t - t > 45 * 60:
        print(h[:8]); break
    if s.startswith("Merge polish"):
        seen_merge = True
    prev_t = t
')"
fi
echo "visual diff: $OLD -> $NEW"

build() { # sha
    local sha="$1" out="$VD/bin/$1"
    [ -x "$out/extraction" ] && return
    echo "building $sha"
    git worktree remove --force "$VD/wt" 2>/dev/null || rm -rf "$VD/wt"
    git worktree prune
    git worktree add -q --detach "$VD/wt" "$sha"
    for d in textures/foliage textures/materials textures/sky textures/terrain .cache; do
        if [ -e "$ROOT/assets/$d" ] && [ ! -e "$VD/wt/assets/$d" ]; then
            ln -s "$ROOT/assets/$d" "$VD/wt/assets/$d"
        fi
    done
    (cd "$VD/wt" && CARGO_TARGET_DIR="$VD/build" cargo build --profile fast 2>&1 | tail -1)
    mkdir -p "$out"
    cp "$VD/build/fast/extraction" "$out/extraction"
    # A binary finds assets/ next to itself first (a packaged build).
    cp -a "$VD/wt/assets" "$out/assets"
    git worktree remove --force "$VD/wt"
}

shoot() { # sha
    local sha="$1" dir="$VD/shots/$1"
    [ -f "$dir/.done" ] && { echo "shots for $sha: reused"; return; }
    build "$sha"
    for MAP in "${MAPS[@]}"; do
        for MOOD in "${MOODS[@]}"; do
            case "$MOOD" in
                day) ARGS=(--hour day --weather clearing) ;;
                dusk) ARGS=(--hour dusk --weather clearing) ;;
                night) ARGS=(--hour night --weather clearing) ;;
                fog) ARGS=(--hour day --weather fog) ;;
                *) echo "unknown mood $MOOD"; exit 1 ;;
            esac
            local out="$dir/$MAP/$MOOD" data
            mkdir -p "$out"
            data="$(mktemp -d)"
            WGPU_BACKEND=vulkan timeout 1800 xvfb-run -a -s "-screen 0 960x540x24" \
                "$VD/bin/$sha/extraction" --map "$MAP" --data-dir "$data" --new-game \
                --no-robots --seed 7 --cinematic "$ROOT/scripts/visual_diff/$MAP.stills" \
                --capture-out "$out" --frames 60 --size 960x540 "${ARGS[@]}" 2>&1 \
                | grep -E 'panicked' || true
            rm -rf "$data"
        done
        echo "$sha $MAP: $(find "$dir/$MAP" -name '*.png' | wc -l) shots"
    done
    touch "$dir/.done"
}

shoot "$OLD"
shoot "$NEW"
python3 "$ROOT/scripts/visual_diff.py" "$VD/shots/$OLD" "$VD/shots/$NEW" "$VD/diff/${OLD}_$NEW" \
    --title "$OLD -> $NEW"

# Keep the newest KEEP shot sets and builds (and the two just compared).
prune() { # dir
    ls -1t "$1" | tail -n +"$((KEEP + 1))" | while read -r old; do
        [ "$old" = "$OLD" ] || [ "$old" = "$NEW" ] || rm -rf "${1:?}/$old"
    done
}
prune "$VD/shots"
prune "$VD/bin"
prune "$VD/diff"
