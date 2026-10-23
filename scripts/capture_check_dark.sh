#!/usr/bin/env bash
# Is a night shot dark? Shoots one view by day and at `--hour 23` with the
# screenshot tooling (scripts/shot.sh's flags) and fails unless the night
# is measurably dark: mean luminance under FLOOR, and under half the
# day's. `--hour 23` once fell through to the raid's own hour and shot
# daylight.
#   scripts/capture_check_dark.sh [map] [camera x,y,z,yaw,pitch] [more game flags]
# Env: FLOOR (mean luminance 0-1, default 0.2; a night is kept readable,
# about 0.13 at Marrow's gate road, where the old bug read 0.38 as by day), OUT (default
# target/check_dark), EXTRACTION_BIN (default ./target/fast/extraction).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
CAM="${2:-40,0,262,0,-6}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
. scripts/capture_common.sh
capture_check_bin "$BIN"
OUT="${OUT:-target/check_dark}"
mkdir -p "$OUT"
for HOUR in 12 23; do
    DATA="$(mktemp -d)"
    rm -f "$OUT/$MAP-$HOUR.png"
    WGPU_BACKEND=vulkan timeout 900 xvfb-run -a -s "-screen 0 960x540x24" \
        "$BIN" --map "$MAP" --screenshot "$OUT/$MAP-$HOUR.png" --frames 60 \
        --camera "$CAM" --hour "$HOUR" --weather clearing --no-robots \
        --data-dir "$DATA" --new-game --size 960x540 "$@" 2>&1 \
        | grep -E 'ERROR|panicked' | grep -v slab_allocator || true
    rm -rf "$DATA"
done
python3 - "$OUT/$MAP-12.png" "$OUT/$MAP-23.png" "${FLOOR:-0.2}" <<'EOF'
import sys
from PIL import Image, ImageStat

def luma(path):
    im = Image.open(path).convert("L")
    return ImageStat.Stat(im).mean[0] / 255.0

day, night, floor = luma(sys.argv[1]), luma(sys.argv[2]), float(sys.argv[3])
print(f"mean luminance: hour 12 {day:.3f}, hour 23 {night:.3f} (floor {floor})")
bad = []
if night >= floor:
    bad.append(f"hour 23 at {night:.3f} is not under {floor}")
if night >= day / 2:
    bad.append(f"hour 23 at {night:.3f} is not under half of hour 12 ({day:.3f})")
if bad:
    print("FAIL: " + "; ".join(bad))
    sys.exit(1)
print("ok: the night is dark")
EOF
