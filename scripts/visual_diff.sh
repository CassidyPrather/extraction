#!/usr/bin/env bash
# Visual regression sweep: the same cameras in every valley, shot at day,
# dusk, night and fog by two builds, compared for what changed.
#   scripts/visual_diff.sh [--quick] [old_sha new_sha]
# --quick is for right after a gate: one camera a valley (the landmark,
# sky and lights in it) at dusk and night only, the moods that broke,
# about 10 min on warm builds. The full sweep is for nightly use. Either
# ends with one line, PASS or CHANGED, and the sheet's path.
# Without shas: main's tip against the end of the round before: the
# first-parent commit under main's latest round, its latest cluster of
# three or more merges within ten minutes. MAIN_REF defaults to
# origin/claude/serene-clarke-rcgj62.
#
# Each sha is built once (scripts/build_at.sh, kept in
# target/vdiff/bin/<sha>/). Its shots go to target/vdiff/shots/<sha>/<map>/<mood>/<view>.png at
# 960x540. Both are reused next round and pruned to the newest KEEP
# (default 3). The cameras are scripts/visual_diff/<map>.stills,
# committed. Remake them with scripts/visual_diff_cameras.sh when a valley
# changes under them.
#
# Output: target/vdiff/diff/<old>_<new>/sheet.jpg (changed shots side by
# side, most changed first), index.html and report.json. Then a second
# line, BUILDINGS PASS or BUILDINGS MISSING: every building within 60 m of
# a camera must change its patch of the picture when hidden
# (scripts/check_buildings.py), or it was drawn somewhere nobody can see. Env: MAPS,
# MOODS, KEEP, STRUCTURE and PIXELS (thresholds; see scripts/visual_diff.py).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ROOT="$(pwd)"
VD="$ROOT/target/vdiff"
MAIN_REF="${MAIN_REF:-origin/claude/serene-clarke-rcgj62}"
QUICK=0
if [ "${1:-}" = "--quick" ]; then
    QUICK=1
    shift
fi
MAPS=(${MAPS:-marrow_creek kettle_pass cinder_works sallow_reservoir rime_fell})
if [ "$QUICK" = 1 ]; then
    MOODS=(${MOODS:-dusk night})
    VIEWS="${VIEWS:-landmark}"
else
    MOODS=(${MOODS:-day dusk night fog})
    VIEWS="${VIEWS:-}"
fi
KEEP="${KEEP:-3}"
mkdir -p "$VD/shots" "$VD/bin" "$VD/diff"

if [ $# -ge 2 ]; then
    OLD="$(git rev-parse --short=8 "$1")"
    NEW="$(git rev-parse --short=8 "$2")"
else
    case "$MAIN_REF" in
        origin/*) git fetch -q origin "${MAIN_REF#origin/}" || true ;;
    esac
    NEW="$(git rev-parse --short=8 "$MAIN_REF")"
    # The round before: main's latest round is its latest cluster of three
    # or more "Merge polish" commits within ten minutes; the round before
    # ends at the first-parent commit just under it.
    OLD="$(git log --first-parent --format='%h %ct %s' "$MAIN_REF" | python3 -c '
import sys
rows = [l.split(" ", 2) for l in sys.stdin.read().splitlines()]
rows = [(h, int(t), s) for h, t, s in rows]
for i, (h, t, s) in enumerate(rows):
    if not s.startswith("Merge polish"):
        continue
    burst = [j for j in range(i, len(rows)) if rows[j][2].startswith("Merge polish") and t - rows[j][1] <= 600]
    if len(burst) >= 3:
        last = burst[-1]
        while last + 1 < len(rows) and t - rows[last + 1][1] <= 600:
            last += 1
        print(rows[last + 1][0][:8])
        break
')"
fi
echo "visual diff: $OLD -> $NEW"

build() { # sha (scripts/build_at.sh)
    "$ROOT/scripts/build_at.sh" "$1" > /dev/null
}

# Shots are kept a file at a time, so a quick run (or a bisect on one
# shot) reuses what a full one made, and the other way round.
want() { # sha map mood: are any wanted shots missing?
    local dir="$VD/shots/$1/$2/$3" v
    for v in $(grep -v '^#' "$ROOT/scripts/visual_diff/$2.stills" | awk 'NF {print $1}'); do
        if [ -z "$VIEWS" ] || [ "$v" = "$VIEWS" ]; then
            [ -f "$dir/$v.png" ] || return 0
        fi
    done
    return 1
}

shoot() { # sha
    local sha="$1" dir="$VD/shots/$1"
    for MAP in "${MAPS[@]}"; do
        for MOOD in "${MOODS[@]}"; do
            want "$sha" "$MAP" "$MOOD" || continue
            build "$sha"
            case "$MOOD" in
                day) ARGS=(--hour day --weather clearing) ;;
                dusk) ARGS=(--hour dusk --weather clearing) ;;
                night) ARGS=(--hour night --weather clearing) ;;
                fog) ARGS=(--hour day --weather fog) ;;
                *) echo "unknown mood $MOOD"; exit 1 ;;
            esac
            local out="$dir/$MAP/$MOOD" data only=()
            [ -n "$VIEWS" ] && only=(--cine-only "$VIEWS")
            mkdir -p "$out"
            data="$(mktemp -d)"
            WGPU_BACKEND=vulkan timeout 1800 xvfb-run -a -s "-screen 0 960x540x24" \
                "$VD/bin/$sha/extraction" --map "$MAP" --data-dir "$data" --new-game \
                --no-robots --seed 7 --cinematic "$ROOT/scripts/visual_diff/$MAP.stills" \
                --capture-out "$out" --frames 60 --size 960x540 --check-buildings 1 \
                "${only[@]}" "${ARGS[@]}" 2>&1 \
                | grep -E 'panicked' || true
            rm -rf "$data"
        done
    done
    touch "$dir"
}

shoot "$OLD"
shoot "$NEW"
TAG="${OLD}_$NEW"
[ "$QUICK" = 1 ] && TAG="${TAG}_quick"
python3 "$ROOT/scripts/visual_diff.py" "$VD/shots/$OLD" "$VD/shots/$NEW" "$VD/diff/$TAG" \
    --title "$OLD -> $NEW" --maps "${MAPS[*]}" --moods "${MOODS[*]}" --views "$VIEWS"
# Every building near a camera really there to see, in the new build's
# shots (--check-buildings: each one hidden in turn, shot again). A build
# from before the check has no probes, and passes it by default.
CHECK=()
for MAP in "${MAPS[@]}"; do
    for MOOD in "${MOODS[@]}"; do
        CHECK+=("$VD/shots/$NEW/$MAP/$MOOD")
    done
done
if python3 "$ROOT/scripts/check_buildings.py" "${CHECK[@]}" > "$VD/diff/$TAG/buildings.txt"; then
    echo "BUILDINGS PASS $NEW: $(tail -1 "$VD/diff/$TAG/buildings.txt")"
else
    echo "BUILDINGS MISSING $NEW: $(tail -1 "$VD/diff/$TAG/buildings.txt")"
    grep '^ERROR' "$VD/diff/$TAG/buildings.txt"
fi
# Buildings it couldn't judge (a sliver, behind something, black fog):
# never counted as there.
grep '^SKIPPED' "$VD/diff/$TAG/buildings.txt" || true

# Keep the newest KEEP shot sets and builds (and the two just compared).
prune() { # dir
    ls -1t "$1" | tail -n +"$((KEEP + 1))" | while read -r old; do
        [ "$old" = "$OLD" ] || [ "$old" = "$NEW" ] || rm -rf "${1:?}/$old"
    done
}
prune "$VD/shots"
prune "$VD/bin"
prune "$VD/diff"
