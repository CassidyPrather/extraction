#!/usr/bin/env bash
# A contact sheet of rounds landing on every kind of machine: each one
# alone, close, every part shot down to failing (`robots::wear`'s look),
# then a round into each part, the frame taken as they land
# (`robots::hit_gauge`, EXTRACTION_HIT_SHOW). The per-part gauge itself
# (effect, sound and look within 100 ms) is the test:
#
#   cargo test --lib hit_gauge -- --ignored --nocapture
#
#   scripts/hit_gauge.sh [OUT_DIR]          (default target/hit_gauge)
#   KINDS="heavy drone" scripts/hit_gauge.sh ...
#
# Needs a built ./target/fast/extraction (or EXTRACTION_BIN) and the
# software Vulkan driver under Xvfb, like scripts/robot_frames.sh.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/hit_gauge}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
KINDS="${KINDS:-walker drone grappler sentry hound heavy crawler dirigible pylon sorter spotter minelayer mender plough}"
# Marrow's farm meadow, as robot_frames.sh. Past the gauge's STAND +
# SETTLE (408 frames of the line-up standing), its rounds landing.
FRAMES=640
for kind in $KINDS; do
    # How far off and how high to look: by the machine's size.
    case "$kind" in
        heavy|sorter) cam="92.9,0,131.9,0,-6" dist=7 ;;
        dirigible) cam="92.9,0,131.9,0,28" dist=14 ;;
        pylon|plough) cam="92.9,0,131.9,0,-8" dist=6 ;;
        drone|mender) cam="92.9,0,131.9,0,4" dist=4 ;;
        *) cam="92.9,0,131.9,0,-10" dist=4.5 ;;
    esac
    data=$(mktemp -d)
    EXTRACTION_HIT_SHOW=1 EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1500 \
        xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
        --map marrow_creek --data-dir "$data" --new-game --no-robots \
        --weather clearing --camera "$cam" \
        --gallery "robots:$kind+hits,20,$dist" \
        --screenshot "$OUT/hits_clearing_$kind.png" --frames $FRAMES --size 960x540 \
        > "$OUT/$kind.log" 2>&1
    rm -rf "$data"
    grep -a "hit gauge: show" "$OUT/$kind.log" | tail -1
    grep -a "panicked" "$OUT/$kind.log" | head -2
done
python3 scripts/frame_check.py --sheet "$OUT/sheet.png" "$OUT"/hits_*.png
