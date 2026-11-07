#!/usr/bin/env bash
# A contact sheet of rounds landing on every kind of machine, a row a kind:
# intact, and from the same view and frame, every part shot down to
# failing (`robots::wear`'s look) with a round into each part, the frame
# taken as they land
# (`robots::hit_gauge`, EXTRACTION_HIT_SHOW). The per-part gauge itself
# (effect, sound and look within 100 ms) is the test:
#
#   cargo test --lib hit_gauge -- --ignored --nocapture
#
#   scripts/hit_gauge.sh [OUT_DIR]          (default target/hit_gauge)
#   KINDS="heavy drone" scripts/hit_gauge.sh ...
#   STATES=worn ...   (one state; KEEP=1 skips frames already shot)
#
# Needs a built ./target/fast/extraction (or EXTRACTION_BIN) and the
# software Vulkan driver under Xvfb, like scripts/robot_frames.sh.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/hit_gauge}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
KINDS="${KINDS:-walker drone grappler sentry hound heavy crawler dirigible pylon sorter spotter minelayer mender plough}"
# Marrow's farm meadow, as robot_frames.sh. The gauge counts from when
# the machine first stands (some 200 frames after the start on lavapipe):
# past its STAND + SETTLE (408 more), its rounds land every 12 frames.
FRAMES=900
for kind in $KINDS; do
    # How far off and how high to look: by the machine's size (the free
    # camera's 75 degrees takes in a lot).
    case "$kind" in
        heavy|sorter) cam="92.9,0,131.9,0,-8" dist=5 ;;
        plough) cam="92.9,0,131.9,0,-10" dist=5 ;;
        dirigible) cam="92.9,0,131.9,0,30" dist=12 ;;
        sentry|spotter) cam="92.9,0,131.9,0,8" dist=4 ;;
        drone|mender) cam="92.9,0,131.9,0,6" dist=2.4 ;;
        pylon|minelayer) cam="92.9,0,131.9,0,-14" dist=3 ;;
        *) cam="92.9,0,131.9,0,-16" dist=2.6 ;;
    esac
    for state in ${STATES:-intact worn}; do
        png="$OUT/${state}_clearing_$kind.png"
        [ -n "${KEEP:-}" ] && [ -f "$png" ] && continue
        data=$(mktemp -d)
        if [ "$state" = worn ]; then
            # Shot down to failing, rounds landing as the frame is taken.
            show=1 subject="robots:$kind+hits,20,$dist"
        else
            # The same view and frame, nothing fired.
            show="" subject="robots:$kind,20,$dist"
        fi
        EXTRACTION_HIT_SHOW=$show EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1500 \
            xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
            --map marrow_creek --data-dir "$data" --new-game --no-robots \
            --weather clearing --camera "$cam" \
            --gallery "$subject" \
            --screenshot "$png" --frames $FRAMES --size 960x540 \
            > "$OUT/${state}_$kind.log" 2>&1
        rm -rf "$data"
        grep -a "hit gauge: show" "$OUT/${state}_$kind.log" | tail -1
        grep -a "panicked" "$OUT/${state}_$kind.log" | head -2
    done
done
python3 scripts/frame_check.py "$OUT"/intact_*.png "$OUT"/worn_*.png
# The sheet: a row a kind, intact | worn, from the same frame.
python3 - "$OUT" $KINDS <<'PY'
import sys
from PIL import Image, ImageDraw
out, kinds = sys.argv[1], sys.argv[2:]
cell = (480, 270)
rows = [k for k in kinds]
sheet = Image.new("RGB", (cell[0] * 2, len(rows) * (cell[1] + 18)), (20, 20, 20))
d = ImageDraw.Draw(sheet)
for i, k in enumerate(rows):
    y = i * (cell[1] + 18)
    for j, state in enumerate(["intact", "worn"]):
        try:
            im = Image.open(f"{out}/{state}_clearing_{k}.png").convert("RGB")
        except OSError:
            continue
        sheet.paste(im.resize(cell), (j * cell[0], y + 18))
        d.text((j * cell[0] + 4, y + 3), f"{k}: {state}", fill=(230, 230, 230))
sheet.save(f"{out}/sheet.png")
print(f"sheet: {out}/sheet.png")
PY
