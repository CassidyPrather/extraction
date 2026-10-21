#!/usr/bin/env bash
# The "what's new" gallery: turntables, stills by day and night, a few
# reels, and before/after pairs for fixes you can see. Shoots into
# target/new/, then scripts/capture_new.py lays out docs/shots/capture/new/.
#   scripts/capture_new.sh [parts...]   (default: turn after before reels)
# Env:
#   BEFORE_BIN  a binary built from the tree with the fixes taken out (see
#               docs/CAPTURE.md, "What's new"), for the before half of the
#               pairs; without it the pairs are skipped.
#   TURN        turntable frames (default 36).
# The subjects and views are in this script and scripts/capture_paths/new/.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT=target/new
PARTS=("$@")
[ ${#PARTS[@]} -eq 0 ] && PARTS=(turn after before reels)
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"

# New models: one turn each, by day and by night.
TURNS="robots/spotter,robots/minelayer,robots/mender,robots/plough,props/gramophone,props/upright_piano"
# Valleys with new views (scripts/capture_paths/new/<map>.stills).
AFTER_MAPS=(marrow_creek kettle_pass sallow_reservoir rime_fell)
# The views that show a fix; shot again by BEFORE_BIN.
BEFORE_MAPS=(marrow_creek kettle_pass sallow_reservoir)

reel() { # name map camera reel [hour]
    local name="$1" map="$2" cam="$3" what="$4" hour="${5:-day}"
    local dir="$OUT/reels/$name"
    mkdir -p "$dir"
    local data
    data="$(mktemp -d)"
    WGPU_BACKEND=vulkan timeout 1800 xvfb-run -a -s "-screen 0 1280x720x24" \
        "$BIN" --map "$map" --screenshot "$dir/f.png" --frames 99999 --camera "$cam" \
        --no-robots --data-dir "$data" --new-game --hour "$hour" --weather clearing \
        --gallery "reel:$what" --size 1280x720 2>&1 | grep -E 'panicked' || true
    rm -rf "$data"
    echo "reel $name: $(ls "$dir"/f_*.png 2>/dev/null | wc -l) frames"
}

for PART in "${PARTS[@]}"; do
    case "$PART" in
        turn)
            LIGHTS=day,night TURN="${TURN:-36}" SIZE=960x720 \
                scripts/capture_sweep.sh "$TURNS" "$OUT/turn"
            ;;
        after)
            for MAP in "${AFTER_MAPS[@]}"; do
                ARGS=""
                [ "$MAP" = marrow_creek ] && ARGS="--gallery minefields"
                OUT="$OUT/after" MOODS="day night" SIZE=1600x900 \
                    STILLS_DIR=scripts/capture_paths/new GAME_ARGS="$ARGS" \
                    scripts/capture_presskit.sh "$MAP"
            done
            ;;
        before)
            if [ -z "${BEFORE_BIN:-}" ]; then
                echo "no BEFORE_BIN: skipping the before shots"
                continue
            fi
            for MAP in "${BEFORE_MAPS[@]}"; do
                ARGS=""
                [ "$MAP" = marrow_creek ] && ARGS="--gallery minefields"
                EXTRACTION_BIN="$BEFORE_BIN" OUT="$OUT/before" MOODS=day SIZE=1600x900 \
                    STILLS_DIR=scripts/capture_paths/new GAME_ARGS="$ARGS" \
                    scripts/capture_presskit.sh "$MAP"
            done
            ;;
        reels)
            # A walker on a shot leg, one with its eye shot dark, one with
            # its core failing, and a drone with a rotor hit, at 24-36 m.
            reel tells_dusk marrow_creek 40,0,262,0,-6 tells,6,30 dusk
            reel tells_day marrow_creek 40,0,262,0,-6 tells,6,30 day
            # A walker leaving its prints in the lying snow at Rime's col.
            reel snow_prints rime_fell 60,0,-250,0,-14 walk,8,30 day
            ;;
        *) echo "unknown part $PART"; exit 1 ;;
    esac
done
python3 scripts/capture_new.py "$OUT" docs/shots/capture/new
