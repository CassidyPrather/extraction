#!/usr/bin/env bash
# The render-based affordance gauge: up to three of each kind of
# interactable on a map, shot from 1.5 m with and without it at the
# headset's pixels a degree, then measured (docs/LEGIBILITY.md).
#   scripts/afford.sh [map] [out dir] [extra game args, e.g. --hour night]
# Exit status: the number of things that don't stand out from what they're on.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
MAP="${1:-training_yard}"
OUT="${2:-target/afford/$MAP}"
shift $(( $# > 2 ? 2 : $# ))
BIN="${EXTRACTION_BIN:-./target/fast/extraction}"
DATA="$(mktemp -d)"
rm -rf "$OUT"
mkdir -p "$OUT"
WGPU_BACKEND=vulkan timeout 5400 xvfb-run -a -s "-screen 0 640x480x24" \
    "$BIN" --map "$MAP" --data-dir "$DATA" --afford "$OUT" --frames 60 \
    --weather drizzle --size 640x480 "$@" 2>&1 \
    | grep -E 'ERROR|panicked|afford:' || true
rm -rf "$DATA"
python3 scripts/wayfind.py "$OUT" --afford --sheet "$OUT/sheet.png"
