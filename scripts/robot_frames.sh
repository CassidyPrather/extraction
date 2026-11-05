#!/usr/bin/env bash
# Every kind of machine drawn, standing and wrecked, in clear weather and
# in a downpour: a frame for each group of four, and the gauge fails if
# any frame is blank (scripts/frame_check.py). A wreck once blanked its
# frame (the body's gallery took the camera under the ground); this
# catches that whole class.
#
#   scripts/robot_frames.sh [OUT_DIR]       (default target/robot_frames)
#   WEATHERS="clearing" scripts/robot_frames.sh ...   (one weather a run;
#   the check covers every frame in OUT_DIR)
#
# Needs a built ./target/fast/extraction (or EXTRACTION_BIN) and the
# software Vulkan driver under Xvfb, like scripts/shot.sh. About 3 min a
# frame on lavapipe; 16 frames.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/robot_frames}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
LINEUPS=(
    "walker+drone+grappler+hound"
    "heavy+sentry+crawler+dirigible"
    "pylon+sorter+spotter+minelayer"
    "mender+plough"
)
# Marrow's farm meadow (lookbook m03), on the map, open ground ahead.
CAM="92.9,0,131.9,315,-4"
frames=()
under=0
# The view with no machines, a weather each: a frame of machines must
# differ from it where they stand, or they weren't drawn.
for weather in ${WEATHERS:-clearing downpour}; do
    data=$(mktemp -d)
    EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1200 \
        xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
        --map marrow_creek --data-dir "$data" --new-game --no-robots \
        --weather "$weather" --camera "$CAM" \
        --screenshot "$OUT/empty_$weather.png" --frames 420 --size 960x540 \
        > "$OUT/empty_$weather.log" 2>&1
    rm -rf "$data"
done
for weather in ${WEATHERS:-clearing downpour}; do
    for g in "${LINEUPS[@]}"; do
        for state in standing wrecked; do
            spec="$g"
            [ "$state" = wrecked ] && spec="$(echo "$g" | sed 's/+/!dead+/g')!dead"
            name="${state}_${weather}_${g//+/-}"
            png="$OUT/$name.png"
            data=$(mktemp -d)
            # (The shelf off: a frame is about what's drawn, not when.)
            EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1200 \
                xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
                --map marrow_creek --data-dir "$data" --new-game --no-robots \
                --weather "$weather" --camera "$CAM" \
                --gallery "robots:$spec,0,17" \
                --screenshot "$png" --frames 420 --size 960x540 \
                > "$OUT/$name.log" 2>&1
            rm -rf "$data"
            grep -a "panicked" "$OUT/$name.log" | head -2
            # The camera stayed over the ground (`dev::camera_over_ground`).
            if grep -aq "camera: .* under the ground" "$OUT/$name.log"; then
                echo "UNDER  $name: $(grep -a -o 'camera: .* under the ground.*' "$OUT/$name.log" | head -1)"
                under=$((under + 1))
            fi
            frames+=("$png")
        done
    done
done
all=$(ls "$OUT"/standing_*.png "$OUT"/wrecked_*.png 2>/dev/null)
python3 scripts/frame_check.py --sheet "$OUT/sheet.png" --empty "$OUT/empty_" $all
blank=$?
under=$(grep -la "camera: .* under the ground" "$OUT"/*.log 2>/dev/null | wc -l)
echo "camera under the ground in $under runs"
[ "$blank" -eq 0 ] && [ "$under" -eq 0 ]
