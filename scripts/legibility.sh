#!/usr/bin/env bash
# The render-based legibility gauge: every text panel on a map (and in the
# truck, and at the camp with --camp) rendered from where it's read at the
# headset's pixels a degree, then read back with OCR.
#   scripts/legibility.sh [map] [out dir] [extra game args...]
# Exit status: the number of panels OCR couldn't read (docs/LEGIBILITY.md).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-training_yard}"
OUT="${2:-target/legibility/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
DATA="$(mktemp -d)"
rm -rf "$OUT"
mkdir -p "$OUT"
WGPU_BACKEND=vulkan timeout 3600 xvfb-run -a -s "-screen 0 640x480x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --legibility "$OUT" --frames 60 \
    --weather clearing --size 640x480 "$@" 2>&1 \
    | grep -E 'ERROR|panicked|legibility:' || true
rm -rf "$DATA"
python3 scripts/legibility.py "$OUT" --sheet "$OUT/sheet.png" --crops "$OUT/crops"
