#!/usr/bin/env bash
# The render-based wayfinding gauge: from the truck's stop and every
# rendezvous on a map, each landmark in range shot with and without it at
# the headset's pixels a degree, then measured (docs/LEGIBILITY.md).
#   scripts/wayfind.sh [map] [out dir] [extra game args, e.g. --weather fog --hour night]
# Exit status: the number of vantages with no landmark to steer by.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-marrow_creek}"
OUT="${2:-target/wayfind/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
DATA="$(mktemp -d)"
rm -rf "$OUT"
mkdir -p "$OUT"
WGPU_BACKEND=vulkan timeout 5400 xvfb-run -a -s "-screen 0 640x480x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --wayfind "$OUT" --frames 60 \
    --weather drizzle --size 640x480 "$@" 2>&1 \
    | grep -E 'ERROR|panicked|wayfind:' || true
rm -rf "$DATA"
python3 scripts/wayfind.py "$OUT" --sheet "$OUT/sheet.png"
