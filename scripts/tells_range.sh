#!/usr/bin/env bash
# How a failing machine reads against an intact one at fighting range:
# each kind alone, 10, 20 and 30 m off, intact and with every part failing
# (`robots::hit_gauge`, EXTRACTION_HIT_SHOW=failing: nothing fired), from
# the same view and frame. For each pair, the pixels that changed by more
# than 12 levels in the machine's column (frame_check's measure), at the
# 1080-line view's scale, against a 0.4 degree square's 33 px; and a sheet,
# a row a kind and range, intact | failing.
#
#   scripts/tells_range.sh [OUT_DIR]        (default target/tells_range)
#   KINDS="heavy minelayer" DISTS="20" KEEP=1 scripts/tells_range.sh ...
#
# Needs a built ./target/fast/extraction (or EXTRACTION_BIN) and the
# software Vulkan driver under Xvfb, like scripts/hit_gauge.sh.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/tells_range}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
KINDS="${KINDS:-heavy spotter minelayer sorter}"
DISTS="${DISTS:-10 20 30}"
# Past the gauge's STAND (300 frames of the line-up standing) and its
# tells' settling.
FRAMES=700
for kind in $KINDS; do
    for d in $DISTS; do
        # A spotter flies its balloon some 23 m up: look up to it.
        case "$kind:$d" in
            spotter:10) cam="92.9,0,131.9,0,64" ;;
            spotter:20) cam="92.9,0,131.9,0,47" ;;
            spotter:30) cam="92.9,0,131.9,0,37" ;;
            *) cam="92.9,0,131.9,0,-2" ;;
        esac
        for state in intact failing; do
            png="$OUT/${state}_${kind}_${d}.png"
            [ -n "${KEEP:-}" ] && [ -f "$png" ] && continue
            data=$(mktemp -d)
            if [ "$state" = failing ]; then
                show=failing subject="robots:$kind+hits,20,$d"
            else
                show="" subject="robots:$kind,20,$d"
            fi
            EXTRACTION_HIT_SHOW=$show EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1500 \
                xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
                --map marrow_creek --data-dir "$data" --new-game --no-robots \
                --weather clearing --camera "$cam" \
                --gallery "$subject" \
                --screenshot "$png" --frames $FRAMES --size 960x540 \
                > "$OUT/${state}_${kind}_${d}.log" 2>&1
            rm -rf "$data"
            grep -a "panicked" "$OUT/${state}_${kind}_${d}.log" | head -2
        done
    done
done
python3 - "$OUT" "$KINDS" "$DISTS" <<'PY'
import sys
from PIL import Image, ImageDraw
out, kinds, dists = sys.argv[1], sys.argv[2].split(), sys.argv[3].split()
CHANGED = 12
# 960x540 here; the bar's view is 1080 lines: a pixel here is four there.
SCALE = (1080 / 540) ** 2
BAR = 33
cell = (480, 270)
rows = [(k, d) for k in kinds for d in dists]
sheet = Image.new("RGB", (cell[0] * 2, len(rows) * (cell[1] + 18)), (20, 20, 20))
draw = ImageDraw.Draw(sheet)
short = 0
for i, (k, d) in enumerate(rows):
    try:
        a = Image.open(f"{out}/intact_{k}_{d}.png").convert("RGB")
        b = Image.open(f"{out}/failing_{k}_{d}.png").convert("RGB")
    except OSError as e:
        print(f"{k:<10} {d:>2} m  missing: {e}")
        short += 1
        continue
    w, h = a.size
    # The machine's column: it stands in the middle of the view.
    box = (int(w * 0.35), 0, int(w * 0.65), int(h * 0.85))
    pa, pb = a.convert("L").crop(box).tobytes(), b.convert("L").crop(box).tobytes()
    changed = sum(1 for p, q in zip(pa, pb) if abs(p - q) > CHANGED) * SCALE
    ok = changed >= BAR
    short += not ok
    print(f"{k:<10} {d:>2} m  changed {changed:7.0f} px (1080-line)  {'ok' if ok else 'UNDER ' + str(BAR)}")
    y = i * (cell[1] + 18)
    for j, (im, label) in enumerate([(a, "intact"), (b, "failing")]):
        sheet.paste(im.resize(cell), (j * cell[0], y + 18))
        draw.text((j * cell[0] + 4, y + 3), f"{k} {d} m: {label}" + (f"  changed {changed:.0f} px" if j else ""), fill=(230, 230, 230))
sheet.save(f"{out}/sheet.png")
print(f"sheet: {out}/sheet.png")
sys.exit(1 if short else 0)
PY
