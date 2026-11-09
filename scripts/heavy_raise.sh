#!/usr/bin/env bash
# Whether a heavy's blow reads at fighting range: its arm at rest against
# the same heavy halfway up its tell (`raise50`, 0.65 s into the 1.3 s
# RAISE_TIME: half the warning left), 5, 10 and 15 m off, facing you, by
# day, at night, in fog and in a downpour, from the same view and frame.
# For each pair, the pixels that changed by more than 12 levels in the
# heavy's column (frame_check's measure), at the 1080-line view's scale;
# and a sheet, a row a condition and range, rest | raised.
#
#   scripts/heavy_raise.sh [OUT_DIR]        (default target/heavy_raise)
#   CONDS="clearing:night" DISTS="10" RAISE=30 KEEP=1 JOBS=2 scripts/heavy_raise.sh
#
# Needs a built ./target/fast/extraction (or EXTRACTION_BIN) and the
# software Vulkan driver under Xvfb, like scripts/tells_range.sh.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/heavy_raise}"
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
mkdir -p "$OUT"
CONDS="${CONDS:-clearing:day clearing:night fog:day downpour:day}"
DISTS="${DISTS:-5 10 15}"
RAISE="${RAISE:-50}"
JOBS="${JOBS:-2}"
# Past the weather's settling and the raise's hold (from frame 30).
FRAMES=400
shot() {
    local cond=$1 d=$2 state=$3
    local weather=${cond%%:*} hour=${cond##*:}
    local png="$OUT/${state}_${weather}_${hour}_${d}.png"
    [ -n "${KEEP:-}" ] && [ -f "$png" ] && return
    # Up a little close in, so the raised claw stays in frame.
    local pitch=-2
    [ "$d" -le 5 ] && pitch=12
    local gal="heavy"
    [ "$state" = raised ] && gal="heavy:raise$RAISE"
    local data
    data=$(mktemp -d)
    EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan timeout 1500 \
        xvfb-run -a -s "-screen 0 960x540x24" "$BIN" \
        --map marrow_creek --data-dir "$data" --new-game --no-robots \
        --weather "$weather" --hour "$hour" --camera "92.9,0,131.9,0,$pitch" \
        --gallery "$gal,0,$d" \
        --screenshot "$png" --frames $FRAMES --size 960x540 \
        > "${png%.png}.log" 2>&1
    rm -rf "$data"
    grep -a "panicked" "${png%.png}.log" | head -2
}
export -f shot
export OUT BIN FRAMES RAISE KEEP
for c in $CONDS; do for d in $DISTS; do for s in rest raised; do
    echo "$c $d $s"
done; done; done | xargs -P "$JOBS" -L 1 bash -c 'shot "$0" "$1" "$2"'
python3 - "$OUT" "$CONDS" "$DISTS" <<'PY'
import sys
from PIL import Image, ImageDraw
out, conds, dists = sys.argv[1], sys.argv[2].split(), sys.argv[3].split()
CHANGED = 12
SCALE = (1080 / 540) ** 2
cell = (480, 270)
rows = [(c, d) for c in conds for d in dists]
sheet = Image.new("RGB", (cell[0] * 2, len(rows) * (cell[1] + 18)), (20, 20, 20))
draw = ImageDraw.Draw(sheet)
missing = 0
for i, (c, d) in enumerate(rows):
    w_, h_ = c.split(":")
    try:
        a = Image.open(f"{out}/rest_{w_}_{h_}_{d}.png").convert("RGB")
        b = Image.open(f"{out}/raised_{w_}_{h_}_{d}.png").convert("RGB")
    except OSError as e:
        print(f"{c:<15} {d:>2} m  missing: {e}")
        missing += 1
        continue
    w, h = a.size
    box = (int(w * 0.25), 0, int(w * 0.75), h)
    pa, pb = a.convert("L").crop(box).tobytes(), b.convert("L").crop(box).tobytes()
    changed = sum(1 for p, q in zip(pa, pb) if abs(p - q) > CHANGED) * SCALE
    print(f"{c:<15} {d:>2} m  changed {changed:7.0f} px (1080-line)")
    y = i * (cell[1] + 18)
    for j, (im, label) in enumerate([(a, "rest"), (b, "raised")]):
        sheet.paste(im.resize(cell), (j * cell[0], y + 18))
        draw.text((j * cell[0] + 4, y + 3), f"{c} {d} m: {label}" + (f"  changed {changed:.0f} px" if j else ""), fill=(230, 230, 230))
sheet.save(f"{out}/sheet.png")
print(f"sheet: {out}/sheet.png")
sys.exit(1 if missing else 0)
PY
